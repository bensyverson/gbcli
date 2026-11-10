//! What a warm request pays to recheck its ROM (design §1's files stamp),
//! on an 8 MiB cart (the largest MBC5 ROM) and the 32 KiB test ROM, beside
//! a stat and a cold machine build for scale.
//!
//!     cargo run --release -p gbcli-bundle --example stamp_cost
//!
//! "settled" is a ROM last written over 2 s ago, the usual warm case; a
//! check is then the store's change token. "racy" is one written just now,
//! which a check reads and hashes until it settles.

use std::fs::{self, File};
use std::path::Path;
use std::time::{Duration, Instant, SystemTime};

use gbcli_bundle::{Bundle, RomSource, SessionStore};
use gbcli_core::Model;

const ROM: &[u8] = include_bytes!("../../engine/tests/roms/rebuild_v1.gb");
const N: u32 = 20;

fn main() {
    let dir = std::env::temp_dir().join(format!("gbcli-stamp-cost-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let rom = dir.join("demo.gb");
    fs::write(&rom, ROM).unwrap();
    let bundle = Bundle::open_or_create(&dir).unwrap();
    let store = bundle.session("demo").unwrap();
    store
        .create(&RomSource {
            rom: rom.clone(),
            sym: None,
            model: Some(Model::DmgB),
            seed: Some(0),
            flash: None,
        })
        .unwrap();

    let mut big = ROM.to_vec();
    big.resize(8 * 1024 * 1024, 0xFF);
    for (label, bytes) in [("8 MiB", big.as_slice()), ("32 KiB", ROM)] {
        fs::write(&rom, bytes).unwrap();
        println!("{label} ROM");
        println!("  racy check      {:>10?}", check(&store, false));
        backdate(&rom);
        println!("  settled check   {:>10?}", check(&store, true));
        println!(
            "  a stat          {:>10?}",
            mean(|| {
                fs::metadata(&rom).unwrap();
            })
        );
        let meta = store.read_meta().unwrap();
        println!(
            "  cold build      {:>10?}",
            mean(|| {
                store.machine(&meta).unwrap();
            })
        );
    }
    let _ = fs::remove_dir_all(&dir);
}

/// The mean `still_current` after a stamp taken as the server takes one.
fn check(store: &SessionStore, settled: bool) -> Duration {
    let meta = store.read_meta().unwrap();
    let mut stamp = store.files_stamp(&meta);
    stamp.built(&store.machine(&meta).unwrap());
    if settled {
        // The first check after settling reads once and adopts the token.
        assert!(stamp.still_current());
    }
    mean(|| assert!(stamp.still_current()))
}

fn mean(mut f: impl FnMut()) -> Duration {
    let start = Instant::now();
    for _ in 0..N {
        f();
    }
    start.elapsed() / N
}

fn backdate(path: &Path) {
    File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(SystemTime::now() - Duration::from_secs(3600))
        .unwrap();
}
