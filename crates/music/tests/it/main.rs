//! The music crate's integration tests, one binary: reading, writing and emitting songs.
//! One binary links once and starts once; a test that needs a process of its
//! own is a file beside this directory instead, saying why.

mod asm;
mod canonical;
mod common;
mod fuzz;
mod piano;
mod samples;
mod subpattern;
mod text;
mod upgrade;
