//! The checkpoint envelope's JSON header (design §6), and the debugger
//! configuration it copies for reference. The engine writes and verifies it;
//! the server and CLI print it. One struct on both sides of the seam.

use std::fmt;
use std::str::FromStr;

use gbcli_core::{JoypadBounce, Model, Rendering, RtcMode, WriteTracking};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::rev::Rev;

/// The header of a `.gbck` envelope. Hashes are of the unpatched ROM file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointHeader {
    pub core: CoreId,
    #[serde(with = "ModelDef")]
    pub model: Model,
    pub rom_sha256: Sha256,
    /// The `.sym` loaded at save time, if any.
    pub sym_sha256: Option<Sha256>,
    pub boot_rom_sha256: Sha256,
    pub seed: u64,
    pub config: RenderConfig,
    /// Emulated time since power-on, in 2^23 Hz ticks.
    pub ticks: u64,
    /// Vblanks since power-on.
    pub frame: u64,
    /// The boot handoff (first FF50 write), if it has happened.
    pub boot: Option<TimeMark>,
    /// Held joypad buttons, one bit each (`gbcli_core::Keys`).
    pub keys: u8,
    /// ROM patches in the order they were applied.
    pub patches: Vec<Patch>,
    /// Symbols the state depends on, for the rebuilt-ROM check.
    pub symbols_live: Vec<LiveSymbol>,
    /// Breakpoints and watches at save time; for reference, not restored by default.
    pub debugger: DebuggerConfig,
    pub created_by: Option<String>,
    /// The checkpoint or head this state ran from.
    pub parent: Option<String>,
    /// For a named checkpoint, the head's revision when it was saved. Absent
    /// from files written before it was recorded, and from history entries,
    /// whose file name is their revision.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rev: Option<Rev>,
}

/// Which backend, at which commit, wrote a blob.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreId {
    pub backend: String,
    pub commit: String,
}

/// The core settings besides model, ROM, boot ROM and seed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderConfig {
    #[serde(with = "RtcModeDef")]
    pub rtc: RtcMode,
    #[serde(with = "RenderingDef")]
    pub rendering: Rendering,
    #[serde(with = "JoypadBounceDef")]
    pub joypad_bounce: JoypadBounce,
    #[serde(with = "WriteTrackingDef")]
    pub write_tracking: WriteTracking,
}

/// A moment in emulated time: ticks and vblanks since power-on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeMark {
    pub ticks: u64,
    pub frame: u64,
}

/// One ROM patch: the bytes written at a file offset, and the bytes they replaced.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Patch {
    pub offset: u32,
    #[serde(with = "crate::hex::bytes")]
    pub bytes: Vec<u8>,
    #[serde(with = "crate::hex::bytes")]
    pub before: Vec<u8>,
}

/// Why a symbol was live at save time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SymbolRole {
    /// The ROM symbol containing PC.
    Pc,
    /// A WRAM, HRAM or cart RAM symbol.
    Ram,
    /// The ROM symbol containing a word on the stack (a possible return address).
    Stack,
}

/// A symbol the saved state depends on, with its address at save time.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveSymbol {
    pub name: String,
    pub bank: u16,
    pub addr: u16,
    pub role: SymbolRole,
}

/// Breakpoints and watches, as the session holds them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DebuggerConfig {
    pub breakpoints: Vec<BreakpointSpec>,
    pub watches: Vec<WatchSpec>,
}

/// Where a breakpoint or watch applies: resolved when set (design §7).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Location {
    pub addr: u16,
    /// `None` matches every bank.
    pub bank: Option<u16>,
    /// The symbol it was set by, if any (`Name` or `Name+$3`).
    pub symbol: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BreakpointSpec {
    pub id: u32,
    pub at: Location,
    /// Condition source text (design §8).
    pub condition: Option<String>,
}

/// Which accesses a watch matches.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WatchAccess {
    Read,
    Write,
    Access,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchSpec {
    pub id: u32,
    pub access: WatchAccess,
    pub at: Location,
    pub condition: Option<String>,
    /// Delete the watch after its first stop.
    pub once: bool,
}

/// A SHA-256 digest, written as 64 lowercase hex digits.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Sha256(pub [u8; 32]);

impl fmt::Display for Sha256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&crate::hex::encode(&self.0))
    }
}

impl fmt::Debug for Sha256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Sha256({self})")
    }
}

impl FromStr for Sha256 {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        let bytes = crate::hex::decode(s)?;
        let digest: [u8; 32] = bytes
            .try_into()
            .map_err(|b: Vec<u8>| format!("a SHA-256 has 32 bytes, got {}", b.len()))?;
        Ok(Sha256(digest))
    }
}

