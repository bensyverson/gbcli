//! `gbcli opcode`: sizes, cycles and flags from the sourced opcode table,
//! with no session, ROM or `.gbcli/`.

use crate::common::{Project, golden};

#[test]
fn opcode_looks_up_by_text_and_byte_and_needs_nothing() {
    let p = Project::new("opcode");
    std::fs::remove_file(p.path("demo.gb")).unwrap();
    let mut out = String::new();
    for args in [
        &["opcode", "jr nz, .loop"][..],
        &["opcode", "xor", "a"],
        &["opcode", "jr nz"],
        &["opcode", "ld a, (hl)"],
        &["opcode", "$CB7E"],
        &["opcode", "ld", "a,", "[hl+]"],
        &["opcode", "ld a, [wSpeed]"],
        &["opcode", "ret"],
        &["opcode", "call nz, Update", "--json"],
        &["opcode", "lda a, 5"],
        &["opcode", "$1FF"],
    ] {
        let run = p.gbcli(args);
        out += &run.transcript(args, &p.dir);
    }
    golden("opcode.txt", &out);
    assert!(!p.path(".gbcli").exists(), "opcode made a bundle");
}

#[test]
fn a_query_that_matches_nothing_is_a_usage_error() {
    let run = Project::new("opcode-none").gbcli(&["opcode", "ld q, a"]);
    assert_eq!(run.code, 2, "{}", run.stderr);
    assert!(run.stderr.contains("gbcli opcode ld"), "{}", run.stderr);
}

#[test]
fn opcode_needs_a_query() {
    let run = Project::new("opcode-empty").gbcli(&["opcode"]);
    assert_eq!(run.code, 2, "{}", run.stderr);
    assert!(
        run.stderr.contains("QUERY"),
        "names what's missing: {}",
        run.stderr
    );
}
