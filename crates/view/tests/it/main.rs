//! The view crate's integration tests, one binary: every view against its golden PNG.
//! One binary links once and starts once; a test that needs a process of its
//! own is a file beside this directory instead, saying why.

mod common;
mod golden;
mod views;
