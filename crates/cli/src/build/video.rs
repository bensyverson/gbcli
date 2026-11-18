//! The flags of screenshots, views and key timelines → their protocol args.

use gbcli_protocol::{
    Crop, FilmstripArgs, ImageOut, KeyEvent, MapSel, ShotArgs, TimeBase, ViewArgs, ViewKind,
    parse_keys,
};

use crate::Env;
use crate::argv::Invocation;
use crate::usage::Usage;
use crate::values;

/// The shot-only options, refused rather than ignored when nothing takes a
/// shot. `--crop` also crops a filmstrip, so it is checked on its own.
const SHOT_OPTIONS: [&str; 3] = ["live", "any-frame", "screen-rows"];

/// The run's images that `--scale` scales besides the shot.
const SCALED_IMAGES: [&str; 3] = ["filmstrip", "piano", "spectrogram"];

/// `run --shot PATH` and its options, or `None` when there is no `--shot`.
pub fn run_shot(inv: &Invocation) -> Result<Option<ShotArgs>, Usage> {
    match inv.one("shot") {
        Some(path) => shot(inv, path).map(Some),
        None => {
            if let Some(flag) = SHOT_OPTIONS.into_iter().find(|f| inv.has(f)) {
                return Err(Usage::new(
                    format!("--{flag} only applies with --shot"),
                    "add --shot PATH, or drop it",
                ));
            }
            if inv.has("crop") && !inv.has("filmstrip") {
                return Err(Usage::new(
                    "--crop only applies with --shot or --filmstrip",
                    "add --shot PATH or --filmstrip PATH, or drop it",
                ));
            }
            if inv.has("scale") && !SCALED_IMAGES.into_iter().any(|f| inv.has(f)) {
                return Err(Usage::new(
                    "--scale only applies with --shot, --filmstrip, --piano or --spectrogram",
                    "add one of them, or drop it",
                ));
            }
            Ok(None)
        }
    }
}

/// `run --filmstrip PATH [--every N] [--crop X,Y,W,H]` (a frame every N
/// frames, default 1, each cropped), or `None` when there is no `--filmstrip`.
pub fn run_filmstrip(inv: &Invocation) -> Result<Option<FilmstripArgs>, Usage> {
    let every = inv
        .one("every")
        .map(|n| values::count("every", n))
        .transpose()?;
    match (inv.one("filmstrip"), every) {
        (Some(path), every) => Ok(Some(FilmstripArgs {
            path: path.to_string(),
            every: every.unwrap_or(1),
            scale: scale(inv)?,
            crop: inv.one("crop").map(crop).transpose()?,
        })),
        (None, Some(_)) => Err(Usage::new(
            "--every only applies with --filmstrip",
            "add --filmstrip PATH, or drop it",
        )),
        (None, None) => Ok(None),
    }
}

/// A shot to `path` with the invocation's shot options. `--screen-rows`
/// without `--json` is refused: only `--json` prints the rows.
pub fn shot(inv: &Invocation, path: &str) -> Result<ShotArgs, Usage> {
    if inv.has("screen-rows") && !inv.has("json") {
        return Err(Usage::new(
            "--screen-rows only applies with --json",
            "add --json to get screen.rows, or drop it",
        ));
    }
    Ok(ShotArgs {
        path: path.to_string(),
        scale: scale(inv)?,
        crop: inv.one("crop").map(crop).transpose()?,
        live: inv.has("live"),
        any_frame: inv.has("any-frame"),
        screen: inv.has("screen-rows"),
    })
}

/// `--scale N`, or `None` for the server's default for that image.
pub(super) fn scale(inv: &Invocation) -> Result<Option<u32>, Usage> {
    inv.one("scale")
        .map(|n| values::count("scale", n).map(|n| n.min(u64::from(u32::MAX)) as u32))
        .transpose()
}

fn crop(text: &str) -> Result<Crop, Usage> {
    let parts: Vec<u32> = text
        .split(',')
        .map(|p| p.trim().parse())
        .collect::<Result<_, _>>()
        .unwrap_or_default();
    match parts[..] {
        [x, y, width, height] => Ok(Crop {
            x,
            y,
            width,
            height,
        }),
        _ => Err(Usage::new(
            format!("--crop takes X,Y,W,H, not `{text}`"),
            "four whole numbers in Game Boy pixels, e.g. `--crop 0,0,80,72`",
        )),
    }
}

