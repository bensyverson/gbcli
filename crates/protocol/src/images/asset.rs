//! Reading an [`ImageAsset`]: the fields as sent, then the checks that tie
//! its tile map to its kind and target. The slot rides in each entry, so a
//! map and its palettes can't differ in length or in where a unit is
//! transparent; what is left is whether the entries fit the image.

use serde::Deserialize;

use super::{GbcImage, ImageAsset, ImageKind, TileEntry};

/// [`ImageAsset`] as sent, before its tile map is checked.
#[derive(Deserialize)]
pub(super) struct Unchecked {
    kind: ImageKind,
    source: String,
    label: String,
    width: u32,
    height: u32,
    tiles: u32,
    #[serde(default)]
    frames: Option<u32>,
    #[serde(default)]
    objects: Option<u32>,
    preview: String,
    #[serde(default)]
    tile_map: Vec<Option<TileEntry>>,
    #[serde(default)]
    gbc: Option<GbcImage>,
}

impl TryFrom<Unchecked> for ImageAsset {
    type Error = String;

    fn try_from(u: Unchecked) -> Result<ImageAsset, String> {
        let asset = ImageAsset {
            kind: u.kind,
            source: u.source,
            label: u.label,
            width: u.width,
            height: u.height,
            tiles: u.tiles,
            frames: u.frames,
            objects: u.objects,
            preview: u.preview,
            tile_map: u.tile_map,
            gbc: u.gbc,
        };
        check(&asset)?;
        Ok(asset)
    }
}

/// The first entry that doesn't fit the image, as a message.
fn check(asset: &ImageAsset) -> Result<(), String> {
    let sheet = asset.kind == ImageKind::Sprites;
    let gbc = asset.gbc.is_some();
    for (i, entry) in asset.tile_map.iter().enumerate() {
        let Some(entry) = entry else {
            if sheet {
                continue;
            }
            return Err(format!(
                "tile_map[{i}] is null, but only a sheet's transparent unit has no tile"
            ));
        };
        if !sheet && !entry.flip.is_none() {
            return Err(format!(
                "tile_map[{i}] flips, but only a sheet's units flip"
            ));
        }
        match (gbc, entry.palette.is_some()) {
            (true, false) => {
                return Err(format!(
                    "tile_map[{i}] has no palette, but the image has a GBC version, so every \
                     tile has a slot"
                ));
            }
            (false, true) => {
                return Err(format!(
                    "tile_map[{i}] has a palette, but the image has no GBC version"
                ));
            }
            _ => {}
        }
    }
    Ok(())
}
