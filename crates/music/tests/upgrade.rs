//! Versions 0-5 upgrade on read the way hUGETracker's song.pas upgrades
//! them. There is no v2 sample, and the samples don't isolate each rule, so
//! these build minimal files from the record layouts in song.pas and
//! hugedatatypes.pas (hUGETracker 3fedced3).

use gbcli_music::{BANK_SIZE, Cell, Instrument, NO_NOTE, ROWS, read_uge};

/// An instrument as versions 1-5 store it (`TInstrumentV1`, plus the noise
/// macro from v4).
#[derive(Clone)]
struct OldInstrument {
    type_code: i32,
    name: &'static str,
    length: i32,
    length_enabled: u8,
    initial_volume: u8,
    volume_direction: i32,
    volume_pace: u8,
    sweep_time: i32,
    sweep_direction: i32,
    sweep_shift: i32,
    duty: u8,
    output_level: i32,
    waveform: i32,
    counter_step: i32,
    noise_macro: [i8; 6],
}

impl OldInstrument {
    fn of_type(type_code: i32) -> OldInstrument {
        OldInstrument {
            type_code,
            name: "",
            length: 0,
            length_enabled: 0,
            initial_volume: 0,
            volume_direction: 0,
            volume_pace: 0,
            sweep_time: 0,
            sweep_direction: 0,
            sweep_shift: 0,
            duty: 0,
            output_level: 0,
            waveform: 0,
            counter_step: 0,
            noise_macro: [0; 6],
        }
    }
}

/// A v1-v5 cell: note, instrument, effect code, parameter.
type OldCell = (i32, i32, i32, u8);

/// A v1-v5 file. v1-v2 hold 15 instruments, v3+ 45; v1-v2 waves are 33
/// bytes; v4+ instruments carry the noise macro; v5+ patterns carry keys;
/// v2+ end with routines.
struct OldFile {
    version: u32,
    instruments: Vec<OldInstrument>,
    waves: [[u8; 32]; 16],
    ticks_per_row: i32,
    patterns: Vec<(i32, Vec<OldCell>)>,
    /// Stored entries, including the unread last one.
    orders: [Vec<i32>; 4],
    routines: Vec<Vec<u8>>,
}

fn short_string(out: &mut Vec<u8>, s: &str) {
    out.push(s.len() as u8);
    out.extend(s.as_bytes());
    out.resize(out.len() + 255 - s.len(), 0);
}

fn i32s(out: &mut Vec<u8>, v: i32) {
    out.extend(v.to_le_bytes());
}

impl OldFile {
    fn new(version: u32) -> OldFile {
        let count = if version < 3 { 15 } else { 45 };
        let mut waves = [[0u8; 32]; 16];
        for (i, w) in waves.iter_mut().enumerate() {
            w.fill(i as u8);
        }
        OldFile {
            version,
            instruments: (0..count)
                .map(|i| OldInstrument::of_type(if version < 3 { 0 } else { i / 15 }))
                .collect(),
            waves,
            ticks_per_row: 6,
            patterns: Vec::new(),
            orders: [vec![0], vec![0], vec![0], vec![0]],
            routines: vec![Vec::new(); 16],
        }
    }

    fn bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend(self.version.to_le_bytes());
        short_string(&mut out, "Old song");
        short_string(&mut out, "Someone");
        short_string(&mut out, "");
        for ins in &self.instruments {
            i32s(&mut out, ins.type_code);
            short_string(&mut out, ins.name);
            i32s(&mut out, ins.length);
            out.push(ins.length_enabled);
            out.push(ins.initial_volume);
            i32s(&mut out, ins.volume_direction);
            out.push(ins.volume_pace);
            i32s(&mut out, ins.sweep_time);
            i32s(&mut out, ins.sweep_direction);
            i32s(&mut out, ins.sweep_shift);
            out.push(ins.duty);
            i32s(&mut out, ins.output_level);
            i32s(&mut out, ins.waveform);
            i32s(&mut out, 7); // ShiftClockFreq, unused
            i32s(&mut out, ins.counter_step);
            i32s(&mut out, 3); // DividingRatio, unused
            if self.version >= 4 {
                out.extend(ins.noise_macro.map(|m| m as u8));
            }
        }
        for wave in &self.waves {
            out.extend(wave);
            if self.version < 3 {
                out.push(0xEE); // the 33rd byte of TWaveV1
            }
        }
        i32s(&mut out, self.ticks_per_row);
        i32s(&mut out, self.patterns.len() as i32);
        for (key, cells) in &self.patterns {
            if self.version >= 5 {
                i32s(&mut out, *key);
            }
            for row in 0..ROWS {
                let (note, instrument, code, param) =
                    cells.get(row).copied().unwrap_or((NO_NOTE, 0, 0, 0));
                i32s(&mut out, note);
                i32s(&mut out, instrument);
                i32s(&mut out, code);
                out.push(param);
            }
        }
        for order in &self.orders {
            i32s(&mut out, order.len() as i32);
            for key in order {
                i32s(&mut out, *key);
            }
        }
        if self.version >= 2 {
            for r in &self.routines {
                i32s(&mut out, r.len() as i32);
                out.extend(r);
            }
        }
        out
    }
}

