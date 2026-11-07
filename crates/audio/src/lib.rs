//! gbcli's audio views: the sound-register write log decoded into notes and
//! settings, WAV export (mixed and per channel), a piano roll and
//! spectrograms, as deterministic bytes.
//!
//! The core collects (`Core::set_apu_write_log`, `Core::set_audio_capture`);
//! this crate only decodes and draws what it is handed, except
//! [`record_takes`], which re-runs a span with channels muted to get
//! per-channel audio.

mod decode;
mod error;
mod fft;
mod notes;
mod piano;
mod pitch;
mod reg;
mod replay;
mod spectrogram;
mod wav;

pub use decode::{ApuSnapshot, Change, DecodedWrite, decode};
pub use error::AudioError;
pub use notes::{NoteSpan, note_spans};
pub use piano::{MAX_WIDTH, PianoRoll, piano_roll};
pub use pitch::{Note, NoteName, Pitch, midi_of, nearest_note, noise_hz};
pub use reg::{Direction, Duty, Envelope, LfsrWidth, SoundReg, Sweep, WaveLevel, channel_name};
pub use replay::{Takes, record_takes};
pub use spectrogram::{FLOOR_DB, HOP, HZ_PER_ROW, LEVELS, ROWS, Spectrum, spectrogram, spectrum};
pub use wav::wav;
