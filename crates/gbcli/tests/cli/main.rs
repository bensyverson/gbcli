//! The CLI as a process: every verb's rows and `--json` against golden
//! transcripts, every error code's remedy, and help with no preconditions.
//! Each test drives the built binary in its own temporary project.

mod common;
mod errors;
mod help;
mod honesty;
mod verbs;
