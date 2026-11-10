//! The encoder's differential test (Ben's ruling, 2026-11-09). rgbgfx 1.0.4
//! wrote gbcli's image data until then; the build now encodes it in Rust
//! (`crate::images::encode`, `gbc`) and rgbgfx is this test's oracle only.
//!
//! Every example game's images, and a fixture of each kind and art (a plain
//! background, a tileset, 8x16 and 8x8 sheets, shared shades, derived color
//! art, GBC multi-palette art and a 16-bit PNG), are converted by `assets`,
//! which runs no tool. Then real rgbgfx reads the source's own pixels, and
//! each cell gbcli's data draws must be the tile rgbgfx wrote for that place
//! in the image:
//!
//! - DMG: rgbgfx reads the source (its `name.dmg.png` when it has one), each
//!   color repainted as the one `-c` lists for the index gbcli gave it, so
//!   two colors sharing a shade are one to rgbgfx too. Its tile there must
//!   be gbcli's byte for byte. gbcli's tile order, map, sheet unit order and
//!   2bpp encoding never reach rgbgfx's input. gbcli's color -> index choice
//!   does: a uniform relabeling of indices passes this comparison. Where the
//!   project's shade rule is a known palette, a background's (or tileset's)
//!   index must also be its color's shade in that palette, which a relabeling
//!   breaks.
//! - GBC: rgbgfx reads the source PNG itself (so it decodes 16-bit samples
//!   itself) with gbcli's palettes, and each pixel must draw the same color
//!   (rgbgfx may pick another palette that holds a tile's colors, research
//!   §3; gbcli always draws from the slot it packed).
//!
//! What it cannot see: an image whose source isn't a PNG (`.pixels` text,
//! 3 of 5 example games): its DMG data is not compared, and its GBC data is
//! compared with a PNG painted from gbcli's own data, a round trip. A
//! sheet's indices are not checked against its OBP's shades. And the
//! fixtures other image tests build live in their own temporary
//! directories, so this file builds the same shapes again.

use crate::common;
use crate::pngs;

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use gbcli_build::{AssetsInput, AssetsOptions, assets};
use gbcli_protocol::{
    AssetsMade, BuildTarget, Flip, GbcPalette, ImageArt, ImageAsset, ImageKind, ImagesMade,
    KnownPalette, Rgb555, ShadeChoice, ShadeRule,
};
use pngs::{BLACK, CLEAR, DARK, LIGHT, Rgba, WHITE, write_png, write_png16};

/// The rgbgfx this test compares with: the pinned 1.0.4 from `PATH`.
fn rgbgfx() -> PathBuf {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let found = std::env::split_paths(&path)
        .map(|dir| dir.join("rgbgfx"))
        .find(|p| p.is_file())
        .expect("this differential test needs rgbgfx 1.0.4 on PATH");
    let out = Command::new(&found).arg("--version").output().unwrap();
    let version = String::from_utf8_lossy(&out.stdout);
    assert!(
        version.trim() == "rgbgfx v1.0.4",
        "this differential test needs rgbgfx 1.0.4; {} is {version}",
        found.display()
    );
    found
}

/// Converts `project` for the GBC, keeping its data in `keep`.
fn convert(project: &Path, base: &Path, keep: &Path) -> ImagesMade {
    fs::create_dir_all(keep).unwrap();
    let mut options = AssetsOptions::new(
        AssetsInput::Project(project.to_path_buf()),
        base.to_path_buf(),
    );
    options.target = BuildTarget::Gbc;
    options.keep = Some(keep.to_path_buf());
    let made: AssetsMade = assets(&options).unwrap_or_else(|e| panic!("{:?}", e.body()));
    made.images
}

/// The data's name under `kind.dir()`: its source's, without a frame size.
fn stem(asset: &ImageAsset) -> String {
    let name = Path::new(&asset.source)
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let name = name
        .rsplit_once('.')
        .map_or(name.clone(), |(n, _)| n.to_string());
    if asset.kind != ImageKind::Sprites {
        return name;
    }
    match name.rsplit_once('.') {
        Some((head, size)) if is_frame_size(size) => head.to_string(),
        _ => name,
    }
}

