//! The build crate's integration tests, one binary: builds, images, music, safety and the doctor.
//! One binary links once and starts once; a test that needs a process of its
//! own is a file beside this directory instead, saying why.

mod build;
mod build_env;
mod build_safety;
mod common;
mod doctor;
mod images_bg;
mod images_dmg_version;
mod images_edges;
mod images_indexed;
mod images_joint;
mod images_named_palettes;
mod images_palette;
mod images_palette_rules;
mod images_png_encodings;
mod images_shade_edges;
mod images_shades;
mod images_sprites;
mod music;
mod music_equiv;
mod music_regressions;
mod player;
mod pngs;
mod rom_banks;
mod scaffold;
