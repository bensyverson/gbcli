//! Compiles the vendored SameBoy `Core/` directly with the cc crate.
//!
//! SameBoy's own `make lib` is unusable on macOS (the dylib exports no symbols,
//! and without `cppp` the installed headers come out empty while make exits 0),
//! so we compile the sources ourselves with `GB_INTERNAL` for struct access.
//! Flags and feature switches are explained in
//! `project/2026-11-06-sameboy-vendoring.md`.

use std::fs;
use std::path::Path;

#[path = "patch_set.rs"]
mod patch_set;

const SAMEBOY: &str = "SameBoy";
/// gbcli's patches over SameBoy, already applied to `SameBoy/Core`; their hash
/// is part of the core identity a save state records.
const PATCHES: &str = "patches";
/// Our C accessors over SameBoy internals, compiled with the same flags.
const SHIM: &str = "shim/gbcli_shim.c";
/// The layer-tag encoding, exported for the test of its Rust mirror.
const SHIM_LAYERS: &str = "shim/gbcli_layers.c";
/// Battery saves without host time.
const SHIM_BATTERY: &str = "shim/gbcli_battery.c";

/// Sources left out because their features are compiled out below
/// (the same files SameBoy's Makefile filters for these switches).
const EXCLUDED_SOURCES: &[&str] = &[
    // GB_DISABLE_DEBUGGER
    "debugger.c",
    "sm83_disassembler.c",
    "symbol_hash.c",
    // GB_DISABLE_REWIND
    "rewind.c",
    // GB_DISABLE_CHEAT_SEARCH (implied by GB_DISABLE_DEBUGGER)
    "cheat_search.c",
];

const FEATURE_SWITCHES: &[&str] = &[
    "GB_DISABLE_DEBUGGER",
    "GB_DISABLE_REWIND",
    "GB_DISABLE_CHEAT_SEARCH",
];

/// SameBoy's own warning set (Makefile `WARNINGS`), minus `-Werror`: a new
/// compiler's warning in vendored code shouldn't break our build.
const WARNING_FLAGS: &[&str] = &[
    "-Wno-extra", // cc adds -Wall -Wextra; SameBoy builds with -Wall only
    "-Wno-unknown-warning-option",
    "-Wno-missing-braces",
    "-Wno-nonnull",
    "-Wno-unused-result",
    "-Wno-multichar",
    "-Wno-int-in-bool-context",
    "-Wno-format-truncation",
    "-Wno-nullability-completeness",
];

fn main() {
    let root = Path::new(SAMEBOY);
    println!("cargo::rerun-if-changed={SAMEBOY}");
    println!("cargo::rerun-if-changed={SHIM}");
    println!("cargo::rerun-if-changed={SHIM_LAYERS}");
    println!("cargo::rerun-if-changed={SHIM_BATTERY}");
    println!("cargo::rerun-if-changed={PATCHES}");
    println!("cargo::rerun-if-changed=patch_set.rs");

    // The core identity: a save state from another commit or another patch
    // set may have another layout (`project/2026-11-06-sameboy-vendoring.md`).
    let commit = fs::read_to_string(root.join("COMMIT")).expect("read SameBoy/COMMIT");
    let patch_set = patch_set::hash(Path::new(PATCHES));
    println!("cargo::rustc-env=GBCLI_SAMEBOY_PATCH_SET={patch_set}");
    println!(
        "cargo::rustc-env=GBCLI_SAMEBOY_CORE_VERSION={}+patches.{patch_set}",
        commit.trim_ascii_end()
    );

    let version = makefile_value(&root.join("version.mk"), "VERSION");
    let copyright_year = copyright_year(&root.join("LICENSE"));

    let mut build = cc::Build::new();
    for entry in fs::read_dir(root.join("Core")).expect("read SameBoy/Core") {
        let path = entry.expect("read SameBoy/Core entry").path();
        let is_c = path.extension().is_some_and(|ext| ext == "c");
        let name = path.file_name().unwrap().to_str().unwrap();
        if is_c && !EXCLUDED_SOURCES.contains(&name) {
            build.file(&path);
        }
    }
    build.file(SHIM);
    build.file(SHIM_LAYERS);
    build.file(SHIM_BATTERY);
    build
        .include(root)
        .std("gnu11")
        .opt_level(3) // even in debug builds: tests run the emulator for real
        .define("_GNU_SOURCE", None)
        .define("NDEBUG", None)
        .define("GB_INTERNAL", None)
        .define("GB_VERSION", format!("\"{version}\"").as_str())
        .define(
            "GB_COPYRIGHT_YEAR",
            format!("\"{copyright_year}\"").as_str(),
        )
        .warnings(true);
    for switch in FEATURE_SWITCHES {
        build.define(switch, None);
    }
    for flag in WARNING_FLAGS {
        build.flag(flag);
    }
    build.compile("sameboy");
}

/// Reads `NAME := value` from a Makefile fragment such as SameBoy's `version.mk`.
fn makefile_value(path: &Path, name: &str) -> String {
    let text = fs::read_to_string(path).expect("read SameBoy/version.mk");
    text.lines()
        .find_map(|line| {
            let (key, value) = line.split_once(":=")?;
            (key.trim() == name).then(|| value.trim().to_owned())
        })
        .unwrap_or_else(|| panic!("{name} not found in {}", path.display()))
}

/// The last year in SameBoy's copyright line ("2015-2026" gives 2026), as its
/// Makefile derives `COPYRIGHT_YEAR` from LICENSE.
fn copyright_year(path: &Path) -> String {
    let text = fs::read_to_string(path).expect("read SameBoy/LICENSE");
    let line = text
        .lines()
        .find(|line| line.trim_start().starts_with("Copyright"))
        .expect("copyright line in SameBoy/LICENSE");
    line.split(|c: char| !c.is_ascii_digit())
        .rfind(|word| word.len() == 4)
        .expect("year in SameBoy's copyright line")
        .to_owned()
}
