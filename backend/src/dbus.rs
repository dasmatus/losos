//! The `org.losos1` D-Bus service.
//!
//! Each method returns the JSON document of the same-named `losos-ctl`
//! subcommand as a single string — no framing, no trailing newline (the CLI
//! adds that when printing). Rejected calls come back as
//! `org.losos1.Error.Failed` carrying the message.
//!
//! There is no authorization code here on purpose: access is decided by the
//! system-bus policy shipped in `modules/daemon.nix` (root always, plus the
//! `losos` group). Adding a second check here would let the two drift.

use crate::facade::{BUS_NAME, OBJECT_PATH};
use crate::io_backend::IoLosos;
use crate::losos::{
    cmd_apply, cmd_change, cmd_factory_reset, cmd_grow, cmd_settings, cmd_state, cmd_status,
};
use crate::model::Mode;
use crate::overrides::validate_apply;
use zbus::{connection::Connection, fdo, interface};

/// The exported object.
///
/// Its backend is a clone of the daemon's one [`IoLosos`], so the lock inside
/// serializes bus calls against HTTP calls and against the rebuild watchers.
/// Writes are already atomic at the filesystem level, but two concurrent
/// `apply`s could otherwise interleave their read-modify-write of the state
/// file.
pub struct Control {
    backend: IoLosos,
}

/// Run a command and turn its result into a D-Bus reply.
///
/// Any error becomes `org.losos1.Error.Failed`, which is what the facade
/// renders to the user. Unlike the HTTP surface this keeps the full context
/// chain: the bus is reachable only by root and the `losos` group.
fn reply(
    backend: &IoLosos,
    f: impl FnOnce(&mut IoLosos) -> anyhow::Result<serde_json::Value>,
) -> fdo::Result<String> {
    match backend.serialized(f) {
        Ok(v) => Ok(v.to_string()),
        Err(e) => Err(fdo::Error::Failed(format!("{e:#}"))),
    }
}

#[interface(name = "org.losos.Control1")]
impl Control {
    /// Current mode and sharing flag.
    fn state(&self) -> fdo::Result<String> {
        reply(&self.backend, cmd_state)
    }

    /// The user-tunable `losos.*` settings.
    fn settings(&self) -> fdo::Result<String> {
        reply(&self.backend, cmd_settings)
    }

    /// Rebuild progress.
    fn status(&self) -> fdo::Result<String> {
        reply(&self.backend, cmd_status)
    }

    /// Switch sharing posture and rebuild.
    fn change(&self, mode: &str) -> fdo::Result<String> {
        let Some(mode) = Mode::parse(mode) else {
            return Err(fdo::Error::Failed(
                "mode must be 'local' or 'mesh'".to_string(),
            ));
        };
        reply(&self.backend, |b| cmd_change(b, mode))
    }

    /// Replace `overrides.nix` and rebuild.
    fn apply(&self, nix_code: &str) -> fdo::Result<String> {
        let code = validate_apply(nix_code)
            .map_err(|e| fdo::Error::Failed(e.to_string()))?
            .to_string();
        reply(&self.backend, |b| cmd_apply(b, &code))
    }

    /// Soft factory reset.
    fn factory_reset(&self) -> fdo::Result<String> {
        reply(&self.backend, cmd_factory_reset)
    }

    /// Extend `/persist` into the volume group's free extents, online.
    ///
    /// Unlike `change` and `apply` this does not rebuild anything and returns
    /// only when the resize is done — it is three short-lived commands, not a
    /// supervised job, so there is no job id to hand back.
    fn grow(&self) -> fdo::Result<String> {
        reply(&self.backend, cmd_grow)
    }
}

/// Claim the well-known name and export the control object.
///
/// The name is requested with replace-existing and do-not-queue: a restarted
/// daemon must take the name over immediately rather than queue behind a stale
/// owner, which would leave `losos-ctl` talking to a dead process.
pub async fn serve(backend: IoLosos) -> anyhow::Result<Connection> {
    let control = Control { backend };

    let conn = zbus::connection::Builder::system()
        .map_err(|e| anyhow::anyhow!("cannot connect to the system bus: {e}"))?
        .serve_at(OBJECT_PATH, control)?
        .name(BUS_NAME)
        .map_err(|e| {
            anyhow::anyhow!(
                "could not acquire {BUS_NAME} on the system bus ({e}); \
                 does the dbus policy ship (services.dbus.packages)?"
            )
        })?
        .build()
        .await
        .map_err(|e| {
            anyhow::anyhow!(
                "could not acquire {BUS_NAME} on the system bus ({e}); \
                 does the dbus policy ship (services.dbus.packages)?"
            )
        })?;

    tracing::info!(bus = BUS_NAME, path = OBJECT_PATH, "D-Bus service exported");
    Ok(conn)
}
