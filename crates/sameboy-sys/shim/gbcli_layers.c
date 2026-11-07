// The layer-tag encoding the display patch defines in Core/display.h, exported
// so sameboy-sys's tests can check the Rust mirror (GBCLI_LAYER_* in
// src/lib.rs) against the C definition.

#include "Core/gb.h"

const uint8_t gbcli_layer_tags[3] = {
    GBCLI_LAYER_BACKGROUND,
    GBCLI_LAYER_WINDOW,
    GBCLI_LAYER_OBJECT,
};