fn is_frame_size(size: &str) -> bool {
    size.split_once('x').is_some_and(|(w, h)| {
        !w.is_empty() && !h.is_empty() && w.parse::<u32>().is_ok() && h.parse::<u32>().is_ok()
    })
}

fn data_path(keep: &Path, asset: &ImageAsset, ext: &str) -> PathBuf {
    keep.join(asset.kind.dir())
        .join(format!("{}.{ext}", stem(asset)))
}

/// 2bpp `data`, a whole number of tiles, as color indices 8 a row.
fn decode(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() / 2 * 8);
    for row in 0..data.len() / 2 {
        out.extend((0..8).map(|x| pngs::index_at(data, row, x)));
    }
    out
}

/// `pixels` with `flip` applied, as the hardware flips an object: 8 wide.
fn flipped(pixels: &[u8], flip: Flip) -> Vec<u8> {
    let bottom = (pixels.len() / 8).saturating_sub(1);
    (0..pixels.len())
        .map(|i| {
            let (x, y) = (i % 8, i / 8);
            let x = if flip.x() { 7 - x } else { x };
            let y = if flip.y() { bottom - y } else { y };
            pixels[y * 8 + x]
        })
        .collect()
}

/// One cell of an image: where it is in 8x8 tiles, the palette slot it
/// draws from, and the pixels it draws, top first.
struct Cell {
    x: u32,
    y: u32,
    slot: Option<u8>,
    pixels: Vec<u8>,
}

/// Every cell of `asset` as the data draws it; `per` is the tiles its object
/// takes (1 for a background's tile).
fn cells(asset: &ImageAsset, tiles: &[u8], per: u32) -> Vec<Cell> {
    asset
        .tile_map
        .iter()
        .enumerate()
        .filter_map(|(i, entry)| {
            let entry = entry.as_ref()?;
            let start = entry.index as usize * 16;
            let pixels = flipped(
                &decode(&tiles[start..start + 16 * per as usize]),
                entry.flip,
            );
            Some(Cell {
                x: i as u32 % asset.width,
                y: i as u32 / asset.width * per,
                slot: entry.palette.map(|s| s.get()),
                pixels,
            })
        })
        .collect()
}

/// The rgbgfx tile for (`x`, `y`) of an image `width` x `height` tiles:
/// reading order, or a sheet's `-Z` column by column.
fn oracle(out: &[u8], sheet: bool, width: u32, height: u32, x: u32, y: u32) -> &[u8] {
    let index = if sheet {
        (x * height + y) as usize
    } else {
        (y * width + x) as usize
    };
    &out[index * 16..index * 16 + 16]
}

/// What rgbgfx wrote for `png` with `spec`, in the order it writes it.
fn run_rgbgfx(
    png: &Path,
    spec: &str,
    sheet: bool,
    attrs: bool,
    into: &Path,
) -> (Vec<u8>, Option<Vec<u8>>) {
    let tiles = into.join("oracle.2bpp");
    let attr = into.join("oracle.attrmap");
    let mut cmd = Command::new(rgbgfx());
    cmd.arg("--color").arg("never").arg("-c").arg(spec);
    if sheet {
        cmd.arg("-Z");
    }
    cmd.arg("-o").arg(&tiles);
    if attrs {
        cmd.arg("-a").arg(&attr);
    }
    cmd.arg(png);
    let out = cmd.output().unwrap();
    assert!(
        out.status.success(),
        "rgbgfx failed on {}: {}",
        png.display(),
        String::from_utf8_lossy(&out.stderr)
    );
    let got = fs::read(&tiles).unwrap();
    let attrs = attrs.then(|| fs::read(&attr).unwrap());
    let _ = fs::remove_file(&tiles);
    let _ = fs::remove_file(&attr);
    (got, attrs)
}

fn hex(c: Rgba) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

