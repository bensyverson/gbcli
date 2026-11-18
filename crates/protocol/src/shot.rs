//! Screenshots on the wire (design §9, §11 step 6): what a run or the `shot`
//! verb asks for, and which frame the PNG shows, counted from the request's
//! time base as the stop record's `frame_shown` is.

use serde::{Deserialize, Serialize};

use crate::request::is_false;
use crate::stop::{FrameShown, TimeBase, vblank_name};
use crate::views::{PpuModeView, PpuView};

/// A screenshot to write after the run (or now, for the `shot` verb).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShotArgs {
    /// Where to write the PNG, relative to the server's working directory.
    pub path: String,
    /// The integer scale; the view's default (4) when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<u32>,
    /// A crop in native pixels, applied before the scale.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crop: Option<Crop>,
    /// The partly drawn framebuffer as it is now, labeled partial.
    #[serde(default, skip_serializing_if = "is_false")]
    pub live: bool,
    /// Take the last complete frame even when its vblank wasn't normal
    /// (LCD off, artificial), which is otherwise refused (design §9).
    #[serde(default, skip_serializing_if = "is_false")]
    pub any_frame: bool,
    /// Also return the pixels as data ([`ShotMade::screen`]); the
    /// fingerprint is always returned.
    #[serde(default, skip_serializing_if = "is_false")]
    pub screen: bool,
}

/// A rectangle in the frame's native pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Crop {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// The `shot` verb's args: the shot, and what its frame number counts from.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShotReadArgs {
    pub shot: ShotArgs,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::request::since_word"
    )]
    pub since: Option<TimeBase>,
}

/// Which frame a PNG shows.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ShotFrame {
    /// A frame a vblank completed: the same numbers as `frame_shown`.
    Complete(FrameShown),
    /// The framebuffer mid-frame: lines above `ppu.ly` are the frame in
    /// progress, the rest still hold the frame before.
    LivePartial {
        ppu: PpuView,
        /// Since power-on, in 2^23 Hz ticks.
        ticks: u64,
        /// The number of the complete frame this one follows, since the base.
        after: Option<i64>,
    },
}

/// A screenshot that was written.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShotMade {
    /// The path as the request gave it.
    pub path: String,
    /// The PNG's size in pixels, after the crop and the scale.
    pub width: u32,
    pub height: u32,
    pub scale: u32,
    /// What `frame` counts from.
    pub since: TimeBase,
    pub frame: ShotFrame,
    /// A fingerprint of `screen`: the first 16 lowercase hex digits of the
    /// SHA-256 of its rows joined by `\n`. Equal pixels give equal
    /// fingerprints, whatever the frame. Among screens of one kind, the rows'
    /// count and length carry the size, so a crop of another shape gives
    /// another fingerprint; the kind isn't hashed, so across kinds it may
    /// not (six shade-0 pixels and one black color pixel are both `000000`).
    pub pixels: String,
    /// The pixels as data, after the crop and before the scale, when the
    /// request asked for them ([`ShotArgs::screen`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screen: Option<ShotScreen>,
}

/// What a screen's pixels are: the frame's own values, not the PNG's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScreenKind {
    /// DMG and MGB: shade 0 (lightest) to 3 (darkest), after BGP/OBP0/OBP1.
    Shades,
    /// CGB, AGB and SGB: each pixel's color as the core output it, after its
    /// color correction: the 24-bit color the PNG holds, not palette RAM's
    /// RGB555.
    Colors,
}

/// A screen's pixels, row by row, top to bottom.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShotScreen {
    pub kind: ScreenKind,
    /// In native pixels.
    pub width: u32,
    pub height: u32,
    /// One text per row, left to right: a digit 0-3 per pixel for shades,
    /// six lowercase hex digits (`rrggbb`) per pixel for colors.
    pub rows: Vec<String>,
}

impl ShotMade {
    /// The label the PNG carries (`gbcli:frame`): what the result says, in words.
    pub fn frame_label(&self) -> String {
        self.frame.label(&self.since)
    }
}

impl ShotFrame {
    /// The frame in words, counted from `since`.
    pub fn label(&self, since: &TimeBase) -> String {
        let base = match since {
            TimeBase::PowerOn => "since power-on".to_string(),
            TimeBase::Boot => "since boot".to_string(),
            TimeBase::Checkpoint(name) => format!("since checkpoint {name}"),
        };
        match self {
            ShotFrame::Complete(shown) => format!(
                "complete frame {} {base} (vblank {})",
                shown.n,
                vblank_name(shown.vblank)
            ),
            ShotFrame::LivePartial { ppu, after, .. } => {
                let lcd = if ppu.lcd_on { "" } else { ", LCD off" };
                let after = match after {
                    Some(n) => format!("after complete frame {n} {base}"),
                    None => "before any complete frame".to_string(),
                };
                format!(
                    "live partial frame: LY {} ({}){lcd}, {after}",
                    ppu.ly,
                    ppu.mode.map_or("mode unknown", PpuModeView::name)
                )
            }
        }
    }
}

/// A filmstrip to write after the run: the last complete frame at every
/// `every`th vblank the run passes, in one row.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FilmstripArgs {
    /// Where to write the PNG, relative to the server's working directory.
    pub path: String,
    /// Keep a frame every this many frames of the run (at least 1).
    pub every: u64,
    /// The integer scale; 1 when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<u32>,
    /// A crop in native pixels, applied to every frame before the scale.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crop: Option<Crop>,
}

impl FilmstripArgs {
    /// The most frames a filmstrip holds; later ones are counted, not kept.
    pub const MAX_FRAMES: usize = 16;
}

