//! `version`: gbcli's version, the protocol's and the core's. Needs no
//! session, so both the bundle-less [`super::build::Tooling`] and the
//! [`super::Handler`] answer it.

use gbcli_protocol::{PROTOCOL_VERSION, Response, ResultBody, VersionMade};

pub(super) fn version(id: String) -> Response {
    Response::ok(
        id,
        None,
        ResultBody {
            version: Some(VersionMade {
                gbcli: env!("CARGO_PKG_VERSION").into(),
                protocol: PROTOCOL_VERSION,
                sameboy: sameboy_sys::SAMEBOY_COMMIT.into(),
                patch_set: sameboy_sys::PATCH_SET.into(),
            }),
            ..ResultBody::default()
        },
    )
}

#[cfg(test)]
mod tests {
    use gbcli_protocol::{Op, PROTOCOL_VERSION, Request, Response, VersionMade};

    use super::super::test_support::{handler, request};
    use crate::fixture::Fixture;
    use crate::serve::Answer;
    use crate::{Host, Tooling};

    fn made(response: Response) -> VersionMade {
        response.outcome.expect("ok").version.expect("a version")
    }

    fn version_request() -> Request {
        request(Op::Version)
    }

    #[test]
    fn version_answers_without_a_bundle() {
        let v = made(Tooling::new(Host::from_process()).answer(&version_request()));
        assert_eq!(v.protocol, PROTOCOL_VERSION);
        assert_eq!(v.gbcli, env!("CARGO_PKG_VERSION"));
        assert_eq!(v.sameboy, sameboy_sys::SAMEBOY_COMMIT);
        assert_eq!(v.patch_set, sameboy_sys::PATCH_SET);
    }

    #[test]
    fn a_handler_answers_it_the_same_and_ignores_its_session() {
        let f = Fixture::new("version");
        let mut h = handler(&f);
        let response = h.handle(&version_request());
        assert_eq!(response.rev, None, "a version names no session");
        let bare = Tooling::new(Host::from_process()).answer(&version_request());
        assert_eq!(made(response), made(bare));
    }
}
