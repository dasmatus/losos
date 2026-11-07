//! LosOS Forum scaffold contracts and hook points.
//!
//! Design: docs/superpowers/specs/2026-11-05-losos-forum-design.md
//!
//! SCAFFOLD SAFETY:
//! - This module defines interfaces/contracts/stubs only.
//! - It does NOT implement ActivityPub HTTP signature verification, inbox
//!   processing, object delivery/retries, WebFinger resolution against real
//!   identities, Matrix appservice transactions, room synchronization, or
//!   E2EE bridge behavior.
//! - Federation and bridge traffic, when implemented, must respect edge-local
//!   routing via the selected edge/proxy path and must not open unsolicited
//!   WAN listeners on the box.
//!
//! Planned responsibilities (not implemented): core hybrid model, @LosOS
//! mention handling, ActivityPub adapter, Matrix DM bridge (appearing in LosOS
//! Chat), edge-local routing via the selected edge/proxy network.
//!
//! Kept alongside `crate::forum` (Phase 1 foundation lives in
//! `backend/src/forum/`). Edge/federation hooks here remain verified stubs only.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// Placeholder status for scaffold verification.
pub const SCAFFOLD: &str = "losos-forum-scaffold";

/// Design spec path, for diagnostics and scaffold metadata.
pub const DESIGN_SPEC: &str = "docs/superpowers/specs/2026-11-05-losos-forum-design.md";

/// Runtime configuration surfaced to lososd via environment variables.
/// All fields are hook points; missing/empty values mean "use module defaults".
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForumRuntimeConfig {
    pub enabled: bool,
    pub state_dir: String,
    pub status_file: String,
    pub contracts_file: String,
    pub port: u16,
    pub api_path: String,
    pub ui_path: String,
    pub edge: EdgeRoutingConfig,
    pub activity_pub: ActivityPubConfig,
    pub matrix_dm_bridge: MatrixDmBridgeConfig,
    pub assistant: AssistantConfig,
}

impl ForumRuntimeConfig {
    /// Reads scaffold env plumbing written by `modules/forum.nix`.
    ///
    /// This is a pure config loader hook; it does not start services, contact
    /// the edge, or perform federation/bridge work.
    pub fn from_env() -> Option<Self> {
        if std::env::var("LOSOS_FORUM_SCAFFOLD").ok().as_deref() != Some("1") {
            return None;
        }
        Some(Self {
            enabled: true,
            state_dir: env_or("LOSOS_FORUM_STATE_DIR", "/var/lib/losos/forum"),
            status_file: env_or(
                "LOSOS_FORUM_STATUS_FILE",
                "/var/lib/losos/forum/scaffold-status.json",
            ),
            contracts_file: env_or(
                "LOSOS_FORUM_CONTRACTS_FILE",
                "/var/lib/losos/forum/federation-contracts.json",
            ),
            port: env_u16("LOSOS_FORUM_PORT", 8096),
            api_path: env_or("LOSOS_FORUM_API_PATH", "/api/forum"),
            ui_path: env_or("LOSOS_FORUM_UI_PATH", "/forum"),
            edge: EdgeRoutingConfig::from_env(),
            activity_pub: ActivityPubConfig::from_env(),
            matrix_dm_bridge: MatrixDmBridgeConfig::from_env(),
            assistant: AssistantConfig::from_env(),
        })
    }
}

/// Edge/proxy routing hook points.
///
/// SCAFFOLD: records the intended routing contract. Real routing must reuse the
/// existing edge-path selection/pin logic and must not create ad hoc WAN
/// listeners.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgeRoutingConfig {
    /// Prefer the selected edge/proxy path for forum federation traffic.
    pub edge_local: bool,
    /// Proxy must be enabled (gate; enforced by Nix module assertions).
    pub require_proxy: bool,
    /// Open design decision: require mesh/cluster membership.
    pub require_mesh: bool,
    /// `box`, `service-hook`, or `disabled` (scaffold registration mode).
    pub registration_mode: String,
    /// Path to lososd's edge-path env file (same contract as proxy/edge).
    pub path_file: String,
}

impl EdgeRoutingConfig {
    fn from_env() -> Self {
        Self {
            edge_local: env_bool("LOSOS_FORUM_EDGE_LOCAL"),
            require_proxy: env_bool("LOSOS_FORUM_EDGE_REQUIRE_PROXY"),
            require_mesh: env_bool("LOSOS_FORUM_EDGE_REQUIRE_MESH"),
            registration_mode: env_or("LOSOS_FORUM_EDGE_REGISTRATION_MODE", "box"),
            path_file: env_or("LOSOS_FORUM_EDGE_PATH_FILE", "/run/losos/edge-path.env"),
        }
    }
}

