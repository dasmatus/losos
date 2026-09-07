//! `lososd` — the appliance control daemon.
//!
//! Runs as root under systemd and does three things, in this order:
//!
//!   1. claims `org.losos1` on the system bus and exports the control
//!      interface for `losos-ctl`,
//!   2. re-attaches a watcher to any rebuild still recorded as in flight, and
//!   3. serves the Bearer-authed loopback HTTP API the admin UI talks to.
//!
//! Step 2 must not be dropped: `nixos-rebuild switch` restarts this daemon
//! during activation, so the watcher that started a rebuild dies partway
//! through it.

use anyhow::Context;
use losos_ctl::io_backend::{IoLosos, Paths};
use losos_ctl::{dbus, http, supervisor};

fn main() -> std::process::ExitCode {
    tracing_subscriber::fmt()
        // systemd stamps its own timestamp on every journal line; a second one
        // in the message is noise.
        .without_time()
        // Diagnostics belong on stderr. Under systemd both streams reach the
        // journal either way, so nothing is lost, and stdout stays clean.
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!(error = ?e, "lososd is stopping");
            std::process::ExitCode::FAILURE
        }
    }
}

/// Resolves when systemd (or a terminal) asks the daemon to stop.
async fn terminated() -> anyhow::Result<()> {
    use tokio::signal::unix::{signal, SignalKind};
    let mut sigterm = signal(SignalKind::terminate()).context("installing the SIGTERM handler")?;
    let mut sigint = signal(SignalKind::interrupt()).context("installing the SIGINT handler")?;
    tokio::select! {
        _ = sigterm.recv() => {}
        _ = sigint.recv() => {}
    }
    Ok(())
}

fn run() -> anyhow::Result<()> {
    // One backend for the whole daemon. Every clone below shares its lock,
    // which is what serialises a bus `Change` against a `POST /api/change`
    // against a rebuild watcher recording an outcome.
    let backend = IoLosos::new(Paths::from_env());

    // The HTTP server needs an actix `System` (a current-thread runtime with a
    // LocalSet); zbus needs a general tokio runtime. Running both in one
    // runtime is the classic way to deadlock this pair, so each gets its own
    // thread. Nothing crosses the boundary but plain data, because the command
    // core is synchronous.
    //
    // The thread reports back over this channel. Without it a failed bind was
    // invisible: the daemon kept running with no admin API, systemd saw a
    // healthy process, and `Restart=on-failure` never fired.
    let (http_outcome, http_failed) = tokio::sync::oneshot::channel::<anyhow::Result<()>>();
    let http_backend = backend.clone();
    let _http_thread = std::thread::Builder::new()
        .name("lososd-http".into())
        .spawn(move || {
            // A closed receiver means the daemon is already on its way down.
            let _ = http_outcome.send(http::serve(http_backend));
        })?;

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .thread_name("lososd")
        .enable_all()
        .build()?;

    rt.block_on(async {
        // LOSOS_NO_DBUS exists for smoke-testing the HTTP surface on a machine
        // with no system bus; the daemon is otherwise useless without it.
        let _conn = match std::env::var("LOSOS_NO_DBUS") {
            Ok(_) => {
                tracing::warn!("LOSOS_NO_DBUS set — D-Bus listener disabled");
                None
            }
            Err(_) => Some(dbus::serve(backend.clone()).await?),
        };

        supervisor::start_supervisor(&backend);

        // The bus connection and the HTTP thread carry the load from here; this
        // task waits for whichever comes first, a stop signal or the HTTP
        // server falling over.
        tokio::select! {
            outcome = http_failed => match outcome {
                Ok(Err(e)) => Err(e),
                Ok(Ok(())) => anyhow::bail!("the admin HTTP server stopped on its own"),
                Err(_) => anyhow::bail!("the admin HTTP thread died without reporting why"),
            },
            signal = terminated() => {
                signal?;
                tracing::info!("stop signal received; shutting down");
                Ok(())
            }
        }
    })
}
