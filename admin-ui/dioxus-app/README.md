# LosOS Frontend (Dioxus) — rewrite start

This directory starts rewriting the LosOS frontend to [Dioxus](https://dioxuslabs.com/) so the UI can target **web, desktop, Android, and iOS** from one Rust codebase and remove the Kotlin companion.

## What this replaces / unifies

- **`admin-ui/companion/`** — Android app (Kotlin/Jetpack Compose, ~2.8k LOC). The primary target for elimination: same box-linking model (address + access token), LAN/TLS behavior, and pane-based admin surface.
- **`admin-ui/tui/`** — Rust terminal client. This frontend **reuses its patterns and shared crates** rather than reimplementing from scratch:
  - TLS trust/pinning semantics are modeled after `admin-ui/tui/src/http.rs` (`Trust`: public CA, pinned/TOFU, permissive for LAN self-signed certs as a starting point).
  - Formatting/helpers follow the TUI/companion style (`format_bytes`, stamps, summaries).
  - `backend/access` (`losos-access`) is available via the `access` feature for the signed/encrypted envelope path used by terminal access (`/api/access/call`). The current companion-style UI uses the bearer-token JSON API (`/api/*`), matching the admin SPA.
  - **Next step for deeper reuse:** extract a shared cross-platform client core (HTTP + trust + API models) that both `admin-ui/tui` and this frontend depend on, so `tui/src/http.rs`, `tui/src/commands.rs` logic, and pane data models are shared without pulling `ratatui` into mobile/web builds.

## Status (started)

- [x] Dioxus project scaffold (`Cargo.toml`, `Dioxus.toml`, assets, feature-gated GUI).
- [x] Cross-platform core (`api`, `store`) compile-able without GUI renderer deps.
- [x] Bearer-token `BoxApi` client (port of companion `ApiClient.kt` behavior) with TUI-aligned trust modes.
- [x] Config persistence: `localStorage` on web, config file on native (replaces SharedPreferences conceptually).
- [x] Connect flow, tabbed dashboard shell, overview with real `GET /api/health|status|storage`.
- [x] Generic route panes (read) for the companion/TUI pane set; settings/disconnect.
- [ ] Full TOFU certificate hash verification on native (mirroring `tui/src/http.rs`) — currently permissive for self-signed LAN certs, pin stored for hardening.
- [ ] mDNS `_losos._tcp` discovery (companion `Discovery.kt`) via platform integrations.
- [ ] Write actions: storage grow, app install/remove/search/chart, config sync, backup run.
- [ ] Deeper browsing (cloud files path navigation), auth/edge panes as needed.
- [ ] Mobile platform packaging (Android/iOS projects via `dx`/Dioxus mobile tooling), icons/branding from `admin-ui/themes/brand`.
- [ ] Shared client-core extraction with `admin-ui/tui`.

## Build

Core (no GUI renderer, useful for CI/sandbox checks):

```sh
cd admin-ui/dioxus-app
cargo check --no-default-features
cargo test --no-default-features
```

Desktop (needs system webview deps on Linux, e.g. webkit2gtk/gtk — use the project devenv/Nix environment):

```sh
cargo run            # default: gui + desktop
# or: dx serve --platform desktop
```

Web:

```sh
rustup target add wasm32-unknown-unknown
dx serve --platform web
# or: cargo build --no-default-features --features web --target wasm32-unknown-unknown
```

Mobile (requires Android/iOS SDKs and Dioxus mobile tooling):

```sh
dx serve --platform android
dx serve --platform ios
```

> Heavy builds/toolchain work should go through the repo's work queue when applicable (`tq submit ...`).

## Structure

- `src/api.rs` — `BoxApi`, trust modes, JSON helpers, formatting (reuses TUI/companion patterns).
- `src/store.rs` — `BoxConfig` + platform persistence.
- `src/app.rs`, `src/screens/*` — Dioxus UI (connect, dashboard, panes).
- `assets/main.css` — tokens ported from `admin-ui/design-system/tokens.css` / companion theme.
- `Dioxus.toml` — app/bundle config (`us.dasmat.losos.frontend`).

## Design notes

- The companion is the clearest Kotlin-removal target; this rewrite ports its behavior first, then expands to cover admin surfaces.
- The large web admin UI (`admin-ui/app/`, React/TS) is a separate, much larger migration; this Dioxus app establishes the cross-platform Rust UI foundation.
- Security/TLS: preserve the TUI's intent (no silent trust downgrade). The current `danger_accept_invalid_certs` path for LAN boxes is a temporary implementation detail to be replaced with certificate hash verification as in `tui/src/http.rs`.
