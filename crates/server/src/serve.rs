//! The JSON Lines protocol over stdio (design §5): one request per line, one
//! final response per request, each flushed. Diagnostics never go to stdout.

use std::io::{BufRead, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};

use gbcli_protocol::{ErrorBody, ErrorCode, Request, Response, parse_request};

use crate::handle::Handler;

/// What answers the requests `serve` reads: the [`Handler`], or a stand-in.
pub trait Answer {
    fn answer(&mut self, request: &Request) -> Response;
    /// Called after `answer` panicked: drop any state the panic may have
    /// left half-changed.
    fn after_panic(&mut self);
}

impl Answer for Handler {
    fn answer(&mut self, request: &Request) -> Response {
        self.handle(request)
    }

    fn after_panic(&mut self) {
        self.forget_all_warm();
        self.disarm();
    }
}

/// Serves requests from `reader` to `writer` until end of input. A line that
/// isn't a request (bad JSON, not UTF-8) is answered with `bad_request`, a
/// request that panics with `internal`, and the server keeps running.
pub fn serve<R: BufRead, W: Write, A: Answer>(
    reader: R,
    writer: &mut W,
    handler: &mut A,
) -> std::io::Result<()> {
    for line in reader.split(b'\n') {
        let line = line?;
        let response = match std::str::from_utf8(&line) {
            Ok(text) if text.trim().is_empty() => continue,
            Ok(text) => answer_line(handler, text.trim_end_matches('\r')),
            Err(e) => {
                let body = ErrorBody::new(
                    ErrorCode::BadRequest,
                    format!("the line isn't UTF-8: {e}"),
                    "send one JSON object per line, encoded as UTF-8",
                );
                Response::failed(String::new(), None, body)
            }
        };
        let text = serde_json::to_string(&response).expect("a response always serializes");
        writeln!(writer, "{text}")?;
        writer.flush()?;
    }
    Ok(())
}

fn answer_line<A: Answer>(handler: &mut A, line: &str) -> Response {
    let request = match parse_request(line) {
        Ok(request) => request,
        Err(e) => {
            let body = ErrorBody::new(ErrorCode::BadRequest, e.message, e.remedy);
            return Response::failed(e.id.unwrap_or_default(), None, body);
        }
    };
    answer(handler, &request)
}

