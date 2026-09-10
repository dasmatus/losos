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
    /// An authenticated tenant asked to join the mesh cluster without
    /// `losos.edge.tenants.<id>.cluster` set. Mesh membership is a second,
    /// narrower whitelist than proxy membership: the master-proxy tunnel only
    /// forwards HTTP to one box, whereas a mesh node runs a kubelet on the
    /// edge's cluster. Enrolling every proxy tenant by default would hand that
    /// out to boxes the operator only ever meant to publish a website for.
    #[error("cluster enrolment not permitted for this id")]
    ClusterForbidden,
    /// A join asked for a node name other than the caller's own appliance id.
    /// This is what makes the handler's stale-node delete safe: the node object
    /// it removes is always the caller's own, so a tenant cannot evict a
    /// neighbour's node from the mesh by naming it.
    #[error("node name must equal the appliance id")]
    NodeNameForbidden,
    /// A join reached an edge started without `--mesh-agent-token-file` /
    /// `--mesh-server-addr`, or whose agent token file is unusable. The route
    /// exists unconditionally so the master-proxy half needs no second binary,
    /// but an edge with `losos.edge.cluster.enable = false` has no node token
    /// to hand out.
    #[error("mesh enrolment not configured on this edge")]
    MeshUnconfigured,
    /// A window a caller supplied is not `HH:MM`. Rejected rather than clamped:
    /// a silently-corrected window is a box that contributes compute at an hour
    /// its owner never agreed to.
    #[error("window_start and window_end must be HH:MM, 00:00-23:59")]
    InvalidWindow,
    /// The mesh apiserver could not be reached, or refused the stale-node
    /// cleanup. The payload names the URL and status for the log; the caller
    /// gets a constant (see [`ApiError::public_body`]).
    #[error("mesh apiserver: {0}")]
    KubeApi(String),
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
            ApiError::HostnameForbidden
            | ApiError::ClusterForbidden
            | ApiError::NodeNameForbidden => StatusCode::FORBIDDEN,
            ApiError::UnknownAppliance => StatusCode::NOT_FOUND,
            ApiError::InvalidWindow => StatusCode::BAD_REQUEST,
            ApiError::Io(_) | ApiError::Serde(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::Registry(_) | ApiError::MeshUnconfigured | ApiError::KubeApi(_) => {
                StatusCode::SERVICE_UNAVAILABLE
            }
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
            ApiError::Unauthorized
            | ApiError::HostnameForbidden
            | ApiError::UnknownAppliance
            | ApiError::ClusterForbidden
            | ApiError::NodeNameForbidden
            | ApiError::MeshUnconfigured
            | ApiError::InvalidWindow => Cow::Owned(self.to_string()),
            ApiError::Io(_) | ApiError::Serde(_) => Cow::Borrowed("internal error"),
            ApiError::Registry(_) => Cow::Borrowed("registry unavailable; retry later"),
            // Deliberately *not* `self.to_string()`: the payload carries the
            // apiserver URL and the status it returned, which tells an
            // unauthenticated prober how the edge's control plane is addressed.
            ApiError::KubeApi(_) => Cow::Borrowed("mesh control plane unavailable; retry later"),
        }
    }
}

impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        match &self {
            ApiError::Io(_) | ApiError::Serde(_) | ApiError::Registry(_) => {
                tracing::error!(target: Action::Serve.target(), "request failed: {self}");
            }
            // The two mesh server-side faults log at error under the join
            // target: both mean an operator has to do something (place the
            // agent token file, or fix the apiserver credentials), and both
            // are invisible to the caller, who only ever sees a 503.
            ApiError::MeshUnconfigured | ApiError::KubeApi(_) => {
                tracing::error!(target: Action::Join.target(), "join failed: {self}");
            }
            ApiError::ClusterForbidden | ApiError::NodeNameForbidden | ApiError::InvalidWindow => {
                tracing::warn!(target: Action::Join.target(), "join rejected: {self}");
            }
            ApiError::Unauthorized | ApiError::HostnameForbidden | ApiError::UnknownAppliance => {
                tracing::debug!(target: Action::Serve.target(), "request rejected: {self}");
            }
        }
        (self.status(), self.public_body().into_owned()).into_response()
    }
}
