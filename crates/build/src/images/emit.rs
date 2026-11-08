//! What the program sees of its images: `images.asm`, assembled with the
//! sources, which INCBINs each image's data under labels named after its
//! file, and `images.inc`, on the include path, with each image's sizes,
//! counts and palette as constants. Both are rewritten by every build.
//! A GBC build adds each image's GBC data and the shared OBJ palettes
//! (`gbc`); every build adds the named palettes. `images.asm` exports
//! every constant too, so the `.sym` names them.

mod gbc;

use std::fmt::Write;

use gbcli_protocol::{GbcImagesMade, ImageKind, NamedPalette};

use super::sheet::{Object, SheetPlan};
pub(crate) use gbc::Gbc;

/// What the images share in the generated files.
pub(crate) struct Shared<'a> {
    /// The GBC version's, under `--gbc`.
    pub gbc: Option<&'a GbcImagesMade>,
    /// `assets/palettes.txt`'s palettes, in file order.
    pub named: &'a [NamedPalette],
}

/// One converted image, as the generated files name it.
pub(crate) enum Entry<'a> {
    Bg {
        source: &'a str,
        label: &'a str,
        constant: &'a str,
        /// The data's path without extension, relative to the project.
        data: &'a str,
        width: u32,
        height: u32,
        tiles: u32,
        gbc: Option<Gbc<'a>>,
    },
    Sheet {
        source: &'a str,
        label: &'a str,
        constant: &'a str,
        data: &'a str,
        plan: &'a SheetPlan,
        gbc: Option<Gbc<'a>>,
    },
    /// A tileset: its unique tiles, and no map.
    Tiles {
        source: &'a str,
        label: &'a str,
        constant: &'a str,
        data: &'a str,
        tiles: u32,
        gbc: Option<Gbc<'a>>,
    },
}

/// Every symbol an image of `kind` may define, from its label and constant
/// stems, its GBC ones included (reserved with or without `--gbc`, so a
/// project doesn't stop building when it adds the flag), but a sheet's
/// `<Label>Frame<n>`: those end in the frame's number after `Frame`, which
/// no other symbol here does, so they can't collide unless the labels do.
pub(crate) fn symbols(kind: ImageKind, label: &str, constant: &str) -> Vec<String> {
    let labels: &[&str] = match kind {
        ImageKind::Bg => &[
            "Tiles",
            "TilesEnd",
            "Tilemap",
            "TilemapEnd",
            "TilesGbc",
            "TilesGbcEnd",
            "Attrmap",
            "AttrmapEnd",
            "Palettes",
            "PalettesEnd",
        ],
        ImageKind::Sprites => &["Tiles", "TilesEnd", "Frames", "TilesGbc", "TilesGbcEnd"],
        ImageKind::Tiles => &[
            "Tiles",
            "TilesEnd",
            "TilesGbc",
            "TilesGbcEnd",
            "Attrs",
            "AttrsEnd",
            "Palettes",
            "PalettesEnd",
        ],
    };
    let labels = labels.iter().map(|s| format!("{label}{s}"));
    labels.chain(constants(kind, constant, true)).collect()
}

/// Every symbol the named palette `name` defines, on either target:
/// `Palette<Name>`, `Palette<Name>End` and `PALETTE_<NAME>`.
pub(crate) fn palette_symbols(name: &str) -> Vec<String> {
    let (label, constant) = gbc::palette_names(name);
    let end = format!("{label}End");
    vec![label, end, constant]
}

/// Every symbol a sheet labeled `label` defines for the named palette
/// `name`: `<Label>Palette<Name>` and its `End`. Reserved for every sheet
/// on either target, as `symbols` reserves the GBC ones, though only an
/// indexed sheet under `--gbc` defines them: an assignment can't bring a
/// collision in either.
pub(crate) fn swap_symbols(label: &str, name: &str) -> Vec<String> {
    let swap = gbc::swap_label(label, name);
    let end = format!("{swap}End");
    vec![swap, end]
}

/// The constants an image of `kind` defines, its GBC ones when `gbc`.
fn constants(kind: ImageKind, constant: &str, gbc: bool) -> Vec<String> {
    let (dmg, more): (&[&str], &[&str]) = match kind {
        ImageKind::Bg => (&["WIDTH", "HEIGHT", "TILES", "BGP"], &["PALETTES"]),
        ImageKind::Sprites => (
            &["TILES", "FRAMES", "FRAME_WIDTH", "FRAME_HEIGHT", "OBP"],
            &[],
        ),
        ImageKind::Tiles => (&["TILES"], &["PALETTES"]),
    };
    let more = if gbc { more } else { &[] };
    dmg.iter()
        .chain(more)
        .map(|s| format!("{constant}_{s}"))
        .collect()
}

