//! What the profiler calls a routine: the `.sym` label at its entry, the
//! hardware vector it was entered at when no label is there, or the label
//! it is inside, as `Label+$n`.

use crate::sym::Symbols;

use super::Entry;

/// The name of the routine entered at `entry`.
pub(super) fn routine_name(symbols: Option<&Symbols>, entry: Entry) -> Option<String> {
    let s = symbols.and_then(|s| s.containing(entry.bank, entry.addr));
    match (s, vector(entry)) {
        (Some(s), _) if s.addr == entry.addr => Some(s.name.clone()),
        // An unlabeled vector's nearest label is whatever came before it
        // (`VBlankVector+$8` for the STAT vector), which names the wrong code.
        (_, Some(v)) => Some(v.to_string()),
        (Some(s), None) => Some(format!("{}+${:X}", s.name, entry.addr - s.addr)),
        (None, None) => None,
    }
}

/// The hardware vector at `entry`: the five interrupts and the eight RSTs.
fn vector(entry: Entry) -> Option<&'static str> {
    if entry.bank != 0 {
        return None;
    }
    Some(match entry.addr {
        0x0000 => "RST_00",
        0x0008 => "RST_08",
        0x0010 => "RST_10",
        0x0018 => "RST_18",
        0x0020 => "RST_20",
        0x0028 => "RST_28",
        0x0030 => "RST_30",
        0x0038 => "RST_38",
        0x0040 => "VBlank_vector",
        0x0048 => "STAT_vector",
        0x0050 => "Timer_vector",
        0x0058 => "Serial_vector",
        0x0060 => "Joypad_vector",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(addr: u16) -> Entry {
        Entry { bank: 0, addr }
    }

    // As the shooter's ROM had it: a label on the VBlank vector, and the STAT
    // vector's section with none (project/2026-11-08-dx-shooter.md).
    const SYM: &str = "00:0040 VBlankVector\n00:0150 Main\n";

    #[test]
    fn an_unlabeled_interrupt_vector_is_named_as_the_vector() {
        let syms = Symbols::parse(SYM).unwrap();
        let name = |addr| routine_name(Some(&syms), at(addr));
        assert_eq!(name(0x0048).as_deref(), Some("STAT_vector"));
        assert_eq!(name(0x0050).as_deref(), Some("Timer_vector"));
        assert_eq!(name(0x0058).as_deref(), Some("Serial_vector"));
        assert_eq!(name(0x0060).as_deref(), Some("Joypad_vector"));
        assert_eq!(name(0x0038).as_deref(), Some("RST_38"));
        // A label on a vector keeps its own name.
        assert_eq!(name(0x0040).as_deref(), Some("VBlankVector"));
        // Past the vectors, and inside one's eight bytes, it is label+offset.
        assert_eq!(name(0x0153).as_deref(), Some("Main+$3"));
        assert_eq!(name(0x0049).as_deref(), Some("VBlankVector+$9"));
    }

    #[test]
    fn vectors_are_named_without_a_sym() {
        assert_eq!(
            routine_name(None, at(0x0040)).as_deref(),
            Some("VBlank_vector")
        );
        assert_eq!(routine_name(None, at(0x0150)), None);
    }

    #[test]
    fn a_code_label_beats_an_end_marker_at_the_same_address() {
        // The racer's .sym: hUGEDriver's WRAM section ends with `end_zero`,
        // and the game's code loaded to RAM starts there
        // (project/2026-11-08-dx-racer.md).
        let syms = Symbols::parse(
            "00:c500 start_zero\n00:c504 RoadLoops\n00:c504 VLoop\n00:c504 end_zero\n",
        )
        .unwrap();
        let entry = Entry {
            bank: 0,
            addr: 0xC504,
        };
        let name = routine_name(Some(&syms), entry).unwrap();
        assert_ne!(name, "end_zero");
        assert!(name == "VLoop" || name == "RoadLoops", "{name}");
    }
}
