//! `font`: `gbcli-build`'s font reader, sample and preview as an op. Needs
//! no session, so both the bundle-less [`super::build::Tooling`] and the
//! [`super::Handler`] answer it.

use gbcli_protocol::{FontArgs, Response, ResultBody};

use super::build::Host;

/// Reads the font `args` names, as `host` sees it.
pub(super) fn font(host: &Host, id: String, args: &FontArgs) -> Response {
    match gbcli_build::font(&host.cwd, args) {
        Ok(made) => Response::ok(
            id,
            None,
            ResultBody {
                font: Some(made),
                ..ResultBody::default()
            },
        ),
        Err(body) => Response::failed(id, None, *body),
    }
}

#[cfg(test)]
mod tests {
    use gbcli_protocol::{ErrorCode, FontArgs, Op};

    use super::super::test_support::{handler, request};
    use crate::fixture::Fixture;
    use crate::serve::Answer;
    use crate::{Host, Tooling};

    fn op(file: &str) -> Op {
        Op::Font(FontArgs {
            file: file.into(),
            sample: Some("ii".into()),
            ..FontArgs::default()
        })
    }

    #[test]
    fn both_sides_answer_font_from_the_hosts_directory_with_no_session() {
        let f = Fixture::new("font");
        let rows = "    @.\n".repeat(8);
        std::fs::write(f.dir.join("main.yaff"), format!("'i':\n{rows}")).unwrap();
        let host = || Host {
            cwd: f.dir.clone(),
            ..Host::from_process()
        };
        let mut h = handler(&f);
        h.set_host(host());
        let response = h.handle(&request(op("main.yaff")));
        assert_eq!(response.rev, None, "font names no session");
        let made = response.outcome.expect("ok").font.expect("a font result");
        assert_eq!((made.glyphs, made.sample.as_ref().unwrap().width), (1, 4));
        let bare = Tooling::new(host()).answer(&request(op("main.yaff")));
        assert_eq!(bare.outcome.expect("ok").font, Some(made));
    }

    #[test]
    fn a_missing_font_is_font_unreadable() {
        let bare = Tooling::new(Host::from_process()).answer(&request(op("no/such.yaff")));
        assert_eq!(
            bare.outcome.expect_err("missing").code,
            ErrorCode::FontUnreadable
        );
    }
}
