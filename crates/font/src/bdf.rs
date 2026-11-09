//! BDF 2.1, the bitmap font format free GUI editors read (bited,
//! Bits'N'Picas, FontForge): a font written as a working copy or an
//! export, and read back or imported.

mod read;
mod write;

pub use read::{Imported, read_bdf};
pub use write::write_bdf;

/// A BDF file's extension.
pub const BDF_EXTENSION: &str = "bdf";

#[cfg(test)]
mod tests;
