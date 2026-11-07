/**
 * Framework-agnostic API contract types for LosOS Forum (Phase 1–2).
 *
 * Canonical spec: docs/superpowers/specs/2026-11-07-losos-forum-ui-api-contracts.md
 * Rust mirror: backend/src/forum_contracts.rs
 *
 * Keep these shapes aligned with backend serde contracts (camelCase JSON).
 */

export type Id = string;
export type Timestamp = string; // RFC 3339 / ISO 8601 UTC

export interface Page<T> {
  items: T[];
  page: { nextCursor?: string; hasMore: boolean };
}

export interface ApiError {
  message: string;
  code?: string;
  forumEnabled?: boolean;
  edgeRequired?: boolean;
  chatBridgeRequired?: boolean;
}

export type ForumPhase =
  | "scaffold"
  | "phase1_core"
  | "phase2_assistant"
  | "phase3_activity_pub"
  | "phase4_matrix_dm_bridge"
  | "phase5_hardening";

export interface ForumEdgePath {
  name: string;
  url: string;
  source: string;
}

export interface ForumStatus {
  scaffold: boolean;
  enabled: boolean;
  phase: ForumPhase;
  edgeLocalRouting: boolean;
  edge: { reachable: boolean; path?: ForumEdgePath | null };
  features: {
    activityPub: { enabled: boolean; implemented: boolean };
    matrixDmBridge: { enabled: boolean; implemented: boolean; chatVisible: boolean };
    assistant: { enabled: boolean; implemented: boolean; providerConfigured: boolean };
  };
  gates: { proxyRequired: boolean; meshRequired?: boolean | null };
  design: string;
}

export type ActorKind = "person" | "assistant" | "service" | "remote";

export interface ActorRef {
  id: Id;
  handle: string;
  displayName?: string;
  avatarUrl?: string;
  kind: ActorKind;
}

export interface ForumPermissions {
  canRead: boolean;
  canPost: boolean;
  canCreateSpace: boolean;
  canModerate: boolean;
  canUseAssistant: boolean;
}

export interface ForumMe {
  actor: ActorRef;
  permissions: ForumPermissions;
}

export type SpaceKind = "topic_space" | "channel_space" | "hybrid";
export type Visibility = "public" | "members" | "moderated";

export interface SpaceSummary {
  id: Id;
  name: string;
  slug: string;
  description?: string;
  kind: SpaceKind;
  visibility: Visibility;
  counts: { topics: number; channels: number; unread: number };
  createdAt: Timestamp;
  updatedAt: Timestamp;
}

export type Space = SpaceSummary;

export interface CreateSpaceRequest {
  name: string;
  slug?: string;
  description?: string;
  kind: SpaceKind;
  visibility?: Visibility;
}

export interface TopicSummary {
  id: Id;
  spaceId: Id;
  title: string;
  slug: string;
  author: ActorRef;
  tags?: string[];
  pinned: boolean;
  locked: boolean;
  resolved?: boolean;
  replyCount: number;
  viewCount?: number;
  lastActivityAt: Timestamp;
  excerpt?: string;
  unread?: boolean;
  createdAt: Timestamp;
}

export interface ReactionSummary {
  emoji: string;
  count: number;
  reactedByMe?: boolean;
}

export type MentionKind = "person" | "assistant" | "channel" | "topic";
export interface Mention {
  kind: MentionKind;
  target: string;
  actorId?: Id;
  start?: number;
  end?: number;
}

export type ProvenanceKind = "local" | "assistant" | "activity_pub" | "matrix" | "bridge";
export interface Provenance {
  kind: ProvenanceKind;
  provider?: string;
  model?: string;
  requestId?: string;
  promptedByMessageId?: Id;
  generatedAt?: Timestamp;
  disclaimer?: string;
  source?: string;
}

export interface Post {
  id: Id;
  topicId: Id;
  threadId?: Id;
  author: ActorRef;
  bodyMarkdown: string;
  bodyHtmlSafe?: string;
  replyToPostId?: Id;
  reactions?: ReactionSummary[];
  mentions?: Mention[];
  createdAt: Timestamp;
  editedAt?: Timestamp;
  deleted?: boolean;
  provenance?: Provenance;
}

export interface TopicDetail {
  topic: TopicSummary;
  posts: Page<Post>;
}

export interface CreateTopicRequest {
  spaceId: Id;
  title: string;
  bodyMarkdown: string;
  tags?: string[];
}

export interface CreatePostRequest {
  bodyMarkdown: string;
  replyToPostId?: Id;
}

export interface ChannelSummary {
  id: Id;
  spaceId?: Id;
  name: string;
  slug: string;
  description?: string;
  visibility: Visibility;
  memberCount?: number;
  unreadCount?: number;
  lastMessageAt?: Timestamp;
  createdAt: Timestamp;
}

export type ChannelDetail = ChannelSummary;

export interface ChannelMessage {
  id: Id;
  channelId: Id;
  threadId?: Id;
  author: ActorRef;
  bodyMarkdown: string;
  bodyHtmlSafe?: string;
  replyToMessageId?: Id;
  reactions?: ReactionSummary[];
  mentions?: Mention[];
  seq?: number;
  createdAt: Timestamp;
  editedAt?: Timestamp;
  deleted?: boolean;
  provenance?: Provenance;
}

export interface CreateChannelRequest {
  spaceId?: Id;
  name: string;
  slug?: string;
  description?: string;
  visibility?: Visibility;
}

export interface CreateChannelMessageRequest {
  bodyMarkdown: string;
  replyToMessageId?: Id;
}

export type DmBridgeState = "local_only" | "pending_bridge" | "bridged" | "degraded";
export interface DmSummary {
  id: Id;
  participants: ActorRef[];
  chatRoomId?: string;
  matrixRoomId?: string;
  bridgeState: DmBridgeState;
  lastMessageAt?: Timestamp;
}

export interface DmMessage {
  id: Id;
  dmId: Id;
  author: ActorRef;
  bodyMarkdown: string;
  createdAt: Timestamp;
  provenance?: Provenance;
}

export interface ForumEvent<T = unknown> {
  eventId: string;
  eventType: string;
  occurredAt: Timestamp;
  actor?: ActorRef;
  payload: T;
}

/** Scaffold status used by the minimal placeholder UI when backend route is unavailable. */
export const SCAFFOLD_FORUM_STATUS: ForumStatus = {
  scaffold: true,
  enabled: false,
  phase: "scaffold",
  edgeLocalRouting: true,
  edge: { reachable: false, path: null },
  features: {
    activityPub: { enabled: true, implemented: false },
    matrixDmBridge: { enabled: true, implemented: false, chatVisible: false },
    assistant: { enabled: false, implemented: false, providerConfigured: false },
  },
  gates: { proxyRequired: true, meshRequired: null },
  design: "docs/superpowers/specs/2026-11-05-losos-forum-design.md",
};