/// An image's pixels as RGBA, expanded from any PNG encoding; 16-bit
/// samples are rounded to 8 bits as libpng's `png_set_scale_16` does, which
/// is how rgbgfx reads them.
fn read_rgba(path: &Path) -> (u32, u32, Vec<Rgba>) {
    let mut decoder = png::Decoder::new(fs::File::open(path).unwrap());
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).unwrap();
    let n = info.color_type.samples();
    let samples: Vec<u8> = match info.bit_depth {
        png::BitDepth::Sixteen => buf[..info.buffer_size()]
            .chunks(2)
            .map(|b| pngs::scale_16(u16::from_be_bytes([b[0], b[1]])))
            .collect(),
        _ => buf[..info.buffer_size()].to_vec(),
    };
    let pixels = samples
        .chunks(n)
        .map(|p| match n {
            1 => [p[0], p[0], p[0], 0xFF],
            2 => [p[0], p[0], p[0], p[1]],
            3 => [p[0], p[1], p[2], 0xFF],
            _ => [p[0], p[1], p[2], p[3]],
        })
        .collect();
    (info.width, info.height, pixels)
}

/// A known palette's colors, shade 0 first, as `KnownPalette` documents
/// them: written out here so the shade check doesn't take them from gbcli.
fn known_colors(palette: KnownPalette) -> [Rgba; 4] {
    let c = |v: u32| [(v >> 16) as u8, (v >> 8) as u8, v as u8, 0xFF];
    let words = match palette {
        KnownPalette::DmgGrays => [0xffffff, 0xaaaaaa, 0x555555, 0x000000],
        KnownPalette::PeaGreens => [0x9bbc0f, 0x8bac0f, 0x306230, 0x0f380f],
        KnownPalette::GbStudioGreens => [0xe0f8cf, 0x86c06c, 0x306850, 0x071821],
        KnownPalette::PocketGrays => [0xc2ce93, 0x818d66, 0x3a4c3a, 0x07100e],
    };
    words.map(c)
}

/// A color's RGB555 key, or none for a transparent pixel.
fn key(c: Rgba) -> Option<(u8, u8, u8)> {
    (c[3] != 0).then_some((c[0] >> 3, c[1] >> 3, c[2] >> 3))
}

/// The image rgbgfx reads for `asset`'s DMG data: its `name.dmg.png` when
/// it has one, else its source; none when that isn't a PNG of its size.
fn dmg_source(base: &Path, asset: &ImageAsset) -> Option<Vec<Rgba>> {
    let given = Path::new(&asset.source);
    let name = given.file_name().unwrap().to_string_lossy().into_owned();
    let stem = name
        .rsplit_once('.')
        .map_or(name.clone(), |(n, _)| n.to_string());
    let dmg = base.join(given.with_file_name(format!("{stem}.dmg.png")));
    let source = if dmg.exists() { dmg } else { base.join(given) };
    if source.extension().is_none_or(|e| e != "png") {
        return None;
    }
    let (pw, ph, pixels) = read_rgba(&source);
    ((pw, ph) == (asset.width * 8, asset.height * 8)).then_some(pixels)
}

