//! Service discovery for Traefik: the registrar's second listener.
//!
//! Traefik's HTTP provider polls `GET /traefik` here every few seconds and
//! routes exactly what it is told: one router per live box name, one per
//! published machine, each to its loopback upstream. The document is the
//! last reconcile pass's [`Routes`], held in memory ([`Discovery`]); nothing
//! is written to Traefik's directory. Traefik hashes every answer and only
//! reloads when it changes, and on any answer other than 200 it keeps the
//! routes it had, so a registrar that is restarting costs no box its route.
//!
//! The same listener serves `/offline/<status>`, the page Traefik's `errors`
//! middleware shows in place of the 502 or 504 it would answer for a box it
//! cannot reach (`config::OFFLINE_MIDDLEWARE`).
//!
//! The listener is loopback-only (`--traefik-provider-listen`, 127.0.0.1 in
//! `modules/edge.nix`), and on purpose not the API's: the API is behind
//! Traefik's public `register.<domain>` router with no path rule, and on a
//! LAN gateway it is bound off-box, so a route there would hand anyone the
//! list of every name this edge serves.

use std::sync::{Arc, RwLock};

use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;

use crate::config::{provider_document, Routes, OFFLINE_STATUSES};

/// What Traefik is told, by source. `None` until that source has been
/// decided once, so the first answer after a restart is never a document
/// that forgot the boxes.
#[derive(Default)]
struct Sources {
    tenants: Option<Routes>,
    vms: Routes,
    /// The boxes' Tor and I2P names (`crate::hidden`).
    hidden: Routes,
    /// Every public name of each live box, by tenant id, as the last
    /// reconcile pass decided: what a box that ends TLS itself is told to
    /// get certificates for (`crate::passthrough`).
    names: std::collections::BTreeMap<String, Vec<String>>,
    /// The provider listener's own origin, once bound: the offline page's
    /// service. `None` (no listener, as on the Vercel host) leaves the
    /// routers without it.
    offline_url: Option<String>,
}

/// The routes the registrar currently publishes. Cheap to clone; every clone
/// sees the same state.
#[derive(Clone, Default)]
pub struct Discovery(Arc<RwLock<Sources>>);

impl Discovery {
    pub(super) fn set_tenants(&self, routes: Routes) -> bool {
        let mut s = self.0.write().unwrap_or_else(|p| p.into_inner());
        let changed = s.tenants.as_ref() != Some(&routes);
        s.tenants = Some(routes);
        changed
    }

    pub(super) fn set_names(&self, names: std::collections::BTreeMap<String, Vec<String>>) {
        self.0.write().unwrap_or_else(|p| p.into_inner()).names = names;
    }

    /// The public names of box `id`, sorted; empty for one this edge does
    /// not route.
    #[must_use]
    pub fn names_of(&self, id: &str) -> Vec<String> {
        self.0
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .names
            .get(id)
            .cloned()
            .unwrap_or_default()
    }

    pub(super) fn set_vms(&self, routes: Routes) -> bool {
        let mut s = self.0.write().unwrap_or_else(|p| p.into_inner());
        let changed = s.vms != routes;
        s.vms = routes;
        changed
    }

    pub(super) fn set_hidden(&self, routes: Routes) -> bool {
        let mut s = self.0.write().unwrap_or_else(|p| p.into_inner());
        let changed = s.hidden != routes;
        s.hidden = routes;
        changed
    }

    pub(super) fn set_offline_url(&self, url: String) {
        self.0
            .write()
            .unwrap_or_else(|p| p.into_inner())
            .offline_url = Some(url);
    }

    /// The provider document as Traefik reads it, or `None` before the first
    /// reconcile pass.
    #[must_use]
    pub fn document(&self) -> Option<String> {
        let s = self.0.read().unwrap_or_else(|p| p.into_inner());
        let tenants = s.tenants.as_ref()?;
        // Only the boxes get the offline page: a machine is not a box.
        let boxes = match &s.offline_url {
            Some(url) => tenants.with_offline_page(url),
            None => tenants.clone(),
        };
        Some(provider_document([&boxes, &s.vms, &s.hidden]))
    }

    /// The names the boxes are routed at, sorted (machines and Tor and I2P
    /// names excluded).
    #[must_use]
    pub fn hosts(&self) -> Vec<String> {
        let s = self.0.read().unwrap_or_else(|p| p.into_inner());
        s.tenants.as_ref().map(Routes::hosts).unwrap_or_default()
    }

