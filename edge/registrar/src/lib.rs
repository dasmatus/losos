//! losos-registrar — the stub Rust HTTP server that owns the edge's proxy config.
//!
//! Three subcommands share this crate:
//!   * `serve`   — runs on the edge. An HTTP API (`/register`, `/heartbeat`,
//!     `/deregister`, `/health` and the rest), a reconciler task that
//!     *constantly updates* the routes Traefik discovers the boxes from (its
//!     HTTP provider polls the registrar's loopback provider listener) and
//!     the tunnel's service file (`/etc/losos-tunnel/server.json`) from a
//!     tenant registry. Sole writer of that file; atomic temp+fsync+rename.
//!     The tunnel re-reads its file every second, so no signal is sent.
//!   * `announce` — runs on the appliance. Reads its token, POSTs `/register`
//!     on start, then `/heartbeat` on a cadence. Stateless.
//!   * `tunnel-server` and `tunnel-client` — the two ends of the QUIC tunnel
//!     between an edge and its boxes ([`tunnel`]). The server runs beside
//!     `serve` on the edge and needs nothing from it but the file; the client
//!     runs on the box, and on an edge that relays its boxes to a hub.
//!   * (`serve` also hosts the optional Stripe Connect storage/compute market —
//!     see [`market`]; its routes answer 503 unless `--market-gate-socket`
//!     is given. The Stripe key is not held by `serve` at all: see
//!     [`stripe_gate`].)
//!   * `stripe-gate` — the separate process that holds the Stripe key and does
//!     the few Stripe calls the market needs, for `serve`, over a Unix socket.
//!   * `lab` — runs on the machine a LosOS Lab page is open on (or on a box
//!     with `losos.lab.libvirt.enable`): boots the Lab's guests under
//!     libvirt through `virsh` and bridges their consoles and network cards
//!     to the browser. See [`lab`].
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
pub mod bitcoin;
pub mod calls_relay;
pub mod ci;
pub mod coins;
pub mod config;
pub mod connect;
pub mod domains;
pub mod ech;
pub mod error;
pub mod evm_tx;
pub mod exit;
mod fsutil;
pub mod gateway;
pub mod hardware;
pub mod hidden;
pub mod identity;
pub mod idle;
pub mod inference;
pub mod join;
pub mod lab;
pub mod market;
pub mod meter;
pub mod monero;
pub mod official;
pub mod onramp;
pub mod operator;
pub mod opts;
pub mod passthrough;
pub mod plans;
pub mod provision;
pub mod registry;
pub mod relay;
pub mod routes;
pub mod secondary;
pub mod server;
pub mod slots;
pub mod solana;
pub mod solana_escrow;
pub mod stablecoin;
pub mod stats;
pub mod stripe_gate;
pub mod stripe_mode;
pub mod sync;
pub mod tang;
pub mod tunnel;
pub mod vms;
pub mod window;
pub mod zcash;
pub mod zone;
pub use config::{
    desired_config, provider_document, uplink_config, Files, Routes, TenantView, UplinkService,
    UplinkTarget,
};
pub use error::{ApiError, RegistryError};
pub use registry::{Registry, Relayed, RelayedView, Snapshot, SnapshotTenant, Tenant};
pub use window::ComputeWindow;
