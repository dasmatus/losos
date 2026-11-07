//! Edge/cluster operator operations: reconcile, secrets, escrow releaser.
//!
//! # Transport mapping
//!
//! The operations in this module are implemented by the edge registrar's
//! operator surface, not by the box's bearer `/api/*` JSON API used by
//! [`crate::api::BoxApi`]. In the TUI they are carried as PGP-signed/encrypted
//! access-envelope calls:
//!
//! - HTTP: `POST <edge>/operator/call` with `Content-Type: application/pgp-encrypted`
//! - Auth: the operator token is inside the sealed envelope (`token` field), not
//!   an HTTP `Authorization: Bearer` header
//! - Host auth: envelope `host` is the pinned edge host fingerprint; answers are
//!   verified against the pinned host key and echoed nonce
//!
//! Inner routes (method + path + JSON body) are defined by the edge registrar
//! operator router (`edge/registrar/src/operator.rs`) and mirrored here as
//! [`EdgeEnvelopeCall`]/[`EdgeEndpoint`] values.
//!
//! Because Dioxus currently ships a bearer box API client and may run on wasm,
//! callers must treat these operations as **envelope/desktop-only** unless a
//! platform-specific bridge provides an equivalent sealed transport. This module
//! therefore focuses on typed models, mapping metadata, safe parsing, and
//! mesh-pane helpers; it does not perform network I/O.
//!
//! # Secret handling
//!
//! Secret values are wrapped in [`SecretValue`], which redacts `Debug`/`Display`
//! output and is never logged or persisted by helpers in this module. Callers
//! must not clone secret values into logs, error messages shown to diagnostics,
//! local storage, or analytics.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fmt;

/// How an edge/cluster operation is carried.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeTransport {
    /// PGP access envelope to the edge operator endpoint (`POST /operator/call`).
    /// Token/fingerprint are envelope fields; not a plain HTTP bearer call.
    EnvelopeOperator,
}

/// One edge operator route inside the sealed envelope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EdgeEndpoint {
    pub method: &'static str,
    pub path: &'static str,
    pub transport: EdgeTransport,
    /// Human note for UI/docs; never contains secrets.
    pub note: &'static str,
}

impl EdgeEndpoint {
    pub const RECONCILE: Self = Self {
        method: "POST",
        path: "/reconcile",
        transport: EdgeTransport::EnvelopeOperator,
        note: "Edge operator envelope call; asks the edge to reconcile state.",
    };

    pub const SECRETS_LIST: Self = Self {
        method: "GET",
        path: "/secrets",
        transport: EdgeTransport::EnvelopeOperator,
        note: "Edge operator envelope call; returns secret metadata only (names/state/timestamps).",
    };

    pub const SECRET_SET: Self = Self {
        method: "POST",
        path: "/secrets",
        transport: EdgeTransport::EnvelopeOperator,
        note: "Edge operator envelope call; body contains a secret value and must not be logged/persisted.",
    };

    pub const ESCROW_RELEASER_GET: Self = Self {
        method: "GET",
        path: "/escrow/releaser",
        transport: EdgeTransport::EnvelopeOperator,
        note: "Edge operator envelope call; returns the releaser address/status, never private key material.",
    };

    pub const ESCROW_RELEASER_MAKE: Self = Self {
        method: "POST",
        path: "/escrow/releaser",
        transport: EdgeTransport::EnvelopeOperator,
        note: "Edge operator envelope call; asks the edge to create/replace the releaser key and returns its address.",
    };

    pub fn is_envelope_only(self) -> bool {
        matches!(self.transport, EdgeTransport::EnvelopeOperator)
    }
}

/// A concrete envelope call description (method/path/body) without executing it.
///
/// This is useful for desktop/native bridges that perform the access-envelope
/// transport on behalf of the UI.
#[derive(Clone, Debug, PartialEq)]
pub struct EdgeEnvelopeCall {
    pub endpoint: EdgeEndpoint,
    pub body: Value,
}

impl EdgeEnvelopeCall {
    pub fn reconcile() -> Self {
        Self {
            endpoint: EdgeEndpoint::RECONCILE,
            body: Value::Null,
        }
    }

    pub fn list_secrets() -> Self {
        Self {
            endpoint: EdgeEndpoint::SECRETS_LIST,
            body: Value::Null,
        }
    }

    pub fn set_secret(request: &SecretSetRequest) -> Self {
        Self {
            endpoint: EdgeEndpoint::SECRET_SET,
            body: request.to_json_body(),
        }
    }

    pub fn get_escrow_releaser() -> Self {
        Self {
            endpoint: EdgeEndpoint::ESCROW_RELEASER_GET,
            body: Value::Null,
        }
    }

