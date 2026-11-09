//! One checked image converted: its DMG data by rgbgfx, under `--gbc` its
//! GBC data (`gbc`, for color art; indexed art shares its DMG tiles), its
//! preview, and its result. Color art's palettes are packed for every
//! image first (`packed`), since the sheets share the OBJ palettes.

use std::fs;
use std::path::{Path, PathBuf};

use gbcli_protocol::{
    GbcImage, GbcPalette, ImageArt, ImageAsset, ImageKind, ImageProblem, PaletteSlot, TileEntry,
};

use super::gbc::{self, Joint, Palettes};
use super::indexed::Indexed;
use super::pack::{self, Layer, Packing};
use super::pixels::Pixels;
use super::preview::{self, Look};
use super::run::Rgbgfx;
use super::{Checked, Plan, Removed, shared};
use crate::project::BuildError;

/// Color art's packed palettes: each color background's own, and the OBJ
/// palettes every color sheet shares, after the assigned ones.
pub(super) struct Packed {
    /// Per checked image, a color background's packing.
    bg: Vec<Option<Packing>>,
    /// The color sheets' packing, and which checked image each source is.
    obj: Option<(Packing, Vec<usize>)>,
}

impl Packed {
    /// The OBJ palettes packed for color sheets, in slot order.
    pub fn obj_palettes(&self) -> &[GbcPalette] {
        self.obj.as_ref().map_or(&[], |(p, _)| &p.palettes)
    }
}

/// Packs the GBC palettes of `checked`'s color art; `reserved` OBJ slots
/// are the assigned ones of indexed sheets. Run after every image's checks
/// passed, `census::alpha` among them: the packer counts any pixel not
/// fully transparent as opaque.
pub(super) fn packed(checked: &[Checked], reserved: usize) -> Result<Packed, Vec<ImageProblem>> {
    let mut problems = Vec::new();
    let mut bg = Vec::with_capacity(checked.len());
    for c in checked {
        let packing = match (&c.plan, &c.color) {
            (Plan::Bg(_), Some(color)) => {
                let source = pack::Source {
                    image: &c.image,
                    pixels: color,
                    cell_height: 8,
                };
                match pack::pack(&[source], Layer::Bg, 0) {
                    Ok(p) => Some(p),
                    Err(found) => {
                        problems.extend(found);
                        None
                    }
                }
            }
            _ => None,
        };
        bg.push(packing);
    }
    let sheets: Vec<usize> = (0..checked.len())
        .filter(|&i| matches!(checked[i].plan, Plan::Sheet(_)) && checked[i].color.is_some())
        .collect();
    let sources: Vec<pack::Source> = sheets
        .iter()
        .map(|&i| pack::Source {
            image: &checked[i].image,
            pixels: checked[i].color.as_ref().expect("a color sheet"),
            cell_height: match &checked[i].plan {
                Plan::Sheet(p) => p.object.height(),
                Plan::Bg(_) => 8,
            },
        })
        .collect();
    let obj = if sources.is_empty() {
        None
    } else {
        match pack::pack(&sources, Layer::Obj, reserved) {
            Ok(p) => Some((p, sheets)),
            Err(found) => {
                problems.extend(found);
                None
            }
        }
    };
    if problems.is_empty() {
        Ok(Packed { bg, obj })
    } else {
        Err(problems)
    }
}

/// What every image's conversion shares.
pub(super) struct Output<'a> {
    pub rgbgfx: Rgbgfx<'a>,
    /// Where the data and previews go, by kind (`pipeline::Place::dir`).
    pub root: &'a Path,
    pub shown: &'a dyn Fn(&Path) -> String,
    pub indexed: Option<&'a Indexed<'a>>,
    pub packed: &'a Packed,
    /// Every OBJ palette, assigned then packed: what a sheet's slots index.
    pub obj_palettes: &'a [GbcPalette],
}

