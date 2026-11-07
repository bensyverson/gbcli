//! SameBoy's side of the shared types: models, boot ROMs, and the capability table.

use gbcli_core::{CoreError, Feature, Model, Support};
use sameboy_sys as sb;

pub(crate) fn sameboy_model(model: Model) -> sb::GB_model_t {
    match model {
        Model::DmgB => sb::GB_MODEL_DMG_B,
        Model::Mgb => sb::GB_MODEL_MGB,
        Model::Sgb => sb::GB_MODEL_SGB,
        Model::Sgb2 => sb::GB_MODEL_SGB2,
        Model::Cgb0 => sb::GB_MODEL_CGB_0,
        Model::CgbA => sb::GB_MODEL_CGB_A,
        Model::CgbB => sb::GB_MODEL_CGB_B,
        Model::CgbC => sb::GB_MODEL_CGB_C,
        Model::CgbD => sb::GB_MODEL_CGB_D,
        Model::CgbE => sb::GB_MODEL_CGB_E,
        Model::Agb => sb::GB_MODEL_AGB_A,
    }
}

/// The open-source boot ROM SameBoy runs for a model when none is supplied.
pub fn builtin_boot_rom(model: Model) -> &'static [u8] {
    use sb::boot_roms;
    match model {
        Model::DmgB => boot_roms::DMG,
        Model::Mgb => boot_roms::MGB,
        Model::Sgb => boot_roms::SGB,
        Model::Sgb2 => boot_roms::SGB2,
        Model::Cgb0 => boot_roms::CGB0,
        Model::CgbA | Model::CgbB | Model::CgbC | Model::CgbD | Model::CgbE => boot_roms::CGB,
        Model::Agb => boot_roms::AGB,
    }
}

/// The capability table.
pub(crate) fn support(feature: Feature) -> Support {
    let reason = match feature {
        Feature::ApuChannels => return Support::Yes,
        Feature::ApuWriteLog => return Support::Yes,
        Feature::AudioSamples => return Support::Yes,
        Feature::PixelLayers => return Support::Yes,
        Feature::CartRamWrites => return Support::Yes,
        Feature::LiveFrame => return Support::Yes,
        // The color mode always; CGB colors only where frames are RGB.
        Feature::DisplayColors => return Support::Yes,
        Feature::DotStep => "SameBoy's finest public step is one CPU step (GB_run)",
        Feature::MCycleStep => "SameBoy's finest public step is one CPU step (GB_run)",
        Feature::PpuDot => {
            "SameBoy keeps no single dot counter: cycles_for_line resets inside mode 0 and \
             stops on lines 144-153 (design §2.2)"
        }
        Feature::SideEffectFreeIo => {
            "NR52, PCM12/PCM34 and wave RAM during playback need the APU synced; PPU \
             registers are unavailable while mode-3 cycles are batched"
        }
        Feature::ExtraOam => "FEA0-FEFF reads are model-specific bus behavior, not storage",
        Feature::ApuInternals => "only SameBoy's per-channel getters are public",
        Feature::WriteHookCoversDma => {
            "SameBoy's write callback misses OAM DMA and HDMA destination writes"
        }
        Feature::ExecHookCoversInterrupts => {
            "SameBoy's execution callback doesn't fire for interrupt dispatch"
        }
        Feature::Mmm01RomPatch => "MMM01 rotates its ROM in memory, so file offsets don't map",
        Feature::EventSinks => return Support::Yes,
        Feature::RasterEvents => return Support::Yes,
        Feature::PpuModeEvents => {
            "SameBoy has no PPU mode callback, and STAT's mode bits lag while mode-3 cycles \
             are batched, so a transition can't be timed"
        }
    };
    Support::No { reason }
}

pub(crate) fn unsupported_err(feature: Feature) -> CoreError {
    match support(feature) {
        Support::No { reason } => CoreError::Unsupported { feature, reason },
        Support::Yes => unreachable!("only unsupported features become errors"),
    }
}

/// Why store filters can't cover cart RAM on some mappers.
pub(crate) const CART_RAM_REASON: &str = "this mapper's cart RAM path (MBC2, MBC7, HuC1/3, TPP1, \
     camera) has modes and side channels the write hook doesn't mirror";

/// Why layer capture is refused on SGB models: their frame is composited in
/// `sgb.c` (border, mask modes, frozen masks) after the PPU's mixing point, so
/// a tag from `display.c` would either miss most of the frame or describe a
/// pixel the mask hid.
pub(crate) const SGB_LAYERS_REASON: &str =
    "the SGB path composites its border and mask modes outside the PPU's mixing point";

/// Why layer capture is refused with rendering off: SameBoy then skips the
/// mixing point (`display.c` drops every pixel before it), so no tag is
/// written and no frame is published.
pub(crate) const RENDERING_OFF_LAYERS_REASON: &str = "rendering is off, so the PPU never mixes a pixel; build the core with Rendering::On \
     to capture layers";
