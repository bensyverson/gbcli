//! rgbasm and rgblink 1.0.4 as the referee of `disasm::lookup`: many
//! instruction lines assembled in one batch, each line's bytes or its
//! refusal, and the lines where the lookup answers otherwise. Needs RGBDS
//! 1.0.4 on `PATH`.
//!
//! The batch shape: one main file that INCLUDEs each line from a file of
//! its own, in a 4-byte slot at a fixed address (the line's bytes, padding,
//! then their count). A file of its own keeps a line's unterminated string,
//! block comment or trailing `\` from reaching the next line, and rgbasm
//! names the included file in every diagnostic, so each error belongs to
//! one line. rgbasm makes no object if any line fails, so the batch is
//! assembled again without the lines that failed until it links: two runs
//! of rgbasm and one of rgblink for most batches, whatever their size.
//!
//! Symbols the lines may use: `wFoo` ($C000), `rLCDC` ($FF40), and `Top`,
//! a code label at the line's own address (so `jr nz, Top` is in reach).

#![allow(dead_code)] // `it` and `opcode_rgbasm_probes` each use a different subset

pub mod grammar;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use gbcli_engine::disasm::{LookupError, Opcode, lookup};

/// Lines one rgbasm run takes: 4-byte slots fill ROM0's 16 KiB.
const BATCH: usize = 0x4000 / SLOT;
const SLOT: usize = 4;

/// The main file's lines before the first slot, and each slot's.
const HEADER: &str = "DEF wFoo EQU $C000\nDEF rLCDC EQU $FF40\nSECTION \"s\", ROM0[$0]\n";
const HEADER_LINES: usize = 3;
const SLOT_LINES: usize = 6;

/// What the lookup answers for a line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ours {
    /// The one opcode's bytes (with the $CB prefix), and the form's length.
    Opcode(Vec<u8>, usize),
    /// Every form of a mnemonic given alone (`jr`): a listing, which rgbasm
    /// has no answer to compare with.
    Listing(usize),
    Refused(LookupError),
}

pub fn ours(line: &str) -> Ours {
    match lookup(line) {
        Ok(found) if found.len() == 1 => Ours::Opcode(
            match found[0].opcode {
                Opcode::Base(b) => vec![b],
                Opcode::Cb(b) => vec![0xCB, b],
            },
            usize::from(found[0].len),
        ),
        Ok(found) => Ours::Listing(found.len()),
        Err(e) => Ours::Refused(e),
    }
}

/// Whether the lookup and rgbasm agree on a line: the same opcode (the
/// line's first bytes) and no more bytes than the form has, or both refuse.
pub fn agrees(ours: &Ours, rgbasm: &Result<Vec<u8>, String>) -> bool {
    match (ours, rgbasm) {
        (Ours::Opcode(op, len), Ok(bytes)) => bytes.starts_with(op) && bytes.len() == *len,
        (Ours::Refused(_), Err(_)) => true,
        _ => false,
    }
}

/// One line per disagreement: the line, the lookup's answer and rgbasm's.
pub fn disagreements<S: AsRef<str>>(lines: &[S]) -> Vec<String> {
    let theirs = answers(lines);
    lines
        .iter()
        .zip(&theirs)
        .filter_map(|(line, rgbasm)| {
            let line = line.as_ref();
            let ours = ours(line);
            (!agrees(&ours, rgbasm)).then(|| describe(line, &ours, rgbasm))
        })
        .collect()
}

pub fn describe(line: &str, ours: &Ours, rgbasm: &Result<Vec<u8>, String>) -> String {
    let ours = match ours {
        Ours::Opcode(op, _) => format!("{op:02X?}"),
        Ours::Listing(n) => format!("lists {n} forms"),
        Ours::Refused(e) => format!("refused ({e})"),
    };
    let rgbasm = match rgbasm {
        Ok(bytes) => format!("{bytes:02X?}"),
        Err(why) => format!("refused ({})", why.replace('\n', "; ")),
    };
    format!("{line:?}: lookup {ours}, rgbasm {rgbasm}")
}

/// The bytes rgbasm and rgblink make of each line, or `None` where either
/// refuses it or it makes no bytes.
pub fn assemble<S: AsRef<str>>(lines: &[S]) -> Vec<Option<Vec<u8>>> {
    answers(lines).into_iter().map(Result::ok).collect()
}

/// Each line's bytes, or why rgbasm or rgblink gave none: their errors
/// about the line, one per line of text. Batches of `BATCH` lines run on
/// threads of their own.
pub fn answers<S: AsRef<str>>(lines: &[S]) -> Vec<Result<Vec<u8>, String>> {
    let lines: Vec<&str> = lines.iter().map(AsRef::as_ref).collect();
    std::thread::scope(|scope| {
        let batches: Vec<_> = lines
            .chunks(BATCH)
            .map(|chunk| scope.spawn(move || assemble_batch(chunk)))
            .collect();
        batches
            .into_iter()
            .flat_map(|b| b.join().expect("rgbasm batch"))
            .collect()
    })
}