impl Serialize for Sha256 {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Sha256 {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

/// A model's wire name, as the header spells it (`dmg-b`, `cgb-e`).
pub fn model_name(model: Model) -> &'static str {
    match model {
        Model::DmgB => "dmg-b",
        Model::Mgb => "mgb",
        Model::Sgb => "sgb",
        Model::Sgb2 => "sgb2",
        Model::Cgb0 => "cgb0",
        Model::CgbA => "cgb-a",
        Model::CgbB => "cgb-b",
        Model::CgbC => "cgb-c",
        Model::CgbD => "cgb-d",
        Model::CgbE => "cgb-e",
        Model::Agb => "agb",
    }
}

/// A model from its wire name; `None` for an unknown name.
pub fn model_from_name(name: &str) -> Option<Model> {
    Some(match name {
        "dmg-b" | "dmg" => Model::DmgB,
        "mgb" => Model::Mgb,
        "sgb" => Model::Sgb,
        "sgb2" => Model::Sgb2,
        "cgb0" | "cgb" => Model::Cgb0,
        "cgb-a" => Model::CgbA,
        "cgb-b" => Model::CgbB,
        "cgb-c" => Model::CgbC,
        "cgb-d" => Model::CgbD,
        "cgb-e" => Model::CgbE,
        "agb" => Model::Agb,
        _ => return None,
    })
}

// Serde mirrors of the core's enums. `remote` derives check every variant
// against the core's definition at compile time, so they can't drift.

#[derive(Serialize, Deserialize)]
#[serde(remote = "Model", rename_all = "kebab-case")]
pub(crate) enum ModelDef {
    DmgB,
    Mgb,
    Sgb,
    Sgb2,
    Cgb0,
    CgbA,
    CgbB,
    CgbC,
    CgbD,
    CgbE,
    Agb,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "RtcMode", rename_all = "snake_case")]
enum RtcModeDef {
    Accurate,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "Rendering", rename_all = "snake_case")]
enum RenderingDef {
    On,
    Off,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "JoypadBounce", rename_all = "snake_case")]
enum JoypadBounceDef {
    Off,
    On,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "WriteTracking", rename_all = "snake_case")]
enum WriteTrackingDef {
    On,
    Off,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header() -> CheckpointHeader {
        CheckpointHeader {
            core: CoreId {
                backend: "sameboy".into(),
                commit: "ea667ad".into(),
            },
            model: Model::CgbE,
            rom_sha256: Sha256([0xAB; 32]),
            sym_sha256: None,
            boot_rom_sha256: Sha256([1; 32]),
            seed: 7,
            config: RenderConfig {
                rtc: RtcMode::Accurate,
                rendering: Rendering::On,
                joypad_bounce: JoypadBounce::Off,
                write_tracking: WriteTracking::On,
            },
            ticks: 1234,
            frame: 3,
            boot: Some(TimeMark {
                ticks: 100,
                frame: 1,
            }),
            keys: 0x81,
            patches: vec![Patch {
                offset: 0x4123,
                bytes: vec![0, 0xC9],
                before: vec![0x3E, 0x01],
            }],
            symbols_live: vec![LiveSymbol {
                name: "wFrame".into(),
                bank: 0,
                addr: 0xC000,
                role: SymbolRole::Ram,
            }],
            debugger: DebuggerConfig::default(),
            created_by: Some("claude".into()),
            parent: None,
            rev: Some(Rev(4)),
        }
    }

    #[test]
    fn a_header_round_trips_through_json() {
        let json = serde_json::to_string(&header()).unwrap();
        let back: CheckpointHeader = serde_json::from_str(&json).unwrap();
        assert_eq!(back, header());
    }

    #[test]
    fn a_header_written_before_it_recorded_its_rev_still_parses() {
        let mut json = serde_json::to_value(header()).unwrap();
        json.as_object_mut().unwrap().remove("rev");
        let back: CheckpointHeader = serde_json::from_value(json).unwrap();
        assert_eq!(back.rev, None);
    }

    #[test]
    fn hashes_patches_and_models_are_readable_in_the_json() {
        let json = serde_json::to_value(header()).unwrap();
        assert_eq!(json["rom_sha256"], "ab".repeat(32));
        assert_eq!(json["model"], "cgb-e");
        assert_eq!(json["patches"][0]["bytes"], "00c9");
        assert_eq!(json["patches"][0]["before"], "3e01");
        assert_eq!(json["symbols_live"][0]["role"], "ram");
    }

    #[test]
    fn model_names_match_the_serialized_names() {
        use Model::*;
        for model in [
            DmgB, Mgb, Sgb, Sgb2, Cgb0, CgbA, CgbB, CgbC, CgbD, CgbE, Agb,
        ] {
            let mut h = header();
            h.model = model;
            let json = serde_json::to_value(h).unwrap();
            assert_eq!(json["model"], model_name(model));
        }
    }

    #[test]
    fn model_names_round_trip() {
        use Model::*;
        for model in [
            DmgB, Mgb, Sgb, Sgb2, Cgb0, CgbA, CgbB, CgbC, CgbD, CgbE, Agb,
        ] {
            assert_eq!(model_from_name(model_name(model)), Some(model));
        }
        assert_eq!(model_from_name("dmg"), Some(DmgB));
        assert_eq!(model_from_name("nintendo"), None);
    }

    #[test]
    fn a_malformed_hash_is_refused() {
        assert!("abc".parse::<Sha256>().is_err());
        assert!("zz".repeat(32).parse::<Sha256>().is_err());
        assert!("00".repeat(31).parse::<Sha256>().is_err());
        assert_eq!("00".repeat(32).parse::<Sha256>(), Ok(Sha256([0; 32])));
    }
}
