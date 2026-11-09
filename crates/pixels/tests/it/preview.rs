//! The preview: four times larger, clear in magenta, an 8x8 grid.

use gbcli_pixels::{PREVIEW_SCALE, Raster, decode_png, preview_png};

#[test]
fn a_preview_is_four_times_larger_with_a_line_on_each_tile_edge() {
    let mut rgba = vec![[0x20, 0x40, 0x60, 255]; 16 * 8];
    rgba[0] = [0, 0, 0, 0];
    let p = decode_png(&preview_png(&Raster {
        width: 16,
        height: 8,
        rgba,
    }))
    .unwrap();
    assert_eq!(PREVIEW_SCALE, 4);
    assert_eq!((p.width, p.height), (64, 32));
    // Clear is the views' transparent magenta.
    assert_eq!(p.at(1, 1), [0xFF, 0x00, 0xFF, 255]);
    assert_eq!(p.at(5, 5), [0x20, 0x40, 0x60, 255]);
    // The line between the two tiles, and none at the image's own edges.
    let grid = p.at(32, 5);
    assert_ne!(grid, [0x20, 0x40, 0x60, 255]);
    assert_eq!(p.at(32, 20), grid);
    assert_eq!(p.at(63, 5), [0x20, 0x40, 0x60, 255]);
    assert_eq!(p.at(5, 31), [0x20, 0x40, 0x60, 255]);
}
