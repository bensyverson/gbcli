//! The native side of the web spike's benchmark (crates/web/page/bench-core.js):
//! the same `Spike` calls the .wasm makes, timed the same way, so the
//! figures compare. `native_bench DIR ROM [FRAMES] [REPS]` prints one JSON
//! object. Start-up here has no compile or instantiate step: open, then the
//! first frame with its shot read back.

use std::path::PathBuf;
use std::time::Instant;

use gbcli_web::{Open, Spike};
use serde_json::{Value, json};

const WARMUP: u64 = 600;
const LOOP: u32 = 300;

fn ms(since: Instant) -> f64 {
    (since.elapsed().as_secs_f64() * 10_000.0).round() / 10.0
}

fn run(spike: &mut Spike, req: Value) -> Value {
    let out: Value = serde_json::from_str(&spike.request_line(&req.to_string())).expect("JSON");
    assert_eq!(out["ok"], true, "{out}");
    out
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (dir, rom) = match args.as_slice() {
        [dir, rom, ..] => (PathBuf::from(dir), rom.clone()),
        _ => {
            eprintln!("usage: native_bench DIR ROM [FRAMES] [REPS]");
            std::process::exit(2);
        }
    };
    let frames: u64 = args.get(2).map_or(3000, |s| s.parse().expect("FRAMES"));
    let reps: usize = args.get(3).map_or(5, |s| s.parse().expect("REPS"));
    std::fs::create_dir_all(&dir).expect("make DIR");

    let t0 = Instant::now();
    std::fs::copy(&rom, dir.join("rom.gb")).expect("copy ROM");
    let mut spike = Spike::open(&Open {
        dir: dir.clone(),
        session: "web".into(),
        rom: "rom.gb".into(),
        model: None,
    })
    .unwrap_or_else(|e| panic!("open: {e:?}"));
    let open = ms(t0);
    let t1 = Instant::now();
    run(
        &mut spike,
        json!({"id":"f","op":"run","args":{"frames":1,"shot":{"path":"f.png","scale":1}}}),
    );
    let png = std::fs::read(dir.join("f.png")).expect("first frame");
    let first = ms(t1);

    run(
        &mut spike,
        json!({"id":"w","op":"run","args":{"frames":WARMUP}}),
    );
    let mut fps = Vec::new();
    for i in 0..reps {
        let a = Instant::now();
        let r = run(
            &mut spike,
            json!({"id":format!("r{i}"),"op":"run","args":{"frames":frames}}),
        );
        let secs = a.elapsed().as_secs_f64();
        assert_eq!(r["result"]["stop"]["budget"]["frames"], frames);
        fps.push(frames as f64 / secs);
    }
    fps.sort_by(f64::total_cmp);
    let median = if reps % 2 == 1 {
        fps[reps / 2]
    } else {
        (fps[reps / 2 - 1] + fps[reps / 2]) / 2.0
    };

    let la = Instant::now();
    for i in 0..LOOP {
        run(
            &mut spike,
            json!({"id":format!("l{i}"),"op":"run","args":{"frames":1,"shot":{"path":"f.png","scale":1}}}),
        );
        std::fs::read(dir.join("f.png")).expect("frame");
    }
    let loop_secs = la.elapsed().as_secs_f64();

    let r1 = |x: f64| (x * 10.0).round() / 10.0;
    let out = json!({
        "host": "native (release)",
        "rom": rom,
        "startup_ms": {"open": open, "first_frame": first, "total": r1(open + first)},
        "first_png_bytes": png.len(),
        "throughput": {"frames": frames, "reps": reps, "fps_median": r1(median),
            "fps_min": r1(fps[0]), "fps_max": r1(fps[reps - 1])},
        "interactive": {"requests": LOOP,
            "per_request_ms": (loop_secs * 1000.0 / f64::from(LOOP) * 100.0).round() / 100.0,
            "fps": r1(f64::from(LOOP) / loop_secs)},
    });
    println!("{out}");
}
