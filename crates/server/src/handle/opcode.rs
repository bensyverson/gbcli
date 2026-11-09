//! `opcode`: the engine's opcode lookup as an op. Needs no session, so both
//! the bundle-less [`super::build::Tooling`] and the [`super::Handler`]
//! answer it.

use gbcli_engine::disasm::{Form, Opcode, lookup};
use gbcli_protocol::{ErrorBody, ErrorCode, OpcodeArgs, OpcodeRow, Response, ResultBody};

pub(super) fn opcode(id: String, args: &OpcodeArgs) -> Response {
    match lookup(&args.query) {
        Ok(forms) => Response::ok(
            id,
            None,
            ResultBody {
                opcodes: Some(forms.iter().map(row).collect()),
                ..ResultBody::default()
            },
        ),
        Err(e) => Response::failed(
            id,
            None,
            ErrorBody::new(ErrorCode::BadRequest, e.to_string(), e.remedy()),
        ),
    }
}

fn row(f: &Form) -> OpcodeRow {
    OpcodeRow {
        code: match f.opcode {
            Opcode::Base(b) => format!("${b:02X}"),
            Opcode::Cb(b) => format!("$CB{b:02X}"),
        },
        asm: f.asm.clone(),
        bytes: f.len,
        t: f.cycles.taken,
        t_not_taken: f.cycles.not_taken,
        flags: f.flags.into(),
    }
}

#[cfg(test)]
mod tests {
    use gbcli_protocol::{Op, OpcodeArgs, Request};

    use crate::serve::Answer;
    use crate::{Host, Tooling};

    fn ask(query: &str) -> gbcli_protocol::Response {
        let request = Request {
            id: "1".into(),
            session: None,
            actor: None,
            rev: None,
            allow_rom_change: false,
            wall_guard_s: None,
            op: Op::Opcode(OpcodeArgs {
                query: query.into(),
            }),
        };
        Tooling::new(Host::from_process()).answer(&request)
    }

    #[test]
    fn opcode_answers_without_a_bundle_with_both_branch_costs() {
        let response = ask("jr nz, .loop");
        let rows = response.outcome.expect("ok").opcodes.expect("rows");
        assert_eq!(rows.len(), 1);
        let r = &rows[0];
        assert_eq!((r.code.as_str(), r.asm.as_str()), ("$20", "jr nz, e8"));
        assert_eq!((r.bytes, r.t, r.t_not_taken), (2, 12, Some(8)));
        assert_eq!(r.flags, "----");
    }

    #[test]
    fn a_cb_opcode_is_spelled_with_its_prefix() {
        let rows = ask("swap a").outcome.expect("ok").opcodes.expect("rows");
        assert_eq!(rows[0].code, "$CB37");
    }

    #[test]
    fn no_match_is_a_bad_request_with_a_remedy() {
        let body = ask("lda a").outcome.expect_err("no such mnemonic");
        assert_eq!(body.code, gbcli_protocol::ErrorCode::BadRequest);
        assert!(body.remedy.contains("gbcli opcode ld"), "{}", body.remedy);
    }
}
