//! Why a store operation failed. Each names the thing, what happened and what
//! to do next; the server maps each to one protocol code (design §5).

use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

use gbcli_protocol::ErrorCode;

use gbcli_engine::{CheckpointError, LoadWarning, SetupError};

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
    /// A session's recorded settings don't build a machine (bad model, bad seed).
    Configuration {
        what: String,
    },
    /// The boot ROM never reached the handoff, so there is no boot start state.
    NoHandoff,
    Checkpoint(CheckpointError),
    Setup(SetupError),
}

impl StoreError {
    pub fn code(&self) -> ErrorCode {
        match self {
            StoreError::Io { .. } | StoreError::NoRoot { .. } => ErrorCode::Refused,
            StoreError::NoSession { .. } => ErrorCode::NoSession,
            StoreError::BadSessionName { .. } => ErrorCode::BadRequest,
            StoreError::Busy { .. } => ErrorCode::SessionBusy,
            StoreError::HeadMissing { .. } => ErrorCode::CheckpointUnreadable,
            StoreError::RevMismatch { .. } => ErrorCode::RevMismatch,
            StoreError::CheckpointMissing { .. } => ErrorCode::CheckpointUnreadable,
            StoreError::CheckpointsPresent { .. } => ErrorCode::Refused,
            StoreError::RomUnreadable { .. } => ErrorCode::RomUnreadable,
            StoreError::SymbolsUnreadable { .. } => ErrorCode::SymbolsUnreadable,
            StoreError::Configuration { .. } => ErrorCode::BadRequest,
            StoreError::NoHandoff => ErrorCode::Refused,
            StoreError::Checkpoint(e) => checkpoint_code(e),
            StoreError::Setup(e) => setup_code(e),
        }
    }

    pub fn remedy(&self) -> String {
        match self {
            StoreError::Io { path, .. } => format!("check permissions on {}", path.display()),
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
            StoreError::BadSessionName { .. } => "names are [a-z0-9-], 1 to 40 characters".into(),
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
            StoreError::Configuration { .. } => "check the session's model and seed".into(),
            StoreError::NoHandoff => {
                "pass --from power-on, which runs without the boot cache".into()
            }
            StoreError::Checkpoint(e) => checkpoint_remedy(e),
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

fn checkpoint_code(e: &CheckpointError) -> ErrorCode {
    match e {
        CheckpointError::OtherBackend { .. }
        | CheckpointError::ModelConflict { .. }
        | CheckpointError::CoreRejected { .. } => ErrorCode::CheckpointMismatch,
        CheckpointError::RomChanged { .. } => ErrorCode::RomChanged,
        CheckpointError::BootRomMissing { .. } => ErrorCode::BootRomMissing,
        _ => ErrorCode::CheckpointUnreadable,
    }
}

fn checkpoint_remedy(e: &CheckpointError) -> String {
    match e {
        CheckpointError::Unknown { known, .. } if known.is_empty() => {
            "this session has no named checkpoints".into()
        }
        CheckpointError::Unknown { known, .. } => {
            format!("known checkpoints: {}", known.join(", "))
        }
        CheckpointError::RomChanged { moved } => {
            let names: Vec<&str> = moved.iter().map(|m| m.name.as_str()).collect();
            format!(
                "this checkpoint's state depends on symbols that moved ({}); restore a checkpoint of this build, or pass --allow-rom-change",
                names.join(", ")
            )
        }
        CheckpointError::BootRomMissing { sha256 } => {
            format!("pass --boot-rom <file> whose SHA-256 is {sha256}")
        }
        CheckpointError::OtherBackend { ours, .. } => {
            format!("this build is {ours}; rebuild the checkpoint with it")
        }
        CheckpointError::ModelConflict { requested, .. } => {
            format!("drop --model {requested:?}, or restore without it")
        }
        other => format!("{other}; delete and recreate the checkpoint"),
    }
}

fn setup_code(e: &SetupError) -> ErrorCode {
    match e {
        SetupError::Core(_) => ErrorCode::CheckpointMismatch,
        SetupError::Symbols(_) => ErrorCode::SymbolsUnreadable,
    }
}

fn setup_remedy(e: &SetupError) -> String {
    match e {
        SetupError::Core(_) => "the core refused to build this ROM; check its header".into(),
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
            StoreError::Configuration { what } => write!(f, "{what}"),
            StoreError::NoHandoff => write!(f, "the boot ROM never wrote FF50 (no handoff)"),
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
