//! Screenshots (design §9, §11 step 6): after a run's reads, or of the head
//! by the `shot` verb. Capturing only reads the core, so neither advances it.
//! The frame number is rebased to the request's time base, as the stop
//! record's `frame_shown` is, and the PNG's label says the same.

use std::fs;
use std::path::Path;

use gbcli_bundle::SessionStore;
use gbcli_engine::{Machine, Since};
use gbcli_protocol::{
    ErrorBody, ErrorCode, FrameShown, PpuView, ResultBody, SessionFile, ShotArgs, ShotFrame,
    ShotMade, ShotReadArgs,
};
use gbcli_view::{DEFAULT_SCALE, FrameChoice, FrameSource, LabelKey, MAX_SCALE, Rect, ViewError};

use super::build::Host;
use super::{Failure, Handler, Need};

impl Handler {
    pub(super) fn shot(
        &mut self,
        store: &SessionStore,
        meta: &SessionFile,
        args: &ShotReadArgs,
        warnings: &mut Vec<String>,
    ) -> Result<ResultBody, Failure> {
        check(&args.shot, &self.host)?;
        let since = self.since(store, args.since.as_ref())?;
        let (machine, loaded) = self.machine_for(store, meta, Need::Head, None)?;
        warnings.extend(loaded);
        let shot = take(&machine, &args.shot, &since, &self.host);
        self.keep_warm(store, meta, machine);
        Ok(ResultBody {
            shot: Some(shot?),
            ..ResultBody::default()
        })
    }
}

/// What a shot can get wrong before the machine moves: the scale, an empty
/// crop, and a directory that isn't there.
pub(super) fn check(args: &ShotArgs, host: &Host) -> Result<(), Failure> {
    check_image(&args.path, args.scale, host)?;
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
    if args.crop.is_some_and(|c| c.width == 0 || c.height == 0) {
        return Err(bad(
            "a crop needs a width and a height".into(),
            "give the crop as x, y, width, height in native pixels".into(),
        ));
    }
    Ok(())
}

/// A PNG's path and scale, checked before anything is drawn; the path is
/// the request's, resolved by `host`.
pub(super) fn check_image(path_text: &str, scale: Option<u32>, host: &Host) -> Result<(), Failure> {
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
        && !host.resolve(dir).is_dir()
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
    host: &Host,
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
    if let Some(c) = args.crop {
        let rect = Rect {
            x: c.x,
            y: c.y,
            width: c.width,
            height: c.height,
        };
        image = image
            .cropped(rect)
            .map_err(|e| view_failure(&e, &rebase, &base))?;
    }
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
    };
    // The view labels the frame from power-on; the PNG says what the result says.
    for (key, text) in &mut image.labels {
        if *key == LabelKey::Frame {
            *text = made.frame_label();
        }
    }
    let bytes = image.png().map_err(|e| view_failure(&e, &rebase, &base))?;
    write_atomically(&host.resolve(&args.path), &bytes)?;
    Ok(made)
}

/// Writes through a temporary file beside the target, so a reader never
/// sees half a PNG.
pub(super) fn write_atomically(path: &Path, bytes: &[u8]) -> Result<(), Failure> {
    let temp = temp_beside(path);
    let written = fs::write(&temp, bytes).and_then(|()| fs::rename(&temp, path));
    written.map_err(|e| {
        let _ = fs::remove_file(&temp);
        Failure::new(ErrorBody::new(
            ErrorCode::IoFailed,
            format!("couldn't write {}: {e}", path.display()),
            ErrorCode::IoFailed.default_remedy(),
        ))
    })
}

/// Writes every file or none: a directory in a file's place is refused
/// before anything is written, then each goes to a temporary file beside it,
/// and only once all of them are written does each replace its file, by a
/// rename in its own directory. Every reported failure but one leaves the
/// files as they were and no temporary file behind; the exception, a rename
/// refused after another succeeded, needs a change on disk between the two.
/// The renames are one at a time, so a process killed between them leaves
/// the files it renamed new beside the rest old, and the rest's temporary
/// files behind: atomic for each file, not for the set.
pub(super) fn write_all_atomically(files: &[(&Path, &[u8])]) -> Result<(), Failure> {
    let io = |path: &Path, e: &dyn std::fmt::Display| {
        Failure::new(ErrorBody::new(
            ErrorCode::IoFailed,
            format!("couldn't write {}: {e}", path.display()),
            ErrorCode::IoFailed.default_remedy(),
        ))
    };
    if let Some((path, _)) = files.iter().find(|(path, _)| path.is_dir()) {
        return Err(io(path, &"a directory has its name"));
    }
    let temps: Vec<_> = files.iter().map(|(path, _)| temp_beside(path)).collect();
    let clean = || {
        for temp in &temps {
            let _ = fs::remove_file(temp);
        }
    };
    for ((path, bytes), temp) in files.iter().zip(&temps) {
        if let Err(e) = fs::write(temp, bytes) {
            clean();
            return Err(io(path, &e));
        }
    }
    for ((path, _), temp) in files.iter().zip(&temps) {
        if let Err(e) = fs::rename(temp, path) {
            clean();
            return Err(io(path, &e));
        }
    }
    Ok(())
}

fn temp_beside(path: &Path) -> std::path::PathBuf {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    path.with_file_name(format!(".{name}.{}.tmp", gbcli_bundle::process_tag()))
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
