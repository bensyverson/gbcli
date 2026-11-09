//! `power_cycle` and `sav` (design §6, the 2026-11-07 battery-save ruling):
//! the battery's RAM and clock across a power cycle, and to and from a
//! `.sav` file only when asked.

use std::fs;
use std::path::Path;

use gbcli_bundle::{SessionStore, StoreError, parse_duration};
use gbcli_core::{CartClock, SavClock};
use gbcli_engine::{Battery, Flash, LoadOptions, LoadWarning, Machine, flash_image_path};
use gbcli_protocol::{
    BatteryFrom, ErrorBody, ErrorCode, FlashFileMade, FlashFrom, PowerCycleArgs, PowerCycleMade,
    ResultBody, SavArgs, SavClockState, SavMade, SessionFile,
};

use super::build::Host;
use super::shot::write_all_atomically;
use super::{Failure, Handler, HeadChange, Need, save_head};

/// The remedy for a cart that keeps nothing with the power off.
const NO_BATTERY_REMEDY: &str = "give the ROM a cart type with a battery: `rgbfix -m \
                                 MBC5+RAM+BATTERY -r 2` (or MBC3+TIMER+RAM+BATTERY for a clock)";

impl Handler {
    /// Power-cycles the head. A head saved over another build of the ROM is
    /// never loaded as a state, so the changed-ROM check can't refuse it:
    /// its battery is carried into a machine built from the ROM on disk.
    pub(super) fn power_cycle(
        &mut self,
        store: &SessionStore,
        meta: &mut SessionFile,
        args: &PowerCycleArgs,
        actor: &Option<String>,
        warnings: &mut Vec<String>,
    ) -> Result<ResultBody, Failure> {
        let rtc_advance = match &args.rtc_advance {
            Some(text) => parse_duration(text)
                .map_err(|e| Failure::from_store(&e))?
                .as_secs(),
            None => 0,
        };
        let flash_session = meta.flash.is_some();
        let (file, image) = match args.sav.as_deref() {
            None => (None, None),
            Some(path) if flash_session => read_flash_pair(path, &self.host, warnings)?,
            Some(path) => (Some(read_sav(path, &self.host)?), None),
        };
        self.forget_warm(store.name());
        let stored = |e| Failure::from_store(&e);
        let mut machine = store.machine(meta).map_err(stored)?;
        if rtc_advance > 0 {
            // The ROM on disk decides the clock: it builds the new core.
            let clock = machine
                .core()
                .cart_clock()
                .map_err(|e| Failure::from_core(&e))?;
            if let Some(clock) = clock.filter(|c| rtc_advance > c.max_advance_s()) {
                let text = args.rtc_advance.as_deref().unwrap_or_default();
                return Err(advance_too_far(clock, text));
            }
        }
        // Breakpoints and watches are the session's, not the machine's, and
        // save_head writes back whatever the machine holds.
        *machine.debugger_config_mut() = meta.debugger.clone();
        let head = store.read_head(meta.rev).map_err(stored)?;
        let rom_rebuilt = head
            .as_ref()
            .is_some_and(|h| h.header.rom_sha256 != machine.rom_sha256());
        let (mut head_state, mut head_flash) = (None, None);
        let mut dropped = Vec::new();
        match &head {
            Some(head) if rom_rebuilt => {
                // Only the battery (and a flash chip's changed sectors, the
                // save) crosses a rebuild; the rest of the state belongs to
                // the old ROM. The old state is read under the new build's
                // header (the old ROM is gone), so the cart on disk decides
                // whether there is a battery. A --sav (and its chip image)
                // replaces what it holds.
                // The restore gives the sectors where the new image has code
                // back to it (the rule a power cycle carries by too), and
                // names them.
                let wants_flash = flash_session && image.is_none();
                if file.is_none() || wants_flash {
                    let mut old = store.machine(meta).map_err(stored)?;
                    let forced = LoadOptions {
                        allow_rom_change: true,
                        ..LoadOptions::default()
                    };
                    let loaded = old
                        .restore(head, &forced)
                        .map_err(|e| Failure::from_store(&StoreError::Checkpoint(e)))?;
                    if wants_flash {
                        dropped.extend(
                            loaded
                                .into_iter()
                                .filter(|w| matches!(w, LoadWarning::FlashSectorsDropped { .. })),
                        );
                    }
                    let core = |e| Failure::from_core(&e);
                    if file.is_none() {
                        head_state = old.core_mut().battery_state().map_err(core)?;
                    }
                    if wants_flash {
                        head_flash = old.core_mut().flash_contents().map_err(core)?;
                    }
                }
                warnings.push(if flash_session {
                    "the head was saved over another build of the ROM: power-cycled into the ROM \
                     on disk, carrying only the battery save and the flash sectors the program \
                     changed"
                        .into()
                } else {
                    "the head was saved over another build of the ROM: power-cycled into the ROM \
                     on disk, carrying only the battery save"
                        .into()
                });
            }
            Some(head) => {
                warnings.extend(
                    store
                        .restore(meta, &mut machine, head, &self.load)
                        .map_err(stored)?,
                );
            }
            None => {}
        }
        warnings.extend(header_warnings(&machine));
        let battery = match (&file, &head_state) {
            (Some(sav), _) => Battery::Sav(sav),
            (None, Some(state)) => Battery::Carried(state),
            (None, None) => Battery::Keep,
        };
        let flash = match (&image, &head_flash) {
            (Some(image), _) => Flash::Image(image),
            (None, Some(contents)) => Flash::Carried(contents),
            (None, None) => Flash::Keep,
        };
        let cycled = machine
            .power_cycle(battery, flash, rtc_advance)
            .map_err(|e| Failure::from_core(&e))?;
        if cycled.flash_sectors_in_image > 0 {
            warnings.push(format!(
                "the flash image differs from the ROM on disk in {} sector(s) inside the game's \
                 own {} bytes: the chip holds the image's bytes there, which are another build's \
                 code if the image is from one",
                cycled.flash_sectors_in_image,
                machine.rom_len()
            ));
        }
        if !cycled.flash_sectors_dropped.is_empty() {
            dropped.push(LoadWarning::FlashSectorsDropped {
                sectors: cycled.flash_sectors_dropped.clone(),
            });
        }
        warnings.extend(
            dropped
                .iter()
                .map(|w| StoreError::warning_text(w, &meta.rom)),
        );
        let from = match (file.is_some(), cycled.battery_bytes) {
            (true, _) => BatteryFrom::Sav,
            (false, Some(_)) => BatteryFrom::Head,
            (false, None) => BatteryFrom::None,
        };
        let change = HeadChange {
            actor: actor.clone(),
            ..HeadChange::default()
        };
        save_head(store, meta, &mut machine, change)?;
        Ok(ResultBody {
            power_cycle: Some(PowerCycleMade {
                battery: from,
                bytes: cycled.battery_bytes.map(|n| n as u64),
                clock: clock_state(cycled.clock),
                rtc_advanced_s: rtc_advance,
                rom_rebuilt,
                flash: flash_session.then_some(if image.is_some() {
                    FlashFrom::Image
                } else {
                    FlashFrom::Head
                }),
                flash_bytes: cycled.flash_bytes.map(|n| n as u64),
            }),
            ..ResultBody::default()
        })
    }

