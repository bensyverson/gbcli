//! Mutated text and random bytes: parsing never panics, and whatever
//! parses writes canonical text that draws the same picture. Seeded, so a
//! failure reproduces.

use gbcli_pixels::{EXAMPLE, Layout, Palettes, Raster, encode_png, parse_text, read, to_text};

const MEADOW: &str = "\
format gbcli-pixels 1
size 24 16
color 0 dmg 0
color 1 dmg 1
color 2 #808080
color k keep
color . clear
fill .

tile sk
11111111
11111111
11112111
11111111
11111111
1k111111
11111111
11111111

tile cl
00000000
01111110
01222210
01200210
01200210
01222210
01111110
00000000

map 0 0
sk cl sk
cl cl sk

rows 3 2
0k0
k2k
";

fn palettes() -> Palettes {
    Palettes {
        dmg: Some([[0xff; 3], [0xaa; 3], [0x55; 3], [0; 3]]),
        named: vec![],
    }
}

/// A small deterministic generator.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// What a mutation inserts: the format's own punctuation, digits and
/// letters, and characters that trip byte counts and trimming.
const ALPHABET: &[char] = &[
    ' ', '\t', '\n', '\r', '#', '0', '1', '2', '3', '8', '9', 'k', '.', 'x', 's', 'r', 'é',
    '\u{FEFF}', '\u{3000}', 'w', 'o', '\u{0}', '-', 'f', '1',
];

fn mutate(rng: &mut Lcg, base: &str) -> String {
    let mut chars: Vec<char> = base.chars().collect();
    for _ in 0..1 + rng.below(4) {
        let at = rng.below(chars.len() + 1);
        match rng.below(6) {
            0 if at < chars.len() => {
                chars.remove(at);
            }
            1 if at < chars.len() => chars[at] = ALPHABET[rng.below(ALPHABET.len())],
            2 => chars.insert(at, ALPHABET[rng.below(ALPHABET.len())]),
            3 => {
                // Duplicate or drop a whole line.
                let text: String = chars.iter().collect();
                let mut lines: Vec<&str> = text.split('\n').collect();
                let i = rng.below(lines.len());
                if rng.below(2) == 0 {
                    lines.insert(i, lines[i]);
                } else {
                    lines.remove(i);
                }
                chars = lines.join("\n").chars().collect();
            }
            4 => {
                let n = ["99999999999", "1024", "1025", "0", "4294967295", "65536"];
                for c in n[rng.below(n.len())].chars().rev() {
                    chars.insert(at, c);
                }
            }
            _ => {}
        }
    }
    chars.into_iter().collect()
}

fn pixels_of(text: &str) -> Option<Raster> {
    parse_text(text).ok()?.raster(&palettes()).ok()
}

#[test]
fn mutated_text_never_panics_and_its_canonical_text_draws_the_same_picture() {
    let mut rng = Lcg(0x5eed_1515);
    let mut parsed = 0;
    for base in [EXAMPLE, MEADOW] {
        for _ in 0..4000 {
            let text = mutate(&mut rng, base);
            let result = std::panic::catch_unwind(|| parse_text(&text));
            let Ok(result) = result else {
                panic!("parse_text panicked on:\n{text:?}");
            };
            let Ok(picture) = result else { continue };
            parsed += 1;
            let Ok(want) = picture.raster(&palettes()) else {
                continue;
            };
            let layouts = [
                Layout::Rows { frame: None },
                Layout::Rows {
                    frame: Some((16, 16)),
                },
                Layout::Tiles,
            ];
            for layout in layouts {
                let Ok(canon) = to_text(&picture, layout) else {
                    continue;
                };
                let again = pixels_of(&canon).unwrap_or_else(|| {
                    panic!("canonical text doesn't parse:\n{canon}\nfrom\n{text}")
                });
                assert_eq!(again, want, "{layout:?} changed the picture:\n{canon}");
                // text -> png -> text -> png is the same picture too.
                let png = encode_png(&want);
                let back = read(&png)
                    .unwrap()
                    .picture
                    .raster(&Palettes::default())
                    .unwrap();
                assert_eq!(back.normalized(), want.normalized());
            }
        }
    }
    assert!(
        parsed > 100,
        "too few mutants parsed ({parsed}) to mean anything"
    );
}

#[test]
fn random_bytes_never_panic_read() {
    let mut rng = Lcg(42);
    for _ in 0..3000 {
        let n = rng.below(200);
        let mut bytes: Vec<u8> = (0..n).map(|_| rng.below(256) as u8).collect();
        if rng.below(3) == 0 {
            let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
            png.append(&mut bytes);
            bytes = png;
        }
        let r = std::panic::catch_unwind(|| read(&bytes).is_ok());
        assert!(r.is_ok(), "read panicked on {bytes:?}");
    }
}
