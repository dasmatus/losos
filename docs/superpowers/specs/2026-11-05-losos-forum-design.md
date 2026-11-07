# LosOS Forum — Phase 0 Design Spec (draft)

- Status: draft / implementation started (scaffold)
- Date: 2026-11-05
- Source: T3 item "Build LosOS Forum" (Waiting on you / not settled) + owner direction in this session

## Goal

Build a **from-scratch LosOS Forum**: a hybrid of **Discourse-style topics** and **Slack-style channels/real-time conversation**, deeply integrated with LosOS.

## Settled constraints

1. **Edge-local routing.** Forum/federation traffic uses the **edge the user/box is located in** (selected edge path; see `backend/src/edge.rs`, `modules/proxy.nix`, `GET /api/edge`). The design must respect edge failover, bans, and custom-domain hold behavior (`handbook/docs/in-depth/edge-federation.md`).
2. **Federation protocols.** Federate with **ActivityPub** and with **Matrix for DMs**.
3. **DMs appear in LosOS Chat.** DM conversations handled/federated by the Forum’s Matrix DM path must also appear in existing LosOS Chat (`modules/chat.nix`, `backend/src/chat.rs`, `/chat/`). This requires a bridge/sync design, not an isolated DM store.
4. **Edge proxy network.** Federation/bridged traffic goes through the **edge proxy network** (tunnel/Traefik/registrar-routed paths). No ad hoc unsolicited WAN listeners on the box.
5. **Mesh/edge dependency.** The feature depends on the box being in the mesh/using its edge path. **Gate definition is an explicit decision:** v1 scaffold should gate on **proxy enabled + resolvable/current edge path** and expose status; owner to confirm whether hard `losos.cluster.enable`/mesh join is also required.

## Default product assumptions (confirm/override)

- One product, two interaction modes over a canonical model: **topics** (threaded, categorized/tagged, long-form/moderation) and **channels** (real-time timeline/threads/reactions).
- Identity uses **LosOS Login**; federated identities map to stable LosOS/AP/Matrix identifiers.
- `@LosOS` mention invokes an **assistant actor** (like mentioning Grok on X). v1: pluggable provider interface; **disabled until configured**; responses carry provenance. First concrete provider TBD (stub/configured/external/edge-mesh).
- Coexists with existing LosOS Chat in v1; DMs are bridged so they appear in Chat.

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

### Phase 0 — Spec/scaffold (this session starts)
- Record constraints/defaults/open decisions.
- Add minimal `modules/forum.nix` + `losos.forum.*` options + wiring.
- Add placeholder status/API/UI hooks where feasible.
- Verify with real checks; commit/push/PR.

### Phase 1 — Core hybrid
- Canonical store + APIs for spaces/topics/channels/threads/messages/reactions/tags.
- Real-time updates; basic topic/channel UI.
- LosOS Login permissions/moderation primitives.

### Phase 2 — `@LosOS`
- Mention parsing, authz/rate limits, context policy.
- Provider interface + stub/configured provider; provenance/audit.

### Phase 3 — ActivityPub
- WebFinger/actors/keys, inbox/outbox, signatures, object mapping.
- Delivery queue/retries/idempotency, abuse controls.
- Exposure/routing through selected edge proxy path.

### Phase 4 — Matrix DM bridge to LosOS Chat
- Map Forum DMs to Matrix DM rooms/users visible in existing Chat/Synapse.
- Route DM federation through edge proxy network.
- Verify appearance in `/chat/`; document E2EE/authz implications.
- Evaluate mechanism (appservice/bridge/lososd-mediated sync) against `backend/src/chat.rs` constraints.

### Phase 5 — Hardening
- Backups/restore, migrations, quotas, moderation/admin tools, audits, CSP/rate limits.
- Handbook/in-depth docs and VM/integration tests (including edge-path/federation cases).

## Open decisions (owner)

1. **Gate:** proxy+edge only, or also require `losos.cluster.enable`/mesh join?
2. **Chat relationship:** coexist with LosOS Chat (bridge DMs) vs long-term convergence?
3. **`@LosOS` v1 backend:** stub, configured external, or edge/mesh/local inference first?
4. **ActivityPub v1 scope:** which objects federate first; public Fediverse reachability in v1?
5. **Matrix DM mechanism/E2EE:** acceptable E2EE limitations for bridged DMs in v1?

## Risks

- Matrix DM bridge into existing Synapse/LosOS Chat is integration-sensitive (auth, device/session, E2EE semantics).
- Edge path changes/bans/custom-domain holds affect federation reachability.
- ActivityPub + Matrix are different protocol models; adapters must stay explicit.
- Mention-triggered AI and federation inboxes are abuse/prompt-injection surfaces.
- Mini-PC resource budget (realtime, queues, search, AI).
- Persistent/encrypted data placement and backup coverage must be correct.

## Verification notes (this session)

Scaffold work is intentionally minimal. Each milestone is verified by a real check (file existence, parse/eval, targeted build/check, or explicit manual evidence when automation is unavailable) and recorded in `.opencode/goal-state.md`.