    pub fn make_escrow_releaser(request: &EscrowReleaserMakeRequest) -> Self {
        Self {
            endpoint: EdgeEndpoint::ESCROW_RELEASER_MAKE,
            body: request.to_json_body(),
        }
    }
}

/// A secret value that must never be logged or persisted.
///
/// `Debug` and `Display` intentionally redact the contents. Serialization is
/// provided only for building the operator request body; callers must not route
/// the serialized form to logs, traces, crash reports, local storage, or URLs.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SecretValue(String);

impl SecretValue {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn trimmed_is_empty(&self) -> bool {
        self.0.trim().is_empty()
    }

    /// Exposes the raw value for the sealed transport/request body.
    ///
    /// Do not pass the returned string to logging, formatting for diagnostics,
    /// persistence, or UI rendering.
    pub fn expose_for_transport(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SecretValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretValue([REDACTED])")
    }
}

impl fmt::Display for SecretValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

/// Request body for `POST /secrets`.
///
/// The edge operator expects `{ "name": ..., "value": ... }`. The secret value
/// is redacted in this type's debug output.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretSetRequest {
    pub name: String,
    pub value: SecretValue,
}

impl SecretSetRequest {
    pub fn new(name: impl Into<String>, value: SecretValue) -> Self {
        Self {
            name: name.into(),
            value,
        }
    }

    pub fn validate(&self) -> Result<(), EdgeRequestError> {
        if self.name.trim().is_empty() {
            return Err(EdgeRequestError::InvalidName);
        }
        if self.value.trimmed_is_empty() {
            return Err(EdgeRequestError::EmptySecretValue);
        }
        Ok(())
    }

    pub fn to_json_body(&self) -> Value {
        json!({
            "name": self.name,
            "value": self.value.expose_for_transport(),
        })
    }
}

impl fmt::Debug for SecretSetRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SecretSetRequest")
            .field("name", &self.name)
            .field("value", &self.value)
            .finish()
    }
}

/// Metadata for a secret stored on the edge.
///
/// This intentionally has no secret value field.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SecretListEntry {
    #[serde(default)]
    pub name: String,
    /// Whether the secret is sealed/encrypted as reported by the edge.
    #[serde(default)]
    pub sealed: bool,
    /// Update timestamp as reported by the edge. May be epoch seconds or an
    /// RFC3339/string form depending on edge version; parsing helpers keep both.
    #[serde(default)]
    pub updated: Option<Value>,
}

impl SecretListEntry {
    pub fn from_value(value: &Value) -> Self {
        Self {
            name: value
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            sealed: value.get("sealed").and_then(Value::as_bool).unwrap_or(false),
            updated: value.get("updated").cloned(),
        }
    }

    pub fn updated_unix_secs(&self) -> Option<i64> {
        match self.updated.as_ref()? {
            Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|v| v as i64)),
            Value::String(s) => chrono::DateTime::parse_from_rfc3339(s)
                .ok()
                .map(|dt| dt.timestamp())
                .or_else(|| s.parse::<i64>().ok()),
            _ => None,
        }
    }

    pub fn updated_label(&self) -> String {
        match self.updated.as_ref() {
            Some(Value::String(s)) if !s.trim().is_empty() => s.clone(),
            Some(v) => {
                if let Some(secs) = self.updated_unix_secs() {
                    chrono::DateTime::from_timestamp(secs, 0)
                        .map(|d| d.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
                        .unwrap_or_else(|| v.to_string())
                } else {
                    v.to_string()
                }
            }
            None => "-".to_string(),
        }
    }
}

/// Safe parsing of `GET /secrets` response body.
///
/// Accepts an array of entries; non-array or malformed items are ignored.
pub fn parse_secret_list(body: &Value) -> Vec<SecretListEntry> {
    body.as_array()
        .into_iter()
        .flatten()
        .map(SecretListEntry::from_value)
        .filter(|e| !e.name.is_empty())
        .collect()
}

/// Request for `POST /escrow/releaser`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct EscrowReleaserMakeRequest {
    /// Replace an existing releaser key when supported by the edge.
    #[serde(default, skip_serializing_if = "is_false")]
    pub replace: bool,
}

fn is_false(v: &bool) -> bool {
    !*v
}

impl EscrowReleaserMakeRequest {
    pub fn status() -> Self {
        Self { replace: false }
    }

    pub fn make_new() -> Self {
        Self { replace: false }
    }

    pub fn replace_existing() -> Self {
        Self { replace: true }
    }

    pub fn to_json_body(&self) -> Value {
        if self.replace {
            json!({ "replace": true })
        } else {
            Value::Null
        }
    }
}

/// Response/status for the escrow releaser.
///
/// This reports addresses and public state only; it never contains private keys.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct EscrowReleaserStatus {
    /// Primary releaser address as reported by the edge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    /// Solana address variant when reported.
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "solanaAddress")]
    pub solana_address: Option<String>,
    /// Optional notes/messages from the edge (public, non-secret).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    /// Preserve unknown fields for forward compatibility / mesh-pane diagnostics.
    #[serde(flatten, default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub extra: serde_json::Map<String, Value>,
}

