//! `view memory` (leaf zuYlzs): a block of bytes as `read` reads each one,
//! with read's never-written rule, and the head left where it was.

use gbcli_protocol::{ErrorCode, MemoryMade, Op, ReadArgs, Response, Rev, ViewArgs, ViewKind};

use gbcli_core::Model;

use super::Handler;
use super::test_support::{handler, request, run_frames};
use crate::fixture::{Fixture, GBC_PALETTE};

/// The V1 ROM with its symbols, run until Main has written wFrame and
/// wSpeed ($C000-$C002); wNever ($C003) is never written.
fn running(name: &str) -> (Fixture, Handler) {
    let f = Fixture::with_sym(name);
    let mut h = handler(&f);
    assert!(run_frames(&mut h, 2).is_ok());
    (f, h)
}

fn memory(addr: &str, len: u32) -> ViewArgs {
    ViewArgs {
        addr: Some(addr.into()),
        len: Some(len),
        ..ViewArgs::new(ViewKind::Memory)
    }
}

fn view(h: &mut Handler, args: ViewArgs) -> Response {
    h.handle(&request(Op::View(args)))
}

fn made(response: Response) -> MemoryMade {
    assert_eq!(response.rev, Some(Rev(1)), "no new head");
    let result = response.outcome.expect("viewed");
    assert!(result.view.is_none(), "memory is its own result");
    result.memory.expect("memory")
}

fn reads(h: &mut Handler, exprs: &[&str]) -> Vec<i64> {
    let response = h.handle(&request(Op::Read(ReadArgs {
        reads: exprs.iter().map(|e| e.to_string()).collect(),
        since: None,
        allow_uninit: false,
    })));
    let result = response.outcome.expect("reads");
    result
        .reads
        .unwrap()
        .iter()
        .map(|r| r.value.unwrap())
        .collect()
}

#[test]
fn a_block_is_the_bytes_read_gives_one_by_one_and_the_head_stays() {
    let (_f, mut h) = running("memory-block");
    let before = reads(&mut h, &["t", "pc"]);
    let block = made(view(&mut h, memory("wFrame", 3)));
    assert_eq!(block.expr, "wFrame");
    assert_eq!(block.addr, Some(0xC000));
    assert_eq!(block.bank, None, "the banks mapped now");
    let one_by_one: Vec<u8> = reads(&mut h, &["[wFrame]", "[wFrame+1]", "[wSpeed]"])
        .into_iter()
        .map(|v| v as u8)
        .collect();
    assert_eq!(block.bytes, one_by_one);
    assert_eq!(
        reads(&mut h, &["t", "pc"]),
        before,
        "read class: no side effects"
    );
}

#[test]
fn the_start_takes_what_read_takes_for_an_address() {
    let (_f, mut h) = running("memory-forms");
    let by_symbol = made(view(&mut h, memory("wFrame+1", 2))).bytes;
    let by_hex = made(view(&mut h, memory("$C001", 2))).bytes;
    let by_region = made(view(&mut h, memory("wram:1", 2)));
    assert_eq!(by_symbol, by_hex);
    assert_eq!(by_region.bytes, by_hex);
    assert_eq!(by_region.addr, None, "a region offset has no CPU address");
}

#[test]
fn a_never_written_byte_is_refused_naming_the_first_unless_allowed() {
    let (_f, mut h) = running("memory-uninit");
    let err = view(&mut h, memory("wFrame", 6))
        .outcome
        .expect_err("wNever was never written");
    assert_eq!(err.code, ErrorCode::UninitializedRead);
    assert!(err.message.contains("wNever ($C003)"), "{}", err.message);
    assert!(err.remedy.contains("--allow-uninit"), "{}", err.remedy);
    // The written bytes before it still read.
    assert_eq!(made(view(&mut h, memory("wFrame", 3))).bytes.len(), 3);

    let response = view(
        &mut h,
        ViewArgs {
            allow_uninit: true,
            ..memory("wFrame", 6)
        },
    );
    let uninit: Vec<&String> = response
        .warnings
        .iter()
        .filter(|w| w.contains("uninitialized"))
        .collect();
    assert_eq!(uninit.len(), 1, "one warning for the block: {uninit:?}");
    assert!(uninit[0].contains("3 never-written bytes"), "{}", uninit[0]);
    assert!(uninit[0].contains("wNever ($C003)"), "{}", uninit[0]);
    assert_eq!(made(response).bytes.len(), 6);
}

#[test]
fn out_writes_the_raw_bytes() {
    let (f, mut h) = running("memory-out");
    let path = f.dir.join("block.bin");
    let block = made(view(
        &mut h,
        ViewArgs {
            out: Some(path.to_string_lossy().into_owned()),
            ..memory("wFrame", 3)
        },
    ));
    assert_eq!(block.out.as_deref(), Some(&*path.to_string_lossy()));
    assert_eq!(std::fs::read(&path).expect("written"), block.bytes);
}

