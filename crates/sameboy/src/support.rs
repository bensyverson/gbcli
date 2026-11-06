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

pub(crate) fn builtin_boot_rom(model: Model) -> &'static [u8] {
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

pub(crate) fn unsupported(feature: Feature) -> Support {
    let reason = match feature {
        Feature::DotStep => "SameBoy's finest public step is one CPU step (GB_run)",
        Feature::MCycleStep => "SameBoy's finest public step is one CPU step (GB_run)",
        Feature::PpuDot => {
            "SameBoy keeps no single dot counter: cycles_for_line resets inside mode 0 and \
             stops on lines 144-153 (design §2.2)"
        }
        Feature::PixelLayers => "SameBoy's renderer doesn't record which layer drew a pixel",
        Feature::SideEffectFreeIo => {
            "APU (NRxx, PCM) and some CGB registers decode only through a bus read; PPU \
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
        Feature::EventSinks => "event sinks are not implemented in the SameBoy backend yet",
    };
    Support::No { reason }
}

pub(crate) fn unsupported_err(feature: Feature) -> CoreError {
    match unsupported(feature) {
        Support::No { reason } => CoreError::Unsupported { feature, reason },
        Support::Yes => unreachable!("unsupported() only says no"),
    }
}
