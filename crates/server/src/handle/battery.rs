//! `power_cycle` and `sav` (design §6, the 2026-11-07 battery-save ruling):
//! the battery's RAM and clock across a power cycle, and to and from a
//! `.sav` file only when asked.

use std::fs;
use std::path::Path;

use gbcli_bundle::{SessionStore, parse_duration};
use gbcli_core::{MAX_RTC_ADVANCE_S, SavClock};
use gbcli_engine::{Battery, LoadOptions, Machine};
use gbcli_protocol::{
    BatteryFrom, ErrorBody, ErrorCode, PowerCycleArgs, PowerCycleMade, ResultBody, SavArgs,
    SavClockState, SavMade, SessionFile,
};

use super::shot::write_atomically;
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
        if rtc_advance > MAX_RTC_ADVANCE_S {
            return Err(Failure::new(ErrorBody::new(
                ErrorCode::BadRequest,
                format!(
                    "--rtc-advance {} is past 1024 days, the most a cart clock moves at once: an \
                     MBC3 clock counts 512 days and then only flags the overflow, so a longer \
                     advance reads like one 512 days shorter",
                    args.rtc_advance.as_deref().unwrap_or_default()
                ),
                "advance by at most 1024d; 512d from day 0 is enough to overflow the day \
                 counter and set its carry flag",
            )));
        }
        let file = args.sav.as_deref().map(read_sav).transpose()?;
        self.forget_warm(store.name());
        let stored = |e| Failure::from_store(&e);
        let mut machine = store.machine(meta).map_err(stored)?;
        // Breakpoints and watches are the session's, not the machine's, and
        // save_head writes back whatever the machine holds.
        *machine.debugger_config_mut() = meta.debugger.clone();
        let head = store.read_head(meta.rev).map_err(stored)?;
        let rom_rebuilt = head
            .as_ref()
            .is_some_and(|h| h.header.rom_sha256 != machine.rom_sha256());
        let mut head_state = None;
        match &head {
            Some(head) if rom_rebuilt => {
                // Only the battery crosses a rebuild; the rest of the state
                // belongs to the old ROM. The old state is read under the new
                // build's header (the old ROM is gone), so the cart on disk
                // decides whether there is a battery. A --sav replaces it.
                if file.is_none() {
                    let mut old = store.machine(meta).map_err(stored)?;
                    let forced = LoadOptions {
                        allow_rom_change: true,
                        ..LoadOptions::default()
                    };
                    store.restore(&mut old, head, &forced).map_err(stored)?;
                    head_state = old
                        .core_mut()
                        .battery_state()
                        .map_err(|e| Failure::from_core(&e))?;
                }
                warnings.push(
                    "the head was saved over another build of the ROM: power-cycled into the ROM \
                     on disk, carrying only the battery save"
                        .into(),
                );
            }
            Some(head) => {
                warnings.extend(
                    store
                        .restore(&mut machine, head, &self.load)
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
        let cycled = machine
            .power_cycle(battery, rtc_advance)
            .map_err(|e| Failure::from_core(&e))?;
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
        self.keep_warm(store, meta, machine);
        let sav = exported
            .map_err(|e| Failure::from_core(&e))?
            .ok_or_else(|| {
                Failure::new(ErrorBody::new(
                    ErrorCode::BadRequest,
                    "this cart has no battery, so it keeps nothing to save",
                    NO_BATTERY_REMEDY,
                ))
            })?;
        write_atomically(Path::new(&args.path), &sav)?;
        Ok(ResultBody {
            sav: Some(SavMade {
                path: args.path.clone(),
                bytes: sav.len() as u64,
                ram_bytes: ram_bytes as u64,
            }),
            ..ResultBody::default()
        })
    }
}

/// The ROM header's disagreements, as warnings with their fix.
pub(super) fn header_warnings(machine: &Machine) -> Vec<String> {
    machine
        .header_mismatches()
        .iter()
        .map(|m| format!("{m}; {}", m.remedy()))
        .collect()
}

fn read_sav(path: &str) -> Result<Vec<u8>, Failure> {
    fs::read(path).map_err(|e| {
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
