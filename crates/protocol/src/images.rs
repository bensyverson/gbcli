//! What `build` reports about the images it converted (design §12, Build,
//! the 2026-11-07 image update): each PNG under `assets/bg/` and
//! `assets/sprites/`, what it became, and every hardware limit it breaks.
//! `gbcli-build` makes them and the CLI prints them. A GBC build adds each
//! image's GBC version (`GbcImage`) and what the images share
//! (`GbcImagesMade`); these are the seam a client converting assets
//! without a build (LoadHigh) reads, so `images.inc` is generated from
//! them and the two can't disagree.

use serde::{Deserialize, Serialize};

use crate::font::FontAsset;
use crate::toolchain::Severity;

mod asset;
mod gbc;
mod made;
mod map;

pub use gbc::{GbcImage, GbcImagesMade, ImageArt};
pub use map::{Flip, NotPaletteSlot, PaletteSlot, TileEntry};

/// Which convention an image follows, from its directory.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageKind {
    /// `assets/bg/*.png`: a background, as unique tiles and a tilemap.
    Bg,
    /// `assets/sprites/*.png`: a sheet of objects ([`ObjectSize`]), as
    /// unique objects and one object list per frame.
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

/// The size of a sheet's objects, as LCDC bit 2 sets it for every object
/// on screen. A sheet's name gives it: a frame size whose height is a
/// multiple of 8 but not 16 (`shot.8x8.png`, `ship.16x8.png`) makes 8x8
/// objects; any other sheet makes 8x16.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObjectSize {
    /// 8x8: one tile an object (LCDC_OBJ_16 clear).
    #[serde(rename = "8x8")]
    Square,
    /// 8x16: two tiles an object, top over bottom (LCDC_OBJ_16 set).
    #[serde(rename = "8x16")]
    Tall,
}

impl ObjectSize {
    /// An object's height in pixels.
    pub fn height(self) -> u32 {
        match self {
            ObjectSize::Square => 8,
            ObjectSize::Tall => 16,
        }
    }

    /// The tiles one object takes.
    pub fn tiles(self) -> u32 {
        self.height() / 8
    }

    /// The name on the wire and in the include's comments.
    pub fn name(self) -> &'static str {
        match self {
            ObjectSize::Square => "8x8",
            ObjectSize::Tall => "8x16",
        }
    }
}

