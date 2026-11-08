//! What `build` reports about the images it converted (design §12, Build,
//! the 2026-11-07 image update): each PNG under `assets/bg/` and
//! `assets/sprites/`, what it became, and every hardware limit it breaks.
//! `gbcli-build` makes them and the CLI prints them. A GBC build adds each
//! image's GBC version (`GbcImage`) and what the images share
//! (`GbcImagesMade`); these are the seam a client converting assets
//! without a build (LoadHigh) reads, so `images.inc` is generated from
//! them and the two can't disagree.

use serde::{Deserialize, Serialize};

use crate::toolchain::Severity;

mod asset;
mod gbc;
mod map;

pub use gbc::{GbcImage, GbcImagesMade, ImageArt};
pub use map::{Flip, NotPaletteSlot, PaletteSlot, TileEntry};

/// Which convention an image follows, from its directory.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageKind {
    /// `assets/bg/*.png`: a background, as unique tiles and a tilemap.
    Bg,
    /// `assets/sprites/*.png`: a sheet of 8x16 objects, as unique 8x16 units
    /// and one object list per frame.
    Sprites,
    /// `assets/tiles/*.png`: a tileset, as unique 8x8 background tiles and
    /// no map; the program (or a client such as LoadHigh) builds its own
    /// maps from [`ImageAsset::tile_map`].
    Tiles,
}

impl ImageKind {
    /// The directory under `assets/`, and the word on the wire.
    pub fn dir(self) -> &'static str {
        match self {
            ImageKind::Bg => "bg",
            ImageKind::Sprites => "sprites",
            ImageKind::Tiles => "tiles",
        }
    }
}

/// A hardware limit an image can break (Pan Docs); or `alpha`, which would
/// make the conversion guess, `frame_grid`, a sheet its frames don't
/// tile, `rgbgfx_agrees`, gbcli's own check on rgbgfx, or `shared_shade`,
/// colors drawn alike: the checks that aren't limits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageLimit {
    /// 256 tiles addressable by one background map.
    BgTilesMap,
    /// 384 tiles of VRAM, shared by the background and objects.
    VramTiles,
    /// 256 tiles addressable by objects ($8000-$8FFF).
    ObjTiles,
    /// DMG: one 4-shade palette for the whole background; 3 shades plus
    /// transparent for an object palette.
    DmgPalette,
    /// Width and height a multiple of 8.
    #[serde(rename = "size_multiple_8")]
    SizeMultiple8,
    /// 8x16 objects pair tiles vertically: a sheet's height is a multiple
    /// of 16.
    #[serde(rename = "obj_pair_8x16")]
    ObjPair8x16,
    /// Color 0 of an object never draws; a sheet with no transparent pixel
    /// draws every object as a solid block. A warning.
    ObjNoTransparency,
    /// 10 objects per line.
    ObjPerLine,
    /// 40 objects in OAM.
    ObjTotal,
    /// A sheet's frames tile it: its width and height are whole frames.
    FrameGrid,
    /// OAM positions are bytes, so a frame spans at most 256 pixels each
    /// way (and its objects' offsets are bytes in the frame lists).
    FrameSpan,
    /// One image's section (its data, and a sheet's frame lists) fits one
    /// 16 KiB ROMX bank.
    RomxBank,
    /// A pixel neither opaque nor fully transparent, or a transparent pixel
    /// in a background, which has none.
    Alpha,
    /// rgbgfx's tile data draws the color indices gbcli planned (and
    /// checked) for every tile. A disagreement is gbcli's bug, not the
    /// image's; the build stops rather than ship unchecked shades.
    RgbgfxAgrees,
    /// Two or more colors take one DMG shade, by the project's shade rule
    /// (`ShadeChoice`), and draw alike. A warning: BGP and OBP map indices
    /// to shades, and the hardware is content to show two colors as one.
    /// With `image`, the colors are in that image; without, they share the
    /// shade only across images, and `message` names the images each is in.
    SharedShade,
    /// `assets/palettes.txt` doesn't read: a line that is neither a
    /// palette nor an assignment, a palette without four colors, a
    /// duplicate name or assignment, an unknown palette, an image path not
    /// spelled as gbcli finds it, an image the project doesn't have, or
    /// `dmg` assigned to an image (the `NamedPalettes` rules, and the
    /// file's). `line` names the line when one is at fault. A warning when
    /// the project still has the old `assets/palette.txt`, which isn't read.
    PaletteFile,
    /// The stretched shade rule put a color two or more shades from its
    /// plain nearest shade. A warning: a `dmg` line in `assets/palettes.txt` picks
    /// the shades instead.
    ShadeStretched,
    /// GBC: 8 palettes. Per background, its packed palettes (or its one
    /// assigned palette); across all sheets, the packed OBJ palettes plus
    /// one per distinct assigned palette. `count` is the lower bound on the
    /// palettes needed (⌈colors / 4⌉, ⌈colors / 3⌉ for sheets) or the
    /// palettes placed, whichever is larger, plus the slots assigned
    /// palettes take; `tiles` are the tiles that landed in palette 9 or
    /// later, one problem per image holding any.
    GbcPalettes,
    /// GBC: 4 colors in one background tile, or 3 opaque colors (plus
    /// transparent) in one object; `tiles` names the tiles at fault.
    GbcTileColors,
    /// `name.dmg.png`, the DMG version that overrides the one derived from
    /// a color image, isn't the color image's size; `count` and `max` are
    /// its width and the color image's when they differ, else the heights.
    DmgOverrideSize,
}