/// Answers one request as `serve` answers a line that parsed, under the same
/// panic guard: for a client in the same process (the CLI), with no JSON
/// round trip.
pub fn answer<A: Answer>(handler: &mut A, request: &Request) -> Response {
    // A panic is a bug, not a reason to end the session: answer it, and let
    // the handler drop what it held, since the panic may have left it half
    // changed. What's on disk was written atomically, so it stays trusted.
    match catch_unwind(AssertUnwindSafe(|| handler.answer(request))) {
        Ok(response) => response,
        Err(payload) => {
            handler.after_panic();
            let what = payload
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "a panic".into());
            Response::failed(
                request.id.clone(),
                None,
                ErrorBody::new(
                    ErrorCode::Internal,
                    format!("the request panicked: {what}"),
                    ErrorCode::Internal.default_remedy(),
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

    fn handler(fixture: &Fixture, session: Option<&str>) -> Handler {
        let watchdog = crate::Watchdog::start().expect("the wall guard's thread");
        Handler::new(fixture.host(), fixture.bundle.clone(), Box::new(watchdog))
            .with_defaults(session.map(str::to_string), None)
    }

    fn run_script(fixture: &Fixture, lines: &[&str]) -> Vec<Response> {
        let input = lines.join("\n") + "\n";
        let mut out = Vec::new();
        let mut handler = handler(fixture, Some("demo"));
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
        assert!(responses[0].is_ok(), "{:?}", responses[0]);
    }

    #[test]
    fn a_malformed_line_answers_bad_request_and_keeps_going() {
        let fixture = Fixture::new("serve-bad");
        let responses = run_script(&fixture, &["not json", r#"{"id":"2","op":"show"}"#]);
        assert_eq!(responses.len(), 2);
        assert_eq!(responses[0].id, "");
        assert_eq!(
            responses[0].error().map(|e| e.code),
            Some(ErrorCode::BadRequest)
        );
        assert!(responses[1].is_ok());
    }

    #[test]
    fn an_unknown_field_answers_bad_request_naming_it_and_the_nearest() {
        let fixture = Fixture::new("serve-unknown");
        let responses = run_script(
            &fixture,
            &[
                r#"{"id":"1","revv":"s0","op":"show"}"#,
                r#"{"id":"2","op":"run","args":{"frame":10}}"#,
            ],
        );
        for (response, (field, near)) in
            responses.iter().zip([("revv", "rev"), ("frame", "frames")])
        {
            let error = response.error().expect("refused");
            assert_eq!(error.code, ErrorCode::BadRequest);
            assert!(error.message.contains(field), "{}", error.message);
            assert!(
                error.remedy.contains(&format!("`{near}`")),
                "{}",
                error.remedy
            );
        }
        assert_eq!(responses[0].id, "1", "the id is echoed");
        let rev = fixture
            .bundle
            .session("demo")
            .unwrap()
            .read_meta()
            .unwrap()
            .rev;
        assert_eq!(rev, gbcli_protocol::Rev(0), "the misspelled run ran");
    }

    #[test]
    fn a_line_that_is_not_utf8_answers_bad_request_and_keeps_going() {
        let fixture = Fixture::new("serve-utf8");
        let mut input = b"{\"id\":\"1\",\"op\":\"sh\xFFow\"}\n".to_vec();
        input.extend_from_slice(b"{\"id\":\"2\",\"op\":\"show\"}\n");
        let mut out = Vec::new();
        let mut handler = handler(&fixture, Some("demo"));
        serve(Cursor::new(input), &mut out, &mut handler).unwrap();
        let responses: Vec<Response> = String::from_utf8(out)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(responses.len(), 2);
        assert_eq!(
            responses[0].error().map(|e| e.code),
            Some(ErrorCode::BadRequest)
        );
        assert!(responses[1].is_ok(), "{:?}", responses[1]);
    }

    /// Panics on the request whose id is "boom".
    struct Fragile {
        resets: usize,
    }

    impl Answer for Fragile {
        fn answer(&mut self, request: &Request) -> Response {
            assert_ne!(request.id, "boom", "injected panic");
            Response::ok(request.id.clone(), None, Default::default())
        }

        fn after_panic(&mut self) {
            self.resets += 1;
        }
    }

    #[test]
    fn a_panic_in_one_request_is_an_error_response_and_the_loop_goes_on() {
        let input = concat!(
            r#"{"id":"boom","op":"show"}"#,
            "\n",
            r#"{"id":"2","op":"show"}"#,
            "\n"
        );
        let mut out = Vec::new();
        let mut fragile = Fragile { resets: 0 };
        serve(Cursor::new(input), &mut out, &mut fragile).unwrap();
        let responses: Vec<Response> = String::from_utf8(out)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(responses.len(), 2);
        assert_eq!(responses[0].id, "boom");
        assert_eq!(
            responses[0].error().map(|e| e.code),
            Some(ErrorCode::Internal)
        );
        assert!(responses[1].is_ok());
        assert_eq!(fragile.resets, 1, "the handler drops what it can't trust");
    }

    #[test]
    fn a_request_answered_in_process_gets_the_same_panic_guard() {
        let mut fragile = Fragile { resets: 0 };
        let boom = gbcli_protocol::parse_request(r#"{"id":"boom","op":"show"}"#).unwrap();
        let response = answer(&mut fragile, &boom);
        assert_eq!(response.id, "boom");
        assert_eq!(response.error().map(|e| e.code), Some(ErrorCode::Internal));
        assert_eq!(fragile.resets, 1);
        let fine = gbcli_protocol::parse_request(r#"{"id":"2","op":"show"}"#).unwrap();
        assert!(answer(&mut fragile, &fine).is_ok());
    }

    #[test]
    fn a_request_naming_no_session_is_refused_with_a_remedy() {
        let fixture = Fixture::new("serve-nosession");
        let input = r#"{"id":"1","op":"show"}"#;
        let mut out = Vec::new();
        let mut handler = handler(&fixture, None);
        serve(Cursor::new(format!("{input}\n")), &mut out, &mut handler).unwrap();
        let response: Response = serde_json::from_slice(&out).unwrap();
        let error = response.outcome.expect_err("an error");
        assert_eq!(error.code, ErrorCode::NoSession);
        assert!(!error.remedy.is_empty());
    }
}
