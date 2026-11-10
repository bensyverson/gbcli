//! `font`: `gbcli-build`'s font reader, sample and preview as an op, and
//! the authoring ops (`font_edit`, `font_save`, `font_import`,
//! `font_export`, `font_new`, `font_diff`). None needs a session, so both
//! the bundle-less [`super::build::Tooling`] and the [`super::Handler`]
//! answer them.

use gbcli_protocol::{ErrorBody, FontArgs, FontWritten, Op, Response, ResultBody};

use super::host::Host;

/// Reads the font `args` names, as `host` sees it.
pub(super) fn font(host: &Host, id: String, args: &FontArgs) -> Response {
    match gbcli_build::font(host.cwd(), args) {
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

/// Whether `op` is a font authoring op, which [`author`] answers.
pub(super) fn is_authoring(op: &Op) -> bool {
    matches!(
        op,
        Op::FontEdit(_)
            | Op::FontSave(_)
            | Op::FontImport(_)
            | Op::FontExport(_)
            | Op::FontNew(_)
            | Op::FontDiff(_)
    )
}

/// A font authoring op, from `host`'s working directory.
pub(super) fn author(host: &Host, id: String, op: &Op) -> Response {
    let cwd = host.cwd();
    let written = |r: Result<FontWritten, Box<ErrorBody>>| {
        r.map(|w| ResultBody {
            font_written: Some(w),
            ..ResultBody::default()
        })
    };
    let result = match op {
        Op::FontEdit(a) => written(gbcli_build::font_edit(cwd, a)),
        Op::FontSave(a) => written(gbcli_build::font_save(cwd, a)),
        Op::FontImport(a) => written(gbcli_build::font_import(cwd, a)),
        Op::FontExport(a) => written(gbcli_build::font_export(cwd, a)),
        Op::FontNew(a) => written(gbcli_build::font_new(cwd, a)),
        Op::FontDiff(a) => gbcli_build::font_diff(cwd, a).map(|d| ResultBody {
            font_diff: Some(d),
            ..ResultBody::default()
        }),
        op => unreachable!("{} isn't a font authoring op", op.name()),
    };
    match result {
        Ok(body) => Response::ok(id, None, body),
        Err(body) => Response::failed(id, None, *body),
    }
}

#[cfg(test)]
mod tests {
    use gbcli_protocol::{ErrorCode, FontArgs, Op};

    use super::super::test_support::{handler, request};
    use crate::Tooling;
    use crate::fixture::{Fixture, bare_host};
    use crate::serve::Answer;

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
        let host = || f.host();
        let mut h = handler(&f);
        let response = h.handle(&request(op("main.yaff")));
        assert_eq!(response.rev, None, "font names no session");
        let made = response.outcome.expect("ok").font.expect("a font result");
        assert_eq!((made.glyphs, made.sample.as_ref().unwrap().width), (1, 4));
        let bare = Tooling::new(host()).answer(&request(op("main.yaff")));
        assert_eq!(bare.outcome.expect("ok").font, Some(made));
    }

    #[test]
    fn both_sides_answer_the_authoring_ops_with_no_session() {
        let f = Fixture::new("font-author");
        let rows = "    @.\n".repeat(8);
        std::fs::write(f.dir.join("main.yaff"), format!("'i':\n{rows}")).unwrap();
        let host = || f.host();
        let diff = Op::FontDiff(gbcli_protocol::FontDiffArgs {
            a: "main.yaff".into(),
            b: "main.yaff".into(),
            png: None,
        });
        let mut h = handler(&f);
        let made = h.handle(&request(diff.clone())).outcome.expect("ok");
        assert_eq!(made.font_diff.expect("a diff").glyphs, vec![]);
        let edit = Op::FontEdit(gbcli_protocol::FontEditArgs {
            file: "main.yaff".into(),
            form: gbcli_protocol::FontForm::Bdf,
        });
        let bare = Tooling::new(host()).answer(&request(edit));
        let written = bare.outcome.expect("ok").font_written.expect("written");
        assert_eq!(written.files[0].path, "build/font-edit/main.bdf");
    }

    #[test]
    fn a_missing_font_is_font_unreadable() {
        let bare = Tooling::new(bare_host()).answer(&request(op("no/such.yaff")));
        assert_eq!(
            bare.outcome.expect_err("missing").code,
            ErrorCode::FontUnreadable
        );
    }
}
