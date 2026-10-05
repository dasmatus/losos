//! The one Vercel Function. `vercel.json` rewrites every path to it, and the
//! registrar's router (plus the two status routes) does the routing.
//!
//! `vercel_runtime::run` listens on loopback for Vercel's bridge and hands
//! each request to the service; `VercelLayer` adapts an axum router to that.
//! `main` therefore runs once per function instance — the settings are read
//! and the registrar built at cold start, and the per-request work lives in
//! `losos_edge_vercel::host`.

use std::sync::Arc;

use losos_edge_vercel::{Host, Settings};
use tower::Layer;
use vercel_runtime::axum::VercelLayer;
use vercel_runtime::{run, Error};

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Vercel collects stdout as the function's log; `RUST_LOG` filters it.
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let settings = Settings::from_env().map_err(report)?;
    let host = Arc::new(Host::new(&settings).await.map_err(report)?);
    run(VercelLayer::new().layer(host.router())).await
}

/// `miette::Report` is not a `std::error::Error`; render it so the cause chain
/// reaches the function log, where a missing variable is otherwise a bare
/// "function crashed".
fn report(e: miette::Report) -> Error {
    tracing::error!("{e:?}");
    Error::from(format!("{e}"))
}