/// ActivityPub federation hook points/contracts.
///
/// STUB ONLY: no HTTP signatures, no actor key management, no inbox/outbox
/// processing, no delivery queue, no remote fetches, no abuse controls beyond
/// future implementation. Endpoint paths define the intended shape only.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityPubConfig {
    pub enabled: bool,
    /// Open design decision; default false in scaffold.
    pub public_federation: bool,
    pub base_path: String,
    pub webfinger_path: String,
    pub nodeinfo_path: String,
    pub shared_inbox_path: String,
    pub outbox_hook_path: String,
}

impl ActivityPubConfig {
    fn from_env() -> Self {
        Self {
            enabled: env_bool("LOSOS_FORUM_ACTIVITYPUB"),
            public_federation: env_bool("LOSOS_FORUM_ACTIVITYPUB_PUBLIC"),
            base_path: env_or("LOSOS_FORUM_ACTIVITYPUB_BASE_PATH", "/forum/ap"),
            webfinger_path: env_or(
                "LOSOS_FORUM_ACTIVITYPUB_WEBFINGER_PATH",
                "/.well-known/webfinger",
            ),
            nodeinfo_path: env_or(
                "LOSOS_FORUM_ACTIVITYPUB_NODEINFO_PATH",
                "/.well-known/nodeinfo",
            ),
            shared_inbox_path: env_or("LOSOS_FORUM_ACTIVITYPUB_SHARED_INBOX", "/forum/ap/inbox"),
            outbox_hook_path: env_or("LOSOS_FORUM_ACTIVITYPUB_OUTBOX_HOOK", "/forum/ap/outbox"),
        }
    }

    /// Returns the planned endpoint contract. This does not register routes or
    /// imply that handlers exist.
    #[must_use]
    pub fn endpoint_contract(&self) -> ActivityPubEndpointContract {
        ActivityPubEndpointContract {
            implemented: false,
            webfinger: self.webfinger_path.clone(),
            nodeinfo: self.nodeinfo_path.clone(),
            actor: format!("{}/users/{{id}}", self.base_path),
            inbox: format!("{}/users/{{id}}/inbox", self.base_path),
            outbox: format!("{}/users/{{id}}/outbox", self.base_path),
            shared_inbox: self.shared_inbox_path.clone(),
            followers: format!("{}/users/{{id}}/followers", self.base_path),
            following: format!("{}/users/{{id}}/following", self.base_path),
            object: format!("{}/objects/{{id}}", self.base_path),
            safety: "STUB: no HTTP signature verification, delivery, fetch, or inbox processing"
                .into(),
        }
    }
}

/// Planned ActivityPub endpoint shapes.
///
/// STUB CONTRACT: field values are route templates/paths, not live handlers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityPubEndpointContract {
    pub implemented: bool,
    pub webfinger: String,
    pub nodeinfo: String,
    pub actor: String,
    pub inbox: String,
    pub outbox: String,
    pub shared_inbox: String,
    pub followers: String,
    pub following: String,
    pub object: String,
    pub safety: String,
}

/// Minimal placeholder types for future ActivityPub object mapping.
///
/// STUB ONLY: these are not complete ActivityStreams representations and must
/// not be used to claim protocol compliance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityPubActorStub {
    pub id: String,
    pub kind: String, // e.g. Person, Service (for @LosOS), Group - TBD
    pub preferred_username: String,
    pub inbox: String,
    pub outbox: String,
    pub implemented: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityPubActivityStub {
    pub id: String,
    pub kind: String, // Create, Follow, Accept, Announce, Like - TBD
    pub actor: String,
    pub object: Option<String>,
    pub target: Option<String>,
    pub implemented: bool,
}

/// Matrix DM bridge hook points/contracts.
///
/// STUB ONLY: no appservice registration/runtime, no Synapse admin calls, no
/// room creation/sync, no identity mapping writes, no E2EE bridge behavior.
/// The contract records how Forum DMs are intended to appear in LosOS Chat.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixDmBridgeConfig {
    pub enabled: bool,
    /// `appservice-contract`, `lososd-mediated`, or `disabled`.
    pub mode: String,
    pub require_chat: bool,
    pub namespace_prefix: String,
    /// `open-decision`, `unencrypted-v1-stub`, or `passthrough-research`.
    pub e2ee_policy: String,
}

