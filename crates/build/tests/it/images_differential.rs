//! The encoder's differential test (Ben's ruling, 2026-11-09). rgbgfx 1.0.4
//! wrote gbcli's image data until then; the build now encodes it in Rust
//! (`crate::images::encode`, `gbc`) and rgbgfx is this test's oracle only.
//!
//! Every example game's images, and a fixture of each kind and art (a plain
//! background, a tileset, 8x16 and 8x8 sheets, shared shades, derived color
//! art, GBC multi-palette art and a 16-bit PNG), are converted by `assets`,
//! which runs no tool. Then real rgbgfx is run on the same image with the
//! same palettes, and what the two wrote must agree: the DMG tile data byte
//! for byte, and the GBC data by the color each pixel draws (rgbgfx may
//! pick another palette that holds a tile's colors, research §3; gbcli
//! always draws from the slot it packed).
//!
//! The fixtures other image tests build live in their own temporary
//! directories and are gone before this test runs, so this file builds the
//! same shapes again rather than reaching into them; every one of them is
//! also built with no rgbgfx on `PATH` by its own test.

use crate::common;
use crate::pngs;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use gbcli_build::{AssetsInput, AssetsOptions, assets};
use gbcli_protocol::{
    AssetsMade, BuildTarget, Flip, GbcPalette, ImageArt, ImageAsset, ImageKind, ImagesMade, Rgb555,
    Tool,
};
use pngs::{BLACK, CLEAR, DARK, LIGHT, Rgba, WHITE, write_png};

