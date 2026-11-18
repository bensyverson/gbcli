//! `gbcli batch -F FILE`: protocol request lines, applied in order with one
//! handler, one status row each (cli-design ▸ Batches). A failed line doesn't
//! stop the lines after it.

use std::io::Read;

use gbcli_protocol::{ErrorBody, ErrorCode, parse_request};

use crate::build::Target;
use crate::exec::{self, emit};
use crate::render::{self, Row};
use crate::{Env, Io};

pub fn batch(target: &Target, file: &str, env: &Env, io: Io) -> u8 {
    let text = match read(file, env, io.stdin) {
        Ok(text) => text,
        Err(body) => return render::error(io.err, &body),
    };
    let mut handler = match exec::responder(target, env) {
        Ok(handler) => handler,
        Err(body) => return render::error(io.err, &body),
    };
    let (mut applied, mut failed, mut exit) = (0, 0, 0);
    for (index, line) in text.split('\n').enumerate() {
        let line = line.trim_end_matches('\r');
        if line.trim().is_empty() {
            continue;
        }
        let number = index + 1;
        let response = exec::answer_line(&mut handler, line);
        for warning in &response.warnings {
            emit(io.err, &format!("warning: line {number}: {warning}\n"));
        }
        if target.json {
            let text = serde_json::to_string(&response).expect("a response always serializes");
            emit(io.out, &format!("{text}\n"));
        } else {
            emit(io.out, &render::session_created(&response));
        }
        let op = parse_request(line).ok().map(|r| r.op.name());
        let row = Row::new("line")
            .field("n", number)
            .opt("id", (!response.id.is_empty()).then_some(&response.id))
            .opt("op", op);
        let row = match &response.outcome {
            Ok(_) => {
                applied += 1;
                row.flag("ok", true).opt("rev", response.rev)
            }
            Err(error) => {
                failed += 1;
                if exit == 0 {
                    exit = error.exit;
                }
                row.flag("ok", false)
                    .field("code", error.code)
                    .field("exit", error.exit)
                    .opt("rev", response.rev)
                    .field("message", &error.message)
                    .field("remedy", &error.remedy)
            }
        };
        if !target.json {
            emit(io.out, &row.line());
        }
    }
    if !target.json {
        let summary = Row::new("batch")
            .field("applied", applied)
            .field("failed", failed);
        emit(io.out, &summary.line());
    }
    exit
}

fn read(file: &str, env: &Env, stdin: &mut dyn std::io::BufRead) -> Result<String, Box<ErrorBody>> {
    let mut text = String::new();
    let result = if file == "-" {
        stdin.read_to_string(&mut text).map(|_| ())
    } else {
        std::fs::File::open(env.cwd.join(file))
            .and_then(|mut f| f.read_to_string(&mut text).map(|_| ()))
    };
    result.map(|()| text).map_err(|e| {
        Box::new(ErrorBody::new(
            ErrorCode::BadRequest,
            format!("can't read {file}: {e}"),
            "check the path; -F - reads the request lines from stdin",
        ))
    })
}
