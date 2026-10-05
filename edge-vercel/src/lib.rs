//! losos-edge-vercel — the edge registrar's HTTP control plane as a Vercel
//! Function, for demonstrations.
//!
//! The self-hosted edge (`modules/edge.nix`) is three things on one VPS: a
//! rathole server holding the appliances' outbound tunnels, Traefik terminating
//! TLS for every tenant hostname, and `losos-registrar serve`, which turns
//! register/heartbeat calls into the other two's config files. Only the third
//! can run on a serverless platform, and this crate runs exactly that third:
//! the registrar's own router ([`losos_registrar::server::build`]) with its own
//! authentication, body cap, timeout and load shedding, unmodified. What is
//! Vercel-specific sits around it and nowhere else:
//!
//!   * [`settings`] reads the operator whitelist and the token secrets from
//!     environment variables, since the function has no `/var/secrets`, and
//!     writes them to the function's scratch disk in the files the registrar
//!     already reads — so the whitelist code path is the production one.
//!   * [`store`] keeps the registry in a Postgres (Neon, as Vercel's
//!     marketplace provisions it), one `jsonb` row read before and upserted
//!     after every request. A function instance may be recreated between two
//!     heartbeats and may run several copies at once, so local state is not
//!     state. Without a database the registry lives in the instance's memory
//!     and survives only while that instance is warm.
//!   * [`host`] wraps every request: load the registry from the store, let the
//!     registrar answer, run one reconcile pass (which is where the heartbeat
//!     TTL prunes), save the registry back. The reconciler is driven by
//!     requests because nothing else runs between them.
//!   * [`status`] adds two read-only routes the real edge does not have,
//!     `/status` and `/status/traefik`, which the page in `public/` polls so an
//!     audience can watch boxes register, heartbeat and expire, and see the
//!     Traefik configuration the edge would be writing for them.
//!
//! What is *not* here, and cannot be: the rathole tunnel (a raw TCP listener
//! holding long-lived connections), Traefik with per-tenant Let's Encrypt
//! certificates, the rke2 mesh control plane, and the Stripe gate (a Unix
//! socket fed by systemd credentials). On this host `/cluster/join` and
//! `/market/*` answer 503, as they do on a self-hosted edge with those halves
//! switched off, and `/noise-public-key` answers 404 because there is no
//! tunnel to pin a key for.
//!
//! The owner has accepted that this exposes the registrar — tenant ids,
//! hostnames and their liveness — to a third-party platform and, through
//! `/status`, to anyone with the URL. It is a demonstration deployment. The
//! tokens themselves never leave the environment variables and the function's
//! scratch disk, and are never written to the database or shown by any route.

#![warn(unreachable_pub)]

pub mod host;
pub mod settings;
pub mod status;
pub mod store;

pub use host::Host;
pub use settings::{Settings, StoreSettings, TenantSpec};
pub use store::{MemStore, PgStore, Store};
