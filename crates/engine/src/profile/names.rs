//! What an address is called, for profile rows and stop rows alike: the
//! `.sym` label at or before it, with the hardware vectors and the cartridge
//! header named when no label of their own is there.

use crate::addr::label;
use crate::sym::{Symbols, is_end_marker};

use super::Entry;

/// The name of the routine entered at `entry`.
pub(super) fn routine_name(symbols: Option<&Symbols>, entry: Entry) -> Option<String> {
    name_at(symbols, entry.bank, entry.addr)
}

/// What an address is called, as `Name` or `Name+$n`: the `.sym` label at or
/// before it, except inside a hardware vector or the cartridge header, where
/// a label from before the region names the wrong code and the region's own
/// name (`STAT_vector`, `Header`) does. Stop rows and profile rows both use
/// this, so one address has one name.
pub(crate) fn name_at(symbols: Option<&Symbols>, bank: u16, addr: u16) -> Option<String> {
    let found = symbols.and_then(|s| s.containing(bank, addr));
    let region = (bank == 0).then(|| region(addr)).flatten();
    match (found, region) {
        (Some(s), Some((start, _)))
            if s.addr >= start && !(s.addr == start && is_end_marker(&s.name)) =>
        {
            Some(label(&s.name, i64::from(addr - s.addr)))
        }
        // An end marker is the end of what came before: on a vector, the
        // vector names the code there better.
        (_, Some((start, name))) => Some(label(name, i64::from(addr - start))),
        (Some(s), None) => Some(label(&s.name, i64::from(addr - s.addr))),
        (None, None) => None,
    }
}

/// The fixed region at `addr` in ROM bank 0, with its start and name: the
/// eight RSTs and five interrupt vectors (eight bytes each), and the header.
fn region(addr: u16) -> Option<(u16, &'static str)> {
    if (0x0100..0x0150).contains(&addr) {
        return Some((0x0100, "Header"));
    }
    let start = addr & !7;
    let name = match start {
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
    };
    Some((start, name))
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
        // Past the vectors, it is label+offset; inside an unlabeled vector's
        // eight bytes it is the vector, not the label before it.
        assert_eq!(name(0x0153).as_deref(), Some("Main+$3"));
        assert_eq!(name(0x0043).as_deref(), Some("VBlankVector+$3"));
        assert_eq!(name(0x0049).as_deref(), Some("STAT_vector+$1"));
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

    /// From review-run14's probe: the label at a vector is the end of what
    /// came before it, not the name of the code there.
    #[test]
    fn an_end_marker_alone_on_a_vector_does_not_name_it() {
        let syms = Symbols::parse("00:0038 Rst38\n00:0040 Rst38_end\n00:0150 Main\n").unwrap();
        let name = |addr| routine_name(Some(&syms), at(addr));
        assert_eq!(name(0x0040).as_deref(), Some("VBlank_vector"));
        // Off a vector, an end marker at the entry is still the best name.
        let syms = Symbols::parse("00:0150 Main\n00:0160 Main_end\n").unwrap();
        assert_eq!(
            routine_name(Some(&syms), at(0x0160)).as_deref(),
            Some("Main_end")
        );
    }

    /// A stop at $0101 read `OAMDMARoutine.end+$9` (review C-11): the label
    /// before the header is not the code there.
    #[test]
    fn the_cartridge_header_is_named_header() {
        let syms = Symbols::parse("00:00f6 OAMDMARoutine.end\n00:0150 Main\n").unwrap();
        let name = |addr| name_at(Some(&syms), 0, addr);
        assert_eq!(name(0x0100).as_deref(), Some("Header"));
        assert_eq!(name(0x0101).as_deref(), Some("Header+$1"));
        assert_eq!(name(0x014F).as_deref(), Some("Header+$4F"));
        assert_eq!(name(0x0150).as_deref(), Some("Main"));
        assert_eq!(name(0x00FF).as_deref(), Some("OAMDMARoutine.end+$9"));
        // A label of the game's own inside the header region wins.
        let syms = Symbols::parse("00:0100 Entry\n").unwrap();
        assert_eq!(name_at(Some(&syms), 0, 0x0102).as_deref(), Some("Entry+$2"));
        // Only ROM bank 0: the same address elsewhere is not the header.
        assert_eq!(name_at(Some(&syms), 0, 0xC101), None);
        assert_eq!(routine_name(None, at(0x0100)).as_deref(), Some("Header"));
    }

    #[test]
    fn inside_an_unlabeled_vector_the_vector_is_the_name() {
        let syms = Symbols::parse("00:0000 Zero\n00:0150 Main\n").unwrap();
        let name = |addr| name_at(Some(&syms), 0, addr);
        assert_eq!(name(0x0043).as_deref(), Some("VBlank_vector+$3"));
        assert_eq!(name(0x0068).as_deref(), Some("Zero+$68"));
    }
}
