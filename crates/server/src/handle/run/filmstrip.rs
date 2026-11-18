//! A run's filmstrip (design §9): the last complete frame at every `every`th
//! vblank the run passes, collected by the run loop's frame sink, laid out in
//! one row, each frame labeled from the request's time base as a shot is.
//! Frames that aren't normal (LCD off) are kept and say so.

use std::cell::{Cell, RefCell};

use gbcli_engine::{Machine, Since};
use gbcli_protocol::{
    ErrorBody, ErrorCode, FilmstripArgs, FilmstripMade, FrameShown, PpuView, ShotFrame,
};
use gbcli_view::{Frame, FrameChoice, FrameSource, LabelKey, ViewError};

use super::super::Failure;
use super::super::files::Files;
use super::super::shot::{check_crop, check_image, rect, view_failure};

/// A filmstrip's scale when the request gives none: 16 frames already make
/// a strip 2,575 pixels wide.
const FILMSTRIP_SCALE: u32 = 1;

/// What can be refused before anything loads.
pub(super) fn check(args: &FilmstripArgs, files: &Files) -> Result<(), Failure> {
    if args.every == 0 {
        return Err(Failure::new(ErrorBody::new(
            ErrorCode::BadRequest,
            "a filmstrip needs a frame every 1 or more frames",
            "pass --every N with N at least 1, e.g. --every 10",
        )));
    }
    check_image(
        &args.path,
        Some(args.scale.unwrap_or(FILMSTRIP_SCALE)),
        files,
    )?;
    check_crop(args.crop)
}

/// Collects frames as the run passes vblanks.
pub(super) struct Collector<'a> {
    args: &'a FilmstripArgs,
    files: &'a Files,
    frames: RefCell<Vec<Frame>>,
    beyond_cap: Cell<u64>,
    error: RefCell<Option<ViewError>>,
}

impl<'a> Collector<'a> {
    pub(super) fn new(args: &'a FilmstripArgs, files: &'a Files) -> Self {
        Collector {
            args,
            files,
            frames: RefCell::new(Vec::new()),
            beyond_cap: Cell::new(0),
            error: RefCell::new(None),
        }
    }

    /// The frame sink: `done` is the frames the run has completed.
    pub(super) fn on_frame(&self, machine: &Machine, done: u64) {
        if !done.is_multiple_of(self.args.every) || self.error.borrow().is_some() {
            return;
        }
        if self.frames.borrow().len() >= FilmstripArgs::MAX_FRAMES {
            self.beyond_cap.set(self.beyond_cap.get() + 1);
            return;
        }
        match gbcli_view::capture(machine.core(), FrameChoice::CompleteAnyKind) {
            Ok(frame) => self.frames.borrow_mut().push(frame),
            Err(e) => *self.error.borrow_mut() = Some(e),
        }
    }

    /// Lays out, labels and writes the frames collected.
    pub(super) fn write(
        &self,
        machine: &Machine,
        since: &Since,
        warnings: &mut Vec<String>,
    ) -> Result<FilmstripMade, Failure> {
        let (base, mark) = machine.named_base(since);
        let rebase = |n: u64| n as i64 - mark.frame as i64;
        if let Some(e) = self.error.take() {
            return Err(view_failure(&e, &rebase, &base));
        }
        let frames = self.frames.take();
        if frames.is_empty() {
            return Err(Failure::new(ErrorBody::new(
                ErrorCode::BadRequest,
                format!(
                    "the run ended before its first frame for the filmstrip (every {} frames)",
                    self.args.every
                ),
                "run for at least as many frames as --every, or lower --every",
            )));
        }
        let shown: Vec<ShotFrame> = frames
            .iter()
            .map(|f| match f.source {
                FrameSource::Complete(tag) => ShotFrame::Complete(FrameShown {
                    n: rebase(tag.number),
                    vblank: tag.kind,
                }),
                FrameSource::Live { ppu, at, after } => ShotFrame::LivePartial {
                    ppu: PpuView::of(&ppu),
                    ticks: at.0,
                    after: after.map(rebase),
                },
            })
            .collect();
        let fail = |e: ViewError| view_failure(&e, &rebase, &base);
        let scale = self.args.scale.unwrap_or(FILMSTRIP_SCALE);
        let crop = self.args.crop.map(rect);
        let frames = match crop {
            Some(r) => frames
                .iter()
                .map(|f| f.cropped(r))
                .collect::<Result<Vec<_>, _>>()
                .map_err(fail)?,
            None => frames,
        };
        let mut image = gbcli_view::filmstrip(&frames)
            .and_then(|i| i.scaled(scale))
            .map_err(fail)?;
        if let Some(r) = crop {
            image = image.label(LabelKey::Detail, r.crop_label());
        }
        // The view labels frames from power-on; the PNG says what the result says.
        let label = shown
            .iter()
            .enumerate()
            .map(|(i, f)| format!("[{i}] {}", f.label(&base)))
            .collect::<Vec<_>>()
            .join("; ");
        for (key, text) in &mut image.labels {
            if *key == LabelKey::Frame {
                *text = label.clone();
            }
        }
        let bytes = image.png().map_err(fail)?;
        self.files.write(&self.args.path, &bytes)?;
        let beyond_cap = self.beyond_cap.get();
        if beyond_cap > 0 {
            warnings.push(format!(
                "the filmstrip holds {} frames; {beyond_cap} more were due and not kept: \
                 raise --every to cover the run",
                FilmstripArgs::MAX_FRAMES
            ));
        }
        Ok(FilmstripMade {
            path: self.args.path.clone(),
            width: image.width,
            height: image.height,
            scale,
            since: base,
            frames: shown,
            beyond_cap,
        })
    }
}
