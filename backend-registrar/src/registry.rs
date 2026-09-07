//! The tenant registry — the runtime state the reconciler derives config from.
//!
//! In-memory `Mutex<HashMap<id, Tenant>>`, persisted to `registry.json`
//! (id → {hostname, port}) so the registrar re-attaches after an edge restart:
//! on startup it loads the file, restores each tenant's port assignment, and
//! sets `last_seen = now` (grace) so a rebooting edge doesn't immediately
//! prune every appliance mid-reconnect. The reconciler then regenerates
//! Traefik + rathole config *before* the API opens — mirrors lososd's
//! rebuild re-attach.
//!
//! The [`tokio::sync::Mutex`] (not `std`) is deliberate: its guard is held
//! across the `.await` on the atomic persist. The atomic write itself
//! (temp + fsync + rename) runs on `spawn_blocking` — rename atomicity is a
//! filesystem concern, not an async one.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;

use crate::config::TenantView;
use crate::error::RegistryError;
use crate::fsutil;

/// A live tenant. `last_seen` is a runtime concern of this module only — it is
/// stripped before reaching [`TenantView`] / config generation.
#[derive(Debug, Clone)]
pub struct Tenant {
    pub hostname: String,
    pub rathole_port: u16,
    pub last_seen: Instant,
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct RegistryFile {
    tenants: HashMap<String, RegistryFileTenant>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct RegistryFileTenant {
    hostname: String,
    port: u16,
}

#[derive(Debug)]
pub struct Registry {
    inner: Mutex<HashMap<String, Tenant>>,
    port_range: (u16, u16),
    path: PathBuf,
}

impl Registry {
    /// A new, empty registry backed by `path`. Call [`Registry::load`] before
    /// the API opens to re-attach to any persisted tenants.
    pub fn new(path: impl Into<PathBuf>, port_range: (u16, u16)) -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
            port_range,
            path: path.into(),
        }
    }

    /// Load persisted tenants and restore their port assignments. Call before
    /// the API opens so existing routes are re-provisioned without waiting for
    /// heartbeats.
    pub async fn load(&self) -> Result<(), RegistryError> {
        let file = match tokio::fs::read(&self.path).await {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(RegistryError::Io(e)),
        };
        let file: RegistryFile = serde_json::from_slice(&file)?;
        let now = Instant::now();
        let mut map = self.inner.lock().await;
        for (id, t) in file.tenants {
            map.insert(
                id,
                Tenant {
                    hostname: t.hostname,
                    rathole_port: t.port,
                    last_seen: now,
                },
            );
        }
        Ok(())
    }

    /// Register (or re-register) a tenant. Allocates a port if new; preserves
    /// the existing port on re-register so Traefik/rathole don't churn.
    /// Returns the assigned port, or [`RegistryError::PortRangeExhausted`]
    /// if no port is free.
    pub async fn register(&self, id: &str, hostname: &str) -> Result<u16, RegistryError> {
        let mut map = self.inner.lock().await;
        if let Some(t) = map.get_mut(id) {
            t.hostname = hostname.to_string();
            t.last_seen = Instant::now();
            let port = t.rathole_port;
            drop(map);
            self.persist().await?;
            return Ok(port);
        }
        let port = alloc_port(&map, self.port_range)?;
        map.insert(
            id.to_string(),
            Tenant {
                hostname: hostname.to_string(),
                rathole_port: port,
                last_seen: Instant::now(),
            },
        );
        drop(map);
        self.persist().await?;
        Ok(port)
    }

    /// Refresh `last_seen` for a known tenant. Returns `false` if the id is
    /// unknown — the appliance should re-register. Performs no IO, so it
    /// cannot fail.
    pub async fn heartbeat(&self, id: &str) -> bool {
        let mut map = self.inner.lock().await;
        if let Some(t) = map.get_mut(id) {
            t.last_seen = Instant::now();
            true
        } else {
            false
        }
    }

    /// Remove a tenant and persist the change.
    pub async fn deregister(&self, id: &str) -> Result<(), RegistryError> {
        {
            let mut map = self.inner.lock().await;
            map.remove(id);
        }
        self.persist().await
    }

    /// Remove tenants whose `last_seen` is older than `ttl`. Returns `true` if
    /// anything changed so the caller knows to reconcile.
    pub async fn prune(&self, ttl: Duration) -> bool {
        let now = Instant::now();
        let changed = {
            let mut map = self.inner.lock().await;
            let before = map.len();
            map.retain(|_, t| now.duration_since(t.last_seen) < ttl);
            map.len() != before
        };
        if changed {
            // best-effort persist: a prune-write failure is non-fatal — the
            // next register/heartbeat retries, and the in-memory state stays
            // authoritative for config generation.
            let _ = self.persist().await;
        }
        changed
    }

    /// Snapshot the live tenants (sorted by id for determinism) for config
    /// generation. Tokens are deliberately NOT read here — the reconciler
    /// enriches views from the on-disk token files so the secret stays the
    /// single source of truth.
    #[must_use]
    pub async fn views(&self) -> Vec<TenantView> {
        let map = self.inner.lock().await;
        let mut v: Vec<TenantView> = map
            .iter()
            .map(|(id, t)| TenantView {
                id: id.clone(),
                hostname: t.hostname.clone(),
                rathole_port: t.rathole_port,
                // Token is enriched by the reconciler; placeholder here.
                token: String::new(),
            })
            .collect();
        v.sort_by(|a, b| a.id.cmp(&b.id));
        v
    }

    /// Serialize and persist the registry atomically. The temp + fsync + rename
    /// sequence runs on `spawn_blocking` (rename atomicity is a filesystem
    /// concern); the lock is released before the write so other reads aren't
    /// blocked on disk.
    async fn persist(&self) -> Result<(), RegistryError> {
        let (bytes, path) = {
            let map = self.inner.lock().await;
            let file = RegistryFile {
                tenants: map
                    .iter()
                    .map(|(id, t)| {
                        (
                            id.clone(),
                            RegistryFileTenant {
                                hostname: t.hostname.clone(),
                                port: t.rathole_port,
                            },
                        )
                    })
                    .collect(),
            };
            (serde_json::to_vec_pretty(&file)?, self.path.clone())
        };
        if let Some(parent) = path.parent() {
            // best-effort: the dir normally exists (StateDirectory).
            let _ = tokio::fs::create_dir_all(parent).await;
        }
        fsutil::atomic_write(&path, &bytes, 0o600).await?;
        Ok(())
    }
}

/// Allocate the lowest free port in `(lo, hi]`. Returns
/// [`RegistryError::PortRangeExhausted`] when none is free — never a sentinel.
fn alloc_port(map: &HashMap<String, Tenant>, (lo, hi): (u16, u16)) -> Result<u16, RegistryError> {
    let used: HashSet<u16> = map.values().map(|t| t.rathole_port).collect();
    for p in lo..=hi {
        if !used.contains(&p) {
            return Ok(p);
        }
    }
    Err(RegistryError::PortRangeExhausted { lo, hi })
}

/// Wrap in `Arc` so multiple axum handlers + the reconciler share one registry.
pub type Shared = Arc<Registry>;
