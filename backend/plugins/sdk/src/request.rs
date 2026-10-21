//! The request a plugin answers and the response it gives.

use crate::wit::types as w;
use serde::de::DeserializeOwned;
use std::collections::BTreeMap;

/// A person LosOS Login signed in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Person {
    pub name: String,
    pub display_name: String,
    pub groups: Vec<String>,
    pub admin: bool,
}

/// Who the host says made the call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Principal {
    Anonymous,
    Spare,
    Person(Person),
    Edge,
    Host,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Caller {
    pub principal: Principal,
    pub remote: String,
    pub section: String,
}

impl Caller {
    /// The signed-in person, if a person made the call.
    #[must_use]
    pub fn person(&self) -> Option<&Person> {
        match &self.principal {
            Principal::Person(p) => Some(p),
            _ => None,
        }
    }

    /// The spare key, an administrator, or lososd itself.
    #[must_use]
    pub fn is_admin(&self) -> bool {
        match &self.principal {
            Principal::Spare | Principal::Host => true,
            Principal::Person(p) => p.admin,
            Principal::Anonymous | Principal::Edge => false,
        }
    }

    /// The name the audit log and the journal use.
    #[must_use]
    pub fn name(&self) -> &str {
        match &self.principal {
            Principal::Anonymous => "anonymous",
            Principal::Spare => "spare-key",
            Principal::Person(p) => &p.name,
            Principal::Edge => "edge",
            Principal::Host => "lososd",
        }
    }
}

/// One request to a route the manifest lists.
#[derive(Debug, Clone)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub route: String,
    pub params: Vec<(String, String)>,
    pub query: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub caller: Caller,
}

impl Request {
    #[doc(hidden)]
    #[must_use]
    pub fn from_wit(r: w::Request) -> Self {
        let principal = match r.caller.principal {
            w::Principal::Anonymous => Principal::Anonymous,
            w::Principal::Spare => Principal::Spare,
            w::Principal::Person(p) => Principal::Person(Person {
                name: p.name,
                display_name: p.display_name,
                groups: p.groups,
                admin: p.admin,
            }),
            w::Principal::Edge => Principal::Edge,
            w::Principal::Host => Principal::Host,
        };
        Request {
            method: r.method,
            path: r.path,
            route: r.route,
            params: r.params,
            query: r.query,
            headers: r.headers,
            body: r.body,
            caller: Caller {
                principal,
                remote: r.caller.remote,
                section: r.caller.section,
            },
        }
    }

    #[doc(hidden)]
    #[must_use]
    pub fn into_wit(self) -> w::Request {
        let principal = match self.caller.principal {
            Principal::Anonymous => w::Principal::Anonymous,
            Principal::Spare => w::Principal::Spare,
            Principal::Person(p) => w::Principal::Person(w::Person {
                name: p.name,
                display_name: p.display_name,
                groups: p.groups,
                admin: p.admin,
            }),
            Principal::Edge => w::Principal::Edge,
            Principal::Host => w::Principal::Host,
        };
        w::Request {
            method: self.method,
            path: self.path,
            route: self.route,
            params: self.params,
            query: self.query,
            headers: self.headers,
            body: self.body,
            caller: w::Caller {
                principal,
                remote: self.caller.remote,
                section: self.caller.section,
            },
        }
    }

    /// A `{name}` segment of the route.
    #[must_use]
    pub fn param(&self, name: &str) -> Option<&str> {
        self.params
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    /// A header, by its lower-case name.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// The query string as a map; the last of a repeated key wins.
    #[must_use]
    pub fn query_map(&self) -> BTreeMap<String, String> {
        self.query
            .split('&')
            .filter(|pair| !pair.is_empty())
            .map(|pair| {
                let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
                (percent_decode(k), percent_decode(v))
            })
            .collect()
    }

    /// One query parameter.
    #[must_use]
    pub fn query_param(&self, name: &str) -> Option<String> {
        self.query_map().remove(name)
    }

    /// The body as JSON, or `None` when it is not.
    #[must_use]
    pub fn json<T: DeserializeOwned>(&self) -> Option<T> {
        serde_json::from_slice(&self.body).ok()
    }

    /// The body as a JSON value; `null` when it is not JSON, which is what
    /// the handlers lososd started with did with a bad body.
    #[must_use]
    pub fn json_value(&self) -> serde_json::Value {
        serde_json::from_slice(&self.body).unwrap_or_default()
    }
}

/// `application/x-www-form-urlencoded` decoding: `+` is a space, `%XX` a
/// byte; a malformed escape is kept as it is.
#[must_use]
pub fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
                match hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                    Some(b) => {
                        out.push(b);
                        i += 2;
                    }
                    None => out.push(b'%'),
                }
            }
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// What a plugin answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Response {
    #[must_use]
    pub fn new(status: u16) -> Self {
        Response {
            status,
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    /// A JSON body.
    #[must_use]
    pub fn json(status: u16, value: &serde_json::Value) -> Self {
        Response::new(status)
            .header("content-type", "application/json")
            .body(serde_json::to_vec(value).unwrap_or_default())
    }

    /// 200 with a JSON body.
    #[must_use]
    pub fn ok(value: &serde_json::Value) -> Self {
        Response::json(200, value)
    }

    /// `{"error": message}`, the shape every admin page reads.
    #[must_use]
    pub fn error(status: u16, message: &str) -> Self {
        Response::json(status, &serde_json::json!({ "error": message }))
    }

    /// The 500 for a failure whose detail is for the journal only: the
    /// detail is logged here and the client gets the sentence lososd always
    /// gave.
    #[must_use]
    pub fn failed(detail: &dyn std::fmt::Display) -> Self {
        crate::log::error(&format!("command failed: {detail:#}"));
        Response::error(500, "command failed; see the lososd journal")
    }

    /// Bytes of a given type.
    #[must_use]
    pub fn bytes(status: u16, content_type: &str, body: Vec<u8>) -> Self {
        Response::new(status)
            .header("content-type", content_type)
            .body(body)
    }

    #[must_use]
    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers
            .push((name.to_ascii_lowercase(), value.to_string()));
        self
    }

    #[must_use]
    pub fn body(mut self, body: Vec<u8>) -> Self {
        self.body = body;
        self
    }

    #[doc(hidden)]
    #[must_use]
    pub fn into_wit(self) -> w::Response {
        w::Response {
            status: self.status,
            headers: self.headers,
            body: self.body,
        }
    }

    #[doc(hidden)]
    #[must_use]
    pub fn from_wit(r: w::Response) -> Self {
        Response {
            status: r.status,
            headers: r.headers,
            body: r.body,
        }
    }

    /// The body as JSON, for tests and for [`crate::dispatch`].
    #[must_use]
    pub fn json_value(&self) -> serde_json::Value {
        serde_json::from_slice(&self.body).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::percent_decode;

    #[test]
    fn decodes_form_escapes() {
        assert_eq!(percent_decode("a+b%2Fc"), "a b/c");
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(percent_decode("%zz"), "%zz");
        assert_eq!(percent_decode("%C3%A9"), "é");
    }
}
