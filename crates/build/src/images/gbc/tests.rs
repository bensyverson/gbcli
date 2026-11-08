//! The GBC agreement check and the joint tiles, on data written by hand.

use gbcli_protocol::{GbcPalette, ImageLimit, PaletteSlot, Rgb555, TileAt};

use super::{Grid, Palettes, Raw, check, tile};
use crate::images::agree;
use crate::images::pack::Layer;
use crate::images::pixels::{Pixels, Rgba};

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

/// Two BG palettes that both hold red, at different indices.
fn palettes() -> [GbcPalette; 2] {
    [
        GbcPalette([Rgb555::WHITE, red(), blue(), Rgb555::BLACK]),
        GbcPalette([red(), Rgb555::BLACK, Rgb555::BLACK, Rgb555::BLACK]),
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
    agree::encode(&[index; 64])
}

const GRID: Grid = Grid {
    width: 2,
    height: 1,
};

/// rgbgfx drew red from palette 1 and blue from palette 0: other choices
/// than gbcli's packing, but the same colors, so they agree.
#[test]
fn a_conversion_drawing_each_color_agrees_whatever_its_palette() {
    let pal = palettes();
    let p = Palettes {
        palettes: &pal,
        first_slot: 0,
        layer: Layer::Bg,
    };
    let raw = Raw {
        tiles: [solid(0), solid(2)].concat(),
        attrs: vec![1, 0],
    };
    assert!(check("town.png", &town(), &p, GRID, &raw).is_ok());
    // Red kept in gbcli's slot 0, where it is index 1: re-indexed.
    let slot = PaletteSlot::new(0).unwrap();
    let at = TileAt { x: 0, y: 0 };
    assert_eq!(tile(&town(), &p, GRID, &raw, at, slot), Ok(solid(1)));
    // Blue was drawn from slot 0: rgbgfx's bytes as written.
    let at = TileAt { x: 1, y: 0 };
    assert_eq!(tile(&town(), &p, GRID, &raw, at, slot), Ok(solid(2)));
}

/// A conversion that draws another color than the image fails the check,
/// naming the tile: here blue drawn as black.
#[test]
fn a_conversion_drawing_another_color_fails_naming_the_tile() {
    let pal = palettes();
    let p = Palettes {
        palettes: &pal,
        first_slot: 0,
        layer: Layer::Bg,
    };
    let raw = Raw {
        tiles: [solid(1), solid(3)].concat(),
        attrs: vec![0, 0],
    };
    let problem = check("town.png", &town(), &p, GRID, &raw).unwrap_err();
    assert_eq!(problem.limit, ImageLimit::RgbgfxAgrees);
    assert_eq!(problem.tiles, [TileAt { x: 1, y: 0 }]);
    // A palette number past the ones given is a disagreement too.
    let raw = Raw {
        tiles: [solid(1), solid(2)].concat(),
        attrs: vec![0, 5],
    };
    let problem = check("town.png", &town(), &p, GRID, &raw).unwrap_err();
    assert_eq!(problem.tiles, [TileAt { x: 1, y: 0 }]);
}

/// An object's index 0 is transparent: drawn opaque, or an opaque pixel
/// drawn as index 0, disagrees.
#[test]
fn an_objects_index_0_is_transparent() {
    let pal = [GbcPalette([Rgb555::WHITE, red(), blue(), Rgb555::BLACK])];
    let p = Palettes {
        palettes: &pal,
        first_slot: 3,
        layer: Layer::Obj,
    };
    // One 8x16 unit: the top tile transparent, the bottom red.
    let rgba = (0..128)
        .map(|i| if i < 64 { [0; 4] } else { rgba(red()) })
        .collect();
    let bat = Pixels {
        width: 8,
        height: 16,
        rgba,
    };
    let grid = Grid {
        width: 1,
        height: 2,
    };
    let good = Raw {
        tiles: [solid(0), solid(1)].concat(),
        attrs: vec![0, 0],
    };
    assert!(check("bat.png", &bat, &p, grid, &good).is_ok());
    let slot = PaletteSlot::new(3).unwrap();
    let bottom = TileAt { x: 0, y: 1 };
    assert_eq!(tile(&bat, &p, grid, &good, bottom, slot), Ok(solid(1)));
    let bad = Raw {
        tiles: [solid(0), solid(0)].concat(),
        attrs: vec![0, 0],
    };
    let problem = check("bat.png", &bat, &p, grid, &bad).unwrap_err();
    assert_eq!(problem.tiles, [bottom]);
}
