//! Runs one row on one model and applies its pass rule.

use std::path::Path;

use gbcli_core::{BootRom, Core, CoreConfig, CoreError, Region, WriteTracking};
use serde::Serialize;

use crate::docboy;
use crate::drive::{Driver, Event, TICKS_PER_FRAME, TICKS_PER_SECOND};
use crate::frame::{self, Match};
use crate::manifest::{Exit, PassRule, Row};
use crate::models::{Target, model_name};
use crate::rules::{self, BlarggLayout, Verdict};

/// Builds a core; the runner never names a backend beyond this.
pub type Factory = fn(CoreConfig) -> Result<Box<dyn Core>, CoreError>;

/// What every run shares.
#[derive(Clone, Copy)]
pub struct Context<'a> {
    /// `tests/corpus/fetched/`.
    pub corpus: &'a Path,
    pub factory: Factory,
    /// Where to write the frame of each failing framebuffer row, if anywhere.
    pub dump: Option<&'a Path>,
}

/// Emulated seconds a row with no stated bound may run before judging.
const DEFAULT_TIMEOUT_S: f64 = 30.0;
/// How often polled exits (`result_reported`) look for a result.
const POLL_TICKS: u64 = TICKS_PER_SECOND / 10;
/// How long after the exit an alternative signal may still arrive.
const ALTERNATIVE_GRACE_TICKS: u64 = TICKS_PER_SECOND / 2;
const CART_RAM: u16 = 0xA000;
const HRAM: u16 = 0xFF80;

/// One row on one model, as the baseline records it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Outcome {
    pub id: String,
    pub suite: String,
    pub model: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub substitute_for: Option<String>,
    #[serde(flatten)]
    pub verdict: Verdict,
    /// Emulated seconds from hand-over to judgment, to the millisecond.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emulated_s: Option<f64>,
    /// The other signals the ROM emits, judged on the same run.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub alternatives: Vec<Verdict>,
}

/// Runs `row` on `target` and judges it: docboy cases from docboy's hand-over
/// state ([`docboy::post_boot_rom`]), every other row after the model's
/// built-in boot ROM.
pub fn run(row: &Row, target: &Target, ctx: Context<'_>) -> Outcome {
    run_with_boot(row, target, ctx, BootRom::BuiltIn)
}

/// [`run`], with `boot_rom` in place of the built-in boot ROM for a row that
/// is not a docboy case (docboy cases always start from docboy's state).
pub fn run_with_boot(row: &Row, target: &Target, ctx: Context<'_>, boot_rom: BootRom) -> Outcome {
    let mut outcome = Outcome {
        id: row.id.clone(),
        suite: row.suite.clone(),
        model: model_name(target.model),
        substitute_for: target.substitute_for.clone(),
        verdict: Verdict::Pass,
        emulated_s: None,
        alternatives: Vec::new(),
    };
    if let PassRule::Manual { reason } = &row.pass {
        outcome.verdict = Verdict::Skip(format!("manual: {reason}"));
        return outcome;
    }
    if let PassRule::DocboyCase {
        config,
        category,
        index,
    } = &row.pass
    {
        let (verdict, elapsed) =
            docboy::run(ctx.corpus, config, category, *index, target, ctx.factory);
        outcome.verdict = verdict;
        outcome.emulated_s = elapsed.map(seconds);
        return outcome;
    }
    match judge(row, target, ctx, boot_rom) {
        Ok((verdict, alternatives, elapsed)) => {
            outcome.verdict = verdict;
            outcome.alternatives = alternatives;
            outcome.emulated_s = Some(seconds(elapsed));
        }
        Err(err) => outcome.verdict = Verdict::fail(format!("runner error: {err}")),
    }
    outcome
}

fn seconds(ticks: u64) -> f64 {
    (ticks as f64 / TICKS_PER_SECOND as f64 * 1000.0).round() / 1000.0
}