impl Output<'_> {
    /// Converts checked image `i`, whose DMG version is `pixels` (in the
    /// file `file`, if any); its result, or `None` when rgbgfx failed or
    /// disagreed (`rgbgfx`'s diagnostics and problems say why).
    pub fn image(
        &mut self,
        i: usize,
        c: &Checked,
        pixels: &Pixels,
        file: Option<&PathBuf>,
    ) -> Result<Option<ImageAsset>, BuildError> {
        let (s, image, root) = (c.s, c.image.as_str(), self.root);
        let under_assets = s.name.as_str();
        // Relative to `root`, rgbgfx's working directory; the images it
        // reads are named whole, wherever they are.
        let data = format!("{}/{}", s.kind.dir(), s.stem);
        // rgbgfx reads the DMG version's file, or, when colors share a
        // shade or the version was derived, a copy (with them merged),
        // removed once converted.
        let merged = PathBuf::from(format!("{data}.merged.png"));
        let transient = if c.plan.merges().is_empty() && file.is_some() {
            None
        } else {
            // Guarded before it is written, so a failed write leaves no
            // partial file behind.
            let removed = Removed(root.join(&merged));
            shared::write(pixels, c.plan.merges(), &removed.0).map_err(io(&removed.0))?;
            Some(removed)
        };
        let rel = match (&transient, file) {
            (None, Some(file)) => file.as_path(),
            _ => merged.as_path(),
        };
        // rgbgfx reads files: a color image in pixel text is written out
        // as a PNG first, removed once converted.
        let color_png = PathBuf::from(format!("{data}.color.png"));
        let _color_written = match &c.color {
            Some(color) if c.color_text => {
                let removed = Removed(root.join(&color_png));
                shared::write(color, &[], &removed.0).map_err(io(&removed.0))?;
                Some(removed)
            }
            _ => None,
        };
        let color_file = if c.color_text {
            color_png.as_path()
        } else {
            s.path.as_path()
        };
        let preview_path = root.join(format!("{data}.preview.png"));
        // A tileset is a background's tiles without its map: the program
        // builds its own maps, from the result's tile map.
        let has_map = s.kind == ImageKind::Bg;
        let (mut asset, picture, gbc_picture) = match &c.plan {
            Plan::Bg(p) => {
                let Some(d) = self.rgbgfx.background(image, rel, p, &data)? else {
                    return Ok(None);
                };
                if has_map {
                    let path = root.join(format!("{data}.tilemap"));
                    fs::write(&path, &d.map).map_err(io(&path))?;
                }
                let mut asset = asset(c, p.width, p.height, p.tiles, None, None);
                let mut slots = vec![None; p.unique.len()];
                let mut gbc_tiles = None;
                if let (Some(color), Some(packing)) = (&c.color, &self.packed.bg[i]) {
                    let palettes = Palettes {
                        palettes: &packing.palettes,
                        first_slot: packing.first_slot,
                        layer: Layer::Bg,
                    };
                    let cells = &packing.cell_palettes[0];
                    let rgbgfx = &mut self.rgbgfx;
                    let made = gbc::background(
                        rgbgfx, image, color_file, p, color, &palettes, cells, &data,
                    )?;
                    let Some(joint) = made else { return Ok(None) };
                    self.write_gbc(&data, &joint)?;
                    slots = joint.slots.into_iter().map(Some).collect();
                    gbc_tiles = Some(joint.tiles);
                    asset.gbc = Some(packing.gbc_image(Layer::Bg));
                }
                asset.tile_map = p
                    .cells
                    .iter()
                    .map(|&u| {
                        Some(TileEntry {
                            index: u as u32,
                            flip: Default::default(),
                            palette: slots[u],
                        })
                    })
                    .collect();
                if let Some(x) = self.indexed {
                    x.fill_background(under_assets, &d.map, &mut asset);
                }
                let picture =
                    preview::background(image, &d.tiles, &d.map, p.width, p.height, &Look::Dmg);
                let gbc_picture = match &asset.gbc {
                    Some(g) => {
                        // The attribute map: each map entry's slot.
                        if has_map {
                            let attr: Vec<u8> = asset.tile_map.iter().map(slot_of).collect();
                            let path = root.join(format!("{data}.attrmap"));
                            fs::write(&path, &attr).map_err(io(&path))?;
                        }
                        let slots = per_tile(&asset.tile_map, p.unique.len(), |e| e.index);
                        let look = Look::Gbc {
                            palettes: &g.palettes,
                            slots: &slots,
                        };
                        // Indexed art draws its DMG tiles on both machines.
                        let tiles = gbc_tiles.as_deref().unwrap_or(&d.tiles);
                        Some(preview::background(
                            image, tiles, &d.map, p.width, p.height, &look,
                        ))
                    }
                    None => None,
                };
                (asset, picture, gbc_picture)
            }
            Plan::Sheet(p) => {
                let Some(units) = self.rgbgfx.sheet(image, rel, p, &data)? else {
                    return Ok(None);
                };
                if !units.is_empty() {
                    let path = root.join(format!("{data}.2bpp"));
                    fs::write(&path, &units).map_err(io(&path))?;
                }
                let objects = p.frames.iter().map(Vec::len).max().unwrap_or(0) as u32;
                let frames = Some(p.frames.len() as u32);
                let mut asset = asset(c, p.width, p.height, p.tiles(), frames, Some(objects));
                let mut slots: Vec<Option<PaletteSlot>> = vec![None; p.kept.len()];
                let mut gbc_units = None;
                if let (Some(color), Some((packing, sheets))) = (&c.color, &self.packed.obj) {
                    let at = sheets.iter().position(|&k| k == i).expect("a packed sheet");
                    let palettes = Palettes {
                        palettes: &packing.palettes,
                        first_slot: packing.first_slot,
                        layer: Layer::Obj,
                    };
                    let cells = &packing.cell_palettes[at];
                    let rgbgfx = &mut self.rgbgfx;
                    let made =
                        gbc::sheet(rgbgfx, image, color_file, p, color, &palettes, cells, &data)?;
                    let Some(joint) = made else { return Ok(None) };
                    self.write_gbc(&data, &joint)?;
                    slots = joint.slots.into_iter().map(Some).collect();
                    gbc_units = Some(joint.tiles);
                    asset.gbc = Some(GbcImage {
                        preview: String::new(),
                        art: ImageArt::Color,
                        palettes: Vec::new(),
                    });
                }
                asset.tile_map = p.tile_map(|unit| slots[unit]);
                if let Some(x) = self.indexed {
                    x.fill_sheet(under_assets, p, &mut asset);
                }
                let picture = preview::sheet(image, &units, p, &Look::Dmg);
                let gbc_picture = asset.gbc.as_ref().map(|_| {
                    let per = p.object.tiles();
                    let slots = per_tile(&asset.tile_map, p.kept.len(), |e| e.index / per);
                    let look = Look::Gbc {
                        palettes: self.obj_palettes,
                        slots: &slots,
                    };
                    preview::sheet(image, gbc_units.as_deref().unwrap_or(&units), p, &look)
                });
                (asset, picture, gbc_picture)
            }
        };
        if let (Some(g), Some(picture)) = (&mut asset.gbc, gbc_picture) {
            let path = root.join(format!("{data}.gbc.preview.png"));
            write_png(&path, &picture)?;
            g.preview = (self.shown)(&path);
        }
        write_png(&preview_path, &picture)?;
        Ok(Some(ImageAsset {
            preview: (self.shown)(&preview_path),
            ..asset
        }))
    }

    /// A GBC version's tiles, beside the DMG data, when it has any.
    fn write_gbc(&self, data: &str, joint: &Joint) -> Result<(), BuildError> {
        if joint.tiles.is_empty() {
            return Ok(());
        }
        let path = self.root.join(format!("{data}.gbc.2bpp"));
        fs::write(&path, &joint.tiles).map_err(io(&path))
    }
}

