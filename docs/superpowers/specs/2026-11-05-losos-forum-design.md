# LosOS Forum — Design Spec (Phase 0 scaffold + Phase 1 foundation)

- Status: **Phase 0 scaffold implemented; Phase 1 backend/UI foundation integrated in PR stack; owner decisions resolved for v1 direction** (Phase 2–5 runtime still largely not implemented — only verifiable contract/guard slices added incrementally)
- Decisions update: 2026-11-07 (owner-directed): LosOS Chat is Matrix-first; forum private DMs are side chats spawned/linked in LosOS Chat; `@LosOS` v1 uses guarded configurable provider interface (default disabled, provenance/rate-limit hooks); ActivityPub v1 is adapter/safety-guard contracts only with no runtime federation; enablement gate remains proxy+admin UI provisional.
- Date: 2026-11-05 (updated 2026-11-07)
- Source: T3 item "Build LosOS Forum" + owner direction (2026-11-05 session) + implementation evidence (2026-11-07)

## Implementation status (evidence)

| Area | Status | Evidence / PRs |
|------|--------|----------------|
| Phase 0 — Spec/scaffold | **Done (in review)** | https://github.com/dasmatus/losos/pull/530 (`feat/losos-forum-scaffold`): spec, `modules/forum.nix`, `modules/options.nix`/`flake.nix` wiring, `backend/src/forum_scaffold.rs` placeholder, `admin-ui/app/src/forum/README.md` placeholder, `backend/sections.json` route-table consistency fix. |
| Phase 1 — Core hybrid foundation | **Implemented in PR stack (CI/e2e validation pending/authoritative)** | https://github.com/dasmatus/losos/pull/539 (**MERGED** 2026-11-07T14:25:14Z, `d844d118`): backend foundation + guarded `/api/forum*` scaffolding/principal fixes. https://github.com/dasmatus/losos/pull/538 (partial integration, open): contracts + edge/federation hook plumbing + safe wiring. https://github.com/dasmatus/losos/pull/541 (full integration, open): `backend/src/forum/{mod,models,storage,authz}.rs`, guarded `/api/forum*` routes/status/spaces/channels/topics/posts/messages/reactions/memberships, React admin UI `admin-ui/app/src/forum/{api,contracts,page}.tsx`, AP/Matrix runtime still stub-only. |
| Phase 2 — `@LosOS` | **Hook/stub only** | Mention hook/config surface exists; full provider behavior, authz/rate limits, context policy, provenance/audit runtime **not implemented**. |
| Phase 3 — ActivityPub | **Stub contracts only** | No WebFinger runtime, no actor/key HTTP signature verification, no inbox/outbox delivery/fetch/queue/retries runtime. |
| Phase 4 — Matrix DM bridge to LosOS Chat | **Stub contracts only** | No appservice/sync/identity provisioning/E2EE bridge runtime; DM appearance in `/chat/` not implemented. |
| Phase 5 — Hardening | **Not implemented** | Backups/restore, migrations, quotas, moderation/admin tools, audits, CSP/rate limits, handbook VM/integration tests pending. |

## Goal

Build a **from-scratch LosOS Forum**: a hybrid of **Discourse-style topics** and **Slack-style channels/real-time conversation**, deeply integrated with LosOS.

## Settled constraints

1. **Edge-local routing.** Forum/federation traffic is intended to use the **edge the user/box is located in** (selected edge path; see `backend/src/edge.rs`, `modules/proxy.nix`, `GET /api/edge`). The design must respect edge failover, bans, and custom-domain hold behavior (`handbook/docs/in-depth/edge-federation.md`). *Phase 1:* hook/config contracts exist; full edge route registration/failover for forum federation runtime is not implemented.
2. **Federation protocols.** Federate with **ActivityPub** and with **Matrix for DMs**. *Phase 1:* contracts/stubs only; no runtime federation/bridge handlers.
3. **DMs appear in LosOS Chat (Matrix-first, decided).** LosOS Chat is **Matrix-first**. Forum private DMs act as **side chats**: creating or using a private forum DM spawns/links a conversation in LosOS Chat (Matrix DM room mapping/contract), not an isolated store (`modules/chat.nix`, `backend/src/chat.rs`, `/chat/`). *Decision resolved; full Synapse appservice/bridge runtime and E2EE-correct behavior remain implementation follow-ups behind verified contracts.*
4. **Edge proxy network.** Federation/bridged traffic goes through the **edge proxy network** (tunnel/Traefik/registrar-routed paths). No ad hoc unsolicited WAN listeners on the box. *Enforced as a design constraint; runtime paths not implemented.*
5. **Mesh/edge dependency (decided, provisional v1 default).** Forum enablement gates on **`losos.proxy.enable` + admin UI presence** (`losos.admin.enable` and `losos.admin.ui`) and exposes status/config hooks. **`losos.cluster.enable`/mesh join is not a hard gate in v1 by default.** Edge proxy network remains the transport constraint. Owner may tighten to require cluster/mesh join later.

