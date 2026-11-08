//! The one rule set for [`NamedPalettes`], from `assets/palettes.txt` or
//! from the wire (the reviews of 81cf3d2 and da98a90): names that can
//! become labels, each once, never `dmg`; a `dmg` palette of four colors
//! distinct in RGB555; assignments to a defined palette, never `dmg`, of
//! an image by its canonical path under `assets/`, each image once. What needs the
//! project (does the image exist?) is the build's to check.

use std::collections::BTreeMap;
use std::fmt;

use serde::Deserialize;

use super::{GbcPalette, NamedPalette, NamedPalettes};
use crate::images::ImageKind;

/// The reserved name: the DMG shades, not a GBC palette.
const DMG: &str = "dmg";

/// What a [`NamedPalettes`] breaks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PaletteFault {
    /// `palettes[index]` is named `dmg`, which is [`NamedPalettes::dmg`].
    ReservedName { index: usize },
    /// `palettes[index]`'s name can't become a label: a lowercase letter,
    /// then lowercase letters, digits or `_`.
    BadName { index: usize, name: String },
    /// `palettes[index]` repeats the name of `palettes[first]`.
    Duplicate {
        index: usize,
        first: usize,
        name: String,
    },
    /// `image` is assigned `dmg`, which colors nothing on the GBC.
    AssignedDmg { image: String },
    /// `image` is assigned `name`, which no palette has.
    UnknownPalette { image: String, name: String },
    /// `image` isn't an image's path under `assets/` as gbcli spells it.
    BadImage { image: String },
    /// `dmg`'s color for `shade` is its color for `first`, so the two
    /// can't name two shades (a named palette may repeat a color).
    RepeatedDmgColor { shade: usize, first: usize },
}

/// Where a fault is: a palette by its index, or an assignment by its image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FaultAt<'a> {
    /// The `dmg` palette.
    Dmg,
    Palette(usize),
    Assignment(&'a str),
}

impl PaletteFault {
    pub fn at(&self) -> FaultAt<'_> {
        match self {
            PaletteFault::ReservedName { index }
            | PaletteFault::BadName { index, .. }
            | PaletteFault::Duplicate { index, .. } => FaultAt::Palette(*index),
            PaletteFault::AssignedDmg { image }
            | PaletteFault::UnknownPalette { image, .. }
            | PaletteFault::BadImage { image } => FaultAt::Assignment(image),
            PaletteFault::RepeatedDmgColor { .. } => FaultAt::Dmg,
        }
    }
}

impl fmt::Display for PaletteFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PaletteFault::ReservedName { .. } => write!(
                f,
                "dmg is reserved for the DMG's shades (the `dmg` field), so no GBC palette \
                 can be named it"
            ),
            PaletteFault::BadName { name, .. } => write!(
                f,
                "{name:?} can't name a palette: a lowercase letter, then lowercase letters, \
                 digits or _"
            ),
            PaletteFault::Duplicate { name, .. } => write!(f, "{name} is defined again"),
            PaletteFault::AssignedDmg { image } => write!(
                f,
                "{image} is assigned dmg, but dmg is the DMG's shades, not a GBC palette, so no \
                 image can be assigned it; name a palette of its own"
            ),
            PaletteFault::UnknownPalette { image, name } => {
                write!(
                    f,
                    "{image} is assigned {name}, which is no palette defined here"
                )
            }
            PaletteFault::BadImage { image } => write!(
                f,
                "{image:?} isn't an image's path under assets/: bg/, sprites/ or tiles/, then \
                 the PNG's file name, joined by '/' (`bg/town.png`)"
            ),
            PaletteFault::RepeatedDmgColor { shade, first } => write!(
                f,
                "dmg's shade {shade} is the color of its shade {first} in RGB555, so the two \
                 can't name two shades"
            ),
        }
    }
}

