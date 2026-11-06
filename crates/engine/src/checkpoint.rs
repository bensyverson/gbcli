//! In-memory checkpoints and the `.gbck` envelope (design §6):
//! `"GBCK" | u32 version | u32 header_len | header JSON | core blob`, integers
//! little-endian.

use gbcli_core::Model;
use gbcli_protocol::CheckpointHeader;

use crate::checkpoint_error::CheckpointError;

/// The envelope's first four bytes.
pub const MAGIC: [u8; 4] = *b"GBCK";
/// The envelope version this engine writes and reads.
pub const VERSION: u32 = 1;
const PREFIX: usize = 12;

/// A saved machine: the header that verifies it and the core's own blob.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checkpoint {
    pub header: CheckpointHeader,
    pub blob: Vec<u8>,
}

impl Checkpoint {
    /// The envelope bytes. Deterministic: the same checkpoint always encodes
    /// to the same bytes.
    pub fn encode(&self) -> Vec<u8> {
        let header = serde_json::to_vec(&self.header).expect("a header always serializes");
        let mut out = Vec::with_capacity(PREFIX + header.len() + self.blob.len());
        out.extend_from_slice(&MAGIC);
        out.extend_from_slice(&VERSION.to_le_bytes());
        out.extend_from_slice(&(header.len() as u32).to_le_bytes());
        out.extend_from_slice(&header);
        out.extend_from_slice(&self.blob);
        out
    }

    /// Reads an envelope: verification step 1 (magic and version). The other
    /// steps run when the checkpoint is restored into a machine.
    pub fn decode(bytes: &[u8]) -> Result<Checkpoint, CheckpointError> {
        let unreadable = |reason: String| CheckpointError::Unreadable { reason };
        if bytes.len() < PREFIX {
            return Err(unreadable(format!(
                "{} bytes is too short for an envelope",
                bytes.len()
            )));
        }
        if bytes[..4] != MAGIC {
            return Err(unreadable("not a gbcli checkpoint (no GBCK magic)".into()));
        }
        let word = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        let version = word(4);
        if version != VERSION {
            return Err(unreadable(format!(
                "envelope version {version}; this gbcli reads version {VERSION}"
            )));
        }
        let header_len = word(8) as usize;
        let header_end = PREFIX
            .checked_add(header_len)
            .filter(|&end| end <= bytes.len())
            .ok_or_else(|| unreadable("truncated inside the header".into()))?;
        let header = serde_json::from_slice(&bytes[PREFIX..header_end])
            .map_err(|e| unreadable(format!("the header doesn't parse: {e}")))?;
        Ok(Checkpoint {
            header,
            blob: bytes[header_end..].to_vec(),
        })
    }
}

/// Who made a checkpoint and what it ran from; recorded as given.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SaveMeta {
    pub created_by: Option<String>,
    pub parent: Option<String>,
}

/// What a restore may use or override.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LoadOptions {
    /// A model the caller asked for; a conflict with the envelope is refused.
    pub model: Option<Model>,
    /// Boot ROMs to search by hash, besides the backend's built-in ones and
    /// the machine's own.
    pub boot_roms: Vec<Vec<u8>>,
    /// Load under a changed ROM even if the symbol check refuses.
    pub allow_rom_change: bool,
}
