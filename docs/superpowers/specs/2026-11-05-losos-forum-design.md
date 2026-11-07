# LosOS Forum — Design Spec (Phase 0 scaffold + Phase 1 foundation)

- Status: **Phase 0 scaffold implemented; Phase 1 backend/UI foundation integrated in PR stack** (Phase 2–5 not implemented)
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
3. **DMs appear in LosOS Chat.** DM conversations handled/federated by the Forum’s Matrix DM path must also appear in existing LosOS Chat (`modules/chat.nix`, `backend/src/chat.rs`, `/chat/`). This requires a bridge/sync design, not an isolated DM store. *Not implemented.*
4. **Edge proxy network.** Federation/bridged traffic goes through the **edge proxy network** (tunnel/Traefik/registrar-routed paths). No ad hoc unsolicited WAN listeners on the box. *Enforced as a design constraint; runtime paths not implemented.*
5. **Mesh/edge dependency (implemented default, provisional).** Current scaffold/integration gates forum enablement on **`losos.proxy.enable` + admin UI presence** (`losos.admin.enable` and `losos.admin.ui`) and exposes status/config hooks. **`losos.cluster.enable`/mesh join is not a hard gate in the current implementation.** Owner decision still required: keep proxy+edge/admin gate only, or also require cluster/mesh join for v1.

## Default product assumptions (confirm/override)

- One product, two interaction modes over a canonical model: **topics** (threaded, categorized/tagged, long-form/moderation) and **channels** (real-time timeline/threads/reactions). *Phase 1 foundation provides spaces/channels/topics/posts/messages models/routes/UI; full moderation primitives, realtime WebSocket/SSE, tags, and rich thread semantics remain follow-ups.*
- Identity uses **LosOS Login**; federated identities map to stable LosOS/AP/Matrix identifiers. *Phase 1 uses guarded principals/forum principal mapping; full federated identity mapping not implemented.*
- `@LosOS` mention invokes an **assistant actor** (like mentioning Grok on X). v1 target: pluggable provider interface; **disabled until configured**; responses carry provenance. *Current implementation:* mention hook/config surface exists; full provider runtime, authz/rate limits, context policy, and audited/provenance responses are **not implemented** (owner decision required for first provider).
- Coexists with existing LosOS Chat in v1; DMs are bridged so they appear in Chat. *Owner decision/implementation pending; no bridge runtime.*

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

### Phase 2 — `@LosOS` — **Not implemented (hook/stub only)**
- [ ] Mention parsing runtime behavior, authz/rate limits, context policy.
- [ ] Provider interface + configured provider; provenance/audit. *Owner decision required for v1 backend.*

### Phase 3 — ActivityPub — **Not implemented (stub contracts only)**
- [ ] WebFinger/actors/keys, inbox/outbox, HTTP signatures, object mapping.
- [ ] Delivery queue/retries/idempotency, abuse controls.
- [ ] Exposure/routing through selected edge proxy path. *Owner decision required for v1 scope/reachability.*

### Phase 4 — Matrix DM bridge to LosOS Chat — **Not implemented (stub contracts only)**
- [ ] Map Forum DMs to Matrix DM rooms/users visible in existing Chat/Synapse.
- [ ] Route DM federation through edge proxy network.
- [ ] Verify appearance in `/chat/`; document E2EE/authz implications.
- [ ] Evaluate mechanism (appservice/bridge/lososd-mediated sync) against `backend/src/chat.rs` constraints. *Owner decision required for mechanism/E2EE.*

### Phase 5 — Hardening — **Not implemented**
- [ ] Backups/restore, migrations, quotas, moderation/admin tools, audits, CSP/rate limits.
- [ ] Handbook/in-depth docs and VM/integration tests (including edge-path/federation cases).

## Open decisions (owner decision required)

| # | Decision | Current implementation default (provisional) | Owner input needed |
|---|----------|-----------------------------------------------|--------------------|
| 1 | **Enablement gate** | Gates on `losos.proxy.enable` + admin UI (`losos.admin.enable`, `losos.admin.ui`); status/config hooks exposed. `losos.cluster.enable`/mesh join **not** required by current implementation. | Confirm: proxy+edge/admin only for v1, or also require `losos.cluster.enable`/mesh join? |
| 2 | **Chat relationship** | Coexists conceptually with LosOS Chat; DMs intended to appear in Chat, but **no bridge runtime implemented**. | Confirm: coexist with bridge (v1) vs long-term convergence path? |
| 3 | **`@LosOS` v1 backend** | Mention hook/config surface exists; provider/runtime behavior **disabled/stub-level**, no provenance/audit runtime. | Choose first concrete provider: stub-only, configured external, or edge/mesh/local inference? Define authz/rate limits/context policy. |
| 4 | **ActivityPub v1 scope/reachability** | **Stub contracts only**; no runtime federation. No public Fediverse exposure implemented. | Decide which objects federate first (actors/topics/posts/reactions?), and whether v1 has public Fediverse reachability vs edge-local/private scope. |
| 5 | **Matrix DM mechanism/E2EE** | **Stub contracts only**; no appservice/sync/bridge runtime. | Choose mechanism (appservice/bridge/lososd-mediated sync) and acceptable E2EE/authz limitations for bridged DMs in v1. |

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
