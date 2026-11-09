//! The onion plugin: `GET /api/onion/`, the admin app's document for this
//! box's Tor and I2P addresses.
//!
//! The box's `.onion` and `.b32.i2p` addresses deliver to Traefik, which
//! routes the admin API to lososd's second loopback listener
//! (`LOSOS_ONION_PORT`, `modules/daemon.nix`) and everything that is not the
//! app's own page — the hashed assets, `/theme-boot.js`, LosOS Cloud, LosOS
//! Git, the handbook — to the front vhost's onion listener
//! (`modules/apps.nix`, `modules/hidden-services.nix`). What is left, the
//! app's document for every path the app's router knows, is this plugin's:
//! Traefik hands the path through, the listener's catch-all calls here, and
//! the answer is the app bundle's `index.html`. The browser renders the rest
//! — the same trade part A of the design makes everywhere else.
//!
//! The plugin never names a caller: it reads one file from the app bundle
//! and hands it out. Who may load it is the listener's and Traefik's
//! business (the forwardAuth middleware, LosOS Login), never this one's.
//!
//! The route is a prefix index (`/api/onion/`, the manifest's one route)
//! because the paths a visitor asks for — `/`, `/storage`,
//! `/settings/network` — are the app's own routes, not this plugin's; the
//! host rewrites them onto the prefix when it dispatches, the way nginx
//! rewrote `/basic/` onto `/api/basic/`.

use losos_plugin::{config, fs, Plugin, Request, Response};
/// The app's document: `index.html` out of the app bundle
/// (`LOSOS_ADMIN_UI_DIR`, modules/daemon.nix). A bundle that is not there
/// (a box with no admin UI, or a dev run without the variable) is a 404, the
/// sentence saying which.
#[must_use]
pub fn document(ui: &str, index: Option<Vec<u8>>) -> Response {
    if ui.is_empty() {
        return Response::error(404, "this box has no admin app");
    }
    match index {
        Some(html) => Response::bytes(200, "text/html; charset=utf-8", html),
        None => Response::error(404, "the admin app's document is missing on this box"),
    }
}

/// `index.html` out of the app bundle named by the manifest's `ui` config.
fn read_document() -> Option<Vec<u8>> {
    let dir = config::get_or("ui", "");
    if dir.is_empty() {
        return None;
    }
    let path = losos_plugin::fs::join(&dir, "index.html");
    match fs::read_optional(&path) {
        Ok(bytes) => bytes,
        Err(e) => {
            losos_plugin::log::warn(&format!("cannot read {path}: {e}"));
            None
        }
    }
}

pub struct OnionPlugin;

impl Plugin for OnionPlugin {
    fn handle(req: Request) -> Response {
        match (req.method.as_str(), req.route.as_str()) {
            ("GET", "/api/onion/") => document(&config::get_or("ui", ""), read_document()),
            _ => Response::error(404, "not found"),
        }
    }
}

losos_plugin::export!(OnionPlugin);

#[cfg(test)]
mod tests {
    use super::*;

    const HTML: &[u8] = b"<!doctype html><html><body><div id=root></div></body></html>";

    #[test]
    fn the_document_is_the_bundle_index_html() {
        let r = document("/nix/store/…-losos-admin-ui", Some(HTML.to_vec()));
        assert_eq!(r.status, 200);
        assert_eq!(r.body, HTML);
        let type_ = r
            .headers
            .iter()
            .find(|(k, _)| k == "content-type")
            .map(|(_, v)| v.as_str());
        assert_eq!(type_, Some("text/html; charset=utf-8"));
    }

    #[test]
    fn a_bundle_without_an_index_is_a_404_and_so_is_no_bundle() {
        let r = document("/nix/store/…-losos-admin-ui", None);
        assert_eq!(r.status, 404);
        assert!(r.json_value()["error"].is_string());
        let r = document("", Some(HTML.to_vec()));
        assert_eq!(r.status, 404);
    }
}
