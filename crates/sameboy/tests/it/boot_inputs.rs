//! `boot_inputs`: the bytes of a ROM that decide its boot handoff, which
//! the boot cache keys on instead of the whole ROM.

use gbcli_core::Model;
use gbcli_sameboy::boot_inputs;

/// A 32 KB ROM whose every byte differs from its neighbor's.
fn rom() -> Vec<u8> {
    (0..0x8000u32).map(|i| (i % 251) as u8).collect()
}

#[test]
fn a_32k_rom_s_inputs_are_its_length_and_the_header_bytes_the_boot_reads() {
    let base = rom();
    let inputs = boot_inputs(&base, Model::DmgB).expect("a 32 KB ROM has inputs");
    let mut want = (base.len() as u64).to_le_bytes().to_vec();
    want.extend_from_slice(&base[0x104..0x14E]);
    assert_eq!(inputs, want);

    // The entry point, the global checksum and the body: never read before
    // the handoff.
    let mut other = base.clone();
    other[0x100] ^= 0xFF;
    other[0x14E] ^= 0xFF;
    other[0x14F] ^= 0xFF;
    other[0x150] ^= 0xFF;
    other[0x7FFF] ^= 0xFF;
    assert_eq!(boot_inputs(&other, Model::DmgB), Some(want.clone()));
    assert_eq!(
        boot_inputs(&other, Model::CgbE),
        boot_inputs(&base, Model::CgbE),
        "the CGB boot reads the title and licensee, not past $014D"
    );

    // The logo, the title and the header checksum are read.
    for at in [0x104, 0x134, 0x147, 0x14D] {
        let mut read = base.clone();
        read[at] ^= 0xFF;
        assert_ne!(
            boot_inputs(&read, Model::DmgB),
            Some(want.clone()),
            "${at:04X}"
        );
    }
}

#[test]
fn a_shorter_rom_s_length_is_an_input() {
    let base = rom();
    let short = &base[..0x4000];
    assert_ne!(
        boot_inputs(short, Model::DmgB),
        boot_inputs(&base, Model::DmgB)
    );
}

#[test]
fn the_super_game_boy_boot_also_sends_the_global_checksum() {
    let base = rom();
    let mut other = base.clone();
    other[0x14E] ^= 0xFF;
    for model in [Model::Sgb, Model::Sgb2] {
        let inputs = boot_inputs(&base, model).expect("inputs");
        assert_eq!(&inputs[8..], &base[0x104..0x150], "{model:?}");
        assert_ne!(boot_inputs(&other, model), Some(inputs), "{model:?}");
    }
}

#[test]
fn a_rom_past_32k_has_no_inputs_short_of_the_whole_rom() {
    // SameBoy's cart setup reads past the header there (MMM01 in the last
    // bank, MBC1's wiring guess), so only the whole ROM keys it.
    let mut big = rom();
    big.extend(rom());
    assert_eq!(boot_inputs(&big, Model::DmgB), None);
}

#[test]
fn a_tpp1_cart_has_no_inputs_short_of_the_whole_rom() {
    // SameBoy reads its RAM size at $0152, past the header.
    let mut tpp1 = rom();
    tpp1[0x147] = 0xBC;
    assert_eq!(boot_inputs(&tpp1, Model::DmgB), None);
}

#[test]
fn a_rom_without_a_whole_header_has_no_inputs() {
    assert_eq!(boot_inputs(&rom()[..0x14D], Model::DmgB), None);
}
