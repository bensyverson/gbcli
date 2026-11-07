//! The SameBoy backend of gbcli's [`Core`] trait, over vendored SameBoy
//! (`sameboy-sys`). Closes the traps the backend evaluation lists: stdout
//! logging, stdin input, real-time pacing, the global RNG, host time in the
//! RTC, the illegal-opcode lock, and side effects of SameBoy's own reads.

mod audio;
mod blob;
mod display;
mod hooks;
mod memory;
mod run;
mod support;
mod watch;

use std::ptr::NonNull;
use std::sync::Mutex;

use gbcli_core::{
    ApuState, ApuWrite, AudioCapture, AudioSamples, BootRom, Budget, BudgetKind, ColorMode, Core,
    CoreConfig, CoreError, CoreInfo, Feature, FrameRef, IoReg, JoypadBounce, Keys, LayerMap,
    LiveFrame, Model, PpuMode, PpuStatus, Region, RegionView, Registers, Rendering, RtcMode,
    RunRequest, StepUnit, Stop, StopReason, Support, Ticks, WriteTracking, WrittenView,
};
use sameboy_sys as sb;

use hooks::Hooks;
pub use support::builtin_boot_rom;
pub(crate) use support::unsupported_err;
use support::{sameboy_model, support};

/// SameBoy's RNG is process-global: seeding and `GB_init` happen under this
/// lock so two constructions can't interleave.
static CONSTRUCTION: Mutex<()> = Mutex::new(());

/// The smallest ROM with a complete header.
const MIN_ROM_LEN: usize = 0x150;

/// One SameBoy instance behind the core trait.
pub struct SameBoy {
    gb: NonNull<sb::GB_gameboy_t>,
    /// Owned; reached by callbacks through SameBoy's user data. Freed after the instance.
    hooks: NonNull<Hooks>,
    model: Model,
    /// The ROM file's length (SameBoy rounds its buffer up to a power of two).
    rom_len: usize,
    ticks: u64,
    vblanks: u64,
    keys: Keys,
    /// `GB_get_registers`, cached: the run loop reads PC before every step.
    regs: *mut sb::GB_registers_t,
}

impl SameBoy {
    /// Builds a core with its determinism setup done: callbacks routed away from
    /// stdio, turbo uncapped, RNG seeded, RTC on emulated time, bounce as asked.
    pub fn new(config: CoreConfig) -> Result<SameBoy, CoreError> {
        if config.rom.len() < MIN_ROM_LEN {
            return Err(CoreError::InvalidState {
                reason: format!(
                    "a ROM needs at least {MIN_ROM_LEN:#x} bytes for its header; got {:#x}",
                    config.rom.len()
                ),
            });
        }
        let boot_rom: &[u8] = match &config.boot_rom {
            BootRom::BuiltIn => builtin_boot_rom(config.model),
            BootRom::Custom(bytes) => bytes,
        };
        let rendering = config.rendering == Rendering::On;
        let hooks = Box::new(Hooks::new(config.model.has_dmg_shades(), rendering));
        let hooks = NonNull::from(Box::leak(hooks));

        let gb = {
            let _lock = CONSTRUCTION.lock().unwrap_or_else(|e| e.into_inner());
            // SAFETY: GB_alloc returns an uninitialized instance that GB_init fills.
            unsafe {
                sb::GB_random_seed(config.seed);
                sb::GB_init(sb::GB_alloc(), sameboy_model(config.model))
            }
        };
        let gb = NonNull::new(gb).expect("GB_init returned null");
        let raw = gb.as_ptr();
        // SAFETY: a freshly initialized instance; `hooks` outlives it (Drop frees
        // it after GB_free), and the palette is a static.
        unsafe {
            sb::GB_set_user_data(raw, hooks.as_ptr().cast());
            sb::GB_set_log_callback(raw, Some(hooks::on_log));
            sb::GB_set_input_callback(raw, Some(hooks::no_input));
            sb::GB_set_async_input_callback(raw, Some(hooks::no_input));
            sb::GB_set_turbo_mode(raw, true, true);
            sb::GB_set_turbo_cap(raw, 0.0);
            match config.rtc {
                RtcMode::Accurate => sb::GB_set_rtc_mode(raw, sb::GB_RTC_MODE_ACCURATE),
            }
            sb::gbcli_clear_rtc_epoch(raw);
            sb::GB_set_emulate_joypad_bouncing(raw, config.joypad_bounce == JoypadBounce::On);
            if config.model.has_dmg_shades() {
                sb::GB_set_palette(raw, &hooks::SENTINEL_PALETTE);
            }
            sb::GB_set_rgb_encode_callback(raw, Some(hooks::encode_rgb));
            sb::GB_set_pixels_output(raw, (*hooks.as_ptr()).screen.as_mut_ptr());
            sb::GB_set_vblank_callback(raw, Some(hooks::on_vblank));
            sb::GB_set_rendering_disabled(raw, !rendering);
            sb::GB_load_boot_rom_from_buffer(raw, boot_rom.as_ptr(), boot_rom.len());
            sb::GB_load_rom_from_buffer(raw, config.rom.as_ptr(), config.rom.len());
        }
        let mut core = SameBoy {
            gb,
            hooks,
            // SAFETY: points into the instance, which never moves.
            regs: unsafe { sb::GB_get_registers(raw) },
            model: config.model,
            rom_len: config.rom.len(),
            ticks: 0,
            vblanks: 0,
            keys: Keys::NONE,
        };
        if config.write_tracking == WriteTracking::On {
            let len = |region| memory::region(&core, region).map_or(0, |v| v.bytes.len());
            let tracking =
                watch::Tracking::new(len(Region::Wram), len(Region::Hram), len(Region::Sram));
            core.hooks_mut().tracking = Some(tracking);
            // SAFETY: installs our callback; it reaches `hooks` through user data.
            unsafe { sb::GB_set_write_memory_callback(raw, Some(watch::on_write)) };
        }
        Ok(core)
    }

