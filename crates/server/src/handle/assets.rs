//! `assets` (design §12): images converted as a build converts them, through
//! `gbcli-build`. It runs no tool and needs no session, so both the
//! bundle-less [`super::build::Tooling`] and the [`super::Handler`] answer
//! it.

use gbcli_build::AssetsOptions;
use gbcli_protocol::{AssetsArgs, Response, ResultBody};

use super::host::Host;

/// Converts what `args` names, as `host` sees it.
pub(super) fn assets(host: &Host, id: String, args: &AssetsArgs) -> Response {
    let options = match AssetsOptions::from_args(args, host.cwd().to_path_buf()) {
        Ok(options) => options,
        Err(e) => return Response::failed(id, None, e.body()),
    };
    match gbcli_build::assets(&options) {
        Ok(made) => Response::ok(
            id,
            None,
            ResultBody {
                assets: Some(made),
                ..ResultBody::default()
            },
        ),
        Err(e) => Response::failed(id, None, e.body()),
    }
}

#[cfg(test)]
mod tests {
    use gbcli_protocol::{AssetManifest, AssetsArgs, Op, Response};

    use super::super::test_support::{handler, request};
    use crate::Tooling;
    use crate::fixture::{Fixture, bare_host};
    use crate::serve::Answer;

    fn empty_manifest() -> Op {
        Op::Assets(AssetsArgs {
            manifest: Some(AssetManifest::default()),
            ..AssetsArgs::default()
        })
    }

    fn ok(response: Response) -> gbcli_protocol::AssetsMade {
        match response.outcome {
            Ok(r) => r.assets.expect("an assets result"),
            Err(e) => panic!("{e:?}"),
        }
    }

    #[test]
    fn both_sides_answer_assets_and_a_handler_names_no_session() {
        let f = Fixture::new("assets");
        let mut h = handler(&f);
        let response = h.handle(&request(empty_manifest()));
        assert_eq!(response.rev, None, "assets names no session");
        let bare = Tooling::new(bare_host()).answer(&request(empty_manifest()));
        let made = ok(response);
        assert!(made.images.images.is_empty());
        assert_eq!(made, ok(bare));
    }

    #[test]
    fn a_project_and_a_manifest_together_are_refused() {
        let op = Op::Assets(AssetsArgs {
            dir: Some("game".into()),
            manifest: Some(AssetManifest::default()),
            ..AssetsArgs::default()
        });
        let response = Tooling::new(bare_host()).answer(&request(op));
        let body = response.outcome.expect_err("refused");
        assert_eq!(body.code, gbcli_protocol::ErrorCode::BadRequest);
        assert!(body.remedy.contains("manifest"), "{}", body.remedy);
    }
}
