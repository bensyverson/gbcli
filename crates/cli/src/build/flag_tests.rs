//! Flags one verb shares with another: `measure`'s key timeline (as `run`
//! reads it), and `run --scale` and `--crop` on every PNG the run writes.

use gbcli_protocol::Op;

use super::tests::{command, request};

#[test]
fn measure_takes_keys_and_a_keys_file_as_run_does() {
    let file = std::env::temp_dir().join(format!("gbcli-mkeys-{}.txt", std::process::id()));
    std::fs::write(&file, "-RIGHT@30 # let go\n").unwrap();
    let path = file.display().to_string();
    let Op::Measure(args) = request(&[
        "measure",
        "Work..Work.done",
        "--keys",
        "RIGHT@0",
        "--keys-file",
        &path,
    ])
    .1
    else {
        panic!()
    };
    std::fs::remove_file(&file).unwrap();
    let text: Vec<String> = args.keys.iter().map(|k| k.to_string()).collect();
    assert_eq!(text, vec!["RIGHT@0", "-RIGHT@30"]);
    let Op::Measure(args) = request(&["measure", "Work..Work.done"]).1 else {
        panic!()
    };
    assert!(args.keys.is_empty(), "no keys unless asked");
    let u = command(&["measure", "A..B", "--keys", "Q@1"]).unwrap_err();
    assert!(u.message.contains("`Q@1`"), "{}", u.message);
}

#[test]
fn run_scale_scales_the_audio_images_and_the_filmstrip_too() {
    let Op::Run(args) = request(&[
        "run",
        "--frames",
        "20",
        "--piano",
        "p.png",
        "--spectrogram",
        "s.png",
        "--filmstrip",
        "f.png",
        "--scale",
        "4",
    ])
    .1
    else {
        panic!()
    };
    let audio = args.audio.expect("audio");
    assert_eq!(audio.piano.unwrap().scale, Some(4));
    assert_eq!(audio.spectrogram.unwrap().scale, Some(4));
    assert_eq!(args.filmstrip.unwrap().scale, Some(4));
    assert!(args.shot.is_none(), "--scale alone takes no shot");
    let Op::Run(args) = request(&["run", "--piano", "p.png"]).1 else {
        panic!()
    };
    assert_eq!(
        args.audio.unwrap().piano.unwrap().scale,
        None,
        "the server owns the default"
    );
}

#[test]
fn run_crop_crops_the_filmstrip_and_the_shot_and_is_refused_with_neither() {
    let crop = gbcli_protocol::Crop {
        x: 0,
        y: 8,
        width: 16,
        height: 24,
    };
    let Op::Run(args) = request(&["run", "--filmstrip", "f.png", "--crop", "0,8,16,24"]).1 else {
        panic!()
    };
    assert_eq!(args.filmstrip.unwrap().crop, Some(crop));
    assert!(args.shot.is_none(), "--crop alone takes no shot");
    let Op::Run(args) = request(&[
        "run",
        "--filmstrip",
        "f.png",
        "--shot",
        "a.png",
        "--crop",
        "0,8,16,24",
    ])
    .1
    else {
        panic!()
    };
    assert_eq!(args.filmstrip.unwrap().crop, Some(crop));
    assert_eq!(args.shot.unwrap().crop, Some(crop));
    let Op::Run(args) = request(&["run", "--filmstrip", "f.png"]).1 else {
        panic!()
    };
    assert_eq!(args.filmstrip.unwrap().crop, None);
    let u = command(&["run", "--crop", "0,8,16,24"]).unwrap_err();
    assert_eq!(u.message, "--crop only applies with --shot or --filmstrip");
    let u = command(&["run", "--live"]).unwrap_err();
    assert_eq!(u.message, "--live only applies with --shot");
}

#[test]
fn screen_rows_asks_for_the_rows_on_shot_and_run_shot_and_is_refused_without_a_shot() {
    let Op::Shot(args) = request(&["shot", "a.png", "--screen-rows"]).1 else {
        panic!()
    };
    assert!(args.shot.screen);
    let Op::Shot(args) = request(&["shot", "a.png"]).1 else {
        panic!()
    };
    assert!(!args.shot.screen, "no rows unless asked");
    let Op::Run(args) = request(&[
        "run",
        "--shot",
        "a.png",
        "--screen-rows",
        "--crop",
        "0,0,8,8",
    ])
    .1
    else {
        panic!()
    };
    assert!(args.shot.unwrap().screen);
    let u = command(&["run", "--filmstrip", "f.png", "--screen-rows"]).unwrap_err();
    assert_eq!(u.message, "--screen-rows only applies with --shot");
}

/// `--wall-guard SECONDS` (leaf xdES5r) rides on the request, not the args,
/// on every verb whose emulation the caller sizes; out of range is a usage
/// error naming the range, and a verb that never emulates doesn't offer it.
#[test]
fn the_verbs_that_emulate_take_wall_guard_on_the_request() {
    for line in [
        &["run", "--frames", "4700", "--wall-guard", "300"][..],
        &["step", "--wall-guard", "300"],
        &["measure", "A..B", "--wall-guard", "300"],
        &["equiv", "--frames", "60", "--wall-guard", "300"],
    ] {
        let (target, op) = request(line);
        let sent = target.request(op);
        assert_eq!(sent.wall_guard_s.map(|s| s.get()), Some(300), "{line:?}");
    }
    let (target, op) = request(&["run", "--frames", "1"]);
    assert_eq!(
        target.request(op).wall_guard_s,
        None,
        "the server's default"
    );
    for bad in ["0", "3601", "5m"] {
        let u = command(&["run", "--wall-guard", bad]).unwrap_err();
        assert!(u.message.contains("--wall-guard"), "{}", u.message);
        assert!(u.message.contains("1 to 3600"), "{}", u.message);
    }
    assert!(command(&["show", "--wall-guard", "300"]).is_err());
}
