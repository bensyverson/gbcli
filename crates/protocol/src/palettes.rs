//! GBC colors and the project's named palettes (design §12, the GBC images
//! plan, `project/2026-11-07-gbc-images-plan.md`). `assets/palettes.txt`
//! parses into [`NamedPalettes`], and a client that declares its palettes
//! elsewhere (LoadHigh's `.ldh`) sends the same struct, so the file and the
//! wire can't disagree about what a palette is.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

mod rules;

pub use rules::{FaultAt, PaletteFault};

/// A color as the GBC stores it: five bits each of red, green and blue in
/// one little-endian word, red in bits 0-4, green in 5-9 and blue in 10-14,
/// bit 15 clear. It is exactly what a program writes to BCPD or OCPD, and
/// what `dw` emits, so on the wire it is that word, a number from 0 to
/// 32767, not a `#rrggbb` string, which would claim precision the hardware
/// doesn't have.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u16", into = "u16")]
pub struct Rgb555(u16);

impl Rgb555 {
    /// The brightest white, $7FFF.
    pub const WHITE: Rgb555 = Rgb555(0x7FFF);
    /// Black, $0000.
    pub const BLACK: Rgb555 = Rgb555(0);

    /// From five-bit channels; `None` when one is over 31.
    pub fn new(r: u8, g: u8, b: u8) -> Option<Rgb555> {
        if r > 31 || g > 31 || b > 31 {
            return None;
        }
        Some(Rgb555(r as u16 | (g as u16) << 5 | (b as u16) << 10))
    }

    /// An 8-bit-per-channel color reduced as rgbgfx reads a PNG: each
    /// channel's top five bits. Every color gbcli counts, shades or packs
    /// goes through this, so a palette entry and an image pixel written as
    /// one `#rrggbb` are one color.
    pub fn from_rgb888(r: u8, g: u8, b: u8) -> Rgb555 {
        Rgb555((r >> 3) as u16 | ((g >> 3) as u16) << 5 | ((b >> 3) as u16) << 10)
    }

    /// The word for BCPD/OCPD and `dw`.
    pub fn word(self) -> u16 {
        self.0
    }

    /// The five-bit channels, red first.
    pub fn channels(self) -> [u8; 3] {
        [
            (self.0 & 0x1F) as u8,
            (self.0 >> 5 & 0x1F) as u8,
            (self.0 >> 10 & 0x1F) as u8,
        ]
    }

    /// As SameBoy draws it with color correction off, which is how gbcli
    /// runs it: each channel `(x << 3) | (x >> 2)` (research §4). A preview
    /// drawn with this matches a screenshot pixel for pixel.
    pub fn to_rgb888(self) -> [u8; 3] {
        self.channels().map(|x| x << 3 | x >> 2)
    }
}

/// A word with bit 15 set isn't a GBC color.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NotRgb555(pub u16);

impl fmt::Display for NotRgb555 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} isn't an RGB555 color: it is 0 to 32767 (bit 15 clear)",
            self.0
        )
    }
}

impl TryFrom<u16> for Rgb555 {
    type Error = NotRgb555;

    fn try_from(word: u16) -> Result<Rgb555, NotRgb555> {
        if word > 0x7FFF {
            Err(NotRgb555(word))
        } else {
            Ok(Rgb555(word))
        }
    }
}

impl From<Rgb555> for u16 {
    fn from(c: Rgb555) -> u16 {
        c.0
    }
}

/// Four GBC colors, index 0 first: one BG or OBJ palette, 8 bytes in
/// palette RAM. For indexed art, index 0 is the DMG's lightest shade; for
/// an object, index 0 is transparent, so its color is never drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GbcPalette(pub [Rgb555; 4]);

/// One palette of `assets/palettes.txt`, by name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NamedPalette {
    /// The name the file gives it (`forest`), which names its labels
    /// (`PaletteForest`, `PALETTE_FOREST`); never `dmg`, which is
    /// [`NamedPalettes::dmg`].
    pub name: String,
    pub colors: GbcPalette,
}

/// The project's named palettes and which images they color: what
/// `assets/palettes.txt` parses into, and what a client that declares its
/// palettes itself sends instead of the file.
///
/// Every palette is data for the program to load, whether or not an image
/// uses it. An image with an assignment is **indexed art**: four-shade art
/// whose tiles serve both machines, colored on the GBC by its palette.
///
/// One rule set ([`NamedPalettes::check`]) holds for the file and the
/// wire: a struct that breaks it doesn't deserialize, and the file's
/// parser reports the same faults at their lines.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "rules::Unchecked")]
pub struct NamedPalettes {
    /// The reserved `dmg` line: the colors that mean DMG shades 0-3 in the
    /// art, the first rule of the shade cascade (`ShadeRule::Declared`).
    /// It is not a GBC palette and can't be assigned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dmg: Option<GbcPalette>,
    /// In file order, `dmg` aside: a palette's position is its index
    /// (`PALETTE_FOREST`). Names are unique.
    #[serde(default)]
    pub palettes: Vec<NamedPalette>,
    /// Image → palette name. The image is its path under `assets/`, spelled
    /// as gbcli finds it (`bg/town.png`: `bg/`, `sprites/` or `tiles/`,
    /// then the file); the name is one of `palettes`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub assignments: BTreeMap<String, String>,
}