impl NamedPalettes {
    /// Every fault: `dmg`'s, then the palettes' in order, then the
    /// assignments' by image.
    pub fn faults(&self) -> Vec<PaletteFault> {
        let mut faults = Vec::new();
        if let Some(GbcPalette(dmg)) = &self.dmg
            && let Some((shade, first)) = (1..4).find_map(|shade| {
                Some((shade, dmg[..shade].iter().position(|&c| c == dmg[shade])?))
            })
        {
            faults.push(PaletteFault::RepeatedDmgColor { shade, first });
        }
        let mut first: BTreeMap<&str, usize> = BTreeMap::new();
        for (index, p) in self.palettes.iter().enumerate() {
            let name = p.name.as_str();
            if name == DMG {
                faults.push(PaletteFault::ReservedName { index });
            } else if !is_label(name) {
                faults.push(PaletteFault::BadName {
                    index,
                    name: name.into(),
                });
            } else if let Some(&first) = first.get(name) {
                faults.push(PaletteFault::Duplicate {
                    index,
                    first,
                    name: name.into(),
                });
            } else {
                first.insert(name, index);
            }
        }
        for (image, name) in &self.assignments {
            let image = image.clone();
            if !is_image_path(&image) {
                faults.push(PaletteFault::BadImage { image });
            } else if name == DMG {
                faults.push(PaletteFault::AssignedDmg { image });
            } else if !first.contains_key(name.as_str()) {
                faults.push(PaletteFault::UnknownPalette {
                    image,
                    name: name.clone(),
                });
            }
        }
        faults
    }

    /// The first fault, if any.
    pub fn check(&self) -> Result<(), PaletteFault> {
        match self.faults().into_iter().next() {
            Some(fault) => Err(fault),
            None => Ok(()),
        }
    }
}

/// [`NamedPalettes`] as sent, before [`NamedPalettes::check`].
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Unchecked {
    #[serde(default)]
    dmg: Option<GbcPalette>,
    #[serde(default)]
    palettes: Vec<NamedPalette>,
    #[serde(default, deserialize_with = "assigned_once")]
    assignments: BTreeMap<String, String>,
}

/// The assignments as sent, refusing an image named twice (a repeated
/// JSON key), as the file refuses a second assignment: read last-wins, a
/// palette the client sent would silently not be the one used.
fn assigned_once<'de, D>(deserializer: D) -> Result<BTreeMap<String, String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct Once;

    impl<'de> serde::de::Visitor<'de> for Once {
        type Value = BTreeMap<String, String>;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("a map of image to palette name, each image once")
        }

        fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::MapAccess<'de>,
        {
            let mut out = BTreeMap::new();
            while let Some((image, name)) = map.next_entry::<String, String>()? {
                if out.contains_key(&image) {
                    return Err(serde::de::Error::custom(format!(
                        "{image} is assigned a palette again"
                    )));
                }
                out.insert(image, name);
            }
            Ok(out)
        }
    }

    deserializer.deserialize_map(Once)
}

impl TryFrom<Unchecked> for NamedPalettes {
    type Error = PaletteFault;

    fn try_from(u: Unchecked) -> Result<NamedPalettes, PaletteFault> {
        let named = NamedPalettes {
            dmg: u.dmg,
            palettes: u.palettes,
            assignments: u.assignments,
        };
        named.check()?;
        Ok(named)
    }
}

/// Whether `name` can name a GBC palette: a label, never `dmg`.
pub(crate) fn names_a_palette(name: &str) -> bool {
    name != DMG && is_label(name)
}

/// A lowercase letter, then lowercase letters, digits and `_`, so it can
/// become a label (`PaletteForest`, `PALETTE_FOREST`).
fn is_label(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// `<kind dir>/<file>.png` (any case of `png`), as the build names the
/// images it finds: one directory of an [`ImageKind`], one file, `/`
/// between, nothing that steps out or round about.
fn is_image_path(path: &str) -> bool {
    let Some((dir, file)) = path.split_once('/') else {
        return false;
    };
    let kinds = [ImageKind::Bg, ImageKind::Sprites, ImageKind::Tiles];
    let png = file
        .rsplit_once('.')
        .is_some_and(|(stem, ext)| !stem.is_empty() && ext.eq_ignore_ascii_case("png"));
    kinds.iter().any(|k| k.dir() == dir) && png && !file.contains(['/', '\\'])
}
