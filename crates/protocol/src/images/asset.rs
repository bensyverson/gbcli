//! Reading an [`ImageAsset`]: the fields as sent, then the checks that tie
//! its tile map to its kind, size and target. The slot rides in each entry,
//! so a map and its palettes can't differ in length or in where a unit is
//! transparent; what is left is whether the entries fit the image: one a
//! cell, each naming a tile it has and a slot its palettes hold, as the
//! build makes them. A sheet's slots index the shared OBJ palettes, which
//! `ImagesMade` checks (`made`).

use serde::Deserialize;

use super::{GbcImage, ImageArt, ImageAsset, ImageKind, ObjectSize, PaletteSlot, TileEntry};

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
    #[serde(default)]
    object_size: Option<ObjectSize>,
    #[serde(default)]
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
            object_size: u.object_size,
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
    if !sheet && asset.object_size.is_some() {
        return Err("object_size is set, but only a sheet has objects".into());
    }
    // A unit is a cell of the map: a tile, or a sheet's object.
    let size = asset.object_size.unwrap_or(ObjectSize::Tall);
    let unit = if sheet { size.tiles() } else { 1 };
    if !asset.height.is_multiple_of(unit) {
        return Err(format!(
            "the sheet is {} tiles high, but a sheet is whole {} objects, {unit} tiles high",
            asset.height,
            size.name()
        ));
    }
    // The BG palettes a background's slots index; none for a sheet.
    let palettes = asset.gbc.as_ref().map_or(0, |g| g.palettes.len());
    if let Some(g) = &asset.gbc {
        let indexed = matches!(g.art, ImageArt::Indexed { .. });
        let wrong = match (sheet, indexed, palettes) {
            (true, _, 0) => None,
            (true, _, _) => {
                Some("a sheet's palettes are the shared OBJ palettes, so it lists none")
            }
            (false, true, 1) => None,
            (false, true, _) => Some("an indexed background has one palette, its named one"),
            (false, false, 1..=PaletteSlot::COUNT) => None,
            (false, false, _) => Some("a color background has 1 to 8 BG palettes"),
        };
        if let Some(why) = wrong {
            return Err(format!("gbc lists {palettes} palettes, but {why}"));
        }
    }
    let indexed_sheet = sheet
        && asset
            .gbc
            .as_ref()
            .is_some_and(|g| matches!(g.art, ImageArt::Indexed { .. }));
    let mut sheet_slot: Option<PaletteSlot> = None;
    // A cell is an 8x8 tile, or a sheet's object.
    let cells = u64::from(asset.width) * u64::from(asset.height / unit);
    let objects = format!("{} objects", size.name());
    let what = if sheet { objects.as_str() } else { "tiles" };
    match asset.tile_map.len() as u64 {
        0 if gbc => {
            return Err(format!(
                "tile_map is empty, but the image has a GBC version, so each of its {cells} \
                 {what} has an entry"
            ));
        }
        0 => {}
        n if n != cells => {
            return Err(format!(
                "tile_map lists {n}, but the {}x{} image has {cells} {what}",
                asset.width, asset.height
            ));
        }
        _ => {}
    }
    let tiles = u64::from(asset.tiles);
    for (i, entry) in asset.tile_map.iter().enumerate() {
        let Some(entry) = entry else {
            if sheet {
                continue;
            }
            return Err(format!(
                "tile_map[{i}] is null, but only a sheet's transparent unit has no tile"
            ));
        };
        let index = u64::from(entry.index);
        let unit = u64::from(unit);
        if !index.is_multiple_of(unit) {
            return Err(format!(
                "tile_map[{i}] is tile {index}, but an 8x16 object's index is its top tile, even"
            ));
        }
        let last = index + unit - 1;
        if last >= tiles {
            return Err(format!(
                "tile_map[{i}] needs tile {last}, but the image has {tiles} tiles"
            ));
        }
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
        let Some(slot) = entry.palette else { continue };
        if !sheet && usize::from(slot.get()) >= palettes {
            return Err(format!(
                "tile_map[{i}] names BG palette {}, but the image has {palettes}",
                slot.get()
            ));
        }
        if indexed_sheet && *sheet_slot.get_or_insert(slot) != slot {
            return Err(format!(
                "tile_map[{i}] names OBJ palette {}, but an indexed sheet has one slot, and \
                 an earlier unit names {}",
                slot.get(),
                sheet_slot.map_or(0, PaletteSlot::get)
            ));
        }
    }
    Ok(())
}
