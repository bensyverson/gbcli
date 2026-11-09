//! The GBC half of the generated files (the GBC images plan, leaf
//! zcxvjc), from the build's results: per image its own section with its
//! GBC tiles (color art only: indexed art's tiles are its DMG tiles), and
//! for a background its attribute map and BG palettes; then the OBJ
//! palettes every sheet shares, under `--gbc`; then every named palette of
//! `assets/palettes.txt`, with or without `--gbc` (plan question 10), so a
//! program swapping palettes finds them all.

use std::fmt::Write;

use gbcli_protocol::{GbcPalette, ImageArt, ImageAsset, ImageKind, NamedPalette};

use super::super::indexed::by_obp;
use super::{Entry, Shared};

/// An image's GBC version, as the generated files name it.
pub(crate) struct Gbc<'a> {
    /// Whether it has GBC tile data of its own (color art), in the DMG
    /// data's order; indexed art draws its DMG tiles on both machines.
    pub tiles: bool,
    /// A background's or tileset's BG palettes, in slot order.
    pub palettes: &'a [GbcPalette],
    /// A sheet's OBJ slot of each kept unit, for its objects' attributes;
    /// a tileset's BG slot of each tile, for `<Name>Attrs`.
    pub slots: Vec<u8>,
}

impl<'a> Gbc<'a> {
    /// The GBC version of `asset`, from its result: its own tiles for color
    /// art, its palettes, and the slot of each of a sheet's `units` kept
    /// units or a tileset's `units` tiles, from its tile map. `None`
    /// without a GBC version.
    pub fn of(asset: &'a ImageAsset, units: usize) -> Option<Gbc<'a>> {
        let g = asset.gbc.as_ref()?;
        let mut slots = vec![0; units];
        let unit = match asset.kind {
            ImageKind::Sprites => Some(2),
            ImageKind::Tiles => Some(1),
            ImageKind::Bg => None,
        };
        if let Some(per) = unit {
            for e in asset.tile_map.iter().flatten() {
                if let Some(s) = e.palette {
                    slots[(e.index / per) as usize] = s.get();
                }
            }
        }
        Some(Gbc {
            tiles: g.art == ImageArt::Color,
            palettes: &g.palettes,
            slots,
        })
    }
}

/// A constant of `images.inc`: its name, value and comment.
pub(super) struct Constant {
    pub name: String,
    pub value: String,
    pub comment: String,
}

/// One palette as `dw` lines take it: four RGB555 words, color 0 first.
pub(super) fn dw(GbcPalette(colors): &GbcPalette) -> String {
    let words: Vec<String> = colors
        .iter()
        .map(|c| format!("${:04X}", c.word()))
        .collect();
    format!("\tdw {}\n", words.join(", "))
}

/// A named palette's label and constant stems: `forest_2` is
/// `PaletteForest2` and `PALETTE_FOREST_2`.
pub(super) fn palette_names(name: &str) -> (String, String) {
    let label: String = name
        .split('_')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            let first = c.next().map(|f| f.to_ascii_uppercase());
            first.into_iter().chain(c).collect::<String>()
        })
        .collect();
    (
        format!("Palette{label}"),
        format!("PALETTE_{}", name.to_ascii_uppercase()),
    )
}

/// The label of the named palette `name` reordered for the sheet labeled
/// `sheet`: `hero` and `ice` are `HeroPaletteIce`.
pub(super) fn swap_label(sheet: &str, name: &str) -> String {
    format!("{sheet}{}", palette_names(name).0)
}

/// An indexed sheet's GBC section: each of the `named` palettes reordered
/// by its OBP, since the GBC has no OBP and its tiles hold OBP's indices,
/// not shades (`indexed::by_obp`, as its own slot is).
fn swaps(
    out: &mut String,
    source: &str,
    label: &str,
    constant: &str,
    obp: u8,
    named: &[NamedPalette],
) {
    let _ = write!(
        out,
        "\nSECTION \"image {label} GBC\", ROMX\n\n\
         ; {source} on the GBC: its tiles are {label}Tiles (indexed art), and its OBJ\n\
         ; palette, in ObjPalettesGbc, is its assigned palette reordered by {constant}_OBP.\n\
         ; Each palette of assets/palettes.txt, reordered alike, to load into that slot\n\
         ; instead (RGB555, 8 bytes each).\n"
    );
    for p in named {
        let swap = swap_label(label, &p.name);
        let _ = write!(out, "{swap}::\n{}{swap}End::\n", dw(&by_obp(p.colors, obp)));
    }
}

