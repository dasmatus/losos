//! One call through the signed and encrypted endpoint. The contract is
//! docs/terminal-access.md, "One call".
//!
//! The request's plaintext is `{v, host, token, method, path, ts, nonce,
//! body}`. It is signed by the user's key and encrypted to the pinned host
//! key in one OpenPGP message, POSTed as `application/pgp-encrypted`. A
//! sealed answer must decrypt with the user's key, carry a signature by the
//! pinned host key and echo the nonce. Anything else is a plain JSON error
//! the server gave before it could trust anyone.

use losos_access::{open_call, seal_call, AnswerError, Request, SeededKey, Unlocked};
use serde_json::Value;

use crate::http::{Http, HttpError};
use crate::problem::{Problem, Result};
use crate::tr;
use crate::vault::{HostKind, HostRecord};

pub const PGP_TYPE: &str = "application/pgp-encrypted";

/// What came back.
#[derive(Debug, Clone, PartialEq)]
pub struct Answer {
    /// The inner call's status, or the HTTP status of a plain refusal.
    pub status: u16,
    /// The inner answer's JSON (a string when it was not JSON), or the plain
    /// refusal's `{error}`.
    pub body: Value,
    /// True when the answer was sealed and checked.
    pub sealed: bool,
}

impl Answer {
    pub fn ok(&self) -> bool {
        self.status < 400
    }

    /// The error sentence of a refusal, plain or sealed.
    pub fn error_text(&self) -> String {
        if let Some(e) = self.body.get("error").and_then(Value::as_str) {
            return e.to_string();
        }
        if let Some(s) = self.body.as_str().map(str::trim).filter(|s| !s.is_empty()) {
            return s.chars().take(300).collect();
        }
        format!("HTTP {}", self.status)
    }

    /// The body of a success, or a problem carrying the host's own sentence.
    pub fn into_result(self) -> Result<Value> {
        if self.ok() {
            Ok(self.body)
        } else {
            Err(Problem::new("losos::refused", self.error_text()))
        }
    }
}

/// Anything that can carry a call: the envelope, or a table of answers in
/// tests and the screenshots.
pub trait Transport: Send + Sync {
    fn request(&self, method: &str, path: &str, body: Value) -> Result<Answer>;

    fn get(&self, path: &str) -> Result<Value> {
        self.request("GET", path, Value::Null)?.into_result()
    }

    fn post(&self, path: &str, body: Value) -> Result<Value> {
        self.request("POST", path, body)?.into_result()
    }
}

pub fn call_url(url: &str, kind: HostKind) -> String {
    match kind {
        HostKind::Edge => format!("{url}/operator/call"),
        HostKind::Box => format!("{url}/api/access/call"),
    }
}

pub fn host_key_url(url: &str, kind: HostKind) -> String {
    match kind {
        HostKind::Edge => format!("{url}/operator/host-key"),
        HostKind::Box => format!("{url}/api/access/host-key"),
    }
}

pub fn new_nonce() -> String {
    hex::encode(rand::random::<[u8; 16]>())
}

pub fn unreachable(url: &str, error: HttpError) -> Problem {
    match error {
        HttpError::Unreachable(detail) => Problem::new(
            "losos::unreachable",
            tr!("tui.envelope.unreachable", url = url, detail = detail),
        ),
        HttpError::CertificateChanged => Problem::new(
            "losos::certificate_changed",
            tr!("tui.host.certificateChanged", url = url),
        )
        .help(tr!("tui.host.certificateHelp", url = url)),
    }
}

/// A pinned host, the key that signs for it, and the connection to it.
pub struct Session {
    pub url: String,
    pub kind: HostKind,
    pub host_key: SeededKey,
    /// 40 hex, upper case: what goes into the envelope's `host`.
    pub host_fingerprint: String,
    pub token: String,
    pub user: Unlocked,
    pub http: Http,
    /// The owner's word for `user`, when `user` is a companion key.
    pub delegation: Option<String>,
}

impl Session {
    pub fn new(record: &HostRecord, user: Unlocked, http: Http) -> Result<Session> {
        let host_key = SeededKey::from_armored(&record.public_key)
            .map_err(|_| Problem::new("losos::pin", tr!("tui.host.pinBroken")))?;
        if !crate::vault::same_fingerprint(&host_key.fingerprint, &record.fingerprint) {
            return Err(Problem::new("losos::pin", tr!("tui.host.pinBroken")));
        }
        Ok(Session {
            url: record.url.clone(),
            kind: record.kind,
            host_fingerprint: host_key.fingerprint.clone(),
            host_key,
            token: record.token.clone(),
            user,
            http,
            delegation: None,
        })
    }

    pub fn seal(
        &self,
        method: &str,
        path: &str,
        body: Value,
        ts: i64,
        nonce: &str,
    ) -> Result<String> {
        let request = Request {
            v: 1,
            host: self.host_fingerprint.clone(),
            token: self.token.clone(),
            method: method.to_ascii_uppercase(),
            path: path.to_string(),
            ts,
            nonce: nonce.to_string(),
            body,
            delegation: self.delegation.clone(),
        };
        seal_call(&self.user, &self.host_key, &request)
            .map_err(|e| Problem::new("losos::seal", e.to_string()))
    }

    /// Open a sealed answer to the request that carried `nonce`.
    pub fn open(&self, armored: &str, nonce: &str) -> Result<(u16, Value)> {
        let answer = open_call(&self.user, &self.host_key, armored.as_bytes()).map_err(|e| {
            let key = match e {
                AnswerError::NotForUs | AnswerError::BadSignature => "tui.envelope.badSignature",
                AnswerError::NotAnswer => "tui.envelope.notAnswer",
            };
            Problem::new("losos::envelope", crate::i18n::t(key))
        })?;
        if !answer.nonce.eq_ignore_ascii_case(nonce) {
            return Err(Problem::new(
                "losos::envelope",
                tr!("tui.envelope.wrongNonce"),
            ));
        }
        Ok((answer.status, answer.body))
    }
}

impl Transport for Session {
    fn request(&self, method: &str, path: &str, body: Value) -> Result<Answer> {
        let nonce = new_nonce();
        let armored = self.seal(method, path, body, losos_access::now_secs(), &nonce)?;
        let _span = tracing::debug_span!("call", %method, %path).entered();
        let response = self
            .http
            .post(
                &call_url(&self.url, self.kind),
                PGP_TYPE,
                &format!("{PGP_TYPE}, application/json"),
                &armored,
            )
            .map_err(|e| unreachable(&self.url, e))?;
        if response
            .content_type
            .to_ascii_lowercase()
            .starts_with(PGP_TYPE)
        {
            let (status, body) = self.open(&response.body, &nonce)?;
            tracing::debug!(status, "sealed answer");
            return Ok(Answer {
                status,
                body,
                sealed: true,
            });
        }
        tracing::debug!(status = response.status, "plain answer");
        // An HTML error page from a proxy: keep the text.
        let body = serde_json::from_str(&response.body).unwrap_or(Value::String(response.body));
        Ok(Answer {
            status: response.status,
            body,
            sealed: false,
        })
    }
}
