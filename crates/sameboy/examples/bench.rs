//! Throughput and restore benchmarks for the SameBoy backend.
//!
//! ```sh
//! cargo run --release -p gbcli-sameboy --example bench -- \
//!     tests/corpus/fetched/blargg/cpu_instrs/cpu_instrs.gb [frames=3600] [render=1] [mode=plain]
//! ```
//!
//! Boots through the built-in DMG boot ROM, then times `frames` frames' worth
//! of ticks (the scripts/bench/sameboy-throughput baseline ran 3,600) and
//! reports speed against real time; then restores a checkpoint and runs one
//! full frame of ticks (70,224 T-cycles), 1,000 times. Modes:
//! - `plain`: write tracking off, no filters;
//! - `tracking`: never-written tracking on (keeps the write hook armed);
//! - `filters`: tracking off, 1,000 armed filters that never match (see `filters`);
//! - `layers`: tracking off, per-pixel layer capture on (see `Feature::PixelLayers`);
//! - `apu-log`: tracking off, the sound-register write log on (keeps the write hook armed);
//! - `audio`: tracking off, sample capture on (SameBoy renders at 32,768 Hz);
//! - `apu-log+audio`: both.

use std::time::Instant;

use gbcli_core::{
    AddrFilter, AudioCapture, BankSel, Budget, Core, CoreConfig, Model, Rendering, RunRequest,
    Ticks, WriteTracking,
};
use gbcli_sameboy::SameBoy;

const BOOT_FRAMES: u64 = 100;
const RESTORES: u32 = 1_000;

/// 1,000 entries that never match but sit on hot addresses, so most accesses
/// pass the cheap bitmap test and pay for the bank-aware check: 334 execution
/// entries across ROM bank 0x7F (cpu_instrs has 4 banks), and 333 read and
/// 333 write entries across WRAM in bank 7 (a DMG maps banks 0 and 1 only).
fn filters() -> (AddrFilter, AddrFilter, AddrFilter) {
    let (mut exec, mut read, mut write) = (AddrFilter::new(), AddrFilter::new(), AddrFilter::new());
    for i in 0..334u16 {
        exec.insert(0x4000 + i * 49, BankSel::Bank(0x7F));
    }
    for i in 0..333u16 {
        read.insert(0xC000 + i * 24, BankSel::Bank(7));
        write.insert(0xC000 + i * 24 + 12, BankSel::Bank(7));
    }
    (exec, read, write)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .expect("usage: bench <rom> [frames] [render 0|1] [mode]");
    let frames: u64 = args.next().map_or(3_600, |a| a.parse().expect("frames"));
    let render = args.next().is_none_or(|a| a != "0");
    let mode = args.next().unwrap_or_else(|| "plain".into());

    let mut config = CoreConfig::new(Model::DmgB, std::fs::read(&path).expect("read ROM"));
    config.rendering = if render {
        Rendering::On
    } else {
        Rendering::Off
    };
    config.write_tracking = match mode.as_str() {
        "tracking" => WriteTracking::On,
        "plain" | "filters" | "layers" | "apu-log" | "audio" | "apu-log+audio" => {
            WriteTracking::Off
        }
        other => panic!("unknown mode {other}"),
    };
    let (exec, read, write) = filters();
    let request = |budget| {
        let mut req = RunRequest::new(budget);
        if mode == "filters" {
            req.exec = &exec;
            req.read = &read;
            req.write = &write;
        }
        req
    };

    let mut core = SameBoy::new(config).expect("construct");
    core.run(&mut request(Budget::frames(BOOT_FRAMES)))
        .expect("boot");
    if mode == "layers" {
        core.set_pixel_layers(true).expect("layer capture");
    }
    if mode.starts_with("apu-log") {
        core.set_apu_write_log(true).expect("write log");
    }
    if mode.ends_with("audio") {
        core.set_audio_capture(Some(AudioCapture::MIXED))
            .expect("audio capture");
    }

    let ticks = frames * Ticks::PER_FRAME;
    let start = Instant::now();
    let stop = core.run(&mut request(Budget::ticks(ticks))).expect("run");
    let wall = start.elapsed().as_secs_f64();
    let (writes, samples) = (
        core.take_apu_writes().map_or(0, |w| w.len()),
        core.take_audio().map_or(0, |a| a.samples.len()),
    );
    let emulated = Ticks(stop.used.ticks).as_secs_f64();
    println!(
        "throughput: mode={mode} frames={frames} emulated={emulated:.2}s wall={wall:.3}s \
         speed={:.1}x realtime ({:.0} fps) render={} stop={:?} apu_writes={writes} \
         samples={samples}",
        emulated / wall,
        frames as f64 / wall,
        u8::from(render),
        stop.reason
    );

    let blob = core.save();
    let mut worst = 0f64;
    let start = Instant::now();
    for _ in 0..RESTORES {
        let one = Instant::now();
        core.load(&blob).expect("load");
        core.run(&mut request(Budget::ticks(Ticks::PER_FRAME)))
            .expect("frame");
        worst = worst.max(one.elapsed().as_secs_f64());
    }
    let mean = start.elapsed().as_secs_f64() / f64::from(RESTORES);
    println!(
        "restore+1 frame: mean={:.3}ms worst={:.3}ms over {RESTORES} (state {} bytes)",
        mean * 1e3,
        worst * 1e3,
        blob.len()
    );
}