impl ImageLimit {
    /// The name on the wire.
    pub fn name(self) -> &'static str {
        match self {
            ImageLimit::BgTilesMap => "bg_tiles_map",
            ImageLimit::VramTiles => "vram_tiles",
            ImageLimit::ObjTiles => "obj_tiles",
            ImageLimit::DmgPalette => "dmg_palette",
            ImageLimit::SizeMultiple8 => "size_multiple_8",
            ImageLimit::ObjPair8x16 => "obj_pair_8x16",
            ImageLimit::ObjNoTransparency => "obj_no_transparency",
            ImageLimit::ObjPerLine => "obj_per_line",
            ImageLimit::ObjTotal => "obj_total",
            ImageLimit::FrameGrid => "frame_grid",
            ImageLimit::FrameSpan => "frame_span",
            ImageLimit::RomxBank => "romx_bank",
            ImageLimit::Alpha => "alpha",
            ImageLimit::RgbgfxAgrees => "rgbgfx_agrees",
            ImageLimit::SharedShade => "shared_shade",
            ImageLimit::PaletteFile => "palette_file",
            ImageLimit::ShadeStretched => "shade_stretched",
            ImageLimit::GbcPalettes => "gbc_palettes",
            ImageLimit::GbcTileColors => "gbc_tile_colors",
            ImageLimit::DmgOverrideSize => "dmg_override_size",
        }
    }
}

/// A tile by its column and row in the source image (pixels / 8).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TileAt {
    pub x: u32,
    pub y: u32,
}

/// One limit an image (or the images together) breaks: what, by how much,
/// where, and what to do.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageProblem {
    /// The source PNG; none when the images break a limit together
    /// (`vram_tiles`, `obj_tiles`) or share a shade across images
    /// (`shared_shade`), which `message` then itemizes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    pub limit: ImageLimit,
    pub severity: Severity,
    /// What the image needs, against `max`, what the hardware allows.
    pub count: u32,
    pub max: u32,
    /// The tiles to look at first, in the source image's tile grid.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tiles: Vec<TileAt>,
    /// The line at fault, 1-based, when the problem is in a text file
    /// (`palette_file`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    pub message: String,
    pub remedy: String,
}

/// Which rule gave the project's colors their DMG shades (Ben's rulings,
/// 2026-11-07): the first that applies, once for the whole project.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShadeRule {
    /// The `dmg` line of `assets/palettes.txt` lists shades 0-3; each color
    /// takes its nearest entry's shade, in RGB555.
    Declared,
    /// Every color belongs to one known palette, which maps them exactly.
    Known,
    /// Lightness rescaled over the project's colors (the lightest shade 0,
    /// the darkest shade 3), then the nearest shade.
    Stretched,
    /// The project has one lightness, so nothing to stretch: the plain
    /// nearest shade.
    Nearest,
}