#[test]
fn a_block_past_ffff_or_of_no_bytes_is_refused() {
    let (_f, mut h) = running("memory-range");
    let past = view(&mut h, memory("$FFFF", 2))
        .outcome
        .expect_err("past $FFFF");
    assert_eq!(past.code, ErrorCode::BadExpression, "{past:?}");
    let empty = view(&mut h, memory("wFrame", 0))
        .outcome
        .expect_err("0 bytes");
    assert_eq!(empty.code, ErrorCode::BadRequest);
    // The last byte of the address space still reads (IE, an IO register).
    assert_eq!(made(view(&mut h, memory("$FFFF", 1))).bytes.len(), 1);
}

fn refused(h: &mut Handler, addr: &str, len: u32) -> gbcli_protocol::ErrorBody {
    view(
        h,
        ViewArgs {
            allow_uninit: true,
            ..memory(addr, len)
        },
    )
    .outcome
    .expect_err(addr)
}

/// A bank names one area's bank: carried past the area's end it would read
/// the next area in a bank nothing maps (VRAM bank 1 on a GBC, a VRAM bank
/// a DMG lacks). Such a block is refused, naming the area's last address
/// and the length that fits.
#[test]
fn a_banked_block_past_its_areas_end_is_refused_naming_what_fits() {
    let f = Fixture::with_model("memory-cross-gbc", GBC_PALETTE, Model::CgbE);
    let mut h = handler(&f);
    assert!(run_frames(&mut h, 5).is_ok());
    let err = refused(&mut h, "01:7FF0", 0x20);
    assert_eq!(err.code, ErrorCode::BadExpression, "{err:?}");
    assert!(err.message.contains("$7FFF"), "{}", err.message);
    assert!(err.remedy.contains("16"), "{}", err.remedy);
    // What fits still reads, and a CPU address crosses through the mapped banks.
    assert!(view(&mut h, memory("01:7FF0", 0x10)).outcome.is_ok());
    assert!(view(&mut h, memory("$7FF0", 0x20)).outcome.is_ok());

    let (_f, mut dmg) = running("memory-cross-dmg");
    let err = refused(&mut dmg, "01:7FF0", 0x20);
    assert!(err.message.contains("$7FFF"), "{}", err.message);
    assert!(err.remedy.contains("16"), "{}", err.remedy);
}

/// `00:3E00 $400` would read ROM bank 0 again at $4000, the header at $4100.
#[test]
fn a_bank0_block_past_4000_is_refused() {
    let (_f, mut h) = running("memory-cross-bank0");
    let err = refused(&mut h, "00:3E00", 0x400);
    assert!(err.message.contains("$3FFF"), "{}", err.message);
    assert!(err.remedy.contains("512"), "{}", err.remedy);
    assert_eq!(
        made(view(&mut h, memory("00:3E00", 0x200))).bytes.len(),
        0x200
    );
}

/// A region offset spans the region's whole backing store; past its end
/// there is nothing to read.
#[test]
fn a_region_block_past_the_regions_end_is_refused_naming_what_fits() {
    let (_f, mut h) = running("memory-cross-region");
    // A DMG's VRAM is $2000 bytes.
    let err = refused(&mut h, "vram:1FF0", 0x20);
    assert_eq!(err.code, ErrorCode::BadExpression, "{err:?}");
    assert!(err.message.contains("vram:1FFF"), "{}", err.message);
    assert!(err.remedy.contains("16"), "{}", err.remedy);
}

#[test]
fn memory_options_belong_to_memory_alone() {
    let (_f, mut h) = running("memory-options");
    let missing = view(&mut h, ViewArgs::new(ViewKind::Memory))
        .outcome
        .expect_err("no addr or len");
    assert_eq!(missing.code, ErrorCode::BadRequest);
    let stray = view(
        &mut h,
        ViewArgs {
            addr: Some("wFrame".into()),
            ..ViewArgs::new(ViewKind::Oam)
        },
    )
    .outcome
    .expect_err("oam takes no address");
    assert_eq!(stray.code, ErrorCode::BadRequest);
    let png = view(
        &mut h,
        ViewArgs {
            png: Some(gbcli_protocol::ImageOut {
                path: "m.png".into(),
                scale: None,
            }),
            ..memory("wFrame", 3)
        },
    )
    .outcome
    .expect_err("memory has no PNG");
    assert_eq!(png.code, ErrorCode::BadRequest);
    assert!(png.remedy.contains("--out"), "{}", png.remedy);
}
