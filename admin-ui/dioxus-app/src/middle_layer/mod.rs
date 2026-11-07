//! Middle layer between Dioxus UI and backend/operator surfaces.
//!
//! This layer defines typed endpoint mappings, contract alignment helpers, and
//! request/response models. Some operations are box `/api/*` bearer calls; edge/
//! operator flows may be transport-agnostic (see `edge`).
//!
//! Modules:
//! - `resources`: canonical endpoint builders/normalizers for apps/market,
//!   storage, backup, and users (contract alignment).
//! - `credentials`: credential-related middle-layer helpers.
//! - `edge`: edge/operator typed contracts and transport abstraction.

pub mod credentials;
pub mod edge;
pub mod resources;
