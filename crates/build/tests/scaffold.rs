//! `new`: what it writes and what it refuses.

mod common;

use std::fs;

use gbcli_build::{HARDWARE_INC, new_project};
use gbcli_protocol::ErrorCode;

use common::Temp;

#[test]
fn the_embedded_hardware_inc_is_the_pinned_release() {
    let source = include_str!("../data/hardware.inc/SOURCE");
    let pinned = source
        .lines()
        .find_map(|l| l.trim().strip_prefix("sha256:"))
        .expect("SOURCE records a sha256")
        .trim();
    assert_eq!(
        gbcli_engine::sha256(HARDWARE_INC.as_bytes()).to_string(),
        pinned
    );
    assert!(HARDWARE_INC.contains("def HARDWARE_INC_VERSION equs \"6.0.0\""));
    let license = include_str!("../data/hardware.inc/LICENSE");
    assert!(license.starts_with("CC0 1.0 Universal"));
}

#[test]
fn new_writes_the_starter_and_says_what_to_run_next() {
    let temp = Temp::new("new-writes");
    let made = new_project(&temp.path("demo"), &temp.0).unwrap();
    assert_eq!(made.name, "demo");
    assert_eq!(made.dir, "demo");
    assert_eq!(
        made.files,
        [
            "demo/.gitignore",
            "demo/README.md",
            "demo/include/hardware.inc",
            "demo/src/main.asm",
            "demo/src/oam_dma.asm",
        ]
    );
    for file in &made.files {
        let bytes = fs::read(temp.path(file)).unwrap();
        assert!(!bytes.is_empty(), "{file}");
    }
    assert_eq!(
        fs::read_to_string(temp.path("demo/include/hardware.inc")).unwrap(),
        HARDWARE_INC
    );
    let readme = fs::read_to_string(temp.path("demo/README.md")).unwrap();
    assert!(readme.starts_with("# demo\n"), "{readme}");
    assert!(readme.contains("build/demo.gb"), "{readme}");
    assert!(!readme.contains("{{"), "every placeholder is filled");
    let ignore = fs::read_to_string(temp.path("demo/.gitignore")).unwrap();
    assert!(ignore.lines().any(|l| l == "/.gbcli/"), "{ignore}");
    assert!(ignore.lines().any(|l| l == "/build/"), "{ignore}");
    assert_eq!(
        made.next,
        [
            "gbcli build demo",
            "gbcli run demo/build/demo.gb --session demo --frames 120 --shot first.png",
        ]
    );
}

#[test]
fn new_into_an_existing_empty_directory_is_fine() {
    let temp = Temp::new("new-empty");
    fs::create_dir(temp.path("game")).unwrap();
    let made = new_project(&temp.path("game"), &temp.path("game")).unwrap();
    assert_eq!(made.dir, ".");
    assert_eq!(made.next[0], "gbcli build");
    assert!(made.next[1].starts_with("gbcli run build/game.gb --session game"));
}

#[test]
fn new_refuses_a_directory_that_holds_anything_and_writes_nothing() {
    let temp = Temp::new("new-full");
    fs::create_dir(temp.path("demo")).unwrap();
    fs::write(temp.path("demo/notes.txt"), "mine").unwrap();
    let err = new_project(&temp.path("demo"), &temp.0).unwrap_err();
    let body = err.body();
    assert_eq!(body.code, ErrorCode::BadRequest);
    assert!(body.message.contains("demo"), "{}", body.message);
    assert!(body.remedy.contains("gbcli new"), "{}", body.remedy);
    assert_eq!(fs::read_dir(temp.path("demo")).unwrap().count(), 1);
}

#[test]
fn new_refuses_a_name_that_cant_name_a_rom_and_a_session() {
    let temp = Temp::new("new-name");
    let err = new_project(&temp.path("my game"), &temp.0).unwrap_err();
    let body = err.body();
    assert_eq!(body.code, ErrorCode::BadRequest);
    assert!(body.message.contains("my game"), "{}", body.message);
    assert!(!temp.path("my game").exists());
}