/// Checks the DMG data of `asset` against rgbgfx reading the source's own
/// pixels, appending mismatches. gbcli's data gives only a color -> index
/// mapping, which must be a function of the RGB555 color; every pixel of
/// rgbgfx's input is drawn from the source at its own place, so the tile it
/// writes for a cell is independent of gbcli's tile order, map, unit order
/// and 2bpp encoding. With `shades` a known palette, a background's indices
/// must also be their colors' shades in it (BGP $E4).
fn check_dmg(
    base: &Path,
    keep: &Path,
    asset: &ImageAsset,
    shades: Option<ShadeChoice>,
    scratch: &Path,
    bad: &mut Vec<String>,
) {
    let sheet = asset.kind == ImageKind::Sprites;
    let per = asset.object_size.map_or(1, |size| size.tiles());
    let Ok(tiles) = fs::read(data_path(keep, asset, "2bpp")) else {
        return;
    };
    let cells = cells(asset, &tiles, per);
    if cells.is_empty() {
        return;
    }
    let Some(real) = dmg_source(base, asset) else {
        return;
    };
    let w = asset.width * 8;
    let known = match shades {
        Some(ShadeChoice {
            rule: ShadeRule::Known,
            palette: Some(p),
        }) if !sheet => Some(known_colors(p).map(|c| key(c).unwrap())),
        _ => None,
    };
    let mut map: HashMap<(u8, u8, u8), u8> = HashMap::new();
    let mut split: HashSet<(u8, u8, u8)> = HashSet::new();
    let mut spec: [Option<Rgba>; 4] = [None; 4];
    for cell in &cells {
        for (n, &index) in cell.pixels.iter().enumerate() {
            let (x, y) = (cell.x * 8 + n as u32 % 8, cell.y * 8 + n as u32 / 8);
            let c = real[(y * w + x) as usize];
            let Some(k) = key(c) else { continue };
            // Once a color: the map check below covers its other pixels.
            if !map.contains_key(&k)
                && let Some(shade) = known.and_then(|colors| colors.iter().position(|&s| s == k))
                && shade != index as usize
            {
                bad.push(format!(
                    "{}: color {} at ({x}, {y}) is shade {shade} but index {index}",
                    asset.source,
                    hex(c)
                ));
            }
            match map.get(&k) {
                Some(&was) if was != index => {
                    // Once a color, at its first other index.
                    if split.insert(k) {
                        bad.push(format!(
                            "{}: color {} at ({x}, {y}) is index {index} here, {was} elsewhere",
                            asset.source,
                            hex(c)
                        ));
                    }
                }
                Some(_) => {}
                None => {
                    map.insert(k, index);
                    spec[index as usize].get_or_insert(c);
                }
            }
        }
    }
    if !split.is_empty() {
        // No one color an index: there is no image to show rgbgfx.
        return;
    }
    let spec_text = (0..4)
        .map(|i| match spec[i] {
            Some(c) if !(sheet && i == 0) => hex(c),
            _ => "#none".into(),
        })
        .collect::<Vec<_>>()
        .join(",");
    let png = scratch.join("oracle-dmg.png");
    write_png(&png, w, asset.height * 8, |x, y| {
        let c = real[(y * w + x) as usize];
        match key(c).and_then(|k| map.get(&k)) {
            Some(&i) => spec[i as usize].unwrap(),
            None => CLEAR,
        }
    });
    let (out, _) = run_rgbgfx(&png, &spec_text, sheet, false, scratch);
    for cell in &cells {
        for t in 0..per {
            let got = decode(oracle(
                &out,
                sheet,
                asset.width,
                asset.height,
                cell.x,
                cell.y + t,
            ));
            let want = &cell.pixels[64 * t as usize..64 * (t as usize + 1)];
            if got != want {
                bad.push(format!(
                    "{}: DMG tile ({}, {}) differs from rgbgfx on the source (spec {spec_text})",
                    asset.source,
                    cell.x,
                    cell.y + t
                ));
            }
        }
    }
}

/// The color a GBC palette's index draws; an object's index 0 is
/// transparent.
fn color_of(palettes: &[GbcPalette], slot: u8, obj: bool, index: u8) -> Option<Rgb555> {
    if obj && index == 0 {
        return None;
    }
    palettes
        .get(usize::from(slot))
        .map(|p| p.0[usize::from(index)])
}

