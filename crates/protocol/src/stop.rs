//! The stop record: the one shape every run and step returns (design §2.2).

use gbcli_core::{Access, BudgetKind, Usage, VblankKind};
use serde::{Deserialize, Serialize};

/// Why the engine stopped, after deciding on the core's candidates.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum StopReason {
    /// A breakpoint whose condition held, before its instruction ran.
    Breakpoint { id: u32 },
    /// A watch whose condition held, after the access.
    Watchpoint {
        id: u32,
        #[serde(with = "AccessDef")]
        access: Access,
        addr: u16,
        /// `None` when the byte couldn't be known without a bus read.
        old: Option<u8>,
        new: u8,
        /// The PC of the instruction that made the access.
        writer_pc: u16,
    },
    /// A step finished.
    StepDone { unit: StepKind },
    /// A budget limit was reached first.
    BudgetExhausted {
        #[serde(with = "BudgetKindDef")]
        kind: BudgetKind,
    },
    /// The next instruction is an illegal opcode, which would hang the CPU.
    IllegalOpcode { opcode: u8 },
    /// HALT with no interrupt enabled in IE: nothing can wake the CPU.
    HaltForever,
    /// STOP with no key held to wake it.
    Stopped,
    /// The caller asked the run to stop.
    UserBreak,
}

/// What a step stepped by.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepKind {
    Instruction,
    /// Over a CALL or RST: until it returns.
    Over,
    /// Until the current routine returns.
    Out,
    /// Until the next vblank.
    Frame,
    /// Until LY changes.
    Scanline,
    /// Until PC reaches an address.
    ToAddress,
}

/// What a frame or cycle count counts from (design §9).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "name", rename_all = "kebab-case")]
pub enum TimeBase {
    PowerOn,
    /// The boot handoff: the first write to FF50.
    Boot,
    /// A named checkpoint.
    Checkpoint(String),
}

/// When a stop happened.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StopTime {
    /// Since power-on, in 2^23 Hz ticks.
    pub ticks: u64,
    /// CPU T-cycles since `since` (negative before it), at the current speed.
    pub t: i64,
    pub since: TimeBase,
    /// Vblanks since `since`.
    pub frame: i64,
    pub ly: u8,
    /// `None` when the core can't report it (always, on SameBoy).
    pub dot: Option<u16>,
}

/// Which frame the last complete framebuffer holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameShown {
    /// Its number since the time base.
    pub n: i64,
    #[serde(with = "VblankKindDef")]
    pub vblank: VblankKind,
}

/// How much of its budget a run used.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetUse {
    pub ticks: u64,
    pub instructions: u64,
    pub frames: u64,
}

impl From<Usage> for BudgetUse {
    fn from(u: Usage) -> Self {
        BudgetUse {
            ticks: u.ticks,
            instructions: u.instructions,
            frames: u.frames,
        }
    }
}

/// The engine's answer to every run and step.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StopRecord {
    #[serde(flatten)]
    pub reason: StopReason,
    /// The next instruction's address.
    pub pc: u16,
    /// The bank mapped at `pc`.
    pub bank: u16,
    /// The symbol containing `pc`, as `Name` or `Name+3`.
    pub symbol: Option<String>,
    pub time: StopTime,
    pub frame_shown: Option<FrameShown>,
    pub budget: BudgetUse,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "Access", rename_all = "snake_case")]
enum AccessDef {
    Exec,
    Read,
    Write,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "BudgetKind", rename_all = "snake_case")]
enum BudgetKindDef {
    Ticks,
    Instructions,
    Frames,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "VblankKind", rename_all = "snake_case")]
enum VblankKindDef {
    Normal,
    LcdOff,
    Artificial,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stop_record_flattens_its_reason_and_names_its_time_base() {
        let record = StopRecord {
            reason: StopReason::Watchpoint {
                id: 2,
                access: Access::Write,
                addr: 0xFF46,
                old: Some(0),
                new: 0xC0,
                writer_pc: 0xFF80,
            },
            pc: 0xFF82,
            bank: 1,
            symbol: Some("hOAMDMA+2".into()),
            time: StopTime {
                ticks: 12_345_678,
                t: -5,
                since: TimeBase::Boot,
                frame: 61,
                ly: 144,
                dot: None,
            },
            frame_shown: Some(FrameShown {
                n: 60,
                vblank: VblankKind::Normal,
            }),
            budget: BudgetUse {
                ticks: 1,
                instructions: 2,
                frames: 31,
            },
        };
        let json = serde_json::to_value(&record).unwrap();
        assert_eq!(json["reason"], "watchpoint");
        assert_eq!(json["access"], "write");
        assert_eq!(json["time"]["since"]["kind"], "boot");
        assert_eq!(json["time"]["dot"], serde_json::Value::Null);
        assert_eq!(json["frame_shown"]["vblank"], "normal");
        let back: StopRecord = serde_json::from_value(json).unwrap();
        assert_eq!(back, record);
    }
}