/// Every constant `images.inc` defines but its guard, for `EXPORT`.
fn exported(entries: &[Entry], shared: &Shared) -> Vec<String> {
    let mut all: Vec<String> = entries
        .iter()
        .flat_map(|e| match e {
            Entry::Bg { constant, gbc, .. } => constants(ImageKind::Bg, constant, gbc.is_some()),
            Entry::Sheet { constant, gbc, .. } => {
                constants(ImageKind::Sprites, constant, gbc.is_some())
            }
            Entry::Tiles { constant, gbc, .. } => {
                constants(ImageKind::Tiles, constant, gbc.is_some())
            }
        })
        .collect();
    all.extend(gbc::shared_constants(shared).into_iter().map(|c| c.name));
    all
}

const HEADER: &str = "; Generated by `gbcli build` from the images under assets/; every build \
rewrites it.\n; Edit the PNGs, not this file. Pan Docs: https://gbdev.io/pandocs/Graphics.html\n";

/// The assembly: one ROMX section per image, another for its GBC data,
/// and the shared palettes.
pub(crate) fn asm(entries: &[Entry], shared: &Shared) -> String {
    let mut out = String::from(HEADER);
    out += "; Constants: images.inc. Object attributes: $20 flips X, $40 flips Y, and on\n\
            ; the GBC bits 0-2 are the OBJ palette (the DMG ignores them; OBP0 is bit 4).\n";
    // Exported here, once, so the .sym names every constant.
    out += "\nINCLUDE \"images.inc\"\n";
    for names in exported(entries, shared).chunks(4) {
        let _ = writeln!(out, "EXPORT {}", names.join(", "));
    }
    for e in entries {
        match e {
            Entry::Bg {
                source,
                label,
                data,
                ..
            } => {
                let _ = write!(
                    out,
                    "\nSECTION \"image {label}\", ROMX\n\n\
                     ; {source}: unique tiles, then the map, a byte per tile, row by row.\n\
                     {label}Tiles::\n\tINCBIN \"{data}.2bpp\"\n{label}TilesEnd::\n\
                     {label}Tilemap::\n\tINCBIN \"{data}.tilemap\"\n{label}TilemapEnd::\n"
                );
            }
            Entry::Sheet {
                source,
                label,
                data,
                plan,
                gbc,
                ..
            } => {
                let _ = write!(
                    out,
                    "\nSECTION \"image {label}\", ROMX\n\n\
                     ; {source}: unique 8x16 objects, 32 bytes each (top tile, bottom tile).\n\
                     {label}Tiles::\n"
                );
                if plan.kept.is_empty() {
                    out += "\t; (no object: the sheet is transparent)\n";
                } else {
                    let _ = writeln!(out, "\tINCBIN \"{data}.2bpp\"");
                }
                let _ = write!(
                    out,
                    "{label}TilesEnd::\n\n\
                     ; One pointer per frame, left to right, then top to bottom. A frame is a\n\
                     ; count, then per object: y, x (pixels from the frame's top-left), tile\n\
                     ; (from {label}Tiles' first) and attributes.\n\
                     {label}Frames::\n"
                );
                let names: Vec<String> = (0..plan.frames.len())
                    .map(|i| format!("{label}Frame{i}"))
                    .collect();
                for chunk in names.chunks(8) {
                    let _ = writeln!(out, "\tdw {}", chunk.join(", "));
                }
                // Each object's GBC palette rides beside its flip.
                let slot = |unit: usize| gbc.as_ref().map_or(0, |g| g.slots[unit]);
                for (name, objects) in names.iter().zip(&plan.frames) {
                    let _ = write!(out, "{name}::\n\tdb {}\n", objects.len());
                    for o in objects {
                        out += &object(o, slot(o.unit));
                    }
                }
            }
            Entry::Tiles {
                source,
                label,
                data,
                ..
            } => {
                let _ = write!(
                    out,
                    "\nSECTION \"image {label}\", ROMX\n\n\
                     ; {source}: unique tiles, in order of first use; no map (the program\n\
                     ; builds its own, from the tile map gbcli reports).\n\
                     {label}Tiles::\n\tINCBIN \"{data}.2bpp\"\n{label}TilesEnd::\n"
                );
            }
        }
        gbc::image(&mut out, e, shared.named);
    }
    gbc::shared(&mut out, shared);
    out
}