impl EscrowReleaserStatus {
    pub fn from_value(body: &Value) -> Self {
        let mut status = Self {
            address: body
                .get("address")
                .and_then(Value::as_str)
                .map(str::to_string),
            solana_address: body
                .get("solanaAddress")
                .and_then(Value::as_str)
                .map(str::to_string),
            notes: Vec::new(),
            extra: serde_json::Map::new(),
        };
        if let Some(arr) = body.get("notes").and_then(Value::as_array) {
            status.notes = arr
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect();
        }
        if let Some(obj) = body.as_object() {
            for (k, v) in obj {
                if matches!(k.as_str(), "address" | "solanaAddress" | "notes") {
                    continue;
                }
                status.extra.insert(k.clone(), v.clone());
            }
        }
        status
    }

    pub fn is_configured(&self) -> bool {
        self.address.as_deref().is_some_and(|s| !s.trim().is_empty())
    }
}

/// Result of a reconcile call.
///
/// The edge may return `null`, a string, or a JSON object with a message/detail.
/// Helpers keep the public message only.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ReconcileResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(default)]
    pub raw: Value,
}

impl ReconcileResult {
    pub fn from_value(body: &Value) -> Self {
        let message = body
            .get("message")
            .and_then(Value::as_str)
            .or_else(|| body.get("result").and_then(Value::as_str))
            .or_else(|| body.as_str())
            .map(str::to_string);
        let detail = body
            .get("detail")
            .and_then(Value::as_str)
            .map(str::to_string);
        Self {
            message,
            detail,
            raw: body.clone(),
        }
    }

    pub fn summary(&self) -> String {
        if let Some(msg) = self.message.as_deref().filter(|s| !s.trim().is_empty()) {
            return msg.to_string();
        }
        if let Some(detail) = self.detail.as_deref().filter(|s| !s.trim().is_empty()) {
            return detail.to_string();
        }
        if self.raw.is_null() {
            "Reconciled".to_string()
        } else {
            "Reconcile completed".to_string()
        }
    }
}

/// A UI-safe summary for mesh/edge panes.
///
/// Contains public status only. It never includes secret values or private key
/// material.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct EdgeClusterPaneState {
    pub reconcile_message: Option<String>,
    pub secret_count: usize,
    pub sealed_secret_count: usize,
    pub secret_names: Vec<String>,
    pub escrow_configured: bool,
    pub escrow_address: Option<String>,
    pub escrow_solana_address: Option<String>,
}

impl EdgeClusterPaneState {
    pub fn apply_reconcile(&mut self, result: &ReconcileResult) {
        self.reconcile_message = Some(result.summary());
    }

    pub fn apply_secrets(&mut self, entries: &[SecretListEntry]) {
        self.secret_count = entries.len();
        self.sealed_secret_count = entries.iter().filter(|e| e.sealed).count();
        self.secret_names = entries.iter().map(|e| e.name.clone()).collect();
    }

    pub fn apply_escrow_releaser(&mut self, status: &EscrowReleaserStatus) {
        self.escrow_configured = status.is_configured();
        self.escrow_address = status.address.clone();
        self.escrow_solana_address = status.solana_address.clone();
    }
}

/// Request validation / mapping errors local to this module.
#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum EdgeRequestError {
    #[error("secret name must not be empty")]
    InvalidName,
    #[error("secret value must not be empty")]
    EmptySecretValue,
}

/// High-level classification for callers integrating these operations.
///
/// Use this to decide whether an operation can be attempted with the current
/// platform transport.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeAvailability {
    /// Requires an edge operator envelope transport (desktop/native/TUI-style).
    EnvelopeOnly,
    /// Can be represented as a box bearer `/api/*` call.
    BoxBearer,
}

