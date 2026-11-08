//! The layer-tag encoding is defined once, in the patched `display.h`; the
//! Rust constants mirror it, and this checks the mirror against the C values.

use sameboy_sys as sb;

#[test]
fn rust_layer_tags_match_display_h() {
    // SAFETY: a constant array defined in shim/gbcli_layers.c.
    let c = unsafe { sb::gbcli_layer_tags };
    assert_eq!(
        [
            sb::GBCLI_LAYER_BACKGROUND,
            sb::GBCLI_LAYER_WINDOW,
            sb::GBCLI_LAYER_OBJECT
        ],
        c
    );
}
