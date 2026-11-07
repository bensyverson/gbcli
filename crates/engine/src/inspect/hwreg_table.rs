//! The hardware registers, named as hardware.inc 4.6 names them (CC0; the
//! copy vendored at `crates/sameboy-sys/SameBoy/BootROMs/hardware.inc`), with
//! their bit fields per Pan Docs.

use super::hwreg::{Field, HwReg, Models, Source};

const fn f(name: &'static str, lsb: u8, width: u8) -> Field {
    Field { name, lsb, width }
}

const fn reg(name: &'static str, addr: u16, source: Source, fields: &'static [Field]) -> HwReg {
    HwReg {
        name,
        aliases: &[],
        addr,
        source,
        models: Models::All,
        fields,
    }
}

impl HwReg {
    const fn aka(self, aliases: &'static [&'static str]) -> HwReg {
        HwReg { aliases, ..self }
    }
    const fn cgb(self) -> HwReg {
        HwReg {
            models: Models::Cgb,
            ..self
        }
    }
}

use Source::{Derived, Stored, WriteOnly};

const INTERRUPTS: &[Field] = &[
    f("joypad", 4, 1),
    f("serial", 3, 1),
    f("timer", 2, 1),
    f("stat", 1, 1),
    f("vblank", 0, 1),
];
const LENGTH_DUTY: &[Field] = &[f("duty", 6, 2), f("length", 0, 6)];
const ENVELOPE: &[Field] = &[f("volume", 4, 4), f("direction", 3, 1), f("pace", 0, 3)];
const PERIOD_HIGH: &[Field] = &[
    f("trigger", 7, 1),
    f("length_enable", 6, 1),
    f("period_high", 0, 3),
];
const DMG_PALETTE: &[Field] = &[
    f("color3", 6, 2),
    f("color2", 4, 2),
    f("color1", 2, 2),
    f("color0", 0, 2),
];
const PALETTE_SPEC: &[Field] = &[f("auto_increment", 7, 1), f("address", 0, 6)];

const WAVE_SAMPLES: &[Field] = &[f("first", 4, 4), f("second", 0, 4)];

/// Wave RAM bytes: the CPU reads storage unless channel 3 is playing.
const fn wave(name: &'static str, n: u16) -> HwReg {
    reg(name, 0xFF30 + n, Derived, WAVE_SAMPLES)
}

