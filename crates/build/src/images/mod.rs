//! Images by convention (design §12, Build): every PNG in `assets/bg/` is
//! a background and every PNG in `assets/sprites/` a sheet of 8x16
//! objects. Each is checked against the DMG's hardware limits first, so a
//! problem comes back as the limit, the count, the tiles to look at and a
//! remedy; one that fits is converted by rgbgfx into `build/images/`, with
//! a preview, and `images.asm` and `images.inc` give the program its labels
//! and constants (see `emit`). Engine budgets (a font, a HUD) are the
//! program's to keep; only the hardware's limits are checked.

mod agree;
mod bg;
mod census;
mod derive;
mod emit;
mod find;
mod gbc;
mod indexed;
mod output;
mod pack;
mod palettes;
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

use gbcli_protocol::{BuildTarget, Diagnostic, ImageKind, ImageProblem, ImagesMade};

use crate::project::BuildError;
use crate::toolchain::Toolchain;
use bg::BgPlan;
use derive::Version;
use emit::Entry;
use find::Source;
use pixels::Pixels;
use sheet::SheetPlan;

/// Where the converted data goes, relative to the project.
const OUT: &str = "build/images";

/// What the images became, for the build to assemble.
pub(crate) struct Converted {
    pub made: ImagesMade,
    /// rgbgfx's diagnostics.
    pub diagnostics: Vec<Diagnostic>,
    /// The generated source and include directory, when every image
    /// converted; none when a problem or rgbgfx stopped the build, or when
    /// there are no images, only something to say about them.
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

/// Converts the project's images for `target`; `None` when it has none.
pub(crate) fn convert(
    tools: &Toolchain,
    project: &Path,
    shown: &dyn Fn(&Path) -> String,
    target: BuildTarget,
    timeout: Duration,
) -> Result<Option<Converted>, BuildError> {
    let sources = find::sources(project, shown)?;
    // Plan question 9: a DMG version a DMG build would ignore is an error.
    if target == BuildTarget::Dmg
        && let Some((s, dmg)) = sources.iter().find_map(|s| Some((s, s.dmg.as_ref()?)))
    {
        return Err(BuildError::Layout {
            message: format!(
                "{} is the DMG version of {} for a GBC build, but this build is for the DMG, \
                 where the image itself is the DMG art",
                shown(dmg),
                shown(&s.path)
            ),
            remedy: format!(
                "build with --gbc to use it, or remove {} to build {} as DMG art",
                shown(dmg),
                shown(&s.path)
            ),
        });
    }
    let mut made = ImagesMade::default();
    let stopped = Converted {
        made: ImagesMade::default(),
        diagnostics: Vec::new(),
        generated: None,
        stopped: true,
    };
    let palette_shown = shown(&project.join(palettes::PATH));
    made.problems.extend(palettes::old_file(project, shown));
    let assets = project.join("assets");
    // Each image by its path under assets/, as an assignment names it.
    let under_assets = |s: &Source| {
        let rel = s.path.strip_prefix(&assets).unwrap_or(&s.path);
        rel.components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/")
    };
    let images: Vec<String> = sources.iter().map(under_assets).collect();
    if sources.is_empty() {
        // No images to shade, but a malformed palette file is still the
        // project's mistake to hear about.
        return match palettes::read(project, &palette_shown, &images) {
            Ok(_) if made.problems.is_empty() => Ok(None),
            Ok(_) => Ok(Some(Converted {
                made,
                stopped: false,
                ..stopped
            })),
            Err(p) => {
                made.problems.push(p);
                Ok(Some(Converted { made, ..stopped }))
            }
        };
    }
    let read_png = |path: &Path| {
        Pixels::read(path).map_err(|e| BuildError::Layout {
            message: format!("{} isn't a PNG gbcli can read: {e}", shown(path)),
            remedy: "save it as a PNG (8-bit gray, RGB or RGBA)".into(),
        })
    };
    let mut all = Vec::with_capacity(sources.len());
    for s in &sources {
        let pixels = read_png(&s.path)?;
        let over = s.dmg.as_deref().map(read_png).transpose()?;
        all.push((s, shown(&s.path), pixels, over));
    }
    // One color, one shade, in every image: the shades are chosen from the
    // whole project's colors before any image is planned.
    // With or without --gbc (plan question 10): `dmg` drives the cascade, the
    // named palettes are checked, and an assigned image builds as DMG art.
    let named_palettes = match palettes::read(project, &palette_shown, &images) {
        Ok(named) => named.unwrap_or_default(),
        Err(p) => {
            made.problems.push(p);
            return Ok(Some(Converted { made, ..stopped }));
        }
    };
    // Each image's DMG version. In a DMG build every image is DMG art.
    let mut versioned = Vec::with_capacity(all.len());
    for ((s, image, pixels, over), rel) in all.into_iter().zip(&images) {
        let assigned = named_palettes.assignments.get(rel);
        let version = derive::version(target, assigned, over, s, &image, shown)?;
        versioned.push((s, image, pixels, version));
    }
    // One color, one shade, in every image, overrides included.
    let colors = project_colors(versioned.iter().flat_map(|(_, _, p, v)| match v {
        Version::Override(o) => vec![p, o],
        _ => vec![p],
    }));
    let shading = shades::choose(named_palettes.dmg.as_ref(), &colors);
    made.shades = Some(shading.choice);
    // Colors shared across images are named only among DMG art: a derived
    // version's shares are scoped to its tiles and objects (plan question 2).
    let dmg_art: Vec<(String, &Pixels)> = versioned
        .iter()
        .filter_map(|(s, image, p, v)| match v {
            Version::Art => Some((image.clone(), p)),
            Version::Override(o) => Some((shown(s.dmg.as_deref()?), o)),
            Version::Derived => None,
        })
        .collect();
    let dmg_colors = project_colors(dmg_art.iter().map(|(_, p)| *p));
    let dmg_named: Vec<(&str, &Pixels)> = dmg_art.iter().map(|(i, p)| (i.as_str(), *p)).collect();
    let across = shared::across(&dmg_named, &dmg_colors, &shading);
    let grays = derive::gray_shading();

    let mut plans: Vec<Checked> = Vec::new();
    // Each plan's DMG version, for rgbgfx: its pixels, and the file that
    // holds them, if any (a derived version is written out first).
    let mut read: Vec<(Pixels, Option<PathBuf>)> = Vec::new();
    for (s, image, pixels, version) in versioned {
        let dmg_shown = shown(&find::dmg_path(&s.path));
        // The DMG version the checks read, under the name they give it,
        // and color art's color image, whose tiles pair with it.
        let (checked_name, dmg, file, shading, color) = match version {
            Version::Art => (image.clone(), pixels, Some(s.path.clone()), &shading, None),
            Version::Override(o) => {
                let wrong = derive::override_size(&image, &dmg_shown, &pixels, &o)
                    .or_else(|| derive::override_alpha(&image, &dmg_shown, s.kind, &pixels, &o));
                if let Some(p) = wrong {
                    made.problems.push(p);
                    continue;
                }
                (dmg_shown, o, s.dmg.clone(), &shading, Some(pixels))
            }
            Version::Derived => {
                match derive::derive(&image, &dmg_shown, s.kind, &pixels, &shading) {
                    Ok(d) => {
                        made.problems.extend(d.warnings);
                        (image.clone(), d.pixels, None, &grays, Some(pixels))
                    }
                    Err(p) => {
                        made.problems.push(p);
                        continue;
                    }
                }
            }
        };
        let checked = match s.kind {
            ImageKind::Bg => bg::check(&checked_name, &dmg, shading, color.as_ref()).map(Plan::Bg),
            ImageKind::Sprites => sheet::check(
                &checked_name,
                &s.stem,
                &dmg,
                s.frame,
                shading,
                color.as_ref(),
            )
            .map(Plan::Sheet),
            ImageKind::Tiles => {
                unreachable!("find yields no tileset: assets/tiles/ isn't read yet")
            }
        };
        match checked {
            Ok(plan) => {
                made.problems.extend(plan.warnings().iter().cloned());
                plans.push(Checked {
                    s,
                    image,
                    plan,
                    color,
                });
                read.push((dmg, file));
            }
            Err(problems) => made.problems.extend(problems),
        }
    }
    made.problems.extend(across);
    made.problems.extend(vram::vram(&plans));
    // On the GBC, indexed art and the OBJ slots its sheets take.
    let indexed = (target == BuildTarget::Gbc).then(|| {
        let sheets: Vec<(String, u8)> = plans
            .iter()
            .filter_map(|c| match &c.plan {
                Plan::Sheet(p) => Some((under_assets(c.s), p.obp)),
                Plan::Bg(_) => None,
            })
            .collect();
        let sheets: Vec<(&str, u8)> = sheets.iter().map(|(s, o)| (s.as_str(), *o)).collect();
        indexed::Indexed::new(&named_palettes, &sheets)
    });
    made.problems
        .extend(indexed.as_ref().and_then(|i| i.problem(&palette_shown)));
    if made.first_error().is_some() {
        return Ok(Some(Converted { made, ..stopped }));
    }
    // Only now, with every check passed (census::alpha among them), are
    // color art's palettes packed.
    let reserved = indexed.as_ref().map_or(0, indexed::Indexed::reserved);
    let packed = match output::packed(&plans, reserved) {
        Ok(p) => p,
        Err(problems) => {
            made.problems.extend(problems);
            return Ok(Some(Converted { made, ..stopped }));
        }
    };

    let rgbgfx = run::Rgbgfx {
        path: tools.rgbgfx().map_err(BuildError::Toolchain)?,
        project,
        timeout,
        diagnostics: Vec::new(),
        problems: Vec::new(),
    };
    let io = |path: PathBuf| move |error| BuildError::Io { path, error };
    for kind in [ImageKind::Bg, ImageKind::Sprites] {
        let dir = project.join(OUT).join(kind.dir());
        fs::create_dir_all(&dir).map_err(io(dir.clone()))?;
    }
    let mut out = output::Output {
        rgbgfx,
        project,
        shown,
        indexed: indexed.as_ref(),
        packed: &packed,
    };
    let mut ok = true;
    for (i, (c, (pixels, file))) in plans.iter().zip(&read).enumerate() {
        match out.image(i, c, pixels, file.as_ref(), &under_assets(c.s))? {
            Some(asset) => made.images.push(asset),
            None => ok = false,
        }
    }
    let diagnostics = std::mem::take(&mut out.rgbgfx.diagnostics);
    made.problems.append(&mut out.rgbgfx.problems);
    if !ok {
        made.images.clear();
        return Ok(Some(Converted {
            made,
            diagnostics,
            ..stopped
        }));
    }

    let data_paths: Vec<String> = plans
        .iter()
        .map(|c| format!("{OUT}/{}/{}", c.s.kind.dir(), c.s.stem))
        .collect();
    let entries: Vec<Entry> = plans
        .iter()
        .zip(&data_paths)
        .map(|(c, data)| match &c.plan {
            Plan::Bg(p) => Entry::Bg {
                source: &c.image,
                label: &c.s.label,
                constant: &c.s.constant,
                data,
                width: p.width,
                height: p.height,
                tiles: p.tiles,
            },
            Plan::Sheet(p) => Entry::Sheet {
                source: &c.image,
                label: &c.s.label,
                constant: &c.s.constant,
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
    made.gbc = indexed.as_ref().map(|i| {
        let mut gbc = i.made();
        gbc.obj_palettes.extend_from_slice(packed.obj_palettes());
        gbc
    });
    Ok(Some(Converted {
        made,
        diagnostics,
        generated: Some(Generated {
            source: asm,
            include_dir: PathBuf::from(OUT),
        }),
        stopped: false,
    }))
}

/// Every color of every image, one per RGB555 key: the key's most used
/// value in the image that uses it most (the first such image on a tie),
/// as `census::colors` stands one value for its key.
fn project_colors<'a>(images: impl Iterator<Item = &'a Pixels>) -> Vec<pixels::Rgba> {
    let mut best: std::collections::BTreeMap<census::Key, (pixels::Rgba, u32)> =
        std::collections::BTreeMap::new();
    for p in images {
        for (c, n) in census::colors(p) {
            let entry = best.entry(census::key(c)).or_insert((c, n));
            if n > entry.1 {
                *entry = (c, n);
            }
        }
    }
    best.into_values().map(|(c, _)| c).collect()
}

/// A transient file, removed when dropped.
struct Removed(PathBuf);

impl Drop for Removed {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
