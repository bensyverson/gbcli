//! The corpus crate's integration tests, one binary: the manifest, the runner's stops and the boot checks.
//! One binary links once and starts once; a test that needs a process of its
//! own is a file beside this directory instead, saying why.

mod docboy_boot;
mod manifest_rows;
mod smoke;
mod stop;
