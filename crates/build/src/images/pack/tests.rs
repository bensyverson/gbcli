//! The packer against research §3's cases, and its determinism.

use std::collections::BTreeSet;

use gbcli_protocol::{
    GbcPalette, ImageArt, ImageLimit, ImageProblem, PaletteSlot, Rgb555, Severity, TileAt,
};

use super::{Layer, Packing, Source, census, pack};
use crate::images::pixels::{Pixels, Rgba};

/// The `n`th of 1024 distinct colors (red, then green), as RGB555.
fn c(n: u16) -> Rgb555 {
    Rgb555::new((n % 32) as u8, (n / 32) as u8, 0).unwrap()
}

fn rgba(color: Rgb555) -> Rgba {
    let [r, g, b] = color.channels();
    [r << 3, g << 3, b << 3, 0xFF]
}

/// Cells side by side, `height` pixels high; each cell's pixels cycle
/// through its colors, `None` being transparent.
fn strip(cells: &[Vec<Option<Rgb555>>], height: u32) -> Pixels {
    let width = 8 * cells.len() as u32;
    let mut rgba_px = Vec::new();
    for y in 0..height {
        for x in 0..width {
            let cell = &cells[(x / 8) as usize];
            let i = ((y * 8 + x % 8) as usize) % cell.len();
            rgba_px.push(cell[i].map_or([0; 4], rgba));
        }
    }
    Pixels {
        width,
        height,
        rgba: rgba_px,
    }
}

/// A background of 8x8 tiles in a row, each in the colors listed.
fn bg(tiles: &[Vec<u16>]) -> Pixels {
    let cells: Vec<Vec<Option<Rgb555>>> = tiles
        .iter()
        .map(|t| t.iter().map(|&n| Some(c(n))).collect())
        .collect();
    strip(&cells, 8)
}

fn pack_bg(pixels: &Pixels, reserved: usize) -> Result<Packing, Vec<ImageProblem>> {
    let sources = [Source {
        image: "assets/bg/town.png",
        pixels,
        cell_height: 8,
    }];
    pack(&sources, Layer::Bg, reserved)
}

/// `n` tiles of `per` colors each, no color shared.
fn disjoint(n: u16, per: u16) -> Vec<Vec<u16>> {
    (0..n)
        .map(|t| (0..per).map(|i| t * per + i).collect())
        .collect()
}

/// The single problem, which must be a `gbc_palettes` error.
fn over_budget(result: Result<Packing, Vec<ImageProblem>>) -> ImageProblem {
    let problems = result.expect_err("over 8 palettes");
    assert_eq!(problems.len(), 1, "{problems:?}");
    let p = problems.into_iter().next().unwrap();
    assert_eq!(p.limit, ImageLimit::GbcPalettes);
    assert_eq!(p.severity, Severity::Error);
    assert_eq!(p.max, 8);
    p
}

/// Every cell's colors are in the palette of its slot.
fn assert_covers(packing: &Packing, pixels: &Pixels, layer: Layer) {
    let tall = if layer == Layer::Bg { 8 } else { 16 };
    let cells = census(pixels, tall);
    for (i, (cell, slot)) in cells.iter().zip(&packing.cell_palettes[0]).enumerate() {
        let (Some(colors), Some(slot)) = (cell, slot) else {
            assert_eq!(cell.is_none(), slot.is_none(), "cell {i}");
            continue;
        };
        let GbcPalette(palette) = packing.palettes[(slot.get() - packing.first_slot) as usize];
        assert!(colors.iter().all(|c| palette.contains(c)), "cell {i}");
    }
}

#[test]
fn nine_disjoint_four_color_tiles_need_nine_palettes() {
    let p = over_budget(pack_bg(&bg(&disjoint(9, 4)), 0));
    assert_eq!(p.count, 9);
    assert_eq!(p.tiles.len(), 1, "{:?}", p.tiles);
    assert!(p.message.contains("at least 9"), "{}", p.message);
    assert_eq!(p.image.as_deref(), Some("assets/bg/town.png"));
}

#[test]
fn seventeen_two_color_tiles_need_nine_palettes_by_their_colors() {
    let p = over_budget(pack_bg(&bg(&disjoint(17, 2)), 0));
    assert_eq!(p.count, 9);
    assert!(p.message.contains("34 colors"), "{}", p.message);
    assert!(p.message.contains("at least 9"), "{}", p.message);
}

#[test]
fn sixteen_two_color_tiles_pack_into_eight() {
    let pixels = bg(&disjoint(16, 2));
    let packing = pack_bg(&pixels, 0).expect("fits");
    assert_eq!(packing.palettes.len(), 8);
    assert_covers(&packing, &pixels, Layer::Bg);
}

