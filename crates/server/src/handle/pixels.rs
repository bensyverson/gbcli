//! `pixels`: `gbcli-build`'s image check and conversion as an op. Needs no
//! session, so both the bundle-less [`super::build::Tooling`] and the
//! [`super::Handler`] answer it.

use gbcli_protocol::{PixelsArgs, Response, ResultBody};

use super::host::Host;

/// Reads and converts what `args` names, as `host` sees it.
pub(super) fn pixels(host: &Host, id: String, args: &PixelsArgs) -> Response {
    match gbcli_build::pixels(host.cwd(), args) {
        Ok(made) => Response::ok(
            id,
            None,
            ResultBody {
                pixels: Some(made),
                ..ResultBody::default()
            },
        ),
        Err(body) => Response::failed(id, None, *body),
    }
}

#[cfg(test)]
mod tests {
    use gbcli_protocol::{ErrorCode, Op, PixelsArgs, PixelsForm};

    use super::super::test_support::{handler, request};
    use crate::Tooling;
    use crate::fixture::{Fixture, bare_host};
    use crate::serve::Answer;

    fn op(file: &str) -> Op {
        Op::Pixels(PixelsArgs {
            file: file.into(),
            ..PixelsArgs::default()
        })
    }

    #[test]
    fn both_sides_answer_pixels_from_the_hosts_directory_with_no_session() {
        let f = Fixture::new("pixels");
        std::fs::write(f.dir.join("hero.pixels"), gbcli_pixels::EXAMPLE).unwrap();
        let host = || f.host();
        let mut h = handler(&f);
        let response = h.handle(&request(op("hero.pixels")));
        assert_eq!(response.rev, None, "pixels names no session");
        let made = response
            .outcome
            .expect("ok")
            .pixels
            .expect("a pixels result");
        assert_eq!(
            (made.form, made.width, made.height),
            (PixelsForm::Text, 32, 16)
        );
        let bare = Tooling::new(host()).answer(&request(op("hero.pixels")));
        assert_eq!(bare.outcome.expect("ok").pixels, Some(made));
    }

    #[test]
    fn a_missing_image_is_pixels_unreadable() {
        let bare = Tooling::new(bare_host()).answer(&request(op("no/such.pixels")));
        let body = bare.outcome.expect_err("missing");
        assert_eq!(body.code, ErrorCode::PixelsUnreadable);
    }
}
