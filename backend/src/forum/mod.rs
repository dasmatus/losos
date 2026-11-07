//! LosOS Forum (Phase 1 foundation).
//!
//! Design: docs/superpowers/specs/2026-11-05-losos-forum-design.md
//!
//! A from-scratch Discourse+Slack hybrid integrated with LosOS. This module
//! provides the Phase 1 backend foundation: a canonical model, SQLite-backed
//! storage, authz stubs, and API-oriented operations. It is wired minimally and
//! gated by the scaffold environment (`LOSOS_FORUM_SCAFFOLD` /
//! `LOSOS_FORUM_STATE_DIR`) so it stays safe when the feature is off.
//!
//! Real-time delivery, full @LosOS handling, ActivityPub, Matrix DM bridging,
//! and the UI are later phases.

pub mod authz;
pub mod models;
pub mod storage;

pub use authz::{AuthzDecision, ForumAuthz, PrincipalRef, FORUM_SECTION};
pub use models::*;
pub use storage::{ForumDb, ForumStoreError};

use std::path::PathBuf;

/// Runtime config derived from the scaffold/NixOS module environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForumConfig {
    /// Feature gate: true when `LOSOS_FORUM_SCAFFOLD=1`.
    pub enabled: bool,
    /// Persistent state directory (`LOSOS_FORUM_STATE_DIR`).
    pub state_dir: PathBuf,
    pub edge_local_routing: bool,
    pub activitypub: bool,
    pub matrix_dm_bridge: bool,
    pub assistant: bool,
}

impl ForumConfig {
    pub fn from_env() -> Self {
        let flag = |key: &str| std::env::var(key).ok().as_deref() == Some("1");
        Self {
            enabled: flag("LOSOS_FORUM_SCAFFOLD"),
            state_dir: PathBuf::from(
                std::env::var("LOSOS_FORUM_STATE_DIR")
                    .unwrap_or_else(|_| "/var/lib/losos/forum".into()),
            ),
            edge_local_routing: flag("LOSOS_FORUM_EDGE_LOCAL"),
            activitypub: flag("LOSOS_FORUM_ACTIVITYPUB"),
            matrix_dm_bridge: flag("LOSOS_FORUM_MATRIX_DM_BRIDGE"),
            assistant: flag("LOSOS_FORUM_ASSISTANT"),
        }
    }

    pub fn disabled() -> Self {
        Self {
            enabled: false,
            state_dir: PathBuf::from("/var/lib/losos/forum"),
            edge_local_routing: false,
            activitypub: false,
            matrix_dm_bridge: false,
            assistant: false,
        }
    }

    pub fn db_path(&self) -> PathBuf {
        self.state_dir.join("forum.sqlite3")
    }
}

/// Forum runtime handle held by lososd's HTTP/API layer.
#[derive(Debug)]
pub struct Forum {
    pub config: ForumConfig,
    store: Option<ForumDb>,
}

impl Forum {
    pub fn from_env() -> Self {
        let config = ForumConfig::from_env();
        if !config.enabled {
            return Self {
                config,
                store: None,
            };
        }
        match ForumDb::open(&config.db_path()) {
            Ok(store) => Self {
                config,
                store: Some(store),
            },
            Err(e) => {
                tracing::warn!(error = %e, "forum storage unavailable; forum API will report unavailable");
                Self {
                    config,
                    store: None,
                }
            }
        }
    }

    pub fn disabled() -> Self {
        Self {
            config: ForumConfig::disabled(),
            store: None,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    pub fn store(&self) -> Result<&ForumDb, ForumStoreError> {
        if !self.config.enabled {
            return Err(ForumStoreError::Disabled);
        }
        self.store.as_ref().ok_or(ForumStoreError::Unavailable)
    }

    pub fn status(&self) -> serde_json::Value {
        serde_json::json!({
            "enabled": self.config.enabled,
            "implemented": true,
            "phase": 1,
            "storageReady": self.store.is_some(),
            "edgeLocalRouting": self.config.edge_local_routing,
            "activityPub": self.config.activitypub,
            "matrixDmBridge": self.config.matrix_dm_bridge,
            "assistant": self.config.assistant,
            "design": "docs/superpowers/specs/2026-11-05-losos-forum-design.md",
        })
    }
}

/// A mention extracted from body text (Phase 1 hook; full @LosOS handling is
/// Phase 2).
pub fn extract_mentions(body: &str) -> Vec<Mention> {
    let mut out = Vec::new();
    for token in body.split_whitespace() {
        let t = token.trim_matches(|c: char| {
            !c.is_alphanumeric() && c != '@' && c != '_' && c != '-' && c != '.'
        });
        if let Some(name) = t.strip_prefix('@') {
            if name.is_empty() {
                continue;
            }
            out.push(Mention {
                target: name.to_string(),
                is_assistant: name.eq_ignore_ascii_case("LosOS"),
            });
        }
    }
    out.sort_by_key(|a| a.target.to_lowercase());
    out.dedup_by(|a, b| a.target.eq_ignore_ascii_case(&b.target));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults_disabled_without_env() {
        let c = ForumConfig::disabled();
        assert!(!c.enabled);
        assert_eq!(c.state_dir, PathBuf::from("/var/lib/losos/forum"));
    }

    #[test]
    fn status_reports_phase() {
        let f = Forum::disabled();
        let s = f.status();
        assert_eq!(s["enabled"], false);
        assert_eq!(s["phase"], 1);
        assert_eq!(s["implemented"], true);
    }

    #[test]
    fn extract_mentions_finds_losos_and_users() {
        let ms = extract_mentions("Hello @alice and @LosOS, ping @bob!");
        assert!(ms.iter().any(|m| m.target.eq_ignore_ascii_case("alice")));
        assert!(ms.iter().any(|m| m.target.eq_ignore_ascii_case("bob")));
        let ai = ms
            .iter()
            .find(|m| m.target.eq_ignore_ascii_case("LosOS"))
            .unwrap();
        assert!(ai.is_assistant);
    }
}