/// `--keys` then `--keys-file`, as one timeline.
pub fn keys(inv: &Invocation, env: &Env) -> Result<Vec<KeyEvent>, Usage> {
    let mut script = inv.one("keys").unwrap_or_default().to_string();
    if let Some(file) = inv.one("keys-file") {
        let text = std::fs::read_to_string(env.cwd.join(file)).map_err(|e| {
            Usage::new(
                format!("can't read {file}: {e}"),
                "check the path; the file holds key tokens like `A@0 -A@2`",
            )
        })?;
        script.push('\n');
        script.push_str(&text);
    }
    parse_keys(&script).map_err(|e| {
        Usage::new(
            e.to_string(),
            "e.g. --keys 'A@0 -A@2 START@10': press at a frame offset, - releases",
        )
    })
}

/// `run --keys-since BASE`, refused without a key timeline to count.
pub fn keys_since(inv: &Invocation) -> Result<Option<TimeBase>, Usage> {
    let Some(base) = inv.one("keys-since") else {
        return Ok(None);
    };
    if !inv.has("keys") && !inv.has("keys-file") {
        return Err(Usage::new(
            "--keys-since only applies with --keys or --keys-file",
            "add the key tokens it counts, or drop it",
        ));
    }
    Ok(Some(values::since(base)))
}

pub fn view(inv: &Invocation) -> Result<ViewArgs, Usage> {
    let what = match inv.positionals[0].as_str() {
        "tiles" => ViewKind::Tiles,
        "map" => ViewKind::Map,
        "oam" => ViewKind::Oam,
        "palettes" => ViewKind::Palettes,
        "memory" => return view_memory(inv),
        other => {
            return Err(Usage::new(
                format!("`{other}` isn't tiles, map, oam, palettes or memory"),
                "e.g. `gbcli view oam`",
            ));
        }
    };
    if let Some(extra) = inv.positionals.get(1) {
        return Err(Usage::new(
            format!("`gbcli view {}` doesn't take `{extra}`", inv.positionals[0]),
            "only view memory takes an address and a length: `gbcli view memory wMap 16`",
        ));
    }
    for flag in MEMORY_ONLY {
        if inv.has(flag) {
            return Err(Usage::new(
                format!("--{flag} only applies to view memory"),
                "drop it, or `gbcli view memory ADDR LEN`",
            ));
        }
    }
    let map = inv
        .one("map")
        .map(|m| match m {
            "background" => Ok(MapSel::Background),
            "window" => Ok(MapSel::Window),
            "9800" => Ok(MapSel::Map9800),
            "9c00" | "9C00" => Ok(MapSel::Map9C00),
            other => Err(Usage::new(
                format!("--map takes background, window, 9800 or 9c00, not `{other}`"),
                "e.g. `gbcli view map --map window`",
            )),
        })
        .transpose()?;
    let png = match inv.one("png") {
        Some(path) => Some(ImageOut {
            path: path.to_string(),
            scale: scale(inv)?,
        }),
        None if inv.has("scale") => {
            return Err(Usage::new(
                "--scale only applies with --png",
                "add --png PATH, or drop it",
            ));
        }
        None => None,
    };
    Ok(ViewArgs {
        map,
        png,
        ..ViewArgs::new(what)
    })
}

/// The flags only `view memory` takes, refused on the VRAM views.
const MEMORY_ONLY: [&str; 2] = ["out", "allow-uninit"];

/// The VRAM views' flags, refused on `view memory`.
const VRAM_ONLY: [&str; 3] = ["map", "png", "scale"];

/// `view memory ADDR LEN [--out FILE] [--allow-uninit]`.
fn view_memory(inv: &Invocation) -> Result<ViewArgs, Usage> {
    let (Some(addr), Some(len)) = (inv.positionals.get(1), inv.positionals.get(2)) else {
        return Err(Usage::new(
            "`gbcli view memory` needs ADDR and LEN",
            "e.g. `gbcli view memory wMap 16`: 16 bytes from wMap",
        ));
    };
    if let Some(flag) = VRAM_ONLY.into_iter().find(|f| inv.has(f)) {
        return Err(Usage::new(
            format!("--{flag} doesn't apply to view memory"),
            "drop it; --out FILE writes the raw bytes",
        ));
    }
    let hex = len.strip_prefix('$').or_else(|| len.strip_prefix("0x"));
    let parsed = match hex {
        Some(digits) => u32::from_str_radix(digits, 16),
        None => len.parse::<u32>(),
    };
    let len = parsed.map_err(|_| {
        Usage::new(
            format!("LEN is a count of bytes, not `{len}`"),
            "decimal or $hex, e.g. `gbcli view memory wMap 16` or `... wMap $400`",
        )
    })?;
    Ok(ViewArgs {
        addr: Some(addr.clone()),
        len: Some(len),
        out: inv.one("out").map(str::to_string),
        allow_uninit: inv.has("allow-uninit"),
        ..ViewArgs::new(ViewKind::Memory)
    })
}