/// The rgbgfx this test compares with: the pinned 1.0.4 from `PATH`.
fn rgbgfx() -> PathBuf {
    let path = std::env::var_os("PATH");
    let cwd = std::env::current_dir().unwrap();
    gbcli_build::probe(Tool::Rgbgfx, path.as_deref(), &cwd)
        .unwrap_or_else(|e| {
            panic!(
                "this differential test needs rgbgfx 1.0.4 on PATH: {}",
                e.message()
            )
        })
        .path
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

/// An image's pixels as RGBA, expanded from any PNG encoding.
fn read_rgba(path: &Path) -> (u32, u32, Vec<Rgba>) {
    let mut decoder = png::Decoder::new(fs::File::open(path).unwrap());
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).unwrap();
    let n = info.color_type.samples();
    let pixels = buf[..info.buffer_size()]
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

/// The four grays a DMG tile's indices are drawn in when the data has no
/// image file of its own (a derived DMG version, pixel text).
const GRAYS: [Rgba; 4] = [WHITE, LIGHT, DARK, BLACK];

/// Checks the DMG data of `asset` against rgbgfx, appending mismatches.
fn check_dmg(base: &Path, keep: &Path, asset: &ImageAsset, scratch: &Path, bad: &mut Vec<String>) {
    let sheet = asset.kind == ImageKind::Sprites;
    let per = asset.object_size.map_or(1, |size| size.tiles());
    let Ok(tiles) = fs::read(data_path(keep, asset, "2bpp")) else {
        return;
    };
    let cells = cells(asset, &tiles, per);
    if cells.is_empty() {
        return;
    }
    let (w, h) = (asset.width * 8, asset.height * 8);
    // The image rgbgfx reads: the source of indexed art, a `dmg` version
    // beside a color image, else a copy of the data in four grays.
    let dmg_png = match asset.gbc.as_ref().map(|g| &g.art) {
        Some(ImageArt::Indexed { .. }) => Some(base.join(&asset.source)),
        _ => {
            let given = Path::new(&asset.source);
            let name = given.file_name().unwrap().to_string_lossy().into_owned();
            let stem = name
                .rsplit_once('.')
                .map_or(name.clone(), |(n, _)| n.to_string());
            let dmg = base.join(given.with_file_name(format!("{stem}.dmg.png")));
            dmg.exists().then_some(dmg)
        }
    };
    let real = dmg_png
        .filter(|p| p.extension().is_some_and(|e| e == "png"))
        .and_then(|p| {
            let (pw, ph, pixels) = read_rgba(&p);
            (pw == w && ph == h).then_some(pixels)
        });
    // One color a color index: the image's own where there is one, else the
    // grays. Two colors sharing an index draw as one, as the plan says.
    let mut spec: [Option<Rgba>; 4] = [None; 4];
    for cell in &cells {
        for (n, &index) in cell.pixels.iter().enumerate() {
            let index = index as usize;
            let (x, y) = (cell.x * 8 + n as u32 % 8, cell.y * 8 + n as u32 / 8);
            let color = real
                .as_ref()
                .map_or(GRAYS[index], |pixels| pixels[(y * w + x) as usize]);
            spec[index].get_or_insert(color);
        }
    }
    let spec_text = (0..4)
        .map(|i| match spec[i] {
            Some(c) if !(sheet && i == 0) => hex(c),
            _ => "#none".into(),
        })
        .collect::<Vec<_>>()
        .join(",");
    let png = scratch.join("oracle-dmg.png");
    let mut drawn = vec![CLEAR; (w * h) as usize];
    for cell in &cells {
        for (n, &index) in cell.pixels.iter().enumerate() {
            let (x, y) = (cell.x * 8 + n as u32 % 8, cell.y * 8 + n as u32 / 8);
            let mut color = spec[index as usize].unwrap_or(GRAYS[index as usize]);
            if sheet && index == 0 {
                color[3] = 0;
            }
            drawn[(y * w + x) as usize] = color;
        }
    }
    write_png(&png, w, h, |x, y| drawn[(y * w + x) as usize]);
    let (out, _) = run_rgbgfx(&png, &spec_text, sheet, false, scratch);
    let (dw, dh) = (asset.width, asset.height);
    for cell in &cells {
        for t in 0..per {
            let got = decode(oracle(&out, sheet, dw, dh, cell.x, cell.y + t));
            let want = &cell.pixels[64 * t as usize..64 * (t as usize + 1)];
            if got != want {
                bad.push(format!(
                    "{}: DMG tile ({}, {}) differs from rgbgfx (spec {spec_text})",
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
    // The color image rgbgfx reads: the source when it is a PNG, else a
    // copy of the colors the data draws.
    let source = base.join(&asset.source);
    let real = source
        .extension()
        .is_some_and(|e| e == "png")
        .then(|| read_rgba(&source))
        .filter(|(pw, ph, _)| *pw == w && *ph == h)
        .map(|(_, _, pixels)| pixels);
    let png = scratch.join("oracle-gbc.png");
    let mut drawn = vec![CLEAR; (w * h) as usize];
    for cell in &cells {
        let slot = cell.slot.expect("a GBC cell has a slot");
        for (n, &index) in cell.pixels.iter().enumerate() {
            let (x, y) = (cell.x * 8 + n as u32 % 8, cell.y * 8 + n as u32 / 8);
            drawn[(y * w + x) as usize] =
                match (real.as_ref(), color_of(palettes, slot, obj, index)) {
                    (Some(pixels), _) => pixels[(y * w + x) as usize],
                    (None, Some(color)) => {
                        let [r, g, b] = color.to_rgb888();
                        [r, g, b, 0xFF]
                    }
                    (None, None) => CLEAR,
                };
        }
    }
    write_png(&png, w, h, |x, y| drawn[(y * w + x) as usize]);
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

fn check(project: &Path, base: &Path, name: &str) -> Vec<String> {
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
        check_dmg(base, &keep, asset, &keep, &mut bad);
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
    bad
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
        bad.extend(check(&project, &examples, &format!("ex-{name}")));
    }
    assert!(games >= 5, "{games} example games");
    assert!(bad.is_empty(), "rgbgfx disagrees:\n{}", bad.join("\n"));
}

/// A fixture of every kind and art matches rgbgfx.
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
    // An 8x8 sheet, and a tileset.
    write_png(&demo.join("assets/sprites/dot.8x8.png"), 8, 8, |x, _| {
        if x < 4 { BLACK } else { CLEAR }
    });
    write_png(&demo.join("assets/tiles/blocks.png"), 8, 8, |x, y| {
        if (x + y) % 2 == 0 { LIGHT } else { DARK }
    });
    let bad = check(&demo, &temp.0, "fixtures");
    assert!(bad.is_empty(), "rgbgfx disagrees:\n{}", bad.join("\n"));
}

/// The build runs no rgbgfx: one that records being called and fails is
/// never run.
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
