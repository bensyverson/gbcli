//! A fast end-to-end check of the runner on SameBoy: one row per rule kind
//! the full baseline leans on most. The full corpus run is `gbcli-corpus`
//! (see `project/2026-11-06-sameboy-baseline.md`), never `cargo test`.

use std::path::PathBuf;

use gbcli_core::{Core, CoreConfig, CoreError};
use gbcli_corpus::judge;
use gbcli_corpus::manifest;
use gbcli_corpus::models;
use gbcli_corpus::rules::Verdict;
use gbcli_sameboy::SameBoy;

const SMOKE: &[&str] = &[
    "mooneye/acceptance/add_sp_e_timing.gb",
    "blargg/cpu_instrs/individual/01-special.gb",
    "gbmicrotest/div_inc_timing_a.gb",
    "dmg-acid2/dmg-acid2.gb@dmg",
];

fn sameboy(config: CoreConfig) -> Result<Box<dyn Core>, CoreError> {
    Ok(Box::new(SameBoy::new(config)?))
}

#[test]
fn smoke_rows_pass_on_their_first_model() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let text = std::fs::read_to_string(root.join("tests/corpus/manifest.json")).unwrap();
    let rows = manifest::parse(&text).unwrap().rows;
    let corpus = root.join("tests/corpus/fetched");
    for id in SMOKE {
        let row = rows
            .iter()
            .find(|r| r.id == *id)
            .expect("smoke row in manifest");
        assert!(
            corpus.join(&row.path).exists(),
            "{} is missing: fetch the corpus with `python3 -I scripts/corpus/fetch.py`",
            row.path
        );
        let target = &models::targets(&row.models)[0];
        let ctx = judge::Context {
            corpus: &corpus,
            factory: sameboy,
            dump: None,
        };
        let outcome = judge::run(row, target, ctx);
        assert_eq!(outcome.verdict, Verdict::Pass, "{id} on {}", outcome.model);
        // Regression: serial bytes were read from SB after a bit had shifted out.
        for alt in &outcome.alternatives {
            assert_eq!(
                *alt,
                Verdict::Pass,
                "{id}'s alternative on {}",
                outcome.model
            );
        }
    }
}