/// A filmstrip that was written.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilmstripMade {
    pub path: String,
    pub width: u32,
    pub height: u32,
    pub scale: u32,
    /// What each frame's number counts from.
    pub since: TimeBase,
    /// Left to right. Frames that aren't normal (LCD off) are kept, and say so.
    pub frames: Vec<ShotFrame>,
    /// Frames due past [`FilmstripArgs::MAX_FRAMES`], not kept.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub beyond_cap: u64,
}

fn is_zero(n: &u64) -> bool {
    *n == 0
}

#[cfg(test)]
mod tests {
    use gbcli_core::VblankKind;

    use super::*;
    use crate::parse::parse_request;
    use crate::request::{Op, RunArgs};

    #[test]
    fn a_run_carries_a_shot_and_refuses_strangers_inside_it() {
        let line = r#"{"id":"1","op":"run","args":{"frames":3,"shot":{"path":"a.png","scale":2,"crop":{"x":0,"y":0,"width":8,"height":8},"any_frame":true,"screen":true}}}"#;
        let request = parse_request(line).expect("parses");
        let Op::Run(RunArgs {
            shot: Some(shot), ..
        }) = &request.op
        else {
            panic!("{request:?}")
        };
        assert_eq!(shot.path, "a.png");
        assert_eq!(shot.scale, Some(2));
        assert_eq!(shot.crop.map(|c| c.width), Some(8));
        assert!(shot.any_frame && !shot.live && shot.screen);
        assert_eq!(serde_json::to_string(&request).unwrap(), line);

        let typo = r#"{"id":"1","op":"run","args":{"shot":{"path":"a.png","scael":2}}}"#;
        let err = parse_request(typo).unwrap_err();
        assert!(err.remedy.contains("`scale`"), "{}", err.remedy);
    }

    #[test]
    fn a_run_carries_a_filmstrip_and_refuses_strangers_inside_it() {
        let line = r#"{"id":"1","op":"run","args":{"frames":6,"filmstrip":{"path":"f.png","every":2,"crop":{"x":0,"y":0,"width":8,"height":8}}}}"#;
        let request = parse_request(line).expect("parses");
        let Op::Run(RunArgs {
            filmstrip: Some(strip),
            ..
        }) = &request.op
        else {
            panic!("{request:?}")
        };
        assert_eq!(
            (strip.path.as_str(), strip.every, strip.scale),
            ("f.png", 2, None)
        );
        assert_eq!(strip.crop.map(|c| c.width), Some(8));
        assert_eq!(serde_json::to_string(&request).unwrap(), line);
        let typo = r#"{"id":"1","op":"run","args":{"filmstrip":{"path":"f.png","evry":2}}}"#;
        assert!(parse_request(typo).unwrap_err().remedy.contains("`every`"));
    }

    #[test]
    fn the_shot_verb_takes_a_shot_and_a_time_base() {
        let line = r#"{"id":"1","op":"shot","args":{"shot":{"path":"b.png"},"since":"power-on"}}"#;
        let request = parse_request(line).expect("parses");
        let Op::Shot(args) = &request.op else {
            panic!("{request:?}")
        };
        assert_eq!(args.since, Some(TimeBase::PowerOn));
        assert!(!request.op.mutates_head(), "a read verb");
        assert_eq!(request.op.name(), "shot");
    }

    #[test]
    fn a_complete_frame_is_labeled_with_its_base_and_a_live_one_as_partial() {
        let made = ShotMade {
            path: "a.png".into(),
            width: 640,
            height: 576,
            scale: 4,
            since: TimeBase::Boot,
            frame: ShotFrame::Complete(FrameShown {
                n: 3,
                vblank: VblankKind::Normal,
            }),
            pixels: "5feceb66ffc86f38".into(),
            screen: Some(ShotScreen {
                kind: ScreenKind::Shades,
                width: 1,
                height: 1,
                rows: vec!["0".into()],
            }),
        };
        let json = serde_json::to_string(&made).unwrap();
        assert!(
            json.contains(r#""frame":{"kind":"complete","n":3,"vblank":"normal"}"#),
            "{json}"
        );
        assert!(
            json.contains(
                r#""pixels":"5feceb66ffc86f38","screen":{"kind":"shades","width":1,"height":1,"rows":["0"]}"#
            ),
            "{json}"
        );
        let bare = ShotMade {
            screen: None,
            ..made.clone()
        };
        let json = serde_json::to_string(&bare).unwrap();
        assert!(json.contains(r#""pixels":"5feceb66ffc86f38""#), "{json}");
        assert!(!json.contains("screen"), "rows only when asked: {json}");
        assert_eq!(
            made.frame_label(),
            "complete frame 3 since boot (vblank normal)"
        );

        let live = ShotMade {
            frame: ShotFrame::LivePartial {
                ppu: PpuView {
                    ly: 67,
                    mode: Some(PpuModeView::OamScan),
                    lcd_on: true,
                    dot: None,
                },
                ticks: 1000,
                after: Some(6),
            },
            ..made
        };
        let json = serde_json::to_string(&live).unwrap();
        assert!(json.contains(r#""kind":"live_partial""#), "{json}");
        assert_eq!(
            live.frame_label(),
            "live partial frame: LY 67 (oam_scan), after complete frame 6 since boot"
        );

        let unknown = ShotMade {
            frame: ShotFrame::LivePartial {
                ppu: PpuView {
                    ly: 67,
                    mode: None,
                    lcd_on: true,
                    dot: None,
                },
                ticks: 1000,
                after: Some(6),
            },
            ..live
        };
        assert_eq!(
            unknown.frame_label(),
            "live partial frame: LY 67 (mode unknown), after complete frame 6 since boot"
        );
    }
}
