//! `losos`, the terminal client for LosOS boxes and edges: the web UI's
//! panes in a terminal, and a few commands for scripts. A box signs in
//! through LosOS Login (docs/terminal-access.md); an edge still uses the
//! signed envelope.

/// The crate's version, or what the build names it (`LOSOS_VERSION`).
/// Kept at the root so the CLI/API surface can name it without the TTY UI
/// tree (which is gated out on wasm32).
pub const VERSION: &str = match option_env!("LOSOS_VERSION") {
    Some(v) => v,
    None => env!("CARGO_PKG_VERSION"),
};

pub mod agents;
pub mod box_session;
pub mod cli;
pub mod commands;
pub mod computer;
pub mod envelope;
pub mod http;
pub mod i18n;
pub mod keychain;
pub mod logging;
pub mod oidc;
pub mod problem;
pub mod prompt;
pub mod theme;
#[cfg(not(target_arch = "wasm32"))]
pub mod tui;
pub mod vault;
