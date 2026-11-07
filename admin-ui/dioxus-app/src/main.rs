//! LosOS cross-platform frontend (Dioxus).
//!
//! Started as a rewrite of the Kotlin companion (`admin-ui/companion/`) to
//! target web, desktop, and mobile from one Rust codebase. It reuses patterns
//! from the TUI client (`admin-ui/tui`): TLS trust/pinning semantics,
//! formatting, and the box API surface. The transport here is the bearer-token
//! JSON API used by the admin SPA and companion (`/api/*`).

use losos_frontend::app;

fn main() {
    dioxus::launch(app::App);
}
