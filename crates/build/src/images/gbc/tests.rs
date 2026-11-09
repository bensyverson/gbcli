//! The GBC encoder, on images and palettes written by hand.

use gbcli_protocol::{GbcPalette, ObjectSize, PaletteSlot, Rgb555};

use super::{Palettes, background, sheet};
use crate::images::bg::BgPlan;
use crate::images::encode;
use crate::images::pack::Layer;
use crate::images::pixels::{Pixels, Rgba};
use crate::images::sheet::SheetPlan;

fn red() -> Rgb555 {
    Rgb555::new(31, 0, 0).unwrap()
}

fn blue() -> Rgb555 {
    Rgb555::new(0, 0, 31).unwrap()
}

fn rgba(c: Rgb555) -> Rgba {
    let [r, g, b] = c.to_rgb888();
    [r, g, b, 0xFF]
}

fn slot(n: u8) -> PaletteSlot {
    PaletteSlot::new(n).unwrap()
}

/// Two BG palettes that hold red and blue, in the other order the second.
fn palettes() -> [GbcPalette; 2] {
    [
        GbcPalette([Rgb555::WHITE, red(), blue(), Rgb555::BLACK]),
        GbcPalette([blue(), red(), Rgb555::BLACK, Rgb555::BLACK]),
    ]
}

/// Two 8x8 tiles side by side: solid red, then solid blue.
fn town() -> Pixels {
    let rgba = (0..64 * 2)
        .map(|i| rgba(if i % 16 < 8 { red() } else { blue() }))
        .collect();
    Pixels {
        width: 16,
        height: 8,
        rgba,
    }
}

fn solid(index: u8) -> Vec<u8> {
    encode::encode(&[index; 64])
}

fn plan() -> BgPlan {
    BgPlan {
        warnings: Vec::new(),
        width: 2,
        height: 1,
        tiles: 2,
        unique: vec![vec![0; 64], vec![0; 64]],
        cells: vec![0, 1],
    }
}

/// Each pixel takes its color's index in the slot the packing gave its
/// cell: red is index 1 of palette 0, blue index 2.
#[test]
fn a_background_tile_is_indexed_into_its_cells_palette() {
    let pal = palettes();
    let p = Palettes {
        palettes: &pal,
        first_slot: 0,
        layer: Layer::Bg,
    };
    let joint = background(&plan(), &town(), &p, &[Some(slot(0)), Some(slot(0))]);
    assert_eq!(joint.slots, [slot(0), slot(0)]);
    assert_eq!(joint.tiles, [solid(1), solid(2)].concat());
}

/// The same colors in the other palette's order: the indices follow the
/// palette, so the bytes do too.
#[test]
fn another_slot_orders_the_same_colors_differently() {
    let pal = palettes();
    let p = Palettes {
        palettes: &pal,
        first_slot: 0,
        layer: Layer::Bg,
    };
    let joint = background(&plan(), &town(), &p, &[Some(slot(1)), Some(slot(1))]);
    assert_eq!(joint.tiles, [solid(1), solid(0)].concat());
}

/// An 8x16 unit: its transparency is index 0 of an object palette, and its
/// red the index red holds there; both tiles take the unit's slot.
#[test]
fn an_objects_transparency_is_index_zero_and_both_tiles_take_the_units_slot() {
    let pal = palettes();
    let p = Palettes {
        palettes: &pal,
        first_slot: 3,
        layer: Layer::Obj,
    };
    let rgba = (0..128)
        .map(|i| if i < 64 { [0; 4] } else { rgba(red()) })
        .collect();
    let bat = Pixels {
        width: 8,
        height: 16,
        rgba,
    };
    let s = SheetPlan {
        object: ObjectSize::Tall,
        obp: 0,
        width: 1,
        height: 2,
        frame: (8, 16),
        kept: vec![(0, 0)],
        uses: vec![1],
        frames: Vec::new(),
        units: vec![Some((0, Default::default()))],
        warnings: Vec::new(),
        drawn: vec![vec![1; 128]],
    };
    let joint = sheet(&s, &bat, &p, &[Some(slot(3))]);
    assert_eq!(joint.slots, [slot(3)]);
    // Palette 0 (slot 3): red at index 1, so the top's transparency is 0.
    assert_eq!(joint.tiles, [solid(0), solid(1)].concat());
}
