//! Crate error types.
//!
//! One `thiserror` enum per subsystem, connected with `#[from]` so `?`
//! crosses boundaries without manual `map_err`. The binary's `main` and the
//! thin reconciler orchestration (see `server::run` / `reconcile_once`)
//! aggregate errors via `miette::Report` — these enums implement
//! `miette::Diagnostic` (empty impls, below) so `?` converts them into a
//! `Report` and `.context()`/`.with_context()` can wrap them. Library code and
//! the HTTP boundary use the concrete enums defined here.
//!
//! At the HTTP handler boundary we never `?`-propagate a raw error into a 500:
//! [`ApiError`] maps each variant to a status code via [`ApiError::status`],
//! and — because every route is reachable from the public internet through
//! Traefik's `register.<domain>` router — the *body* is a fixed string per
//! status. Server-side detail (paths, parse positions, port ranges) goes to
//! the log via [`ApiError::into_response`], never to the caller.

use std::borrow::Cow;
use std::io;

use axum::http::StatusCode;

use crate::action::Action;

/// Registry-layer failures: persistence IO, JSON parse, or the rathole port
/// range being exhausted (no free port for a new tenant).
///
/// `PortRangeExhausted` replaces an earlier `0`-sentinel return from port
/// allocation — "no free port" is now unrepresentable as a success.
#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    /// A filesystem operation on the registry store failed.
    #[error("registry io: {0}")]
    Io(#[from] io::Error),
    /// The on-disk registry was not valid JSON / did not match the schema.
    #[error("registry parse: {0}")]
    Serde(#[from] serde_json::Error),
    /// Every port in the configured range is already assigned.
    #[error("rathole port range {lo}-{hi} exhausted")]
    PortRangeExhausted { lo: u16, hi: u16 },
}

/// The HTTP handler boundary error. Each variant maps to a status code; see
/// [`ApiError::status`]. Handlers return `Result<_, ApiError>` and use `?` —
/// the `#[from]` impls convert `io::Error`, `serde_json::Error`, and
/// [`RegistryError`] automatically.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// Unknown appliance id, or a token that did not constant-time-match.
    #[error("unauthorized")]
    Unauthorized,
    /// A tenant tried to register a hostname other than its whitelisted one.
    #[error("hostname not permitted for this id")]
    HostnameForbidden,
    /// Heartbeat for an id the registry no longer knows — caller should
    /// re-register.
    #[error("unknown appliance; re-register")]
    UnknownAppliance,
    /// A filesystem operation backing a request failed (token/tenants read).
    #[error(transparent)]
    Io(#[from] io::Error),
    /// The tenants whitelist did not parse as JSON.
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    /// A registry mutation failed (port exhaustion or persist failure).
    #[error(transparent)]
    Registry(#[from] RegistryError),
}

// `miette::Diagnostic` has no required methods (every method is
// default-implemented), so an empty impl is all it takes to let these enums
// flow through `?` and `.context()` into a `miette::Report` (which only has
// `From<E: Diagnostic>` — there is no blanket `impl Diagnostic for E: Error`).
// Without these impls the reconciler orchestration, which returns
// `miette::Result`, could not convert `ApiError`/`RegistryError` via `?`.
impl miette::Diagnostic for ApiError {}
impl miette::Diagnostic for RegistryError {}

impl ApiError {
    /// Map this error to the HTTP status the handler should return.
    ///
    /// Auth/forbidden/not-found are precise; IO and serde are server faults
    /// (500); a registry fault is treated as the service being temporarily
    /// unavailable (503) — port exhaustion is "retry later", and a persist
    /// failure likewise means the registry could not commit the change.
    #[must_use]
    pub fn status(&self) -> StatusCode {
        match self {
            ApiError::Unauthorized => StatusCode::UNAUTHORIZED,
            ApiError::HostnameForbidden => StatusCode::FORBIDDEN,
            ApiError::UnknownAppliance => StatusCode::NOT_FOUND,
            ApiError::Io(_) | ApiError::Serde(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::Registry(_) => StatusCode::SERVICE_UNAVAILABLE,
        }
    }

    /// The response body for this error — a fixed string, never the rendered
    /// error.
    ///
    /// The client-fault variants carry no server state, so their own
    /// `#[error(...)]` text is safe and useful. The server-fault variants wrap
    /// an `io::Error`/`serde_json::Error` whose message names the path that
    /// failed or the byte offset that would not parse; on an
    /// internet-reachable route that is free reconnaissance, so the caller
    /// gets a constant and the operator gets the detail in the log.
    #[must_use]
    fn public_body(&self) -> Cow<'static, str> {
        match self {
            ApiError::Unauthorized | ApiError::HostnameForbidden | ApiError::UnknownAppliance => {
                Cow::Owned(self.to_string())
            }
            ApiError::Io(_) | ApiError::Serde(_) => Cow::Borrowed("internal error"),
            ApiError::Registry(_) => Cow::Borrowed("registry unavailable; retry later"),
        }
    }
}

impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        match &self {
            ApiError::Io(_) | ApiError::Serde(_) | ApiError::Registry(_) => {
                tracing::error!(target: Action::Serve.target(), "request failed: {self}");
            }
            ApiError::Unauthorized | ApiError::HostnameForbidden | ApiError::UnknownAppliance => {
                tracing::debug!(target: Action::Serve.target(), "request rejected: {self}");
            }
        }
        (self.status(), self.public_body().into_owned()).into_response()
    }
}
