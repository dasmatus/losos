//! The ci plugin: `/api/ci/*`, the admin pages' way to Mesh CI.
//!
//! Every route goes on to `losos-ci`'s page socket as `/ci/*`, with who is
//! asking in two headers: `X-Losos-Person` (the LosOS Login name, empty
//! for the spare key) and `X-Losos-Admin`. `losos-ci` decides per
//! repository what that person may see and do, from LosOS Git's own
//! answer. While the box does not share or is in no mesh there is no
//! `losos-ci` and every route answers 404.

use losos_plugin::{config, http, Plugin, Request, Response};

/// The most a page asks for at once: a log is read in pieces.
const MAX_ANSWER: u64 = 8 * 1024 * 1024;

pub struct CiPlugin;

/// The headers `losos-ci` reads, for this caller.
pub fn who(req: &Request) -> (String, bool) {
    let name = req
        .caller
        .person()
        .map(|p| p.name.clone())
        .unwrap_or_default();
    (name, req.caller.is_admin())
}

/// `/api/ci/...?q` to `/ci/...?q`.
pub fn target(path: &str, query: &str) -> Option<String> {
    let rest = path.strip_prefix("/api")?;
    if !(rest == "/ci" || rest.starts_with("/ci/")) {
        return None;
    }
    Some(if query.is_empty() {
        rest.to_string()
    } else {
        format!("{rest}?{query}")
    })
}

impl Plugin for CiPlugin {
    fn handle(req: Request) -> Response {
        let Some(target) = target(&req.path, &req.query) else {
            return Response::error(404, "not found");
        };
        let socket = config::get_or("socket", "/run/losos-ci/api.sock");
        let (name, admin) = who(&req);
        let mut out = http::Request::new(&req.method, &format!("unix:{socket}{target}"))
            .header("x-losos-person", &name)
            .header("x-losos-admin", if admin { "1" } else { "0" })
            .timeout_ms(55_000);
        if !req.body.is_empty() {
            out = out
                .header("content-type", "application/json")
                .body(req.body.clone());
        }
        match out.fetch(MAX_ANSWER) {
            Ok(answer) => {
                let kind = answer
                    .header("content-type")
                    .unwrap_or("application/json")
                    .to_string();
                Response::bytes(answer.status, &kind, answer.body)
            }
            // No socket: the service is not there, because the box is not
            // on the shared side.
            Err(e) => {
                losos_plugin::log::info(&format!("losos-ci did not answer: {e}"));
                Response::error(
                    404,
                    "Mesh CI runs only while this box shares its storage and is in a mesh.",
                )
            }
        }
    }
}

losos_plugin::export!(CiPlugin);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_map_onto_the_page_socket() {
        assert_eq!(target("/api/ci", ""), Some("/ci".into()));
        assert_eq!(
            target("/api/ci/runs", "owner=a&repo=b"),
            Some("/ci/runs?owner=a&repo=b".into())
        );
        assert_eq!(target("/api/cix", ""), None);
        assert_eq!(target("/api/pins", ""), None);
    }
}
