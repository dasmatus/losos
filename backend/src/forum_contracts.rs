//! Framework-agnostic API contract types for LosOS Forum (Phase 1–2).
//!
//! Design: `docs/superpowers/specs/2026-11-05-losos-forum-design.md`
//! Contracts: `docs/superpowers/specs/2026-11-07-losos-forum-ui-api-contracts.md`
//!
//! These types define request/response shapes for the planned `/api/forum/*`
//! surfaces. They are intentionally transport-light (serde data contracts) so
//! backend routes, React admin UI, and the Dioxus rewrite can stay aligned.
//! This module is a scaffold/reference contract and is not wired into HTTP
//! routes yet.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// Opaque identifier (string form; UI/backend must not assume numeric/UUID).
pub type Id = String;

/// RFC 3339 / ISO 8601 UTC timestamp string.
pub type Timestamp = String;

/// Cursor-based page envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    pub items: Vec<T>,
    pub page: PageInfo,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PageInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ApiError {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub forum_enabled: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub edge_required: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub chat_bridge_required: bool,
}

fn is_false(v: &bool) -> bool {
    !*v
}

/* ── Status / gate ─────────────────────────────────────────────────────── */

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForumStatus {
    pub scaffold: bool,
    pub enabled: bool,
    pub phase: ForumPhase,
    pub edge_local_routing: bool,
    pub edge: ForumEdgeStatus,
    pub features: ForumFeatures,
    pub gates: ForumGates,
    pub design: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForumPhase {
    Scaffold,
    Phase1Core,
    Phase2Assistant,
    Phase3ActivityPub,
    Phase4MatrixDmBridge,
    Phase5Hardening,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForumEdgeStatus {
    pub reachable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<ForumEdgePath>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForumEdgePath {
    pub name: String,
    pub url: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForumFeatures {
    pub activity_pub: FeatureStatus,
    pub matrix_dm_bridge: MatrixDmBridgeFeatureStatus,
    pub assistant: AssistantFeatureStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureStatus {
    pub enabled: bool,
    pub implemented: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixDmBridgeFeatureStatus {
    pub enabled: bool,
    pub implemented: bool,
    pub chat_visible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantFeatureStatus {
    pub enabled: bool,
    pub implemented: bool,
    pub provider_configured: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForumGates {
    pub proxy_required: bool,
    /// `None` = open design decision (proxy+edge only vs mesh join required).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mesh_required: Option<bool>,
}

/* ── Identity / actors ─────────────────────────────────────────────────── */

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForumMe {
    pub actor: ActorRef,
    pub permissions: ForumPermissions,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActorRef {
    pub id: Id,
    pub handle: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
    pub kind: ActorKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActorKind {
    Person,
    Assistant,
    Service,
    Remote,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ForumPermissions {
    pub can_read: bool,
    pub can_post: bool,
    pub can_create_space: bool,
    pub can_moderate: bool,
    pub can_use_assistant: bool,
}

/* ── Spaces ────────────────────────────────────────────────────────────── */

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpaceSummary {
    pub id: Id,
    pub name: String,
    pub slug: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub kind: SpaceKind,
    pub visibility: Visibility,
    pub counts: SpaceCounts,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

pub type Space = SpaceSummary;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpaceKind {
    TopicSpace,
    ChannelSpace,
    Hybrid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    Public,
    Members,
    Moderated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SpaceCounts {
    pub topics: u64,
    pub channels: u64,
    pub unread: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSpaceRequest {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub kind: SpaceKind,
    #[serde(default = "default_visibility")]
    pub visibility: Visibility,
}

fn default_visibility() -> Visibility {
    Visibility::Members
}

/* ── Topics / posts (Discourse-style) ──────────────────────────────────── */

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopicSummary {
    pub id: Id,
    pub space_id: Id,
    pub title: String,
    pub slug: String,
    pub author: ActorRef,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    pub pinned: bool,
    pub locked: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved: Option<bool>,
    pub reply_count: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub view_count: Option<u64>,
    pub last_activity_at: Timestamp,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub excerpt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unread: Option<bool>,
    pub created_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopicDetail {
    pub topic: TopicSummary,
    pub posts: Page<Post>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Post {
    pub id: Id,
    pub topic_id: Id,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<Id>,
    pub author: ActorRef,
    pub body_markdown: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_html_safe: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to_post_id: Option<Id>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reactions: Vec<ReactionSummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mentions: Vec<Mention>,
    pub created_at: Timestamp,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edited_at: Option<Timestamp>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub deleted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Provenance>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTopicRequest {
    pub space_id: Id,
    pub title: String,
    pub body_markdown: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePostRequest {
    pub body_markdown: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to_post_id: Option<Id>,
}

/* ── Channels / messages (Slack-style) ─────────────────────────────────── */

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelSummary {
    pub id: Id,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub space_id: Option<Id>,
    pub name: String,
    pub slug: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub visibility: Visibility,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub member_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unread_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_message_at: Option<Timestamp>,
    pub created_at: Timestamp,
}

pub type ChannelDetail = ChannelSummary;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelMessage {
    pub id: Id,
    pub channel_id: Id,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<Id>,
    pub author: ActorRef,
    pub body_markdown: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_html_safe: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to_message_id: Option<Id>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reactions: Vec<ReactionSummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mentions: Vec<Mention>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seq: Option<u64>,
    pub created_at: Timestamp,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edited_at: Option<Timestamp>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub deleted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Provenance>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateChannelRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub space_id: Option<Id>,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default = "default_visibility")]
    pub visibility: Visibility,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateChannelMessageRequest {
    pub body_markdown: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to_message_id: Option<Id>,
}

/* ── Reactions / mentions / provenance ─────────────────────────────────── */

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReactionSummary {
    pub emoji: String,
    pub count: u64,
    #[serde(default, skip_serializing_if = "is_false")]
    pub reacted_by_me: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReactionRequest {
    pub emoji: String,
    #[serde(default = "default_true")]
    pub add: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mention {
    pub kind: MentionKind,
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor_id: Option<Id>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MentionKind {
    Person,
    Assistant,
    Channel,
    Topic,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Provenance {
    pub kind: ProvenanceKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompted_by_message_id: Option<Id>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generated_at: Option<Timestamp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disclaimer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvenanceKind {
    Local,
    Assistant,
    ActivityPub,
    Matrix,
    Bridge,
}

/* ── DMs / bridge (Phase 1 contract, Phase 4 implementation) ───────────── */

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DmSummary {
    pub id: Id,
    pub participants: Vec<ActorRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat_room_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matrix_room_id: Option<String>,
    pub bridge_state: DmBridgeState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_message_at: Option<Timestamp>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DmBridgeState {
    LocalOnly,
    PendingBridge,
    Bridged,
    Degraded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DmMessage {
    pub id: Id,
    pub dm_id: Id,
    pub author: ActorRef,
    pub body_markdown: String,
    pub created_at: Timestamp,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Provenance>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDmRequest {
    pub participant_ids: Vec<Id>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDmMessageRequest {
    pub body_markdown: String,
}

/* ── Events (Phase 1 real-time contract) ───────────────────────────────── */

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForumEvent {
    pub event_id: String,
    pub event_type: String,
    pub occurred_at: Timestamp,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor: Option<ActorRef>,
    pub payload: serde_json::Value,
}

/* ── Moderation / assistant (Phase 1–2) ────────────────────────────────── */

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModerateRequest {
    pub action: ModerationAction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModerationAction {
    Lock,
    Unlock,
    Pin,
    Unpin,
    Hide,
    Restore,
    Delete,
    Resolve,
    Unresolve,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantPreviewRequest {
    pub context_kind: String,
    pub context_id: Id,
    pub prompt: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantPreviewResponse {
    pub allowed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub provider_configured: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_roundtrip() {
        let status = ForumStatus {
            scaffold: true,
            enabled: true,
            phase: ForumPhase::Scaffold,
            edge_local_routing: true,
            edge: ForumEdgeStatus {
                reachable: false,
                path: None,
            },
            features: ForumFeatures {
                activity_pub: FeatureStatus {
                    enabled: true,
                    implemented: false,
                },
                matrix_dm_bridge: MatrixDmBridgeFeatureStatus {
                    enabled: true,
                    implemented: false,
                    chat_visible: false,
                },
                assistant: AssistantFeatureStatus {
                    enabled: false,
                    implemented: false,
                    provider_configured: false,
                },
            },
            gates: ForumGates {
                proxy_required: true,
                mesh_required: None,
            },
            design: "docs/superpowers/specs/2026-11-05-losos-forum-design.md".to_owned(),
        };
        let json = serde_json::to_string(&status).expect("serialize");
        let back: ForumStatus = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(status, back);
        assert!(json.contains("\"edgeLocalRouting\""));
        assert!(json.contains("\"activityPub\""));
        assert!(!json.contains("\"meshRequired\""));
    }

    #[test]
    fn page_roundtrip() {
        let page = Page::<String> {
            items: vec!["a".to_owned()],
            page: PageInfo {
                next_cursor: None,
                has_more: false,
            },
        };
        let json = serde_json::to_string(&page).expect("serialize");
        let back: Page<String> = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(page, back);
    }
}
