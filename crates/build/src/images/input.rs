//! What a conversion reads: its images and their palettes, from a
//! project's `assets/` as `build` finds them, or from an `assets` manifest,
//! which names each image as a project would hold it, so both convert by
//! one convention (labels, kinds, frame sizes and assignments alike).

use std::path::{Path, PathBuf};

use gbcli_protocol::{AssetManifest, ImageProblem, NamedPalettes};

use super::find::{self, KINDS, Source};
use super::palettes;
use crate::project::BuildError;

/// Where an input's images were named, which decides the remedy for one
/// that can't be read.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Origin {
    Project,
    Manifest,
}

/// The images to convert and the palettes they are converted with.
pub(crate) struct Input {
    pub origin: Origin,
    pub sources: Vec<Source>,
    pub named: NamedPalettes,
    /// Where the palettes came from, as problems name it.
    pub palette_shown: String,
    /// What was found before any image was read: the old palette file's
    /// warning, or a bad palette file, an error that stops the conversion.
    pub problems: Vec<ImageProblem>,
}

/// The project's images and `assets/palettes.txt`.
pub(crate) fn project(
    project: &Path,
    shown: &dyn Fn(&Path) -> String,
) -> Result<Input, BuildError> {
    let sources = find::sources(project, shown)?;
    let palette_shown = shown(&project.join(palettes::PATH));
    let mut problems: Vec<ImageProblem> = palettes::old_file(project, shown).into_iter().collect();
    let names: Vec<String> = sources.iter().map(|s| s.name.clone()).collect();
    let named = match palettes::read(project, &palette_shown, &names) {
        Ok(named) => named.unwrap_or_default(),
        Err(p) => {
            problems.push(p);
            NamedPalettes::default()
        }
    };
    Ok(Input {
        origin: Origin::Project,
        sources,
        named,
        palette_shown,
        problems,
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
    Ok(Input {
        origin: Origin::Manifest,
        sources,
        named: m.palettes.clone(),
        palette_shown: "the manifest's palettes".into(),
        problems: Vec::new(),
    })
}
