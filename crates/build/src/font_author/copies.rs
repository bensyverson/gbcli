//! Where a font's working copy lives: `build/font-edit/` of the font's
//! project (the folder holding `assets/` when the font is in
//! `assets/fonts/`, else the font's own folder), which the starter's
//! `.gitignore` already ignores (`/build/`), as `NAME.png` with
//! `NAME.chars`, or `NAME.bdf`. A build refuses while one is newer than
//! its `.yaff` ([`newer_copy`]).

use std::fs;
use std::path::{Path, PathBuf};

use gbcli_font::bdf::BDF_EXTENSION;

use crate::paths::normalize;

/// The working copies' folder under a project.
pub(crate) const COPY_DIR: &str = "build/font-edit";

/// A font's working-copy files, whether or not they exist.
#[derive(Clone, Debug)]
pub(crate) struct Copies {
    pub png: PathBuf,
    pub chars: PathBuf,
    pub bdf: PathBuf,
}

impl Copies {
    /// The ones that exist.
    pub fn existing(&self) -> Vec<PathBuf> {
        [&self.png, &self.chars, &self.bdf]
            .into_iter()
            .filter(|p| p.is_file())
            .cloned()
            .collect()
    }
}

/// The project a font at `yaff` belongs to.
fn project_of(yaff: &Path) -> PathBuf {
    let dir = yaff.parent().unwrap_or(Path::new("")).to_path_buf();
    let named = |p: &Path, n: &str| p.file_name().is_some_and(|f| f == n);
    match dir.parent() {
        Some(assets) if named(&dir, "fonts") && named(assets, "assets") => {
            assets.parent().unwrap_or(Path::new("")).to_path_buf()
        }
        _ => dir,
    }
}

/// The working-copy files of the font at `yaff`.
pub(crate) fn copies(yaff: &Path) -> Copies {
    let yaff = normalize(yaff);
    let dir = project_of(&yaff).join(COPY_DIR);
    let stem = yaff.file_stem().unwrap_or_default();
    let file = |ext: &str| dir.join(stem).with_extension(ext);
    Copies {
        png: file("png"),
        chars: file(gbcli_font::CHARS_EXTENSION),
        bdf: file(BDF_EXTENSION),
    }
}

/// The `.yaff` a working copy at `copy` belongs to: in the project's
/// `assets/fonts/` when that exists there (or that folder does), else
/// beside the project folder's own fonts.
pub(crate) fn font_of(copy: &Path) -> Option<PathBuf> {
    let copy = normalize(copy);
    let dir = copy.parent()?;
    let project = dir.strip_suffix_path(Path::new(COPY_DIR))?;
    let name = Path::new(copy.file_stem()?).with_extension(gbcli_font::EXTENSION);
    let assets = project.join("assets").join("fonts");
    let candidates = [assets.join(&name), project.join(&name)];
    candidates
        .iter()
        .find(|c| c.is_file())
        .cloned()
        .or_else(|| {
            Some(if assets.is_dir() {
                candidates[0].clone()
            } else {
                candidates[1].clone()
            })
        })
}

/// The first working-copy file of the font at `yaff` modified later than
/// it.
pub(crate) fn newer_copy(yaff: &Path) -> Option<PathBuf> {
    let since = fs::metadata(yaff).and_then(|m| m.modified()).ok()?;
    copies(yaff).existing().into_iter().find(|p| {
        fs::metadata(p)
            .and_then(|m| m.modified())
            .is_ok_and(|t| t > since)
    })
}

trait StripSuffix {
    fn strip_suffix_path(&self, suffix: &Path) -> Option<PathBuf>;
}

impl StripSuffix for Path {
    /// `self` without `suffix`'s components at its end.
    fn strip_suffix_path(&self, suffix: &Path) -> Option<PathBuf> {
        let mine: Vec<_> = self.components().collect();
        let theirs: Vec<_> = suffix.components().collect();
        let keep = mine.len().checked_sub(theirs.len())?;
        (mine[keep..] == theirs[..]).then(|| mine[..keep].iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_projects_font_has_its_copies_under_the_projects_build_folder() {
        let c = copies(Path::new("/p/game/assets/fonts/./main.yaff"));
        assert_eq!(c.png, Path::new("/p/game/build/font-edit/main.png"));
        assert_eq!(c.chars, Path::new("/p/game/build/font-edit/main.chars"));
        assert_eq!(c.bdf, Path::new("/p/game/build/font-edit/main.bdf"));
        let loose = copies(Path::new("/tmp/x/talk.yaff"));
        assert_eq!(loose.png, Path::new("/tmp/x/build/font-edit/talk.png"));
    }

    #[test]
    fn a_copy_leads_back_to_its_font() {
        let tmp = std::env::temp_dir().join(format!("gbcli-copies-{}", std::process::id()));
        let fonts = tmp.join("assets/fonts");
        fs::create_dir_all(&fonts).unwrap();
        let png = tmp.join("build/font-edit/main.png");
        assert_eq!(font_of(&png), Some(fonts.join("main.yaff")));
        assert_eq!(font_of(Path::new("/elsewhere/main.png")), None);
        fs::remove_dir_all(&tmp).unwrap();
        assert_eq!(font_of(&png), Some(tmp.join("main.yaff")));
    }
}