/// Eight full palettes, then a tile joining a color of the first and of
/// the fifth: no single tile is to blame (research §3), and the packer
/// names the bridge, the tile it couldn't place.
#[test]
fn the_bridge_past_eight_full_palettes_is_the_tile_named() {
    let mut tiles = disjoint(8, 4);
    tiles.push(vec![0, 16]);
    let p = over_budget(pack_bg(&bg(&tiles), 0));
    assert_eq!(p.count, 9);
    assert_eq!(p.tiles, [TileAt { x: 8, y: 0 }]);
    assert!(p.message.contains("at least 8"), "{}", p.message);
}

#[test]
fn a_five_color_tile_is_named_by_its_coordinates() {
    let pixels = bg(&[vec![0, 1], vec![2, 3, 4, 5, 6], vec![7]]);
    let problems = pack_bg(&pixels, 0).expect_err("5 colors in a tile");
    assert_eq!(problems.len(), 1, "{problems:?}");
    let p = &problems[0];
    assert_eq!(p.limit, ImageLimit::GbcTileColors);
    assert_eq!((p.count, p.max), (5, 4));
    assert_eq!(p.tiles, [TileAt { x: 1, y: 0 }]);
}

/// An object's index 0 is transparent, so 4 opaque colors are one too many;
/// 3 and transparency fit. The unit is named by its top tile.
#[test]
fn four_opaque_colors_in_an_object_are_named_by_its_top_tile() {
    let unit = |ns: &[u16], clear: bool| -> Vec<Option<Rgb555>> {
        let mut cell: Vec<Option<Rgb555>> = ns.iter().map(|&n| Some(c(n))).collect();
        if clear {
            cell.push(None);
        }
        cell
    };
    let pixels = strip(&[unit(&[0, 1, 2], true), unit(&[3, 4, 5, 6], true)], 16);
    let sources = [Source {
        image: "assets/sprites/hero.png",
        pixels: &pixels,
        cell_height: 16,
    }];
    let problems = pack(&sources, Layer::Obj, 0).expect_err("4 opaque colors");
    assert_eq!(problems.len(), 1, "{problems:?}");
    let p = &problems[0];
    assert_eq!(p.limit, ImageLimit::GbcTileColors);
    assert_eq!((p.count, p.max), (4, 3));
    assert_eq!(p.tiles, [TileAt { x: 1, y: 0 }]);
    assert_eq!(p.image.as_deref(), Some("assets/sprites/hero.png"));
}

#[test]
fn the_same_input_always_gives_the_same_palettes() {
    let red = Rgb555::new(31, 0, 0).unwrap();
    let cells = vec![
        vec![Some(red), Some(Rgb555::BLACK)],
        vec![Some(Rgb555::WHITE)],
        vec![Some(red)],
    ];
    let pixels = strip(&cells, 8);
    let first = pack_bg(&pixels, 0).unwrap();
    assert_eq!(
        first,
        Packing {
            palettes: vec![GbcPalette([
                Rgb555::WHITE,
                red,
                Rgb555::BLACK,
                Rgb555::BLACK
            ])],
            first_slot: 0,
            cell_palettes: vec![vec![slot(0); 3]],
        }
    );
    for _ in 0..10 {
        assert_eq!(pack_bg(&pixels, 0).unwrap(), first);
    }
    let many = bg(&disjoint(16, 2));
    let packed = pack_bg(&many, 0).unwrap();
    for _ in 0..10 {
        assert_eq!(pack_bg(&many, 0).unwrap(), packed);
    }
}

#[test]
fn a_tile_inside_another_costs_no_palette() {
    let mut tiles = disjoint(8, 4);
    tiles.extend((0..8).map(|t| vec![t * 4 + 1, t * 4 + 3]));
    let pixels = bg(&tiles);
    let packing = pack_bg(&pixels, 0).expect("subsets fit");
    assert_eq!(packing.palettes.len(), 8);
    assert_covers(&packing, &pixels, Layer::Bg);
}

/// With two slots, first fit puts {3,4} with {1,2,3} and then can't place
/// {3,9}; the search puts {3,4} with {4,5,6} instead.
#[test]
fn the_search_finds_a_packing_first_fit_misses() {
    let sets = [vec![1, 2, 3], vec![4, 5, 6], vec![3, 4], vec![3, 9]];
    let as_colors: Vec<BTreeSet<Rgb555>> = sets
        .iter()
        .map(|s| s.iter().map(|&n| c(n)).collect())
        .collect();
    assert_eq!(super::first_fit(&as_colors, 4).len(), 3, "first fit alone");
    let pixels = bg(&sets);
    let packing = pack_bg(&pixels, 6).expect("fits in two");
    assert_eq!(packing.palettes.len(), 2);
    assert_covers(&packing, &pixels, Layer::Bg);
}

#[test]
fn reserved_slots_count_against_the_eight() {
    let p = over_budget(pack_bg(&bg(&disjoint(16, 2)), 1));
    assert_eq!(p.count, 9);
    assert!(p.message.contains("1 assigned"), "{}", p.message);
}