impl NamedPalettes {
    /// The palette called `name`, with its index.
    pub fn get(&self, name: &str) -> Option<(usize, &NamedPalette)> {
        self.palettes
            .iter()
            .enumerate()
            .find(|(_, p)| p.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #88cc44 is the color `#8c4` names (plan revision 2): each channel's
    /// top five bits, 17, 25 and 8.
    #[test]
    fn a_color_keeps_each_channels_top_five_bits_red_lowest() {
        let c = Rgb555::from_rgb888(0x88, 0xCC, 0x44);
        assert_eq!(c.channels(), [17, 25, 8]);
        assert_eq!(c.word(), 17 | 25 << 5 | 8 << 10);
        assert_eq!(Rgb555::new(17, 25, 8), Some(c));
        assert_eq!(Rgb555::from_rgb888(0xFF, 0xFF, 0xFF), Rgb555::WHITE);
        assert_eq!(Rgb555::from_rgb888(0x07, 0x07, 0x07), Rgb555::BLACK);
        assert_eq!(Rgb555::new(32, 0, 0), None);
    }

    /// SameBoy's channel scaling with no color correction.
    #[test]
    fn a_color_draws_as_sameboy_scales_it() {
        assert_eq!(Rgb555::WHITE.to_rgb888(), [0xFF; 3]);
        assert_eq!(Rgb555::BLACK.to_rgb888(), [0; 3]);
        assert_eq!(
            Rgb555::new(17, 25, 8).unwrap().to_rgb888(),
            [0x8C, 0xCE, 0x42]
        );
    }

    #[test]
    fn a_color_is_its_word_on_the_wire_and_bit_15_is_refused() {
        let c = Rgb555::new(31, 0, 0).unwrap();
        assert_eq!(serde_json::to_string(&c).unwrap(), "31");
        assert_eq!(
            serde_json::from_str::<Rgb555>("32767").unwrap(),
            Rgb555::WHITE
        );
        let e = serde_json::from_str::<Rgb555>("32768").unwrap_err();
        assert!(e.to_string().contains("isn't an RGB555 color"), "{e}");
        assert!(serde_json::from_str::<Rgb555>("-1").is_err());
        assert!(serde_json::from_str::<Rgb555>("\"#ffffff\"").is_err());
    }

    #[test]
    fn named_palettes_round_trip_and_drop_what_they_lack() {
        let forest = GbcPalette([
            Rgb555::WHITE,
            Rgb555::from_rgb888(0x88, 0xCC, 0x44),
            Rgb555::from_rgb888(0x33, 0x66, 0x33),
            Rgb555::from_rgb888(0x00, 0x22, 0x11),
        ]);
        let named = NamedPalettes {
            dmg: Some(GbcPalette([
                Rgb555::WHITE,
                Rgb555::new(21, 21, 21).unwrap(),
                Rgb555::new(10, 10, 10).unwrap(),
                Rgb555::BLACK,
            ])),
            palettes: vec![NamedPalette {
                name: "forest".into(),
                colors: forest,
            }],
            assignments: BTreeMap::from([("bg/town.png".into(), "forest".into())]),
        };
        let json = serde_json::to_string(&named).unwrap();
        assert_eq!(
            json,
            r#"{"dmg":[32767,22197,10570,0],"palettes":[{"name":"forest","colors":[32767,9009,6534,2176]}],"assignments":{"bg/town.png":"forest"}}"#
        );
        assert_eq!(serde_json::from_str::<NamedPalettes>(&json).unwrap(), named);
        assert_eq!(named.get("forest").map(|(i, _)| i), Some(0));
        assert_eq!(named.get("ember"), None);

        let empty = NamedPalettes::default();
        assert_eq!(serde_json::to_string(&empty).unwrap(), r#"{"palettes":[]}"#);
        assert_eq!(serde_json::from_str::<NamedPalettes>("{}").unwrap(), empty);
    }

    /// A client's struct is checked as strictly as a request.
    #[test]
    fn a_palette_of_three_colors_or_an_unknown_field_is_refused() {
        let three = r#"{"palettes":[{"name":"a","colors":[0,1,2]}]}"#;
        assert!(serde_json::from_str::<NamedPalettes>(three).is_err());
        let stranger = r#"{"palettes":[],"assigned":{}}"#;
        assert!(serde_json::from_str::<NamedPalettes>(stranger).is_err());
    }
}