/// Checks the GBC data of a color image against rgbgfx, by color, appending
/// mismatches. `palettes` are the ones its slots index.
fn check_gbc(
    base: &Path,
    keep: &Path,
    asset: &ImageAsset,
    palettes: &[GbcPalette],
    scratch: &Path,
    bad: &mut Vec<String>,
) {
    let Some(gbc) = &asset.gbc else { return };
    if !matches!(gbc.art, ImageArt::Color) {
        return;
    }
    let sheet = asset.kind == ImageKind::Sprites;
    let per = asset.object_size.map_or(1, |size| size.tiles());
    let Ok(tiles) = fs::read(data_path(keep, asset, "gbc.2bpp")) else {
        return;
    };
    let cells = cells(asset, &tiles, per);
    if cells.is_empty() {
        return;
    }
    let (w, h) = (asset.width * 8, asset.height * 8);
    let obj = sheet;
    // The color image rgbgfx reads: the source itself when it is a PNG of
    // the image's size, else (pixel text) a copy of the colors the data
    // draws, which only round-trips gbcli's own data.
    let source = base.join(&asset.source);
    let is_png = source.extension().is_some_and(|e| e == "png") && {
        let (pw, ph, _) = read_rgba(&source);
        (pw, ph) == (w, h)
    };
    let png = if is_png {
        source
    } else {
        let png = scratch.join("oracle-gbc.png");
        let mut drawn = vec![CLEAR; (w * h) as usize];
        for cell in &cells {
            let slot = cell.slot.expect("a GBC cell has a slot");
            for (n, &index) in cell.pixels.iter().enumerate() {
                let (x, y) = (cell.x * 8 + n as u32 % 8, cell.y * 8 + n as u32 / 8);
                if let Some(color) = color_of(palettes, slot, obj, index) {
                    let [r, g, b] = color.to_rgb888();
                    drawn[(y * w + x) as usize] = [r, g, b, 0xFF];
                }
            }
        }
        write_png(&png, w, h, |x, y| drawn[(y * w + x) as usize]);
        png
    };
    let spec = palettes
        .iter()
        .map(|GbcPalette(colors)| {
            colors
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    if obj && i == 0 {
                        "#none".into()
                    } else {
                        let [r, g, b] = c.to_rgb888();
                        format!("#{r:02x}{g:02x}{b:02x}")
                    }
                })
                .collect::<Vec<_>>()
                .join(",")
        })
        .collect::<Vec<_>>()
        .join(";");
    let (out, attrs) = run_rgbgfx(&png, &spec, sheet, true, scratch);
    let attrs = attrs.expect("rgbgfx wrote attributes");
    let (dw, dh) = (asset.width, asset.height);
    for cell in &cells {
        let slot = cell.slot.expect("a GBC cell has a slot");
        for t in 0..per {
            let (x, y) = (cell.x, cell.y + t);
            let written = oracle(&out, sheet, dw, dh, x, y);
            let attr = if sheet {
                attrs[(x * dh + y) as usize]
            } else {
                attrs[(y * dw + x) as usize]
            };
            let got = decode(written);
            let want = &cell.pixels[64 * t as usize..64 * (t as usize + 1)];
            for (n, &index) in want.iter().enumerate() {
                let p = (n as u32 % 8, n as u32 / 8);
                let theirs = color_of(palettes, attr & 7, obj, got[n]);
                let ours = color_of(palettes, slot, obj, index);
                if theirs != ours {
                    bad.push(format!(
                        "{}: GBC tile ({x}, {y}) pixel {p:?} draws {:?}, rgbgfx {:?} (slot {slot}, \
                         rgbgfx slot {})",
                        asset.source,
                        ours,
                        theirs,
                        attr & 7
                    ));
                }
            }
        }
    }
}

/// Converts `project` and checks every image against rgbgfx; the
/// mismatches, and the project's shade choice.
fn check(project: &Path, base: &Path, name: &str) -> (Vec<String>, Option<ShadeChoice>) {
    let keep = std::env::temp_dir().join(format!("gbcli-differ-{name}"));
    let _ = fs::remove_dir_all(&keep);
    let made = convert(project, base, &keep);
    let obj_palettes = made.gbc.as_ref().map_or(&[][..], |g| &g.obj_palettes);
    let mut bad = Vec::new();
    for asset in &made.images {
        let palettes = match asset.gbc.as_ref() {
            Some(_) if asset.kind == ImageKind::Sprites => obj_palettes,
            Some(g) => &g.palettes,
            None => &[],
        };
        check_dmg(base, &keep, asset, made.shades, &keep, &mut bad);
        check_gbc(base, &keep, asset, palettes, &keep, &mut bad);
        // A color background's attribute map is its tile map's slots.
        if asset.kind == ImageKind::Bg
            && asset
                .gbc
                .as_ref()
                .is_some_and(|g| matches!(g.art, ImageArt::Color))
        {
            let attr = fs::read(data_path(&keep, asset, "attrmap")).unwrap();
            let want: Vec<u8> = asset
                .tile_map
                .iter()
                .map(|e| e.as_ref().and_then(|e| e.palette).map_or(0, |s| s.get()))
                .collect();
            if attr != want {
                bad.push(format!(
                    "{}: the attribute map isn't its tile map's slots",
                    asset.source
                ));
            }
        }
    }
    let _ = fs::remove_dir_all(&keep);
    (bad, made.shades)
}

