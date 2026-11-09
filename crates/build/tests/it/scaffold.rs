//! `new`: what it writes and what it refuses.

use crate::common;

use std::fs;

use gbcli_build::{HARDWARE_INC, HUGEDRIVER, new_project};
use gbcli_protocol::ErrorCode;

use common::Temp;

#[test]
fn the_embedded_hardware_inc_is_the_pinned_release() {
    let source = include_str!("../../data/hardware.inc/SOURCE");
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
    let license = include_str!("../../data/hardware.inc/LICENSE");
    assert!(license.starts_with("CC0 1.0 Universal"));
}

/// hUGEDriver as `data/hUGEDriver/SOURCE` pins it, each file by its sha256.
#[test]
fn the_embedded_hugedriver_is_the_pinned_commit() {
    let source = include_str!("../../data/hUGEDriver/SOURCE");
    assert!(source.contains("a3cbd0cea48e6784d7f625066d0300f7cb075926"));
    assert!(source.contains("dedicated to the public"));
    for (name, text) in HUGEDRIVER {
        let file = name.rsplit('/').next().unwrap();
        let pinned = source
            .lines()
            .find_map(|l| {
                let l = l.trim().strip_prefix("sha256:").unwrap_or(l).trim();
                l.strip_prefix(file)
                    .filter(|rest| rest.starts_with(' '))
                    .map(str::trim)
            })
            .unwrap_or_else(|| panic!("SOURCE records no sha256 for {file}"));
        assert_eq!(
            gbcli_engine::sha256(text.as_bytes()).to_string(),
            pinned,
            "{file}"
        );
    }
    assert_eq!(HUGEDRIVER.len(), 3);
}

#[test]
fn the_starter_song_is_the_help_topics_example() {
    let temp = Temp::new("new-song");
    new_project(&temp.path("demo"), &temp.0).unwrap();
    assert_eq!(
        fs::read_to_string(temp.path("demo/assets/music/theme.song")).unwrap(),
        gbcli_music::EXAMPLE
    );
    let player = fs::read_to_string(temp.path("demo/src/music.asm")).unwrap();
    assert!(player.contains("call hUGE_dosound"), "{player}");
    assert!(player.contains("song_theme"), "{player}");
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
            "demo/include/hUGE.inc",
            "demo/include/hUGE_note_table.inc",
            "demo/src/hUGEDriver.asm",
            "demo/src/music.asm",
            "demo/assets/music/theme.song",
            "demo/.gbcli/",
        ]
    );
    assert!(
        temp.path("demo/.gbcli").is_dir(),
        "the project's own bundle"
    );
    for file in made.files.iter().filter(|f| !f.ends_with('/')) {
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
    // Into the project first, so its own .gbcli/ is the bundle every later
    // command finds.
    assert_eq!(
        made.next,
        [
            "cd demo",
            "gbcli build",
            "gbcli run build/demo.gb --session demo --frames 120 --shot first.png",
        ]
    );
}

#[test]
fn new_into_an_existing_empty_directory_is_fine() {
    let temp = Temp::new("new-empty");
    fs::create_dir(temp.path("game")).unwrap();
    let made = new_project(&temp.path("game"), &temp.path("game")).unwrap();
    assert_eq!(made.dir, ".");
    assert_eq!(made.next.len(), 2, "already there, no cd: {:?}", made.next);
    assert_eq!(made.next[0], "gbcli build");
    assert!(made.next[1].starts_with("gbcli run build/game.gb --session game"));
}

/// Only the last component must be a session name; the rest of the path may
/// hold anything, so every command `new` prints quotes it for a shell.
#[test]
fn new_prints_commands_a_shell_can_run() {
    let temp = Temp::new("new-space");
    fs::create_dir_all(temp.path("my games")).unwrap();
    let made = new_project(&temp.path("my games/demo"), &temp.0).unwrap();
    assert_eq!(
        made.next,
        [
            "cd 'my games/demo'",
            "gbcli build",
            "gbcli run build/demo.gb --session demo --frames 120 --shot first.png",
        ]
    );
    let refused = new_project(&temp.path("my games/Demo Two"), &temp.0).unwrap_err();
    assert!(
        refused
            .body()
            .remedy
            .contains("`gbcli new 'my games/demo-two'`"),
        "{}",
        refused.body().remedy
    );
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

#[test]
fn new_refuses_a_name_a_session_cant_take_and_offers_one_it_can() {
    let temp = Temp::new("new-session-name");
    let long = "a".repeat(41);
    for (name, offered) in [
        ("my-game_2", "my-game-2".to_string()),
        ("my.game", "my-game".to_string()),
        ("MyGame", "mygame".to_string()),
        (long.as_str(), "a".repeat(40)),
    ] {
        let err = new_project(&temp.path(name), &temp.0).unwrap_err();
        let body = err.body();
        assert_eq!(body.code, ErrorCode::BadRequest, "{name}");
        assert!(body.message.contains(name), "{}", body.message);
        assert!(
            body.message.contains("[a-z0-9-]"),
            "the message states the session rule: {}",
            body.message
        );
        assert!(
            body.remedy.contains(&format!("`gbcli new {offered}`")),
            "{name}: {}",
            body.remedy
        );
        assert!(!temp.path(name).exists(), "{name}");
    }
    // The offer keeps the directory's parent.
    let body = new_project(&temp.path("games/My_Game"), &temp.0)
        .unwrap_err()
        .body();
    assert!(
        body.remedy.contains("`gbcli new games/my-game`"),
        "{}",
        body.remedy
    );
}

#[test]
fn every_name_new_accepts_is_a_session_name() {
    let temp = Temp::new("new-accepts");
    for name in ["game", "my-game-2", "a1"] {
        let made = new_project(&temp.path(name), &temp.0).unwrap();
        assert!(gbcli_protocol::is_session_name(&made.name), "{name}");
        assert!(
            made.next
                .last()
                .unwrap()
                .contains(&format!("--session {name} ")),
            "{:?}",
            made.next
        );
    }
}