/// An object's four bytes; `slot` is its GBC OBJ palette.
fn object(o: &Object, slot: u8) -> String {
    let attr = match o.flip.oam_bits() | slot {
        0 => "0".to_string(),
        a => format!("${a:02X}"),
    };
    format!("\tdb {}, {}, {}, {attr}\n", o.y, o.x, o.unit * 2)
}

/// The include: constants only, so any source may include it.
pub(crate) fn inc(entries: &[Entry], shared: &Shared) -> String {
    let mut out = String::from(HEADER);
    out += "; Labels (in images.asm, exported): <Name>Tiles/<Name>TilesEnd, and\n\
            ; <Name>Tilemap/<Name>TilemapEnd for a background or <Name>Frames for a sheet\n\
            ; (a tileset has no map); Palette<Name>/Palette<Name>End for each palette of\n\
            ; assets/palettes.txt. With --gbc: <Name>TilesGbc/End (color art; indexed\n\
            ; art uses <Name>Tiles), <Name>Attrmap/End (a background) or <Name>Attrs/End\n\
            ; (a tileset: a byte per tile), <Name>Palettes/End for both,\n\
            ; ObjPalettesGbc/ObjPalettesGbcEnd for every sheet, and for an indexed sheet\n\
            ; <Name>Palette<Palette>/End: each palette reordered by its OBP, to load into\n\
            ; its slot. GBCLI_GBC is not defined here: gbcli's --gbc build defines it for\n\
            ; every source (rgbasm -D GBCLI_GBC=1), so anything else assembling these\n\
            ; files defines it itself.\n\
            \nIF !DEF(GBCLI_IMAGES_INC)\nDEF GBCLI_IMAGES_INC EQU 1\n";
    for e in entries {
        match e {
            Entry::Bg {
                source,
                constant: c,
                width,
                height,
                tiles,
                ..
            } => {
                let _ = write!(
                    out,
                    "\n; {source}: a background, {width}x{height} tiles.\n\
                     DEF {c}_WIDTH EQU {width} ; tiles per map row\n\
                     DEF {c}_HEIGHT EQU {height} ; map rows\n\
                     DEF {c}_TILES EQU {tiles} ; unique tiles, 16 bytes each\n\
                     DEF {c}_BGP EQU %11_10_01_00 ; each color index is its shade\n"
                );
            }
            Entry::Sheet {
                source,
                constant: c,
                plan,
                ..
            } => {
                let (fw, fh) = plan.frame;
                let obp = plan.obp;
                let _ = write!(
                    out,
                    "\n; {source}: 8x16 objects (set LCDC_OBJ_16), {} frame(s) of {fw}x{fh}.\n\
                     DEF {c}_TILES EQU {} ; tiles, two per object\n\
                     DEF {c}_FRAMES EQU {}\n\
                     DEF {c}_FRAME_WIDTH EQU {fw}\n\
                     DEF {c}_FRAME_HEIGHT EQU {fh}\n\
                     DEF {c}_OBP EQU %{:02b}_{:02b}_{:02b}_{:02b} ; shades of indices 3, 2, 1 \
                     (0 is transparent)\n",
                    plan.frames.len(),
                    plan.tiles(),
                    plan.frames.len(),
                    obp >> 6,
                    (obp >> 4) & 3,
                    (obp >> 2) & 3,
                    obp & 3,
                );
            }
            Entry::Tiles {
                source,
                constant: c,
                tiles,
                ..
            } => {
                let _ = write!(
                    out,
                    "\n; {source}: a tileset, no map.\n\
                     DEF {c}_TILES EQU {tiles} ; unique tiles, 16 bytes each\n"
                );
            }
        }
        for c in gbc::image_constants(e) {
            let _ = writeln!(out, "DEF {} EQU {} ; {}", c.name, c.value, c.comment);
        }
    }
    let shared = gbc::shared_constants(shared);
    if !shared.is_empty() {
        out += "\n";
    }
    for c in shared {
        let _ = writeln!(out, "DEF {} EQU {} ; {}", c.name, c.value, c.comment);
    }
    out += "\nENDC\n";
    out
}

#[cfg(test)]
mod tests;
