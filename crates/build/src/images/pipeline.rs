//! One conversion, from what `input` read to the results: every image's
//! DMG version, the project's shades, each image's checks, the GBC
//! palettes, the encoded data, and each image's result. `build` and
//! `assets` both convert here; only a build writes the generated files
//! (`emit`). No tool is run: the encode is gbcli's own (`encode`).

use std::fs;
use std::path::{Path, PathBuf};

use gbcli_protocol::{BuildTarget, ImageKind, ImageLimit, ImageProblem, ImagesMade};

use super::derive::{self, Version};
use super::emit::{self, Entry};
use super::find::{self, KINDS, Source};
use super::fonts::{self, Select};
use super::input::{Input, Origin};
use super::pixels::{Color, Pixels, Read, Unread};
use super::{Checked, Converted, Generated, Plan, bg, census, indexed, output, shades};
use super::{shared, sheet, vram};
use crate::project::BuildError;

/// Where a conversion writes.
pub(crate) struct Place<'a> {
    /// The data and previews, laid out by kind (`bg/town.2bpp`).
    pub dir: &'a Path,
    /// For a build, `dir` as `images.asm` INCBINs from it, relative to the
    /// project (`build/images`); the generated files are written only then.
    pub generated: Option<&'a str>,
}

/// Converts `input`'s images for `target`; `None` when there are none and
/// nothing to say about them.
pub(crate) fn convert(
    input: Input,
    place: &Place,
    shown: &dyn Fn(&Path) -> String,
    target: BuildTarget,
) -> Result<Option<Converted>, BuildError> {
    let Input {
        origin,
        sources,
        named: named_palettes,
        palettes,
        objects,
        problems,
        fonts,
        glyphs,
    } = input;
    let mut made = ImagesMade {
        problems,
        ..ImagesMade::default()
    };
    // A build emits what its fonts allow; `assets` returns what was asked.
    let select = match place.generated {
        Some(_) => Select::Build,
        None => Select::Request(&glyphs),
    };
    let fonts::Fonts {
        assets: font_assets,
        emitted: font_entries,
        problems: font_problems,
        overlaps: font_overlaps,
    } = fonts::convert(&fonts, &select, shown)?;
    made.fonts = font_assets;
    made.font_overlaps = font_overlaps;
    let stopped = Converted {
        made: ImagesMade::default(),
        diagnostics: Vec::new(),
        generated: None,
        stopped: true,
    };
    if sources.is_empty() {
        // A client may ask `assets` for its palettes' bytes alone; a build
        // with no images reports no images.
        if place.generated.is_none() && !named_palettes.palettes.is_empty() {
            let shading = shades::choose(&palettes, named_palettes.dmg.as_ref(), &[])
                .pinned(&named_palettes.pins);
            made.shades = Some(shading.choice);
            made.dmg_palettes = shading.named(&named_palettes.palettes);
            made.color_shades = shading.color_shades(&named_palettes.palettes);
        }
        // No images to shade, but a malformed palette file is still the
        // project's mistake to hear about.
        made.problems.extend(font_problems);
        if made.problems.is_empty() && fonts.is_empty() && made.dmg_palettes.is_empty() {
            return Ok(None);
        }
        let stops = made.first_error().is_some();
        // A warning alone, with no fonts, generates nothing, as before fonts.
        let generated = if stops || fonts.is_empty() {
            None
        } else {
            let shared = emit::Shared {
                gbc: None,
                named: &[],
                objects: None,
                fonts: &font_entries,
            };
            let written = write_generated(place, &[], &shared, shown)?;
            written.map(|(generated, include)| {
                made.include = Some(include);
                generated
            })
        };
        return Ok(Some(Converted {
            made,
            stopped: stops,
            generated,
            ..stopped
        }));
    }
    // Pixel text's palette entries resolve through the named palettes.
    let resolve = super::palettes::eight_bit(&named_palettes);
    let read_image = |path: &Path| match Pixels::read(path, &resolve) {
        Ok(read) => Ok(Ok(read)),
        Err(Unread::Text(e)) => Ok(Err((text_problem(&shown(path), e), false))),
        Err(Unread::Palette(e)) => Ok(Err((text_problem(&shown(path), e), true))),
        Err(Unread::Png(e)) => Err(BuildError::Layout {
            message: format!("{} isn't a PNG gbcli can read: {e}", shown(path)),
            remedy: match origin {
                Origin::Project => "save it as a PNG (8-bit gray, RGB or RGBA)".into(),
                Origin::Manifest => "check the path in the manifest, and that the file is a PNG \
                                     (8-bit gray, RGB or RGBA) or pixel text; `gbcli assets \
                                     --manifest` reads a relative path from the manifest's \
                                     folder, and the wire from the server's working directory"
                    .into(),
            },
        }),
    };
    // Each pixel text image's picture, by its name in problems, so a
    // hardware problem can name the line that drew its tile.
    let mut texts: Vec<(String, gbcli_pixels::Picture)> = Vec::new();
    // Pixel text that doesn't read is a problem of its own image: every
    // other image is still checked. Each is kept with whether it is a
    // palette reference that didn't resolve.
    let mut unread: Vec<(ImageProblem, bool)> = Vec::new();
    let mut all = Vec::with_capacity(sources.len());
    for s in &sources {
        let mut take = |read: Result<Read, (ImageProblem, bool)>, path: &Path| match read {
            Ok(Read { pixels, text }) => {
                let is_text = text.is_some();
                texts.extend(text.map(|t| (shown(path), t)));
                Some((pixels, is_text))
            }
            Err(p) => {
                unread.push(p);
                None
            }
        };
        let Some(image) = take(read_image(&s.path)?, &s.path) else {
            continue;
        };
        let over = match s.dmg.as_deref() {
            Some(d) => match take(read_image(d)?, d) {
                Some(o) => Some(o),
                None => continue,
            },
            None => None,
        };
        all.push((s, shown(&s.path), image, over));
    }
    // A bad palette file stops the conversion once the images are known to
    // read: the shades are chosen from the palettes and every image. Pixel
    // text that doesn't parse is said beside it, but not a palette
    // reference, which can't resolve when the file didn't read (review D-4:
    // one bad line had given every such image "the palettes define none").
    let bad_palettes = made.first_error().is_some();
    made.problems.extend(
        unread
            .into_iter()
            .filter(|(_, reference)| !(bad_palettes && *reference))
            .map(|(p, _)| p),
    );
    if bad_palettes {
        made.problems.extend(font_problems);
        return Ok(Some(Converted { made, ..stopped }));
    }
    find::palette_collision(&sources, &named_palettes, palettes.shown(), shown)?;
    // Each image's DMG version, the same in every build (Ben's ruling,
    // 2026-11-08): color art's derived or overridden, indexed art's its
    // own. With or without --gbc (plan question 10) `dmg` drives the
    // cascade and the named palettes are checked.
    let gbc = target == BuildTarget::Gbc;
    let mut versioned = Vec::with_capacity(all.len());
    for (s, image, (pixels, _text), over) in all {
        let assigned = named_palettes.assignments.get(&s.name);
        let over = over.map(|o| o.0);
        let version = derive::version(assigned, &palettes, over, s, &image, shown)?;
        versioned.push((s, image, pixels, version));
    }
    // One color, one shade, in every image, overrides included.
    let colors = project_colors(versioned.iter().flat_map(|(_, _, p, v)| match v {
        Version::Override(o) => vec![p, o],
        _ => vec![p],
    }));
    // A plain build draws a `dmg` override, not the color sheet it replaces
    // (which still shaped the choice above); --gbc draws both.
    let drawn = project_colors(versioned.iter().flat_map(|(_, _, p, v)| match v {
        Version::Override(o) if !gbc => vec![o],
        Version::Override(o) => vec![p, o],
        _ => vec![p],
    }));
    let shading = shades::choose(&palettes, named_palettes.dmg.as_ref(), &colors)
        .drawing(&drawn)
        .pinned(&named_palettes.pins);
    made.shades = Some(shading.choice);
    made.dmg_palettes = shading.named(&named_palettes.palettes);
    made.color_shades = shading.color_shades(&named_palettes.palettes);
    // Colors shared across images are named only among DMG art: a derived
    // version's shares are its own image's (a plain build) or its tiles'
    // and objects' (--gbc, plan question 2), in `derive`.
    let dmg_art: Vec<(String, &Pixels, Option<&str>)> = versioned
        .iter()
        .filter_map(|(s, image, p, v)| {
            let palette = named_palettes.assignments.get(&s.name).map(String::as_str);
            match v {
                Version::Art => Some((image.clone(), p, palette)),
                Version::Override(o) => Some((shown(s.dmg.as_deref()?), o, palette)),
                Version::Derived => None,
            }
        })
        .collect();
    let dmg_colors = project_colors(dmg_art.iter().map(|(_, p, _)| *p));
    let held: Vec<shared::Held> = dmg_art
        .iter()
        .map(|(name, pixels, palette)| shared::Held {
            name,
            pixels,
            palette: *palette,
        })
        .collect();
    let across = shared::across(&held, &dmg_colors, &shading);
    let grays = derive::gray_shading(&palettes);
    // Every sheet's object size, when the project sets one, and the
    // setting as a problem names it.
    let objects_setting = palettes.objects();
    let objects = objects.map(|size| (size, objects_setting.as_str()));

    let mut plans: Vec<Checked> = Vec::new();
    for (s, image, pixels, version) in versioned {
        // A manifest's `dmg` is any file; name the one given. Without one, a
        // manifest reads none beside `file`, so its remedies name the field.
        let dmg_shown = match (&s.dmg, origin) {
            (Some(d), _) => shown(d),
            (None, Origin::Project) => shown(&find::dmg_path(&s.path)),
            (None, Origin::Manifest) => format!(
                "a PNG named by the `dmg` field of {} in the manifest",
                s.name
            ),
        };
        let frames = sheet::Frames {
            named: s.frame,
            objects,
            resident: s.resident,
        };
        // The DMG version the checks read, under the name they give it,
        // and color art's color image, whose tiles pair with it under
        // --gbc.
        let (checked_name, dmg, shading, color) = match version {
            Version::Art => (image.clone(), pixels, &shading, None),
            Version::Override(o) => {
                let wrong = derive::override_size(&image, &dmg_shown, &pixels, &o)
                    .or_else(|| derive::override_alpha(&image, &dmg_shown, s.kind, &pixels, &o));
                if let Some(p) = wrong {
                    made.problems.push(p);
                    continue;
                }
                (dmg_shown, o, &shading, Some(pixels))
            }
            Version::Derived => {
                let object = frames.object();
                let derived = derive::derive(
                    &image, &dmg_shown, s.kind, object, &pixels, &shading, target,
                );
                match derived {
                    Ok(d) => {
                        made.problems.extend(d.warnings);
                        (image.clone(), d.pixels, &grays, Some(pixels))
                    }
                    Err(p) => {
                        made.problems.push(p);
                        continue;
                    }
                }
            }
        };
        let drawn = color.as_ref().unwrap_or(&dmg);
        // Only --gbc makes a GBC version.
        let joint = color.as_ref().filter(|_| gbc).map(|pixels| Color {
            image: &image,
            pixels,
        });
        let checked = match s.kind {
            ImageKind::Bg | ImageKind::Tiles => {
                bg::check(&checked_name, s.kind, &dmg, shading, joint).map(Plan::Bg)
            }
            ImageKind::Sprites => {
                let name = &checked_name;
                sheet::check(name, &s.stem, &dmg, frames, shading, drawn, joint).map(Plan::Sheet)
            }
        };
        let color = color.filter(|_| gbc);
        match checked {
            Ok(plan) => {
                made.problems.extend(plan.warnings().iter().cloned());
                plans.push(Checked {
                    s,
                    image,
                    plan,
                    color,
                });
            }
            Err(problems) => made.problems.extend(problems),
        }
    }
    made.problems.extend(across);
    made.problems.extend(vram::vram(&plans));
    if objects.is_none() {
        made.problems
            .extend(vram::object_sizes(&plans, &palettes.add_objects()));
    }
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
        .extend(indexed.as_ref().and_then(|i| i.problem(palettes.shown())));
    if target == BuildTarget::Gbc {
        let assigned: Vec<(String, ImageKind)> = sources
            .iter()
            .filter(|s| indexed::assigned(&named_palettes, &s.name).is_some())
            .map(|s| (shown(&s.path), s.kind))
            .collect();
        made.problems
            .extend(indexed::unpinned(&named_palettes, &assigned, &shading));
    }
    locate(&mut made.problems, &texts);
    made.problems.extend(font_problems);
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
            locate(&mut made.problems, &texts);
            return Ok(Some(Converted { made, ..stopped }));
        }
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
        root: place.dir,
        shown,
        indexed: indexed.as_ref(),
        packed: &packed,
        obj_palettes: gbc_made.as_ref().map_or(&[], |g| &g.obj_palettes),
    };
    for (i, c) in plans.iter().enumerate() {
        if let Some(asset) = out.image(i, c)? {
            made.images.push(asset);
        }
    }
    let generated = match place.generated {
        Some(from_project) => {
            let shared = emit::Shared {
                gbc: gbc_made.as_ref(),
                named: &named_palettes.palettes,
                objects,
                fonts: &font_entries,
            };
            let data: Vec<String> = plans
                .iter()
                .map(|c| format!("{from_project}/{}/{}", c.s.kind.dir(), c.s.stem))
                .collect();
            // The generated files come from the results (`made.images`, in
            // the plans' order), so the include and the protocol can't
            // disagree.
            let entries = entries(&plans, &data, &made);
            let written = write_generated(place, &entries, &shared, shown)?;
            drop(entries);
            written.map(|(generated, include)| {
                made.include = Some(include);
                generated
            })
        }
        None => None,
    };
    made.gbc = gbc_made;
    Ok(Some(Converted {
        made,
        diagnostics: Vec::new(),
        generated,
        stopped: false,
    }))
}

