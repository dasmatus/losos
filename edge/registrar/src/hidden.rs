//! Tor and I2P addresses for the boxes this edge routes
//! (`losos.edge.hidden.*`, `modules/edge-hidden.nix`).
//!
//! The registrar keeps no Tor or I2P state. On every reconcile it writes the
//! list of live boxes to `<dir>/services.json`; `losos-hidden-sync`
//! (`modules/hidden-lib.nix`) turns that into an onion service and an I2P
//! server tunnel per box, reloads the daemons and writes the addresses they
//! made to `addresses.json`. The registrar reads that back to route each
//! address to its box ([`crate::config::hidden_routes_yaml`]) and to hand a
//! box its own addresses on its heartbeat.
//!
//! Every service points at the same two loopback ports, Traefik's
//! `losos-hidden` and `losos-hidden-tls` entry points: the Host header the
//! visitor's browser sends picks the box, and only routers for these
//! addresses listen there. So nobody on the internet can ask the public
//! entry points for an onion name, which would tell them which edge serves
//! it, and nobody on Tor can reach a box by another box's name.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// The file the registrar writes in `--hidden-dir`.
pub const SERVICES_FILE: &str = "services.json";
/// Traefik's entry points for what Tor and I2P deliver (`modules/edge.nix`).
pub const PLAIN_ENTRY_POINT: &str = "losos-hidden";
pub const TLS_ENTRY_POINT: &str = "losos-hidden-tls";

/// `serve --hidden-dir DIR [--hidden-addresses FILE] [--hidden-http-port N]
/// [--hidden-https-port N]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HiddenOpts {
    pub dir: String,
    pub addresses_file: String,
    /// Where Tor and I2P deliver port 80 of an address: Traefik's
    /// `losos-hidden` entry point.
    pub http_port: u16,
    /// And port 443: `losos-hidden-tls`.
    pub https_port: u16,
}

/// A box's addresses. Either may be missing: that network is off, or its
/// daemon has not made the address yet.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Addresses {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub i2p: Option<String>,
}

impl Addresses {
    /// Both names, the ones there are, for a Host rule.
    #[must_use]
    pub fn hosts(&self) -> Vec<String> {
        self.tor.iter().chain(self.i2p.iter()).cloned().collect()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tor.is_none() && self.i2p.is_none()
    }

    /// Only the names that look like what they claim to be.
    #[must_use]
    pub fn checked(self) -> Self {
        Addresses {
            tor: self.tor.filter(|n| valid_onion(n)),
            i2p: self.i2p.filter(|n| valid_b32(n)),
        }
    }
}

/// A v3 onion name: 56 base32 characters and `.onion`.
#[must_use]
pub fn valid_onion(name: &str) -> bool {
    name.strip_suffix(".onion")
        .is_some_and(|label| label.len() == 56 && label.bytes().all(base32_char))
}

/// An I2P destination's hash name: 52 base32 characters and `.b32.i2p`.
#[must_use]
pub fn valid_b32(name: &str) -> bool {
    name.strip_suffix(".b32.i2p")
        .is_some_and(|label| label.len() == 52 && label.bytes().all(base32_char))
}

fn base32_char(b: u8) -> bool {
    b.is_ascii_lowercase() || (b'2'..=b'7').contains(&b)
}

#[derive(Serialize)]
struct Service<'a> {
    id: &'a str,
    http: u16,
    https: u16,
}

/// The list `losos-hidden-sync` reads: one service per box, sorted, each to
/// the two entry points.
#[must_use]
pub fn services_json(ids: &[&str], opts: &HiddenOpts) -> String {
    let mut ids: Vec<&str> = ids.to_vec();
    ids.sort_unstable();
    ids.dedup();
    let list: Vec<Service<'_>> = ids
        .into_iter()
        .map(|id| Service {
            id,
            http: opts.http_port,
            https: opts.https_port,
        })
        .collect();
    let mut text = serde_json::to_string_pretty(&list).unwrap_or_else(|_| "[]".to_string());
    text.push('\n');
    text
}

/// `addresses.json` as `losos-hidden-sync` writes it, with every name that
/// does not look right dropped and boxes with none left out. Anything
/// unreadable is no addresses: the file is rewritten every half minute.
#[must_use]
pub fn parse_addresses(text: &str) -> BTreeMap<String, Addresses> {
    serde_json::from_str::<BTreeMap<String, Addresses>>(text)
        .unwrap_or_default()
        .into_iter()
        .map(|(id, a)| (id, a.checked()))
        .filter(|(_, a)| !a.is_empty())
        .collect()
}

