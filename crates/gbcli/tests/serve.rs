//! `gbcli serve` as a process: exact bytes over stdio, and two processes
//! alternating on one session's head (design §1, §5).

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use gbcli_bundle::{Bundle, RomSource};
use gbcli_core::Model;
use gbcli_protocol::{Response, Rev};

const V1: &[u8] = include_bytes!("../../engine/tests/roms/rebuild_v1.gb");

fn project(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("gbcli-serve-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("demo.gb"), V1).unwrap();
    dir
}

fn create_session(dir: &Path) {
    // Explicit: a GBCLI_DIR in the environment must not move the bundle.
    let bundle = Bundle::open_or_create_in(dir, None).unwrap();
    bundle
        .session("demo")
        .unwrap()
        .create(&RomSource {
            rom: dir.join("demo.gb"),
            sym: None,
            model: Some(Model::DmgB),
            seed: Some(0),
            flash: None,
        })
        .unwrap();
}

struct Server {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl Server {
    fn start(dir: &Path) -> Server {
        let mut child = Command::new(env!("CARGO_BIN_EXE_gbcli"))
            .args(["serve", "--session", "demo", "--dir"])
            .arg(dir)
            .env_remove("GBCLI_DIR")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn gbcli serve");
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Server {
            child,
            stdin,
            stdout,
        }
    }

    fn request(&mut self, line: &str) -> Response {
        writeln!(self.stdin, "{line}").unwrap();
        self.stdin.flush().unwrap();
        let mut text = String::new();
        self.stdout.read_line(&mut text).expect("a response line");
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("{e}: {text:?}"))
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn a_scripted_session_produces_exact_bytes() {
    let dir = project("golden");
    let input = concat!(
        r#"{"id":"1","op":"show"}"#,
        "\n",
        r#"{"id":"2","op":"run","args":{"frames":2}}"#,
        "\n",
        r#"{"id":"3","op":"read","args":{"reads":["u16[$C000]","[$FF44]"]}}"#,
        "\n",
    );
    let mut child = Command::new(env!("CARGO_BIN_EXE_gbcli"))
        .args(["serve", "--session", "demo", "--dir"])
        .arg(&dir)
        .arg("--rom")
        .arg(dir.join("demo.gb"))
        .env_remove("GBCLI_DIR")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    drop(child.stdin.take());
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let expected = concat!(
        r#"{"id":"1","ok":true,"rev":"s0","result":{"registers":{"a":0,"f":0,"b":0,"c":0,"d":0,"e":0,"h":0,"l":0,"sp":0,"pc":0,"af":0,"bc":0,"de":0,"hl":0,"flags":{"z":false,"n":false,"h":false,"c":false},"ime":false,"halted":false,"double_speed":false},"ppu":{"ly":0,"mode":"hblank","lcd_on":false}},"warnings":[]}"#,
        "\n",
        r#"{"id":"2","ok":true,"rev":"s1","result":{"stop":{"reason":"budget_exhausted","kind":"frames","pc":355,"bank":0,"symbol":null,"time":{"ticks":12042792,"t":140288,"since":{"kind":"boot"},"frame":2,"ly":144,"dot":null},"frame_shown":{"n":2,"vblank":"normal"},"budget":{"ticks":280576,"instructions":11276,"frames":2,"limits":{"frames":2},"defaulted":false}},"reads":[]},"warnings":[]}"#,
        "\n",
        r#"{"id":"3","ok":true,"rev":"s1","result":{"reads":[{"expr":"u16[$C000]","value":1251},{"expr":"[$FF44]","value":144}]},"warnings":[]}"#,
        "\n",
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), expected);
    let _ = fs::remove_dir_all(&dir);
}

/// A read of the machine's own frame counter.
const FRAME: &str = r#"{"id":"r","op":"read","args":{"reads":["frame"]}}"#;

fn frame(response: &Response) -> i64 {
    let result = response.result().expect("a read result");
    result.reads.as_ref().expect("reads")[0]
        .value
        .expect("a value")
}

#[test]
fn two_processes_alternate_on_one_head_and_a_stale_rev_is_refused() {
    let dir = project("alternate");
    create_session(&dir);
    let mut a = Server::start(&dir);
    let mut b = Server::start(&dir);

    let first = a.request(r#"{"id":"1","op":"run","args":{"frames":1}}"#);
    assert!(first.is_ok(), "{:?}", first.error());
    assert_eq!(first.rev, Some(Rev(1)));

    // B saw A's head, even though B's own handler was cold.
    let seen = b.request(FRAME);
    assert_eq!(seen.rev.as_ref().map(|r| r.0), Some(1));
    assert_eq!(frame(&seen), 1, "B read power-on, not the head");

    // B advances from that rev.
    let second = b.request(r#"{"id":"3","op":"run","args":{"frames":1},"rev":"s1"}"#);
    assert!(second.is_ok(), "{:?}", second.error());
    assert_eq!(second.rev.as_ref().map(|r| r.0), Some(2));

    // A reloads on the next read: the rev comes from session.json, the frame
    // from the machine, so a stale warm machine shows here.
    let seen = a.request(FRAME);
    assert_eq!(seen.rev.as_ref().map(|r| r.0), Some(2));
    assert_eq!(frame(&seen), 2, "A answered from its stale warm machine");

    // A's stale revision is refused.
    let stale = a.request(r#"{"id":"5","op":"run","args":{"frames":1},"rev":"s1"}"#);
    let error = stale.outcome.expect_err("a stale rev is refused");
    assert_eq!(error.code, gbcli_protocol::ErrorCode::RevMismatch);
    assert!(!error.remedy.is_empty());

    let _ = fs::remove_dir_all(&dir);
}
