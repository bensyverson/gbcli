use gbcli_bundle::SessionStore;
use gbcli_core::{Gap, Region};
use gbcli_engine::cart::cgb_flag;
use gbcli_engine::cond::{Condition, EvalCtx, EvalWarning, ExprError, oam_entry};
use gbcli_engine::inspect::video::{ObjectAttributes, ObjectPalette, object_attributes};
use gbcli_engine::{Area, Machine, Since};
use gbcli_protocol::{
    ErrorBody, ErrorCode, EventsArgs, MachineView, OamEntryRead, PpuView, ReadArgs, ReadFailed,
    ReadValue, RegistersView, ResultBody, SessionFile, TimeBase, TimeMark,
};

use super::{Failure, Handler, Need};
use crate::convert::{self};
impl Handler {
    pub(super) fn show(
        &mut self,
        store: &SessionStore,
        meta: &SessionFile,
        warnings: &mut Vec<String>,
    ) -> Result<ResultBody, Failure> {
        let (machine, loaded) = self.machine_for(store, meta, Need::Head, None)?;
        warnings.extend(loaded);
        // The ROM the machine loaded, not the file on disk, which a rebuild
        // may have replaced since.
        let rom = machine.core().region(Region::Rom).ok();
        let result = ResultBody {
            machine: Some(MachineView {
                model: meta.model,
                cgb: rom.and_then(|r| cgb_flag(r.bytes)),
            }),
            registers: Some(RegistersView::of(&machine.core().registers())),
            ppu: Some(PpuView::of(&machine.core().ppu())),
            debugger: Some(machine.debugger_config().clone()),
            stop: meta.last_stop.clone(),
            ..ResultBody::default()
        };
        self.keep_warm(store, meta, machine);
        Ok(result)
    }

    pub(super) fn events(
        &self,
        store: &SessionStore,
        args: &EventsArgs,
    ) -> Result<ResultBody, Failure> {
        let rows = store
            .events(args.limit)
            .map_err(|e| Failure::from_store(&e))?;
        Ok(ResultBody {
            events: Some(rows),
            ..ResultBody::default()
        })
    }

    pub(super) fn read(
        &mut self,
        store: &SessionStore,
        meta: &SessionFile,
        args: &ReadArgs,
        warnings: &mut Vec<String>,
    ) -> Result<ResultBody, Failure> {
        let since = self.since(store, args.since.as_ref())?;
        let (machine, loaded) = self.machine_for(store, meta, Need::Head, None)?;
        warnings.extend(loaded);
        let compiled = compile_reads(&machine, &args.reads);
        let reads =
            compiled.and_then(|c| eval_reads(&machine, &c, &since, args.allow_uninit, warnings));
        self.keep_warm(store, meta, machine);
        let reads = reads?;
        Ok(ResultBody {
            reads: Some(reads),
            ..ResultBody::default()
        })
    }

    /// The time base a request's `--since` names (design §9).
    pub(super) fn since(
        &self,
        store: &SessionStore,
        base: Option<&TimeBase>,
    ) -> Result<Since, Failure> {
        match base {
            None | Some(TimeBase::Boot) => Ok(Since::Boot),
            Some(TimeBase::PowerOn) => Ok(Since::PowerOn),
            Some(TimeBase::Checkpoint(name)) => {
                let cp = store
                    .checkpoint(name)
                    .map_err(|e| Failure::from_store(&e))?;
                Ok(Since::Checkpoint {
                    name: name.to_string(),
                    mark: TimeMark {
                        ticks: cp.header.ticks,
                        frame: cp.header.frame,
                    },
                })
            }
        }
    }
}

pub(super) fn warning_text(warning: &EvalWarning) -> String {
    warning.to_string()
}

/// A `--read` expression, compiled.
pub(super) struct CompiledRead {
    text: String,
    form: ReadForm,
}

enum ReadForm {
    Value(Condition),
    /// `oam[N]`, printed as fields (design §7).
    OamEntry(u8),
}

/// Compiles every read against the machine's symbols, before it runs.
pub(super) fn compile_reads(
    machine: &Machine,
    reads: &[String],
) -> Result<Vec<CompiledRead>, Failure> {
    compile_as(machine, reads, Wanted::Value)
}

/// What a compiled expression is for: a value prints a bare symbol as its
/// address; a truth refuses a RAM symbol taken as true or false.
#[derive(Clone, Copy)]
enum Wanted {
    Value,
    Truth,
}

