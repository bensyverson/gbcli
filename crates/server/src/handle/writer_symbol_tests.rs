//! An audio writer's symbol (review-obs): a PC the boot ROM overlays runs no
//! cart code, so no `.sym` label names it.

use gbcli_protocol::{AudioArgs, Op, RunArgs, Start};

use super::test_support::{handler, request};
use crate::fixture::{Fixture, TUNE, TUNE_SYM};

#[test]
fn a_boot_rom_writer_gets_no_cart_symbol() {
    // tune.sym plus a cart label at $0000, as many projects have (rst vectors).
    let mut sym = b"00:0000 Rst00\n".to_vec();
    sym.extend_from_slice(TUNE_SYM);
    let f = Fixture::with_rom_sym("boot-writer", TUNE, &sym);
    let mut h = handler(&f);
    let r = h.handle(&request(Op::Run(RunArgs {
        from: Some(Start::PowerOn),
        frames: Some(400),
        audio: Some(Box::new(AudioArgs {
            summary: true,
            ..AudioArgs::default()
        })),
        ..RunArgs::default()
    })));
    let result = r.outcome.expect("ran");
    let summary = result.audio.expect("audio").summary.expect("summary");
    let boot: Vec<_> = summary.writers.iter().filter(|w| w.pc < 0x100).collect();
    assert!(!boot.is_empty(), "the boot ROM writes sound registers");
    assert!(
        boot.iter().all(|w| w.symbol.is_none()),
        "a boot ROM PC named by a cart label: {boot:?}"
    );
    // The cart's own writers are still named.
    assert!(
        summary
            .writers
            .iter()
            .any(|w| w.pc >= 0x150 && w.symbol.is_some()),
        "{:?}",
        summary.writers
    );
}
