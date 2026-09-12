//! losos-registrar — the stub Rust HTTP server that owns the edge's proxy config.
//!
//! Three subcommands share this crate:
//!   * `serve`   — runs on the edge. An HTTP API serving exactly the four
//!     routes the approved design lists (`/register`, `/heartbeat`,
//!     `/deregister`, `/health`), plus a reconciler task that *constantly
//!     updates* Traefik dynamic config (`/etc/traefik/dynamic/losos.yml`) and
//!     rathole server config (`/etc/rathole/server.toml`) from a tenant
//!     registry. Sole writer of both files; atomic temp+fsync+rename. rathole
//!     hot-reloads from its own file watcher, so no signal is sent.
//!   * `announce` — runs on the appliance. Reads its token, POSTs `/register`
//!     on start, then `/heartbeat` on a cadence. Stateless.
//!   * `seed` — runs once on the edge at boot, before `serve`. Writes the
//!     zero-tenant rathole `[server]` base (via the same
//!     [`config::desired_config`] `serve`'s reconciler calls) so rathole can
//!     start before the reconciler's first pass; a no-op if the file already
//!     exists, since `serve` then owns it.
//!   * `join` — runs once per boot on the appliance, before the rke2 agent.
//!     POSTs `/cluster/join` with the same per-appliance token `announce`
//!     uses, and writes the mesh node token the edge returns. Bounded retry,
//!     then a non-zero exit: the unit is `Requires=`d by `rke2-agent.service`,
//!     so failing loudly is what stops the agent wedging on a token that never
//!     arrived.
//!
//! The pure core ([`config::desired_config`]) is separated from IO so the
//! state machine is unit-tested with no filesystem, mirroring the Haskell
//! backend's `Losos`/`TestM` split. Everything with an IO or HTTP surface is
//! exercised end to end from `tests/` instead — `serve` takes a bound
//! listener and a shutdown future precisely so a test can drive the real
//! router over a real socket.
//!
//! Nothing here is loopback-only in practice: `modules/edge.nix` fronts this
//! API with a Traefik router on `register.<publicDomain>` that has no path
//! rule and no middleware, and the unit runs as root. Every route is
//! internet-reachable and is written that way.

#![warn(unreachable_pub)]

pub mod action;
pub mod announce;
pub mod config;
pub mod error;
mod fsutil;
pub mod idle;
pub mod join;
pub mod opts;
pub mod registry;
pub mod seed;
pub mod server;
pub mod window;
pub use config::{desired_config, EdgeOpts, Files, TenantView};
pub use error::{ApiError, RegistryError};
pub use registry::{Registry, Tenant};
pub use window::ComputeWindow;