    /// Each service's address, boxes and machines alike
    /// ([`Routes::hosts_by_service`]).
    #[must_use]
    pub fn hosts_by_service(&self) -> std::collections::BTreeMap<String, String> {
        let s = self.0.read().unwrap_or_else(|p| p.into_inner());
        let mut hosts = s.vms.hosts_by_service();
        if let Some(tenants) = &s.tenants {
            hosts.extend(tenants.hosts_by_service());
        }
        hosts
    }
}

/// The provider listener's routes.
pub(super) fn router(discovery: Discovery) -> Router {
    Router::new()
        .route("/traefik", get(document))
        .route("/offline/{status}", get(offline))
        .with_state(discovery)
}

async fn document(State(d): State<Discovery>) -> Response {
    match d.document() {
        Some(doc) => (
            [(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            )],
            doc,
        )
            .into_response(),
        // Traefik keeps its routes on anything but 200.
        None => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}

/// The page for a box Traefik could not reach. Traefik sends the visitor the
/// original status with this body; the status here is the same for anyone
/// who reaches the listener directly.
async fn offline(Path(status): Path<String>, headers: HeaderMap) -> Response {
    let status = OFFLINE_STATUSES
        .iter()
        .find(|s| **s == status)
        .copied()
        .unwrap_or("502");
    let code = status
        .parse::<u16>()
        .ok()
        .and_then(|c| StatusCode::from_u16(c).ok())
        .unwrap_or(StatusCode::BAD_GATEWAY);
    // Traefik passes the visitor's Host on. A name that is not a plain DNS
    // name is left out rather than escaped into the page.
    let host = headers
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .map(|h| h.split(':').next().unwrap_or(h).to_ascii_lowercase())
        .filter(|h| crate::relay::dns_name(h));
    let lang = Lang::from_headers(&headers);
    (
        code,
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static("text/html; charset=utf-8"),
            ),
            (header::CACHE_CONTROL, HeaderValue::from_static("no-store")),
            (
                header::CONTENT_SECURITY_POLICY,
                HeaderValue::from_static(
                    "default-src 'none'; style-src 'unsafe-inline'; img-src data:",
                ),
            ),
        ],
        offline_page(lang, host.as_deref(), status),
    )
        .into_response()
}

/// The page's three languages, the handbook's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Lang {
    En,
    Sk,
    De,
}

impl Lang {
    /// The first of Slovak, German and English in `Accept-Language`, in the
    /// visitor's order; English otherwise.
    pub(super) fn from_headers(headers: &HeaderMap) -> Self {
        let Some(accept) = headers
            .get(header::ACCEPT_LANGUAGE)
            .and_then(|h| h.to_str().ok())
        else {
            return Self::En;
        };
        for tag in accept.split(',') {
            let tag = tag
                .split(';')
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase();
            match tag.split('-').next().unwrap_or("") {
                "sk" => return Self::Sk,
                "de" => return Self::De,
                "en" => return Self::En,
                _ => {}
            }
        }
        Self::En
    }

    pub(super) fn code(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Sk => "sk",
            Self::De => "de",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::En => "This box can’t be reached right now",
            Self::Sk => "Tento box je teraz nedostupný",
            Self::De => "Diese Box ist gerade nicht erreichbar",
        }
    }

    /// The sentence naming the box, or the box in general without a name.
    fn lead(self, host: Option<&str>) -> String {
        match (self, host) {
            (Self::En, Some(h)) => {
                format!("<strong>{h}</strong> is a LosOS box, and it isn’t answering.")
            }
            (Self::En, None) => "The LosOS box at this address isn’t answering.".to_string(),
            (Self::Sk, Some(h)) => format!("<strong>{h}</strong> je box LosOS a práve neodpovedá."),
            (Self::Sk, None) => "Box LosOS na tejto adrese práve neodpovedá.".to_string(),
            (Self::De, Some(h)) => {
                format!("<strong>{h}</strong> ist eine LosOS-Box und antwortet gerade nicht.")
            }
            (Self::De, None) => {
                "Die LosOS-Box unter dieser Adresse antwortet gerade nicht.".to_string()
            }
        }
    }

    fn why(self) -> &'static str {
        match self {
            Self::En => "It may be switched off, restarting or without internet. Boxes restart for a minute each night, so try again in a few minutes.",
            Self::Sk => "Môže byť vypnutý, práve sa reštartovať alebo byť bez internetu. Boxy sa každú noc na minútu reštartujú, skúste to preto znova o pár minút.",
            Self::De => "Sie ist vielleicht ausgeschaltet, startet gerade neu oder hat kein Internet. Boxen starten jede Nacht für eine Minute neu, versuch es also in ein paar Minuten noch einmal.",
        }
    }

    fn again(self) -> &'static str {
        match self {
            Self::En => "Try again",
            Self::Sk => "Skúsiť znova",
            Self::De => "Erneut versuchen",
        }
    }
}

