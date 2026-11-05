# Matrix client compatibility (Element and other Matrix clients)

**Status:** implementation foundation landed in this worktree (2026-11-05). Not end-to-end verified on a running LosOS box.

## Goal

Make LosOS Chat usable from standard Matrix clients (Element Web/Desktop and other Matrix-spec clients), not only the built-in LosOS web chat.

## Current architecture (before this work)

- Synapse runs on loopback; LosOS web chat uses `matrix-js-sdk` against the same origin.
- No Matrix passwords; accounts/devices are provisioned via lososd as an application service (`m.login.application_service`).
- `/_matrix/client/` and `/_matrix/media/` are gated by nginx `auth_request` to `/internal/chat/authz` (`modules/chat.nix`, `backend/src/http.rs:get_chat_authz`, `backend/src/chat.rs:authorize`). The gate required a live LosOS Login/Authelia session cookie and enforced that any Matrix bearer token belonged to the same LosOS person as that cookie.
- Synapse had `password_config.enabled = false` and `enable_registration = false`.

## What this foundation changes

### 1. Split authz path (`backend/src/chat.rs`)

`Chat::authorize` now supports two LosOS-controlled paths:

- **LosOS web chat (cookie present):** unchanged. Requires a live LosOS Login session, a current non-locked local person, and (when a Matrix token is present) the token must belong to that same person. Muted users may still read but cannot perform “speaking” actions.
- **External Matrix clients (no LosOS cookie, Matrix bearer token present):** allowed only when the token resolves (`whoami`) to an active, non-disabled-by-state, non-locked local LosOS person on this box. Muted users cannot perform speaking actions. Unknown tokens are passed through so Synapse can reject them with its own error.
- **Unauthenticated standard client entry points** (no cookie, no token): `GET /_matrix/client/versions`, Matrix login/SSO discovery/login flows, and `POST login|refresh|register|logout` under `/_matrix/client/` are allowed through to Synapse. Synapse still enforces its own auth/policy (registration remains disabled).

Security invariant preserved: LosOS person state remains authoritative for local access decisions; tokens cannot be used as a different local person than they belong to.

### 2. Matrix client password management (`backend/src/chat.rs`, `backend/src/http.rs`)

- `Chat::set_matrix_password(person, password)` sets/rotates a local person’s Matrix password via the client API using an application-service-authenticated flow, then logs out devices (`logout_devices: true`). Locked people are refused.
- `POST /api/chat/client-password` (authenticated LosOS person, same guard style as other chat routes): `{"password":"..."}` sets a password; `{"rotate":true}` generates and returns a fresh password once. The returned password is shown only on rotation responses; setting an explicit password returns `password: null`.

This is the MVP credential mechanism (assumption D): a LosOS-managed Matrix credential/password for standard clients, revocable by rotation. **Multi-app-password/named-device-password management is not implemented yet.**

### 3. Synapse config (`modules/chat.nix`)

- `password_config.enabled = true` so `m.login.password` can work for accounts that have a LosOS-set password. `enable_registration` remains `false`. Accounts still have no password until LosOS sets one.

### 4. Existing flows preserved

- `/api/chat/session` appservice login for the LosOS web chat is unchanged.
- `/_matrix/federation/`, `/_matrix/key/`, and `.well-known` behavior remain as configured.
- nginx still uses `auth_request` for `/_matrix/client/` and `/_matrix/media/`; lososd now makes the compatibility decisions.

## Assumptions / owner decisions needed

These were assumed to make progress; confirm or override:

1. **Auth approach:** D (LosOS-issued/revocable app credentials + split authz). C (OIDC/SSO via Authelia/Synapse) deferred pending Authelia capability/deployment review.
2. **MVP clients/features:** Element Web/Desktop-class Matrix clients; discovery/login/sync/messages/rooms/invites/media/basic E2EE. **Out of scope for MVP:** mobile push notifications (needs push gateway/Sygnal integration), Element Call / MatrixRTC parity beyond existing experiments, named multi-device app passwords.
3. **Network/TLS:** should work for both public `serverName` deployments and LAN `*.local` deployments in principle, but `.local` + self-signed/private CA requires client trust setup. `/.well-known/matrix/client` points at `https://<serverName>`.
4. **Lifecycle/security:** removal/disable/lock should block external-client access; moderation speaking restrictions apply. Password rotation logs out devices. A dedicated “revoke all Matrix client credentials” UX/policy may still be desirable.

## How to use (expected, once deployed)

1. Ensure chat is enabled and the box is reachable at its `serverName` (and TLS/certificates are trusted by the client, especially on LAN/`.local`).
2. As a signed-in LosOS person, set/rotate a Matrix client password:
   ```sh
   # set explicit password
   curl -fsS -X POST -H 'Content-Type: application/json' \
     --cookie 'losos_session=...' \
     -d '{"password":"...strong password..."}' \
     https://<box>/api/chat/client-password

   # rotate and receive a generated password once
   curl -fsS -X POST -H 'Content-Type: application/json' \
     --cookie 'losos_session=...' \
     -d '{"rotate":true}' \
     https://<box>/api/chat/client-password
   ```
3. In Element (or another Matrix client), sign in manually:
   - Homeserver: `https://<serverName>` (or the URL that serves `/.well-known/matrix/client`)
   - Username: your LosOS login name (localpart of `@name:server`)
   - Password: the Matrix client password you set/rotated

## Limitations / known gaps

- **Not end-to-end verified** in this environment (no running Synapse/Authelia/nginx/Element). Live validation is required.
- **Single password per account** in Synapse; no per-device named app-password records in LosOS yet.
- **No admin UI** for setting/rotating the Matrix client password yet; API-only.
- **Mobile push** is not configured.
- **Calls:** LosOS has its own SFU/call integration and Synapse MatrixRTC experiments; compatibility with Element’s call flows is not guaranteed and is out of MVP scope.
- **Cookie-less 401 UX:** nginx still maps authz 401 to `@losos-chat-sign-in` (LosOS web-oriented message). Acceptable for protocol operation but may be confusing in some client error displays; can be refined.
- **Disabled-person detection** for external token path relies on current person state from `people` (and lock groups/marks). Review against the exact `people.get`/disabled semantics used in production.
- **`set_matrix_password` implementation detail:** uses client API password change with an application-service-obtained token and `auth.type=m.login.application_service`. Verify against the deployed Synapse version; fallback options include Synapse admin API (kept loopback-only/not nginx-exposed).

## Verification performed (this session)

- `cargo test --manifest-path backend/Cargo.toml --lib chat` passed after changes (22 passed, 0 failed in the filtered set; existing chat-related tests remained green, plus new tests for external-client authz and password setting).
- `nix-instantiate --parse` succeeded for `modules/chat.nix` and `modules/options.nix`.
- Unrelated minimal compile blockers encountered in the broader lib test build (`backend/src/cloud/office.rs` zip `name()` handling, `backend/src/office/mod.rs` test move) were fixed only to unblock verification; review those changes separately.

## Next steps

1. Owner confirms scope/auth/network decisions above.
2. Live LAN test: set client password, sign in with Element Web/Desktop, verify sync, DM, room, media, E2EE, logout, lock/disable/removal behavior.
3. Add admin UI for Matrix client password management (and optionally named/revocable app passwords).
4. Evaluate OIDC/SSO (C) if Authelia + deployment naming/TLS support it.
5. If mobile is in scope, design push gateway integration.
6. If Element-call compatibility is required, treat as a separate MatrixRTC/call compatibility track.