    /// SameBoy's log lines since the last call (the most recent 256).
    pub fn take_log(&mut self) -> Vec<String> {
        std::mem::take(&mut self.hooks_mut().log)
    }

    /// Pixels SameBoy draws for this model (more for SGB, which has a border).
    fn screen_pixels(&self) -> usize {
        // SAFETY: plain getters on a live instance.
        let (w, h) = unsafe {
            (
                sb::GB_get_screen_width(self.raw()),
                sb::GB_get_screen_height(self.raw()),
            )
        };
        ((w * h) as usize).min(hooks::SCREEN_PIXELS)
    }

    /// Why this core can't capture pixel layers, if it can't.
    fn layers_refusal(&self) -> Option<&'static str> {
        if self.model.is_sgb() {
            Some(support::SGB_LAYERS_REASON)
        } else if !self.hooks().rendering {
            Some(support::RENDERING_OFF_LAYERS_REASON)
        } else {
            None
        }
    }

    /// Whether no pixel of the frame in progress has been drawn yet: the LCD
    /// is off, or the PPU is in vblank or in line 0's OAM scan. STAT and LY in
    /// the IO array describe how far the PPU has run, and it draws nothing
    /// ahead of that, even while it batches.
    fn frame_untouched(&self) -> bool {
        let ppu = self.ppu();
        !ppu.lcd_on || ppu.mode == PpuMode::VBlank || (ppu.ly == 0 && ppu.mode == PpuMode::OamScan)
    }

    fn raw(&self) -> *mut sb::GB_gameboy_t {
        self.gb.as_ptr()
    }

    fn hooks(&self) -> &Hooks {
        // SAFETY: owned by self; callbacks only run inside GB_run, during which
        // no reference from here is alive.
        unsafe { self.hooks.as_ref() }
    }

    fn hooks_mut(&mut self) -> &mut Hooks {
        // SAFETY: as in `hooks`, and `&mut self` makes this the only reference.
        unsafe { self.hooks.as_mut() }
    }
}

impl Drop for SameBoy {
    fn drop(&mut self) {
        // SAFETY: the instance and hooks were created in `new` and are freed once,
        // the instance first so no callback can see freed hooks.
        unsafe {
            sb::GB_dealloc(self.raw());
            drop(Box::from_raw(self.hooks.as_ptr()));
        }
    }
}

impl Core for SameBoy {
    fn info(&self) -> CoreInfo {
        CoreInfo {
            backend: "sameboy",
            version: sb::CORE_VERSION,
            model: self.model,
        }
    }

