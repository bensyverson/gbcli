//! `build` under the environment an agent's shell may hand it: forced color,
//! a relative `PATH` entry.

use crate::common;

use std::ffi::OsStr;
use std::fs;

use gbcli_build::{Toolchain, build};
use gbcli_protocol::{Severity, Tool};

use common::{failed, fake_tools, real, starter};

#[test]
fn forced_color_still_gives_plain_located_diagnostics() {
    let (temp, options) = starter("force-color");
    let colored = |tool| real(tool, "FORCE_COLOR=1");
    let path = fake_tools(
        &temp,
        &[
            ("rgbasm", &colored(Tool::Rgbasm)),
            ("rgblink", &colored(Tool::Rgblink)),
            ("rgbfix", &colored(Tool::Rgbfix)),
        ],
    );
    let tools = Toolchain::discover(Some(&path), &temp.0).unwrap();

    let main = temp.path("demo/src/main.asm");
    let text = fs::read_to_string(&main).unwrap();
    fs::write(&main, text.replacen("\tld sp, $E000", "\tld sp,", 1)).unwrap();
    let made = failed(build(&tools, &options));
    let error = &made.diagnostics[0];
    assert_eq!(error.tool, Tool::Rgbasm);
    assert_eq!(
        error.file.as_deref(),
        Some("demo/src/main.asm"),
        "{error:?}"
    );
    assert!(error.line.is_some(), "{error:?}");
    assert!(!error.message.contains('\x1b'), "{error:?}");

    fs::write(&main, &text).unwrap();
    fs::write(
        temp.path("demo/src/extra.asm"),
        "SECTION \"extra\", ROM0\n\tjp Nowhere\n",
    )
    .unwrap();
    let made = failed(build(&tools, &options));
    let error = &made.diagnostics[0];
    assert_eq!(
        (error.tool, error.severity),
        (Tool::Rgblink, Severity::Error)
    );
    assert_eq!(
        error.file.as_deref(),
        Some("demo/src/extra.asm"),
        "{error:?}"
    );
    assert_eq!(error.line, Some(2));
    assert!(!error.message.contains('\x1b'), "{error:?}");
}

#[test]
fn a_relative_path_entry_finds_tools_that_still_run_from_the_project() {
    let (temp, options) = starter("relative-path");
    fake_tools(
        &temp,
        &[
            ("rgbasm", &real(Tool::Rgbasm, "")),
            ("rgblink", &real(Tool::Rgblink, "")),
            ("rgbfix", &real(Tool::Rgbfix, "")),
        ],
    );
    // `fake-bin` is relative to the working directory, not the project,
    // where the tools run.
    let tools = Toolchain::discover(Some(OsStr::new("fake-bin")), &temp.0).unwrap();
    assert!(tools.path(Tool::Rgbasm).is_absolute());
    let made = build(&tools, &options).unwrap();
    assert_eq!(made.outputs.len(), 3);
}