## Default product assumptions (confirm/override)

- One product, two interaction modes over a canonical model: **topics** (threaded, categorized/tagged, long-form/moderation) and **channels** (real-time timeline/threads/reactions). *Phase 1 foundation provides spaces/channels/topics/posts/messages models/routes/UI; full moderation primitives, realtime WebSocket/SSE, tags, and rich thread semantics remain follow-ups.*
- Identity uses **LosOS Login**; federated identities map to stable LosOS/AP/Matrix identifiers. *Phase 1 uses guarded principals/forum principal mapping; full federated identity mapping not implemented.*
- `@LosOS` mention invokes an **assistant actor** (like mentioning Grok on X). **v1 decided:** guarded configurable provider interface; **default disabled/stub**; configured external provider hook supported by contract; responses must carry **provenance metadata**; **authz/rate-limit/context-policy hooks** required. *Implementation is incremental:* contracts/guards/stubs may land before a production provider.
- **LosOS Chat is Matrix-first (decided).** Forum coexists with LosOS Chat; private forum DMs are side chats that **spawn/link conversations in LosOS Chat** via Matrix DM mapping contracts. Full bridge/appservice runtime and E2EE semantics remain follow-ups.

## Architecture (draft)

```text
Browser/UI (/forum/) + admin UI
  -> nginx front door + LosOS Login/authz
     -> losos-forum service (new, first-party)
        - canonical model: spaces/categories, topics, channels, threads, messages,
          reactions, tags, mentions, DMs
        - realtime: WebSocket/SSE
        - @LosOS mention handler -> assistant provider (pluggable)
        - ActivityPub adapter (actors, webfinger, inbox/outbox, HTTP signatures)
          via selected edge/proxy path
        - Matrix DM adapter/bridge <-> LosOS Chat/Synapse-visible DMs
          via edge proxy network
        - edge routing: current edge discovery, route registration, failover/ban/hold
     -> lososd: config/status, enablement gate, credentials
```

Implementation form: prefer a **first-party host service/module** (patterns from `modules/chat.nix`, `modules/git.nix`, `modules/apps.nix`) rather than an off-the-shelf app, because of deep integration, `@LosOS`, federation adapters, and mesh/edge coupling. Exact language/runtime is an implementation choice; Rust aligns with `backend/`.

## Phase plan

