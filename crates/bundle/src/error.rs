//! Why a store operation failed. Each names the thing, what happened and what
//! to do next; the server maps each to one protocol code (design §5).

use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

use gbcli_protocol::{ErrorCode, Rev};

use gbcli_core::CoreError;
use gbcli_engine::{CheckpointError, DebugError, LoadWarning, SetupError};

#[derive(Debug)]
pub enum StoreError {
    /// A filesystem operation failed.
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    /// No `.gbcli/` above the working directory, and none could be made.
    NoRoot {
        start: PathBuf,
        source: std::io::Error,
    },
    NoSession {
        name: String,
        known: Vec<String>,
    },
    /// No `.gbcli/` where the search looked, for a caller that may not
    /// create one: `start` and the directories above it, or `GBCLI_DIR`.
    NoBundle {
        start: PathBuf,
        gbcli_dir: Option<PathBuf>,
    },
    BadSessionName {
        name: String,
    },
    /// Another process held the session's lock past the wait budget.
    Busy {
        name: String,
        waited: Duration,
    },
    /// The head is missing although the session's revision says it exists.
    HeadMissing {
        name: String,
        rev: u64,
    },
    RevMismatch {
        expected: u64,
        found: u64,
        by: Option<String>,
    },
    CheckpointMissing {
        name: String,
        known: Vec<String>,
    },
    /// `sessions rm` refuses while named checkpoints exist.
    CheckpointsPresent {
        session: String,
        checkpoints: Vec<String>,
    },
    RomUnreadable {
        path: PathBuf,
        reason: String,
    },
    SymbolsUnreadable {
        path: PathBuf,
        reason: String,
    },
    /// `create` on a name that already has a session.
    SessionExists {
        name: String,
    },
    /// `--rom` names another file than the existing session was made from.
    RomMismatch {
        name: String,
        /// The ROM path in `session.json`.
        stored: String,
        given: PathBuf,
    },
    /// A model, seed or `.sym` given for an existing session that has another.
    SettingDiffers {
        name: String,
        setting: Setting,
        /// The session's value, as its flag spells it.
        session: String,
        given: String,
    },
    /// The session's `session.json` exists but doesn't parse.
    SessionUnreadable {
        name: String,
        path: PathBuf,
        reason: String,
    },
    /// `undo` with no earlier entry kept: the head is the oldest state.
    UndoPastOldest {
        name: String,
        head: Rev,
        /// The oldest entry kept, which the walk has reached; `None` before
        /// the first head.
        oldest: Option<Rev>,
    },
    /// An `--at ~N` that isn't `~` and a number.
    BadHistoryRef {
        text: String,
    },
    BadCheckpointName {
        name: String,
    },
    /// A pruning duration that isn't a number and a unit.
    BadDuration {
        text: String,
    },
    /// The run to the boot handoff failed.
    BootFailed(DebugError),
    /// The boot ROM never reached the handoff, so there is no boot start state.
    NoHandoff,
    /// The caller's wall guard ended the boot run before the handoff.
    BootInterrupted,
    Checkpoint(CheckpointError),
    Setup(SetupError),
}

/// A session setting a caller can give when opening one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Setting {
    Model,
    Seed,
    Sym,
}

impl Setting {
    /// The flag that gives it.
    pub fn flag(self) -> &'static str {
        match self {
            Setting::Model => "--model",
            Setting::Seed => "--seed",
            Setting::Sym => "--sym",
        }
    }
}

impl StoreError {
    pub fn code(&self) -> ErrorCode {
        match self {
            StoreError::Io { .. } | StoreError::NoRoot { .. } => ErrorCode::IoFailed,
            StoreError::NoSession { .. } | StoreError::NoBundle { .. } => ErrorCode::NoSession,
            StoreError::BadSessionName { .. } => ErrorCode::BadRequest,
            StoreError::Busy { .. } => ErrorCode::SessionBusy,
            StoreError::HeadMissing { .. } => ErrorCode::CheckpointUnreadable,
            StoreError::RevMismatch { .. } => ErrorCode::RevMismatch,
            StoreError::CheckpointMissing { .. } => ErrorCode::BadRequest,
            StoreError::CheckpointsPresent { .. } => ErrorCode::Refused,
            StoreError::RomUnreadable { .. } => ErrorCode::RomUnreadable,
            StoreError::SymbolsUnreadable { .. } => ErrorCode::SymbolsUnreadable,
            StoreError::SessionExists { .. }
            | StoreError::RomMismatch { .. }
            | StoreError::SettingDiffers { .. }
            | StoreError::UndoPastOldest { .. }
            | StoreError::BadHistoryRef { .. }
            | StoreError::BadCheckpointName { .. }
            | StoreError::BadDuration { .. } => ErrorCode::BadRequest,
            StoreError::SessionUnreadable { .. } => ErrorCode::SessionUnreadable,
            StoreError::BootFailed(e) => e.code(),
            StoreError::NoHandoff => ErrorCode::Refused,
            StoreError::BootInterrupted => ErrorCode::BudgetExhausted,
            StoreError::Checkpoint(e) => e.code(),
            StoreError::Setup(e) => setup_code(e),
        }
    }