impl ShadeRule {
    /// The name on the wire.
    pub fn name(self) -> &'static str {
        match self {
            ShadeRule::Declared => "declared",
            ShadeRule::Known => "known",
            ShadeRule::Stretched => "stretched",
            ShadeRule::Nearest => "nearest",
        }
    }
}

/// A palette gbcli knows, shade 0 first, matched in RGB555.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnownPalette {
    /// #ffffff #aaaaaa #555555 #000000.
    DmgGrays,
    /// The DMG-01's screen: #9bbc0f #8bac0f #306230 #0f380f.
    PeaGreens,
    /// GB Studio's default: #e0f8cf #86c06c #306850 #071821.
    GbStudioGreens,
    /// The Game Boy Pocket's screen, as SameBoy draws it
    /// (`GB_PALETTE_MGB`): #c2ce93 #818d66 #3a4c3a #07100e.
    PocketGrays,
}

impl KnownPalette {
    /// In the order they are tried.
    pub const ALL: [KnownPalette; 4] = [
        KnownPalette::DmgGrays,
        KnownPalette::PeaGreens,
        KnownPalette::GbStudioGreens,
        KnownPalette::PocketGrays,
    ];

    /// The name on the wire.
    pub fn name(self) -> &'static str {
        match self {
            KnownPalette::DmgGrays => "dmg_grays",
            KnownPalette::PeaGreens => "pea_greens",
            KnownPalette::GbStudioGreens => "gb_studio_greens",
            KnownPalette::PocketGrays => "pocket_grays",
        }
    }
}

/// The project's shade rule, and the known palette when it is `known`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShadeChoice {
    pub rule: ShadeRule,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub palette: Option<KnownPalette>,
}

/// One image, converted. On read, its tile map must agree with it
/// (`asset::Unchecked`): a slot in every entry exactly when it has a GBC
/// version, and a flip or a missing tile only in a sheet.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "asset::Unchecked")]
pub struct ImageAsset {
    pub kind: ImageKind,
    pub source: String,
    /// The labels' stem in the generated assembly (`Town` for `town.png`:
    /// `TownTiles`, `TownTilemap`); the constants' is its upper-case form.
    pub label: String,
    /// The image's size in tiles.
    pub width: u32,
    pub height: u32,
    /// Unique 8x8 tiles after deduplication: what it costs in VRAM.
    pub tiles: u32,
    /// Sprite sheets: frames, and objects in the largest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frames: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objects: Option<u32>,
    /// The converted data drawn as the hardware shows it, then its unique
    /// tiles by use, most used first.
    pub preview: String,
    /// Where each source tile went ([`TileEntry`]: its tile, its flip and
    /// its GBC palette slot), one entry per 8x8 tile of the image (8x16
    /// unit of a sheet), rows top to bottom, left to right in each. None
    /// for a sheet's fully transparent unit, which takes no tile. A
    /// tileset's result is this map; a background's is also its tilemap
    /// and, on the GBC, its attribute map.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tile_map: Vec<Option<TileEntry>>,
    /// The GBC version, when the build's target is the GBC. Every image has
    /// a DMG version, so this is the only per-image sign of the target.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gbc: Option<GbcImage>,
}

/// The images a build converted, and where the program finds them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImagesMade {
    /// The include with every image's constants (`INCLUDE "images.inc"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include: Option<String>,
    pub images: Vec<ImageAsset>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub problems: Vec<ImageProblem>,
    /// The rule that gave every image's colors their shades; none when the
    /// build stopped before choosing (a bad `assets/palettes.txt`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shades: Option<ShadeChoice>,
    /// What the images share on the GBC, when the target is the GBC.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gbc: Option<GbcImagesMade>,
}

impl ImagesMade {
    /// The first problem that stops the build.
    pub fn first_error(&self) -> Option<&ImageProblem> {
        self.problems.iter().find(|p| p.severity == Severity::Error)
    }
}

#[cfg(test)]
mod tests;
