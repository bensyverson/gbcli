//! `song`: `gbcli-build`'s song conversion as an op, a song file's RGBDS
//! source for hUGEDriver. Needs no session, so both the bundle-less
//! [`super::build::Tooling`] and the [`super::Handler`] answer it.

use gbcli_protocol::{Response, ResultBody, SongArgs};

use super::host::Host;

/// Reads and converts the song `args` names, as `host` sees it.
pub(super) fn song(host: &Host, id: String, args: &SongArgs) -> Response {
    match gbcli_build::song(host.cwd(), args) {
        Ok(made) => Response::ok(
            id,
            None,
            ResultBody {
                song_asm: Some(made),
                ..ResultBody::default()
            },
        ),
        Err(body) => Response::failed(id, None, *body),
    }
}

#[cfg(test)]
mod tests {
    use gbcli_protocol::{ErrorCode, Op, SongArgs};

    use super::super::test_support::{handler, request};
    use crate::Tooling;
    use crate::fixture::{Fixture, bare_host};
    use crate::serve::Answer;

    fn op(path: &str) -> Op {
        Op::Song(SongArgs {
            path: path.into(),
            descriptor: "song_theme".into(),
        })
    }

    #[test]
    fn both_sides_answer_song_from_the_hosts_directory_with_no_session() {
        let f = Fixture::new("song");
        std::fs::write(f.dir.join("theme.song"), gbcli_music::EXAMPLE).unwrap();
        let mut h = handler(&f);
        let response = h.handle(&request(op("theme.song")));
        assert_eq!(response.rev, None, "song names no session");
        let made = response
            .outcome
            .expect("ok")
            .song_asm
            .expect("a song_asm result");
        let want = gbcli_music::rgbds_asm(
            &gbcli_music::parse_text(gbcli_music::EXAMPLE).unwrap(),
            "song_theme",
        );
        assert_eq!(made.asm.as_bytes(), want);
        let bare = Tooling::new(f.host()).answer(&request(op("theme.song")));
        assert_eq!(bare.outcome.expect("ok").song_asm, Some(made));
    }

    #[test]
    fn a_missing_song_is_song_unreadable() {
        let bare = Tooling::new(bare_host()).answer(&request(op("no/such.uge")));
        let body = bare.outcome.expect_err("missing");
        assert_eq!(body.code, ErrorCode::SongUnreadable);
    }
}