/// A hardware limit an image can break (Pan Docs); or `alpha`, which would
/// make the conversion guess, `frame_grid`, a sheet its frames don't
/// tile, or `shared_shade`, colors drawn alike: the checks that aren't
/// limits.
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
    /// of 16, and so is a frame's unless it is 8x8 objects' (a multiple of
    /// 8 but not 16; [`ObjectSize`]). Also a warning with no `image` when
    /// the project sets no object size and its sheets are cut in both
    /// (LCDC bit 2 shows one at a time); `count` 2, `max` 1, the message
    /// naming the sheets of each size.
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
    /// in a background, which has none; or, under `--gbc`, a sheet pixel
    /// transparent in its color image but not in its `name.dmg.png`, or
    /// the other way round (one frame list serves both).
    Alpha,
    /// Two or more colors take one DMG shade, by the project's shade rule
    /// (`ShadeChoice`), and draw alike. A warning: BGP and OBP map indices
    /// to shades, and the hardware is content to show two colors as one.
    /// With `image`, the colors are in that image (color art's: in one tile
    /// or object of it); without, they share the shade only across images
    /// of one named palette (or of none), one problem a shade, and
    /// `message` names the images each is in. Colors of different named
    /// palettes sharing a shade aren't a problem: on the DMG every
    /// palette's lightest color is white.
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
    /// plain nearest shade. A warning: a `dmg` line in `assets/palettes.txt`
    /// (a manifest's `palettes.dmg`) picks the shades instead.
    ShadeStretched,
    /// GBC: 8 palettes. Per background, its packed palettes (or its one
    /// assigned palette); across all sheets, the packed OBJ palettes plus
    /// one per assigned palette and shade mapping (OBP) of indexed sheets.
    /// `count` is the palettes placed plus the slots assigned palettes take
    /// (the placed are never fewer than the lower bound, ⌈colors / 4⌉ or
    /// ⌈colors / 3⌉ for sheets, which the message names); `tiles` are the
    /// tiles that landed in palette 9 or later, one problem per image
    /// holding any.
    GbcPalettes,
    /// GBC: 4 colors in one background tile, or 3 opaque colors (plus
    /// transparent) in one object; `tiles` names the tiles at fault.
    GbcTileColors,
    /// `name.dmg.png`, the DMG version that overrides the one derived from
    /// a color image (in every build), isn't the color image's size; `count` and `max` are
    /// its width and the color image's when they differ, else the heights.
    DmgOverrideSize,
    /// GBC: the project has indexed art and its palettes have no `dmg`
    /// (the `dmg` line of `assets/palettes.txt`, or an `assets` manifest's
    /// `palettes.dmg`), so its shades are chosen from every color in the project
    /// and other art can recolor it: a sheet's OBP, and so its reordered OBJ
    /// palette, or a background's tile data and so the palette entries it
    /// draws. A warning, once for the project (no `image`); `count` is the
    /// indexed images, `max` 0; the message names them.
    ShadesUnpinned,
    /// A pixel text image (`.pixels`) doesn't parse, or names a palette
    /// entry the palettes don't have; `line` and `column` say where. The
    /// build still checks every other image.
    PixelsText,
    /// A font file (`assets/fonts/NAME.yaff`) isn't gbcli's YAFF subset;
    /// `line` and `column` say where.
    FontText,
    /// A build emits at most 254 glyphs of a font (`$FE` and `$FF` are the
    /// charmap's newline and end), and this font holds more with no
    /// character list (`NAME.charlist`) to limit them; `count` is its
    /// glyphs, or the list's.
    FontGlyphs,
    /// A character asked for (an `assets` request's `glyphs`, or a
    /// build's character list) that the font has no glyph for; `message`
    /// names it and its code point.
    FontMissing,
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
            ImageLimit::SharedShade => "shared_shade",
            ImageLimit::PaletteFile => "palette_file",
            ImageLimit::ShadeStretched => "shade_stretched",
            ImageLimit::GbcPalettes => "gbc_palettes",
            ImageLimit::GbcTileColors => "gbc_tile_colors",
            ImageLimit::DmgOverrideSize => "dmg_override_size",
            ImageLimit::ShadesUnpinned => "shades_unpinned",
            ImageLimit::PixelsText => "pixels_text",
            ImageLimit::FontText => "font_text",
            ImageLimit::FontGlyphs => "font_glyphs",
            ImageLimit::FontMissing => "font_missing",
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
    /// (`palette_file`, `pixels_text`); for a hardware limit a pixel text
    /// image breaks, the line of the block (or map row, or `fill`) that drew
    /// the most of the first tile's legend characters, the later on a tie.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    /// The column at fault, 1-based in characters (`pixels_text`); never 0,
    /// which keeps the problem small.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<std::num::NonZeroU32>,
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

/// What decided one color's DMG shade: its pin, else the project's rule
/// ([`ShadeRule`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShadeSource {
    /// A pin ([`crate::ShadePin`]), ahead of every rule.
    Pinned,
    Declared,
    Known,
    Stretched,
    Nearest,
}

impl ShadeSource {
    /// The name on the wire.
    pub fn name(self) -> &'static str {
        match self {
            ShadeSource::Pinned => "pinned",
            ShadeSource::Declared => "declared",
            ShadeSource::Known => "known",
            ShadeSource::Stretched => "stretched",
            ShadeSource::Nearest => "nearest",
        }
    }
}

impl From<ShadeRule> for ShadeSource {
    fn from(rule: ShadeRule) -> ShadeSource {
        match rule {
            ShadeRule::Declared => ShadeSource::Declared,
            ShadeRule::Known => ShadeSource::Known,
            ShadeRule::Stretched => ShadeSource::Stretched,
            ShadeRule::Nearest => ShadeSource::Nearest,
        }
    }
}

