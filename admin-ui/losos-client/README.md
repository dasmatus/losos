# losos-client (shared primitives)

Shared, cross-platform Rust primitives for LosOS clients:
- TLS trust modes (`Trust`), aligned with `admin-ui/tui/src/http.rs`
- JSON/formatting helpers used by the TUI and the Dioxus frontend

Used by `admin-ui/dioxus-app`. Intended to be adopted by `admin-ui/tui` (replacing duplicated formatting/trust definitions and enabling a shared async-capable HTTP core) so code is reused across terminal and GUI frontends.
