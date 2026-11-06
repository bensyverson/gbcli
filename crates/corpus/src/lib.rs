//! Runs the pinned test-ROM corpus (`tests/corpus/manifest.json`) through any
//! core behind the `Core` trait and records a baseline. The corpus and its
//! pass rules: `project/2026-11-06-test-rom-corpus.md`; the SameBoy baseline:
//! `project/2026-11-06-sameboy-baseline.md`.

pub mod baseline;
pub mod docboy;
pub mod drive;
pub mod frame;
pub mod judge;
pub mod manifest;
pub mod models;
pub mod rules;
