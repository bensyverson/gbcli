//! Indexed art (plan question 4, leaf k5Kadv): an image `NamedPalettes`
//! assigns a palette is drawn in four shades, as DMG art, and colored on
//! the GBC by that palette. Its shades come from the project's cascade
//! (`shades`), so its GBC tile data is its DMG tile data; what the GBC
//! adds is the palette's slot in every entry of its tile map.
//!
//! A background's one palette is BG slot 0 (a background is color or
//! indexed art, never both, so nothing else shares its slots). Sheets
//! share the 8 OBJ palettes: each distinct palette assigned to a sheet
//! takes one slot, in the order of `NamedPalettes::palettes`, before the
//! packed palettes of color sheets (`pack`'s `reserved`), so a program
//! swapping palettes finds them where they were.

use gbcli_protocol::{
    GbcImage, GbcImagesMade, GbcPalette, ImageArt, ImageAsset, ImageLimit, ImageProblem,
    NamedPalette, NamedPalettes, PaletteSlot, TileEntry,
};

use super::census::error;
use super::sheet::SheetPlan;

/// A GBC build's indexed art: the project's named palettes and the OBJ
/// slots its sheets' palettes take. The build fills each indexed image's
/// result from it, and the shared OBJ palettes.
pub(crate) struct Indexed<'a> {
    named: &'a NamedPalettes,
    slots: ObjSlots,
}

impl<'a> Indexed<'a> {
    /// `sheets` are the sheets' paths under `assets/`.
    pub fn new(named: &'a NamedPalettes, sheets: &[&str]) -> Indexed<'a> {
        Indexed {
            named,
            slots: ObjSlots::new(named, sheets),
        }
    }

    /// `ObjSlots::problem`.
    pub fn problem(&self, shown: &str) -> Option<ImageProblem> {
        self.slots.problem(shown)
    }

    /// When the background at `path` is indexed, its GBC version and tile
    /// map from its tilemap `map`.
    pub fn fill_background(&self, path: &str, map: &[u8], asset: &mut ImageAsset) {
        if let Some(palette) = assigned(self.named, path) {
            let (gbc, entries) = background(palette, map);
            (asset.gbc, asset.tile_map) = (Some(gbc), entries);
        }
    }

    /// When the sheet at `path` is indexed, its GBC version and tile map.
    pub fn fill_sheet(&self, path: &str, plan: &SheetPlan, asset: &mut ImageAsset) {
        let Some(palette) = assigned(self.named, path) else {
            return;
        };
        if let Some(slot) = self.slots.slot(&palette.name) {
            let (gbc, entries) = sheet(palette, plan, slot);
            (asset.gbc, asset.tile_map) = (Some(gbc), entries);
        }
    }

    /// What the images share on the GBC: the assigned OBJ palettes.
    pub fn made(&self) -> GbcImagesMade {
        GbcImagesMade {
            obj_palettes: self.slots.colors(),
        }
    }
}

/// The palette `named` assigns the image at `path` (under `assets/`), if
/// any: an assigned image is indexed art.
pub(crate) fn assigned<'a>(named: &'a NamedPalettes, path: &str) -> Option<&'a NamedPalette> {
    let name = named.assignments.get(path)?;
    named.get(name).map(|(_, p)| p)
}

/// The OBJ slots that palettes assigned to sheets take.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct ObjSlots {
    /// In slot order.
    palettes: Vec<NamedPalette>,
}

impl ObjSlots {
    /// The palettes assigned to `sheets` (paths under `assets/`), each
    /// once, in the order `named` lists them.
    pub fn new(named: &NamedPalettes, sheets: &[&str]) -> ObjSlots {
        let used: Vec<&str> = sheets
            .iter()
            .filter_map(|s| named.assignments.get(*s).map(String::as_str))
            .collect();
        ObjSlots {
            palettes: named
                .palettes
                .iter()
                .filter(|p| used.contains(&p.name.as_str()))
                .cloned()
                .collect(),
        }
    }

    /// The slots taken: `pack`'s `reserved` for color sheets.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the joint conversion (leaf joint) passes it to pack"
        )
    )]
    pub fn reserved(&self) -> usize {
        self.palettes.len()
    }

    /// The slot of the palette called `name`; none if no sheet is assigned
    /// it, or it is past the eighth (`problem`).
    pub fn slot(&self, name: &str) -> Option<PaletteSlot> {
        let i = self.palettes.iter().position(|p| p.name == name)?;
        PaletteSlot::new(u8::try_from(i).ok()?)
    }

    /// The colors, in slot order: the start of `GbcImagesMade::obj_palettes`.
    pub fn colors(&self) -> Vec<GbcPalette> {
        self.palettes.iter().map(|p| p.colors).collect()
    }

    /// `gbc_palettes` when the assigned palettes alone pass the eight.
    pub fn problem(&self, shown: &str) -> Option<ImageProblem> {
        let n = self.palettes.len();
        if n <= PaletteSlot::COUNT {
            return None;
        }
        let names: Vec<&str> = self.palettes.iter().map(|p| p.name.as_str()).collect();
        Some(ImageProblem {
            image: None,
            ..error(
                shown,
                ImageLimit::GbcPalettes,
                n,
                PaletteSlot::COUNT,
                format!(
                    "the sprite sheets are assigned {n} distinct palettes ({}); the GBC has {} \
                     OBJ palettes, one a palette, shared by every sheet",
                    names.join(", "),
                    PaletteSlot::COUNT
                ),
                "assign fewer distinct palettes to sheets: sheets that share a palette share \
                 its slot, and a program can load another palette into a slot itself",
            )
        })
    }
}

/// An indexed background's GBC version and tile map: its tilemap `map`
/// (rgbgfx's, one tile index a byte), every entry in BG slot 0.
pub(crate) fn background(palette: &NamedPalette, map: &[u8]) -> (GbcImage, Vec<Option<TileEntry>>) {
    let slot = PaletteSlot::new(0);
    let entries = map
        .iter()
        .map(|&t| {
            Some(TileEntry {
                index: u32::from(t),
                flip: Default::default(),
                palette: slot,
            })
        })
        .collect();
    (gbc(palette, vec![palette.colors]), entries)
}

/// An indexed sheet's GBC version and tile map: its units and flips, every
/// opaque unit in `slot`; its palette is in the shared OBJ palettes.
pub(crate) fn sheet(
    palette: &NamedPalette,
    plan: &SheetPlan,
    slot: PaletteSlot,
) -> (GbcImage, Vec<Option<TileEntry>>) {
    (gbc(palette, Vec::new()), plan.tile_map(Some(slot)))
}

fn gbc(palette: &NamedPalette, palettes: Vec<GbcPalette>) -> GbcImage {
    GbcImage {
        art: ImageArt::Indexed {
            palette: palette.name.clone(),
        },
        palettes,
    }
}

#[cfg(test)]
mod tests;
