//! Web spike (Stage 12, leaf 3zNDPj): gbcli's own protocol handler, unchanged
//! but for the cuts the findings doc lists, built for `wasm32-wasip1` and
//! driven from JavaScript through a few C-ABI exports. Spike code: disposable.
//!
//! The session bundle stays the filesystem: under WASI it is whatever the
//! host preopens (a temp directory in Node, an in-memory tree in a browser
//! shim). A ROM comes in as bytes (`gbw_put_file`) and a shot goes out as
//! bytes (`gbw_get_file`); requests and responses are the protocol's own
//! JSON lines, answered by `gbcli_server::serve` exactly as `gbcli serve`
//! answers them.

use std::cell::RefCell;
use std::io::Cursor;
use std::path::PathBuf;

use gbcli_bundle::{Bundle, RomSource};
use gbcli_protocol::ErrorBody;
use gbcli_server::{Handler, Host};
use serde::Deserialize;

mod ffi;

/// What `open` needs: the project directory the bundle lives in (and
/// relative request paths start at), the session and its ROM.
#[derive(Debug, Deserialize)]
pub struct Open {
    pub dir: PathBuf,
    pub session: String,
    /// Relative to `dir`.
    pub rom: PathBuf,
    /// A model's wire name (`dmg`, `gbc`, ...); the bundle's default when
    /// absent.
    #[serde(default)]
    pub model: Option<String>,
}

/// One handler over one project's bundle, its target session opened (and
/// created from the ROM) first, as the CLI's `start_handler` does.
pub struct Spike {
    handler: Handler,
}

impl Spike {
    pub fn open(open: &Open) -> Result<Spike, Box<ErrorBody>> {
        let model = match open.model.as_deref() {
            None => None,
            Some(name) => Some(gbcli_protocol::model_from_name(name).ok_or_else(|| {
                Box::new(ErrorBody::plain(
                    gbcli_protocol::ErrorCode::BadRequest,
                    format!("unknown model {name:?}"),
                ))
            })?),
        };
        let bundle = Bundle::open_or_create(&open.dir).map_err(|e| Box::new(e.body()))?;
        let source = RomSource {
            rom: open.rom.clone(),
            sym: None,
            model,
            seed: None,
            flash: None,
        };
        bundle
            .open_session(&open.session, Some(&source))
            .map_err(|e| Box::new(e.body()))?;
        let mut handler = Handler::new(bundle, Some(open.session.clone()), None);
        handler.set_host(Host {
            cwd: open.dir.clone(),
            path: None,
        });
        Ok(Spike { handler })
    }

    /// Answers one protocol line as `gbcli serve` would, returning the
    /// response line without its newline.
    pub fn request_line(&mut self, line: &str) -> String {
        let mut out = Vec::new();
        let input = Cursor::new(format!("{line}\n"));
        gbcli_server::serve(input, &mut out, &mut self.handler)
            .expect("in-memory reader and writer can't fail");
        let mut text = String::from_utf8(out).expect("responses are UTF-8");
        if text.ends_with('\n') {
            text.pop();
        }
        text
    }
}

/// SHA-256 of SameBoy's color conversion (`Core::rgb555_color`, default
/// color correction, which runs libm's `pow`) for every RGB555 color on a
/// GBC built straight through the engine from `rom`: the check that the
/// wasm libm (wasi-libc's) and the native one (Apple's) agree everywhere.
pub fn color_table_sha256(rom: &[u8]) -> Result<String, String> {
    use gbcli_core::{CoreConfig, Model};
    let config = CoreConfig::new(Model::CgbE, rom.to_vec());
    let machine = gbcli_engine::Machine::new(gbcli_engine::Backend::sameboy(), config, None)
        .map_err(|e| e.to_string())?;
    let mut bytes = Vec::with_capacity(0x8000 * 4);
    for color in 0..0x8000u16 {
        let rgb = machine
            .core()
            .rgb555_color(color)
            .map_err(|e| e.to_string())?;
        bytes.extend_from_slice(&rgb.to_le_bytes());
    }
    Ok(gbcli_engine::sha256(&bytes).to_string())
}

thread_local! {
    /// The one handler a page drives; wasm32 has one thread.
    static SPIKE: RefCell<Option<Spike>> = const { RefCell::new(None) };
}

fn with_spike<T>(f: impl FnOnce(&mut Option<Spike>) -> T) -> T {
    SPIKE.with(|cell| f(&mut cell.borrow_mut()))
}
