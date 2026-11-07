//! Images by convention (design §12, Build): every PNG in `assets/bg/` is
//! a background and every PNG in `assets/sprites/` a sheet of 8x16
//! objects. Each is checked against the DMG's hardware limits first, so a
//! problem comes back as the limit, the count, the tiles to look at and a
//! remedy; one that fits is converted by rgbgfx into `build/images/`, with
//! a preview, and `images.asm` and `images.inc` give the program its labels
//! and constants (see `emit`). Engine budgets (a font, a HUD) are the
//! program's to keep; only the hardware's limits are checked.

mod bg;
mod census;
mod emit;
mod find;
mod pixels;
mod preview;
mod run;
mod shades;
mod sheet;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use gbcli_protocol::{Diagnostic, ImageAsset, ImageKind, ImageLimit, ImageProblem, ImagesMade};

use crate::project::BuildError;
use crate::toolchain::Toolchain;
use bg::BgPlan;
use emit::Entry;
use find::Source;
use pixels::Pixels;
use sheet::SheetPlan;

/// Where the converted data goes, relative to the project.
const OUT: &str = "build/images";
/// VRAM's tiles on the DMG, shared by the background and objects.
const VRAM_TILES: u32 = 384;

/// What the images became, for the build to assemble.
pub(crate) struct Converted {
    pub made: ImagesMade,
    /// rgbgfx's diagnostics.
    pub diagnostics: Vec<Diagnostic>,
    /// The generated source and include directory, when every image
    /// converted; none when a problem or rgbgfx stopped the build.
    pub generated: Option<Generated>,
}

pub(crate) struct Generated {
    /// The assembly that INCBINs the data, to assemble with the sources.
    pub source: PathBuf,
    /// The include directory holding `images.inc`, relative to the project.
    pub include_dir: PathBuf,
}

enum Plan {
    Bg(BgPlan),
    Sheet(SheetPlan),
}

impl Plan {
    fn tiles(&self) -> u32 {
        match self {
            Plan::Bg(p) => p.tiles,
            Plan::Sheet(p) => p.tiles(),
        }
    }
}

/// Converts the project's images; `None` when it has none.
pub(crate) fn convert(
    tools: &Toolchain,
    project: &Path,
    shown: &dyn Fn(&Path) -> String,
    timeout: Duration,
) -> Result<Option<Converted>, BuildError> {
    let sources = find::sources(project, shown)?;
    if sources.is_empty() {
        return Ok(None);
    }
    let mut made = ImagesMade::default();
    let mut plans: Vec<(&Source, String, Plan)> = Vec::new();
    for s in &sources {
        let image = shown(&s.path);
        let pixels = Pixels::read(&s.path).map_err(|e| BuildError::Layout {
            message: format!("{image} isn't a PNG gbcli can read: {e}"),
            remedy: "save it as a PNG (8-bit gray, RGB or RGBA)".into(),
        })?;
        let checked = match s.kind {
            ImageKind::Bg => bg::check(&image, &pixels).map(Plan::Bg),
            ImageKind::Sprites => sheet::check(&image, &s.stem, &pixels, s.frame).map(Plan::Sheet),
        };
        match checked {
            Ok(plan) => {
                if let Plan::Sheet(p) = &plan {
                    made.problems.extend(p.warnings.iter().cloned());
                }
                plans.push((s, image, plan));
            }
            Err(problems) => made.problems.extend(problems),
        }
    }
    made.problems.extend(vram(&plans));
    let stopped = Converted {
        made: ImagesMade::default(),
        diagnostics: Vec::new(),
        generated: None,
    };
    if made.first_error().is_some() {
        return Ok(Some(Converted { made, ..stopped }));
    }

    let mut rgbgfx = run::Rgbgfx {
        path: tools.rgbgfx().map_err(BuildError::Toolchain)?,
        project,
        timeout,
        diagnostics: Vec::new(),
    };
    let io = |path: PathBuf| move |error| BuildError::Io { path, error };
    for kind in [ImageKind::Bg, ImageKind::Sprites] {
        let dir = project.join(OUT).join(kind.dir());
        fs::create_dir_all(&dir).map_err(io(dir.clone()))?;
    }
    let mut data_paths = Vec::new();
    let mut ok = true;
    for (s, image, plan) in &plans {
        let rel = s.path.strip_prefix(project).unwrap_or(&s.path);
        let data = format!("{OUT}/{}/{}", s.kind.dir(), s.stem);
        let preview_path = project.join(format!("{data}.preview.png"));
        let (asset, picture) = match plan {
            Plan::Bg(p) => {
                let Some(d) = rgbgfx.background(image, rel, p, &data)? else {
                    ok = false;
                    continue;
                };
                let asset = asset(s, image, p.width, p.height, p.tiles, None, None);
                (
                    asset,
                    preview::background(image, &d.tiles, &d.map, p.width, p.height),
                )
            }
            Plan::Sheet(p) => {
                let Some(units) = rgbgfx.sheet(image, rel, p, &data)? else {
                    ok = false;
                    continue;
                };
                if !units.is_empty() {
                    let path = project.join(format!("{data}.2bpp"));
                    fs::write(&path, &units).map_err(io(path.clone()))?;
                }
                let objects = p.frames.iter().map(Vec::len).max().unwrap_or(0) as u32;
                let frames = Some(p.frames.len() as u32);
                let asset = asset(
                    s,
                    image,
                    p.width,
                    p.height,
                    p.tiles(),
                    frames,
                    Some(objects),
                );
                (asset, preview::sheet(image, &units, p))
            }
        };
        let png = picture.png().map_err(|e| BuildError::Io {
            path: preview_path.clone(),
            error: std::io::Error::other(e.to_string()),
        })?;
        fs::write(&preview_path, png).map_err(io(preview_path.clone()))?;
        made.images.push(ImageAsset {
            preview: shown(&preview_path),
            ..asset
        });
        data_paths.push(data);
    }
    let diagnostics = std::mem::take(&mut rgbgfx.diagnostics);
    if !ok {
        made.images.clear();
        return Ok(Some(Converted {
            made,
            diagnostics,
            ..stopped
        }));
    }

    let entries: Vec<Entry> = plans
        .iter()
        .zip(&data_paths)
        .map(|((s, image, plan), data)| match plan {
            Plan::Bg(p) => Entry::Bg {
                source: image,
                label: &s.label,
                constant: &s.constant,
                data,
                width: p.width,
                height: p.height,
                tiles: p.tiles,
            },
            Plan::Sheet(p) => Entry::Sheet {
                source: image,
                label: &s.label,
                constant: &s.constant,
                data,
                plan: p,
            },
        })
        .collect();
    let out = project.join(OUT);
    let (asm, inc) = (out.join("images.asm"), out.join("images.inc"));
    fs::write(&asm, emit::asm(&entries)).map_err(io(asm.clone()))?;
    fs::write(&inc, emit::inc(&entries)).map_err(io(inc.clone()))?;
    made.include = Some(shown(&inc));
    Ok(Some(Converted {
        made,
        diagnostics,
        generated: Some(Generated {
            source: asm,
            include_dir: PathBuf::from(OUT),
        }),
    }))
}

