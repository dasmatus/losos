//! Core library for the LosOS Dioxus frontend.
//!
//! Exposes the cross-platform API client, config/state, and helpers so they
//! can be reused (and checked) independently of the GUI renderer. The GUI
//! modules are compiled when the `gui` feature is enabled.

pub mod api;
pub mod store;
#[cfg(not(target_arch = "wasm32"))]
pub mod tls;

#[cfg(feature = "gui")]
pub mod app;
#[cfg(feature = "gui")]
pub mod screens;

pub mod discovery;