/// One color the project's shading decided, and how.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColorShade {
    pub color: crate::Rgb555,
    /// 0 (lightest) to 3.
    pub shade: u8,
    pub rule: ShadeSource,
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

/// A named palette as the DMG shows it, by the project's [`ShadeChoice`], so
/// a palette and the art agree on what a color's shade is.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DmgPalette {
    /// The palette's name in [`crate::NamedPalette`].
    pub name: String,
    /// What to write to BGP (or an OBP, whose index 0 is never drawn): entry
    /// `i`'s shade in bits `2i` and `2i + 1`. Two entries may share a shade;
    /// the byte says so and nothing warns, since BGP allows it.
    pub byte: u8,
}

/// One image, converted. On read, its tile map must agree with it
/// (`asset::Unchecked`): one entry a cell, or none on a DMG image; a tile
/// it has in each, a sheet unit's even; a slot in every entry exactly when
/// it has a GBC version; and a flip or a missing tile only in a sheet.
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
    /// A sheet's object size; a sheet read without one (a document from
    /// before 8x8 sheets) is 8x16, and an image that isn't a sheet has
    /// none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object_size: Option<ObjectSize>,
    /// A sheet's OBP on the DMG: the shade each color index of its data
    /// shows, two bits an index, index 0 in bits 0-1 (transparent, so 0).
    /// Indices 1 up are the shades the sheet uses, lightest first; an
    /// index past them is 0. An image that isn't a sheet has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub obp: Option<u8>,
    /// The converted data drawn as the hardware shows it, then its unique
    /// tiles by use, most used first. Every build writes one; empty, and
    /// left off the wire, when an `assets` request kept no previews.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub preview: String,
    /// Where each source tile went ([`TileEntry`]: its tile, its flip and
    /// its GBC palette slot), one entry per 8x8 tile of the image (object
    /// of a sheet), rows top to bottom, left to right in each. None
    /// for a sheet's fully transparent unit, which takes no tile. A
    /// tileset's result is this map; a background's is also its tilemap
    /// and, on the GBC, its attribute map; a sheet's matches its frame
    /// lists. Every build fills it, a DMG build included, each entry
    /// without a slot when the image has no GBC version (Ben's ruling,
    /// 2026-11-07). Empty only in a document from before that ruling.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tile_map: Vec<Option<TileEntry>>,
    /// The GBC version, when the build's target is the GBC: every image of
    /// a GBC build has one, color or indexed art. Every image has a DMG
    /// version; under `--gbc` color art's tiles are deduplicated on both
    /// versions at once, so `tiles` and `tile_map` serve both machines.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gbc: Option<GbcImage>,
}

/// The images a build converted, and where the program finds them. On
/// read, an image with a GBC version needs `gbc` beside it, and a sheet's
/// slots must be among its `obj_palettes` (`made::Unchecked`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "made::Unchecked")]
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
    /// Each named palette as DMG shades, in the palettes' order, whether or
    /// not an image uses it; empty when the project names none or the build
    /// stopped before choosing `shades`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dmg_palettes: Vec<DmgPalette>,
    /// Every art color's shade and every named palette's colors', each
    /// color once, sorted by color (its RGB555 word), with what decided
    /// it (LoadHigh's question 993); empty when the build stopped before
    /// choosing `shades`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub color_shades: Vec<ColorShade>,
    /// What the images share on the GBC, when the target is the GBC.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gbc: Option<GbcImagesMade>,
    /// The fonts under `assets/fonts/` (or a manifest's), in name order:
    /// for a build, the glyphs it emitted; for `assets`, those asked for.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fonts: Vec<FontAsset>,
    /// Composed glyphs among `fonts` whose mark lands on their base's ink:
    /// a warning each.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub font_overlaps: Vec<crate::font_author::FontOverlap>,
}

impl ImagesMade {
    /// The first problem that stops the build.
    pub fn first_error(&self) -> Option<&ImageProblem> {
        self.problems.iter().find(|p| p.severity == Severity::Error)
    }
}

#[cfg(test)]
mod tests;