fn assemble_batch(lines: &[&str]) -> Vec<Result<Vec<u8>, String>> {
    assert!(lines.len() <= BATCH);
    let dir = scratch();
    for (i, line) in lines.iter().enumerate() {
        assert!(!line.contains('\n'), "{line:?}: one line at a time");
        std::fs::write(dir.join(format!("l{i}.asm")), format!("    {line}\n")).unwrap();
    }
    let mut refused: BTreeMap<usize, String> = BTreeMap::new();
    let rom = loop {
        std::fs::write(dir.join("main.asm"), main(lines.len(), &refused)).unwrap();
        let failed = match run(&dir, "rgbasm", &["-X", "0", "-o", "m.o", "main.asm"]) {
            Ok(()) => match run(&dir, "rgblink", &["-o", "m.gb", "m.o"]) {
                Ok(()) => break std::fs::read(dir.join("m.gb")).unwrap(),
                Err(stderr) => stderr,
            },
            Err(stderr) => stderr,
        };
        let before = refused.len();
        for (i, message) in failing_lines(&failed, lines.len()) {
            let why = refused.entry(i).or_default();
            if !why.is_empty() {
                why.push('\n');
            }
            why.push_str(&message);
        }
        assert!(
            refused.len() > before,
            "rgbasm or rgblink failed on no line of the batch:\n{failed}"
        );
    };
    std::fs::remove_dir_all(&dir).ok();
    (0..lines.len())
        .map(|i| {
            if let Some(why) = refused.get(&i) {
                return Err(why.clone());
            }
            let slot = &rom[i * SLOT..(i + 1) * SLOT];
            let len = usize::from(slot[SLOT - 1]);
            if len == 0 {
                Err(NO_BYTES.into())
            } else {
                Ok(slot[..len].to_vec())
            }
        })
        .collect()
}

/// Why a line that rgbasm takes has no answer: it makes no bytes.
pub const NO_BYTES: &str = "no bytes";

/// The main file: each line in its slot, except those already refused.
fn main(count: usize, refused: &BTreeMap<usize, String>) -> String {
    let mut src = String::from(HEADER);
    for i in 0..count {
        let include = if refused.contains_key(&i) {
            String::new()
        } else {
            format!("INCLUDE \"l{i}.asm\"")
        };
        // SLOT_LINES lines, so a diagnostic in this file names its slot.
        src.push_str(&format!(
            "Top:\n{include}\nREDEF LEN EQU @ - Top\n    ds {} - LEN, 0\n    db LEN\nPURGE Top\n",
            SLOT - 1
        ));
    }
    src
}

/// The lines an rgbasm or rgblink error names, with its message, in either
/// the line's own file or its slot of the main file: `error: …` followed
/// by `    at l7.asm(1)` and `    <- main.asm(47)`.
fn failing_lines(stderr: &str, count: usize) -> Vec<(usize, String)> {
    let mut failing = Vec::new();
    let mut error: Option<&str> = None;
    for text in stderr.lines() {
        if !text.starts_with(' ') {
            let fatal = text.to_ascii_lowercase().starts_with("fatal");
            error = (text.starts_with("error:") || fatal).then_some(text);
            continue;
        }
        let Some(message) = error else { continue };
        let at = text.trim_start();
        let at = at
            .strip_prefix("at ")
            .or_else(|| at.strip_prefix("<- "))
            .unwrap_or(at);
        let slot = if let Some((n, _)) = at.strip_prefix('l').and_then(|s| s.split_once(".asm(")) {
            n.parse::<usize>().ok()
        } else if let Some(n) = at.strip_prefix("main.asm(") {
            let line: usize = n.trim_end_matches(')').parse().unwrap_or(0);
            line.checked_sub(HEADER_LINES + 1).map(|l| l / SLOT_LINES)
        } else {
            None
        };
        if let Some(i) = slot.filter(|&i| i < count) {
            failing.push((i, message.to_string()));
            error = None;
        }
    }
    failing
}

/// Runs an RGBDS tool in `dir`: its stderr if it fails.
fn run(dir: &Path, cmd: &str, args: &[&str]) -> Result<(), String> {
    let out = Command::new(cmd)
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap_or_else(|e| {
            panic!("{cmd} is needed for this test ({e}): brew install rgbds (1.0.4)")
        });
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).into_owned())
    }
}

fn scratch() -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "gbcli-rgbasm-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