    pub fn remedy(&self) -> String {
        match self {
            StoreError::Io { path, .. } => format!(
                "check permissions on {} and the free disk space",
                path.display()
            ),
            StoreError::NoRoot { start, .. } => format!(
                "create a writable project directory, or set GBCLI_DIR (tried {})",
                start.display()
            ),
            StoreError::NoSession { known, .. } if known.is_empty() => {
                "pass --session <name> --rom <file> to create one".into()
            }
            StoreError::NoSession { known, .. } => {
                format!("known sessions: {}", known.join(", "))
            }
            StoreError::NoBundle { .. } => {
                "create a session, which makes .gbcli/ in the working directory: \
                 `gbcli run game.gb --session NAME` (or pass --dir to a project that has one)"
                    .into()
            }
            StoreError::SettingDiffers {
                setting, session, ..
            } => format!(
                "drop {flag} (the session keeps {session}), or name a new session with \
                 --session NAME",
                flag = setting.flag()
            ),
            StoreError::BadSessionName { .. } => {
                format!("names are {}", gbcli_protocol::SESSION_NAME_RULE)
            }
            StoreError::Busy { .. } => {
                "wait for the other writer to finish, or use a different --session".into()
            }
            StoreError::HeadMissing { name, .. } => format!(
                "the session {name} has no head; run a request that advances it, or recreate it"
            ),
            StoreError::RevMismatch { found, by, .. } => match by {
                Some(who) => format!(
                    "re-read the session (rev s{found}, last changed by {who}), then repeat"
                ),
                None => format!("re-read the session (now rev s{found}), then repeat"),
            },
            StoreError::CheckpointMissing { known, .. } if known.is_empty() => {
                "this session has no named checkpoints".into()
            }
            StoreError::CheckpointMissing { known, .. } => {
                format!("known checkpoints: {}", known.join(", "))
            }
            StoreError::CheckpointsPresent { checkpoints, .. } => format!(
                "delete them first, or pass --with-checkpoints ({} would go)",
                checkpoints.join(", ")
            ),
            StoreError::RomUnreadable { path, .. } => format!("check {}", path.display()),
            StoreError::SymbolsUnreadable { path, .. } => {
                format!("rebuild with `-n {}`, or pass --sym <file>", path.display())
            }
            StoreError::SessionExists { name } => format!(
                "use another --session name, or remove this one with `gbcli sessions rm {name}`"
            ),
            StoreError::RomMismatch { name, stored, .. } => format!(
                "omit --rom to open {name} over {stored}, use another --session name, or remove \
                 it with `gbcli sessions rm {name}` and create it again"
            ),
            StoreError::SessionUnreadable { name, path, .. } => format!(
                "fix {} by hand, or remove the session with `gbcli sessions rm {name}` and \
                 create it again",
                path.display()
            ),
            StoreError::UndoPastOldest { oldest: None, .. } => {
                "this session has no earlier state; run it first".into()
            }
            StoreError::UndoPastOldest {
                head,
                oldest: Some(oldest),
                ..
            } => {
                let reached = if head == oldest {
                    format!("the head, {head}, is the oldest state kept")
                } else {
                    format!(
                        "undo has reached {oldest}, the oldest state kept, which the head, {head}, holds"
                    )
                };
                format!(
                    "{reached}; `run --from power-on` starts over, and `gbcli checkpoint <name>` \
                     keeps a state for longer"
                )
            }
            StoreError::BadHistoryRef { .. } => {
                "use ~N for the head N requests back, e.g. ~1".into()
            }
            StoreError::BadCheckpointName { .. } => {
                "use letters, digits, -, _ and . (1 to 64 characters)".into()
            }
            StoreError::BadDuration { .. } => {
                "use a number and a unit (s, m, h or d): 30m, 12h or 7d".into()
            }
            StoreError::BootFailed(_) => "pass --from power-on to run the boot ROM inside the \
                 request and see where it stops"
                .into(),
            StoreError::NoHandoff => {
                "pass --from power-on, which runs without the boot cache".into()
            }
            StoreError::BootInterrupted => {
                "retry: the boot run takes well under a second unless the host is starved; \
                 nothing was cached"
                    .into()
            }
            StoreError::Checkpoint(e) => e.remedy(),
            StoreError::Setup(e) => setup_remedy(e),
        }
    }

