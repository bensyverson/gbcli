//! `op: "assets"` (design §12, leaf 7gGMeZ): images converted as `build`
//! converts them, without assembling or linking, for a client (LoadHigh)
//! that needs each image's tile map and palettes before and apart from a
//! build. The images are a project's `assets/`, or a manifest's.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::images::{ImagesMade, ObjectSize};
use crate::palettes::{NamedPalettes, is_image_path};
use crate::toolchain::{BuildTarget, Diagnostic};

/// `op: "assets"`. Paths are relative to the server's working directory.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetsArgs {
    /// The project whose `assets/` to convert, as `build` reads it; default
    /// the server's working directory. Not with `manifest`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dir: Option<String>,
    /// The images one by one, in place of a project.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest: Option<AssetManifest>,
    /// The machine the images are for, as `build`'s; default the DMG.
    #[serde(default, skip_serializing_if = "BuildTarget::is_dmg")]
    pub target: BuildTarget,
    /// The directory to keep what the conversion writes (each image's data
    /// and previews, laid out as a build's `build/images/`; no
    /// `images.asm` or `images.inc`, which only a build assembles). Without
    /// it nothing is kept, and the results name no previews.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keep: Option<String>,
    /// The glyphs to return from each font, by its path under `assets/`
    /// (`fonts/talk.yaff`): each a character or a glyph's name, any number
    /// of them. A font not named here returns every glyph it has.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub glyphs: BTreeMap<String, Vec<String>>,
}

/// Images to convert without a project, and the palettes and object size
/// a project would hold in `assets/palettes.txt`. On read, each image is
/// named once, as a project names it, and every assignment names one of
/// them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "UncheckedManifest")]
pub struct AssetManifest {
    pub images: Vec<ManifestImage>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub palettes: NamedPalettes,
    /// Every sheet's object size, as the `objects` line of a project's
    /// `assets/palettes.txt` sets it (LCDC bit 2 is one size for the whole
    /// screen); unset, each sheet's frame height decides (`ObjectSize`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objects: Option<ObjectSize>,
    /// Fonts, each named as a project holds it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fonts: Vec<ManifestFont>,
}

/// One font of a manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestFont {
    /// Its path under `assets/` in a project (`fonts/talk.yaff`): it sets
    /// the font's labels, and the name a request's `glyphs` uses.
    pub font: String,
    /// The `.yaff` file to read.
    pub file: String,
}

/// One image of a manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestImage {
    /// Its path under `assets/` in a project (`bg/town.png`,
    /// `sprites/hero.16x16.png`, `tiles/font.png`): it sets the image's
    /// kind, its labels, a sheet's frame size, and the name `palettes`
    /// assigns it by.
    pub image: String,
    /// The PNG to read.
    pub file: String,
    /// Its DMG version, which a project keeps beside it as
    /// `<name>.dmg.png`: read for both targets. A manifest reads one only
    /// from here, never from a file beside `file`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dmg: Option<String>,
}

/// What `assets` converted: the images as a build of the same target
/// records them (`BuildMade.images`), and rgbgfx's diagnostics.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetsMade {
    pub target: BuildTarget,
    pub images: ImagesMade,
    pub diagnostics: Vec<Diagnostic>,
}

fn is_default(p: &NamedPalettes) -> bool {
    *p == NamedPalettes::default()
}

/// [`AssetManifest`] as sent, before its images are checked.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UncheckedManifest {
    images: Vec<ManifestImage>,
    #[serde(default)]
    palettes: NamedPalettes,
    #[serde(default)]
    objects: Option<ObjectSize>,
    #[serde(default)]
    fonts: Vec<ManifestFont>,
}

impl TryFrom<UncheckedManifest> for AssetManifest {
    type Error = String;

    fn try_from(u: UncheckedManifest) -> Result<AssetManifest, String> {
        let manifest = AssetManifest {
            images: u.images,
            palettes: u.palettes,
            objects: u.objects,
            fonts: u.fonts,
        };
        manifest.check()?;
        Ok(manifest)
    }
}

impl AssetManifest {
    /// The first image or assignment that a project couldn't hold, as a
    /// message: an image not spelled `bg/`, `sprites/` or `tiles/`
    /// `NAME.png`, a DMG version given as an image, an image listed twice,
    /// or an assignment to an image the manifest doesn't list.
    pub fn check(&self) -> Result<(), String> {
        let mut seen = BTreeSet::new();
        for i in &self.images {
            if !is_image_path(&i.image) {
                return Err(format!(
                    "image `{}` isn't a path under assets/: it is bg/, sprites/ or tiles/, then \
                     a file NAME.png or NAME.pixels",
                    i.image
                ));
            }
            let stem = i
                .image
                .rsplit_once('.')
                .map_or(i.image.as_str(), |(s, _)| s);
            if stem
                .rsplit_once('.')
                .is_some_and(|(_, s)| s.eq_ignore_ascii_case("dmg"))
            {
                return Err(format!(
                    "image `{}` is named as a DMG version; give it as the `dmg` of its color \
                     image instead",
                    i.image
                ));
            }
            if !seen.insert(i.image.as_str()) {
                return Err(format!("image `{}` is listed twice", i.image));
            }
        }
        let mut fonts = BTreeSet::new();
        for f in &self.fonts {
            if !is_font_path(&f.font) {
                return Err(format!(
                    "font `{}` isn't a path under assets/: it is fonts/NAME.yaff",
                    f.font
                ));
            }
            if !fonts.insert(f.font.as_str()) {
                return Err(format!("font `{}` is listed twice", f.font));
            }
        }
        if let Some(image) = self
            .palettes
            .assignments
            .keys()
            .find(|a| !seen.contains(a.as_str()))
        {
            return Err(format!(
                "palettes assign a palette to `{image}`, which the manifest doesn't list"
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;

/// `fonts/NAME.yaff` (any case of `yaff`): one file in `fonts/`, as a
/// build finds a project's fonts.
pub fn is_font_path(path: &str) -> bool {
    let Some(file) = path.strip_prefix("fonts/") else {
        return false;
    };
    let font = file
        .rsplit_once('.')
        .is_some_and(|(stem, ext)| !stem.is_empty() && ext.eq_ignore_ascii_case("yaff"));
    font && !file.contains(['/', '\\'])
}
