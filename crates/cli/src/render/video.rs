//! Rows for screenshots, VRAM views, symbols, stored states and the keys a
//! run didn't reach.

use gbcli_protocol::{ResultBody, VideoData};

use super::row::{Row, addr, byte};

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
    out += &unapplied(r.keys_unapplied.as_deref());
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
