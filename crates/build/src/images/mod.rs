//! Images by convention (design §12, Build): every PNG in `assets/bg/` is
//! a background, every PNG in `assets/sprites/` a sheet of objects (8x16,
//! or 8x8: `sheet::object_size`), and every PNG in `assets/tiles/` a
//! tileset (a background's tiles with no map). Each is checked against the DMG's hardware limits first, so a
//! problem comes back as the limit, the count, the tiles to look at and a
//! remedy; one that fits is converted by rgbgfx into `build/images/`, with
//! a preview, and `images.asm` and `images.inc` give the program its labels
//! and constants (see `emit`). Engine budgets (a font, a HUD) are the
//! program's to keep; only the hardware's limits are checked. `assets`
//! converts the same way without a build (`pipeline`, `input`).

mod agree;
mod bg;
mod census;
mod derive;
mod emit;
mod find;
mod gbc;
mod indexed;
pub(crate) mod input;
mod output;
mod pack;
pub(crate) mod palettes;
pub(crate) mod pipeline;
mod pixels;
mod preview;
mod run;
mod shades;
mod shared;
mod sheet;
mod vram;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use gbcli_protocol::{BuildTarget, Diagnostic, ImageProblem, ImagesMade};

use crate::project::BuildError;
use crate::toolchain::Toolchain;
use bg::BgPlan;
use find::Source;
use pixels::Pixels;
use sheet::SheetPlan;

/// Where a build's converted data goes, relative to the project.
const OUT: &str = "build/images";

/// What the images became, for the build to assemble.
pub(crate) struct Converted {
    pub made: ImagesMade,
    /// rgbgfx's diagnostics.
    pub diagnostics: Vec<Diagnostic>,
    /// The generated source and include directory, when every image
    /// converted for a build; none when a problem or rgbgfx stopped it,
    /// when there are no images, only something to say about them, or
    /// when `assets` converted them.
    pub generated: Option<Generated>,
    /// Whether the images stopped the build: a problem, or rgbgfx. Without
    /// images there is nothing to stop, only warnings to carry.
    pub stopped: bool,
}

pub(crate) struct Generated {
    /// The assembly that INCBINs the data, to assemble with the sources.
    pub source: PathBuf,
    /// The include directory holding `images.inc`, relative to the project.
    pub include_dir: PathBuf,
}

enum Plan {
    /// A background's, or a tileset's: its tiles are checked and
    /// deduplicated alike, and only a background keeps its map.
    Bg(BgPlan),
    Sheet(SheetPlan),
}

/// An image that passed its checks: its source, as shown, its plan, and
/// for color art under `--gbc` its color image, whose tiles the plan pairs
/// with the DMG version's (plan question 1).
pub(super) struct Checked<'a> {
    s: &'a Source,
    image: String,
    plan: Plan,
    color: Option<Pixels>,
    /// The color image is pixel text, so rgbgfx reads a PNG of `color`.
    color_text: bool,
}

impl Plan {
    fn tiles(&self) -> u32 {
        match self {
            Plan::Bg(p) => p.tiles,
            Plan::Sheet(p) => p.tiles(),
        }
    }

    fn warnings(&self) -> &[ImageProblem] {
        match self {
            Plan::Bg(p) => &p.warnings,
            Plan::Sheet(p) => &p.warnings,
        }
    }

    fn merges(&self) -> &[(census::Key, pixels::Rgba)] {
        match self {
            Plan::Bg(p) => &p.merges,
            Plan::Sheet(p) => &p.merges,
        }
    }
}

/// Converts the project's images for `target` into `build/images/`, with
/// the generated files; `None` when it has none.
pub(crate) fn convert(
    tools: &Toolchain,
    project: &Path,
    shown: &dyn Fn(&Path) -> String,
    target: BuildTarget,
    timeout: Duration,
) -> Result<Option<Converted>, BuildError> {
    let input = input::project(project, shown)?;
    let dir = project.join(OUT);
    let place = pipeline::Place {
        dir: &dir,
        generated: Some(OUT),
    };
    pipeline::convert(tools.images(), input, &place, shown, target, timeout)
}

/// A transient file, removed when dropped.
struct Removed(PathBuf);

impl Drop for Removed {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
