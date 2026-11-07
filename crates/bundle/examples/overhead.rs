//! Bundle load + save overhead, excluding emulation: the number design §1's
//! daemon-revival trigger is measured against.
//!
//!     cargo run --release -p gbcli-bundle --example overhead
//!
//! Reports the mean of one head write (with history rotation), one head read
//! and decode, and the metadata read and write around them.

use std::fs;
use std::time::Instant;

use gbcli_bundle::{Bundle, RomSource};
use gbcli_core::{Budget, Model};
use gbcli_engine::{RunOptions, SaveMeta};
use gbcli_protocol::Rev;

const V1: &[u8] = include_bytes!("../../engine/tests/roms/rebuild_v1.gb");
const CYCLES: u64 = 200;

fn main() {
    let dir = std::env::temp_dir().join(format!("gbcli-overhead-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("demo.gb"), V1).unwrap();

    let bundle = Bundle::open_or_create(&dir).unwrap();
    let store = bundle.session("demo").unwrap();
    let mut machine = store
        .create(&RomSource {
            rom: dir.join("demo.gb"),
            sym: None,
            model: Some(Model::DmgB),
            seed: Some(0),
        })
        .unwrap();
    machine
        .run(&RunOptions::new(Budget::frames(10)))
        .expect("run");
    let cp = machine.capture(SaveMeta::default());
    store.write_head(&cp, Rev(1), None).unwrap();
    let mut head_rev = Rev(1);

    let (mut meta_read, mut head_read, mut head_write, mut meta_write) =
        (0u128, 0u128, 0u128, 0u128);
    for cycle in 1..=CYCLES {
        let _lock = store.lock(gbcli_bundle::LOCK_WAIT).unwrap();

        let start = Instant::now();
        let meta = store.read_meta().unwrap();
        meta_read += start.elapsed().as_nanos();

        let start = Instant::now();
        let head = store.read_head(head_rev).unwrap().expect("a head");
        head_read += start.elapsed().as_nanos();
        assert_eq!(head.header.rom_sha256, cp.header.rom_sha256);

        let start = Instant::now();
        head_rev = Rev(cycle + 1);
        store.write_head(&cp, head_rev, None).unwrap();
        head_write += start.elapsed().as_nanos();

        let start = Instant::now();
        store.write_meta(&meta).unwrap();
        meta_write += start.elapsed().as_nanos();
    }

    let n = u128::from(CYCLES);
    let us = |nanos: u128| nanos as f64 / n as f64 / 1000.0;
    let total = us(meta_read + head_read + head_write + meta_write);
    println!("{CYCLES} load+save cycles in {}", dir.display());
    println!("  meta read   {:8.1} µs", us(meta_read));
    println!("  head read   {:8.1} µs  (decode included)", us(head_read));
    println!(
        "  head write  {:8.1} µs  (history rotation included)",
        us(head_write)
    );
    println!("  meta write  {:8.1} µs", us(meta_write));
    println!(
        "  total       {total:8.1} µs  ({:.2}% of a 28 ms round trip)",
        total / 28_000.0 * 100.0
    );

    let _ = fs::remove_dir_all(&dir);
}
