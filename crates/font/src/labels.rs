//! A 3x5 label font for the pictures gbcli draws about fonts (the guide
//! sheet, a diff): digits, capital letters, `-`, `_` and `+`, enough to
//! name any glyph by its code point or its name.

use crate::key::Key;

/// Pixels a label character advances: 3 wide and a gap.
pub(crate) const ADVANCE: u32 = 4;

/// Each character's 5 rows of 3 bits, bit 2 the leftmost.
fn rows(c: char) -> [u8; 5] {
    match c {
        '0' => [7, 5, 5, 5, 7],
        '1' => [2, 6, 2, 2, 7],
        '2' => [7, 1, 7, 4, 7],
        '3' => [7, 1, 3, 1, 7],
        '4' => [5, 5, 7, 1, 1],
        '5' => [7, 4, 7, 1, 7],
        '6' => [7, 4, 7, 5, 7],
        '7' => [7, 1, 1, 2, 2],
        '8' => [7, 5, 7, 5, 7],
        '9' => [7, 5, 7, 1, 7],
        'A' => [2, 5, 7, 5, 5],
        'B' => [6, 5, 6, 5, 6],
        'C' => [3, 4, 4, 4, 3],
        'D' => [6, 5, 5, 5, 6],
        'E' => [7, 4, 6, 4, 7],
        'F' => [7, 4, 6, 4, 4],
        'G' => [3, 4, 5, 5, 3],
        'H' => [5, 5, 7, 5, 5],
        'I' => [7, 2, 2, 2, 7],
        'J' => [1, 1, 1, 5, 2],
        'K' => [5, 5, 6, 5, 5],
        'L' => [4, 4, 4, 4, 7],
        'M' => [5, 7, 7, 5, 5],
        'N' => [6, 5, 5, 5, 5],
        'O' => [2, 5, 5, 5, 2],
        'P' => [6, 5, 6, 4, 4],
        'Q' => [2, 5, 5, 6, 3],
        'R' => [6, 5, 6, 5, 5],
        'S' => [3, 4, 2, 1, 6],
        'T' => [7, 2, 2, 2, 2],
        'U' => [5, 5, 5, 5, 7],
        'V' => [5, 5, 5, 5, 2],
        'W' => [5, 5, 7, 7, 5],
        'X' => [5, 5, 2, 5, 5],
        'Y' => [5, 5, 2, 2, 2],
        'Z' => [7, 1, 2, 4, 7],
        '-' => [0, 0, 7, 0, 0],
        '_' => [0, 0, 0, 0, 7],
        '+' => [0, 2, 7, 2, 0],
        _ => [0, 0, 0, 0, 0],
    }
}

/// What a picture labels `key` with: a character's code point in hex
/// (`E9`, `1F600`), or a name in capitals.
pub(crate) fn label(key: &Key) -> String {
    match key {
        Key::Char(c) => format!("{:02X}", u32::from(*c)),
        Key::Name(n) => n.to_ascii_uppercase(),
    }
}

/// Calls `ink(x, y)` for every ink pixel of `text` drawn from (0, 0), up
/// to `max` characters.
pub(crate) fn draw(text: &str, max: usize, mut ink: impl FnMut(u32, u32)) {
    for (i, c) in text.chars().take(max).enumerate() {
        for (y, bits) in rows(c).iter().enumerate() {
            for x in 0..3 {
                if bits & (4 >> x) != 0 {
                    ink(i as u32 * ADVANCE + x, y as u32);
                }
            }
        }
    }
}
