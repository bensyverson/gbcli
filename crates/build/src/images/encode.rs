//! The image plans encoded as the Game Boy's data: 2bpp tile bytes, a
//! background's tilemap, and under `--gbc` the color data and its attribute
//! map (`gbc`). gbcli decides every pixel's color index, every cell's tile
//! and every tile's palette before this runs, so encoding is a function of
//! the plan alone: no tool is run and no file is read (Ben's ruling,
//! 2026-11-09). rgbgfx 1.0.4 wrote these bytes until then; it is the
//! differential test's oracle only (`tests/it/images_differential.rs`).

use super::bg::BgPlan;
use super::sheet::SheetPlan;

/// A converted background: unique tiles (2bpp) and its tilemap.
pub(crate) struct BgData {
    pub tiles: Vec<u8>,
    pub map: Vec<u8>,
}

/// Color indices, 8 a row, as 2bpp: each row's low plane, then its high.
pub(crate) fn encode(indices: &[u8]) -> Vec<u8> {
    indices
        .chunks(8)
        .flat_map(|row| {
            let plane = |bit: u8| {
                row.iter()
                    .enumerate()
                    .fold(0u8, |b, (x, &i)| b | (((i >> bit) & 1) << (7 - x)))
            };
            [plane(0), plane(1)]
        })
        .collect()
}

/// A background's unique tiles, 16 bytes each, in `unique`'s order, and its
/// map: one byte a cell, the unique tile it draws.
pub(crate) fn background(plan: &BgPlan) -> BgData {
    BgData {
        tiles: plan.unique.iter().flat_map(|tile| encode(tile)).collect(),
        map: plan.cells.iter().map(|&unique| unique as u8).collect(),
    }
}

/// A sheet's kept units, each its tiles top first, 16 bytes a tile.
pub(crate) fn sheet(plan: &SheetPlan) -> Vec<u8> {
    plan.drawn.iter().flat_map(|unit| encode(unit)).collect()
}

#[cfg(test)]
mod tests {
    use gbcli_protocol::ObjectSize;

    use super::*;

    /// 2bpp as color indices, 8 a row: the reader the differential test
    /// uses, kept here to check `encode` against something independent.
    fn indices(data: &[u8]) -> Vec<u8> {
        data.chunks(2)
            .flat_map(|row| {
                let (low, high) = (row[0], row[1]);
                (0..8).map(move |x| ((low >> (7 - x)) & 1) | (((high >> (7 - x)) & 1) << 1))
            })
            .collect()
    }

    fn plan(unique: Vec<Vec<u8>>, cells: Vec<usize>) -> BgPlan {
        BgPlan {
            warnings: Vec::new(),
            width: cells.len() as u32,
            height: 1,
            tiles: unique.len() as u32,
            unique,
            cells,
        }
    }

    fn sheet_plan(drawn: Vec<Vec<u8>>, object: ObjectSize) -> SheetPlan {
        let kept = drawn.len();
        SheetPlan {
            object,
            obp: 0,
            width: kept as u32,
            height: kept as u32 * object.tiles(),
            frame: (kept as u32 * 8, object.height()),
            kept: (0..kept as u32).map(|n| (n * 8, 0)).collect(),
            uses: vec![1; kept],
            frames: Vec::new(),
            units: vec![Some((0, Default::default()))],
            warnings: Vec::new(),
            drawn,
        }
    }

    fn tall(indices: u8) -> Vec<u8> {
        vec![indices; 128]
    }

    fn square(indices: u8) -> Vec<u8> {
        vec![indices; 64]
    }

    #[test]
    fn a_row_of_indices_is_two_planes_low_first_pixel_seven_high() {
        // $80 low and $01 high: pixel 0 is index 1, pixel 7 index 2.
        assert_eq!(encode(&[1, 0, 0, 0, 0, 0, 0, 2]), [0x80, 0x01]);
        let mixed: Vec<u8> = (0..64).map(|i| (i * 7 % 4) as u8).collect();
        assert_eq!(encode(&mixed).len(), 16);
        assert_eq!(indices(&encode(&mixed)), mixed);
        // Index 3 is both planes, index 0 neither, index 1 the low alone.
        assert_eq!(&encode(&[3; 8])[..], [0xFF, 0xFF]);
        assert_eq!(&encode(&[0; 8])[..], [0x00, 0x00]);
        assert_eq!(&encode(&[1; 8])[..], [0xFF, 0x00]);
    }

    #[test]
    fn a_backgrounds_tiles_are_its_unique_tiles_and_its_map_is_its_cells() {
        let d = background(&plan(
            vec![vec![0; 64], vec![1; 64], vec![3; 64]],
            vec![2, 0, 1, 2],
        ));
        assert_eq!(d.tiles.len(), 3 * 16);
        assert_eq!(&d.tiles[32..48], &[0xFFu8; 16]);
        assert_eq!(d.map, [2, 0, 1, 2]);
    }

    #[test]
    fn a_sheets_data_is_its_kept_units_top_tile_first() {
        let d = sheet(&sheet_plan(
            vec![tall(1), tall(2), tall(3)],
            ObjectSize::Tall,
        ));
        // Three 8x16 units: two tiles each, the unit's 16 rows a tile.
        assert_eq!(d.len(), 6 * 16);
        let two = [encode(&[2; 64]), encode(&[2; 64])].concat();
        assert_eq!(&d[32..64], &two[..]);
        assert_eq!(&d[0..16], &encode(&[1; 64])[..]);
    }

    #[test]
    fn an_8x8_sheets_data_is_a_tile_a_unit() {
        let d = sheet(&sheet_plan(vec![square(0), square(3)], ObjectSize::Square));
        assert_eq!(d.len(), 2 * 16);
        assert_eq!(&d[16..32], &encode(&[3; 64])[..]);
    }
}