/// Writes `images.asm` and `images.inc` for a build (`place.generated`),
/// with the include as shown; nothing for `assets`.
fn write_generated(
    place: &Place,
    entries: &[Entry],
    shared: &emit::Shared,
    shown: &dyn Fn(&Path) -> String,
) -> Result<Option<(Generated, String)>, BuildError> {
    let Some(from_project) = place.generated else {
        return Ok(None);
    };
    let io = |path: PathBuf| move |error| BuildError::Io { path, error };
    fs::create_dir_all(place.dir).map_err(io(place.dir.to_path_buf()))?;
    let (asm, inc) = (place.dir.join("images.asm"), place.dir.join("images.inc"));
    fs::write(&asm, emit::asm(entries, shared)).map_err(io(asm.clone()))?;
    fs::write(&inc, emit::inc(entries, shared)).map_err(io(inc.clone()))?;
    let include = shown(&inc);
    Ok(Some((
        Generated {
            source: asm,
            include_dir: PathBuf::from(from_project),
        },
        include,
    )))
}

/// Pixel text that doesn't read (or names a palette entry the palettes
/// lack), as its image's problem.
fn text_problem(image: &str, e: gbcli_pixels::ParseError) -> ImageProblem {
    ImageProblem {
        line: Some(e.line),
        column: std::num::NonZeroU32::new(e.column),
        ..census::error(image, ImageLimit::PixelsText, 1, 1, e.message, &e.remedy)
    }
}

