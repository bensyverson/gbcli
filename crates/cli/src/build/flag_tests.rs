//! Flags one verb shares with another: `measure`'s key timeline (as `run`
//! reads it) and `run --scale` on every PNG the run writes.

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
