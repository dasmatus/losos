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
pub mod app_repo;
pub mod apps;
pub mod backup;
pub mod boxid;
pub mod builder;
pub mod calls;
pub mod catalogue;
pub mod chat;
pub mod ci;
pub mod cloud;
pub mod cloud_share;
pub mod companion;
pub mod compose;
pub mod config_repo;
pub mod contacts;
pub mod credentials;
pub mod dbus;
pub mod devices;
pub mod directory;
pub mod edge;
pub mod erase;
pub mod facade;
pub mod fake;
pub mod forge;
pub mod git_rooms;
pub mod grow;
pub mod guard;
pub mod http;
pub mod ical;
pub mod install_report;
pub mod install_screen;
pub mod installer;
pub mod installer_io;
pub mod io_backend;
pub mod jsx;
pub mod lab;
pub mod login;
pub mod losos;
pub mod mail;
pub mod mail_pgp;
pub mod mail_setup;
pub mod market;
pub mod model;
pub mod moderation;
pub mod monero_key;
pub mod office;
pub mod options;
pub mod overrides;
pub mod people;
pub mod pictures;
pub mod pins;
pub mod plugin;
pub mod profiles;
pub mod receipt;
pub mod recovery;
pub mod roles;
pub mod schizo;
pub mod sections;
pub mod setup;
pub mod signin;
pub mod site;
pub mod supervisor;
pub mod textsearch;
pub mod user_apps;
pub mod vcard;
pub mod vm_upload;
pub mod windows;
pub mod zcash_key;
