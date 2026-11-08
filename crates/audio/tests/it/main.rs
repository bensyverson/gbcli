//! The audio crate's integration tests, one binary: the write log and the views drawn from it.
//! One binary links once and starts once; a test that needs a process of its
//! own is a file beside this directory instead, saying why.

mod common;
mod log;
mod views;
