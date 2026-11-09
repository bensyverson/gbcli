//! Indexed art (plan question 4, leaf k5Kadv): an image `NamedPalettes`
//! assigns a palette is drawn in four shades, as DMG art, and colored on
//! the GBC by that palette. Its shades come from the project's cascade
//! (`shades`), so its GBC tile data is its DMG tile data; what the GBC
//! adds is a palette slot in every entry of its tile map.
//!
//! A background's color index is its shade (BGP $E4), so its one palette
//! is the named palette as listed, in BG slot 0 (a background is color or
//! indexed art, never both, so nothing else shares its slots).
//!
//! A sheet's color indices are not its shades: index 0 is transparent and
//! 1-3 are the shades the sheet uses, lightest first, which its OBP maps
//! back (`sheet::check`). The GBC has no OBP, so index `i` draws entry `i`
//! of the slot; the slot holds the named palette reordered by the sheet's
//! OBP, entry `i` the color of the shade OBP gives index `i` (Ben's ruling,
//! plan question 4). So an OBJ slot is a palette and a shade mapping:
//! sheets share one only when both agree. The slots come before the packed
//! palettes of color sheets (`pack`'s `reserved`), in the order of
//! `NamedPalettes::palettes`, then each palette's mappings by OBP value,
//! so a program swapping palettes finds them where they were.

use gbcli_protocol::{
    GbcImage, GbcImagesMade, GbcPalette, ImageArt, ImageAsset, ImageKind, ImageLimit, ImageProblem,
    NamedPalette, NamedPalettes, PaletteSlot, Severity, TileEntry,
};

use super::census::error;
use super::shades::Shading;
use super::shared::{and, rule};
use super::sheet::SheetPlan;

/// A GBC build's indexed art: the project's named palettes and the OBJ
/// slots its sheets take. The build fills each indexed image's result from
/// it, and the shared OBJ palettes.
pub(crate) struct Indexed<'a> {
    named: &'a NamedPalettes,
    slots: ObjSlots,
}

impl<'a> Indexed<'a> {
    /// `sheets` are the sheets' paths under `assets/`, each with its OBP.
    pub fn new(named: &'a NamedPalettes, sheets: &[(&str, u8)]) -> Indexed<'a> {
        Indexed {
            named,
            slots: ObjSlots::new(named, sheets),
        }
    }

    /// The OBJ slots indexed sheets take (`ObjSlots::reserved`).
    pub fn reserved(&self) -> usize {
        self.slots.reserved()
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
        if let Some(slot) = self.slots.slot(&palette.name, plan.obp) {
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

/// One project-level `shades_unpinned` warning (no image) when `images`
/// (the indexed art, as shown, each with its kind) is not empty and `named`
/// has no `dmg` line (Ben's ruling, plan question 3, leaf d8FHRl). The
/// cascade then chooses the shades from every color in the project, even
/// when today's rule is known (a new color can make it stretched), so other
/// art can move them: a sheet's OBP, and with it its reordered OBJ palette;
/// a background's (or tileset's) tile data, whose index is its shade, and
/// with it the entries of its palette it draws. `shading` is today's rule,
/// and names where the palettes came from. Only a GBC build asks:
/// without --gbc indexed art is DMG art (plan question 10), its shades
/// moving as all DMG art's do, with no GBC palette to recolor.
pub(crate) fn unpinned(
    named: &NamedPalettes,
    images: &[(String, ImageKind)],
    shading: &Shading,
) -> Option<ImageProblem> {
    if images.is_empty() || named.dmg.is_some() {
        return None;
    }
    let one = images.len() == 1;
    let mut how = Vec::new();
    if images.iter().any(|(_, k)| *k == ImageKind::Sprites) {
        how.push("a sheet's OBP, and with it its OBJ palette (its palette reordered by OBP)");
    }
    if images.iter().any(|(_, k)| *k != ImageKind::Sprites) {
        how.push(
            "a background's or tileset's tile data (its index is its shade), and with it the \
             entries of its palette it draws",
        );
    }
    let names: Vec<String> = images.iter().map(|(n, _)| n.clone()).collect();
    let message = format!(
        "{} {} indexed art with {}, so {} shades are chosen from every \
         color in the project (today's rule: {}); adding or changing other art can change \
         them, and so recolor {} on the GBC: {}",
        and(&names),
        if one { "is" } else { "are" },
        shading.source.no_dmg(),
        if one { "its" } else { "their" },
        rule(shading),
        if one { "it" } else { "them" },
        how.join("; "),
    );
    Some(ImageProblem {
        image: None,
        severity: Severity::Warning,
        ..error(
            "",
            ImageLimit::ShadesUnpinned,
            images.len(),
            0,
            message,
            &format!(
                "{} to pin the shades: the colors of shades 0-3, each color taking its \
                 nearest entry's shade (gbcli help images)",
                shading.source.add_dmg()
            ),
        )
    })
}

/// The palette `named` assigns the image at `path` (under `assets/`), if
/// any: an assigned image is indexed art.
pub(crate) fn assigned<'a>(named: &'a NamedPalettes, path: &str) -> Option<&'a NamedPalette> {
    let name = named.assignments.get(path)?;
    named.get(name).map(|(_, p)| p)
}

/// The OBJ slots indexed sheets take: one per palette and OBP that some
/// sheet is assigned and drawn with.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct ObjSlots {
    /// In slot order: each slot's palette and the OBP that reorders it.
    slots: Vec<(NamedPalette, u8)>,
}

impl ObjSlots {
    /// The slots `sheets` (paths under `assets/`, each with its OBP) take,
    /// each (palette, OBP) once: palettes in the order `named` lists them,
    /// each palette's OBPs from the lowest value.
    pub fn new(named: &NamedPalettes, sheets: &[(&str, u8)]) -> ObjSlots {
        let mut slots = Vec::new();
        for palette in &named.palettes {
            let mut obps: Vec<u8> = sheets
                .iter()
                .filter(|(s, _)| named.assignments.get(*s) == Some(&palette.name))
                .map(|&(_, obp)| obp)
                .collect();
            obps.sort_unstable();
            obps.dedup();
            slots.extend(obps.into_iter().map(|obp| (palette.clone(), obp)));
        }
        ObjSlots { slots }
    }

