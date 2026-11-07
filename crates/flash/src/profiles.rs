//! Named profiles, written by hand from FlashGBX's cart database
//! (github.com/lesserkuma/FlashGBX, `FlashGBX/config/fc_DMG_*.txt` at commit
//! 5b2398bb36a0c2de0acbea9982fe65869317b02d). FlashGBX is GPL-3.0, so no
//! file of it is vendored: each profile restates the facts of one file
//! (unlock cycles, write pin, sector map, IDs), and
//! `scripts/flash/flashgbx-summary.py` re-reads them from a checkout. The
//! busy times are [`BusyTicks::TYPICAL`]; FlashGBX records none.

use crate::profile::{BusyTicks, DataLines, FlashProfile, SectorRun, WritePin};

const COMMIT: &str = "5b2398b";

/// The chip size of the generic boards, whose FlashGBX files give none
/// (assumed): 2 MiB, the 16-Mbit 29LV160 class most of FlashGBX's
/// swapped-data DMG boards carry (26LV160, AM29LV160, HY29LV160, MX29LV160,
/// ES29LV160).
const GENERIC_SIZE: u32 = 0x20_0000;

/// One row of the table below.
struct Row {
    name: &'static str,
    file: &'static str,
    unlock: [u16; 2],
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

    /// A named profile, or `None` for an unknown name.
    pub fn named(name: &str) -> Option<FlashProfile> {
        ROWS.iter()
            .find(|row| row.name == name)
            .map(|row| FlashProfile {
                name: row.name.into(),
                source: format!("FlashGBX {COMMIT} config/{}", row.file),
                unlock: row.unlock,
                command_mask: 0x7FFF,
                data_lines: row.data_lines,
                write_pin: row.write_pin,
                size: row.size,
                sectors: row.sectors.to_vec(),
                ids: row.ids.map(<[u8]>::to_vec),
                busy: BusyTicks::TYPICAL,
            })
    }
}
