//! The SameBoy backend's integration tests, one binary (`stdout` keeps its own).
//! One binary links once and starts once; a test that needs a process of its
//! own is a file beside this directory instead, saying why.

mod apu;
mod audio;
mod battery;
mod boot_inputs;
mod clock_bounds;
mod common;
mod cpu_instrs;
mod display;
mod filters;
mod flash;
mod frames;
mod ignored_counts;
mod ignored_writes;
mod inspection;
mod io_decode;
mod layers;
mod mappers;
mod memory;
mod mode_sweep;
mod run;
mod save_load_property;
mod speed_switch;
mod state;
