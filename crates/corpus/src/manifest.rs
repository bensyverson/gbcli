//! The corpus manifest (`tests/corpus/manifest.json`, schema 1) as typed
//! structs. The schema is documented in `project/2026-11-06-test-rom-corpus.md`.

use serde::Deserialize;

/// The manifest schema this runner reads.
pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Deserialize)]
pub struct Manifest {
    pub schema_version: u32,
    pub rows: Vec<Row>,
}

/// One test: a ROM plus, where references differ by model, one model variant.
#[derive(Clone, Debug, Deserialize)]
pub struct Row {
    pub id: String,
    pub suite: String,
    /// Relative to `tests/corpus/fetched/`.
    pub path: String,
    pub models: Vec<ModelSpec>,
    pub pass: PassRule,
    #[serde(default)]
    pub pass_alternatives: Vec<PassRule>,
    pub exit: Exit,
    /// Upstream's stated bound in emulated seconds; `None` where it states none.
    pub timeout_s: Option<f64>,
    #[serde(default)]
    pub boot_sensitive: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Family {
    Dmg,
    Mgb,
    Sgb,
    Sgb2,
    Cgb,
    Agb,
}

/// A model family and, optionally, the revisions the row is valid on.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct ModelSpec {
    pub family: Family,
    /// Omitted means any revision.
    #[serde(default)]
    pub revisions: Option<Vec<String>>,
}

/// A named frame palette from the manifest's `palettes` table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Palette {
    /// DMG shades as 8-bit gray $FF, $AA, $55, $00.
    DmgGray,
    /// CGB colors, each 5-bit channel x as (x << 3) | (x >> 2).
    CgbRgb555,
    /// A DMG ROM on CGB, colored by the boot ROM's compatibility palette.
    CgbCompatCsp,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PassRule {
    FibonacciRegisters {
        b: u8,
        c: u8,
        d: u8,
        e: u8,
        h: u8,
        l: u8,
        fail_value: u8,
    },
    SerialBytes {
        bytes: Vec<u8>,
        fail_bytes: Vec<u8>,
    },
    SerialText {
        pass_contains: String,
        fail_contains: String,
    },
    BlarggMemory {
        signature_address: u16,
        signature: Vec<u8>,
        status_address: u16,
        running_value: u8,
        pass_value: u8,
        text_address: u16,
    },
    Framebuffer {
        reference: String,
        palette: Palette,
    },
    MemoryByte {
        address: u16,
        pass_value: u8,
        fail_value: u8,
    },
    DocboyCase {
        config: String,
        category: String,
        index: usize,
    },
    Manual {
        reason: String,
    },
}

/// When to judge a row.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Exit {
    LdBB,
    EmulatedSeconds { seconds: f64 },
    EmulatedFrames { frames: u32 },
    ResultReported,
    DocboyCase,
}

/// Parses a manifest, refusing a schema this runner doesn't know.
pub fn parse(text: &str) -> Result<Manifest, String> {
    let manifest: Manifest = serde_json::from_str(text).map_err(|e| format!("manifest: {e}"))?;
    if manifest.schema_version != SCHEMA_VERSION {
        return Err(format!(
            "manifest schema {} is not the {SCHEMA_VERSION} this runner reads",
            manifest.schema_version
        ));
    }
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(rows: &str) -> String {
        format!(r#"{{"schema_version": 1, "palettes": {{}}, "rows": [{rows}]}}"#)
    }

    #[test]
    fn a_mooneye_row_parses_with_its_alternative() {
        let text = manifest(
            r#"{"id": "mooneye/a.gb", "suite": "mooneye", "path": "mooneye/a.gb",
            "models": [{"family": "dmg"}, {"family": "cgb", "revisions": ["C", "E"]}],
            "pass": {"kind": "fibonacci_registers", "b": 3, "c": 5, "d": 8, "e": 13, "h": 21, "l": 34, "fail_value": 66},
            "exit": {"kind": "ld_b_b"}, "timeout_s": 120, "license": "MIT",
            "pass_alternatives": [{"kind": "serial_bytes", "bytes": [3, 5], "fail_bytes": [66, 66]}]}"#,
        );
        let row = &parse(&text).unwrap().rows[0];
        assert_eq!(row.models[1].family, Family::Cgb);
        assert_eq!(
            row.models[1].revisions.as_deref(),
            Some(&["C".into(), "E".into()][..])
        );
        assert_eq!(row.models[0].revisions, None);
        assert!(matches!(
            row.pass,
            PassRule::FibonacciRegisters {
                l: 34,
                fail_value: 66,
                ..
            }
        ));
        assert_eq!(row.exit, Exit::LdBB);
        assert_eq!(row.timeout_s, Some(120.0));
        assert_eq!(
            row.pass_alternatives,
            vec![PassRule::SerialBytes {
                bytes: vec![3, 5],
                fail_bytes: vec![66, 66]
            }]
        );
    }

    #[test]
    fn framebuffer_blargg_memory_and_timed_exits_parse() {
        let text = manifest(
            r#"{"id": "x@cgb", "suite": "blargg", "path": "x.gb", "models": [{"family": "cgb"}],
            "pass": {"kind": "framebuffer", "reference": "x.png", "palette": "cgb_compat_csp"},
            "exit": {"kind": "emulated_seconds", "seconds": 0.5}, "timeout_s": null, "license": "x"},
            {"id": "y", "suite": "blargg", "path": "y.gb", "models": [{"family": "dmg"}],
            "pass": {"kind": "blargg_memory", "signature_address": 40961, "signature": [222, 176, 97],
                     "status_address": 40960, "running_value": 128, "pass_value": 0, "text_address": 40964},
            "exit": {"kind": "emulated_frames", "frames": 2}, "timeout_s": 37, "license": "x", "boot_sensitive": true}"#,
        );
        let rows = parse(&text).unwrap().rows;
        assert_eq!(
            rows[0].pass,
            PassRule::Framebuffer {
                reference: "x.png".into(),
                palette: Palette::CgbCompatCsp
            }
        );
        assert_eq!(rows[0].exit, Exit::EmulatedSeconds { seconds: 0.5 });
        assert_eq!(rows[0].timeout_s, None);
        assert!(!rows[0].boot_sensitive);
        assert!(matches!(
            rows[1].pass,
            PassRule::BlarggMemory {
                status_address: 0xA000,
                ..
            }
        ));
        assert_eq!(rows[1].exit, Exit::EmulatedFrames { frames: 2 });
        assert!(rows[1].boot_sensitive);
    }

    #[test]
    fn an_unknown_rule_kind_is_an_error() {
        let text = manifest(
            r#"{"id": "z", "suite": "s", "path": "z.gb", "models": [], "pass": {"kind": "vibes"},
            "exit": {"kind": "ld_b_b"}, "timeout_s": null}"#,
        );
        assert!(parse(&text).unwrap_err().contains("vibes"));
    }

    #[test]
    fn another_schema_version_is_refused() {
        let err = parse(r#"{"schema_version": 2, "rows": []}"#).unwrap_err();
        assert!(err.contains("schema 2"), "{err}");
    }
}
