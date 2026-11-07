# LosOS Forum — UI/API Contracts (Phase 1–2) and Implementation Plan

- Status: draft (contracts + coordination plan)
- Date: 2026-11-07
- Base: `feat/losos-forum-scaffold` / PR #530 scaffold
- Coordinates with: PR #522 (`t3/rewrite-frontend-dioxus`, branch `origin/t3/rewrite-frontend-dioxus`)
- Design spec: `docs/superpowers/specs/2026-11-05-losos-forum-design.md`

## Purpose

Define framework-agnostic API contracts (request/response shapes) for Phase 1–2 Forum surfaces and a UI implementation plan that coordinates the current React admin UI (`admin-ui/app`) with the Dioxus rewrite (`admin-ui/dioxus-app`, PR #522) with minimal duplication.

## Hard constraints (from design/owner direction)

- Discourse+Slack hybrid: canonical model supports **topics** (threaded, categorized/tagged, long-form/moderation) and **channels** (real-time timeline/threads/reactions).
- `@LosOS` mentions: Phase 2 mention parsing/handler with pluggable provider, provenance, authz/rate limits; disabled until configured.
- ActivityPub + Matrix DMs (DMs appear in LosOS Chat) via edge proxy network, edge-local routing, mesh/edge dependency.
- UI must follow design system/reuse components; coordinate with Dioxus rewrite PR #522 — do not break it.

## Current state (scaffold)

- Nix module/options: `modules/forum.nix`, `modules/options.nix` (`losos.forum.*`, `lososInternal.forum` gate: `forum.enable && proxy.enable && admin.enable && admin.ui != null`; exact mesh gate TBD).
- Backend scaffold: `backend/src/forum_scaffold.rs` (not wired into `lib.rs`/HTTP routes).
- React admin UI placeholder: `admin-ui/app/src/forum/README.md` (not routed).
- Dioxus rewrite (PR #522): separate `admin-ui/dioxus-app` crate with its own routes/API/actions; early WIP, not present in `feat/losos-forum-scaffold` tree.

## API contract principles

1. **Canonical JSON over HTTP** under `/api/forum/*` (Bearer-authenticated like other admin API routes via `lososd`/front-door auth), with real-time updates planned via WebSocket/SSE at `/api/forum/events` (Phase 1). Contracts below define shapes; transport/auth/rate-limit details follow existing `backend/src/http.rs` patterns.
2. **Stable IDs**: opaque string IDs (`id`, `spaceId`, `topicId`, `channelId`, `threadId`, `messageId`, `userId`, `dmId`). Do not assume numeric/UUID format in UI.
3. **Timestamps**: RFC 3339 / ISO 8601 strings in UTC (`createdAt`, `updatedAt`, `editedAt`, `deletedAt?`).
4. **Pagination**: cursor-based where lists can grow. Request: `?cursor=<opaque>&limit=<n>` (default/maximum enforced server-side). Response includes `items` and `page: { nextCursor?: string, hasMore: boolean }`.
5. **Errors**: follow existing API error shape conventions (`message`, optional `code`, plus any gate flags like `edgeRequired` where applicable). Forum-specific gates: `forumEnabled`, `edgeRequired`, `chatBridgeRequired` as applicable.
6. **Edge/mesh gating**: endpoints that require federation/bridge behavior must surface gate status; reads of local scaffold/status should still work when enabled but federation not ready.
7. **Provenance**: assistant/federated content carries explicit provenance fields; never render federated/assistant content as local-authoritative without markers.

## Phase 1–2 surface contracts

Canonical type definitions are provided in:
- Rust (serde-friendly, backend stub/reference): `backend/src/forum_contracts.rs`
- TypeScript (React/Dioxus-consumable shape reference): `admin-ui/app/src/forum/contracts.ts`

The shapes below are normative for Phase 1–2 UI/backend integration.

### Status / gate

`GET /api/forum/status` → `ForumStatus`

```json
{
  "scaffold": true,
  "enabled": true,
  "phase": "scaffold",
  "edgeLocalRouting": true,
  "edge": { "reachable": false, "path": null },
  "features": {
    "activityPub": { "enabled": true, "implemented": false },
    "matrixDmBridge": { "enabled": true, "implemented": false, "chatVisible": false },
    "assistant": { "enabled": false, "implemented": false, "providerConfigured": false }
  },
  "gates": { "proxyRequired": true, "meshRequired": null },
  "design": "docs/superpowers/specs/2026-11-05-losos-forum-design.md"
}
```

### Identity / actor (local view)

`GET /api/forum/me` → `ForumMe` (current LosOS identity mapped to forum actor profile; permissions summary).

### Spaces / categories

- `GET /api/forum/spaces?cursor&limit` → `Page<SpaceSummary>`
- `GET /api/forum/spaces/{spaceId}` → `Space`
- `POST /api/forum/spaces` (admin/mod; Phase 1+) body `CreateSpaceRequest` → `Space`

`Space` represents a category/space that can contain topics and/or channels (hybrid container). Fields: `id`, `name`, `slug`, `description?`, `kind` (`topic_space` | `channel_space` | `hybrid`), `visibility` (`public` | `members` | `moderated`), `counts`, timestamps.

### Topics (Discourse-style)

- `GET /api/forum/topics?spaceId&tag&cursor&limit&sort` → `Page<TopicSummary>`
- `POST /api/forum/topics` body `CreateTopicRequest` → `Topic`
- `GET /api/forum/topics/{topicId}` → `TopicDetail` (topic + initial posts/page)
- `POST /api/forum/topics/{topicId}/posts` body `CreatePostRequest` → `Post`
- `PATCH /api/forum/topics/{topicId}` / moderation actions (Phase 1 moderation primitives): `ModerateTopicRequest` → `Topic`

`TopicSummary`: `id`, `spaceId`, `title`, `slug`, `author`, `tags[]`, `pinned`, `locked`, `resolved?`, `replyCount`, `viewCount?`, `lastActivityAt`, `excerpt?`, `unread?`.
`Post`: `id`, `topicId`, `threadId?`, `author`, `bodyMarkdown`, `bodyHtmlSafe?` (server-sanitized; UI must still treat carefully), `replyToPostId?`, `reactions[]`, `mentions[]`, `editedAt?`, `deleted?`, `provenance?`.

### Channels / real-time (Slack-style)

- `GET /api/forum/channels?spaceId&cursor&limit` → `Page<ChannelSummary>`
- `POST /api/forum/channels` body `CreateChannelRequest` → `Channel`
- `GET /api/forum/channels/{channelId}` → `ChannelDetail`
- `GET /api/forum/channels/{channelId}/messages?cursor&limit&before&after` → `Page<ChannelMessage>`
- `POST /api/forum/channels/{channelId}/messages` body `CreateChannelMessageRequest` → `ChannelMessage`
- `POST /api/forum/channels/{channelId}/threads/{threadId}/messages` body `CreateChannelMessageRequest` → `ChannelMessage`
- `POST /api/forum/messages/{messageId}/reactions` body `ReactionRequest` → `ReactionSummary`

`ChannelSummary`: `id`, `spaceId?`, `name`, `slug`, `description?`, `visibility`, `memberCount?`, `unreadCount?`, `lastMessageAt?`.
`ChannelMessage`: like `Post` but `channelId`, optional `threadId`, realtime-friendly ordering fields (`seq?`), `mentions[]` including `@LosOS` tokens.

### Real-time events (Phase 1)

`GET /api/forum/events` (SSE/WebSocket; exact transport TBD) emits `ForumEvent`:
`topic.created|updated|replied`, `channel.message.created|updated|deleted`, `reaction.added|removed`, `dm.bridged` (Phase 4 visibility signal), `assistant.response` (Phase 2), `status.changed`. Events carry `eventId`, `type`, `occurredAt`, `actor`, and a typed payload reference (`topicId`/`channelId`/`messageId`).

### DMs / Chat bridge (Phase 1 contract, Phase 4 implementation)

- `GET /api/forum/dms?cursor&limit` → `Page<DmSummary>` (Forum view of bridged DMs; source of truth/appearance ultimately coordinated with LosOS Chat per design).
- `POST /api/forum/dms` body `CreateDmRequest` → `DmSummary` (may create/resolve Matrix DM bridge target; implementation Phase 4).
- `GET /api/forum/dms/{dmId}/messages?cursor&limit` → `Page<DmMessage>`
- `POST /api/forum/dms/{dmId}/messages` body `CreateDmMessageRequest` → `DmMessage`

`DmSummary` includes `id`, `participants[]`, `chatRoomId?`, `matrixRoomId?`, `bridgeState` (`local_only` | `pending_bridge` | `bridged` | `degraded`), `lastMessageAt?`. UI must label bridge state clearly; DMs appearing in LosOS Chat remain the Phase 4 acceptance criterion.

### `@LosOS` mentions (Phase 2)

Mention parsing is represented in message/post contracts via `mentions[]` entries of type `assistant` with target `losos`. API surface:
- `POST /api/forum/assistant/preview` (optional, Phase 2) body `AssistantPreviewRequest` → validation/rate-limit/context summary; no provider call required in scaffold.
- Assistant responses appear as `Post`/`ChannelMessage` with `provenance: { kind: "assistant", provider, model?, requestId?, promptedByMessageId, generatedAt, disclaimer }`.
- Disabled/configured state comes from `GET /api/forum/status.features.assistant`.

### Moderation primitives (Phase 1)

Minimal actions exposed via topic/post/message moderation endpoints: lock/unlock, pin/unpin, hide/delete/restore (soft where possible), tag changes, move topic/channel. Requests carry `reason?` and require appropriate permissions; responses include updated resource + `moderationEventId?`.

## UI implementation plan (React now, Dioxus coordinated)

### Direction

- **Phase 1 delivery target is the current React admin UI** (`admin-ui/app`), because it is the shipped/admin surface in this branch and PR #522’s Dioxus rewrite is an early, separate app not present in `feat/losos-forum-scaffold`.
- **Contracts are shared by design**: keep request/response shapes framework-agnostic (this spec + `forum_contracts.rs` + `forum/contracts.ts`). The React placeholder consumes the TS contracts; Dioxus (PR #522) should consume equivalent Rust/serde contracts (prefer moving/deriving from `backend/src/forum_contracts.rs` or a shared crate when the Dioxus app lands/integrates).
- **No duplication of product logic**: UI logic for API mapping, gate handling, mention/provenance rendering rules, and moderation affordances should be specified once (this doc) and implemented as thin framework adapters. Avoid copying complex behavior between React and Dioxus; share types first, then evaluate a shared `forum-client` core (Rust/WASM or generated types) when PR #522 matures.

### PR #522 (Dioxus rewrite) coordination

PR #522 (`origin/t3/rewrite-frontend-dioxus`) adds `admin-ui/dioxus-app` (Dioxus 0.6, router, `losos-client` path dep, shared TLS/trust primitives) with routes in `src/routes.rs` and API/actions in `src/api.rs`/`src/actions.rs`. Coordination rules:
1. **Do not modify or depend on PR #522 files from this branch**; `admin-ui/dioxus-app` is absent here. Changes in this branch must not break React admin UI or create merge conflicts with PR #522 except additive documentation/contracts where appropriate.
2. **Add a Dioxus Forum route only in the PR #522 branch/app** when implementing there: e.g. `Route::Forum {}` → `/forum` (and later nested routes) using `api.rs`/`actions.rs` patterns and design tokens (`assets/tokens.css`). Mirror the same contract shapes and status/gating behavior.
3. **Reuse design system tokens**: React uses `admin-ui/design-system/tokens.css` and `components/ui/*`; Dioxus already imports/aligns tokens in PR #522. Forum UI must use those tokens/components, not ad hoc colors.
4. **Shared contract source of truth**: when Dioxus implementation starts, either (a) reference/copy the Rust contract types from `backend/src/forum_contracts.rs` into a shared crate used by backend and `dioxus-app`, or (b) generate TS/Rust from a single schema. Until then, keep `forum_contracts.rs` and `forum/contracts.ts` structurally aligned and update both in the same PR/commit when contracts change.
5. **Migration path**: React `/forum` placeholder remains functional until Dioxus reaches feature parity and the product switches admin UI targets. Feature flags/gates (`GET /api/forum/status`) control visibility regardless of framework.

### React (current admin UI) implementation notes

- Place forum feature code under `admin-ui/app/src/forum/` (`contracts.ts`, API client helpers, components, screens/hooks as needed).
- Add a lazy-loaded route in `admin-ui/app/src/App.tsx` for `/forum` (and later nested paths). Keep it non-breaking and behind the normal shell/auth patterns.
- Use existing UI primitives (`components/ui/card`, `badge`, `empty`, `button`, `field`, etc.) and app-shell patterns; follow `screen`/chunk loading style used by Mail/Chat/Wikis.
- API client calls go through `src/lib/api.ts` conventions (`call(...)`, error handling including gate flags). Add `getForumStatus()`/forum calls in a focused way; do not refactor unrelated API code.
- Navigation: add a sidebar/nav entry only if it fits current IA and gate behavior (feature-gated by forum status/enablement). Placeholder may be reachable by direct route first.

### Dioxus (PR #522) future implementation notes

- Add `forum` module under `admin-ui/dioxus-app/src/screens/` or `pages/` consistent with PR #522 conventions; register `Route::Forum` in `src/routes.rs`.
- Implement API fetch/actions in `src/api.rs`/`src/actions.rs` (or a forum submodule) using `reqwest`/serde models matching `forum_contracts.rs`.
- Render status/gates, spaces/topics/channels placeholders, and provenance/bridge-state labels. Reuse `components/ui` and tokenized CSS.

## Minimal placeholder added in this branch (safe)

- React route `/forum` renders a lazy Forum scaffold page using existing design-system UI components. It displays Phase 0/contract status information and does not call unimplemented backend endpoints by default except where safely optional/fakeable; it is explicitly a shell/placeholder.
- Contracts added: `backend/src/forum_contracts.rs`, `admin-ui/app/src/forum/contracts.ts`, this spec.
- No Dioxus files added/modified (PR #522 owns `admin-ui/dioxus-app`).

## Test/check guidance

- React/admin-ui: `cd admin-ui/app && bun --bun tsc --noEmit` (or targeted checks) for changed TS files; avoid running full unrelated browser suite unless needed.
- Rust backend: `cargo test`/`cargo check` for targeted crates if feasible in environment; at minimum keep `forum_contracts.rs` compiling as a module (serde derives) and run formatting/checks available.
- Nix/module eval not required for TS/Rust contract-only additions.

## Open follow-ups

- Finalize mesh-join gate vs proxy+edge-only gate in status/contracts.
- Choose real-time transport (SSE vs WebSocket) and event auth/reconnect semantics.
- Decide shared contract packaging (shared Rust crate / schema generation) before duplicating models across backend, React codegen, and Dioxus.
- Wire backend `/api/forum/*` routes and real services in Phase 1+; connect React/Dioxus pages to live data.