/// The image's GBC section, if it has a GBC version; an indexed sheet's
/// holds its swap palettes, from `named`.
pub(super) fn image(out: &mut String, e: &Entry, named: &[NamedPalette]) {
    match e {
        Entry::Bg {
            source,
            label,
            data,
            gbc: Some(g),
            ..
        } => {
            let _ = write!(
                out,
                "\nSECTION \"image {label} GBC\", ROMX\n\n\
                 ; {source} on the GBC: "
            );
            if g.tiles {
                let _ = write!(
                    out,
                    "its tiles (as many, in {label}Tiles' order), the attribute map\n\
                     ; (a byte per map entry, its BG palette in bits 0-2; write it with VBK=1),\n\
                     ; and its BG palettes (RGB555, 8 bytes each, slot 0 first).\n\
                     {label}TilesGbc::\n\tINCBIN \"{data}.gbc.2bpp\"\n{label}TilesGbcEnd::\n"
                );
            } else {
                let _ = write!(
                    out,
                    "its tiles are {label}Tiles (indexed art). The attribute map\n\
                     ; (a byte per map entry, its BG palette in bits 0-2; write it with VBK=1),\n\
                     ; and its BG palette (RGB555, 8 bytes).\n"
                );
            }
            let _ = write!(
                out,
                "{label}Attrmap::\n\tINCBIN \"{data}.attrmap\"\n{label}AttrmapEnd::\n\
                 {label}Palettes::\n"
            );
            for p in g.palettes {
                out.push_str(&dw(p));
            }
            let _ = writeln!(out, "{label}PalettesEnd::");
        }
        Entry::Sheet {
            source,
            label,
            data,
            plan,
            gbc: Some(g),
            ..
        } if g.tiles => {
            let _ = write!(
                out,
                "\nSECTION \"image {label} GBC\", ROMX\n\n\
                 ; {source} on the GBC: its {} objects, in {label}Tiles' order; its\n\
                 ; palettes are ObjPalettesGbc, and each object's slot is in its frame list.\n\
                 {label}TilesGbc::\n",
                plan.object.name()
            );
            if plan.kept.is_empty() {
                out.push_str("\t; (no object: the sheet is transparent)\n");
            } else {
                let _ = writeln!(out, "\tINCBIN \"{data}.gbc.2bpp\"");
            }
            let _ = writeln!(out, "{label}TilesGbcEnd::");
        }
        Entry::Sheet {
            source,
            label,
            constant,
            plan,
            gbc: Some(_),
            ..
        } => swaps(out, source, label, constant, plan.obp, named),
        Entry::Tiles {
            source,
            label,
            data,
            gbc: Some(g),
            ..
        } => {
            let _ = write!(
                out,
                "\nSECTION \"image {label} GBC\", ROMX\n\n\
                 ; {source} on the GBC: "
            );
            if g.tiles {
                let _ = write!(
                    out,
                    "its tiles (as many, in {label}Tiles' order), then a byte\n\
                     ; per tile, its BG palette in bits 0-2 (what a map entry using the tile\n\
                     ; writes with VBK=1), and its BG palettes (RGB555, 8 bytes each, slot 0\n\
                     ; first).\n\
                     {label}TilesGbc::\n\tINCBIN \"{data}.gbc.2bpp\"\n{label}TilesGbcEnd::\n"
                );
            } else {
                let _ = write!(
                    out,
                    "its tiles are {label}Tiles (indexed art). A byte per tile,\n\
                     ; its BG palette in bits 0-2 (what a map entry using the tile writes with\n\
                     ; VBK=1), and its BG palette (RGB555, 8 bytes).\n"
                );
            }
            let _ = writeln!(out, "{label}Attrs::");
            for chunk in g.slots.chunks(16) {
                let words: Vec<String> = chunk.iter().map(u8::to_string).collect();
                let _ = writeln!(out, "\tdb {}", words.join(", "));
            }
            let _ = write!(out, "{label}AttrsEnd::\n{label}Palettes::\n");
            for p in g.palettes {
                out.push_str(&dw(p));
            }
            let _ = writeln!(out, "{label}PalettesEnd::");
        }
        _ => {}
    }
}

/// The OBJ palettes (under `--gbc`) and the named palettes.
pub(super) fn shared(out: &mut String, s: &Shared) {
    if let Some(made) = s.gbc {
        out.push_str(
            "\nSECTION \"images GBC OBJ palettes\", ROMX\n\n\
                ; The OBJ palettes every sheet shares, slot 0 first (RGB555, 8 bytes each):\n\
                ; the assigned palettes of indexed sheets, then the packed ones. Color 0 is\n\
                ; never drawn.\nObjPalettesGbc::\n",
        );
        for p in &made.obj_palettes {
            out.push_str(&dw(p));
        }
        out.push_str("ObjPalettesGbcEnd::\n");
    }
    if s.named.is_empty() {
        return;
    }
    out.push_str(
        "\nSECTION \"images named palettes\", ROMX\n\n\
            ; Every palette of assets/palettes.txt, used by an image or not, as written\n\
            ; (RGB555, color 0 first), for a program to load into a slot itself.\n",
    );
    for p in s.named {
        let (label, _) = palette_names(&p.name);
        let _ = write!(out, "{label}::\n{}{label}End::\n", dw(&p.colors));
    }
}

/// The image's GBC constants.
pub(super) fn image_constants(e: &Entry) -> Vec<Constant> {
    match e {
        Entry::Bg {
            label,
            constant,
            gbc: Some(g),
            ..
        }
        | Entry::Tiles {
            label,
            constant,
            gbc: Some(g),
            ..
        } => vec![Constant {
            name: format!("{constant}_PALETTES"),
            value: g.palettes.len().to_string(),
            comment: format!("BG palettes at {label}Palettes, from slot 0"),
        }],
        _ => Vec::new(),
    }
}

/// The constants every image shares: the OBJ palettes' count under
/// `--gbc`, and each named palette's index. `GBCLI_GBC` isn't one: the
/// build defines it for every source (`rgbasm -D`), images or not.
pub(super) fn shared_constants(s: &Shared) -> Vec<Constant> {
    let mut all = Vec::new();
    if let Some(made) = s.gbc {
        all.push(Constant {
            name: "OBJ_PALETTES".into(),
            value: made.obj_palettes.len().to_string(),
            comment: "OBJ palettes at ObjPalettesGbc, from slot 0".into(),
        });
    }
    for (i, p) in s.named.iter().enumerate() {
        let (label, constant) = palette_names(&p.name);
        all.push(Constant {
            name: constant,
            value: i.to_string(),
            comment: format!(
                "{}, 8 bytes at {label}; its place in assets/palettes.txt",
                p.name
            ),
        });
    }
    all
}
