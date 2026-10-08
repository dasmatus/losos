//! LosOS Lab's canvas, drawn by Bevy on the GPU. The module this crate
//! compiles to also carries the Lab's core (`losos_lab_core::Lab`, exported
//! unchanged), so the page makes one `Lab`, drives it as before, and hands it
//! to `LabCanvas.attach`, which renders that same instance every frame.
//! README.md is the contract.
//!
//! - `paint`, `theme`, `icons`, `scene`, `interact`, `tunables`: plain Rust
//!   over the core, natively unit-tested. They hold the geometry, the
//!   palette, hit testing, pointer handling and the canvas's own settings.
//! - `web`, wasm32 only: the JavaScript API, the DOM listeners and the one
//!   per-page state.
//! - `app`, wasm32 only: the Bevy app, one plugin per concern, which turns
//!   the scene into 3D meshes and text every frame.

#![forbid(unsafe_code)]

pub mod icons;
pub mod interact;
pub mod paint;
pub mod scene;
pub mod theme;
pub mod tunables;

#[cfg(target_arch = "wasm32")]
mod app;
#[cfg(target_arch = "wasm32")]
mod web;

// The core's `Lab` class is exported from this module too: wasm-bindgen
// keeps every `#[wasm_bindgen]` item of a linked crate.
pub use losos_lab_core::Lab;
