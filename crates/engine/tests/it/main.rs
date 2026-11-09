//! The engine's integration tests, one binary: the debugger, inspection, equivalence, profiling and the opcode tables.
//! One binary links once and starts once; a test that needs a process of its
//! own is a file beside this directory instead, saying why.

mod boot_rom_overlay;
mod cart_memory;
mod checkpoints;
mod common;
mod conditions;
mod debugger;
mod disasm_decode;
mod disasm_table;
mod equiv;
mod equiv_late;
mod equiv_timing;
mod event_sink;
mod exec_candidates;
mod flash_save;
mod flash_session;
mod frame_sink;
mod idle_breakpoints;
mod ignored_writes;
mod illegal_opcode_targets;
mod inspect_hwreg;
mod inspect_pure;
mod inspect_regions;
mod inspect_video;
mod lcd_off;
mod measure_keys;
mod no_symbols;
mod oam_attributes;
mod opcode_lookup;
mod opcode_lookup_spellings;
mod opcode_rgbasm_differential;
mod power_cycle;
mod ppu_mode;
mod profile;
mod profile_accounting;
mod raster;
mod rgbasm;
mod truth_positions;
mod until;
mod watch_targets;
