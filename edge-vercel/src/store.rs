//! The registry's home between requests: one row in a Postgres.
//!
//! A function instance may be created for one request and gone after it, and
//! several may run at once, so the registry has to live somewhere every
//! instance can reach. That is a Postgres — Neon, as Vercel's marketplace
//! provisions it — holding the whole [`Snapshot`] as one `jsonb` row under one
//! key, read before and upserted after every request. One row rather than a
//! table per tenant because the registrar's own model is one file: the
//! reconciler reasons about the whole set at once, and a snapshot that is one
//! value cannot be half-read.
//!
//! Two instances can still interleave a read and a write; the later upsert
//! wins, which for a registry of a few boxes heartbeating every half minute
//! costs at most one heartbeat. The host serialises requests within an
//! instance (see [`crate::host`]), so there is no race inside one.
//!
//! The connection is made lazily and kept for the life of the instance, and
//! remade when it drops; TLS is the registrar's own rustls (with `ring`), with
//! Mozilla's roots, so Neon's certificate is verified like any other. The
//! snapshot carries no secret (see [`losos_registrar::Registry::export`]), so
//! the database holds tenant ids, hostnames, ports and ages, and nothing else.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use losos_registrar::Snapshot;
use tokio::sync::Mutex;
use tokio_postgres::config::SslMode;
use tokio_postgres::Client;
use tokio_postgres_rustls::MakeRustlsConnect;

use crate::settings::StoreSettings;

/// The one table. Created on first connect, idempotently, so a fresh database
/// from the marketplace needs no migration step.
///
/// Under an advisory lock, because two cold instances answering their first
/// requests at once both run this, and two concurrent `CREATE TABLE IF NOT
/// EXISTS` on a table that does not exist yet is a documented Postgres race:
/// one of them fails on `pg_type`'s unique index instead of seeing the other's
/// table. The lock serialises them; the second then finds the table and
/// no-ops — quietly, since the `NOTICE` it would otherwise raise lands in the
/// function log on every cold start.
const SCHEMA: &str = "SET client_min_messages = warning; \
    BEGIN; \
    SELECT pg_advisory_xact_lock(705051); \
    CREATE TABLE IF NOT EXISTS losos_edge_registry (\
    key text PRIMARY KEY, \
    snapshot jsonb NOT NULL, \
    updated_at timestamptz NOT NULL DEFAULT now()); \
    COMMIT";

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("database: {0}")]
    Postgres(#[from] tokio_postgres::Error),
    #[error("database TLS setup: {0}")]
    Tls(#[from] rustls::Error),
    #[error("stored snapshot is not a snapshot: {0}")]
    Corrupt(#[from] serde_json::Error),
}

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Where a snapshot is kept between requests. Object-safe so the host can hold
/// whichever one the settings name.
pub trait Store: Send + Sync {
    /// The saved snapshot, or `None` when nothing has been saved yet.
    fn load(&self) -> BoxFuture<'_, Result<Option<Snapshot>, StoreError>>;
    /// Replace the saved snapshot.
    fn save<'a>(&'a self, snapshot: &'a Snapshot) -> BoxFuture<'a, Result<(), StoreError>>;
    /// `"postgres"` or `"memory"`, for the status route.
    fn kind(&self) -> &'static str;
}

/// A Postgres, holding the snapshot under one key.
pub struct PgStore {
    config: tokio_postgres::Config,
    tls: MakeRustlsConnect,
    key: String,
    /// The live connection, if one has been made and has not dropped. Behind
    /// a lock so one instance never opens two.
    client: Mutex<Option<Client>>,
}

impl PgStore {
    /// Parse the connection string and prepare TLS; nothing connects yet.
    ///
    /// `sslmode` is taken from the string when it says so and is otherwise
    /// `require`, not tokio-postgres's `prefer`: Neon refuses plaintext, and a
    /// store that silently fell back to it elsewhere would send the registry
    /// in the clear. `sslmode=disable` stays honoured so a test can point at a
    /// Postgres on loopback.
    pub fn new(settings: &StoreSettings) -> Result<Self, StoreError> {
        let mut config: tokio_postgres::Config = settings.url.parse()?;
        if config.get_ssl_mode() == SslMode::Prefer {
            config.ssl_mode(SslMode::Require);
        }
        let roots = rustls::RootCertStore {
            roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
        };
        // Named rather than `ClientConfig::builder()`: that picks the one
        // enabled crypto provider and panics if the tree ever enables two.
        let tls = rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()?
        .with_root_certificates(roots)
        .with_no_client_auth();
        Ok(Self {
            config,
            tls: MakeRustlsConnect::new(tls),
            key: settings.key.clone(),
            client: Mutex::new(None),
        })
    }

    /// Open a connection, drive it on a task, and make sure the table exists.
    async fn connect(&self) -> Result<Client, StoreError> {
        let (client, connection) = self.config.connect(self.tls.clone()).await?;
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                tracing::warn!("database connection ended: {e}");
            }
        });
        client.batch_execute(SCHEMA).await?;
        Ok(client)
    }

    /// The live client, connecting first if there is none or the last one
    /// dropped. The guard is handed back so a caller can forget a client whose
    /// query failed, and the next call reconnects rather than retrying a dead
    /// socket.
    async fn client(&self) -> Result<tokio::sync::MutexGuard<'_, Option<Client>>, StoreError> {
        let mut slot = self.client.lock().await;
        if slot.as_ref().is_none_or(Client::is_closed) {
            *slot = Some(self.connect().await?);
        }
        Ok(slot)
    }
}

