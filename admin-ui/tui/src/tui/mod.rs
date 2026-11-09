//! The full-screen view: the web UI's panes as data, drawn with ratatui.
//!
//! The whole TTY rendering tree is gated out on wasm32: crossterm 0.29 has
//! no wasm32 sys backend (its terminal/cursor/event sys re-exports are
//! unix/windows only) and ratatui's CrosstermBackend cannot compile there, so
//! the browser/API leg must not pull this module set in. The wasm32 build
//! keeps the CLI/envelope/drive surface; interactive screens answer with the
//! needs-terminal refusal.

#[cfg(not(target_arch = "wasm32"))]
pub mod app;
#[cfg(not(target_arch = "wasm32"))]
pub mod draw;
#[cfg(not(target_arch = "wasm32"))]
pub mod edge;
#[cfg(not(target_arch = "wasm32"))]
pub mod form;
#[cfg(not(target_arch = "wasm32"))]
pub mod login;
#[cfg(not(target_arch = "wasm32"))]
pub mod model;
#[cfg(not(target_arch = "wasm32"))]
pub mod panes;
#[cfg(not(target_arch = "wasm32"))]
pub mod resources;
#[cfg(not(target_arch = "wasm32"))]
pub mod term;