fn write_png(path: &Path, picture: &gbcli_view::Image) -> Result<(), BuildError> {
    let png = picture.png().map_err(|e| BuildError::Io {
        path: path.to_path_buf(),
        error: std::io::Error::other(e.to_string()),
    })?;
    fs::write(path, png).map_err(io(path))
}

/// An entry's GBC slot, 0 where it has none (a transparent unit).
fn slot_of(entry: &Option<TileEntry>) -> u8 {
    entry.and_then(|e| e.palette).map_or(0, PaletteSlot::get)
}

/// The slot of each of `n` tiles (or kept units), from the tile map
/// entries that name it (`which`).
fn per_tile(map: &[Option<TileEntry>], n: usize, which: impl Fn(&TileEntry) -> u32) -> Vec<u8> {
    let mut slots = vec![0; n];
    for e in map.iter().flatten() {
        slots[which(e) as usize] = e.palette.map_or(0, PaletteSlot::get);
    }
    slots
}

fn io(path: &Path) -> impl FnOnce(std::io::Error) -> BuildError + '_ {
    move |error| BuildError::Io {
        path: path.to_path_buf(),
        error,
    }
}

fn asset(
    c: &Checked,
    width: u32,
    height: u32,
    tiles: u32,
    frames: Option<u32>,
    objects: Option<u32>,
) -> ImageAsset {
    ImageAsset {
        kind: c.s.kind,
        source: c.image.clone(),
        label: c.s.label.clone(),
        width,
        height,
        tiles,
        frames,
        objects,
        object_size: match &c.plan {
            Plan::Sheet(p) => Some(p.object),
            Plan::Bg(_) => None,
        },
        preview: String::new(),
        tile_map: Vec::new(),
        gbc: None,
    }
}
