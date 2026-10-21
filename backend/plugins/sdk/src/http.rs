//! HTTP to the manifest's `http` endpoints.

use crate::wit::http as w;

pub use w::Body;

/// A request to send. Built like a builder; [`Request::fetch`] or
/// [`Request::send`] sends it.
#[derive(Debug, Clone)]
pub struct Request {
    method: String,
    url: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    timeout_ms: Option<u32>,
}

/// A whole answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Response {
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    #[must_use]
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    #[must_use]
    pub fn json<T: serde::de::DeserializeOwned>(&self) -> Option<T> {
        serde_json::from_slice(&self.body).ok()
    }

    #[must_use]
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

/// An answer whose body is read as it arrives.
pub struct Streamed {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Body,
}

impl Request {
    #[must_use]
    pub fn new(method: &str, url: &str) -> Self {
        Request {
            method: method.to_string(),
            url: url.to_string(),
            headers: Vec::new(),
            body: Vec::new(),
            timeout_ms: None,
        }
    }

    #[must_use]
    pub fn get(url: &str) -> Self {
        Request::new("GET", url)
    }

    #[must_use]
    pub fn post(url: &str) -> Self {
        Request::new("POST", url)
    }

    #[must_use]
    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_string(), value.to_string()));
        self
    }

    #[must_use]
    pub fn body(mut self, body: Vec<u8>) -> Self {
        self.body = body;
        self
    }

    /// A JSON body and its content type.
    #[must_use]
    pub fn json(self, value: &serde_json::Value) -> Self {
        self.header("content-type", "application/json")
            .body(serde_json::to_vec(value).unwrap_or_default())
    }

    #[must_use]
    pub fn timeout_ms(mut self, ms: u32) -> Self {
        self.timeout_ms = Some(ms);
        self
    }

    fn wit(&self) -> w::Outgoing {
        w::Outgoing {
            method: self.method.clone(),
            url: self.url.clone(),
            headers: self.headers.clone(),
            body: self.body.clone(),
            timeout_ms: self.timeout_ms,
        }
    }

    /// Send it and read the whole answer, at most `max_body` bytes of body.
    pub fn fetch(&self, max_body: u64) -> crate::Result<Response> {
        let r = w::fetch(&self.wit(), max_body)?;
        Ok(Response {
            status: r.status,
            headers: r.headers,
            body: r.body,
        })
    }

    /// Send it and stream the answer's body.
    pub fn send(&self) -> crate::Result<Streamed> {
        let r = w::send(&self.wit())?;
        Ok(Streamed {
            status: r.status,
            headers: r.headers,
            body: r.body,
        })
    }
}

impl Streamed {
    /// Up to `max` more bytes; `None` at the end.
    pub fn read(&self, max: u32) -> crate::Result<Option<Vec<u8>>> {
        self.body.read(max).map_err(Into::into)
    }
}
