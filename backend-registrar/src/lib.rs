//! losos-registrar — the stub Rust HTTP server that owns the edge's proxy config.
//!
//! Three subcommands share this crate:
//!   * `serve`   — runs on the edge. A loopback HTTP API (`/register`,
//!     `/heartbeat`, `/deregister`, `/health`, `/config`) + a reconciler task
//!     that *constantly updates* Traefik dynamic config
//!     (`/etc/traefik/dynamic/losos.yml`) and rathole server config
//!     (`/etc/rathole/server.toml`) from a tenant registry. Sole writer of
//!     both files; atomic temp+fsync+rename; SIGHUPs rathole only when the
//!     rathole config content changed.
//!   * `announce` — runs on the appliance. Reads its token, POSTs `/register`
//!     on start, then `/heartbeat` on a cadence. Stateless.
//!   * `seed` — runs once on the edge at boot, before `serve`. Writes the
//!     zero-tenant rathole `[server]` base (via the same
//!     [`config::desired_config`] `serve`'s reconciler calls) so rathole can
//!     start before the reconciler's first pass; a no-op if the file already
//!     exists, since `serve` then owns it.
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
pub mod seed;
pub mod server;
pub mod action;
pub use config::{desired_config, EdgeOpts, Files, TenantView};
pub use error::{ApiError, RegistryError};
pub use registry::{Registry, Tenant};