/// Builds a core for `rom` on `target` and runs `boot_rom` to the hand-over.
pub fn boot(
    rom: &[u8],
    target: &Target,
    factory: Factory,
    boot_rom: BootRom,
) -> Result<Driver, String> {
    let mut config = CoreConfig::new(target.model, rom.to_vec());
    config.write_tracking = WriteTracking::Off;
    config.boot_rom = boot_rom;
    let core = factory(config).map_err(|e| e.to_string())?;
    Driver::boot(core, rom)
}

type Judged = (Verdict, Vec<Verdict>, u64);

fn judge(
    row: &Row,
    target: &Target,
    ctx: Context<'_>,
    boot_rom: BootRom,
) -> Result<Judged, String> {
    let rom =
        std::fs::read(ctx.corpus.join(&row.path)).map_err(|e| format!("{}: {e}", row.path))?;
    let mut d = boot(&rom, target, ctx.factory, boot_rom)?;
    let timeout = (row.timeout_s.unwrap_or(DEFAULT_TIMEOUT_S) * TICKS_PER_SECOND as f64) as u64;

    let ended = match &row.exit {
        Exit::LdBB => d.run_to_ld_b_b(timeout)?,
        Exit::EmulatedSeconds { seconds } => {
            d.run_for((seconds * TICKS_PER_SECOND as f64) as u64)?
        }
        Exit::EmulatedFrames { frames } => d.run_for(u64::from(*frames) * TICKS_PER_FRAME)?,
        Exit::ResultReported => loop {
            let until = (d.elapsed() + POLL_TICKS).min(timeout);
            let event = d.advance(until, false)?;
            if polled(&row.pass, &d)?.is_some() || !matches!(event, Event::Serial | Event::Budget) {
                break event;
            }
            if d.elapsed() >= timeout {
                break Event::Budget;
            }
        },
        Exit::DocboyCase => return Err("a docboy exit outside a docboy row".into()),
    };
    let elapsed = d.elapsed();
    let at = || {
        format!(
            "{:.2} s after hand-over",
            elapsed as f64 / TICKS_PER_SECOND as f64
        )
    };
    let ended_early = match ended {
        Event::Locked(op) => Some(format!(
            "the CPU locked on illegal opcode {op:#04x} {}",
            at()
        )),
        Event::Stopped => Some(format!("the CPU stopped with no wake source {}", at())),
        Event::Budget if row.exit == Exit::LdBB => Some(format!("no LD B,B within {}", at())),
        _ => None,
    };
    let verdict = match ended_early {
        Some(why) => Verdict::Fail(why),
        None => apply(&row.pass, &d, ctx, &dump_name(row, target))?,
    };
    // An alternative signal may trail the exit: give unanswered ones a short
    // grace period.
    let grace = d.elapsed() + ALTERNATIVE_GRACE_TICKS;
    while d.elapsed() < grace
        && row
            .pass_alternatives
            .iter()
            .any(|alt| matches!(polled(alt, &d), Ok(None)))
    {
        if !matches!(d.advance(grace, false)?, Event::Serial | Event::Budget) {
            break;
        }
    }
    let alternatives = row
        .pass_alternatives
        .iter()
        .map(|alt| polled(alt, &d).map(|v| v.unwrap_or_else(|| no_result(alt, &d))))
        .collect::<Result<_, _>>()?;
    Ok((verdict, alternatives, elapsed))
}

/// The rules a run can answer at any moment; `None` while there is no result yet.
fn polled(rule: &PassRule, d: &Driver) -> Result<Option<Verdict>, String> {
    Ok(match rule {
        PassRule::SerialBytes { bytes, fail_bytes } => {
            rules::serial_bytes(&d.serial, bytes, fail_bytes)
        }
        PassRule::SerialText {
            pass_contains,
            fail_contains,
        } => rules::serial_text(&d.serial, pass_contains, fail_contains),
        PassRule::BlarggMemory {
            signature_address,
            signature,
            status_address,
            running_value,
            pass_value,
            text_address,
        } => {
            let layout = BlarggLayout {
                signature_offset: usize::from(signature_address - CART_RAM),
                signature,
                status_offset: usize::from(status_address - CART_RAM),
                running_value: *running_value,
                pass_value: *pass_value,
                text_offset: usize::from(text_address - CART_RAM),
            };
            rules::blargg_memory(d.region_bytes(Region::Sram)?, &layout)
        }
        _ => None,
    })
}

