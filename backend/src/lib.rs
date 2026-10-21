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

pub mod access;
pub mod app_names;
pub mod apps;
pub mod backup;
pub mod boxid;
pub mod builder;
pub mod catalogue;
pub mod cloud_apps;
pub mod cloud_share;
pub mod config_repo;
pub mod dbus;
pub mod devices;
pub mod edge;
pub mod erase;
pub mod facade;
pub mod fake;
pub mod grow;
pub mod guard;
pub mod http;
pub mod install_screen;
pub mod installer;
pub mod installer_io;
pub mod io_backend;
pub mod jsx;
pub mod lab;
pub mod login;
pub mod look;
pub mod losos;
pub mod market;
pub mod model;
pub mod monero_key;
pub mod options;
pub mod overrides;
pub mod people;
pub mod pins;
pub mod receipt;
pub mod recovery;
pub mod sections;
pub mod setup;
pub mod signin;
pub mod site;
pub mod supervisor;
pub mod textsearch;
pub mod user_apps;
pub mod vm_upload;
pub mod zcash_key;
