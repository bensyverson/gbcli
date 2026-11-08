//! Named profiles, written by hand from FlashGBX's cart database
//! (github.com/lesserkuma/FlashGBX, `FlashGBX/config/fc_DMG_*.txt` at commit
//! 5b2398bb36a0c2de0acbea9982fe65869317b02d). FlashGBX is GPL-3.0, so no
//! file of it is vendored: each profile restates the facts of one file
//! (unlock cycles, write pin, sector map, IDs), and
//! `scripts/flash/flashgbx-summary.py` re-reads them from a checkout. The
//! busy times are [`BusyTicks::TYPICAL`]; FlashGBX records none.

use crate::profile::{BusyTicks, DataLines, FlashProfile, SectorRun, WritePin, is_cycle};

const COMMIT: &str = "5b2398b";

/// The chip size of the generic boards, whose FlashGBX files give none
/// (assumed): 2 MiB, the 16-Mbit 29LV160 class most of FlashGBX's
/// swapped-data DMG boards carry (26LV160, AM29LV160, HY29LV160, MX29LV160,
/// ES29LV160).
const GENERIC_SIZE: u32 = 0x20_0000;

/// The command decode of an AMD-style chip in byte mode, the cart's A0 on
/// the chip's A-1 (the "AAA" boards): A10-A0 and A-1 are compared, byte
/// bits 11-0, and A11 and up are don't-care. S29AL016D (successor of
/// Am29LV160D), publication S29AL016D_00 rev A amendment 7, Table 10.1 note
/// 5: "Address bits A19–A11 are don't cares for unlock and command cycles,
/// unless SA or PA required"; MX29LV160T/B PM0866 rev 3.7, Table 5 note 3:
/// "AAAH or 555H to Address A10~A-1 in byte mode. Address bit A11~A19=X=
/// Don't care"; Am29DL16xD publication 21533 rev E, Table 14 note 5. The
/// MX29LV320E datasheet (PM1575 rev 1.3) states no don't-care rule: its mask
/// is the family's, inferred.
const BYTE_MODE: u32 = 0xFFF;
/// The same decode on a "555" board, whose $0555/$02AA unlock addresses put
/// the cart's A0 on the chip's A0: cart bits 10-0 (inferred from the
/// addresses; FlashGBX doesn't describe the wiring).
const WORD_ADDRESSED: u32 = 0x7FF;
/// SST39SF010A/020A/040 (DS20005022F rev F), Table 4-2 note 1: "Address
/// format A14-A0 (Hex), Addresses AMS-A15 can be VIL or VIH, but no other
/// value, for the command sequence".
const SST39: u32 = 0x7FFF;

/// One row of the table below.
struct Row {
    name: &'static str,
    file: &'static str,
    unlock: [u16; 2],
    /// Which chip-address bits its chip decodes in command cycles.
    command_mask: u32,
    data_lines: DataLines,
    write_pin: WritePin,
    /// FlashGBX's `flash_size`, or [`GENERIC_SIZE`] where it gives none.
    size: u32,
    sectors: &'static [SectorRun],
    ids: Option<&'static [u8]>,
}

const fn run(size: u32, count: u32) -> SectorRun {
    SectorRun { size, count }
}

use DataLines::{Straight, SwappedD0D1};
use WritePin::{Audio, Wr};