/// Every image of every example game matches rgbgfx.
#[test]
fn every_example_games_images_match_rgbgfx() {
    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let examples = fs::canonicalize(examples).unwrap();
    let mut bad = Vec::new();
    let mut games = 0;
    for game in fs::read_dir(&examples).unwrap().flatten() {
        let project = game.path();
        if !project.join("assets").is_dir() {
            continue;
        }
        games += 1;
        let name = game.file_name().to_string_lossy().into_owned();
        bad.extend(check(&project, &examples, &format!("ex-{name}")).0);
    }
    assert!(games >= 5, "{games} example games");
    assert!(bad.is_empty(), "rgbgfx disagrees:\n{}", bad.join("\n"));
}

/// A fixture of every kind and art matches rgbgfx, a 16-bit PNG among them.
#[test]
fn every_kind_and_art_matches_rgbgfx() {
    let (temp, _options) = common::starter("differ");
    let demo = temp.path("demo");
    // Indexed art: its own DMG version, one palette.
    write_png(&demo.join("assets/bg/sky.png"), 16, 8, |x, y| {
        if (x / 8 + y / 8) % 2 == 0 {
            WHITE
        } else {
            BLACK
        }
    });
    // A shared shade: near-black draws as black.
    write_png(&demo.join("assets/bg/haze.png"), 24, 8, |x, _| {
        [WHITE, BLACK, [0x10, 0x10, 0x10, 0xFF]][(x / 8) as usize]
    });
    pngs::indexed(&demo, &["bg/sky.png", "bg/haze.png"]);
    // Color art, derived: the GBC palettes come from its own tiles.
    write_png(&demo.join("assets/bg/town.png"), 16, 8, |x, _| {
        [WHITE, LIGHT, DARK, BLACK][(x / 4) as usize]
    });
    // A color sheet with four frames, two of them mirrors.
    write_png(
        &demo.join("assets/sprites/hero.16x16.png"),
        64,
        16,
        |x, y| {
            let (frame, fx) = (x / 16, x % 16);
            let figure = |x: u32, y: u32| match (x * 3 + y * 5) % 4 {
                0 => WHITE,
                1 => DARK,
                2 => BLACK,
                _ => CLEAR,
            };
            match frame {
                0 | 3 => figure(fx, y),
                1 => figure(15 - fx, y),
                _ => CLEAR,
            }
        },
    );
    // Color art whose 8x8 tiles need several palettes on the GBC.
    write_png(&demo.join("assets/bg/bands.png"), 48, 8, |x, _| {
        let n = (x / 8) as usize;
        let palette: [Rgba; 3] = [
            [0xFF, 0x00, 0x00, 0xFF],
            [0x00, 0xFF, 0x00, 0xFF],
            [0x00, 0x00, 0xFF, 0xFF],
        ];
        [
            WHITE,
            [0xAA, 0x00, 0x00, 0xFF],
            [0x00, 0xAA, 0x00, 0xFF],
            palette[n % 3],
        ][(x % 8 / 2) as usize]
    });
    // 16-bit color art whose samples sit just under an RGB555 step:
    // rounded to 8 bits (libpng's scale_16, as rgbgfx reads them) they reach
    // it, truncated they don't, so a decoder that truncates draws colors
    // rgbgfx doesn't find among gbcli's palettes.
    write_png16(&demo.join("assets/bg/deep.png"), 8, 8, |x, y| {
        [
            [0x77FF, 0x0000, 0x0000, 0xFFFF],
            [0x0000, 0xBFFF, 0x0000, 0xFFFF],
            [0x0000, 0x0000, 0xF7FF, 0xFFFF],
            [0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF],
        ][((x + y) % 4) as usize]
    });
    // An 8x8 sheet, and a tileset.
    write_png(&demo.join("assets/sprites/dot.8x8.png"), 8, 8, |x, _| {
        if x < 4 { BLACK } else { CLEAR }
    });
    write_png(&demo.join("assets/tiles/blocks.png"), 8, 8, |x, y| {
        if (x + y) % 2 == 0 { LIGHT } else { DARK }
    });
    let (bad, _) = check(&demo, &temp.0, "fixtures");
    assert!(bad.is_empty(), "rgbgfx disagrees:\n{}", bad.join("\n"));
}