/// The subpattern hUGETracker's v4 → v6 upgrade gives a noise instrument
/// (song.pas `ConvertNoiseMacro`).
fn macro_subpattern(noise_macro: [i8; 6], ticks_per_row: i32) -> [Cell; ROWS] {
    let mut rows = [Cell::BLANK; ROWS];
    for (j, m) in noise_macro.iter().enumerate() {
        rows[j + 1].note = i32::from(*m) + 36;
    }
    let wrap = ticks_per_row.min(7);
    rows[(wrap - 1) as usize].volume = wrap;
    rows
}

#[test]
fn a_v2_file_moves_each_instrument_to_its_banks_slot_and_defaults_the_rest() {
    let mut file = OldFile::new(2);
    file.instruments[0] = OldInstrument {
        name: "Lead",
        length: 20,
        length_enabled: 1,
        initial_volume: 12,
        volume_direction: 1,
        volume_pace: 3,
        duty: 1,
        ..OldInstrument::of_type(0)
    };
    file.instruments[1] = OldInstrument {
        name: "Bass",
        output_level: 2,
        waveform: 5,
        ..OldInstrument::of_type(1)
    };
    file.instruments[2] = OldInstrument {
        name: "Hat",
        initial_volume: 9,
        counter_step: 1,
        ..OldInstrument::of_type(2)
    };
    file.routines[3] = b"ld a, 1\r\n".to_vec();
    file.patterns = vec![(0, vec![(24, 1, 0xC, 0x08)])];
    file.orders = [vec![0, 0], vec![0, 0], vec![0, 0], vec![0, 9]];
    let read = read_uge(&file.bytes()).unwrap();
    assert_eq!(read.version, 2);
    let song = read.song;
    assert_eq!(song.name.bytes(), b"Old song");
    assert_eq!(song.artist.bytes(), b"Someone");

    let square = &song.instruments[0];
    assert_eq!(square.name.bytes(), b"Lead");
    assert_eq!(
        (square.length, square.length_enabled, square.initial_volume),
        (20, 1, 12)
    );
    assert_eq!((square.volume_pace, square.duty), (3, 1));
    // Instrument 2 was a wave: it lands in wave slot 2, not square slot 2.
    let wave = &song.instruments[BANK_SIZE + 1];
    assert_eq!(wave.name.bytes(), b"Bass");
    assert_eq!((wave.output_level, wave.waveform), (2, 5));
    assert_eq!(song.instruments[1], Instrument::default_for(1));
    let noise = &song.instruments[2 * BANK_SIZE + 2];
    assert_eq!(noise.name.bytes(), b"Hat");
    assert_eq!((noise.initial_volume, noise.counter_step), (9, 1));
    // Every noise instrument gets the converted (empty) macro, disabled.
    assert_eq!(noise.subpattern, macro_subpattern([0; 6], 6));
    assert_eq!(noise.subpattern_enabled, 0);
    // An untouched slot keeps the defaults hUGETracker's upgrade sets.
    let wave1 = &song.instruments[BANK_SIZE];
    assert_eq!(
        (wave1.type_code, wave1.output_level, wave1.waveform),
        (1, 1, 0)
    );

    // Waves keep their first 32 of 33 bytes.
    assert_eq!(song.waves[3], [3; 32]);
    assert_eq!((song.ticks_per_row, song.timer_enabled), (6, 0));
    assert_eq!(song.routines[3], b"ld a, 1\r\n");
    assert_eq!(song.patterns.len(), 1);
    let cell = song.patterns[0].rows[0];
    // Channel 4 plays pattern 0, and the v1/v2 upgrade runs through the v3
    // one, so its C-5 is renoted for the noise channel (59; see the v3 test).
    assert_eq!(
        (
            cell.note,
            cell.instrument,
            cell.volume,
            cell.effect_code,
            cell.effect_param
        ),
        (59, 1, 0, 0xC, 0x08)
    );
    // The unread last order entry is dropped.
    assert_eq!(song.orders[3], [0]);
}

#[test]
fn version_0_reads_as_version_1() {
    let file = OldFile::new(0);
    let read = read_uge(&file.bytes()).unwrap();
    assert_eq!(read.version, 0);
    assert_eq!(read.song.name.bytes(), b"Old song");
    let v1 = read_uge(&OldFile::new(1).bytes()).unwrap();
    assert!(read.song == v1.song);
}