    fn supports(&self, feature: Feature) -> Support {
        match feature {
            Feature::AudioSamples => match self.audio_refusal() {
                Some(reason) => Support::No { reason },
                None => Support::Yes,
            },
            Feature::PixelLayers => match self.layers_refusal() {
                Some(reason) => Support::No { reason },
                None => Support::Yes,
            },
            // SAFETY: a plain query on a live instance.
            Feature::CartRamWrites if !unsafe { sb::gbcli_cart_ram_mirrored(self.raw()) } => {
                Support::No {
                    reason: support::CART_RAM_REASON,
                }
            }
            _ => support(feature),
        }
    }

    fn run(&mut self, req: &mut RunRequest<'_>) -> Result<Stop, CoreError> {
        run::run(self, req)
    }

    fn step(&mut self, unit: StepUnit) -> Result<Stop, CoreError> {
        match unit {
            StepUnit::Instruction => {}
            StepUnit::MCycle => return Err(unsupported_err(Feature::MCycleStep)),
            StepUnit::Dot => return Err(unsupported_err(Feature::DotStep)),
        }
        let mut stop = run::run(self, &mut RunRequest::new(Budget::instructions(1)))?;
        if stop.reason == StopReason::Budget(BudgetKind::Instructions) {
            stop.reason = StopReason::Step(unit);
        }
        Ok(stop)
    }

    fn now(&self) -> Ticks {
        Ticks(self.ticks)
    }

    fn registers(&self) -> Registers {
        let gb = self.raw();
        // SAFETY: plain reads of a live instance.
        unsafe {
            let r = *sb::GB_get_registers(gb);
            let [f, a] = r.af.to_le_bytes();
            let [c, b] = r.bc.to_le_bytes();
            let [e, d] = r.de.to_le_bytes();
            let [l, h] = r.hl.to_le_bytes();
            Registers {
                a,
                f,
                b,
                c,
                d,
                e,
                h,
                l,
                sp: r.sp,
                pc: r.pc,
                ime: sb::gbcli_ime(gb),
                halted: sb::gbcli_halted(gb),
                double_speed: sb::gbcli_double_speed(gb),
            }
        }
    }

