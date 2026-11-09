//! The native reference for the Node check: the same `Spike` calls the
//! .wasm makes, natively. `native_ref DIR ROM LINE MODEL` copies ROM into
//! DIR as `rom.gb`, opens session `web` (MODEL `-` for the default), answers
//! LINE and prints `{"response_line": <line>, "files": {name: sha256}}` for
//! `shot.png` and `mix.wav` in DIR (`null` when one wasn't written).

use std::path::PathBuf;

use gbcli_web::{Open, Spike};

pub const OUTPUTS: [&str; 2] = ["shot.png", "mix.wav"];

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [dir, rom, line, model] = args.as_slice() else {
        eprintln!("usage: native_ref DIR ROM LINE MODEL");
        std::process::exit(2);
    };
    let dir = PathBuf::from(dir);
    std::fs::create_dir_all(&dir).expect("make DIR");
    std::fs::copy(rom, dir.join("rom.gb")).expect("copy ROM");
    let mut spike = Spike::open(&Open {
        dir: dir.clone(),
        session: "web".into(),
        rom: "rom.gb".into(),
        model: (model != "-").then(|| model.clone()),
    })
    .unwrap_or_else(|e| panic!("open: {e:?}"));
    let response = spike.request_line(line);
    let files: serde_json::Map<String, serde_json::Value> = OUTPUTS
        .iter()
        .map(|name| {
            let hash = std::fs::read(dir.join(name))
                .ok()
                .map(|bytes| gbcli_engine::sha256(&bytes).to_string());
            (name.to_string(), hash.into())
        })
        .collect();
    let out = serde_json::json!({ "response_line": response, "files": files });
    println!("{out}");
}