/// Gray art in the DMG's own grays, so its shades are the known palette's:
/// tiles that share colors in different arrangements (no relabeling of
/// indices hides a misplaced tile), an indexed 8x16 sheet of distinct units
/// (a misordered unit shows), derived color art and a tileset. The DMG data
/// must be rgbgfx's on the source pixels, and each background index its
/// color's shade.
#[test]
fn gray_art_matches_rgbgfx_on_the_source_and_in_its_shades() {
    let (temp, _options) = common::starter("differ-grays");
    let demo = temp.path("demo");
    write_png(&demo.join("assets/bg/sky.png"), 16, 8, |x, y| {
        if (x / 8 + y / 8) % 2 == 0 {
            WHITE
        } else {
            BLACK
        }
    });
    write_png(&demo.join("assets/sprites/bugs.png"), 24, 16, |x, y| {
        let (u, fx) = (x / 8, x % 8);
        match (u + fx + y) % 4 {
            0 => CLEAR,
            1 => [DARK, WHITE, BLACK][u as usize],
            2 => WHITE,
            _ => BLACK,
        }
    });
    write_png(&demo.join("assets/bg/split.png"), 24, 8, |x, y| {
        let (t, fx) = (x / 8, x % 8);
        let dark = match t {
            0 => fx < 4,
            1 => y < 4,
            _ => fx > y,
        };
        if dark { BLACK } else { WHITE }
    });
    pngs::indexed(&demo, &["bg/sky.png", "sprites/bugs.png", "bg/split.png"]);
    write_png(&demo.join("assets/bg/town.png"), 16, 8, |x, _| {
        [WHITE, LIGHT, DARK, BLACK][(x / 4) as usize]
    });
    write_png(&demo.join("assets/tiles/blocks.png"), 16, 8, |x, y| {
        if (x / 8 + x + y) % 2 == 0 {
            LIGHT
        } else {
            DARK
        }
    });
    let (bad, shades) = check(&demo, &temp.0, "grays");
    assert_eq!(
        shades,
        Some(ShadeChoice {
            rule: ShadeRule::Known,
            palette: Some(KnownPalette::DmgGrays),
        }),
        "the shade check needs the known grays"
    );
    assert!(
        bad.is_empty(),
        "rgbgfx disagrees on the source:\n{}",
        bad.join("\n")
    );
}

/// The build runs no rgbgfx: one that records being called and fails is
/// never run. `assets` is not tested this way: it takes no toolchain and
/// no `PATH`, and the protocol has no rgbgfx `Tool` to discover (Ben's
/// ruling, 2026-11-09), so it has no way to find one.
#[test]
fn a_build_never_runs_rgbgfx() {
    use gbcli_protocol::Tool;
    let (temp, options) = common::starter("differ-no-call");
    let marker = temp.path("rgbgfx-ran");
    let body = format!("touch '{}'; exit 1", marker.display());
    let scripts = [
        ("rgbasm", common::real(Tool::Rgbasm, "")),
        ("rgblink", common::real(Tool::Rgblink, "")),
        ("rgbfix", common::real(Tool::Rgbfix, "")),
        ("rgbgfx", body),
    ];
    let scripts: Vec<(&str, &str)> = scripts.iter().map(|(n, b)| (*n, b.as_str())).collect();
    let path = common::fake_tools(&temp, &scripts);
    let tools = gbcli_build::Toolchain::discover(Some(&path), &temp.0).unwrap();
    write_png(&temp.path("demo/assets/bg/sign.png"), 16, 8, |x, _| {
        if x < 8 { WHITE } else { BLACK }
    });
    let made = gbcli_build::build(&tools, &options).expect("the build needs no rgbgfx");
    assert!(made.images.unwrap().images.len() == 1);
    assert!(!marker.exists(), "the build ran rgbgfx");
}