    fn region(&self, region: Region) -> Result<RegionView<'_>, CoreError> {
        memory::region(self, region)
    }

    fn rom0_bank(&self) -> u16 {
        memory::rom0_bank(self)
    }

    fn io_register(&self, reg: IoReg) -> Result<u8, CoreError> {
        memory::io_register(self, reg)
    }

    /// LY and the mode come from the IO array, which can lag by the cycles of
    /// a batched mode-3 stretch (see `Feature::SideEffectFreeIo`).
    fn ppu(&self) -> PpuStatus {
        let io = memory::region(self, Region::Io)
            .expect("the IO region always exists")
            .bytes;
        let mode = match io[usize::from(IoReg::STAT.0)] & 3 {
            0 => PpuMode::HBlank,
            1 => PpuMode::VBlank,
            2 => PpuMode::OamScan,
            _ => PpuMode::Drawing,
        };
        PpuStatus {
            ly: io[usize::from(IoReg::LY.0)],
            mode,
            dot: None,
            lcd_on: io[usize::from(IoReg::LCDC.0)] & 0x80 != 0,
        }
    }

    fn last_complete_frame(&self) -> Option<FrameRef<'_>> {
        let frame = &self.hooks().frame;
        Some(FrameRef {
            tag: frame.tag?,
            width: frame.width,
            height: frame.height,
            pixels: frame.pixels(),
        })
    }

    fn live_frame(&self) -> Result<LiveFrame, CoreError> {
        Ok(display::live_frame(self))
    }

    fn color_mode(&self) -> Result<ColorMode, CoreError> {
        Ok(display::color_mode(self))
    }

    fn rgb555_color(&self, color: u16) -> Result<u32, CoreError> {
        display::rgb555_color(self, color)
    }

    fn pixel_layers(&self) -> Result<LayerMap<'_>, CoreError> {
        if !self.hooks().layers_on {
            return Err(CoreError::InvalidState {
                reason: "pixel layer capture is off; call set_pixel_layers(true) first".into(),
            });
        }
        let frame = &self.hooks().frame;
        if frame.tag.is_none() || frame.layers.len() != (frame.width * frame.height) as usize {
            return Err(CoreError::InvalidState {
                reason: "the last complete frame has no layer map: it was already being drawn, or \
                         done, when capture was turned on or a state was loaded, or it was not \
                         rendered normally (LCD off, or the blank frame after the LCD is turned \
                         on); run another frame"
                    .into(),
            });
        }
        Ok(LayerMap {
            width: frame.width,
            height: frame.height,
            layers: &frame.layers,
        })
    }

    fn set_pixel_layers(&mut self, on: bool) -> Result<(), CoreError> {
        if on && let Some(reason) = self.layers_refusal() {
            return Err(CoreError::Unsupported {
                feature: Feature::PixelLayers,
                reason,
            });
        }
        if on == self.hooks().layers_on {
            return Ok(());
        }
        let gb = self.raw();
        let fresh = self.frame_untouched();
        let hooks = self.hooks_mut();
        if on {
            hooks.layers_on = true;
            hooks.frame.layers.clear();
            hooks.restart_capture(fresh);
            // SAFETY: the buffer lives in `hooks`, which outlives the instance.
            unsafe { sb::gbcli_set_layer_buffer(gb, hooks.layers.as_mut_ptr()) };
        } else {
            hooks.layers_on = false;
            hooks.layers_skip_next = false;
            hooks.frame.layers.clear();
            // SAFETY: detaches the buffer before it could be written again.
            unsafe { sb::gbcli_set_layer_buffer(gb, std::ptr::null_mut()) };
        }
        Ok(())
    }

    fn apu(&self) -> Result<ApuState, CoreError> {
        Ok(memory::apu(self))
    }

    fn set_apu_write_log(&mut self, on: bool) -> Result<(), CoreError> {
        self.set_apu_log(on);
        Ok(())
    }

    fn take_apu_writes(&mut self) -> Result<Vec<ApuWrite>, CoreError> {
        Ok(self
            .hooks_mut()
            .apu_log
            .as_mut()
            .map(std::mem::take)
            .unwrap_or_default())
    }

    fn set_audio_capture(&mut self, capture: Option<AudioCapture>) -> Result<(), CoreError> {
        self.set_capture(capture)
    }

    fn take_audio(&mut self) -> Result<AudioSamples, CoreError> {
        Ok(self
            .hooks_mut()
            .audio
            .as_mut()
            .map(std::mem::take)
            .unwrap_or_default())
    }

    fn written(&self, region: Region) -> Result<WrittenView<'_>, CoreError> {
        let tracking = self
            .hooks()
            .tracking
            .as_ref()
            .ok_or_else(|| CoreError::InvalidState {
                reason: "this core was built with write tracking off".into(),
            })?;
        match region {
            Region::Vram | Region::Oam => Err(unsupported_err(Feature::WriteHookCoversDma)),
            // The hook can't place this mapper's cart RAM writes, so it can't
            // tell an unwritten byte from a written one: untracked, not unwritten.
            Region::Sram if self.supports(Feature::CartRamWrites) != Support::Yes => {
                Err(CoreError::Unsupported {
                    feature: Feature::CartRamWrites,
                    reason: support::CART_RAM_REASON,
                })
            }
            _ => tracking.view(region).ok_or(CoreError::InvalidState {
                reason: format!("{region:?} isn't tracked: only WRAM, HRAM and cart RAM are"),
            }),
        }
    }

    fn bus_write(&mut self, addr: u16, value: u8) -> Result<(), CoreError> {
        let (ticks, pc) = (self.ticks, self.registers().pc);
        let hooks = self.hooks_mut();
        hooks.step_ticks = ticks;
        hooks.step_pc = pc;
        // SAFETY: a bus write on a live instance outside GB_run.
        unsafe { sb::GB_write_memory(self.raw(), addr, value) };
        Ok(())
    }

    fn bus_read(&mut self, addr: u16) -> Result<u8, CoreError> {
        // SAFETY: a bus read on a live instance outside GB_run.
        Ok(unsafe { sb::GB_read_memory(self.raw(), addr) })
    }

    fn poke(&mut self, region: Region, offset: u32, bytes: &[u8]) -> Result<(), CoreError> {
        memory::poke(self, region, offset, bytes)
    }

    fn patch_rom(&mut self, offset: u32, bytes: &[u8]) -> Result<(), CoreError> {
        memory::patch_rom(self, offset, bytes)
    }

    fn set_registers(&mut self, regs: &Registers) {
        let gb = self.raw();
        // SAFETY: writes to a live instance outside GB_run.
        unsafe {
            *sb::GB_get_registers(gb) = sb::GB_registers_t {
                af: u16::from_le_bytes([regs.f, regs.a]),
                bc: u16::from_le_bytes([regs.c, regs.b]),
                de: u16::from_le_bytes([regs.e, regs.d]),
                hl: u16::from_le_bytes([regs.l, regs.h]),
                sp: regs.sp,
                pc: regs.pc,
            };
            sb::gbcli_set_ime(gb, regs.ime);
            sb::gbcli_set_halted(gb, regs.halted);
        }
    }

    fn set_keys(&mut self, keys: Keys) {
        self.keys = keys;
        // SAFETY: a setter on a live instance outside GB_run. Keys bits match
        // GB_key_mask_t's order.
        unsafe { sb::GB_set_key_mask(self.raw(), sb::GB_key_mask_t::from(keys.0)) };
    }

    fn save(&self) -> Vec<u8> {
        let gb = self.raw();
        // SAFETY: SameBoy fills exactly GB_get_save_state_size bytes; saving reads only.
        let sameboy = unsafe {
            let mut state = vec![0u8; sb::GB_get_save_state_size(gb)];
            sb::GB_save_state_to_buffer(gb, state.as_mut_ptr());
            state
        };
        // SAFETY: a plain getter on the live instance.
        let model = unsafe { sb::GB_get_model(gb) };
        blob::encode(&blob::Blob {
            model,
            ticks: self.ticks,
            vblanks: self.vblanks,
            written: self
                .hooks()
                .tracking
                .as_ref()
                .map_or_else(Vec::new, |t| t.words()),
            frame: self.hooks().frame.without_layers(),
            screen: self.hooks().screen[..self.screen_pixels()].to_vec(),
            sameboy,
        })
    }

    fn load(&mut self, bytes: &[u8]) -> Result<(), CoreError> {
        let blob = blob::decode(bytes)?;
        if blob.screen.len() != self.screen_pixels() {
            return Err(CoreError::InvalidState {
                reason: "the state's framebuffer has the wrong size".into(),
            });
        }
        if let Some(tracking) = &self.hooks().tracking
            && tracking.words().len() != blob.written.len()
        {
            return Err(CoreError::InvalidState {
                reason: "the state's write-tracking bitmaps don't fit this core (built with \
                         tracking off, or for other RAM sizes)"
                    .into(),
            });
        }
        let gb = self.raw();
        // SAFETY: plain query on a live instance.
        let model = unsafe { sb::GB_get_model(gb) };
        if blob.model != model {
            // SameBoy would silently switch the model within a family.
            return Err(CoreError::InvalidState {
                reason: format!(
                    "the state is from model {:#x}, this core is {model:#x}",
                    blob.model
                ),
            });
        }
        // SAFETY: SameBoy validates the buffer and leaves the instance unchanged on failure.
        let status =
            unsafe { sb::GB_load_state_from_buffer(gb, blob.sameboy.as_ptr(), blob.sameboy.len()) };
        if status != 0 {
            return Err(CoreError::InvalidState {
                reason: format!("SameBoy refused the state (error {status})"),
            });
        }
        self.ticks = blob.ticks;
        self.vblanks = blob.vblanks;
        let fresh = self.frame_untouched();
        let hooks = self.hooks_mut();
        // The blob carries no layer map, so the loaded frame has none.
        hooks.frame = blob.frame;
        if hooks.layers_on {
            // The buffer holds the old timeline's tags for the rows the loaded
            // frame has already drawn.
            hooks.restart_capture(fresh);
        }
        // In place: SameBoy holds a pointer to this buffer.
        hooks.screen[..blob.screen.len()].copy_from_slice(&blob.screen);
        if let Some(tracking) = &mut hooks.tracking {
            tracking.restore(&blob.written);
        }
        self.audio_after_load();
        Ok(())
    }
}
