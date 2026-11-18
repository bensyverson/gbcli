//! gbcli's image views over a core: the frame at native size or an integer
//! scale, crops, the last complete frame or the live partial one (always
//! labeled), filmstrips and atlases of frames the caller collected, frame
//! diffs, and the tileset, tilemap and OAM views. Each renders to an
//! [`Image`] whose [`png`](Image::png) bytes are deterministic.
//!
//! Every view only reads the core (design §2.3): none runs, steps or writes
//! it, so a screenshot never changes the machine. Colors are the core's own
//! output; see [`colors`]. The structured siblings of the debug views are in
//! `gbcli_engine::inspect::video`.

pub mod colors;
mod encode;
mod error;
mod frame;
mod image;
mod screen;
mod sheet;
mod tiles;

pub use colors::{DMG_GRAYS, TilePalette};
pub use error::ViewError;
pub use frame::{Frame, FrameChoice, FrameSource, capture};
pub use image::{DEFAULT_SCALE, Image, LabelKey, MAX_SCALE, Rect};
pub use screen::{fingerprint, screen};
pub use sheet::{DiffSummary, diff, filmstrip, sheet};
pub use tiles::{MapChoice, MapLayout, map_layout, oam_sheet, object_height, tilemap, tileset};