fn compile_as(
    machine: &Machine,
    exprs: &[String],
    wanted: Wanted,
) -> Result<Vec<CompiledRead>, Failure> {
    let bad = |e: ExprError| Failure::new(convert::expr(&e));
    let compile = match wanted {
        Wanted::Value => Condition::compile_value,
        Wanted::Truth => Condition::compile,
    };
    exprs
        .iter()
        .map(|text| {
            let form = match oam_entry(text).map_err(bad)? {
                Some(index) => ReadForm::OamEntry(index),
                None => ReadForm::Value(compile(text, machine.symbols()).map_err(bad)?),
            };
            Ok(CompiledRead {
                text: text.clone(),
                form,
            })
        })
        .collect()
}

/// Compiles conditions that must have a truth value (`run --expect`): a
/// whole OAM entry, which a read prints as fields, has none, and a RAM
/// symbol taken as true or false is refused by the engine, as `--until`'s.
pub(super) fn compile_conditions(
    machine: &Machine,
    exprs: &[String],
) -> Result<Vec<CompiledRead>, Failure> {
    let compiled = compile_as(machine, exprs, Wanted::Truth)?;
    if let Some(read) = compiled
        .iter()
        .find(|r| matches!(r.form, ReadForm::OamEntry(_)))
    {
        return Err(Failure::new(ErrorBody::new(
            ErrorCode::BadRequest,
            format!(
                "`{}` is a whole OAM entry, which is neither true nor false",
                read.text
            ),
            "expect one of its bytes, e.g. `oam[0].y == 16`; --read prints the whole entry",
        )));
    }
    Ok(compiled)
}

fn oam_view(e: &ObjectAttributes) -> OamEntryRead {
    let cgb = match e.palette {
        ObjectPalette::Cgb(c) => Some(c),
        ObjectPalette::Dmg(_) => None,
    };
    OamEntryRead {
        index: e.index,
        y: e.y,
        x: e.x,
        tile: e.tile,
        attr: e.attr,
        priority: e.priority,
        yflip: e.y_flip,
        xflip: e.x_flip,
        palette: match e.palette {
            ObjectPalette::Dmg(p) => Some(p),
            ObjectPalette::Cgb(_) => None,
        },
        cgb_bank: cgb.map(|c| c.bank),
        cgb_palette: cgb.map(|c| c.palette),
    }
}

/// A hint when the whole read is a bare RAM symbol. In the expression grammar
/// a symbol is its address, which is right for a ROM label, but a reader who
/// names a variable almost always wants what it holds.
fn bare_ram_symbol(machine: &Machine, text: &str) -> Option<String> {
    let name = bare_ram_symbol_name(machine, text)?;
    let addr = machine.symbols()?.get(name)?.addr;
    Some(format!(
        "{name} is the address ${addr:04X}; [{name}] reads its byte, u16[{name}] two"
    ))
}

/// The symbol, when the whole expression is one RAM symbol.
fn bare_ram_symbol_name<'a>(machine: &Machine, text: &'a str) -> Option<&'a str> {
    let name = text.trim();
    let symbol = machine.symbols()?.get(name)?;
    Area::of(symbol.addr).is_ram().then_some(name)
}

/// Evaluates compiled reads; `allow_uninit` reads a never-written byte's
/// power-on value with a warning instead of failing (design §8).
pub(super) fn eval_reads(
    machine: &Machine,
    reads: &[CompiledRead],
    since: &Since,
    allow_uninit: bool,
    warnings: &mut Vec<String>,
) -> Result<Vec<ReadValue>, Failure> {
    let base = machine.time_base(since);
    reads
        .iter()
        .map(|read| eval_read(machine, read, base, allow_uninit, warnings))
        .collect()
}

/// A run's reads: one that fails says why in its own row, and the stop and
/// the other reads stand. The gap of a read the core can't answer goes to
/// `gaps`, for the ledger.
pub(super) fn eval_reads_each(
    machine: &Machine,
    reads: &[CompiledRead],
    since: &Since,
    allow_uninit: bool,
    warnings: &mut Vec<String>,
    gaps: &mut Vec<Gap>,
) -> Vec<ReadValue> {
    let base = machine.time_base(since);
    let mut values = Vec::new();
    for read in reads {
        values.push(
            match eval_read(machine, read, base, allow_uninit, warnings) {
                Ok(value) => value,
                Err(Failure { body, gap }) => {
                    gaps.extend(gap);
                    ReadValue {
                        expr: read.text.clone(),
                        value: None,
                        oam: None,
                        error: Some(ReadFailed {
                            code: body.code,
                            message: body.message,
                            remedy: body.remedy,
                        }),
                    }
                }
            },
        );
    }
    values
}