impl MatrixDmBridgeConfig {
    fn from_env() -> Self {
        Self {
            enabled: env_bool("LOSOS_FORUM_MATRIX_DM_BRIDGE"),
            mode: env_or("LOSOS_FORUM_MATRIX_DM_BRIDGE_MODE", "appservice-contract"),
            require_chat: env_bool("LOSOS_FORUM_MATRIX_DM_REQUIRE_CHAT"),
            namespace_prefix: env_or("LOSOS_FORUM_MATRIX_DM_NAMESPACE_PREFIX", "_losos_forum_"),
            e2ee_policy: env_or("LOSOS_FORUM_MATRIX_DM_E2EE_POLICY", "open-decision"),
        }
    }

    /// Returns the intended DM mapping contract.
    ///
    /// STUB ONLY: no runtime behavior is implied.
    #[must_use]
    pub fn mapping_contract(&self) -> MatrixDmBridgeContract {
        MatrixDmBridgeContract {
            implemented: false,
            mode: self.mode.clone(),
            requires_losos_chat: self.require_chat,
            e2ee_policy: self.e2ee_policy.clone(),
            forum_dm_id_format: "forum-dm:<conversation-id>".into(),
            matrix_target: "Direct Matrix room visible in LosOS Chat/Synapse for participating LosOS identities".into(),
            namespace_prefix: self.namespace_prefix.clone(),
            safety: "STUB: no appservice runtime, room sync, identity provisioning, or E2EE bridge logic".into(),
            open_decisions: vec![
                "Bridge mechanism: appservice vs lososd-mediated sync vs hybrid (evaluate against backend/src/chat.rs).".into(),
                "E2EE: acceptable limitations for bridged DMs in v1; no E2EE bridge shipped by this scaffold.".into(),
                "Identity mapping: stable LosOS/AP/Matrix identifiers and device/session constraints.".into(),
            ],
        }
    }
}

/// Planned Matrix DM bridge mapping contract.
///
/// STUB CONTRACT: describes intended integration, not a working bridge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixDmBridgeContract {
    pub implemented: bool,
    pub mode: String,
    pub requires_losos_chat: bool,
    pub e2ee_policy: String,
    pub forum_dm_id_format: String,
    pub matrix_target: String,
    pub namespace_prefix: String,
    pub safety: String,
    pub open_decisions: Vec<String>,
}

/// Placeholder representation of a Forum DM mapping target.
///
/// STUB ONLY.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForumDmMappingStub {
    pub forum_conversation_id: String,
    pub matrix_room_id: Option<String>,
    pub losos_participants: Vec<String>,
    pub federated_participants: Vec<String>,
    pub appears_in_losos_chat: bool,
    pub implemented: bool,
}

/// @LosOS mention assistant hook points.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantConfig {
    pub enabled: bool,
    /// `disabled`, `stub`, `external`, or `edge-mesh`.
    pub provider: String,
}

impl AssistantConfig {
    fn from_env() -> Self {
        Self {
            enabled: env_bool("LOSOS_FORUM_ASSISTANT"),
            provider: env_or("LOSOS_FORUM_ASSISTANT_PROVIDER", "disabled"),
        }
    }
}

fn env_or(key: &str, default: &str) -> String {
    match std::env::var(key) {
        Ok(v) if !v.is_empty() => v,
        _ => default.to_owned(),
    }
}

fn env_bool(key: &str) -> bool {
    matches!(
        std::env::var(key).ok().as_deref(),
        Some("1" | "true" | "yes")
    )
}

fn env_u16(key: &str, default: u16) -> u16 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_contract_has_expected_shapes() {
        let cfg = ActivityPubConfig {
            enabled: true,
            public_federation: false,
            base_path: "/forum/ap".into(),
            webfinger_path: "/.well-known/webfinger".into(),
            nodeinfo_path: "/.well-known/nodeinfo".into(),
            shared_inbox_path: "/forum/ap/inbox".into(),
            outbox_hook_path: "/forum/ap/outbox".into(),
        };
        let c = cfg.endpoint_contract();
        assert!(!c.implemented);
        assert_eq!(c.actor, "/forum/ap/users/{id}");
        assert_eq!(c.inbox, "/forum/ap/users/{id}/inbox");
        assert_eq!(c.shared_inbox, "/forum/ap/inbox");
        assert!(c.safety.contains("STUB"));
    }

    #[test]
    fn dm_mapping_contract_records_open_decisions() {
        let cfg = MatrixDmBridgeConfig {
            enabled: true,
            mode: "appservice-contract".into(),
            require_chat: true,
            namespace_prefix: "_losos_forum_".into(),
            e2ee_policy: "open-decision".into(),
        };
        let c = cfg.mapping_contract();
        assert!(!c.implemented);
        assert!(c.requires_losos_chat);
        assert!(!c.open_decisions.is_empty());
        assert!(c.safety.contains("STUB"));
    }
}
