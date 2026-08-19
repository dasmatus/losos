//! losos-registrar — the stub Rust HTTP server that owns the edge's proxy config.
//!
//! Two subcommands share this crate:
//!   * `serve`   — runs on the EDGE. A loopback HTTP API (`/register`,
//!     `/heartbeat`, `/deregister`, `/health`, `/config`) + a reconciler task
//!     that *constantly updates* Traefik dynamic config
//!     (`/etc/traefik/dynamic/losos.yml`) and rathole server config
//!     (`/etc/rathole/server.toml`) from a tenant registry. Sole writer of
//!     both files; atomic temp+fsync+rename; SIGHUPs rathole only when the
//!     rathole config content changed.
//!   * `announce` — runs on the APPLIANCE. Reads its token, POSTs `/register`
//!     on start, then `/heartbeat` on a cadence. Stateless.
//!
//! The pure core ([`config::desired_config`]) is separated from IO so the
//! state machine is unit-tested with no filesystem, mirroring the Haskell
//! backend's `Losos`/`TestM` split.

#![warn(unreachable_pub)]

pub mod announce;
pub mod config;
pub mod error;
mod fsutil;
pub mod opts;
pub mod registry;
pub mod server;
pub mod action;
pub use config::{desired_config, EdgeOpts, Files, TenantView};
pub use error::{ApiError, RegistryError};
pub use registry::{Registry, Tenant};