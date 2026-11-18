//! Screenshots (design §9, §11 step 6): after a run's reads, or of the head
//! by the `shot` verb. Capturing only reads the core, so neither advances it.
//! The frame number is rebased to the request's time base, as the stop
//! record's `frame_shown` is, and the PNG's label says the same.

use std::path::Path;

use gbcli_bundle::SessionStore;
use gbcli_engine::{Machine, Since};
use gbcli_protocol::{
    Crop, ErrorBody, ErrorCode, FrameShown, PpuView, ResultBody, SessionFile, ShotArgs, ShotFrame,
    ShotMade, ShotReadArgs,
};
use gbcli_view::{DEFAULT_SCALE, FrameChoice, FrameSource, LabelKey, MAX_SCALE, Rect, ViewError};

use super::files::Files;
use super::{Failure, Handler, Need};

impl Handler {
    pub(super) fn shot(
        &mut self,
        store: &SessionStore,
        meta: &SessionFile,
        args: &ShotReadArgs,
        warnings: &mut Vec<String>,
    ) -> Result<ResultBody, Failure> {
        let files = self.files();
        check(&args.shot, &files)?;
        let since = self.since(store, args.since.as_ref())?;
        let (machine, loaded) = self.machine_for(store, meta, Need::Head, None)?;
        warnings.extend(loaded);
        let shot = take(&machine, &args.shot, &since, &files);
        self.keep_warm(store, meta, machine);
        Ok(ResultBody {
            shot: Some(shot?),
            ..ResultBody::default()
        })
    }
}

/// What a shot can get wrong before the machine moves: the scale, an empty
/// crop, and a directory that isn't there.
pub(super) fn check(args: &ShotArgs, files: &Files) -> Result<(), Failure> {
    check_image(&args.path, args.scale, files)?;
    if args.live && args.any_frame {
        // A live shot is the frame being drawn, never a stored one, so
        // any_frame would be ignored.
        return Err(bad(
            "a shot is the live frame or the last complete one, not both".into(),
            "drop one of them: live for the pixels drawn so far, any-frame for an LCD-off or \
             blank last frame"
                .into(),
        ));
    }
    check_crop(args.crop)
}

/// An empty crop, refused before anything is drawn; one outside the frame
/// is found only once there is a frame.
pub(super) fn check_crop(crop: Option<Crop>) -> Result<(), Failure> {
    if crop.is_some_and(|c| c.width == 0 || c.height == 0) {
        return Err(bad(
            "a crop needs a width and a height".into(),
            "give the crop as x, y, width, height in native pixels".into(),
        ));
    }
    Ok(())
}

/// A PNG's path and scale, checked before anything is drawn; the path is
/// the request's, resolved by the host.
pub(super) fn check_image(
    path_text: &str,
    scale: Option<u32>,
    files: &Files,
) -> Result<(), Failure> {
    let scale = scale.unwrap_or(DEFAULT_SCALE);
    if !(1..=MAX_SCALE).contains(&scale) {
        return Err(bad(
            format!("scale {scale} is outside 1-{MAX_SCALE}"),
            format!("pass a scale from 1 to {MAX_SCALE}; the default is {DEFAULT_SCALE}"),
        ));
    }
    let path = Path::new(path_text);
    if path_text.is_empty() || path.file_name().is_none() {
        return Err(bad(
            format!("`{path_text}` isn't a file path"),
            "name the PNG to write, e.g. shot.png".into(),
        ));
    }
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty())
        && !files.is_dir(dir)
    {
        return Err(bad(
            format!("the directory {} doesn't exist", dir.display()),
            format!("create {} first, or write the PNG elsewhere", dir.display()),
        ));
    }
    Ok(())
}

/// Captures, crops, scales, encodes and writes; the result names the frame
/// counted from `since`.
pub(super) fn take(
    machine: &Machine,
    args: &ShotArgs,
    since: &Since,
    files: &Files,
) -> Result<ShotMade, Failure> {
    let (base, mark) = machine.named_base(since);
    let rebase = |n: u64| n as i64 - mark.frame as i64;
    let choice = match (args.live, args.any_frame) {
        (true, _) => FrameChoice::Live,
        (false, true) => FrameChoice::CompleteAnyKind,
        (false, false) => FrameChoice::Complete,
    };
    let captured = gbcli_view::capture(machine.core(), choice);
    let frame = captured.map_err(|e| view_failure(&e, &rebase, &base))?;
    let shown = match frame.source {
        FrameSource::Complete(tag) => ShotFrame::Complete(FrameShown {
            n: rebase(tag.number),
            vblank: tag.kind,
        }),
        FrameSource::Live { ppu, at, after } => ShotFrame::LivePartial {
            ppu: PpuView::of(&ppu),
            ticks: at.0,
            after: after.map(rebase),
        },
    };
    let mut image = frame.image();
    let mut region = frame;
    if let Some(r) = args.crop.map(rect) {
        let fail = |e| view_failure(&e, &rebase, &base);
        image = image.cropped(r).map_err(fail)?;
        region = region.cropped(r).map_err(fail)?;
    }
    let screen = gbcli_view::screen(&region);
    let scale = args.scale.unwrap_or(DEFAULT_SCALE);
    let mut image = image
        .scaled(scale)
        .map_err(|e| view_failure(&e, &rebase, &base))?;
    let made = ShotMade {
        path: args.path.clone(),
        width: image.width,
        height: image.height,
        scale,
        since: base.clone(),
        frame: shown,
        pixels: gbcli_view::fingerprint(&screen),
        screen: args.screen.then_some(screen),
    };
    // The view labels the frame from power-on; the PNG says what the result says.
    for (key, text) in &mut image.labels {
        if *key == LabelKey::Frame {
            *text = made.frame_label();
        }
    }
    let bytes = image.png().map_err(|e| view_failure(&e, &rebase, &base))?;
    files.write(&args.path, &bytes)?;
    Ok(made)
}

/// The wire's crop as the view's rectangle.
pub(super) fn rect(c: Crop) -> Rect {
    Rect {
        x: c.x,
        y: c.y,
        width: c.width,
        height: c.height,
    }
}

fn bad(message: String, remedy: String) -> Failure {
    Failure::new(ErrorBody::new(ErrorCode::BadRequest, message, remedy))
}

pub(super) fn view_failure(
    e: &ViewError,
    rebase: &dyn Fn(u64) -> i64,
    base: &gbcli_protocol::TimeBase,
) -> Failure {
    match e {
        ViewError::NotNormal { tag } => {
            let label = ShotFrame::Complete(FrameShown {
                n: rebase(tag.number),
                vblank: tag.kind,
            })
            .label(base);
            Failure::new(ErrorBody::plain(
                ErrorCode::FrameNotNormal,
                format!("the last complete frame ({label}) is not a normal frame"),
            ))
        }
        ViewError::NoFrame => bad(
            e.to_string(),
            "run at least one frame first: `gbcli run --frames 1`".into(),
        ),
        ViewError::BadScale { .. } => {
            bad(e.to_string(), format!("pass a scale from 1 to {MAX_SCALE}"))
        }
        ViewError::CropOutside { width, height, .. } => bad(
            e.to_string(),
            format!("the frame is {width}x{height} native pixels; crop inside it"),
        ),
        ViewError::Core(core) => Failure::from_core(core),
        ViewError::Png(_)
        | ViewError::NoFrames
        | ViewError::BadColumns
        | ViewError::Mismatch { .. } => {
            Failure::new(ErrorBody::plain(ErrorCode::Internal, e.to_string()))
        }
    }
}