/// The page itself: no script, no request of its own (the mark is inline),
/// light and dark from the visitor's setting, the palette of
/// `admin-ui/app/src/styles/tokens.css`.
fn offline_page(lang: Lang, host: Option<&str>, status: &str) -> String {
    format!(
        r#"<!doctype html>
<html lang="{code}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="robots" content="noindex">
<title>{title}</title>
<style>
:root {{ color-scheme: light dark; --ground: #f3f5f7; --surface: #ffffff; --ink: #18212a; --muted: #5c6a77; --line: #dce1e6; --accent: #0e6e7d; --accent-ink: #ffffff; }}
@media (prefers-color-scheme: dark) {{ :root {{ --ground: #0a0e12; --surface: #141b22; --ink: #e2e8ee; --muted: #94a2b0; --line: #212c36; --accent: #48b3c0; --accent-ink: #0a0e12; }} }}
* {{ box-sizing: border-box; }}
body {{ margin: 0; min-height: 100vh; display: grid; place-items: center; padding: 16px; background: var(--ground); color: var(--ink); font: 16px/1.55 system-ui, -apple-system, "Segoe UI", Roboto, sans-serif; }}
main {{ width: 100%; max-width: 30rem; background: var(--surface); border: 1px solid var(--line); border-radius: 14px; padding: 32px 28px; }}
svg {{ display: block; margin-bottom: 20px; color: var(--accent); }}
h1 {{ font-size: 1.3rem; line-height: 1.3; font-weight: 600; margin: 0 0 12px; }}
p {{ margin: 0 0 12px; }}
.why {{ color: var(--muted); }}
strong {{ font-weight: 600; overflow-wrap: anywhere; }}
a {{ display: inline-block; margin-top: 12px; padding: 9px 16px; border-radius: 9px; background: var(--accent); color: var(--accent-ink); text-decoration: none; font-weight: 500; }}
a:focus-visible {{ outline: 2px solid var(--accent); outline-offset: 3px; }}
footer {{ margin-top: 24px; font-size: 0.8rem; color: var(--muted); }}
</style>
</head>
<body>
<main>
<svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="3" y="4" width="18" height="12" rx="2"/><path d="M8 20h8M12 16v4"/><path d="M9.5 8.5l5 3M14.5 8.5l-5 3"/></svg>
<h1>{title}</h1>
<p>{lead}</p>
<p class="why">{why}</p>
<a href="">{again}</a>
<footer>{status} · LosOS edge</footer>
</main>
</body>
</html>
"#,
        code = lang.code(),
        title = lang.title(),
        lead = lang.lead(host),
        why = lang.why(),
        again = lang.again(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_language_follows_the_visitors_order() {
        let mut h = HeaderMap::new();
        assert_eq!(Lang::from_headers(&h), Lang::En);
        h.insert(
            header::ACCEPT_LANGUAGE,
            HeaderValue::from_static("fr-FR, de-AT;q=0.8, sk;q=0.5"),
        );
        assert_eq!(Lang::from_headers(&h), Lang::De);
        h.insert(
            header::ACCEPT_LANGUAGE,
            HeaderValue::from_static("sk-SK,sk;q=0.9,en;q=0.8"),
        );
        assert_eq!(Lang::from_headers(&h), Lang::Sk);
        h.insert(header::ACCEPT_LANGUAGE, HeaderValue::from_static("ja"));
        assert_eq!(Lang::from_headers(&h), Lang::En);
    }

    #[test]
    fn nothing_is_published_before_the_first_pass() {
        let d = Discovery::default();
        assert_eq!(d.document(), None);
        d.set_tenants(Routes::default());
        assert_eq!(d.document().as_deref(), Some("{}"));
    }

    #[test]
    fn the_page_names_a_plain_host_only() {
        let page = offline_page(Lang::En, Some("mattbox.losos.cfd"), "502");
        assert!(page.contains("<strong>mattbox.losos.cfd</strong>"));
        assert!(page.contains("502 · LosOS edge"));
        let page = offline_page(Lang::En, None, "504");
        assert!(page.contains("at this address"));
    }
}
