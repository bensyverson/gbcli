//! gbcli's logic, as a library the server and CLI adapters wrap. The engine
//! never prints; adapters format. Stage 2 is being built per
//! `project/2026-11-06-design.md`: inspection and the disassembler so far.

pub mod disasm;
pub mod inspect;