    /// Writes the head's battery save to `args.path`.
    pub(super) fn sav(
        &mut self,
        store: &SessionStore,
        meta: &mut SessionFile,
        args: &SavArgs,
        warnings: &mut Vec<String>,
    ) -> Result<ResultBody, Failure> {
        let (mut machine, loaded) = self.machine_for(store, meta, Need::Head, None)?;
        warnings.extend(loaded);
        let exported = machine.core_mut().battery_sav();
        let ram_bytes = machine
            .core()
            .region(gbcli_core::Region::Sram)
            .map_or(0, |r| r.bytes.len());
        let image = machine.flash_image();
        self.keep_warm(store, meta, machine);
        let sav = exported.map_err(|e| Failure::from_core(&e))?;
        if sav.is_none() && image.is_none() {
            return Err(Failure::new(ErrorBody::new(
                ErrorCode::BadRequest,
                "this cart has no battery, so it keeps nothing to save",
                NO_BATTERY_REMEDY,
            )));
        }
        // A flash session's save may live in the chip (GB Studio's
        // batteryless saves do), so the chip image goes beside the `.sav`;
        // both are written or neither, as one alone is half a save (short
        // of a kill between the two renames: `write_all_atomically`).
        let image_path = flash_image_path(Path::new(&args.path));
        let (sav_at, image_at) = (
            self.host.resolve(&args.path),
            self.host.resolve(&image_path),
        );
        let mut files: Vec<(&Path, &[u8])> = Vec::new();
        if let Some(sav) = &sav {
            files.push((&sav_at, sav));
        }
        if let Some(image) = &image {
            files.push((&image_at, image));
        }
        write_all_atomically(&files)?;
        let flash = image.as_ref().map(|image| FlashFileMade {
            path: image_path.display().to_string(),
            bytes: image.len() as u64,
        });
        Ok(ResultBody {
            sav: Some(SavMade {
                path: sav.as_ref().map(|_| args.path.clone()),
                bytes: sav.as_ref().map_or(0, |s| s.len() as u64),
                ram_bytes: ram_bytes as u64,
                flash,
            }),
            ..ResultBody::default()
        })
    }
}

