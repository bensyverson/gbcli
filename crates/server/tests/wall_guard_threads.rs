//! The wall guard holds one thread per handler, however many requests it
//! answers. Alone in its own test binary, so the process's thread count is
//! this test's alone.

use std::fs;
use std::path::PathBuf;

use gbcli_bundle::{Bundle, RomSource};
use gbcli_core::Model;
use gbcli_protocol::{Op, Request, StepArgs, StepKind};
use gbcli_server::Handler;

const V1: &[u8] = include_bytes!("../../engine/tests/roms/rebuild_v1.gb");
const STEPS: usize = 1000;

/// The threads this process has now, read from the OS.
fn threads() -> usize {
    #[cfg(target_os = "linux")]
    {
        fs::read_dir("/proc/self/task").unwrap().count()
    }
    #[cfg(not(target_os = "linux"))]
    {
        let pid = std::process::id().to_string();
        let out = std::process::Command::new("ps")
            .args(["-M", "-p", &pid])
            .output()
            .expect("ps lists this process's threads");
        // A header line, then one line per thread.
        String::from_utf8_lossy(&out.stdout).lines().count() - 1
    }
}

#[test]
fn a_thousand_steps_leave_no_threads_behind() {
    let dir: PathBuf = std::env::temp_dir().join(format!("gbcli-threads-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("demo.gb"), V1).unwrap();
    let bundle = Bundle::open_or_create_in(&dir, None).unwrap();
    bundle
        .session("demo")
        .unwrap()
        .create(&RomSource {
            rom: dir.join("demo.gb"),
            sym: None,
            model: Model::DmgB,
            seed: 0,
        })
        .unwrap();
    let mut handler = Handler::new(bundle, Some("demo".into()), None);
    let step = Request {
        id: "1".into(),
        session: None,
        actor: None,
        rev: None,
        op: Op::Step(StepArgs {
            unit: StepKind::Instruction,
        }),
    };
    assert!(handler.handle(&step).is_ok());
    let before = threads();
    for _ in 1..STEPS {
        let response = handler.handle(&step);
        assert!(response.is_ok(), "{:?}", response.error());
    }
    let after = threads();
    assert!(after <= before, "{before} threads before, {after} after");
    let _ = fs::remove_dir_all(&dir);
}
