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

/// A hardware limit an image can break (Pan Docs), or `alpha`, the one
/// check that isn't a limit but would make the conversion guess.
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
    /// A pixel neither opaque nor fully transparent, or a transparent pixel
    /// in a background, which has none.
    Alpha,
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
            ImageLimit::Alpha => "alpha",
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
    /// (`vram_tiles`, `obj_tiles`), which `message` then itemizes.
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
    pub message: String,
    pub remedy: String,
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
            Alpha,
        ] {
            assert_eq!(serde_json::to_value(limit).unwrap(), limit.name());
        }
        for kind in [ImageKind::Bg, ImageKind::Sprites] {
            assert_eq!(serde_json::to_value(kind).unwrap(), kind.dir());
        }
    }
}
