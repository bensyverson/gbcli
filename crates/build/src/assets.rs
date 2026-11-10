//! `assets` (design §12, leaf 7gGMeZ): a project's or a manifest's images
//! converted exactly as `build` converts them, encoding them in Rust: no
//! RGBDS, no toolchain, and nothing written to the project (Ben's ruling,
//! 2026-11-09). What it would write goes to a scratch directory, removed
//! after, unless the caller names one to keep it in.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use gbcli_protocol::{
    AssetManifest, AssetsArgs, AssetsMade, BuildTarget, ErrorBody, ErrorCode, ImagesMade,
    ResultBody,
};

use crate::images::{input, pipeline};
use crate::paths::display;
use crate::project::BuildError;

/// The images to convert.
#[derive(Clone, Debug)]
pub enum AssetsInput {
    /// The project directory whose `assets/` `build` would read.
    Project(PathBuf),
    /// Images one by one, each file relative to the options' `base`.
    Manifest(AssetManifest),
}

/// What to convert, for which machine, and where to keep it.
#[derive(Clone, Debug)]
pub struct AssetsOptions {
    pub input: AssetsInput,
    /// Relative paths start here, and the result's paths are relative to
    /// it when inside it.
    pub base: PathBuf,
    pub target: BuildTarget,
    /// Where to keep the data and previews (`build/images/`'s layout);
    /// none keeps nothing.
    pub keep: Option<PathBuf>,
    /// The glyphs to return from each font, by its path under `assets/`
    /// (`fonts/talk.yaff`); a font not named returns every glyph.
    pub glyphs: BTreeMap<String, Vec<String>>,
}

impl AssetsOptions {
    /// The conversion `args` asks for, paths relative to `base`. A project
    /// and a manifest together are refused: one names the images.
    pub fn from_args(args: &AssetsArgs, base: PathBuf) -> Result<AssetsOptions, AssetsError> {
        let input = match (&args.dir, &args.manifest) {
            (Some(dir), Some(_)) => {
                return Err(AssetsError::Build(BuildError::Layout {
                    message: format!(
                        "assets converts a project ({dir}) or a manifest's images, not both"
                    ),
                    remedy: "drop the project to convert the manifest's images (--manifest \
                             FILE), or drop the manifest to convert the project's assets/"
                        .into(),
                }));
            }
            (_, Some(m)) => AssetsInput::Manifest(m.clone()),
            (dir, None) => AssetsInput::Project(PathBuf::from(dir.as_deref().unwrap_or("."))),
        };
        Ok(AssetsOptions {
            target: args.target,
            keep: args.keep.as_ref().map(PathBuf::from),
            glyphs: args.glyphs.clone(),
            ..AssetsOptions::new(input, base)
        })
    }

    /// A DMG conversion of `input` that keeps nothing.
    pub fn new(input: AssetsInput, base: PathBuf) -> AssetsOptions {
        AssetsOptions {
            input,
            base,
            target: BuildTarget::Dmg,
            keep: None,
            glyphs: BTreeMap::new(),
        }
    }
}

/// Why the images didn't convert.
#[derive(Debug)]
pub enum AssetsError {
    /// The input or a file couldn't be used, as for a build.
    Build(BuildError),
    /// An image broke a limit: the result says which.
    Failed(Box<AssetsMade>),
}

impl From<BuildError> for AssetsError {
    fn from(e: BuildError) -> AssetsError {
        AssetsError::Build(e)
    }
}

impl AssetsError {
    pub fn body(&self) -> ErrorBody {
        let made = match self {
            AssetsError::Build(e) => return e.body(),
            AssetsError::Failed(made) => made,
        };
        let (message, remedy) = match made.images.first_error() {
            Some(p) => {
                let image = p
                    .image
                    .as_deref()
                    .map_or(String::new(), |i| format!("{i}: "));
                (
                    format!(
                        "the images didn't convert: {image}{} ({} of {}): {}",
                        p.limit.name(),
                        p.count,
                        p.max,
                        p.message
                    ),
                    p.remedy.clone(),
                )
            }
            // The images stop only on an error problem, and no tool runs to
            // say anything else; this is gbcli's bug if it is ever reached.
            None => (
                "the images didn't convert, and no problem says why".to_string(),
                "this is gbcli's bug: report it with the images".to_string(),
            ),
        };
        let mut body = ErrorBody::new(ErrorCode::BuildFailed, message, remedy);
        body.result = Some(Box::new(ResultBody {
            assets: Some((**made).clone()),
            ..ResultBody::default()
        }));
        body
    }
}

/// Converts the images `options` names. Warnings ride in the result.
pub fn assets(options: &AssetsOptions) -> Result<AssetsMade, AssetsError> {
    let base = fs::canonicalize(&options.base).unwrap_or(options.base.clone());
    let shown = |p: &Path| display(p, &base);
    let input = match &options.input {
        AssetsInput::Project(dir) => {
            let dir = base.join(dir);
            let project = fs::canonicalize(&dir).map_err(|_| BuildError::Layout {
                message: format!("no project directory at {}", shown(&dir)),
                remedy: "pass the project directory, or name the images in a manifest".into(),
            })?;
            input::project(&project, &shown)?
        }
        AssetsInput::Manifest(m) => input::manifest(m, &base, &shown)?,
    };
    let input = input::Input {
        glyphs: options.glyphs.clone(),
        ..input
    };
    let scratch;
    let dir = match &options.keep {
        Some(out) => {
            let out = base.join(out);
            fs::create_dir_all(&out).map_err(|error| BuildError::Io {
                path: out.clone(),
                error,
            })?;
            // As the base is, so paths under it show relative to it.
            fs::canonicalize(&out).unwrap_or(out)
        }
        None => {
            scratch = Scratch::new()?;
            scratch.0.clone()
        }
    };
    let place = pipeline::Place {
        dir: &dir,
        generated: None,
    };
    let converted = pipeline::convert(input, &place, &shown, options.target)?;
    let Some(converted) = converted else {
        return Ok(AssetsMade {
            target: options.target,
            images: ImagesMade::default(),
            diagnostics: Vec::new(),
        });
    };
    let mut images = converted.made;
    if options.keep.is_none() {
        // They were drawn in the scratch directory, which is gone.
        for i in &mut images.images {
            i.preview.clear();
            if let Some(g) = &mut i.gbc {
                g.preview.clear();
            }
        }
    }
    let made = AssetsMade {
        target: options.target,
        images,
        diagnostics: converted.diagnostics,
    };
    if converted.stopped {
        return Err(AssetsError::Failed(Box::new(made)));
    }
    Ok(made)
}

/// A directory of this process's own, removed with what it holds when
/// dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Result<Scratch, BuildError> {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("gbcli-assets-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).map_err(|error| BuildError::Io {
            path: dir.clone(),
            error,
        })?;
        Ok(Scratch(dir))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
