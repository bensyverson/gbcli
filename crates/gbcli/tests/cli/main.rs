//! The CLI as a process: every verb's rows and `--json` against golden
//! transcripts, every error code's remedy, help with no preconditions, and
//! `gbcli serve` over stdio.
//! Each test drives the built binary in its own temporary project.

mod assets;
mod audio;
mod battery;
mod common;
mod creation;
mod equiv;
mod errors;
mod filmstrip;
mod flash;
mod flash_build;
mod font;
mod help;
mod honesty;
mod images;
mod images_gbc;
mod model;
mod music;
mod observe;
mod opcode;
mod pixels;
mod profile;
mod raster;
mod recipes;
mod restart;
mod rgbds;
mod serve;
mod serve_bare;
mod serve_ephemeral;
mod song;
mod starter_gbc;
mod starter_text;
mod traps;
mod verbs;
