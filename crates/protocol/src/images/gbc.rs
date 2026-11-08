//! An image's GBC version and what the images share on the GBC (the GBC
//! images plan, revision 2): the per-image results a GBC build adds to
//! `ImagesMade`, re-exported from `images`.

use serde::{Deserialize, Serialize};

use crate::palettes::GbcPalette;

/// How an image is colored on the GBC (plan revision 2: two kinds of art).
/// On the wire, `"art":"color"` or `"art":"indexed","palette":NAME`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "art", rename_all = "snake_case")]
pub enum ImageArt {
    /// Drawn in any colors: gbcli packs its GBC palettes and derives its
    /// DMG version (or reads `name.dmg.png`). Its GBC tile data is its own.
    Color,
    /// Drawn in four shades, as DMG art, and colored on the GBC by the
    /// named palette `assets/palettes.txt` assigns it. Its tile data serves
    /// both machines, so a program can recolor it by loading another
    /// named palette.
    Indexed {
        /// The assigned palette's name in [`crate::NamedPalettes`].
        palette: String,
    },
}

/// An image's GBC version: how it is colored, its palettes, and the
/// palette of each tile or object.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GbcImage {
    #[serde(flatten)]
    pub art: ImageArt,
    /// A background's or tileset's BG palettes, in slot order (slot 0 is
    /// palette 0 of attribute bits 0-2): packed for color art, the assigned
    /// palette alone for indexed art. Empty for a sheet, whose slots are
    /// in the shared [`GbcImagesMade::obj_palettes`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub palettes: Vec<GbcPalette>,
    /// The palette slot (0-7) of each source tile, parallel to
    /// [`crate::ImageAsset::tile_indices`]: a background's attribute map, palette
    /// bits only; for a sheet, the OBJ palette of each unit. None where
    /// `tile_indices` is none.
    pub tile_palettes: Vec<Option<u8>>,
}

/// What the GBC version of the images shares across them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GbcImagesMade {
    /// The 8 OBJ palettes every sheet shares, in slot order: the packed
    /// palettes of color sheets, then one per distinct palette assigned to
    /// an indexed sheet. Slots the program loads itself aren't listed.
    pub obj_palettes: Vec<GbcPalette>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::images::{ImageAsset, ImageKind, ImagesMade};
    use crate::palettes::Rgb555;

    fn asset(kind: ImageKind) -> ImageAsset {
        ImageAsset {
            kind,
            source: "assets/bg/town.png".into(),
            label: "Town".into(),
            width: 2,
            height: 1,
            tiles: 2,
            frames: None,
            objects: None,
            preview: "build/images/bg/town.preview.png".into(),
            tile_indices: Vec::new(),
            gbc: None,
        }
    }

    /// A DMG build fills neither new field, so its JSON is as before the
    /// GBC types (the CLI golden `images.json.txt` pins a whole build).
    #[test]
    fn a_dmg_image_has_no_tile_indices_or_gbc_on_the_wire() {
        let a = asset(ImageKind::Bg);
        let json = serde_json::to_string(&a).unwrap();
        assert_eq!(
            json,
            r#"{"kind":"bg","source":"assets/bg/town.png","label":"Town","width":2,"height":1,"tiles":2,"preview":"build/images/bg/town.preview.png"}"#
        );
        assert_eq!(serde_json::from_str::<ImageAsset>(&json).unwrap(), a);
        assert_eq!(
            serde_json::to_string(&ImagesMade::default()).unwrap(),
            r#"{"images":[]}"#
        );
    }

    #[test]
    fn a_color_background_carries_its_palettes_and_each_tiles_slot() {
        let grass = GbcPalette([
            Rgb555::WHITE,
            Rgb555::new(0, 31, 0).unwrap(),
            Rgb555::new(0, 15, 0).unwrap(),
            Rgb555::BLACK,
        ]);
        let a = ImageAsset {
            tile_indices: vec![Some(0), Some(1)],
            gbc: Some(GbcImage {
                art: ImageArt::Color,
                palettes: vec![grass],
                tile_palettes: vec![Some(0), Some(0)],
            }),
            ..asset(ImageKind::Bg)
        };
        let json = serde_json::to_string(&a).unwrap();
        assert!(
            json.ends_with(
                r#""tile_indices":[0,1],"gbc":{"art":"color","palettes":[[32767,992,480,0]],"tile_palettes":[0,0]}}"#
            ),
            "{json}"
        );
        assert_eq!(serde_json::from_str::<ImageAsset>(&json).unwrap(), a);
    }

    /// An indexed sheet names its palette; its slots are in the shared OBJ
    /// palettes, and a transparent unit has no tile and no slot.
    #[test]
    fn an_indexed_sheet_names_its_palette_and_shares_the_obj_palettes() {
        let ember = GbcPalette([
            Rgb555::WHITE,
            Rgb555::new(31, 16, 0).unwrap(),
            Rgb555::new(20, 4, 0).unwrap(),
            Rgb555::new(4, 0, 0).unwrap(),
        ]);
        let made = ImagesMade {
            images: vec![ImageAsset {
                tile_indices: vec![Some(0), None],
                gbc: Some(GbcImage {
                    art: ImageArt::Indexed {
                        palette: "ember".into(),
                    },
                    palettes: Vec::new(),
                    tile_palettes: vec![Some(2), None],
                }),
                ..asset(ImageKind::Sprites)
            }],
            gbc: Some(GbcImagesMade {
                obj_palettes: vec![ember],
            }),
            ..ImagesMade::default()
        };
        let json = serde_json::to_string(&made).unwrap();
        assert!(
            json.contains(
                r#""tile_indices":[0,null],"gbc":{"art":"indexed","palette":"ember","tile_palettes":[2,null]}}"#
            ),
            "{json}"
        );
        assert!(
            json.ends_with(r#""gbc":{"obj_palettes":[[32767,543,148,4]]}}"#),
            "{json}"
        );
        assert_eq!(serde_json::from_str::<ImagesMade>(&json).unwrap(), made);
        let bad = r#"{"art":"tinted","tile_palettes":[]}"#;
        assert!(serde_json::from_str::<GbcImage>(bad).is_err());
    }

    /// A tileset: tiles and the source tile → tile map, no frames.
    #[test]
    fn a_tileset_is_its_tile_map() {
        let a = ImageAsset {
            source: "assets/tiles/walls.png".into(),
            tile_indices: vec![Some(0), Some(0)],
            tiles: 1,
            ..asset(ImageKind::Tiles)
        };
        let json = serde_json::to_string(&a).unwrap();
        assert!(json.starts_with(r#"{"kind":"tiles","source":"assets/tiles/walls.png""#));
        assert!(json.ends_with(r#""tile_indices":[0,0]}"#), "{json}");
        assert_eq!(serde_json::from_str::<ImageAsset>(&json).unwrap(), a);
    }
}