pub fn availability(endpoint: EdgeEndpoint) -> EdgeAvailability {
    match endpoint.transport {
        EdgeTransport::EnvelopeOperator => EdgeAvailability::EnvelopeOnly,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_value_redacts_debug_and_display() {
        let secret = SecretValue::new("top-secret");
        assert_eq!(format!("{secret}"), "[REDACTED]");
        let dbg = format!("{secret:?}");
        assert!(!dbg.contains("top-secret"), "debug leaked secret: {dbg}");
        assert!(dbg.contains("REDACTED"));
    }

    #[test]
    fn secret_set_request_redacts_debug_and_keeps_body_value() {
        let req = SecretSetRequest::new("API_KEY", SecretValue::new("abc123"));
        let dbg = format!("{req:?}");
        assert!(!dbg.contains("abc123"), "debug leaked secret: {dbg}");
        assert!(dbg.contains("API_KEY"));
        let body = req.to_json_body();
        assert_eq!(body["name"], "API_KEY");
        assert_eq!(body["value"], "abc123");
    }

    #[test]
    fn secret_set_request_validation_rejects_empty_values() {
        let bad_name = SecretSetRequest::new(" ", SecretValue::new("x"));
        assert_eq!(bad_name.validate(), Err(EdgeRequestError::InvalidName));
        let bad_value = SecretSetRequest::new("KEY", SecretValue::new("   "));
        assert_eq!(bad_value.validate(), Err(EdgeRequestError::EmptySecretValue));
    }

    #[test]
    fn envelope_mappings_match_edge_operator_routes() {
        assert_eq!(EdgeEndpoint::RECONCILE.method, "POST");
        assert_eq!(EdgeEndpoint::RECONCILE.path, "/reconcile");
        assert_eq!(EdgeEndpoint::SECRETS_LIST.method, "GET");
        assert_eq!(EdgeEndpoint::SECRETS_LIST.path, "/secrets");
        assert_eq!(EdgeEndpoint::SECRET_SET.method, "POST");
        assert_eq!(EdgeEndpoint::SECRET_SET.path, "/secrets");
        assert_eq!(EdgeEndpoint::ESCROW_RELEASER_GET.method, "GET");
        assert_eq!(EdgeEndpoint::ESCROW_RELEASER_GET.path, "/escrow/releaser");
        assert_eq!(EdgeEndpoint::ESCROW_RELEASER_MAKE.method, "POST");
        assert_eq!(EdgeEndpoint::ESCROW_RELEASER_MAKE.path, "/escrow/releaser");
        assert!(EdgeEndpoint::SECRET_SET.is_envelope_only());
        assert_eq!(availability(EdgeEndpoint::RECONCILE), EdgeAvailability::EnvelopeOnly);
    }

    #[test]
    fn reconcile_and_escrow_make_body_shapes_match_tui() {
        assert_eq!(EdgeEnvelopeCall::reconcile().body, Value::Null);
        let make = EdgeEnvelopeCall::make_escrow_releaser(&EscrowReleaserMakeRequest::make_new());
        assert_eq!(make.body, Value::Null);
        let replace = EdgeEnvelopeCall::make_escrow_releaser(&EscrowReleaserMakeRequest::replace_existing());
        assert_eq!(replace.body, json!({"replace": true}));
    }

    #[test]
    fn parse_secret_list_ignores_values_and_extracts_metadata() {
        let body = json!([
            {"name": "A", "sealed": true, "updated": 123},
            {"name": "B", "sealed": false, "updated": "2026-01-01T00:00:00Z"},
            {"name": ""},
            "junk"
        ]);
        let entries = parse_secret_list(&body);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "A");
        assert!(entries[0].sealed);
        assert_eq!(entries[0].updated_unix_secs(), Some(123));
        assert_eq!(entries[1].updated_label(), "2026-01-01T00:00:00Z");
    }

    #[test]
    fn escrow_releaser_status_is_public_only() {
        let body = json!({
            "address": "addr1",
            "solanaAddress": "sol1",
            "notes": ["ok"],
            "extraField": true
        });
        let status = EscrowReleaserStatus::from_value(&body);
        assert!(status.is_configured());
        assert_eq!(status.address.as_deref(), Some("addr1"));
        assert_eq!(status.solana_address.as_deref(), Some("sol1"));
        assert_eq!(status.notes, vec!["ok"]);
        assert_eq!(status.extra.get("extraField"), Some(&json!(true)));
    }

    #[test]
    fn reconcile_result_summary_prefers_message() {
        let result = ReconcileResult::from_value(&json!({"message": "done", "detail": "x"}));
        assert_eq!(result.summary(), "done");
        let null = ReconcileResult::from_value(&Value::Null);
        assert_eq!(null.summary(), "Reconciled");
    }

    #[test]
    fn mesh_pane_state_collects_public_fields_only() {
        let mut state = EdgeClusterPaneState::default();
        state.apply_reconcile(&ReconcileResult::from_value(&Value::Null));
        state.apply_secrets(&parse_secret_list(&json!([
            {"name": "A", "sealed": true},
            {"name": "B", "sealed": false}
        ])));
        state.apply_escrow_releaser(&EscrowReleaserStatus::from_value(&json!({"address": "addr"})));
        assert_eq!(state.reconcile_message.as_deref(), Some("Reconciled"));
        assert_eq!(state.secret_count, 2);
        assert_eq!(state.sealed_secret_count, 1);
        assert_eq!(state.secret_names, vec!["A", "B"]);
        assert!(state.escrow_configured);
        assert_eq!(state.escrow_address.as_deref(), Some("addr"));
    }
}
