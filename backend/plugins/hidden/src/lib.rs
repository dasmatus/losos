//! The hidden plugin: `GET /api/hidden`, the box's addresses on Tor and I2P
//! for the Network page. Two files, both written by root outside lososd
//! (`modules/hidden-lib.nix`): the addresses the box's own Tor and I2P made
//! for its front door, and those the edge it heartbeats to made for it.
//!
//! ```json
//! { "box":  { "tor": "<56>.onion", "i2p": "<52>.b32.i2p" },
//!   "edge": { "tor": null, "i2p": null } }
//! ```
//!
//! A name is `null` until its daemon has made it, and when its network is
//! off. Only names of the right shape are passed on: the page shows them
//! as links.

use losos_plugin::{config, fs, Plugin, Request, Response};
use serde_json::{json, Value};

/// A v3 onion name.
#[must_use]
pub fn valid_onion(name: &str) -> bool {
    name.strip_suffix(".onion")
        .is_some_and(|label| label.len() == 56 && label.bytes().all(base32_char))
}

/// An I2P destination's hash name.
#[must_use]
pub fn valid_b32(name: &str) -> bool {
    name.strip_suffix(".b32.i2p")
        .is_some_and(|label| label.len() == 52 && label.bytes().all(base32_char))
}

fn base32_char(b: u8) -> bool {
    b.is_ascii_lowercase() || (b'2'..=b'7').contains(&b)
}

/// The two names in `{ "tor": …, "i2p": … }`, each only when it looks right.
#[must_use]
pub fn pair(v: &Value) -> Value {
    let tor = v["tor"].as_str().filter(|n| valid_onion(n));
    let i2p = v["i2p"].as_str().filter(|n| valid_b32(n));
    json!({ "tor": tor, "i2p": i2p })
}

/// The answer from the two files' contents; a file that is missing or not
/// JSON has no names.
#[must_use]
pub fn view(addresses: Option<&[u8]>, edge: Option<&[u8]>) -> Value {
    let parse = |bytes: Option<&[u8]>| -> Value {
        bytes
            .and_then(|b| serde_json::from_slice(b).ok())
            .unwrap_or(Value::Null)
    };
    json!({
        "box": pair(&parse(addresses)["front"]),
        "edge": pair(&parse(edge)),
    })
}

fn read(key: &str, default: &str) -> Option<Vec<u8>> {
    let path = config::get_or(key, default);
    match fs::read_optional(&path) {
        Ok(bytes) => bytes,
        Err(e) => {
            losos_plugin::log::warn(&format!("cannot read {path}: {e}"));
            None
        }
    }
}

pub struct HiddenPlugin;

impl Plugin for HiddenPlugin {
    fn handle(req: Request) -> Response {
        match (req.method.as_str(), req.route.as_str()) {
            ("GET", "/api/hidden") => Response::ok(&view(
                read("addressesFile", "/var/lib/losos-hidden/addresses.json").as_deref(),
                read("edgeFile", "/var/lib/losos-hidden/edge.json").as_deref(),
            )),
            _ => Response::error(404, "not found"),
        }
    }
}

losos_plugin::export!(HiddenPlugin);

#[cfg(test)]
mod tests {
    use super::*;

    const ONION: &str = "abcdefghijklmnopqrstuvwxyz234567abcdefghijklmnopqrstuvwx.onion";
    const B32: &str = "abcdefghijklmnopqrstuvwxyz234567abcdefghijklmnopqrst.b32.i2p";

    #[test]
    fn the_front_door_and_the_edge() {
        let own = format!(r#"{{"front":{{"tor":"{ONION}","i2p":"{B32}"}}}}"#);
        let edge = format!(r#"{{"tor":"{ONION}"}}"#);
        assert_eq!(
            view(Some(own.as_bytes()), Some(edge.as_bytes())),
            json!({
                "box": { "tor": ONION, "i2p": B32 },
                "edge": { "tor": ONION, "i2p": null },
            })
        );
    }

    #[test]
    fn nothing_yet() {
        let none = json!({ "tor": null, "i2p": null });
        assert_eq!(view(None, None), json!({ "box": none, "edge": none }));
        assert_eq!(
            view(Some(b"{not json"), Some(br#"{"tor":"<script>.onion"}"#)),
            json!({ "box": none, "edge": none })
        );
    }
}
