//! gbcli's request/response layer and JSON Lines stdio server (design §5, §10).
//! The CLI and the server build the same `gbcli_protocol::Request` and share
//! everything below; this crate holds the decisions, the `gbcli` binary only
//! wires argv and stdio.

mod convert;
mod handle;
mod serve;

#[cfg(test)]
mod fixture;

pub use handle::Handler;
pub use serve::serve;
