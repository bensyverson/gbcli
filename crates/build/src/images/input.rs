//! What a conversion reads: its images and their palettes, from a
//! project's `assets/` as `build` finds them, or from an `assets` manifest,
//! which names each image as a project would hold it, so both convert by
//! one convention (labels, kinds, frame sizes and assignments alike).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use gbcli_protocol::{AssetManifest, ImageProblem, NamedPalettes, ObjectSize};

use super::find::{self, KINDS, Source};
use super::fonts::{self, FontSource};
use super::palettes;
use crate::project::BuildError;

/// Where an input's images were named, which decides the remedy for one
/// that can't be read.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Origin {
    Project,
    Manifest,
}

/// Where a conversion's palettes came from, which decides how a problem
/// names them and the setting a remedy points at (leaf zqarpL): a
/// project's file has a `dmg` line, a manifest a `palettes.dmg` field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PaletteSource {
    /// A project's `assets/palettes.txt`, as shown.
    File(String),
    /// An `assets` manifest's `palettes`.
    Manifest,
}

impl PaletteSource {
    /// The palettes as a problem names them.
    pub fn shown(&self) -> &str {
        match self {
            PaletteSource::File(shown) => shown,
            PaletteSource::Manifest => "the manifest's palettes",
        }
    }

    /// The setting that declares the shades' colors: "the dmg line of
    /// assets/palettes.txt".
    pub fn dmg(&self) -> String {
        match self {
            PaletteSource::File(shown) => format!("the dmg line of {shown}"),
            PaletteSource::Manifest => {
                "the manifest's palettes.dmg (four RGB555 colors, shades 0-3)".into()
            }
        }
    }

    /// The words saying that setting is missing.
    pub fn no_dmg(&self) -> String {
        match self {
            PaletteSource::File(shown) => format!("no dmg line in {shown}"),
            PaletteSource::Manifest => "no palettes.dmg in the manifest".into(),
        }
    }

    /// The remedy's words for adding that setting.
    pub fn add_dmg(&self) -> String {
        match self {
            PaletteSource::File(shown) => format!("add a dmg line to {shown}"),
            PaletteSource::Manifest => "set the manifest's palettes.dmg".into(),
        }
    }

    /// The setting that sizes every sheet's objects: "the objects line of
    /// assets/palettes.txt".
    pub fn objects(&self) -> String {
        match self {
            PaletteSource::File(shown) => format!("the objects line of {shown}"),
            PaletteSource::Manifest => "the manifest's objects".into(),
        }
    }

    /// The remedy's words for setting every sheet's object size.
    pub fn add_objects(&self) -> String {
        match self {
            PaletteSource::File(shown) => {
                format!("add `objects: 8x8` (or `objects: 8x16`) to {shown}")
            }
            PaletteSource::Manifest => "set the manifest's objects to 8x8 (or 8x16)".into(),
        }
    }

    /// The words saying where an image is assigned `palette`.
    pub fn assigns(&self, palette: &str) -> String {
        match self {
            PaletteSource::File(shown) => format!("{shown} assigns it `{palette}`"),
            PaletteSource::Manifest => {
                format!("the manifest's palettes.assignments give it `{palette}`")
            }
        }
    }
}

/// The images to convert and the palettes they are converted with.
pub(crate) struct Input {
    pub origin: Origin,
    pub sources: Vec<Source>,
    pub named: NamedPalettes,
    /// Where the palettes came from, and the object size with them.
    pub palettes: PaletteSource,
    /// Every sheet's object size, when the project sets one; else each
    /// sheet's frame height decides (`sheet::object_size`).
    pub objects: Option<ObjectSize>,
    /// What was found before any image was read: the old palette file's
    /// warning, or a bad palette file, an error that stops the conversion.
    pub problems: Vec<ImageProblem>,
    /// The fonts (`assets/fonts/*.yaff`, or a manifest's).
    pub fonts: Vec<FontSource>,
    /// An `assets` request's glyphs, by font name; empty for a build.
    pub glyphs: BTreeMap<String, Vec<String>>,
}

/// The project's images and `assets/palettes.txt`.
pub(crate) fn project(
    project: &Path,
    shown: &dyn Fn(&Path) -> String,
) -> Result<Input, BuildError> {
    let sources = find::sources(project, shown)?;
    let fonts = fonts::find(project, shown)?;
    fonts::collisions(&fonts, &sources, shown)?;
    let palette_shown = shown(&project.join(palettes::PATH));
    let mut problems: Vec<ImageProblem> = palettes::old_file(project, shown).into_iter().collect();
    let names: Vec<String> = sources.iter().map(|s| s.name.clone()).collect();
    let settings = match palettes::read(project, &palette_shown, &names) {
        Ok(settings) => settings.unwrap_or_default(),
        Err(p) => {
            problems.push(p);
            palettes::Settings::default()
        }
    };
    Ok(Input {
        origin: Origin::Project,
        sources,
        named: settings.named,
        palettes: PaletteSource::File(palette_shown),
        objects: settings.objects,
        problems,
        fonts,
        glyphs: BTreeMap::new(),
    })
}

/// The manifest's images, each read from its `file` (and its `dmg`),
/// relative to `base`.
pub(crate) fn manifest(
    m: &AssetManifest,
    base: &Path,
    shown: &dyn Fn(&Path) -> String,
) -> Result<Input, BuildError> {
    // A manifest made in Rust never went through the wire's check.
    m.check().map_err(|message| BuildError::Layout {
        message: format!("the manifest: {message}"),
        remedy: "name each image once, as its path under assets/ in a project \
                 (`bg/town.png`, `sprites/hero.16x16.png`, `tiles/font.png`), and assign \
                 palettes only to images it lists"
            .into(),
    })?;
    m.palettes.check().map_err(|fault| BuildError::Layout {
        message: format!("the manifest's palettes: {fault}"),
        remedy: "fix the palettes as `gbcli schema` gives them for `manifest`: assign an image \
                 only to a palette the manifest defines"
            .into(),
    })?;
    let mut sources = Vec::with_capacity(m.images.len());
    for i in &m.images {
        let dir = i.image.split_once('/').map_or("", |(d, _)| d);
        let kind = KINDS
            .into_iter()
            .find(|k| k.dir() == dir)
            .expect("the manifest's check names every image's kind");
        let mut s = find::source(kind, PathBuf::from(&i.image), shown)?;
        s.path = base.join(&i.file);
        s.dmg = i.dmg.as_ref().map(|d| base.join(d));
        sources.push(s);
    }
    find::collisions(&sources, shown)?;
    let fonts: Vec<FontSource> = m
        .fonts
        .iter()
        .map(|f| fonts::source(base.join(&f.file), f.font.clone(), shown))
        .collect::<Result<_, _>>()?;
    fonts::collisions(&fonts, &sources, shown)?;
    Ok(Input {
        origin: Origin::Manifest,
        sources,
        named: m.palettes.clone(),
        palettes: PaletteSource::Manifest,
        objects: m.objects,
        problems: Vec::new(),
        fonts,
        glyphs: BTreeMap::new(),
    })
}
