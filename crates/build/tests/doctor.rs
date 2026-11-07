//! `doctor`: each check's verdict and remedy.

mod common;

use std::fs;

use gbcli_build::{DoctorInput, doctor};
use gbcli_protocol::{CheckMade, CheckName, CheckStatus, DoctorMade, ErrorCode};

use common::{Temp, set_mode};

const GOOD_ROMS: &[(&str, &[u8])] = &[("dmg", &[0; 256]), ("cgb", &[0; 2304])];

fn check(made: &DoctorMade, name: CheckName) -> &CheckMade {
    made.checks
        .iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("no {name:?} check in {made:?}"))
}

fn run(project: &std::path::Path, path: Option<&std::ffi::OsStr>) -> Result<DoctorMade, ErrorCode> {
    doctor(&DoctorInput {
        path_var: path,
        cwd: project,
        project,
        boot_roms: GOOD_ROMS,
    })
    .map_err(|e| e.code)
}

#[test]
fn with_the_pinned_rgbds_every_check_passes_or_says_why_it_was_skipped() {
    common::rgbds();
    let temp = Temp::new("doctor-ok");
    let path = std::env::var_os("PATH");
    let made = run(&temp.0, path.as_deref()).unwrap();
    let names: Vec<_> = made.checks.iter().map(|c| c.name).collect();
    assert_eq!(
        names,
        [
            CheckName::Rgbasm,
            CheckName::Rgblink,
            CheckName::Rgbfix,
            CheckName::BootRoms,
            CheckName::Gitignore,
            CheckName::Writable,
            CheckName::CoreSupport,
        ]
    );
    for name in [CheckName::Rgbasm, CheckName::Rgblink, CheckName::Rgbfix] {
        let c = check(&made, name);
        assert_eq!(c.status, CheckStatus::Ok, "{c:?}");
        assert!(c.detail.contains("1.0.4"), "{c:?}");
    }
    let core = check(&made, CheckName::CoreSupport);
    assert_eq!(core.status, CheckStatus::Skip);
    assert!(
        core.remedy.as_deref().unwrap_or("").contains("gbcli gaps"),
        "{core:?}"
    );
    assert!(
        fs::read_dir(&temp.0).unwrap().next().is_none(),
        "the writability probe leaves nothing behind"
    );
}

#[test]
fn missing_rgbds_fails_with_toolchain_missing_and_the_install_command() {
    let temp = Temp::new("doctor-missing");
    let empty = temp.path("bin");
    fs::create_dir(&empty).unwrap();
    let err = doctor(&DoctorInput {
        path_var: Some(empty.as_os_str()),
        cwd: &temp.0,
        project: &temp.0,
        boot_roms: GOOD_ROMS,
    })
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::ToolchainMissing);
    assert!(err.remedy.contains("brew install rgbds"), "{}", err.remedy);
    let made = err
        .result
        .and_then(|r| r.doctor)
        .expect("checks ride along");
    let asm = check(&made, CheckName::Rgbasm);
    assert_eq!(asm.status, CheckStatus::Fail);
    assert!(
        asm.remedy
            .as_deref()
            .unwrap()
            .contains("releases/tag/v1.0.4"),
        "{asm:?}"
    );
}

#[test]
fn a_git_project_that_doesnt_ignore_gbcli_warns_and_one_that_does_passes() {
    let temp = Temp::new("doctor-git");
    fs::create_dir(temp.path(".git")).unwrap();
    fs::create_dir(temp.path("game")).unwrap();
    let empty = temp.path("bin");
    fs::create_dir(&empty).unwrap();
    let checks = |dir: &std::path::Path| {
        doctor(&DoctorInput {
            path_var: Some(empty.as_os_str()),
            cwd: dir,
            project: dir,
            boot_roms: GOOD_ROMS,
        })
        .unwrap_err()
        .result
        .and_then(|r| r.doctor)
        .unwrap()
    };
    let made = checks(&temp.path("game"));
    let ignore = check(&made, CheckName::Gitignore);
    assert_eq!(ignore.status, CheckStatus::Warn, "{ignore:?}");
    assert!(
        ignore.remedy.as_deref().unwrap().contains(".gbcli/"),
        "{ignore:?}"
    );

    // An ignore file anywhere from the project up to the repository root counts.
    fs::write(temp.path(".gitignore"), "target\n/.gbcli/\n").unwrap();
    let made = checks(&temp.path("game"));
    assert_eq!(check(&made, CheckName::Gitignore).status, CheckStatus::Ok);
}

