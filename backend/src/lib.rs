//! losos appliance control plane.
//!
//! Two binaries share this library:
//!
//!   * **`lososd`** — a root systemd daemon that owns
//!     `/var/lib/losos/state.json`, exports the `org.losos1` D-Bus service and
//!     serves a Bearer-authed loopback HTTP API for the admin UI.
//!   * **`losos-ctl`** — a facade CLI that relays each subcommand to `lososd`
//!     over D-Bus, plus the `install` subcommand, which runs standalone on the
//!     installer ISO where no daemon exists.
//!
//! The layering is the point:
//!
//! ```text
//!   bin/losos-ctl ─ facade ──D-Bus──┐
//!                                   ├─► losos::cmd_* ──► dyn Losos ──┬─ io_backend (real)
//!   bin/lososd ──── dbus / http ────┘                                └─ fake       (tests)
//! ```
//!
//! [`losos`] holds the state machine and knows nothing about transports or
//! paths; [`io_backend`] and [`fake`] are the two interpreters of its effect
//! trait. That split is what lets the command logic be tested without a
//! filesystem, a bus or a `nixos-rebuild`.
//!
//! `backend/schema.json` is the normative wire specification for every JSON
//! document, D-Bus method and HTTP route here.

pub mod dbus;
pub mod facade;
pub mod fake;
pub mod grow;
pub mod http;
pub mod installer;
pub mod installer_io;
pub mod io_backend;
pub mod losos;
pub mod model;
pub mod overrides;
pub mod recovery;
pub mod setup;
pub mod supervisor;