/// Applies a rule at the exit.
fn apply(rule: &PassRule, d: &Driver, ctx: Context<'_>, name: &str) -> Result<Verdict, String> {
    Ok(match rule {
        PassRule::FibonacciRegisters {
            b,
            c,
            d: rd,
            e,
            h,
            l,
            fail_value,
        } => rules::fibonacci(&d.core.registers(), [*b, *c, *rd, *e, *h, *l], *fail_value),
        PassRule::MemoryByte {
            address,
            pass_value,
            fail_value,
        } => {
            let hram = d.region_bytes(Region::Hram)?;
            let offset = usize::from(
                address
                    .checked_sub(HRAM)
                    .ok_or("memory_byte outside HRAM")?,
            );
            rules::memory_byte(hram[offset], *pass_value, *fail_value)
        }
        PassRule::Framebuffer { reference, palette } => {
            let reference = frame::load_png(&ctx.corpus.join(reference))?;
            let Some(frame) = d.core.last_complete_frame() else {
                return Ok(Verdict::fail("no complete frame yet"));
            };
            let actual = frame::render(&frame, *palette)?;
            let verdict = frame_verdict(frame::compare(&actual, &reference));
            if let (Some(dir), Verdict::Fail(_)) = (ctx.dump, &verdict) {
                frame::save_png(&dir.join(name), &actual)?;
            }
            verdict
        }
        other => polled(other, d)?.unwrap_or_else(|| no_result(other, d)),
    })
}

/// A file name for a row's dumped frame: its id and model, path separators flattened.
fn dump_name(row: &Row, target: &Target) -> String {
    format!(
        "{}@{}.png",
        row.id.replace('/', "__"),
        model_name(target.model)
    )
}

/// What a rule that never answered leaves behind, for the baseline's detail.
fn no_result(rule: &PassRule, d: &Driver) -> Verdict {
    let mut detail = format!("{} gave no result", kind(rule));
    if let PassRule::SerialBytes { .. } | PassRule::SerialText { .. } = rule {
        detail += &format!(
            " (serial so far: {:02x?})",
            &d.serial[d.serial.len().saturating_sub(16)..]
        );
    }
    if let PassRule::BlarggMemory { .. } = rule {
        match d.region_bytes(Region::Sram) {
            Ok(sram) if sram.len() >= 4 => {
                detail += &format!(" (cart RAM starts {:02x?})", &sram[..4])
            }
            Ok(_) => detail += " (the cartridge has no RAM)",
            Err(e) => detail += &format!(" ({e})"),
        }
    }
    Verdict::Fail(detail)
}

fn frame_verdict(m: Match) -> Verdict {
    match m {
        Match::Exact => Verdict::Pass,
        Match::Recolored => {
            Verdict::fail("frame differs only in colors (a one-to-one recoloring matches)")
        }
        Match::Differs {
            pixels,
            first: (x, y),
        } => Verdict::fail(format!(
            "frame differs in {pixels} pixels, first at ({x}, {y})"
        )),
        Match::WrongSize { got: (w, h) } => Verdict::fail(format!("frame is {w}×{h}")),
    }
}

fn kind(rule: &PassRule) -> &'static str {
    match rule {
        PassRule::FibonacciRegisters { .. } => "fibonacci_registers",
        PassRule::SerialBytes { .. } => "serial_bytes",
        PassRule::SerialText { .. } => "serial_text",
        PassRule::BlarggMemory { .. } => "blargg_memory",
        PassRule::Framebuffer { .. } => "framebuffer",
        PassRule::MemoryByte { .. } => "memory_byte",
        PassRule::DocboyCase { .. } => "docboy_case",
        PassRule::Manual { .. } => "manual",
    }
}