#[test]
fn outside_a_git_repository_there_is_nothing_to_ignore() {
    let temp = Temp::new("doctor-nogit");
    let empty = temp.path("bin");
    fs::create_dir(&empty).unwrap();
    let made = doctor(&DoctorInput {
        path_var: Some(empty.as_os_str()),
        cwd: &temp.0,
        project: &temp.0,
        boot_roms: GOOD_ROMS,
    })
    .unwrap_err()
    .result
    .and_then(|r| r.doctor)
    .unwrap();
    // The temporary directory may itself sit in a repository; only a
    // verdict other than a failure is promised.
    assert_ne!(check(&made, CheckName::Gitignore).status, CheckStatus::Fail);
}

#[test]
fn a_boot_rom_of_the_wrong_size_fails_with_boot_rom_missing() {
    common::rgbds();
    let temp = Temp::new("doctor-boot");
    let path = std::env::var_os("PATH");
    let err = doctor(&DoctorInput {
        path_var: path.as_deref(),
        cwd: &temp.0,
        project: &temp.0,
        boot_roms: &[("dmg", &[0; 256]), ("cgb", &[0; 100])],
    })
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::BootRomMissing);
    let made = err.result.and_then(|r| r.doctor).unwrap();
    let boot = check(&made, CheckName::BootRoms);
    assert_eq!(boot.status, CheckStatus::Fail);
    assert!(boot.detail.contains("cgb"), "{boot:?}");
}

#[test]
fn an_unwritable_project_fails_with_io_failed() {
    common::rgbds();
    let temp = Temp::new("doctor-readonly");
    let project = temp.path("ro");
    fs::create_dir(&project).unwrap();
    set_mode(&project, 0o555).unwrap();
    let path = std::env::var_os("PATH");
    let result = run(&project, path.as_deref());
    set_mode(&project, 0o755).unwrap();
    assert_eq!(result.unwrap_err(), ErrorCode::IoFailed);
}

/// Every check, whether or not one failed.
fn all_checks(project: &std::path::Path) -> DoctorMade {
    let path = std::env::var_os("PATH");
    let input = DoctorInput {
        path_var: path.as_deref(),
        cwd: project,
        project,
        boot_roms: GOOD_ROMS,
    };
    match doctor(&input) {
        Ok(made) => made,
        Err(body) => body.result.and_then(|r| r.doctor).unwrap(),
    }
}

#[test]
fn whether_gbcli_is_ignored_is_asked_of_git_itself() {
    let temp = Temp::new("doctor-git-real");
    let init = std::process::Command::new("git")
        .args(["init", "-q", "."])
        .current_dir(&temp.0)
        .status()
        .expect("these tests need git on PATH");
    assert!(init.success());
    for dir in ["target/rv", "game", "other"] {
        fs::create_dir_all(temp.path(dir)).unwrap();
    }
    let status = |dir: &str| check(&all_checks(&temp.path(dir)), CheckName::Gitignore).status;

    // An ignored parent directory ignores everything under it.
    fs::write(temp.path(".gitignore"), "/target/\n").unwrap();
    assert_eq!(status("target/rv"), CheckStatus::Ok);
    assert_eq!(status("other"), CheckStatus::Warn);

    // A pattern anchored at the root ignores the root's .gbcli/ only.
    fs::write(temp.path(".gitignore"), "/.gbcli/\n").unwrap();
    assert_eq!(status("game"), CheckStatus::Warn);

    // `**/` patterns and .git/info/exclude count.
    fs::write(temp.path(".gitignore"), "**/.gbcli\n").unwrap();
    assert_eq!(status("game"), CheckStatus::Ok);
    fs::write(temp.path(".gitignore"), "").unwrap();
    fs::write(temp.path(".git/info/exclude"), ".gbcli/\n").unwrap();
    assert_eq!(status("game"), CheckStatus::Ok);
    let ok = check(&all_checks(&temp.path("game")), CheckName::Gitignore).clone();
    assert!(ok.detail.contains("git check-ignore"), "{ok:?}");
}
