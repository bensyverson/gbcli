//! The CLI as a process: every verb's rows and `--json` against golden
//! transcripts, every error code's remedy, help with no preconditions, and
//! `gbcli serve` over stdio.
//! Each test drives the built binary in its own temporary project.

mod audio;
mod battery;
mod common;
mod equiv;
mod errors;
mod filmstrip;
mod flash;
mod flash_build;
mod help;
mod honesty;
mod images;
mod images_gbc;
mod model;
mod music;
mod opcode;
mod profile;
mod raster;
mod recipes;
mod rgbds;
mod serve;
mod song;
mod traps;
mod verbs;
