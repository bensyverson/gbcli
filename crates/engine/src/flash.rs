//! Flash notes as a run reports them: warning sentences, with the next step
//! where there is one, and rows for `--json`.

use gbcli_core::flash::{FlashNote, FlashNotes, NoteKind};
use gbcli_protocol::{FlashNoteKind, FlashNoteRow};

/// Where a flash session's chip image goes beside its `.sav`: the same name
/// with `.flash.gb` for the extension (`game.sav` → `game.flash.gb`). A ROM
/// file, because it is one: the whole chip a flasher would write to the
/// cart, which any emulator runs and whose game then finds its save.
pub fn flash_image_path(sav: &std::path::Path) -> std::path::PathBuf {
    sav.with_extension("flash.gb")
}

/// One warning per note, and one for any dropped past the chip's cap. A
/// mask-ROM cart's flash commands name the flag that models the board.
pub fn flash_warnings(notes: &FlashNotes) -> Vec<String> {
    let mut out: Vec<String> = notes.notes.iter().map(warning).collect();
    if notes.dropped > 0 {
        out.push(format!(
            "{} more flash notes were dropped past the first {}",
            notes.dropped,
            notes.notes.len()
        ));
    }
    out
}

/// The notes as `--json` rows.
pub fn flash_note_rows(notes: &FlashNotes) -> Vec<FlashNoteRow> {
    notes
        .notes
        .iter()
        .map(|note| FlashNoteRow {
            ticks: note.at,
            addr: note.bus_addr,
            value: note.value,
            kind: kind(note.kind),
            board: match note.kind {
                NoteKind::NoChip { board } => Some(board.to_string()),
                _ => None,
            },
            text: warning(note),
        })
        .collect()
}

fn warning(note: &FlashNote) -> String {
    match note.kind {
        NoteKind::NoChip { board } => format!(
            "{note}; to model the flash-save cart it was written for, create the session with \
             --flash {board} (`gbcli help flash` lists the boards)"
        ),
        _ => note.to_string(),
    }
}

fn kind(kind: NoteKind) -> FlashNoteKind {
    match kind {
        NoteKind::DataLines { .. } => FlashNoteKind::DataLines,
        NoteKind::UnlockAddress { .. } => FlashNoteKind::UnlockAddress,
        NoteKind::WritePinAudio => FlashNoteKind::WritePinAudio,
        NoteKind::BrokenSequence { .. } => FlashNoteKind::BrokenSequence,
        NoteKind::ProgramSetsBits { .. } => FlashNoteKind::ProgramSetsBits,
        NoteKind::WriteWhileBusy { .. } => FlashNoteKind::WriteWhileBusy,
        NoteKind::CutOff { .. } => FlashNoteKind::CutOff,
        NoteKind::NoChip { .. } => FlashNoteKind::NoChip,
    }
}
