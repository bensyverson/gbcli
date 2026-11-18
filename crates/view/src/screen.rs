//! A frame's pixels as data, and a fingerprint of them that names the
//! pixels alone: equal pixels give equal fingerprints whatever frame they
//! came from, and the PNG's labels (which name the frame) play no part.

use gbcli_core::PixelBuf;
use gbcli_protocol::{ScreenKind, ShotScreen};

use crate::frame::Frame;

/// The frame's pixels row by row: shades as a digit 0-3 each, colors as
/// six lowercase hex digits (`rrggbb`) each.
pub fn screen(frame: &Frame) -> ShotScreen {
    let width = frame.width.max(1) as usize;
    let (kind, rows) = match &frame.pixels {
        PixelBuf::Shades(s) => (
            ScreenKind::Shades,
            s.chunks(width)
                .map(|row| row.iter().map(|&p| char::from(b'0' + (p & 3))).collect())
                .collect(),
        ),
        PixelBuf::Rgb(rgb) => (
            ScreenKind::Colors,
            rgb.chunks(width)
                .map(|row| {
                    row.iter()
                        .map(|&p| format!("{:06x}", p & 0xFF_FF_FF))
                        .collect()
                })
                .collect(),
        ),
    };
    ShotScreen {
        kind,
        width: frame.width,
        height: frame.height,
        rows,
    }
}

/// The first 16 lowercase hex digits of the SHA-256 of the screen's rows
/// joined by `\n` (no trailing newline). Among screens of one kind, the
/// rows' count and length carry the size, so a screen of another shape
/// can't share its bytes. The kind isn't hashed, so across kinds it can: six
/// shade-0 pixels in a row and one black color pixel are both `000000`.
pub fn fingerprint(screen: &ShotScreen) -> String {
    let mut digest = gbcli_engine::sha256(screen.rows.join("\n").as_bytes()).to_string();
    digest.truncate(16);
    digest
}

#[cfg(test)]
mod tests {
    use gbcli_core::{FrameTag, Ticks, VblankKind};

    use super::*;
    use crate::frame::FrameSource;

    fn frame(number: u64, width: u32, height: u32, pixels: PixelBuf) -> Frame {
        Frame {
            source: FrameSource::Complete(FrameTag {
                number,
                kind: VblankKind::Normal,
                at: Ticks(number * 70_224),
            }),
            width,
            height,
            pixels,
        }
    }

    fn shades(number: u64, width: u32, height: u32, s: &[u8]) -> Frame {
        frame(number, width, height, PixelBuf::Shades(s.to_vec()))
    }

    #[test]
    fn shades_are_a_digit_a_pixel_row_by_row() {
        let screen = screen(&shades(1, 3, 2, &[0, 1, 2, 3, 3, 0]));
        assert_eq!(screen.kind, ScreenKind::Shades);
        assert_eq!((screen.width, screen.height), (3, 2));
        assert_eq!(screen.rows, ["012", "330"]);
    }

    #[test]
    fn colors_are_six_hex_digits_a_pixel_as_the_core_output_them() {
        let f = frame(1, 2, 1, PixelBuf::Rgb(vec![0xFF_84_00, 0x00_0A_FF]));
        let screen = screen(&f);
        assert_eq!(screen.kind, ScreenKind::Colors);
        assert_eq!(screen.rows, ["ff8400000aff"]);
    }

    #[test]
    fn the_fingerprint_is_sha256_of_the_rows_joined_by_newlines() {
        // `printf '0' | shasum -a 256` and `printf '01\n23' | shasum -a 256`.
        assert_eq!(
            fingerprint(&screen(&shades(1, 1, 1, &[0]))),
            "5feceb66ffc86f38"
        );
        assert_eq!(
            fingerprint(&screen(&shades(1, 2, 2, &[0, 1, 2, 3]))),
            "47ca4e045f04b648"
        );
    }

    #[test]
    fn equal_pixels_share_a_fingerprint_whatever_the_frame() {
        let pixels = [0, 1, 2, 3, 0, 1];
        let a = fingerprint(&screen(&shades(4, 3, 2, &pixels)));
        let b = fingerprint(&screen(&shades(90, 3, 2, &pixels)));
        assert_eq!(a, b);
        assert_eq!(a.len(), 16);
    }

    #[test]
    fn one_changed_pixel_or_another_shape_changes_the_fingerprint() {
        let base = fingerprint(&screen(&shades(1, 3, 2, &[0, 1, 2, 3, 0, 1])));
        let one = fingerprint(&screen(&shades(1, 3, 2, &[0, 1, 2, 3, 0, 2])));
        assert_ne!(base, one);
        let tall = fingerprint(&screen(&shades(1, 2, 3, &[0, 1, 2, 3, 0, 1])));
        assert_ne!(base, tall, "same values, another shape");
        let color = |c| fingerprint(&screen(&frame(1, 1, 1, PixelBuf::Rgb(vec![c]))));
        assert_ne!(color(0xFF_84_00), color(0xFF_84_01));
    }

    /// The fingerprint names the rows' bytes, not their kind: a 6x1 crop of
    /// shade 0 and a 1x1 crop of black are both `000000`, so they share
    /// it. Only screens of one kind are told apart by shape (Ben's hash
    /// ruling; shot --help says so).
    #[test]
    fn across_kinds_rows_of_the_same_bytes_share_a_fingerprint() {
        let dmg = screen(&shades(1, 6, 1, &[0; 6]));
        let gbc = screen(&frame(1, 1, 1, PixelBuf::Rgb(vec![0])));
        assert_ne!(dmg, gbc, "different screens");
        assert_eq!(dmg.rows, gbc.rows);
        assert_eq!(fingerprint(&dmg), fingerprint(&gbc));
    }
}
