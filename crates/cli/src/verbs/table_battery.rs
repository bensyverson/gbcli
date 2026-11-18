//! Battery saves: `power-cycle` (an act verb) and `sav` (a read verb).

use super::flags::{CHANGE, OUTPUT, SESSION, value};
use super::{Arity, Class, FlagSpec, Verb, VerbSpec};

const POWER_CYCLE: &[FlagSpec] = &[
    value(
        "sav",
        "PATH",
        "carry this .sav in instead of the cart's own battery (SameBoy's format: cart RAM, \
         then the clock footer on a clocked cart)",
    ),
    value(
        "rtc-advance",
        "DURATION",
        "move the cart's clock on while the power is off: 45s, 30m, 12h or 7d, up to the \
         point where the clock's readings repeat: 1024d on MBC3, 3584d on TPP1, 65536d on \
         HuC3",
    ),
];

pub const POWER_CYCLE_VERB: VerbSpec = VerbSpec {
    verb: Verb::PowerCycle,
    name: "power-cycle",
    class: Class::Act,
    summary: "turn the machine off and on, keeping the cart's battery RAM and clock; the boot ROM runs first",
    usage: "[--sav PATH] [--rtc-advance DURATION] [flags]",
    arity: Arity {
        min: 0,
        max: Some(0),
        name: "",
    },
    flags: POWER_CYCLE,
    groups: &[SESSION, CHANGE, OUTPUT],
    about: "Saves a new head at power-on (in the boot ROM, time 0) built afresh from the \
            session's ROM, patches and seed, carrying in the cart's battery-backed RAM and \
            clock; everything else starts over, and a cart without a battery comes back \
            with blank RAM, as on hardware. Prints where the battery came from (head, sav \
            or none), its size, and the clock: restored (the save's clock, with no host \
            time added), footer_missing (a RAM-only .sav: the clock is flagged for the game \
            to set) or not_clocked. A head saved over an earlier build of the ROM is not \
            refused: only its battery crosses into the ROM on disk (rom_rebuilt), so this \
            is how a save survives a rebuild. Nothing reads a .sav unless --sav names one. \
            The machine is at power-on and the boot ROM runs first, so run to the program's \
            main loop (`run --to <label>`) before reading its state.",
    example: (
        "gbcli power-cycle --session dx --rtc-advance 8h",
        "restarts the game with its save, the clock eight hours on; `gbcli run --frames \
         300` then shows whether the save loads",
    ),
};

pub const SAV_VERB: VerbSpec = VerbSpec {
    verb: Verb::Sav,
    name: "sav",
    class: Class::Read,
    summary: "write the head's battery save as a .sav file",
    usage: "PATH [flags]",
    arity: Arity {
        min: 1,
        max: Some(1),
        name: "PATH",
    },
    flags: &[],
    groups: &[SESSION, OUTPUT],
    about: "Writes the cart's battery-backed RAM, and on a clocked cart (MBC3) the 48-byte \
            clock footer BGB and VBA use, in SameBoy's .sav format, so a user-facing \
            emulator or a flash cart opens the game with its save. The footer's stamp is \
            the host time now. A cart without a battery has nothing to save (exit 2).",
    example: (
        "gbcli sav game.sav --session dx",
        "writes game.sav beside the ROM; `gbcli power-cycle --sav game.sav` reads it back",
    ),
};
