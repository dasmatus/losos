//! D-Bus client used by the `losos-ctl` CLI.
//!
//! The CLI does no privileged work of its own: it makes one blocking method
//! call to `lososd` and prints the JSON string that comes back. Authorization
//! is the bus policy shipped by `modules/daemon.nix` (root, or the `losos`
//! group) — there is no sudo and no polkit anywhere in this path.
//!
//! The blocking API is deliberate: a single request/response needs no async
//! runtime, and keeping one out of the CLI keeps its startup instant on the
//! installer ISO.

use zbus::blocking::Connection;

/// Well-known name `lososd` owns on the system bus.
pub const BUS_NAME: &str = "org.losos1";
/// Object path the control interface is exported at.
pub const OBJECT_PATH: &str = "/org/losos1";
/// The control interface.
pub const INTERFACE: &str = "org.losos.Control1";
/// Error name `lososd` returns for a rejected call.
pub const ERROR_NAME: &str = "org.losos1.Error.Failed";

/// A failed round trip to the daemon, already phrased for the user.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct BackendFailure(pub String);

/// Call `member` on `lososd` and return the JSON document it replied with.
///
/// `args` is the method body: `()` for the argument-less methods, or a
/// single-element tuple such as `("mesh",)` for `Change` and `Apply`.
pub fn call_backend<B>(member: &str, args: &B) -> Result<String, BackendFailure>
where
    B: serde::ser::Serialize + zbus::zvariant::DynamicType,
{
    let conn = Connection::system().map_err(|e| {
        BackendFailure(format!(
            "losos-ctl {member}: cannot reach the system bus — {e}"
        ))
    })?;

    let reply = conn
        .call_method(Some(BUS_NAME), OBJECT_PATH, Some(INTERFACE), member, args)
        .map_err(|e| BackendFailure(format!("losos-ctl {member}: lososd call failed — {e}")))?;

    reply.body().deserialize::<String>().map_err(|_| {
        BackendFailure("lososd returned a malformed reply (expected a single JSON string)".into())
    })
}