const ROWS: &[Row] = &[
    // GB Studio's batteryless saves write this board's sequence
    // (gbvm flasher_s.s: $0AAA<-$A9, $0555<-$56).
    Row {
        name: "generic-wr-aaa-a9",
        file: "fc_DMG_Generic_WR_AAA_A9.txt",
        unlock: [0x0AAA, 0x0555],
        command_mask: BYTE_MODE,
        data_lines: SwappedD0D1,
        write_pin: Wr,
        size: GENERIC_SIZE,
        sectors: &[],
        ids: None,
    },
    Row {
        name: "generic-wr-aaa-aa",
        file: "fc_DMG_Generic_WR_AAA_AA.txt",
        unlock: [0x0AAA, 0x0555],
        command_mask: BYTE_MODE,
        data_lines: Straight,
        write_pin: Wr,
        size: GENERIC_SIZE,
        sectors: &[],
        ids: None,
    },
    Row {
        name: "generic-wr-555-a9",
        file: "fc_DMG_Generic_WR_555_A9.txt",
        unlock: [0x0555, 0x02AA],
        command_mask: WORD_ADDRESSED,
        data_lines: SwappedD0D1,
        write_pin: Wr,
        size: GENERIC_SIZE,
        sectors: &[],
        ids: None,
    },
    Row {
        name: "generic-wr-555-aa",
        file: "fc_DMG_Generic_WR_555_AA.txt",
        unlock: [0x0555, 0x02AA],
        command_mask: WORD_ADDRESSED,
        data_lines: Straight,
        write_pin: Wr,
        size: GENERIC_SIZE,
        sectors: &[],
        ids: None,
    },
    Row {
        name: "generic-audio-aaa-a9",
        file: "fc_DMG_Generic_AUDIO_AAA_A9.txt",
        unlock: [0x0AAA, 0x0555],
        command_mask: BYTE_MODE,
        data_lines: SwappedD0D1,
        write_pin: Audio,
        size: GENERIC_SIZE,
        sectors: &[],
        ids: None,
    },
    Row {
        name: "generic-audio-aaa-aa",
        file: "fc_DMG_Generic_AUDIO_AAA_AA.txt",
        unlock: [0x0AAA, 0x0555],
        command_mask: BYTE_MODE,
        data_lines: Straight,
        write_pin: Audio,
        size: GENERIC_SIZE,
        sectors: &[],
        ids: None,
    },
    // A bottom-boot map: eight 8 KiB sectors, then 64 KiB ones.
    Row {
        name: "29dl163bd-90",
        file: "fc_DMG_29DL163BD-90.txt",
        unlock: [0x0AAA, 0x0555],
        command_mask: BYTE_MODE,
        data_lines: SwappedD0D1,
        write_pin: Wr,
        size: 0x20_0000,
        sectors: &[run(0x2000, 8), run(0x10000, 31)],
        ids: Some(&[0x04, 0x04, 0x2B, 0x2B]),
    },
    // Commands in bank 1's window.
    Row {
        name: "am29lv160db",
        file: "fc_DMG_AM29LV160DB.txt",
        unlock: [0x4AAA, 0x4555],
        command_mask: BYTE_MODE,
        data_lines: SwappedD0D1,
        write_pin: Wr,
        size: 0x20_0000,
        sectors: &[],
        ids: Some(&[0x02, 0x02, 0x4A, 0x4A]),
    },
    // A top-boot map: 64 KiB sectors, then eight 8 KiB ones.
    Row {
        name: "mx29lv320et",
        file: "fc_DMG_MX29LV320ET.txt",
        unlock: [0x0AAA, 0x0555],
        command_mask: BYTE_MODE,
        data_lines: Straight,
        write_pin: Wr,
        size: 0x40_0000,
        sectors: &[run(0x10000, 63), run(0x2000, 8)],
        ids: Some(&[0xC2, 0xC2, 0xA7, 0xA7]),
    },
    // JEDEC $5555/$2AAA unlock, /WE on AUDIO. FlashGBX issues this chip's
    // commands with bank 1 mapped (`flash_commands_on_bank_1`); with the
    // mask below, $5555 is bank 1's $1555. Its 4 KiB sectors aren't listed
    // in the file, so the map is the default.
    Row {
        name: "sst39sf040-audio",
        file: "fc_DMG_SST39SF040_AUDIO.txt",
        unlock: [0x5555, 0x2AAA],
        command_mask: SST39,
        data_lines: Straight,
        write_pin: Audio,
        size: 0x8_0000,
        sectors: &[],
        ids: Some(&[0xBF, 0xB7]),
    },
];

impl FlashProfile {
    /// Every named profile's name, in the order [`FlashProfile::named`]
    /// knows them.
    pub fn names() -> impl Iterator<Item = &'static str> {
        ROWS.iter().map(|row| row.name)
    }

    /// The first named board, /WE on WR, for which a CPU write at
    /// `chip_addr` (the bank's base plus the offset) of `value` is the first
    /// unlock cycle, by the test the chip itself uses: what a program that
    /// sends it was written for. `None` for an ordinary mapper write.
    pub fn unlocked_by(chip_addr: u32, value: u8) -> Option<&'static str> {
        ROWS.iter()
            .find(|row| {
                row.write_pin == Wr
                    && row.data_lines.cross(value) == 0xAA
                    && is_cycle(row.unlock[0], row.command_mask, chip_addr)
            })
            .map(|row| row.name)
    }

    /// A named profile, or `None` for an unknown name.
    pub fn named(name: &str) -> Option<FlashProfile> {
        ROWS.iter()
            .find(|row| row.name == name)
            .map(|row| FlashProfile {
                name: row.name.into(),
                source: format!("FlashGBX {COMMIT} config/{}", row.file),
                unlock: row.unlock,
                command_mask: row.command_mask,
                data_lines: row.data_lines,
                write_pin: row.write_pin,
                size: row.size,
                sectors: row.sectors.to_vec(),
                ids: row.ids.map(<[u8]>::to_vec),
                busy: BusyTicks::TYPICAL,
            })
    }
}
