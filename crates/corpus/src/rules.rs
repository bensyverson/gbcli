//! Pass rules as pure functions over what a run observed.

use gbcli_core::Registers;
use serde::Serialize;

/// The outcome of one judged run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "status", content = "detail", rename_all = "lowercase")]
pub enum Verdict {
    Pass,
    Fail(String),
    Skip(String),
}

impl Verdict {
    pub fn fail(detail: impl Into<String>) -> Self {
        Verdict::Fail(detail.into())
    }
}

/// Mooneye's convention: B/C/D/E/H/L hold the expected values at `LD B,B`,
/// or every one holds `fail_value`.
pub fn fibonacci(regs: &Registers, expected: [u8; 6], fail_value: u8) -> Verdict {
    let got = [regs.b, regs.c, regs.d, regs.e, regs.h, regs.l];
    if got == expected {
        Verdict::Pass
    } else if got == [fail_value; 6] {
        Verdict::fail(format!("the test reported failure (all {fail_value:#04x})"))
    } else {
        Verdict::fail(format!("BCDEHL = {got:02x?}, wanted {expected:02x?}"))
    }
}

/// Mooneye's serial alternative: the six bytes, or the failure bytes. `None`
/// while neither has appeared.
pub fn serial_bytes(captured: &[u8], pass: &[u8], fail: &[u8]) -> Option<Verdict> {
    let has =
        |needle: &[u8]| !needle.is_empty() && captured.windows(needle.len()).any(|w| w == needle);
    if has(pass) {
        Some(Verdict::Pass)
    } else if has(fail) {
        Some(Verdict::fail(format!(
            "serial reported failure: {captured:02x?}"
        )))
    } else {
        None
    }
}

/// Blargg's text output: `None` until it contains the pass or fail marker.
pub fn serial_text(captured: &[u8], pass: &str, fail: &str) -> Option<Verdict> {
    let text = String::from_utf8_lossy(captured);
    if text.contains(pass) {
        Some(Verdict::Pass)
    } else if text.contains(fail) {
        Some(Verdict::fail(summarize(&text)))
    } else {
        None
    }
}

/// Where blargg's memory protocol lives, as offsets into cart RAM at $A000.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlarggLayout<'a> {
    pub signature_offset: usize,
    pub signature: &'a [u8],
    pub status_offset: usize,
    pub running_value: u8,
    pub pass_value: u8,
    pub text_offset: usize,
}

/// Blargg's "output to memory": `None` until the signature is present and the
/// status has left the running value.
pub fn blargg_memory(sram: &[u8], layout: &BlarggLayout<'_>) -> Option<Verdict> {
    let sig =
        sram.get(layout.signature_offset..layout.signature_offset + layout.signature.len())?;
    if sig != layout.signature {
        return None;
    }
    let status = *sram.get(layout.status_offset)?;
    if status == layout.running_value {
        return None;
    }
    if status == layout.pass_value {
        return Some(Verdict::Pass);
    }
    let text = sram.get(layout.text_offset..).unwrap_or_default();
    let end = text.iter().position(|&b| b == 0).unwrap_or(text.len());
    let text = String::from_utf8_lossy(&text[..end]);
    Some(Verdict::fail(format!(
        "result code {status}: {}",
        summarize(&text)
    )))
}

/// GBMicrotest's convention: one byte holds the pass or fail value.
pub fn memory_byte(value: u8, pass: u8, fail: u8) -> Verdict {
    if value == pass {
        Verdict::Pass
    } else if value == fail {
        Verdict::fail(format!("the test reported failure ({fail:#04x})"))
    } else {
        Verdict::fail(format!("no result: the byte holds {value:#04x}"))
    }
}

/// Test output as one short line for a baseline's detail field.
fn summarize(text: &str) -> String {
    let line: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut short: String = line.chars().take(160).collect();
    if short.len() < line.len() {
        short.push('…');
    }
    short
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIB: [u8; 6] = [3, 5, 8, 13, 21, 34];

    fn regs(v: [u8; 6]) -> Registers {
        Registers {
            b: v[0],
            c: v[1],
            d: v[2],
            e: v[3],
            h: v[4],
            l: v[5],
            ..Registers::default()
        }
    }

    #[test]
    fn fibonacci_passes_only_on_the_exact_values() {
        assert_eq!(fibonacci(&regs(FIB), FIB, 0x42), Verdict::Pass);
        let off_by_one = fibonacci(&regs([3, 5, 8, 13, 21, 35]), FIB, 0x42);
        assert!(matches!(off_by_one, Verdict::Fail(d) if d.contains("BCDEHL")));
        let failed = fibonacci(&regs([0x42; 6]), FIB, 0x42);
        assert!(matches!(failed, Verdict::Fail(d) if d.contains("reported failure")));
    }

    #[test]
    fn serial_bytes_waits_then_finds_pass_or_fail() {
        assert_eq!(serial_bytes(&[3, 5, 8], &FIB, &[0x42; 6]), None);
        assert_eq!(
            serial_bytes(&[0, 3, 5, 8, 13, 21, 34], &FIB, &[0x42; 6]),
            Some(Verdict::Pass)
        );
        assert!(matches!(
            serial_bytes(&[0x42; 6], &FIB, &[0x42; 6]),
            Some(Verdict::Fail(_))
        ));
    }

    #[test]
    fn serial_text_waits_then_finds_the_marker() {
        assert_eq!(serial_text(b"01-special\n\n", "Passed", "Failed"), None);
        assert_eq!(
            serial_text(b"01-special\n\nPassed\n", "Passed", "Failed"),
            Some(Verdict::Pass)
        );
        let fail = serial_text(b"02-interrupts\nEI\nFailed #2\n", "Passed", "Failed");
        assert!(matches!(fail, Some(Verdict::Fail(d)) if d.contains("EI Failed #2")));
    }

    fn layout() -> BlarggLayout<'static> {
        BlarggLayout {
            signature_offset: 1,
            signature: &[0xDE, 0xB0, 0x61],
            status_offset: 0,
            running_value: 0x80,
            pass_value: 0,
            text_offset: 4,
        }
    }

    #[test]
    fn blargg_memory_needs_the_signature_and_a_final_status() {
        assert_eq!(
            blargg_memory(&[0, 0, 0, 0, 0], &layout()),
            None,
            "no signature yet"
        );
        assert_eq!(
            blargg_memory(&[0x80, 0xDE, 0xB0, 0x61, 0], &layout()),
            None,
            "running"
        );
        assert_eq!(
            blargg_memory(&[0, 0xDE, 0xB0, 0x61, 0], &layout()),
            Some(Verdict::Pass)
        );
        let mut failed = vec![3, 0xDE, 0xB0, 0x61];
        failed.extend_from_slice(b"05-sweep\nFailed #3\0junk");
        let verdict = blargg_memory(&failed, &layout());
        assert!(
            matches!(&verdict, Some(Verdict::Fail(d)) if d == "result code 3: 05-sweep Failed #3"),
            "{verdict:?}"
        );
        assert_eq!(blargg_memory(&[], &layout()), None, "no cart RAM");
    }

    #[test]
    fn memory_byte_distinguishes_fail_from_no_result() {
        assert_eq!(memory_byte(1, 1, 0xFF), Verdict::Pass);
        assert!(matches!(memory_byte(0xFF, 1, 0xFF), Verdict::Fail(d) if d.contains("reported")));
        assert!(matches!(memory_byte(0, 1, 0xFF), Verdict::Fail(d) if d.contains("no result")));
    }
}
