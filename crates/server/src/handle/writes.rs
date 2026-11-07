use gbcli_bundle::{SessionStore, Start};
use gbcli_core::Region;
use gbcli_engine::addr::{self, RegionSizes, WriteTarget};
use gbcli_engine::{Machine, mem};
use gbcli_protocol::{
    ErrorBody, ErrorCode, Location, ResultBody, SessionFile, WriteArg, WriteArgs, WriteKind,
    WriteMade,
};

use super::{Failure, Handler, HeadChange, Need, save_head};
use crate::convert::{self};
impl Handler {
    /// Writes start where a run would: before the first run, that is the boot
    /// handoff, so the head they save isn't a power-on state.
    pub(super) fn write(
        &mut self,
        store: &SessionStore,
        meta: &mut SessionFile,
        args: &WriteArgs,
        actor: &Option<String>,
        warnings: &mut Vec<String>,
    ) -> Result<ResultBody, Failure> {
        let need = Need::Start(Start::Boot);
        let (mut machine, loaded) = self.machine_for(store, meta, need, None)?;
        warnings.extend(loaded);
        let writes = apply_writes(&mut machine, &args.writes)?;
        let change = HeadChange {
            actor: actor.clone(),
            ..HeadChange::default()
        };
        save_head(store, meta, &mut machine, change)?;
        self.keep_warm(store, meta, machine);
        Ok(ResultBody {
            writes: Some(writes),
            ..ResultBody::default()
        })
    }
}

/// The sizes of this machine's regions, which bound a region offset or bank.
fn sizes(machine: &Machine) -> Result<RegionSizes, Failure> {
    RegionSizes::of(machine.core()).map_err(|e| Failure::from_core(&e))
}

/// A §7 address on this machine, bounded by its model's and cart's sizes.
pub(super) fn parse_location(machine: &Machine, text: &str) -> Result<Location, Failure> {
    addr::parse_on(text, machine.symbols(), &sizes(machine)?)
        .map_err(|e| Failure::new(convert::addr(&e)))
}

fn parse_write_target(machine: &Machine, text: &str) -> Result<WriteTarget, Failure> {
    addr::parse_write_on(text, machine.symbols(), &sizes(machine)?)
        .map_err(|e| Failure::new(convert::addr(&e)))
}

/// A poke to ROM, refused with the remedy design §4 gives.
fn poke_to_rom(target: &str) -> Failure {
    Failure::new(ErrorBody::new(
        ErrorCode::Refused,
        format!("{target} is ROM, which a poke can't change"),
        "use --patch; ROM changes are recorded so checkpoints stay honest",
    ))
}

/// Applies the three kinds of write in order (design §4).
pub(super) fn apply_writes(
    machine: &mut Machine,
    writes: &[WriteArg],
) -> Result<Vec<WriteMade>, Failure> {
    let mut made = Vec::new();
    for write in writes {
        match write.kind {
            WriteKind::Bus => {
                let at = parse_location(machine, &write.target)?;
                let base = at.addr;
                for (i, &byte) in write.bytes.iter().enumerate() {
                    machine
                        .core_mut()
                        .bus_write(base.wrapping_add(i as u16), byte)
                        .map_err(|e| Failure::from_core(&e))?;
                }
                made.push(WriteMade {
                    kind: write.kind,
                    target: write.target.clone(),
                    bytes: write.bytes.clone(),
                    addr: Some(base),
                });
            }
            WriteKind::Poke => {
                let WriteTarget::Cpu(at) = parse_write_target(machine, &write.target)? else {
                    return Err(poke_to_rom(&write.target));
                };
                let base = at.addr;
                for (i, &byte) in write.bytes.iter().enumerate() {
                    let addr = base.wrapping_add(i as u16);
                    // A bank-qualified target writes that bank, mapped or not.
                    let bank = at
                        .bank
                        .unwrap_or_else(|| mem::bank_at(machine.core(), addr));
                    let target =
                        mem::banked_target(bank, addr).map_err(|e| Failure::from_core(&e))?;
                    let (region, offset) = match target {
                        mem::Target::Backing {
                            region: Region::Rom,
                            ..
                        } => return Err(poke_to_rom(&write.target)),
                        mem::Target::Backing { region, offset } => (region, offset),
                        mem::Target::Io(reg) => (Region::Io, usize::from(reg.0)),
                    };
                    machine
                        .core_mut()
                        .poke(region, offset as u32, &[byte])
                        .map_err(|e| Failure::from_core(&e))?;
                }
                made.push(WriteMade {
                    kind: write.kind,
                    target: write.target.clone(),
                    bytes: write.bytes.clone(),
                    addr: Some(base),
                });
            }
            WriteKind::Patch => {
                let offset = rom_offset(machine, &write.target)?;
                machine
                    .patch(offset, &write.bytes)
                    .map_err(|e| Failure::from_core(&e))?;
                made.push(WriteMade {
                    kind: write.kind,
                    target: write.target.clone(),
                    bytes: write.bytes.clone(),
                    addr: None,
                });
            }
        }
    }
    Ok(made)
}

/// A patch target as a ROM file offset (design §7).
fn rom_offset(machine: &Machine, text: &str) -> Result<u32, Failure> {
    match parse_write_target(machine, text)? {
        WriteTarget::RomOffset(offset) => Ok(offset),
        WriteTarget::Cpu(at) => {
            let bank = at
                .bank
                .unwrap_or_else(|| mem::bank_at(machine.core(), at.addr));
            match mem::banked_target(bank, at.addr).map_err(|e| Failure::from_core(&e))? {
                mem::Target::Backing {
                    region: Region::Rom,
                    offset,
                } => Ok(offset as u32),
                _ => Err(Failure::new(ErrorBody::new(
                    ErrorCode::BadAddress,
                    format!("{text} isn't in ROM"),
                    "a patch targets ROM; use a poke for RAM or IO",
                ))),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::Fixture;
    use crate::handle::test_support::{handler, request};
    use gbcli_protocol::{Op, ReadArgs};

    #[test]
    fn a_poke_is_backing_memory_and_the_next_read_sees_it() {
        let f = Fixture::new("poke");
        let mut h = handler(&f);
        let written = h.handle(&request(Op::Write(WriteArgs {
            writes: vec![WriteArg {
                kind: WriteKind::Poke,
                target: "$C002".into(),
                bytes: vec![2],
            }],
        })));
        assert!(written.is_ok(), "{:?}", written.error());
        let read = h.handle(&request(Op::Read(ReadArgs {
            reads: vec!["[$C002]".into()],
            since: None,
            allow_uninit: false,
        })));
        let reads = read.outcome.unwrap().reads.unwrap();
        assert_eq!(reads[0].value, Some(2));
    }
}