fn eval_read(
    machine: &Machine,
    CompiledRead { text, form }: &CompiledRead,
    base: TimeMark,
    allow_uninit: bool,
    warnings: &mut Vec<String>,
) -> Result<ReadValue, Failure> {
    let condition = match form {
        ReadForm::Value(condition) => condition,
        ReadForm::OamEntry(index) => {
            let entry =
                object_attributes(machine.core(), *index).map_err(|e| Failure::from_core(&e))?;
            return Ok(ReadValue {
                expr: text.clone(),
                value: None,
                oam: Some(oam_view(&entry)),
                error: None,
            });
        }
    };
    let ctx = EvalCtx {
        core: machine.core(),
        symbols: machine.symbols(),
        access: None,
        base,
        allow_uninit,
    };
    let mut eval_warnings: Vec<EvalWarning> = Vec::new();
    let value = condition
        .eval(&ctx, &mut eval_warnings)
        .map_err(|e| Failure {
            body: Box::new(convert::eval(&e)),
            gap: convert::gap_of_eval(&e),
        })?;
    warnings.extend(eval_warnings.iter().map(warning_text));
    warnings.extend(bare_ram_symbol(machine, text));
    Ok(ReadValue {
        expr: text.clone(),
        value: Some(value),
        oam: None,
        error: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::Fixture;
    use crate::handle::test_support::{handler, request, run_frames};
    use gbcli_protocol::{ErrorCode, Op};

    #[test]
    fn an_unsupported_read_writes_the_gap_ledger_and_shows_in_the_event_log() {
        let f = Fixture::new("gaps");
        let mut h = handler(&f);
        let response = h.handle(&request(Op::Read(ReadArgs {
            reads: vec!["[$FEA0]".into()],
            since: None,
            allow_uninit: false,
        })));
        let error = response.outcome.expect_err("FEA0 has no backing store");
        assert_eq!(error.code, ErrorCode::Unsupported);

        let gaps = f.bundle.gap_counts().unwrap();
        assert_eq!(gaps.len(), 1);
        assert_eq!(gaps[0].feature, "extra_oam");

        let store = f.bundle.session("demo").unwrap();
        let events = store.events(None).unwrap();
        let last = events.last().expect("one event");
        assert_eq!(last.gaps.get("extra_oam"), Some(&1));
    }

    fn read_warnings(f: &Fixture, reads: &[&str]) -> Vec<String> {
        let mut h = handler(f);
        let response = h.handle(&request(Op::Read(ReadArgs {
            reads: reads.iter().map(|r| r.to_string()).collect(),
            since: None,
            allow_uninit: true,
        })));
        assert!(response.outcome.is_ok(), "{:?}", response.outcome);
        response.warnings
    }

    #[test]
    fn a_bare_ram_symbol_reads_its_address_and_says_how_to_read_its_byte() {
        let f = Fixture::with_sym("bare-ram");
        let warnings = read_warnings(&f, &["wFrame"]);
        let hint = warnings
            .iter()
            .find(|w| w.contains("wFrame"))
            .unwrap_or_else(|| panic!("no hint: {warnings:?}"));
        assert!(hint.contains("address $C000"), "{hint}");
        assert!(hint.contains("[wFrame] reads its byte"), "{hint}");
        assert!(hint.contains("u16[wFrame]"), "{hint}");
    }

    #[test]
    fn a_rom_label_a_bracketed_symbol_or_a_sum_gets_no_hint() {
        let f = Fixture::with_sym("bare-rom");
        let warnings = read_warnings(&f, &["Main", "[wFrame]", "u16[wFrame]", "wFrame+1"]);
        assert!(
            !warnings.iter().any(|w| w.contains("reads its byte")),
            "{warnings:?}"
        );
    }

    /// `show` names the machine and what the loaded ROM was built for (leaf
    /// jGOxRH): the rogue ran DMG sessions over its GBC build unawares.
    #[test]
    fn show_names_the_model_and_the_roms_cgb_flag() {
        use gbcli_core::Model;
        use gbcli_protocol::CgbFlag;
        let machine = |f: &Fixture| {
            let result = handler(f).handle(&request(Op::Show)).outcome.unwrap();
            assert!(result.registers.is_some(), "the rest of show stands");
            result.machine.expect("machine")
        };
        let dmg = machine(&Fixture::new("show-dmg"));
        assert_eq!((dmg.model, dmg.cgb), (Model::DmgB, Some(CgbFlag::Dmg)));
        let gbc = Fixture::with_model("show-gbc", crate::fixture::GBC_PALETTE, Model::CgbE);
        let gbc = machine(&gbc);
        assert_eq!((gbc.model, gbc.cgb), (Model::CgbE, Some(CgbFlag::Dual)));
    }

    #[test]
    fn show_reports_registers_and_the_last_stop() {
        let f = Fixture::new("show");
        let mut h = handler(&f);
        assert!(run_frames(&mut h, 1).is_ok());
        let shown = h.handle(&request(Op::Show));
        let result = shown.outcome.unwrap();
        assert!(result.registers.is_some());
        assert!(result.ppu.is_some());
        assert!(result.stop.is_some(), "the last stop is still there");
    }
}