#[rustfmt::skip]
pub(super) const HW_REGISTERS: &[HwReg] = &[
    // rJOYP and rWBK (below) are Pan Docs' names, not hardware.inc 4.6's.
    reg("rP1", 0xFF00, Derived, &[f("select_buttons", 5, 1), f("select_dpad", 4, 1), f("inputs", 0, 4)]).aka(&["rJOYP"]),
    reg("rSB", 0xFF01, Stored, &[]),
    reg("rSC", 0xFF02, Stored, &[f("transfer", 7, 1), f("speed", 1, 1), f("clock", 0, 1)]),
    reg("rDIV", 0xFF04, Derived, &[]),
    reg("rTIMA", 0xFF05, Derived, &[]),
    reg("rTMA", 0xFF06, Stored, &[]),
    reg("rTAC", 0xFF07, Stored, &[f("enable", 2, 1), f("clock", 0, 2)]),
    reg("rIF", 0xFF0F, Stored, INTERRUPTS),
    reg("rNR10", 0xFF10, Stored, &[f("pace", 4, 3), f("direction", 3, 1), f("step", 0, 3)]).aka(&["rAUD1SWEEP"]),
    reg("rNR11", 0xFF11, Stored, LENGTH_DUTY).aka(&["rAUD1LEN"]),
    reg("rNR12", 0xFF12, Stored, ENVELOPE).aka(&["rAUD1ENV"]),
    reg("rNR13", 0xFF13, WriteOnly, &[]).aka(&["rAUD1LOW"]),
    reg("rNR14", 0xFF14, Stored, PERIOD_HIGH).aka(&["rAUD1HIGH"]),
    reg("rNR21", 0xFF16, Stored, LENGTH_DUTY).aka(&["rAUD2LEN"]),
    reg("rNR22", 0xFF17, Stored, ENVELOPE).aka(&["rAUD2ENV"]),
    reg("rNR23", 0xFF18, WriteOnly, &[]).aka(&["rAUD2LOW"]),
    reg("rNR24", 0xFF19, Stored, PERIOD_HIGH).aka(&["rAUD2HIGH"]),
    reg("rNR30", 0xFF1A, Stored, &[f("dac", 7, 1)]).aka(&["rAUD3ENA"]),
    reg("rNR31", 0xFF1B, WriteOnly, &[]).aka(&["rAUD3LEN"]),
    reg("rNR32", 0xFF1C, Stored, &[f("output_level", 5, 2)]).aka(&["rAUD3LEVEL"]),
    reg("rNR33", 0xFF1D, WriteOnly, &[]).aka(&["rAUD3LOW"]),
    reg("rNR34", 0xFF1E, Stored, PERIOD_HIGH).aka(&["rAUD3HIGH"]),
    reg("rNR41", 0xFF20, WriteOnly, &[]).aka(&["rAUD4LEN"]),
    reg("rNR42", 0xFF21, Stored, ENVELOPE).aka(&["rAUD4ENV"]),
    reg("rNR43", 0xFF22, Stored, &[f("shift", 4, 4), f("width", 3, 1), f("divider", 0, 3)]).aka(&["rAUD4POLY"]),
    reg("rNR44", 0xFF23, Stored, &[f("trigger", 7, 1), f("length_enable", 6, 1)]).aka(&["rAUD4GO"]),
    reg("rNR50", 0xFF24, Stored, &[f("vin_left", 7, 1), f("left_volume", 4, 3), f("vin_right", 3, 1), f("right_volume", 0, 3)]).aka(&["rAUDVOL"]),
    reg("rNR51", 0xFF25, Stored, &[
        f("ch4_left", 7, 1), f("ch3_left", 6, 1), f("ch2_left", 5, 1), f("ch1_left", 4, 1),
        f("ch4_right", 3, 1), f("ch3_right", 2, 1), f("ch2_right", 1, 1), f("ch1_right", 0, 1),
    ]).aka(&["rAUDTERM"]),
    reg("rNR52", 0xFF26, Derived, &[f("on", 7, 1), f("ch4_on", 3, 1), f("ch3_on", 2, 1), f("ch2_on", 1, 1), f("ch1_on", 0, 1)]).aka(&["rAUDENA"]),
    wave("_AUD3WAVERAM", 0x0),
    wave("_AUD3WAVERAM+1", 0x1),
    wave("_AUD3WAVERAM+2", 0x2),
    wave("_AUD3WAVERAM+3", 0x3),
    wave("_AUD3WAVERAM+4", 0x4),
    wave("_AUD3WAVERAM+5", 0x5),
    wave("_AUD3WAVERAM+6", 0x6),
    wave("_AUD3WAVERAM+7", 0x7),
    wave("_AUD3WAVERAM+8", 0x8),
    wave("_AUD3WAVERAM+9", 0x9),
    wave("_AUD3WAVERAM+10", 0xA),
    wave("_AUD3WAVERAM+11", 0xB),
    wave("_AUD3WAVERAM+12", 0xC),
    wave("_AUD3WAVERAM+13", 0xD),
    wave("_AUD3WAVERAM+14", 0xE),
    wave("_AUD3WAVERAM+15", 0xF),
    reg("rLCDC", 0xFF40, Stored, &[
        f("on", 7, 1), f("window_map", 6, 1), f("window", 5, 1), f("tile_data", 4, 1),
        f("bg_map", 3, 1), f("obj_size", 2, 1), f("obj", 1, 1), f("bg", 0, 1),
    ]),
    reg("rSTAT", 0xFF41, Stored, &[
        f("lyc_int", 6, 1), f("mode2_int", 5, 1), f("mode1_int", 4, 1), f("mode0_int", 3, 1),
        f("lyc_match", 2, 1), f("mode", 0, 2),
    ]),
    reg("rSCY", 0xFF42, Stored, &[]),
    reg("rSCX", 0xFF43, Stored, &[]),
    reg("rLY", 0xFF44, Stored, &[]),
    reg("rLYC", 0xFF45, Stored, &[]),
    reg("rDMA", 0xFF46, Stored, &[]),
    reg("rBGP", 0xFF47, Stored, DMG_PALETTE),
    reg("rOBP0", 0xFF48, Stored, DMG_PALETTE),
    reg("rOBP1", 0xFF49, Stored, DMG_PALETTE),
    reg("rWY", 0xFF4A, Stored, &[]),
    reg("rWX", 0xFF4B, Stored, &[]),
    reg("rKEY1", 0xFF4D, Derived, &[f("double_speed", 7, 1), f("armed", 0, 1)]).aka(&["rSPD"]).cgb(),
    reg("rVBK", 0xFF4F, Derived, &[f("bank", 0, 1)]).cgb(),
    // Not in hardware.inc 4.6; SameBoy's name (GB_IO_BANK).
    reg("rBANK", 0xFF50, Derived, &[f("boot_rom_off", 0, 1)]),
    reg("rHDMA1", 0xFF51, WriteOnly, &[]).cgb(),
    reg("rHDMA2", 0xFF52, WriteOnly, &[]).cgb(),
    reg("rHDMA3", 0xFF53, WriteOnly, &[]).cgb(),
    reg("rHDMA4", 0xFF54, WriteOnly, &[]).cgb(),
    reg("rHDMA5", 0xFF55, Derived, &[f("idle", 7, 1), f("blocks_left", 0, 7)]).cgb(),
    reg("rRP", 0xFF56, Derived, &[f("read_enable", 6, 2), f("receiving", 1, 1), f("emitting", 0, 1)]).cgb(),
    reg("rBCPS", 0xFF68, Stored, PALETTE_SPEC).aka(&["rBGPI"]).cgb(),
    reg("rBCPD", 0xFF69, Derived, &[]).aka(&["rBGPD"]).cgb(),
    reg("rOCPS", 0xFF6A, Stored, PALETTE_SPEC).aka(&["rOBPI"]).cgb(),
    reg("rOCPD", 0xFF6B, Derived, &[]).aka(&["rOBPD"]).cgb(),
    reg("rOPRI", 0xFF6C, Stored, &[f("by_coordinate", 0, 1)]).cgb(),
    reg("rSVBK", 0xFF70, Stored, &[f("bank", 0, 3)]).aka(&["rSMBK", "rWBK"]).cgb(),
    reg("rPCM12", 0xFF76, Derived, &[f("ch2", 4, 4), f("ch1", 0, 4)]).cgb(),
    reg("rPCM34", 0xFF77, Derived, &[f("ch4", 4, 4), f("ch3", 0, 4)]).cgb(),
    reg("rIE", 0xFFFF, Stored, INTERRUPTS),
];
