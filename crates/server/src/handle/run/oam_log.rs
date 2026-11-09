//! A run's OAM log (`--oam-log`): OAM at every vblank the run passes, read
//! by the run loop's frame sink as `view oam` reads it at a stop. A vblank
//! stop comes before the game's own vblank code, so each entry is the OAM
//! the frame that just ended was drawn from.

use std::cell::RefCell;

use gbcli_core::{CoreError, VblankKind};
use gbcli_engine::inspect::video::{ObjectHeight, oam, object_height};
use gbcli_engine::{Machine, Since};
use gbcli_protocol::{OamFrameView, OamLogMade};

use super::super::Failure;
use super::super::view::oam_view;

/// One frame as read, numbered from power-on.
struct Read {
    number: u64,
    lcd_off: bool,
    height: u8,
    objects: Vec<gbcli_protocol::OamView>,
}

/// Collects OAM as the run passes vblanks.
#[derive(Default)]
pub(super) struct Collector {
    frames: RefCell<Vec<Read>>,
    error: RefCell<Option<CoreError>>,
}

impl Collector {
    /// The frame sink.
    pub(super) fn on_frame(&self, machine: &Machine) {
        if self.error.borrow().is_some() {
            return;
        }
        match read(machine) {
            Ok(frame) => self.frames.borrow_mut().push(frame),
            Err(e) => *self.error.borrow_mut() = Some(e),
        }
    }

    /// The frames collected, numbered from the request's time base.
    pub(super) fn finish(&self, machine: &Machine, since: &Since) -> Result<OamLogMade, Failure> {
        if let Some(e) = self.error.take() {
            return Err(Failure::from_core(&e));
        }
        let (base, mark) = machine.named_base(since);
        let frames = self
            .frames
            .take()
            .into_iter()
            .map(|f| OamFrameView {
                frame: f.number as i64 - mark.frame as i64,
                lcd_off: f.lcd_off,
                height: f.height,
                objects: f.objects,
            })
            .collect();
        Ok(OamLogMade {
            since: base,
            frames,
        })
    }
}

fn read(machine: &Machine) -> Result<Read, CoreError> {
    let core = machine.core();
    let tag = core.last_complete_frame().map(|f| f.tag);
    let height = object_height(core)?;
    let objects = oam(core, height)?
        .iter()
        .filter(|o| o.on_screen)
        .map(oam_view)
        .collect();
    Ok(Read {
        number: tag.map_or(0, |t| t.number),
        lcd_off: tag.is_some_and(|t| t.kind == VblankKind::LcdOff),
        height: match height {
            ObjectHeight::Eight => 8,
            ObjectHeight::Sixteen => 16,
        },
        objects,
    })
}
