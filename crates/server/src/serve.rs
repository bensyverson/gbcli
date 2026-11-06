//! The JSON Lines protocol over stdio (design §5): one request per line, one
//! final response per request, each flushed. Diagnostics never go to stdout.

use std::io::{BufRead, Write};

use gbcli_protocol::{ErrorBody, ErrorCode, Request, Response};

use crate::handle::Handler;

/// Serves requests from `reader` to `writer` until end of input. An error line
/// is answered with `bad_request`, and the server keeps running.
pub fn serve<R: BufRead, W: Write>(
    reader: R,
    writer: &mut W,
    handler: &mut Handler,
) -> std::io::Result<()> {
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let response = answer(handler, &line);
        let text = serde_json::to_string(&response).expect("a response always serializes");
        writeln!(writer, "{text}")?;
        writer.flush()?;
    }
    Ok(())
}

fn answer(handler: &mut Handler, line: &str) -> Response {
    match serde_json::from_str::<Request>(line) {
        Ok(request) => handler.handle(&request),
        Err(e) => {
            let id = serde_json::from_str::<serde_json::Value>(line)
                .ok()
                .and_then(|value| value.get("id")?.as_str().map(str::to_string))
                .unwrap_or_default();
            Response::failed(
                id,
                None,
                ErrorBody::new(
                    ErrorCode::BadRequest,
                    format!("not a request: {e}"),
                    "one JSON object per line; see `gbcli help schema`",
                ),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::Fixture;
    use std::io::Cursor;

    fn run_script(fixture: &Fixture, lines: &[&str]) -> Vec<Response> {
        let input = lines.join("\n") + "\n";
        let mut out = Vec::new();
        let mut handler = Handler::new(fixture.bundle.clone(), Some("demo".into()), None);
        serve(Cursor::new(input), &mut out, &mut handler).unwrap();
        String::from_utf8(out)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    #[test]
    fn a_blank_line_is_ignored_and_one_line_is_one_response() {
        let fixture = Fixture::new("serve-blank");
        let responses = run_script(&fixture, &["", "  ", r#"{"id":"1","op":"show"}"#]);
        assert_eq!(responses.len(), 1);
        assert!(responses[0].ok, "{:?}", responses[0]);
    }

    #[test]
    fn a_malformed_line_answers_bad_request_and_keeps_going() {
        let fixture = Fixture::new("serve-bad");
        let responses = run_script(&fixture, &["not json", r#"{"id":"2","op":"show"}"#]);
        assert_eq!(responses.len(), 2);
        assert_eq!(responses[0].id, "");
        assert_eq!(
            responses[0].error.as_ref().map(|e| e.code),
            Some(ErrorCode::BadRequest)
        );
        assert!(responses[1].ok);
    }

    #[test]
    fn a_request_naming_no_session_is_refused_with_a_remedy() {
        let fixture = Fixture::new("serve-nosession");
        let input = r#"{"id":"1","op":"show"}"#;
        let mut out = Vec::new();
        let mut handler = Handler::new(fixture.bundle.clone(), None, None);
        serve(Cursor::new(format!("{input}\n")), &mut out, &mut handler).unwrap();
        let response: Response = serde_json::from_slice(&out).unwrap();
        let error = response.error.expect("an error");
        assert_eq!(error.code, ErrorCode::NoSession);
        assert!(!error.remedy.is_empty());
    }
}