### Phase 0 — Spec/scaffold — **Done (in review, PR #530)**
- [x] Record constraints/defaults/open decisions.
- [x] Add minimal `modules/forum.nix` + `losos.forum.*` options + wiring (`modules/options.nix`, `flake.nix`).
- [x] Add placeholder status/API/UI hooks where feasible (`backend/src/forum_scaffold.rs`, admin placeholder, `backend/sections.json` consistency).
- [x] Verify with real checks where feasible; commit/push/PR (https://github.com/dasmatus/losos/pull/530).

### Phase 1 — Core hybrid foundation — **Implemented in PR stack (validation pending/CI authoritative)**
- [x] Canonical foundation store + guarded APIs for spaces/channels/topics/posts/messages/reactions/memberships (`backend/src/forum/*`, `/api/forum*` routes) — see #539 (merged), #538/#541 (open).
- [x] Basic admin UI for spaces/channels/topics/posts/messages with status/gate/loading/empty/error states and compose forms (`admin-ui/app/src/forum/*`, React admin app, PR #541). Framework-agnostic contracts coordinated with Dioxus rewrite https://github.com/dasmatus/losos/pull/522.
- [ ] Real-time updates (WebSocket/SSE).
- [ ] Full LosOS Login permissions/moderation primitives, pagination, and reconciliation of `forum_contracts` vs runtime models.

### Phase 2 — `@LosOS` — **Decision resolved; implementation incremental (guards/stubs before production provider)**
- [ ] Mention parsing runtime behavior, authz/rate limits, context policy.
- [ ] Provider interface + configured provider runtime; provenance/audit enforcement; production provider. *v1 direction decided (guarded configurable, default disabled, provenance/rate-limit hooks); runtime remains incremental.*

### Phase 3 — ActivityPub — **Decision resolved; v1 runtime disabled by default (adapter/safety guards only until security review)**
- [ ] WebFinger/actors/keys, inbox/outbox, HTTP signatures, object mapping.
- [ ] Delivery queue/retries/idempotency, abuse controls.
- [ ] Exposure/routing through selected edge proxy path. *v1 direction decided: adapter/contracts/safety guards only now; no runtime inbox/outbox/delivery/fetch/signature verification; public Fediverse reachability deferred until security review + explicit enablement.*

### Phase 4 — Matrix DM bridge to LosOS Chat — **Decision resolved (Matrix-first side chats); runtime incremental**
- [ ] Map Forum DMs to Matrix DM rooms/users visible in existing Chat/Synapse.
- [ ] Route DM federation through edge proxy network.
- [ ] Verify appearance in `/chat/`; document E2EE/authz implications.
- [ ] Evaluate/implement mechanism (appservice/bridge/lososd-mediated sync) against `backend/src/chat.rs` constraints. *Direction decided: Matrix-first side chats spawned/linked in LosOS Chat; contract allows limited E2EE for bridged/forum-spawned side chats until verified; full mechanism/E2EE implementation deferred.*

### Phase 5 — Hardening — **Not implemented**
- [ ] Backups/restore, migrations, quotas, moderation/admin tools, audits, CSP/rate limits.
- [ ] Handbook/in-depth docs and VM/integration tests (including edge-path/federation cases).

## Decisions (resolved for v1 direction, 2026-11-07)

| # | Decision | Resolved direction | Implementation status / follow-ups |
|---|----------|--------------------|-----------------------------------|
| 1 | **Enablement gate** | **Proxy+admin UI provisional default.** `losos.proxy.enable` + admin UI required; `losos.cluster.enable`/mesh join not hard-required in v1. Edge proxy network remains transport constraint. | Implemented as current gate behavior; owner may tighten later. |
| 2 | **Chat relationship** | **LosOS Chat is Matrix-first.** Forum private DMs act as **side chats** spawned/linked in LosOS Chat (Matrix DM conversation mapping), not an isolated store. | Decision resolved; full bridge/appservice runtime and verified UX in `/chat/` remain follow-ups. |
| 3 | **`@LosOS` v1 backend** | **Guarded configurable provider interface.** Default **disabled/stub**; configured external provider hook allowed by contract; responses carry **provenance**; **authz/rate-limit/context-policy hooks** required. | Incremental: contracts/guards/stubs can land before production provider runtime. |
| 4 | **ActivityPub v1 scope/reachability** | **Adapter/safety-guard contracts only.** No runtime inbox/outbox/delivery/fetch/signature verification enabled by default. Public Fediverse reachability deferred until security review + explicit owner enablement. | Runtime federation remains unimplemented; treat as security-sensitive. |
| 5 | **Matrix DM mechanism/E2EE** | **Matrix-native integration via LosOS Chat/Synapse-compatible contract** (appservice/bridge/lososd-mediated path evaluated against `backend/src/chat.rs`). v1 contract allows **limited E2EE for bridged/forum-spawned side chats until verified**; sensitive cases must be explicit. | Full mechanism and E2EE-correct behavior deferred pending implementation/testing. |

## Risks

- Matrix DM bridge into existing Synapse/LosOS Chat is integration-sensitive (auth, device/session, E2EE semantics).
- Edge path changes/bans/custom-domain holds affect federation reachability.
- ActivityPub + Matrix are different protocol models; adapters must stay explicit.
- Mention-triggered AI and federation inboxes are abuse/prompt-injection surfaces.
- Mini-PC resource budget (realtime, queues, search, AI).
- Persistent/encrypted data placement and backup coverage must be correct.
- `forum_contracts` vs runtime model reconciliation and Phase 1 authz/moderation gaps can cause integration drift until addressed.

## Verification notes

- **Phase 0 scaffold:** verified by file/parse checks and PR #530 (scaffold spec/module/options/placeholders). CI is authoritative for full repo gates.
- **Phase 1 foundation:** implemented across PR stack (#539 merged backend foundation; #538 partial integration; #541 full integration + React admin UI). Evidence is recorded in PRs and `.opencode/goal-state.md`; **CI must be treated as authoritative** (cargo/Nix/admin UI/VM checks were not all completed locally/non-contended in session).
- **Federation/bridge:** intentionally stub-only — no AP signature verification/inbox/outbox/delivery/fetch and no Matrix appservice/sync/E2EE runtime. Any claim of federation/DM bridge functionality requires real runtime tests.
