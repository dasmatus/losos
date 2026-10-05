//! The per-request wrapper that makes a serverless function behave like the
//! long-lived edge process.
//!
//! `losos-registrar serve` is one process that loads the registry once, answers
//! requests against the copy in memory and runs the reconciler on a timer. A
//! Vercel Function instance may be created for one request and destroyed after
//! it, several may run at once, and nothing runs between requests. So every
//! request here is a complete life of the edge in miniature:
//!
//!   1. load the registry from the store ([`Registry::import`], which also
//!      prunes by TTL — the reconciler's prune never ran while no instance
//!      was alive);
//!   2. run one reconcile pass ([`App::reconcile`]): the real edge's prune,
//!      whitelist repair and file rewrites, so the scratch-disk `losos.yml`
//!      the status route shows is what Traefik would be reloading;
//!   3. hand the request to the registrar's router, untouched, then run the
//!      pass again so what the request changed reaches the files;
//!   4. save the registry to the store.
//!
//! One mutex serialises the four steps within an instance, which is what makes
//! a load followed by a save safe against a concurrent request on the same
//! instance. Across instances the store's last writer wins; see
//! [`crate::store`] for what that costs.
//!
//! A database that cannot be read fails the request with 503 rather than serving
//! from whatever this instance remembers: a stale registry that then got saved
//! would overwrite every heartbeat other instances recorded. A database that
//! cannot be written fails a mutating request the same way, so the appliance's
//! announce loop retries, and lets a read through, since nothing was lost.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Request, State};
use axum::http::{Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::Router;
use losos_registrar::server::{build, App};
use miette::{Result, WrapErr};
use tokio::sync::Mutex;

use crate::settings::Settings;
use crate::status::{self, Status};
use crate::store::{PgStore, Store, StoreError};

/// A built registrar plus the pieces this host adds around it.
pub struct Host {
    app: App,
    store: Option<Box<dyn Store>>,
    ttl: Duration,
    traefik_file: PathBuf,
    /// Held for the whole of every request; see the module header.
    turn: Mutex<()>,
}

impl Host {
    /// Materialise the settings onto the scratch disk and build the registrar,
    /// with the Postgres the settings name (or none).
    pub async fn new(settings: &Settings) -> Result<Self> {
        let store: Option<Box<dyn Store>> = match &settings.store {
            Some(s) => Some(Box::new(PgStore::new(s).into_diagnostic_store()?)),
            None => None,
        };
        Self::with_store(settings, store).await
    }

    /// As [`Host::new`], with the store supplied — a [`crate::store::MemStore`]
    /// in the tests, where two hosts on one cell stand in for two instances
    /// on one database.
    pub async fn with_store(settings: &Settings, store: Option<Box<dyn Store>>) -> Result<Self> {
        let opts = settings
            .materialise()
            .wrap_err("write whitelist and token files")?;
        let ttl = opts.heartbeat_ttl;
        let traefik_file = PathBuf::from(&opts.traefik_dir).join("losos.yml");
        let app = build(opts).await.wrap_err("build registrar")?;
        tracing::info!(
            "edge host ready: {} tenant(s) whitelisted, ttl {}s, store {}",
            settings.tenants.len(),
            ttl.as_secs(),
            store.as_ref().map_or("none", |s| s.kind()),
        );
        Ok(Self {
            app,
            store,
            ttl,
            traefik_file,
            turn: Mutex::new(()),
        })
    }

    /// The registrar's router with the status routes merged in, every route
    /// under the load/reconcile/save wrapper.
    pub fn router(self: &Arc<Self>) -> Router {
        self.app
            .router()
            .merge(status::routes(Arc::clone(self)))
            .layer(middleware::from_fn_with_state(Arc::clone(self), turn))
    }

    /// The registry as of the last reconcile pass, for the status route.
    pub async fn status(&self) -> Status {
        let snapshot = self.app.registry().export().await;
        let store = self.store.as_ref().map_or("memory", |s| s.kind());
        Status::from_snapshot(snapshot, self.ttl, store)
    }

    /// The Traefik dynamic configuration the last reconcile pass wrote, or
    /// `None` when it removed the file (no live tenants).
    pub async fn traefik_config(&self) -> Option<String> {
        tokio::fs::read_to_string(&self.traefik_file).await.ok()
    }

    /// One pass, logged and not fatal, as on the real edge: the next request
    /// runs it again.
    async fn reconcile(&self) {
        if let Err(e) = self.app.reconcile().await {
            tracing::error!("reconcile failed: {e:?}");
        }
    }

    async fn load(&self, store: &dyn Store) -> Result<(), crate::store::StoreError> {
        let snapshot = store.load().await?.unwrap_or_default();
        self.app.registry().import(snapshot, self.ttl).await;
        Ok(())
    }

    async fn save(&self, store: &dyn Store) -> Result<(), crate::store::StoreError> {
        let snapshot = self.app.registry().export().await;
        store.save(&snapshot).await
    }
}

/// Steps 1 to 4 of the module header, around one request.
async fn turn(State(host): State<Arc<Host>>, req: Request, next: Next) -> Response {
    let _turn = host.turn.lock().await;
    if let Some(store) = &host.store {
        if let Err(e) = host.load(store.as_ref()).await {
            tracing::error!("cannot load the registry from the store: {e}");
            return unavailable();
        }
    }
    // Once before the handler, so a read (`/status`) sees the registry as the
    // real edge's files would show it — pruned and whitelist-checked — and once
    // after, so what the handler changed reaches the files before the save.
    host.reconcile().await;
    let mutating = !matches!(*req.method(), Method::GET | Method::HEAD);
    let response = next.run(req).await;
    host.reconcile().await;
    if let Some(store) = &host.store {
        if let Err(e) = host.save(store.as_ref()).await {
            tracing::error!("cannot save the registry to the store: {e}");
            if mutating {
                return unavailable();
            }
        }
    }
    response
}

fn unavailable() -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        "state store unavailable; retry later",
    )
        .into_response()
}

/// `StoreError` is a `thiserror` enum without a `Diagnostic` impl, so `?`
/// cannot lift it into a `miette::Report` directly.
trait IntoDiagnosticStore<T> {
    fn into_diagnostic_store(self) -> Result<T>;
}

impl<T> IntoDiagnosticStore<T> for std::result::Result<T, StoreError> {
    fn into_diagnostic_store(self) -> Result<T> {
        self.map_err(|e| miette::miette!("state store: {e}"))
    }
}