/// A query's outcome; on failure the client in `slot` is forgotten.
fn settle<T>(
    slot: &mut Option<Client>,
    outcome: Result<T, tokio_postgres::Error>,
) -> Result<T, StoreError> {
    match outcome {
        Ok(v) => Ok(v),
        Err(e) => {
            *slot = None;
            Err(e.into())
        }
    }
}

impl Store for PgStore {
    fn load(&self) -> BoxFuture<'_, Result<Option<Snapshot>, StoreError>> {
        Box::pin(async move {
            let mut slot = self.client().await?;
            let outcome = slot
                .as_ref()
                .expect("connected above")
                .query_opt(
                    "SELECT snapshot FROM losos_edge_registry WHERE key = $1",
                    &[&self.key],
                )
                .await;
            match settle(&mut slot, outcome)? {
                None => Ok(None),
                Some(row) => {
                    let value: serde_json::Value = row.try_get(0)?;
                    Ok(Some(serde_json::from_value(value)?))
                }
            }
        })
    }

    fn save<'a>(&'a self, snapshot: &'a Snapshot) -> BoxFuture<'a, Result<(), StoreError>> {
        Box::pin(async move {
            let value = serde_json::to_value(snapshot)?;
            let mut slot = self.client().await?;
            let outcome = slot
                .as_ref()
                .expect("connected above")
                .execute(
                    "INSERT INTO losos_edge_registry (key, snapshot, updated_at) \
                     VALUES ($1, $2, now()) \
                     ON CONFLICT (key) DO UPDATE \
                     SET snapshot = EXCLUDED.snapshot, updated_at = now()",
                    &[&self.key, &value],
                )
                .await;
            settle(&mut slot, outcome).map(|_| ())
        })
    }

    fn kind(&self) -> &'static str {
        "postgres"
    }
}

/// A store that is only this process's memory — what the host runs with when
/// no database is configured, and what the tests hand two hosts to share.
///
/// Shared by cloning: every clone sees the same cell, so two [`crate::Host`]s
/// built on clones of one `MemStore` behave like two function instances on
/// one database.
#[derive(Debug, Clone, Default)]
pub struct MemStore {
    cell: Arc<std::sync::Mutex<Option<String>>>,
}

impl MemStore {
    /// What the last `save` left, as the JSON text it was serialised to; for
    /// tests that want to see exactly what a database would be holding.
    pub fn saved_json(&self) -> Option<String> {
        self.cell.lock().map_or(None, |c| c.clone())
    }

    /// Put arbitrary JSON text where the next `load` will find it, as if a
    /// previous instance had saved it.
    pub fn seed_json(&self, json: impl Into<String>) {
        if let Ok(mut c) = self.cell.lock() {
            *c = Some(json.into());
        }
    }
}

impl Store for MemStore {
    fn load(&self) -> BoxFuture<'_, Result<Option<Snapshot>, StoreError>> {
        Box::pin(async move {
            match self.saved_json() {
                None => Ok(None),
                Some(json) => Ok(Some(serde_json::from_str(&json)?)),
            }
        })
    }

    fn save<'a>(&'a self, snapshot: &'a Snapshot) -> BoxFuture<'a, Result<(), StoreError>> {
        Box::pin(async move {
            let json = serde_json::to_string(snapshot)?;
            if let Ok(mut c) = self.cell.lock() {
                *c = Some(json);
            }
            Ok(())
        })
    }

    fn kind(&self) -> &'static str {
        "memory"
    }
}
