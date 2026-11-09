//! Rows for screenshots, VRAM views, symbols, stored states and the keys a
//! run didn't reach.

use gbcli_protocol::{OamLogMade, ResultBody, VideoData};

use super::row::{Row, addr, byte};
use super::stop::since;

pub fn rows(r: &ResultBody) -> String {
    let mut out = String::new();
    if let Some(shot) = &r.shot {
        out += &Row::new("shot")
            .field("path", &shot.path)
            .field("width", shot.width)
            .field("height", shot.height)
            .field("scale", shot.scale)
            .field("frame", shot.frame_label())
            .line();
    }
    if let Some(strip) = &r.filmstrip {
        out += &Row::new("filmstrip")
            .field("path", &strip.path)
            .field("width", strip.width)
            .field("height", strip.height)
            .field("scale", strip.scale)
            .field("frames", strip.frames.len())
            .opt(
                "beyond_cap",
                (strip.beyond_cap > 0).then_some(strip.beyond_cap),
            )
            .line();
        for (i, frame) in strip.frames.iter().enumerate() {
            out += &Row::new("frame")
                .field("index", i)
                .field("label", frame.label(&strip.since))
                .line();
        }
    }
    out += &unapplied(r.keys_unapplied.as_deref());
    if let Some(log) = &r.oam_log {
        out += &oam_log(log);
    }
    if let Some(view) = &r.view {
        out += &data(&view.data);
        if let Some(png) = &view.png {
            out += &Row::new("png")
                .field("path", &png.path)
                .field("width", png.width)
                .field("height", png.height)
                .field("scale", png.scale)
                .line();
        }
    }
    if let Some(symbols) = &r.symbols {
        for s in symbols {
            let json = serde_json::to_value(s).expect("serializes");
            out += &Row::new("symbol")
                .field("name", &s.name)
                .field("bank", s.bank)
                .field("addr", addr(s.addr))
                .field("area", json["area"].as_str().unwrap_or("?"))
                .line();
        }
        if symbols.is_empty() {
            out += "none\n";
        }
    }
    if let Some(saved) = &r.checkpoints {
        for c in saved {
            let rev = c
                .rev
                .map_or_else(|| "unknown".to_string(), |r| r.to_string());
            out += &Row::new("saved")
                .field("at", &c.at)
                .field("rev", rev)
                .opt("frame", c.frame)
                .opt("rom_matches", c.rom_matches.map(u8::from))
                .opt("error", c.error.as_deref())
                .line();
        }
        if saved.is_empty() {
            out += "none\n";
        }
    }
    out
}

/// A view's data: a header row, then its picture rows indented under it.
fn data(data: &VideoData) -> String {
    let json = serde_json::to_value(data).expect("serializes");
    let wire = |key: &str| json[key].as_str().unwrap_or("?").to_string();
    let mut out = String::new();
    match data {
        VideoData::Tiles { tiles, blank } => {
            out += &Row::new("tiles")
                .field("shown", tiles.len())
                .field("blank", blank)
                .line();
            for tile in tiles {
                out += &Row::new("tile")
                    .field("bank", tile.bank)
                    .field("index", tile.index)
                    .line();
                out += &indented(&tile.rows);
            }
        }
        VideoData::Map {
            background,
            ids,
            attributes,
            ..
        } => {
            out += &Row::new("map")
                .field("area", wire("area"))
                .field("addressing", wire("addressing"))
                .flag("background", *background)
                .line();
            out += &indented(ids);
            if let Some(attributes) = attributes {
                out += "attributes\n";
                out += &indented(attributes);
            }
        }
        VideoData::Palettes {
            bgp,
            obp0,
            obp1,
            bg,
            obj,
        } => {
            for (reg, p) in [("BGP", bgp), ("OBP0", obp0), ("OBP1", obp1)] {
                let shades: Vec<String> = p.shades.iter().map(u8::to_string).collect();
                out += &Row::new("dmg_palette")
                    .field("reg", reg)
                    .field("value", byte(p.value))
                    .field("shades", shades.join(","))
                    .line();
            }
            for (kind, set) in [("bg_palette", bg), ("obj_palette", obj)] {
                for p in set {
                    let rgb555: Vec<String> = p
                        .colors
                        .iter()
                        .map(|c| format!("${:04X}", c.rgb555))
                        .collect();
                    let hex: Vec<&str> = p.colors.iter().map(|c| c.hex.as_str()).collect();
                    out += &Row::new(kind)
                        .field("index", p.index)
                        .field("rgb555", rgb555.join(","))
                        .field("hex", hex.join(","))
                        .line();
                }
            }
        }
        VideoData::Oam { height, objects } => {
            out += &Row::new("oam").field("height", height).line();
            for o in objects {
                out += &Row::new("object")
                    .field("index", o.index)
                    .field("y", o.y)
                    .field("x", o.x)
                    .field("tile", byte(o.tile))
                    .field("flags", byte(o.flags))
                    .field("screen_x", o.screen_x)
                    .field("screen_y", o.screen_y)
                    .flag("on_screen", o.on_screen)
                    .flag("dropped_by_line_limit", o.dropped_by_line_limit)
                    .line();
            }
        }
    }
    out
}

/// `run --oam-log`: a row per frame; `--json` has every object's fields.
fn oam_log(log: &OamLogMade) -> String {
    let mut out = Row::new("oam_log")
        .field("since", since(&log.since))
        .field("frames", log.frames.len())
        .line();
    for frame in &log.frames {
        let dropped: Vec<String> = frame.dropped().iter().map(u8::to_string).collect();
        let shown: Vec<String> = frame.objects.iter().map(|o| o.index.to_string()).collect();
        out += &Row::new("oam_frame")
            .field("frame", frame.frame)
            .opt("lcd_off", frame.lcd_off.then_some(1))
            .field("height", frame.height)
            .field("on_screen", frame.objects.len())
            .opt("objects", (!shown.is_empty()).then(|| shown.join(",")))
            .opt("dropped", (!dropped.is_empty()).then(|| dropped.join(",")))
            .line();
    }
    out
}

fn indented(lines: &[String]) -> String {
    lines.iter().map(|l| format!("  {l}\n")).collect()
}

/// The key events a run stopped before, whether it succeeded or failed.
pub fn unapplied(keys: Option<&[gbcli_protocol::KeyEvent]>) -> String {
    let mut out = String::new();
    for key in keys.into_iter().flatten() {
        out += &Row::new("key_unapplied").field("event", key).line();
    }
    out
}
