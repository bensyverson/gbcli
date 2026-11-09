//! gbcli's pixel fonts (project/2026-11-09-pixel-fonts-design.md, Ben's
//! decisions). A font is a `.yaff` file, a strict subset of monobit's YAFF
//! 1.0: glyphs 8 pixels tall and 1-8 wide, one bit a pixel, each keyed by
//! one character, a code point or a name, in file order. This crate reads
//! and writes that subset (`yaff`), sets text by the layout rules that
//! `gbcli help fonts` states (`layout`), and draws previews (`preview`).
//! It decides nothing about a project; `gbcli-build` does.

pub mod bdf;
mod chars;
mod compose;
pub mod diff;
mod error;
pub mod grid;
mod key;
mod labels;
mod layout;
mod marks;
mod model;
mod preview;
mod problem;
mod sets;
pub mod yaff;

pub use chars::{CHARS_EXTENSION, GRID_COLUMNS, Slot, read_chars, read_list, write_chars};
pub use compose::{Composed, compose};
pub use error::FontError;
pub use key::{Key, KeyError};
pub use layout::{Line, RULES, draw, set};
pub use marks::{decompose, precomposed, singleton};
pub use model::{DEFAULT_ASCENT, Font, Glyph, HEIGHT, MAX_WIDTH, Property};
pub use preview::{PREVIEW_SCALE, preview_font, preview_line};
pub use problem::Problem;
pub use sets::{SET_NAMES, named_set};
pub use yaff::{EXTENSION, read, write};
