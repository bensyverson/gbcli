//! What the cycle profiler costs: one mode per run, over the same frames of
//! the same ROM from the same state, so `scripts/bench/profile-overhead/run.sh`
//! can interleave the modes.
//!
//! ```sh
//! cargo run --release -p gbcli-engine --example profile_bench -- ROM FRAMES MODE
//! ```
//!
//! Modes: `plain` (a run with no sink), `profile` (`Machine::profile`: the
//! execution events and the shadow stack), `frames` (`Machine::frame_budget`).
//! The ROM boots on the built-in DMG boot ROM, then runs 100 frames before
//! timing starts. Prints `MODE SECONDS SPEED` where SPEED is times realtime.

use std::time::Instant;

use gbcli_core::{Budget, CoreConfig, Model, RunRequest};
use gbcli_engine::{Backend, Machine};

const WARMUP_FRAMES: u64 = 100;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let [_, rom, frames, mode] = args.as_slice() else {
        eprintln!("usage: profile_bench ROM FRAMES plain|profile|frames");
        std::process::exit(2);
    };
    let rom = std::fs::read(rom).expect("read ROM");
    let frames: u32 = frames.parse().expect("FRAMES is a number");
    let mut m =
        Machine::new(Backend::sameboy(), CoreConfig::new(Model::DmgB, rom), None).expect("machine");
    m.core_mut()
        .run(&mut RunRequest::new(Budget::frames(WARMUP_FRAMES)))
        .expect("warm-up");

    let start_ticks = m.core().now();
    let started = Instant::now();
    match mode.as_str() {
        "plain" => {
            m.core_mut()
                .run(&mut RunRequest::new(Budget::frames(u64::from(frames))))
                .expect("run");
        }
        "profile" => {
            let profile = m
                .profile(Budget::frames(u64::from(frames)))
                .expect("profile");
            std::hint::black_box(profile);
        }
        "frames" => {
            let budget = m.frame_budget(frames).expect("frame budget");
            std::hint::black_box(budget);
        }
        other => panic!("unknown mode {other}"),
    }
    let secs = started.elapsed().as_secs_f64();
    let emulated = (m.core().now() - start_ticks).as_secs_f64();
    println!("{mode} {secs:.4} {:.1}", emulated / secs);
}