    /// The slots taken: `pack`'s `reserved` for color sheets.
    pub fn reserved(&self) -> usize {
        self.slots.len()
    }

    /// The slot of the palette called `name` reordered by `obp`; none if
    /// no sheet takes it, or it is past the eighth (`problem`).
    pub fn slot(&self, name: &str, obp: u8) -> Option<PaletteSlot> {
        let i = self
            .slots
            .iter()
            .position(|(p, o)| p.name == name && *o == obp)?;
        PaletteSlot::new(u8::try_from(i).ok()?)
    }

    /// The colors, in slot order: the start of `GbcImagesMade::obj_palettes`.
    pub fn colors(&self) -> Vec<GbcPalette> {
        self.slots
            .iter()
            .map(|(p, obp)| by_obp(p.colors, *obp))
            .collect()
    }

    /// `gbc_palettes` when the assigned slots alone pass the eight.
    pub fn problem(&self, shown: &str) -> Option<ImageProblem> {
        let n = self.slots.len();
        if n <= PaletteSlot::COUNT {
            return None;
        }
        let names: Vec<String> = self
            .slots
            .iter()
            .map(|(p, obp)| format!("{} by OBP %{obp:08b}", p.name))
            .collect();
        Some(ImageProblem {
            image: None,
            ..error(
                shown,
                ImageLimit::GbcPalettes,
                n,
                PaletteSlot::COUNT,
                format!(
                    "the sprite sheets take {n} OBJ palettes ({}): one for each assigned \
                     palette and set of shades a sheet is drawn in; the GBC has {}, shared by \
                     every sheet",
                    names.join(", "),
                    PaletteSlot::COUNT
                ),
                "assign fewer distinct palettes to sheets, or draw the sheets that share a \
                 palette in the same shades: those share its slot, and a program can load \
                 another palette into a slot itself",
            )
        })
    }
}

/// `colors` as a sheet drawn with `obp` sees them on the GBC: entry `i` is
/// the color of the shade OBP gives index `i`. Index 0 is never drawn; it
/// takes shade 0's color, as OBP's bits 0-1 are clear.
pub(crate) fn by_obp(GbcPalette(colors): GbcPalette, obp: u8) -> GbcPalette {
    GbcPalette([0, 1, 2, 3].map(|i| colors[usize::from(obp >> (2 * i) & 3)]))
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
    (gbc(palette, Vec::new()), plan.tile_map(|_| Some(slot)))
}

fn gbc(palette: &NamedPalette, palettes: Vec<GbcPalette>) -> GbcImage {
    GbcImage {
        preview: String::new(),
        art: ImageArt::Indexed {
            palette: palette.name.clone(),
        },
        palettes,
    }
}

#[cfg(test)]
mod tests;
