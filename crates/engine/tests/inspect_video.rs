//! Tiles, tilemaps, OAM and CGB palettes as data.

mod common;

use common::*;
use gbcli_core::{Core, Region};
use gbcli_engine::inspect::video::{
    CgbObjectAttributes, MapArea, ObjectHeight, PaletteSet, TileAddressing, cgb_palettes, oam,
    object_attributes, object_height, rgb555, tile, tilemap,
};

/// A tile whose row y has color y % 4 in every pixel except pixel 0, which is 3.
fn striped_tile() -> [u8; 16] {
    let mut bytes = [0u8; 16];
    for y in 0..8 {
        let color = y % 4;
        let lo = if color & 1 != 0 { 0xFF } else { 0x80 };
        let hi = if color & 2 != 0 { 0xFF } else { 0x80 };
        bytes[2 * y] = lo;
        bytes[2 * y + 1] = hi;
    }
    bytes
}

#[test]
fn a_tile_decodes_to_color_indices() {
    let mut core = quick_dmg(BUSY);
    core.poke(Region::Vram, 5 * 16, &striped_tile()).unwrap();
    let t = tile(&core, 0, 5).unwrap();
    for y in 0..8 {
        assert_eq!(t.pixels[y][0], 3, "row {y} pixel 0");
        assert!(
            t.pixels[y][1..].iter().all(|&c| c as usize == y % 4),
            "row {y}"
        );
    }
    assert!(
        tile(&core, 0, 384).is_err(),
        "VRAM holds 384 tiles per bank"
    );
    assert!(tile(&core, 1, 0).is_err(), "a DMG has one VRAM bank");
}

#[test]
fn a_tilemap_resolves_tile_ids_under_both_addressings() {
    let mut core = quick_dmg(BUSY);
    core.poke(Region::Vram, 0x1C00, &[0x00, 0x7F, 0x80, 0xFF])
        .unwrap(); // $9C00..
    let unsigned = tilemap(&core, MapArea::Map9C00, TileAddressing::Unsigned8000).unwrap();
    let ids: Vec<u16> = unsigned.entries[..4].iter().map(|e| e.tile_index).collect();
    assert_eq!(ids, [0, 127, 128, 255]);
    let signed = tilemap(&core, MapArea::Map9C00, TileAddressing::Signed8800).unwrap();
    let ids: Vec<u16> = signed.entries[..4].iter().map(|e| e.tile_index).collect();
    assert_eq!(ids, [256, 383, 128, 255]);
    assert_eq!(signed.entries.len(), 32 * 32);
    assert_eq!(signed.entries[1].tile_id, 0x7F);
    assert!(
        signed.entries[0].attributes.is_none(),
        "no attributes on a DMG"
    );
}

#[test]
fn cgb_tilemap_entries_carry_their_attributes() {
    let mut core = booted_cgb(BUSY);
    core.poke(Region::Vram, 0x2000 + 0x1800, &[0b1110_1101])
        .unwrap(); // bank 1, $9800
    let map = tilemap(&core, MapArea::Map9800, TileAddressing::Unsigned8000).unwrap();
    let a = map.entries[0].attributes.unwrap();
    assert!(a.priority && a.y_flip && a.x_flip);
    assert_eq!((a.bank, a.palette), (1, 5));
}

#[test]
fn oam_entries_decode_position_flags_and_the_line_limit() {
    let mut core = quick_dmg(BUSY);
    let mut table = [0u8; 160];
    // Eleven objects on line 0 (y = 16): the eleventh is dropped there.
    for i in 0..11 {
        table[4 * i..4 * i + 4].copy_from_slice(&[16, 8 + 8 * i as u8, i as u8, 0]);
    }
    table[4 * 11..4 * 12].copy_from_slice(&[0, 50, 7, 0b1111_0000]); // y = 0: off-screen
    core.poke(Region::Oam, 0, &table).unwrap();
    let entries = oam(&core, ObjectHeight::Eight).unwrap();
    assert_eq!(entries.len(), 40);
    let first = entries[0];
    assert_eq!((first.screen_x, first.screen_y), (0, 0));
    assert!(first.on_screen && !first.dropped_by_line_limit);
    assert!(entries[10].dropped_by_line_limit && !entries[9].dropped_by_line_limit);
    let hidden = entries[11];
    assert!(!hidden.on_screen);
    assert!(hidden.behind_background && hidden.y_flip && hidden.x_flip);
    assert_eq!((hidden.dmg_palette, hidden.tile), (1, 7));
}

#[test]
fn cgb_palettes_decode_rgb555_colors() {
    let mut core = booted_cgb(BUSY);
    core.poke(Region::BgPalette, 8, &[0x1F, 0x7C]).unwrap(); // palette 1 color 0
    let palettes = cgb_palettes(&core, PaletteSet::Background).unwrap();
    assert_eq!(palettes[1].colors[0], 0x7C1F);
    assert_eq!(rgb555(0x7C1F), (31, 0, 31));
    assert!(cgb_palettes(&quick_dmg(BUSY), PaletteSet::Background).is_err());
}

#[test]
fn tall_objects_cover_sixteen_lines_and_the_height_comes_from_lcdc() {
    let mut core = quick_dmg(BUSY);
    core.bus_write(0xFF40, 0x04).unwrap(); // LCD off, 8x16 objects
    assert_eq!(object_height(&core), Ok(ObjectHeight::Sixteen));
    core.poke(Region::Oam, 0, &[2, 8, 0, 0]).unwrap(); // y = 2: lines -14..=1
    assert!(oam(&core, ObjectHeight::Sixteen).unwrap()[0].on_screen);
    assert!(!oam(&core, ObjectHeight::Eight).unwrap()[0].on_screen);
}

#[test]
fn one_oam_entry_reads_as_its_bytes_and_decoded_attributes() {
    let mut core = quick_dmg(BUSY);
    core.poke(Region::Oam, 3 * 4, &[80, 40, 7, 0b1010_1101])
        .unwrap();
    let e = object_attributes(&core, 3).unwrap();
    assert_eq!((e.index, e.y, e.x, e.tile, e.attr), (3, 80, 40, 7, 0xAD));
    assert!(e.priority && !e.y_flip && e.x_flip);
    assert_eq!(e.palette, 0);
    assert_eq!(e.cgb, None, "a DMG has no OAM bank or color palette");
    assert!(
        object_attributes(&core, 40).is_err(),
        "OAM holds 40 entries"
    );
}

#[test]
fn on_a_cgb_in_cgb_mode_an_oam_entry_names_its_vram_bank_and_palette() {
    let mut core = booted_cgb(BUSY);
    core.poke(Region::Oam, 0, &[16, 8, 1, 0b0101_1101]).unwrap();
    let e = object_attributes(&core, 0).unwrap();
    assert!(!e.priority && e.y_flip && !e.x_flip);
    assert_eq!(e.palette, 1, "the DMG palette bit, read as written");
    assert_eq!(
        e.cgb,
        Some(CgbObjectAttributes {
            bank: 1,
            palette: 5
        })
    );
}