/// [`parse_addresses`] of the file, or none when it is not there yet.
pub async fn read_addresses(file: &Path) -> BTreeMap<String, Addresses> {
    match tokio::fs::read_to_string(file).await {
        Ok(text) => parse_addresses(&text),
        Err(_) => BTreeMap::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ONION: &str = "abcdefghijklmnopqrstuvwxyz234567abcdefghijklmnopqrstuvwx.onion";
    const B32: &str = "abcdefghijklmnopqrstuvwxyz234567abcdefghijklmnopqrst.b32.i2p";

    fn opts() -> HiddenOpts {
        HiddenOpts {
            dir: "/var/lib/losos-registrar/hidden".into(),
            addresses_file: "/var/lib/losos-hidden/addresses.json".into(),
            http_port: 8480,
            https_port: 8444,
        }
    }

    #[test]
    fn names_are_checked_by_shape() {
        assert!(valid_onion(ONION));
        assert!(valid_b32(B32));
        assert!(!valid_onion("example.com"));
        assert!(!valid_onion(&ONION.to_uppercase()));
        assert!(!valid_onion(&format!("x{ONION}")));
        assert!(!valid_b32(&ONION.replace(".onion", ".b32.i2p")));
        assert!(!valid_b32("evil.i2p"));
        // A subdomain of an onion would route a second name to the box.
        assert!(!valid_onion(&format!("www.{ONION}")));
    }

    #[test]
    fn services_are_sorted_and_unique() {
        let text = services_json(&["b", "a", "b"], &opts());
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            v,
            serde_json::json!([
                { "id": "a", "http": 8480, "https": 8444 },
                { "id": "b", "http": 8480, "https": 8444 },
            ])
        );
        assert_eq!(services_json(&[], &opts()), "[]\n");
    }

    #[test]
    fn addresses_drop_what_does_not_look_right() {
        let text = format!(
            r#"{{
              "box-a": {{ "tor": "{ONION}", "i2p": "{B32}" }},
              "box-b": {{ "tor": "Host(`evil`)", "i2p": null }},
              "box-c": {{ "i2p": "{B32}" }},
              "box-d": {{}}
            }}"#
        );
        let parsed = parse_addresses(&text);
        assert_eq!(parsed.len(), 2);
        assert_eq!(
            parsed["box-a"].hosts(),
            vec![ONION.to_string(), B32.to_string()]
        );
        assert_eq!(parsed["box-c"].tor, None);
        assert_eq!(parsed["box-c"].i2p.as_deref(), Some(B32));
        assert!(parse_addresses("not json").is_empty());
    }

    #[test]
    fn routes_listen_on_the_hidden_entry_points_only() {
        let yaml = crate::config::hidden_routes_yaml(&[
            crate::config::HiddenRoute {
                id: "box-a".into(),
                tunnel_port: 50001,
                hosts: vec![B32.into(), ONION.into()],
            },
            crate::config::HiddenRoute {
                id: "box-b".into(),
                tunnel_port: 50002,
                hosts: vec![],
            },
        ])
        .unwrap();
        let v: serde_json::Value = serde_yaml::from_str(&yaml).unwrap();
        let routers = v["http"]["routers"].as_object().unwrap();
        assert_eq!(routers.len(), 2, "{yaml}");
        let plain = &routers["box-a-hidden"];
        assert_eq!(plain["entryPoints"], serde_json::json!([PLAIN_ENTRY_POINT]));
        assert_eq!(plain["rule"], format!("Host(`{B32}`) || Host(`{ONION}`)"));
        assert!(plain.get("tls").is_none());
        let tls = &routers["box-a-hidden-tls"];
        assert_eq!(tls["entryPoints"], serde_json::json!([TLS_ENTRY_POINT]));
        // Traefik's own certificate: no resolver, no domains to ask for.
        assert_eq!(tls["tls"], serde_json::json!({}));
        assert_eq!(
            v["http"]["services"]["box-a-hidden"]["loadBalancer"]["servers"][0]["url"],
            "http://127.0.0.1:50001"
        );
        assert!(crate::config::hidden_routes_yaml(&[]).is_none());
    }

    #[test]
    fn a_heartbeat_answer_keeps_only_good_names() {
        use crate::announce::hidden_from_answer;
        let body = format!(r#"{{"hidden":{{"tor":"{ONION}","i2p":"bad"}}}}"#);
        assert_eq!(
            hidden_from_answer(Some(&body)).as_deref(),
            Some(format!("{{\"tor\":\"{ONION}\"}}\n").as_str())
        );
        assert_eq!(
            hidden_from_answer(Some(r#"{"hidden":{"tor":"x.onion"}}"#)),
            None
        );
        assert_eq!(hidden_from_answer(Some("")), None);
        assert_eq!(hidden_from_answer(None), None);
    }
}
