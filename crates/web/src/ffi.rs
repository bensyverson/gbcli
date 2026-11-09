//! The C-ABI surface JavaScript calls. Buffers cross as (pointer, length) in
//! the module's memory; a result comes back packed into one `u64`,
//! `pointer << 32 | length`, which the caller frees with `gbw_free`.
//! `u64::MAX` means the call failed before it had anything to say.

use std::fs;

use gbcli_protocol::{ErrorBody, ErrorCode};

use crate::{Open, Spike, with_spike};

const FAILED: u64 = u64::MAX;

/// A buffer of `len` bytes for the caller to fill.
#[unsafe(no_mangle)]
pub extern "C" fn gbw_alloc(len: usize) -> *mut u8 {
    let mut buffer = Vec::<u8>::with_capacity(len);
    let ptr = buffer.as_mut_ptr();
    std::mem::forget(buffer);
    ptr
}

/// Frees a buffer from `gbw_alloc` or a packed result.
///
/// # Safety
/// `ptr` and `len` are exactly what one `gbw_alloc` or one packed result
/// gave, freed once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gbw_free(ptr: *mut u8, len: usize) {
    // SAFETY: the caller hands back a buffer this module allocated with
    // capacity `len`, once.
    drop(unsafe { Vec::from_raw_parts(ptr, 0, len) });
}

fn pack(bytes: Vec<u8>) -> u64 {
    let boxed = bytes.into_boxed_slice();
    let len = boxed.len();
    let ptr = Box::into_raw(boxed) as *mut u8;
    ((ptr as u64) << 32) | len as u64
}

/// # Safety
/// `ptr..ptr+len` is initialized memory of this module.
unsafe fn bytes<'a>(ptr: *const u8, len: usize) -> &'a [u8] {
    if len == 0 {
        return &[];
    }
    // SAFETY: the caller's contract.
    unsafe { std::slice::from_raw_parts(ptr, len) }
}

/// # Safety
/// As [`bytes`].
unsafe fn text<'a>(ptr: *const u8, len: usize) -> Result<&'a str, String> {
    // SAFETY: the caller's contract.
    std::str::from_utf8(unsafe { bytes(ptr, len) }).map_err(|e| e.to_string())
}

fn error_json(body: &ErrorBody) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({ "ok": false, "error": body }))
        .expect("an error body serializes")
}

/// Opens the handler from `{"dir","session","rom"}`; answers
/// `{"ok":true}` or `{"ok":false,"error":ErrorBody}`.
///
/// # Safety
/// `ptr..ptr+len` is a buffer of this module.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gbw_open(ptr: *const u8, len: usize) -> u64 {
    // SAFETY: the caller's contract.
    let open: Result<Open, String> =
        unsafe { text(ptr, len) }.and_then(|t| serde_json::from_str(t).map_err(|e| e.to_string()));
    let open = match open {
        Ok(open) => open,
        Err(e) => {
            let body = ErrorBody::plain(ErrorCode::BadRequest, format!("open: {e}"));
            return pack(error_json(&body));
        }
    };
    match Spike::open(&open) {
        Ok(spike) => {
            with_spike(|slot| *slot = Some(spike));
            pack(br#"{"ok":true}"#.to_vec())
        }
        Err(body) => pack(error_json(&body)),
    }
}

/// Answers one protocol request line with one response line.
///
/// # Safety
/// `ptr..ptr+len` is a buffer of this module.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gbw_request(ptr: *const u8, len: usize) -> u64 {
    // SAFETY: the caller's contract.
    let Ok(line) = (unsafe { text(ptr, len) }) else {
        return FAILED;
    };
    let line = line.to_owned();
    with_spike(|slot| match slot {
        Some(spike) => pack(spike.request_line(&line).into_bytes()),
        None => FAILED,
    })
}

/// Writes `data` to `path` in the module's filesystem (WASI preopens);
/// answers an empty buffer, or the error's text.
///
/// # Safety
/// Both ranges are buffers of this module.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gbw_put_file(
    path_ptr: *const u8,
    path_len: usize,
    data_ptr: *const u8,
    data_len: usize,
) -> u64 {
    // SAFETY: the caller's contract.
    let path = match unsafe { text(path_ptr, path_len) } {
        Ok(path) => path,
        Err(e) => return pack(e.into_bytes()),
    };
    // SAFETY: the caller's contract.
    let data = unsafe { bytes(data_ptr, data_len) };
    if let Some(parent) = std::path::Path::new(path).parent()
        && let Err(e) = fs::create_dir_all(parent)
    {
        return pack(format!("{path}: {e}").into_bytes());
    }
    match fs::write(path, data) {
        Ok(()) => pack(Vec::new()),
        Err(e) => pack(format!("{path}: {e}").into_bytes()),
    }
}

/// [`crate::color_table_sha256`] over the ROM in `ptr..ptr+len`: answers
/// the hex digest, or the error's text prefixed `error: `.
///
/// # Safety
/// `ptr..ptr+len` is a buffer of this module.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gbw_color_table_sha256(ptr: *const u8, len: usize) -> u64 {
    // SAFETY: the caller's contract.
    let rom = unsafe { bytes(ptr, len) };
    match crate::color_table_sha256(rom) {
        Ok(hex) => pack(hex.into_bytes()),
        Err(e) => pack(format!("error: {e}").into_bytes()),
    }
}

#[cfg(feature = "cli")]
/// Runs one `gbcli` invocation in-process from `{"args": [...], "cwd"}`
/// ([`crate::cli`]); answers `{"code","out","err"}`.
///
/// # Safety
/// `ptr..ptr+len` is a buffer of this module.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gbw_cli(ptr: *const u8, len: usize) -> u64 {
    #[derive(serde::Deserialize)]
    struct Call {
        args: Vec<String>,
        cwd: String,
    }
    // SAFETY: the caller's contract.
    let call: Result<Call, String> =
        unsafe { text(ptr, len) }.and_then(|t| serde_json::from_str(t).map_err(|e| e.to_string()));
    match call {
        Ok(call) => pack(crate::cli(&call.args, &call.cwd).to_string().into_bytes()),
        Err(e) => {
            let body = ErrorBody::plain(ErrorCode::BadRequest, format!("cli: {e}"));
            pack(error_json(&body))
        }
    }
}

/// Reads `path` from the module's filesystem; `u64::MAX` if it can't.
///
/// # Safety
/// `ptr..ptr+len` is a buffer of this module.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gbw_get_file(ptr: *const u8, len: usize) -> u64 {
    // SAFETY: the caller's contract.
    let Ok(path) = (unsafe { text(ptr, len) }) else {
        return FAILED;
    };
    match fs::read(path) {
        Ok(data) => pack(data),
        Err(_) => FAILED,
    }
}