/// Gives each problem on a pixel text image's tiles the line that drew
/// the first tile it names (`drew`): a rows block's header, a map's row,
/// or `fill`.
fn locate(problems: &mut [ImageProblem], texts: &[(String, gbcli_pixels::Picture)]) {
    for p in problems.iter_mut().filter(|p| p.line.is_none()) {
        let picture = texts
            .iter()
            .find(|(name, _)| p.image.as_deref() == Some(name.as_str()))
            .map(|(_, picture)| picture);
        if let (Some(picture), Some(t)) = (picture, p.tiles.first()) {
            p.line = drew(picture, t.x * 8, t.y * 8);
        }
    }
}

/// The line that drew the 8x8 tile at (`left`, `top`): of the lines that
/// drew its pixels, the one that drew the most of its legend characters,
/// the later on a tie. A hardware problem is about a tile's colors, so
/// this is the block that drew them, not a `fill` (or earlier block) that
/// shows only at the tile's corner.
fn drew(picture: &gbcli_pixels::Picture, left: u32, top: u32) -> Option<u32> {
    use std::collections::{BTreeMap, BTreeSet};
    let mut by_line: BTreeMap<u32, BTreeSet<u16>> = BTreeMap::new();
    let (right, bottom) = ((left + 8).min(picture.width), (top + 8).min(picture.height));
    for y in top..bottom {
        for x in left..right {
            if let Some(line) = picture.line_at(x, y) {
                let symbol = picture.pixels[(y * picture.width + x) as usize];
                by_line.entry(line).or_default().insert(symbol);
            }
        }
    }
    // Lines ascend, so `max_by_key` keeps the later of equals.
    by_line
        .into_iter()
        .max_by_key(|(_, symbols)| symbols.len())
        .map(|(line, _)| line)
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