/// All sheets share the 8 OBJ palettes; index 0 is never drawn and holds
/// white, and a transparent unit has no slot.
#[test]
fn sheets_share_the_obj_palettes() {
    let (r, g, b) = (c(1), c(2), c(3));
    let hero = strip(&[vec![Some(r), Some(g), Some(b), None]], 16);
    let coin = strip(&[vec![Some(r), Some(g), None], vec![None]], 16);
    let sources = [
        Source {
            image: "assets/sprites/hero.png",
            pixels: &hero,
            cell_height: 16,
        },
        Source {
            image: "assets/sprites/coin.png",
            pixels: &coin,
            cell_height: 16,
        },
    ];
    let packing = pack(&sources, Layer::Obj, 0).expect("one palette");
    assert_eq!(packing.palettes.len(), 1);
    assert_eq!(packing.palettes[0].0[0], Rgb555::WHITE);
    assert_eq!(
        packing.palettes[0].0[1..].iter().collect::<BTreeSet<_>>(),
        BTreeSet::from([&r, &g, &b])
    );
    assert_eq!(
        packing.cell_palettes,
        vec![vec![slot(0)], vec![slot(0), None]]
    );
}

/// Sprite bound: 27 opaque colors need at least ⌈27 / 3⌉ = 9 palettes.
#[test]
fn nine_three_color_objects_across_sheets_overflow_with_the_sprite_bound() {
    let cells = |from: u16, n: u16| -> Pixels {
        let units: Vec<Vec<Option<Rgb555>>> = (from..from + n)
            .map(|u| (0..3).map(|i| Some(c(u * 3 + i))).collect())
            .collect();
        strip(&units, 16)
    };
    let (hero, coin) = (cells(0, 5), cells(5, 4));
    let sources = [
        Source {
            image: "assets/sprites/hero.png",
            pixels: &hero,
            cell_height: 16,
        },
        Source {
            image: "assets/sprites/coin.png",
            pixels: &coin,
            cell_height: 16,
        },
    ];
    let p = over_budget(pack(&sources, Layer::Obj, 0));
    assert_eq!(p.count, 9);
    assert_eq!(p.tiles.len(), 1);
    assert!(p.message.contains("27 colors"), "{}", p.message);
    assert!(p.message.contains("at least 9"), "{}", p.message);
}

/// A background carries its own palettes; a sheet's are the shared OBJ
/// palettes, so its GBC version lists only each unit's slot.
#[test]
fn a_packing_fills_the_protocols_gbc_image() {
    let pixels = bg(&[vec![0, 1], vec![2]]);
    let packing = pack_bg(&pixels, 0).unwrap();
    let town = packing.gbc_image(Layer::Bg);
    assert_eq!(town.art, ImageArt::Color);
    assert_eq!(town.palettes, packing.palettes);
    assert_eq!(packing.cell_palettes[0], [slot(0), slot(0)]);

    let hero = strip(&[vec![Some(c(1)), None], vec![None]], 16);
    let sources = [Source {
        image: "assets/sprites/hero.png",
        pixels: &hero,
        cell_height: 16,
    }];
    let packing = pack(&sources, Layer::Obj, 0).unwrap();
    let image = packing.gbc_image(Layer::Obj);
    assert!(image.palettes.is_empty());
    assert_eq!(packing.cell_palettes[0], [slot(0), None]);
}

fn slot(n: u8) -> Option<PaletteSlot> {
    PaletteSlot::new(n)
}

/// Assigned palettes take the first slots (`GbcImagesMade::obj_palettes`),
/// so a packed palette's slot counts from `reserved`.
#[test]
fn packed_slots_follow_the_reserved_ones() {
    let hero = strip(&[vec![Some(c(1)), None], vec![None]], 16);
    let sources = [Source {
        image: "assets/sprites/hero.png",
        pixels: &hero,
        cell_height: 16,
    }];
    let packing = pack(&sources, Layer::Obj, 2).unwrap();
    assert_eq!(packing.palettes.len(), 1);
    assert_eq!(packing.cell_palettes[0], [slot(2), None]);
}

/// Colors are counted as rgbgfx reads them: two that agree in each
/// channel's top five bits are one.
#[test]
fn the_census_truncates_to_rgb555_and_skips_empty_objects() {
    let mut pixels = strip(&[vec![Some(c(1))], vec![None]], 16);
    pixels.rgba[0] = [0x0F, 0, 0, 0xFF];
    pixels.rgba[1] = [0x08, 0, 0, 0xFF];
    assert_eq!(
        census(&pixels, 16),
        vec![Some(BTreeSet::from([c(1)])), None]
    );
    assert_eq!(census(&pixels, 8).len(), 4);
}

/// A message names at most a few cells past the budget; `tiles` keeps
/// every one.
#[test]
fn the_gbc_palettes_message_caps_its_cell_list() {
    let p = over_budget(pack_bg(&bg(&disjoint(40, 4)), 0));
    let past = p.tiles.len();
    assert!(past > 30, "{past}");
    let named = p.message.matches(" #").count();
    assert!(named <= 4 * 8, "{named} colors named in {}", p.message);
    let more = format!("and {} more", past - 8);
    assert!(p.message.contains(&more), "{more:?} in {}", p.message);
}
