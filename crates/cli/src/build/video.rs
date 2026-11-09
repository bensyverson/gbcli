//! The flags of screenshots, views and key timelines → their protocol args.

use gbcli_protocol::{
    Crop, FilmstripArgs, ImageOut, KeyEvent, MapSel, ShotArgs, ViewArgs, ViewKind, parse_keys,
};

use crate::Env;
use crate::argv::Invocation;
use crate::usage::Usage;
use crate::values;

/// The shot options, refused rather than ignored when nothing takes a shot.
const SHOT_OPTIONS: [&str; 3] = ["crop", "live", "any-frame"];

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

/// `run --filmstrip PATH [--every N]` (a frame every N frames, default 1),
/// or `None` when there is no `--filmstrip`.
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
        })),
        (None, Some(_)) => Err(Usage::new(
            "--every only applies with --filmstrip",
            "add --filmstrip PATH, or drop it",
        )),
        (None, None) => Ok(None),
    }
}

/// A shot to `path` with the invocation's shot options.
pub fn shot(inv: &Invocation, path: &str) -> Result<ShotArgs, Usage> {
    Ok(ShotArgs {
        path: path.to_string(),
        scale: scale(inv)?,
        crop: inv.one("crop").map(crop).transpose()?,
        live: inv.has("live"),
        any_frame: inv.has("any-frame"),
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

pub fn view(inv: &Invocation) -> Result<ViewArgs, Usage> {
    let what = match inv.positionals[0].as_str() {
        "tiles" => ViewKind::Tiles,
        "map" => ViewKind::Map,
        "oam" => ViewKind::Oam,
        "palettes" => ViewKind::Palettes,
        other => {
            return Err(Usage::new(
                format!("`{other}` isn't tiles, map, oam or palettes"),
                "e.g. `gbcli view oam`",
            ));
        }
    };
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
    Ok(ViewArgs { what, map, png })
}