fn asset(
    s: &Source,
    image: &str,
    width: u32,
    height: u32,
    tiles: u32,
    frames: Option<u32>,
    objects: Option<u32>,
) -> ImageAsset {
    ImageAsset {
        kind: s.kind,
        source: image.to_string(),
        label: s.label.clone(),
        width,
        height,
        tiles,
        frames,
        objects,
        preview: String::new(),
    }
}

/// The limits the images break together: each background with every sheet
/// must fit VRAM's 384 tiles (one background on screen at a time, all
/// sheets loaded), and the sheets the 256 tiles objects address.
fn vram(plans: &[(&Source, String, Plan)]) -> Vec<ImageProblem> {
    let sheets: Vec<(&str, u32)> = plans
        .iter()
        .filter(|(_, _, p)| matches!(p, Plan::Sheet(_)))
        .map(|(_, image, p)| (image.as_str(), p.tiles()))
        .collect();
    let sprite_tiles: u32 = sheets.iter().map(|(_, n)| n).sum();
    let listed = sheets
        .iter()
        .map(|(image, n)| format!("{image} {n}"))
        .collect::<Vec<_>>()
        .join(", ");
    let mut problems = Vec::new();
    let biggest = plans
        .iter()
        .filter(|(_, _, p)| matches!(p, Plan::Bg(_)))
        .max_by_key(|(_, _, p)| p.tiles());
    if let Some((_, image, bg)) = biggest
        && bg.tiles() + sprite_tiles > VRAM_TILES
    {
        let total = bg.tiles() + sprite_tiles;
        problems.push(ImageProblem {
            image: None,
            ..census::error(
                image,
                ImageLimit::VramTiles,
                total as usize,
                VRAM_TILES as usize,
                format!(
                    "{image} uses {} tiles and the sprite sheets {sprite_tiles} ({listed}): \
                     {total} of the {VRAM_TILES} tiles VRAM holds for both",
                    bg.tiles()
                ),
                "cut unique tiles from the background or the sheets (flipped copies of an \
                 8x16 object are free), or load them at different times as separate images",
            )
        });
    }
    if sheets.len() > 1 && sprite_tiles > sheet::OBJ_TILES as u32 {
        problems.push(ImageProblem {
            image: None,
            ..census::error(
                "",
                ImageLimit::ObjTiles,
                sprite_tiles as usize,
                sheet::OBJ_TILES,
                format!(
                    "the sprite sheets need {sprite_tiles} tiles ({listed}); objects address {}",
                    sheet::OBJ_TILES
                ),
                "reuse units across frames, or split the sheets so fewer load at once",
            )
        });
    }
    problems
}
