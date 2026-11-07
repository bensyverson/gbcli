//! What `build` reports about the images it converted (design §12, Build,
//! the 2026-11-07 image update): each PNG under `assets/bg/` and
//! `assets/sprites/`, what it became, and every hardware limit it breaks.
//! `gbcli-build` makes them and the CLI prints them.

use serde::{Deserialize, Serialize};

use crate::toolchain::Severity;

/// Which convention an image follows, from its directory.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageKind {
    /// `assets/bg/*.png`: a background, as unique tiles and a tilemap.
    Bg,
    /// `assets/sprites/*.png`: a sheet of 8x16 objects, as unique 8x16 units
    /// and one object list per frame.
    Sprites,
}

impl ImageKind {
    /// The directory under `assets/`, and the word on the wire.
    pub fn dir(self) -> &'static str {
        match self {
            ImageKind::Bg => "bg",
            ImageKind::Sprites => "sprites",
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
    /// `assets/palette.txt`, the declared shades, isn't four `#rrggbb`
    /// lines; `line` names the line when one is at fault.
    PaletteFile,
    /// The stretched shade rule put a color two or more shades from its
    /// plain nearest shade. A warning: declaring `assets/palette.txt` picks
    /// the shades instead.
    ShadeStretched,
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
    /// `assets/palette.txt` lists shades 0-3; each color takes its nearest
    /// entry's shade.
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

/// One image, converted.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
    /// build stopped before choosing (a bad `assets/palette.txt`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shades: Option<ShadeChoice>,
}

impl ImagesMade {
    /// The first problem that stops the build.
    pub fn first_error(&self) -> Option<&ImageProblem> {
        self.problems.iter().find(|p| p.severity == Severity::Error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_problem_spells_its_limit_snake_case_and_drops_what_it_lacks() {
        let p = ImageProblem {
            image: Some("assets/bg/town.png".into()),
            limit: ImageLimit::BgTilesMap,
            severity: Severity::Error,
            count: 291,
            max: 256,
            tiles: vec![TileAt { x: 3, y: 4 }],
            line: None,
            message: "too many".into(),
            remedy: "fewer".into(),
        };
        let json = serde_json::to_string(&p).unwrap();
        assert_eq!(
            json,
            r#"{"image":"assets/bg/town.png","limit":"bg_tiles_map","severity":"error","count":291,"max":256,"tiles":[{"x":3,"y":4}],"message":"too many","remedy":"fewer"}"#
        );
        assert_eq!(serde_json::from_str::<ImageProblem>(&json).unwrap(), p);
    }

    #[test]
    fn a_shared_shade_warning_round_trips_as_shared_shade() {
        let p = ImageProblem {
            image: Some("assets/bg/cave.png".into()),
            limit: ImageLimit::SharedShade,
            severity: Severity::Warning,
            count: 2,
            max: 1,
            tiles: vec![TileAt { x: 1, y: 0 }],
            line: None,
            message: "one shade".into(),
            remedy: "distinct".into(),
        };
        let json = serde_json::to_string(&p).unwrap();
        assert_eq!(
            json,
            r#"{"image":"assets/bg/cave.png","limit":"shared_shade","severity":"warning","count":2,"max":1,"tiles":[{"x":1,"y":0}],"message":"one shade","remedy":"distinct"}"#
        );
        assert_eq!(serde_json::from_str::<ImageProblem>(&json).unwrap(), p);
    }

    #[test]
    fn a_palette_file_problem_carries_its_line() {
        let p = ImageProblem {
            image: Some("assets/palette.txt".into()),
            limit: ImageLimit::PaletteFile,
            severity: Severity::Error,
            count: 2,
            max: 4,
            tiles: Vec::new(),
            line: Some(3),
            message: "not a color".into(),
            remedy: "one #rrggbb a line".into(),
        };
        let json = serde_json::to_string(&p).unwrap();
        assert_eq!(
            json,
            r#"{"image":"assets/palette.txt","limit":"palette_file","severity":"error","count":2,"max":4,"line":3,"message":"not a color","remedy":"one #rrggbb a line"}"#
        );
        assert_eq!(serde_json::from_str::<ImageProblem>(&json).unwrap(), p);
    }

    #[test]
    fn the_shade_rule_round_trips_with_its_known_palette() {
        let made = ImagesMade {
            shades: Some(ShadeChoice {
                rule: ShadeRule::Known,
                palette: Some(KnownPalette::PeaGreens),
            }),
            ..ImagesMade::default()
        };
        let json = serde_json::to_string(&made).unwrap();
        assert_eq!(
            json,
            r#"{"images":[],"shades":{"rule":"known","palette":"pea_greens"}}"#
        );
        assert_eq!(serde_json::from_str::<ImagesMade>(&json).unwrap(), made);
        for rule in [
            ShadeRule::Declared,
            ShadeRule::Known,
            ShadeRule::Stretched,
            ShadeRule::Nearest,
        ] {
            assert_eq!(serde_json::to_value(rule).unwrap(), rule.name());
        }
        for palette in KnownPalette::ALL {
            assert_eq!(serde_json::to_value(palette).unwrap(), palette.name());
        }
        let names: Vec<&str> = KnownPalette::ALL.iter().map(|p| p.name()).collect();
        assert_eq!(
            names,
            [
                "dmg_grays",
                "pea_greens",
                "gb_studio_greens",
                "pocket_grays"
            ]
        );
    }

    #[test]
    fn every_limit_name_matches_the_wire() {
        use ImageLimit::*;
        for limit in [
            BgTilesMap,
            VramTiles,
            ObjTiles,
            DmgPalette,
            SizeMultiple8,
            ObjPair8x16,
            ObjNoTransparency,
            ObjPerLine,
            ObjTotal,
            FrameGrid,
            FrameSpan,
            RomxBank,
            Alpha,
            RgbgfxAgrees,
            SharedShade,
            PaletteFile,
            ShadeStretched,
        ] {
            assert_eq!(serde_json::to_value(limit).unwrap(), limit.name());
        }
        for kind in [ImageKind::Bg, ImageKind::Sprites] {
            assert_eq!(serde_json::to_value(kind).unwrap(), kind.dir());
        }
    }
}
