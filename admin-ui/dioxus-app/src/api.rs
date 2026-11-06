//! HTTP client for the box's JSON API — the same surface the admin SPA and the
//! companion use (`Authorization: Bearer <token>`, `/api/*`).
//!
//! Trust modes and formatting/helpers are shared via `losos-client` (reused
//! with `admin-ui/tui`). TLS behavior is aligned with the TUI's model
//! (`admin-ui/tui/src/http.rs`); full TOFU cert-hash verification is a
//! follow-up hardening step.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fmt;
use std::time::Duration;

// Shared primitives (reused across LosOS clients).
pub use losos_client::{
    describe_window, encode_query, format_bytes, format_stamp, minutes_of, opt_array, opt_bool,
    opt_f64, opt_i64, opt_obj, opt_str, short_sha, Trust,
};

#[derive(Debug, Clone)]
pub struct ApiError {
    pub status: u16,
    pub message: String,
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ApiError {}

#[derive(Clone)]
pub struct BoxApi {
    base: String,
    token: String,
    trust: Trust,
    http: reqwest::Client,
}

impl BoxApi {
    pub fn new(address: &str, token: impl Into<String>, trust: Trust) -> Result<Self, ApiError> {
        let base = if address.starts_with("http://") || address.starts_with("https://") {
            address.trim_end_matches('/').to_string()
        } else {
            format!("https://{}", address.trim_end_matches('/'))
        };
        let http = build_client(&trust)?;
        Ok(Self {
            base,
            token: token.into(),
            trust,
            http,
        })
    }

    pub fn base(&self) -> &str {
        &self.base
    }
    pub fn trust(&self) -> &Trust {
        &self.trust
    }

    pub async fn get(&self, path: &str) -> Result<Value, ApiError> {
        self.request(reqwest::Method::GET, path, None, false).await
    }
    pub async fn get_anonymous(&self, path: &str) -> Result<Value, ApiError> {
        self.request(reqwest::Method::GET, path, None, true).await
    }
    pub async fn post(&self, path: &str) -> Result<Value, ApiError> {
        self.request(reqwest::Method::POST, path, Some(Value::Null), false)
            .await
    }
    pub async fn post_json(&self, path: &str, body: Value) -> Result<Value, ApiError> {
        self.request(reqwest::Method::POST, path, Some(body), false)
            .await
    }

    async fn request(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<Value>,
        anonymous: bool,
    ) -> Result<Value, ApiError> {
        let url = format!("{}{}", self.base, path);
        let mut req = self
            .http
            .request(method, &url)
            .timeout(Duration::from_secs(20));
        if !anonymous && !self.token.is_empty() {
            req = req.bearer_auth(&self.token);
        }
        if let Some(b) = body {
            if !b.is_null() {
                req = req.json(&b);
            }
        }
        let resp = req.send().await.map_err(|e| ApiError {
            status: 0,
            message: format!("No answer from {}: {}", self.base, e),
        })?;
        let status = resp.status().as_u16();
        let text = resp.text().await.unwrap_or_default();
        if status == 401 {
            return Err(ApiError {
                status,
                message: "The box wants its token again".into(),
            });
        }
        if !(200..300).contains(&status) {
            let msg = serde_json::from_str::<Value>(&text)
                .ok()
                .and_then(|v| v.get("error").and_then(Value::as_str).map(str::to_string))
                .unwrap_or_else(|| format!("HTTP {status}"));
            return Err(ApiError {
                status,
                message: msg,
            });
        }
        if text.trim().is_empty() {
            return Ok(json!({}));
        }
        serde_json::from_str(&text).map_err(|_| ApiError {
            status,
            message: "The box sent a response that was not JSON".into(),
        })
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn build_client(trust: &Trust) -> Result<reqwest::Client, ApiError> {
    let tls = crate::tls::client_config(trust).map_err(|e| ApiError {
        status: 0,
        message: e,
    })?;
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(4))
        .user_agent(format!("losos-frontend/{}", env!("CARGO_PKG_VERSION")))
        .use_preconfigured_tls(tls)
        .build()
        .map_err(|e| ApiError {
            status: 0,
            message: e.to_string(),
        })
}

#[cfg(target_arch = "wasm32")]
fn build_client(_trust: &Trust) -> Result<reqwest::Client, ApiError> {
    reqwest::Client::builder().build().map_err(|e| ApiError {
        status: 0,
        message: e.to_string(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StorageSnapshot {
    pub used_gb: f64,
    pub total_gb: f64,
    pub label: String,
}

impl StorageSnapshot {
    pub fn fraction(&self) -> f64 {
        if self.total_gb > 0.0 {
            (self.used_gb / self.total_gb).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
    pub fn phrase(&self) -> String {
        format!(
            "{} GB used of {} GB",
            self.used_gb.round() as i64,
            self.total_gb.round() as i64
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn format_bytes_matches_human_units() {
        assert_eq!(format_bytes(None), "–");
        assert_eq!(format_bytes(Some(512.0 * 1024.0 * 1024.0)), "512 MB");
        assert_eq!(format_bytes(Some(1.5 * 1024.0 * 1024.0 * 1024.0)), "1.5 GB");
    }

    #[test]
    fn window_description_wraps_midnight() {
        assert_eq!(
            describe_window("22:00", "06:00"),
            "22:00–06:00, 8 h a day, wrapping midnight"
        );
    }

    #[test]
    fn json_helpers_read_missing_as_absent() {
        let v = json!({"a": "x", "n": 3, "ok": true, "obj": {"k": 1}});
        assert_eq!(opt_str(&v, "a").as_deref(), Some("x"));
        assert_eq!(opt_i64(&v, "n"), Some(3));
        assert_eq!(opt_bool(&v, "ok"), Some(true));
    }

    #[test]
    fn config_needs_connect_like_companion() {
        let mut c = crate::store::BoxConfig::default();
        assert!(c.needs_connect());
        c.host = "box.local".into();
        assert!(!c.needs_connect());
        c.demo = false;
        c.token.clear();
        assert!(c.needs_connect());
        c.token = "abc".into();
        assert!(!c.needs_connect());
    }
}