    /// Warnings a load turned into workable advice.
    pub fn warning_text(warning: &LoadWarning) -> String {
        match warning {
            LoadWarning::RomChanged {
                moved,
                checked,
                forced,
            } if *forced => format!(
                "the ROM changed; loaded anyway with --allow-rom-change ({} symbols moved, {checked} checked)",
                moved.len()
            ),
            LoadWarning::RomChanged { checked, .. } => {
                format!("the ROM changed but no live symbol moved ({checked} checked)")
            }
            LoadWarning::OtherCommit { saved, ours } => format!(
                "the checkpoint came from SameBoy {saved}, this build is {ours}; the core accepted it"
            ),
            LoadWarning::PatchOverChangedBytes { offset } => {
                format!("a patch at ROM offset {offset:#x} differs from the bytes it recorded")
            }
        }
    }
}

fn setup_code(e: &SetupError) -> ErrorCode {
    match e {
        SetupError::Core(CoreError::Unsupported { .. }) => ErrorCode::Unsupported,
        SetupError::Core(
            CoreError::OutOfRange { .. }
            | CoreError::InvalidState { .. }
            | CoreError::Backend { .. },
        ) => ErrorCode::RomUnreadable,
        SetupError::Symbols(_) => ErrorCode::SymbolsUnreadable,
    }
}

fn setup_remedy(e: &SetupError) -> String {
    match e {
        SetupError::Core(CoreError::Unsupported { reason, .. }) => {
            format!(
                "the core can't run this ROM ({reason}); `gbcli gaps` lists what it can't do \
                 that this project has hit"
            )
        }
        SetupError::Core(_) => {
            "the core refused this ROM; check that the file is a Game Boy ROM with a full header"
                .into()
        }
        SetupError::Symbols(_) => "fix or drop the .sym file".into(),
    }
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::Io { path, source } => write!(f, "{}: {source}", path.display()),
            StoreError::NoRoot { start, source } => {
                write!(f, "no .gbcli/ from {}: {source}", start.display())
            }
            StoreError::NoSession { name, .. } => write!(f, "no session {name:?}"),
            StoreError::NoBundle {
                gbcli_dir: Some(dir),
                ..
            } => write!(
                f,
                "GBCLI_DIR names {}, which is not a directory",
                dir.display()
            ),
            StoreError::NoBundle { start, .. } => write!(
                f,
                "no .gbcli/ in {} or any directory above it",
                start.display()
            ),
            StoreError::SettingDiffers {
                name,
                setting,
                session,
                given,
            } => write!(
                f,
                "session {name:?} has {flag} {session}, not {given}",
                flag = setting.flag()
            ),
            StoreError::BadSessionName { name } => write!(f, "bad session name {name:?}"),
            StoreError::Busy { name, waited } => {
                write!(f, "session {name:?} stayed locked for {waited:?}")
            }
            StoreError::HeadMissing { name, rev } => {
                write!(f, "session {name:?} has no head at rev {rev}")
            }
            StoreError::RevMismatch {
                expected, found, ..
            } => write!(f, "rev mismatch: asked for s{expected}, found s{found}"),
            StoreError::CheckpointMissing { name, .. } => {
                write!(f, "no checkpoint {name:?}")
            }
            StoreError::CheckpointsPresent {
                session,
                checkpoints,
            } => write!(
                f,
                "session {session:?} has {} named checkpoint(s): {}",
                checkpoints.len(),
                checkpoints.join(", ")
            ),
            StoreError::RomUnreadable { path, reason } => {
                write!(f, "can't read ROM {}: {reason}", path.display())
            }
            StoreError::SymbolsUnreadable { path, reason } => {
                write!(f, "can't read symbols {}: {reason}", path.display())
            }
            StoreError::SessionExists { name } => write!(f, "session {name:?} already exists"),
            StoreError::RomMismatch {
                name,
                stored,
                given,
            } => write!(f, "session {name:?} runs {stored}, not {}", given.display()),
            StoreError::SessionUnreadable { path, reason, .. } => {
                write!(f, "{} doesn't parse: {reason}", path.display())
            }
            StoreError::UndoPastOldest { name, .. } => {
                write!(f, "session {name:?} has no earlier state to undo to")
            }
            StoreError::BadHistoryRef { text } => {
                write!(f, "{text:?} is not a history reference")
            }
            StoreError::BadCheckpointName { name } => {
                write!(f, "{name:?} is not a checkpoint name")
            }
            StoreError::BadDuration { text } => write!(f, "{text:?} is not a duration"),
            StoreError::BootFailed(e) => write!(f, "the boot ROM didn't reach the handoff: {e}"),
            StoreError::NoHandoff => write!(f, "the boot ROM never wrote FF50 (no handoff)"),
            StoreError::BootInterrupted => {
                write!(f, "the wall guard ended the boot run before the handoff")
            }
            StoreError::Checkpoint(e) => write!(f, "{e}"),
            StoreError::Setup(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<CheckpointError> for StoreError {
    fn from(e: CheckpointError) -> Self {
        StoreError::Checkpoint(e)
    }
}

impl From<SetupError> for StoreError {
    fn from(e: SetupError) -> Self {
        StoreError::Setup(e)
    }
}

#[cfg(test)]
mod tests;
