//! `opcode` rows: one per form, with T- and M-cycles.

use gbcli_protocol::ResultBody;

use super::Row;

/// T-cycles per M-cycle at single speed.
const T_PER_M: u8 = 4;

pub fn rows(r: &ResultBody) -> String {
    let mut out = String::new();
    for o in r.opcodes.iter().flatten() {
        out += &Row::new("opcode")
            .field("code", &o.code)
            .field("asm", &o.asm)
            .field("bytes", o.bytes)
            .field("t", o.t)
            .opt("t_not_taken", o.t_not_taken)
            .field("m", o.t / T_PER_M)
            .opt("m_not_taken", o.t_not_taken.map(|t| t / T_PER_M))
            .field("flags", &o.flags)
            .line();
    }
    out
}
