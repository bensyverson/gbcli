//! The sound-register write log and sample capture.
//!
//! The log rides SameBoy's write callback (`watch::on_write`), which sees every
//! CPU write and every `GB_write_memory`, before the APU applies it; nothing
//! else writes the sound registers (OAM DMA and HDMA can't reach IO, and a
//! poke to the IO region bypasses the APU). Samples come from SameBoy's own
//! renderer through its sample callback, at [`AudioCapture::SAMPLE_RATE`].

use gbcli_core::{
    ApuChannel, ApuWrite, ApuWriteEffect, AudioCapture, AudioSamples, CoreError, Feature,
    StereoSample, Ticks,
};
use sameboy_sys as sb;

use crate::SameBoy;
use crate::hooks::{Hooks, guard};

/// Why capture is refused on SGB models: SameBoy clocks them at the SNES's
/// rate, so a sample is not a whole number of ticks, and it renders nothing
/// during the SGB intro animation.
pub(crate) const SGB_AUDIO_REASON: &str =
    "the SGB runs at the SNES's clock, so its samples don't fall on whole ticks";

/// When a callback fired: the start of the current step plus how far SameBoy
/// got into it, or the step's start outside a run (a bus write).
pub(crate) fn tap_time(gb: *mut sb::GB_gameboy_t, hooks: &Hooks) -> Ticks {
    let into = if hooks.in_run {
        // SAFETY: a plain read of the live instance, inside its callback.
        u64::from(unsafe { sb::gbcli_cycles_since_run(gb) })
    } else {
        0
    };
    Ticks(hooks.step_ticks + into)
}

/// Records a sound write in the log, if the log is on. Called from the write
/// callback with the address the write lands at.
#[inline(always)]
pub(crate) fn log_write(gb: *mut sb::GB_gameboy_t, hooks: &mut Hooks, addr: u16, value: u8) {
    if hooks.apu_log.is_none() || !gbcli_core::is_sound_addr(addr) {
        return;
    }
    // SAFETY: a side-effect-free accessor on the live instance.
    let effect = match unsafe { sb::gbcli_apu_write_effect(gb, addr) } {
        sb::GBCLI_APU_WRITE_IGNORED_OFF => ApuWriteEffect::IgnoredApuOff,
        sb::GBCLI_APU_WRITE_WAVE_PLAYING => ApuWriteEffect::WaveRamWhilePlaying,
        _ => ApuWriteEffect::Applied,
    };
    let write = ApuWrite {
        at: tap_time(gb, hooks),
        frame: hooks.frame.tag.map_or(0, |tag| tag.number),
        // SAFETY: a side-effect-free accessor on the live instance. As the
        // raster log reads it: out of range with the LCD off.
        line: u8::try_from(unsafe { sb::gbcli_ppu_line(gb) }).ok(),
        pc: hooks.step_pc,
        // SAFETY: a side-effect-free accessor on the live instance. The
        // write that unmaps the boot ROM ($FF50) is no sound write, so the
        // mapping now is the mapping the instruction was fetched under.
        boot_rom: unsafe { sb::gbcli_reads_boot_rom(gb, hooks.step_pc) },
        addr,
        value,
        effect,
    };
    if let Some(log) = &mut hooks.apu_log {
        log.push(write);
    }
}

pub(crate) unsafe extern "C" fn on_sample(gb: *mut sb::GB_gameboy_t, sample: *mut sb::GB_sample_t) {
    guard(gb, (), |hooks| {
        if hooks.audio.is_none() || sample.is_null() {
            return;
        }
        let at = tap_time(gb, hooks);
        // SAFETY: SameBoy passes a valid sample for the call.
        let sample = unsafe { *sample };
        if let Some(audio) = &mut hooks.audio {
            audio.first.get_or_insert(at);
            audio.samples.push(StereoSample {
                left: sample.left,
                right: sample.right,
            });
        }
    });
}

fn channel_id(channel: ApuChannel) -> sb::GB_channel_t {
    match channel {
        ApuChannel::Square1 => sb::GB_SQUARE_1,
        ApuChannel::Square2 => sb::GB_SQUARE_2,
        ApuChannel::Wave => sb::GB_WAVE,
        ApuChannel::Noise => sb::GB_NOISE,
    }
}

impl SameBoy {
    pub(crate) fn audio_refusal(&self) -> Option<&'static str> {
        self.model.is_sgb().then_some(SGB_AUDIO_REASON)
    }

    pub(crate) fn set_apu_log(&mut self, on: bool) {
        let gb = self.raw();
        let hooks = self.hooks_mut();
        hooks.apu_log = on.then(Vec::new);
        let callback: sb::GB_write_memory_callback_t = if hooks.write_hook_wanted() {
            Some(crate::watch::on_write)
        } else {
            None
        };
        // SAFETY: outside GB_run; the callback reaches the hooks through user data.
        unsafe { sb::GB_set_write_memory_callback(gb, callback) };
    }

    pub(crate) fn set_capture(&mut self, capture: Option<AudioCapture>) -> Result<(), CoreError> {
        if capture.is_some()
            && let Some(reason) = self.audio_refusal()
        {
            return Err(CoreError::Unsupported {
                feature: Feature::AudioSamples,
                reason,
            });
        }
        let gb = self.raw();
        let muted = capture.map_or([false; 4], |c| c.muted);
        // SAFETY: setters on a live instance outside GB_run.
        unsafe {
            for channel in ApuChannel::ALL {
                sb::GB_set_channel_muted(gb, channel_id(channel), muted[channel as usize]);
            }
            match capture {
                Some(_) => {
                    sb::GB_set_sample_rate(gb, AudioCapture::SAMPLE_RATE);
                    sb::GB_apu_set_sample_callback(gb, Some(on_sample));
                    // Cycles still batched would be rendered after the
                    // restart, a few ticks late; run them first, so capture
                    // starts at this moment whatever the APU had batched.
                    // What they render is dropped with the samples below.
                    sb::gbcli_sync_apu(gb);
                    sb::gbcli_audio_restart(gb);
                }
                None => {
                    sb::GB_set_sample_rate(gb, 0);
                    sb::GB_apu_set_sample_callback(gb, None);
                }
            }
        }
        self.hooks_mut().audio = capture.map(|_| AudioSamples::default());
        Ok(())
    }

    /// After a load: what was collected belongs to the replaced timeline.
    pub(crate) fn audio_after_load(&mut self) {
        let gb = self.raw();
        let hooks = self.hooks_mut();
        if let Some(log) = &mut hooks.apu_log {
            log.clear();
        }
        if let Some(log) = &mut hooks.lcd_off_log {
            log.clear();
        }
        if hooks.audio.is_some() {
            // As in `set_capture`: a state saved by another build may hold
            // batched cycles. The sync may render into the hooks, so it runs
            // before the samples are cleared, through no live reference.
            // SAFETY: outside GB_run; no reference into the hooks is alive.
            unsafe {
                sb::gbcli_sync_apu(gb);
                sb::gbcli_audio_restart(gb);
            }
            if let Some(audio) = &mut self.hooks_mut().audio {
                *audio = AudioSamples::default();
            }
        }
    }
}