#[test]
fn a_v3_noise_pattern_is_renoted_for_the_new_noise_instruments() {
    let mut file = OldFile::new(3);
    file.patterns = vec![
        // Used by channel 4: notes with an instrument move to the note
        // whose noise clock matches the old period (C-5: period 1546,
        // shift 15 - (1546 >> 7) = 3, clock 2^16, note 59).
        (
            0,
            vec![
                (24, 1, 0, 0),
                (24, 0, 0, 0),
                (NO_NOTE, 1, 0, 0),
                (0, 2, 0, 0),
            ],
        ),
        // Used by channel 1 only: unchanged.
        (1, vec![(24, 1, 0, 0)]),
        // Named only by the unread last entry: unchanged.
        (2, vec![(24, 1, 0, 0)]),
    ];
    file.orders = [vec![1, 0], vec![1, 0], vec![1, 0], vec![0, 2]];
    let song = read_uge(&file.bytes()).unwrap().song;
    let notes = |key: usize| -> Vec<i32> {
        song.patterns[key].rows[..4]
            .iter()
            .map(|c| c.note)
            .collect()
    };
    // C-3 (period 44, shift 15, clock 16) becomes note 11.
    assert_eq!(notes(0), [59, 24, NO_NOTE, 11]);
    assert_eq!(notes(1)[0], 24);
    assert_eq!(notes(2)[0], 24);
}

#[test]
fn a_v4_noise_macro_becomes_the_noise_instruments_subpattern() {
    let mut file = OldFile::new(4);
    file.ticks_per_row = 4;
    file.instruments[30].noise_macro = [1, -2, 0, 0, 0, 3];
    file.instruments[30].name = "Snare";
    let song = read_uge(&file.bytes()).unwrap().song;
    let snare = &song.instruments[30];
    assert_eq!(snare.subpattern_enabled, 1);
    assert_eq!(snare.subpattern, macro_subpattern([1, -2, 0, 0, 0, 3], 4));
    let quiet = &song.instruments[31];
    assert_eq!(quiet.subpattern_enabled, 0);
    assert_eq!(quiet.subpattern, macro_subpattern([0; 6], 4));
    // A square instrument's subpattern stays blank.
    assert_eq!(song.instruments[0].subpattern, [Cell::BLANK; ROWS]);
}

#[test]
fn a_v5_file_keeps_its_pattern_keys_and_their_storage_order() {
    let mut file = OldFile::new(5);
    file.patterns = vec![(7, vec![(0, 1, 0, 0)]), (3, vec![(12, 1, 0, 0)])];
    file.orders = [vec![7, 3, 0], vec![3, 0], vec![3, 0], vec![3, 0]];
    let song = read_uge(&file.bytes()).unwrap().song;
    let keys: Vec<i32> = song.patterns.iter().map(|p| p.key).collect();
    assert_eq!(keys, [7, 3]);
    assert_eq!(song.orders[0], [7, 3]);
    assert_eq!(song.pattern(3).unwrap().rows[0].note, 12);
}

#[test]
fn a_v5_file_with_repeated_keys_numbers_them_by_position_as_gb_studio_does() {
    // GB Studio 3.0.2 and earlier saved v5 files with every key 0.
    let mut file = OldFile::new(5);
    file.patterns = vec![(0, vec![(0, 1, 0, 0)]), (0, vec![(12, 1, 0, 0)])];
    let song = read_uge(&file.bytes()).unwrap().song;
    let keys: Vec<i32> = song.patterns.iter().map(|p| p.key).collect();
    assert_eq!(keys, [0, 1]);
}

#[test]
fn an_unknown_version_is_refused_with_a_remedy() {
    let mut bytes = OldFile::new(5).bytes();
    bytes[..4].copy_from_slice(&7u32.to_le_bytes());
    let err = read_uge(&bytes).unwrap_err();
    assert_eq!(err.offset, 0);
    assert!(err.message.contains("version 7"), "{}", err.message);
    assert!(err.remedy.contains("hUGETracker"), "{}", err.remedy);
}

#[test]
fn a_truncated_file_is_refused_at_the_offset_it_ran_out() {
    let bytes = OldFile::new(4).bytes();
    let cut = &bytes[..1000];
    let err = read_uge(cut).unwrap_err();
    assert!(err.offset <= 1000 && err.offset > 700, "{}", err.offset);
    assert!(err.message.contains("ends"), "{}", err.message);
    assert!(!err.remedy.is_empty());
}

#[test]
fn a_pattern_count_larger_than_the_file_is_refused() {
    let mut file = OldFile::new(6 - 1);
    file.patterns = vec![(0, vec![])];
    let mut bytes = file.bytes();
    // The count sits right after the ticks per row.
    let at = bytes.len() - 16 * 4 - 4 * 8 - (4 + 64 * 13) - 4;
    bytes[at..at + 4].copy_from_slice(&100_000i32.to_le_bytes());
    let err = read_uge(&bytes).unwrap_err();
    assert_eq!(err.offset, at);
    assert!(err.message.contains("100000 patterns"), "{}", err.message);
}

#[test]
fn trailing_bytes_after_the_routines_are_refused() {
    let mut bytes = OldFile::new(3).bytes();
    let end = bytes.len();
    bytes.extend([1, 2, 3]);
    let err = read_uge(&bytes).unwrap_err();
    assert_eq!(err.offset, end);
    assert!(err.message.contains("3 bytes"), "{}", err.message);
}
