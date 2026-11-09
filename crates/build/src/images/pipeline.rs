//! One conversion, from what `input` read to the results: every image's
//! DMG version, the project's shades, each image's checks, the GBC
//! palettes, rgbgfx, and each image's result. `build` and `assets` both
//! convert here; only a build writes the generated files (`emit`).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use gbcli_protocol::{BuildTarget, ImageKind, ImagesMade};

use super::derive::{self, Version};
use super::emit::{self, Entry};
use super::find::{self, KINDS, Source};
use super::input::{Input, Origin};
use super::pixels::{Color, Pixels};
use super::{Checked, Converted, Generated, Plan, bg, census, indexed, output, run, shades};
use super::{shared, sheet, vram};
use crate::project::BuildError;
use crate::toolchain::ImageTool;

/// Where a conversion writes.
pub(crate) struct Place<'a> {
    /// The data and previews, laid out by kind (`bg/town.2bpp`); rgbgfx
    /// runs here.
    pub dir: &'a Path,
    /// For a build, `dir` as `images.asm` INCBINs from it, relative to the
    /// project (`build/images`); the generated files are written only then.
    pub generated: Option<&'a str>,
}

/// Converts `input`'s images for `target`; `None` when there are none and
/// nothing to say about them.
pub(crate) fn convert(
    tools: &ImageTool,
    input: Input,
    place: &Place,
    shown: &dyn Fn(&Path) -> String,
    target: BuildTarget,
    timeout: Duration,
) -> Result<Option<Converted>, BuildError> {
    let Input {
        origin,
        sources,
        named: named_palettes,
        palette_shown,
        problems,
    } = input;
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
    let mut made = ImagesMade {
        problems,
        ..ImagesMade::default()
    };
    let stopped = Converted {
        made: ImagesMade::default(),
        diagnostics: Vec::new(),
        generated: None,
        stopped: true,
    };
    if sources.is_empty() {
        // No images to shade, but a malformed palette file is still the
        // project's mistake to hear about.
        if made.problems.is_empty() {
            return Ok(None);
        }
        let stops = made.first_error().is_some();
        return Ok(Some(Converted {
            made,
            stopped: stops,
            ..stopped
        }));
    }
    let read_png = |path: &Path| {
        Pixels::read(path).map_err(|e| BuildError::Layout {
            message: format!("{} isn't a PNG gbcli can read: {e}", shown(path)),
            remedy: match origin {
                Origin::Project => "save it as a PNG (8-bit gray, RGB or RGBA)".into(),
                Origin::Manifest => "check the path in the manifest, and that the file is a PNG \
                                     (8-bit gray, RGB or RGBA); `gbcli assets --manifest` reads \
                                     a relative path from the manifest's folder, and the wire \
                                     from the server's working directory"
                    .into(),
            },
        })
    };
    let mut all = Vec::with_capacity(sources.len());
    for s in &sources {
        let pixels = read_png(&s.path)?;
        let over = s.dmg.as_deref().map(read_png).transpose()?;
        all.push((s, shown(&s.path), pixels, over));
    }
    // A bad palette file stops the conversion once the images are known to
    // read: the shades are chosen from the palettes and every image.
    if made.first_error().is_some() {
        return Ok(Some(Converted { made, ..stopped }));
    }
    find::palette_collision(&sources, &named_palettes, &palette_shown, shown)?;
    // Each image's DMG version. In a DMG build every image is DMG art; with
    // or without --gbc (plan question 10) `dmg` drives the cascade, the
    // named palettes are checked, and an assigned image builds as DMG art.
    let mut versioned = Vec::with_capacity(all.len());
    for (s, image, pixels, over) in all {
        let assigned = named_palettes.assignments.get(&s.name);
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
        // A manifest's `dmg` is any file; name the one given.
        let dmg_shown = shown(&s.dmg.clone().unwrap_or_else(|| find::dmg_path(&s.path)));
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
        let joint = color.as_ref().map(|pixels| Color {
            image: &image,
            pixels,
        });
        let checked = match s.kind {
            ImageKind::Bg | ImageKind::Tiles => {
                bg::check(&checked_name, s.kind, &dmg, shading, joint).map(Plan::Bg)
            }
            ImageKind::Sprites => {
                sheet::check(&checked_name, &s.stem, &dmg, s.frame, shading, joint).map(Plan::Sheet)
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
        let sheets: Vec<(&str, u8)> = plans
            .iter()
            .filter_map(|c| match &c.plan {
                Plan::Sheet(p) => Some((c.s.name.as_str(), p.obp)),
                Plan::Bg(_) => None,
            })
            .collect();
        indexed::Indexed::new(&named_palettes, &sheets)
    });
    made.problems
        .extend(indexed.as_ref().and_then(|i| i.problem(&palette_shown)));
    if target == BuildTarget::Gbc {
        let assigned: Vec<(String, ImageKind)> = sources
            .iter()
            .filter(|s| indexed::assigned(&named_palettes, &s.name).is_some())
            .map(|s| (shown(&s.path), s.kind))
            .collect();
        made.problems.extend(indexed::unpinned(
            &named_palettes,
            &assigned,
            shading.choice,
            &palette_shown,
        ));
    }
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
        path: tools.path().map_err(BuildError::Toolchain)?,
        root: place.dir,
        timeout,
        diagnostics: Vec::new(),
        problems: Vec::new(),
    };
    let io = |path: PathBuf| move |error| BuildError::Io { path, error };
    for kind in KINDS.into_iter().filter(|&k| has_kind(&sources, k)) {
        let dir = place.dir.join(kind.dir());
        fs::create_dir_all(&dir).map_err(io(dir.clone()))?;
    }
    // What the images share on the GBC: the assigned OBJ palettes, then
    // the packed ones.
    let gbc_made = indexed.as_ref().map(|i| {
        let mut gbc = i.made();
        gbc.obj_palettes.extend_from_slice(packed.obj_palettes());
        gbc
    });
    let mut out = output::Output {
        rgbgfx,
        root: place.dir,
        shown,
        indexed: indexed.as_ref(),
        packed: &packed,
        obj_palettes: gbc_made.as_ref().map_or(&[], |g| &g.obj_palettes),
    };
    let mut ok = true;
    for (i, (c, (pixels, file))) in plans.iter().zip(&read).enumerate() {
        match out.image(i, c, pixels, file.as_ref())? {
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
    let generated = match place.generated {
        Some(from_project) => {
            let shared = emit::Shared {
                gbc: gbc_made.as_ref(),
                named: &named_palettes.palettes,
            };
            let (asm, inc) = (place.dir.join("images.asm"), place.dir.join("images.inc"));
            let data: Vec<String> = plans
                .iter()
                .map(|c| format!("{from_project}/{}/{}", c.s.kind.dir(), c.s.stem))
                .collect();
            // The generated files come from the results (`made.images`, in
            // the plans' order), so the include and the protocol can't
            // disagree.
            let entries = entries(&plans, &data, &made);
            fs::write(&asm, emit::asm(&entries, &shared)).map_err(io(asm.clone()))?;
            fs::write(&inc, emit::inc(&entries, &shared)).map_err(io(inc.clone()))?;
            made.include = Some(shown(&inc));
            Some(Generated {
                source: asm,
                include_dir: PathBuf::from(from_project),
            })
        }
        None => None,
    };
    made.gbc = gbc_made;
    Ok(Some(Converted {
        made,
        diagnostics,
        generated,
        stopped: false,
    }))
}

fn has_kind(sources: &[Source], kind: ImageKind) -> bool {
    sources.iter().any(|s| s.kind == kind)
}

/// Each converted image as the generated files name it, its data at
/// `data` (the INCBIN path, without extension).
fn entries<'a>(plans: &'a [Checked], data: &'a [String], made: &'a ImagesMade) -> Vec<Entry<'a>> {
    plans
        .iter()
        .zip(data)
        .zip(&made.images)
        .map(|((c, data), asset)| match &c.plan {
            Plan::Bg(p) if c.s.kind == ImageKind::Tiles => Entry::Tiles {
                source: &c.image,
                label: &c.s.label,
                constant: &c.s.constant,
                data,
                tiles: p.tiles,
                gbc: emit::Gbc::of(asset, p.unique.len()),
            },
            Plan::Bg(p) => Entry::Bg {
                source: &c.image,
                label: &c.s.label,
                constant: &c.s.constant,
                data,
                width: p.width,
                height: p.height,
                tiles: p.tiles,
                gbc: emit::Gbc::of(asset, 0),
            },
            Plan::Sheet(p) => Entry::Sheet {
                source: &c.image,
                label: &c.s.label,
                constant: &c.s.constant,
                data,
                plan: p,
                gbc: emit::Gbc::of(asset, p.kept.len()),
            },
        })
        .collect()
}

/// Every color of every image, one per RGB555 key: the key's most used
/// value in the image that uses it most (the first such image on a tie),
/// as `census::colors` stands one value for its key.
fn project_colors<'a>(images: impl Iterator<Item = &'a Pixels>) -> Vec<super::pixels::Rgba> {
    let mut best: std::collections::BTreeMap<census::Key, (super::pixels::Rgba, u32)> =
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
