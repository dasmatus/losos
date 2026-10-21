//! Work that runs after the call answers: this plugin's
//! [`crate::Plugin::run`], in a fresh instance on a host thread.

/// Start `kind` with `input`.
pub fn spawn(kind: &str, input: &[u8]) -> crate::Result<()> {
    crate::wit::jobs::spawn(kind, input).map_err(Into::into)
}
