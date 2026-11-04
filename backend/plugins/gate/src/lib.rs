//! The connection gate.
//!
//! Some things LosOS offers only while the box is on a **primary or tertiary
//! connection**: it reaches an official edge itself (tertiary), or sits in
//! the main cluster (primary). Behind a local gateway (secondary), or with
//! no edge at all, the path to whoever vouches for community code and shared
//! spaces runs through someone else's machine or nowhere, so those features
//! stay stopped (agent addons, LosOS Forum, ActivityPub blogs). Local use of
//! everything else keeps working.
//!
//! There is one implementation. lososd writes the box's chosen path to
//! `/run/losos/edge-path.env` (`backend/src/edge.rs` `path_env`), and the
//! marker `/run/losos/edge-none` when no edge answers; [`gate`] reads both
//! texts and decides. A plugin reads the two files with its `fs` import and
//! passes the text in; the host passes what it read. The plugin manifest
//! needs `"files": [{ "path": "/run/losos", "access": "ro" }]`.
//!
//! ```ignore
//! let env = fs::read_string_optional("/run/losos/edge-path.env")?;
//! let none = fs::exists("/run/losos/edge-none")?;
//! let gate = losos_gate::gate(env.as_deref(), none);
//! if !gate.open {
//!     return Response::json(409, &json!({ "error": gate.sentence(), "reason": gate.reason }));
//! }
//! ```
//!
//! Answer a closed gate with `409` and `reason` (`noEdge`, `secondaryPath`
//! or `unknownTier`), and show [`Gate::view`] to the page so it can say why.

use serde_json::{json, Value};

/// Whether the gated features may run on this box's connection right now.
///
/// They may on a primary or tertiary connection: a box that reaches an
/// official edge itself, or sits in the main cluster. Behind a local gateway
/// (secondary) the path runs through someone else's machine, so they stay
/// stopped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gate {
    pub open: bool,
    /// `primary`, `secondary` or `tertiary`, as the path file says.
    pub tier: Option<String>,
    /// Why it is shut: `noEdge`, `secondaryPath` or `unknownTier`.
    pub reason: Option<&'static str>,
}

/// From the path file lososd writes (`/run/losos/edge-path.env`) and whether
/// its no-edge marker exists.
#[must_use]
pub fn gate(path_env: Option<&str>, none_marker: bool) -> Gate {
    let shut = |tier: Option<String>, reason| Gate {
        open: false,
        tier,
        reason: Some(reason),
    };
    let Some(env) = path_env.filter(|_| !none_marker) else {
        return shut(None, "noEdge");
    };
    let tier = env
        .lines()
        .find_map(|l| l.strip_prefix("LOSOS_EDGE_PATH_TIER="))
        .map(|t| t.trim().to_string());
    match tier.as_deref() {
        Some("tertiary" | "primary") => Gate {
            open: true,
            tier,
            reason: None,
        },
        Some("secondary") => shut(tier, "secondaryPath"),
        // An older file has no tier: the source says it (a configured edge is
        // tertiary, a LAN one secondary).
        None => match env
            .lines()
            .find_map(|l| l.strip_prefix("LOSOS_EDGE_PATH_SOURCE="))
            .map(str::trim)
        {
            Some("configured") => Gate {
                open: true,
                tier: Some("tertiary".into()),
                reason: None,
            },
            Some(_) => shut(Some("secondary".into()), "secondaryPath"),
            None => shut(None, "noEdge"),
        },
        Some(_) => shut(tier, "unknownTier"),
    }
}

impl Gate {
    #[must_use]
    pub fn view(&self) -> Value {
        json!({ "open": self.open, "tier": self.tier, "reason": self.reason })
    }

    /// The sentence for something refused at the gate.
    #[must_use]
    pub fn sentence(&self) -> &'static str {
        match self.reason {
            Some("secondaryPath") => {
                "This needs the box to reach an official edge directly. It is behind a local \
                 gateway now, so it stays stopped."
            }
            Some("noEdge") => {
                "This needs the box to reach an official edge. No edge answers right now, \
                 so it stays stopped."
            }
            _ => "This is not available on the box's connection.",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gate_follows_the_tier() {
        let file =
            |tier: &str| format!("LOSOS_EDGE_PATH_URL=http://e\nLOSOS_EDGE_PATH_TIER={tier}\n");
        assert!(gate(Some(&file("tertiary")), false).open);
        assert!(gate(Some(&file("primary")), false).open);
        let g = gate(Some(&file("secondary")), false);
        assert!(!g.open);
        assert_eq!(g.reason, Some("secondaryPath"));
        assert_eq!(gate(Some(&file("tertiary")), true).reason, Some("noEdge"));
        assert_eq!(gate(None, false).reason, Some("noEdge"));
        assert_eq!(
            gate(Some(&file("fourth")), false).reason,
            Some("unknownTier")
        );
        // A file from before tiers: the source decides.
        assert!(gate(Some("LOSOS_EDGE_PATH_SOURCE=configured\n"), false).open);
        assert!(!gate(Some("LOSOS_EDGE_PATH_SOURCE=lan\n"), false).open);
        assert!(gate(Some(&file("secondary")), false)
            .sentence()
            .contains("local gateway"));
    }
}