/// The refusal of an advance past `clock`'s bound, in that cart's terms.
fn advance_too_far(clock: CartClock, text: &str) -> Failure {
    let remedy = match clock {
        CartClock::Mbc3 => {
            "advance by at most 1024d; 512d from day 0 is enough to overflow the day counter and \
             set its carry flag"
        }
        CartClock::Tpp1 => {
            "advance by at most 3584d; 1792d from week 0 is enough to overflow the week counter \
             and set MR4's overflow flag"
        }
        CartClock::Huc3 => {
            "advance by at most 65536d; 65535d from day 0 reaches the last day before the day \
             counter wraps"
        }
    };
    Failure::new(ErrorBody::new(
        ErrorCode::BadRequest,
        format!(
            "--rtc-advance {text} is past {} days, the most this cart's clock moves at once: {}",
            clock.max_advance_days(),
            clock.bound_reason()
        ),
        remedy,
    ))
}

/// The ROM header's disagreements, as warnings with their fix.
pub(super) fn header_warnings(machine: &Machine) -> Vec<String> {
    machine
        .header_mismatches()
        .iter()
        .map(|m| format!("{m}; {}", m.remedy()))
        .collect()
}

/// A `--sav`'s files: the `.sav`, and on a flash session the chip image.
type SavFiles = (Option<Vec<u8>>, Option<Vec<u8>>);

/// A flash session's `--sav PATH`: the `.sav` and the chip image beside it
/// (`flash_image_path`), each read if it exists, at least one of them. A
/// half that is missing keeps the head's own, with a warning.
fn read_flash_pair(
    path: &str,
    host: &Host,
    warnings: &mut Vec<String>,
) -> Result<SavFiles, Failure> {
    let image_path = flash_image_path(Path::new(path));
    let (sav_there, image_there) = (
        host.resolve(path).exists(),
        host.resolve(&image_path).exists(),
    );
    if !sav_there && !image_there {
        return Err(Failure::new(ErrorBody::new(
            ErrorCode::IoFailed,
            format!(
                "neither the .sav {path} nor its flash image {} exists",
                image_path.display()
            ),
            "check the path; `gbcli sav <file>` writes both from a flash session",
        )));
    }
    let sav = sav_there.then(|| read_sav(path, host)).transpose()?;
    let image = image_there
        .then(|| read_sav(&image_path.display().to_string(), host))
        .transpose()?;
    if sav.is_none() {
        warnings.push(format!("no .sav at {path}: the cart RAM is the head's"));
    }
    if image.is_none() {
        warnings.push(format!(
            "no flash image at {}: the flash chip keeps the head's contents",
            image_path.display()
        ));
    }
    Ok((sav, image))
}

fn read_sav(path: &str, host: &Host) -> Result<Vec<u8>, Failure> {
    fs::read(host.resolve(path)).map_err(|e| {
        Failure::new(ErrorBody::new(
            ErrorCode::IoFailed,
            format!("couldn't read the .sav {path}: {e}"),
            "check the path; `gbcli sav <file>` writes one from a session",
        ))
    })
}

fn clock_state(clock: SavClock) -> SavClockState {
    match clock {
        SavClock::NotClocked => SavClockState::NotClocked,
        SavClock::Restored => SavClockState::Restored,
        SavClock::FooterMissing => SavClockState::FooterMissing,
    }
}
