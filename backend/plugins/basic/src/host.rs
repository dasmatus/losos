//! Everything the pages ask of the box, behind one trait so the pages can
//! be drawn in a test from fixtures: lososd's own routes as the caller
//! ([`losos_plugin::dispatch`]), LosOS Login's API, the built admin page's
//! files and whether LosOS Cloud and LosOS Git are running.

use losos_plugin::{config, dispatch, fs, http, Caller, Request};
use serde_json::Value;

/// One answer from lososd: its status and its JSON (or `Null`).
#[derive(Debug, Clone, PartialEq)]
pub struct Answer {
    pub status: u16,
    pub json: Value,
}

impl Answer {
    #[must_use]
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// The JSON when the call went through.
    #[must_use]
    pub fn value(self) -> Option<Value> {
        self.ok().then_some(self.json)
    }
}

/// An answer from LosOS Login, with the cookies it set.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LoginAnswer {
    pub status: u16,
    pub json: Value,
    pub cookies: Vec<String>,
}

pub trait Host {
    /// One of lososd's routes, as the caller of this page.
    fn call(&mut self, method: &str, path: &str, content_type: Option<&str>, body: &[u8])
        -> Answer;

    fn get(&mut self, path: &str) -> Option<Value> {
        self.call("GET", path, None, &[]).value()
    }

    /// LosOS Login's API at `path` (under `/auth/api/`), as the browser
    /// would reach it through nginx: `forwarded` is its host, scheme and
    /// address, `session` its cookie when it has one.
    fn login(
        &mut self,
        method: &str,
        path: &str,
        forwarded: &Forwarded,
        session: Option<&str>,
        body: Option<&Value>,
    ) -> Option<LoginAnswer>;

    /// Whether the spare admin key opens lososd.
    fn key_opens(&mut self, key: &str) -> bool;

    /// A file of the built admin page, by its path under the page's root.
    fn ui_file(&mut self, path: &str) -> Option<Vec<u8>>;

    /// Whether LosOS Cloud and LosOS Git answer on this box.
    fn apps_running(&mut self) -> (bool, bool);

    /// Whether this box edits a folder of documents (`losos.office.folder`),
    /// which puts Documents in the sidebar.
    fn office_folder(&mut self) -> bool {
        false
    }
}

/// Where the browser's request came from, as nginx said.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Forwarded {
    pub host: String,
    pub proto: String,
    pub address: String,
}

impl Forwarded {
    #[must_use]
    pub fn origin(&self) -> String {
        format!("{}://{}", self.proto, self.host)
    }
}

/// The box itself.
pub struct Real {
    pub caller: Caller,
}

impl Host for Real {
    fn call(
        &mut self,
        method: &str,
        path: &str,
        content_type: Option<&str>,
        body: &[u8],
    ) -> Answer {
        let (path, query) = path.split_once('?').unwrap_or((path, ""));
        let request = Request {
            method: method.to_string(),
            path: path.to_string(),
            route: String::new(),
            params: Vec::new(),
            query: query.to_string(),
            headers: content_type
                .map(|t| vec![("content-type".to_string(), t.to_string())])
                .unwrap_or_default(),
            body: body.to_vec(),
            caller: self.caller.clone(),
        };
        match dispatch::call(request) {
            Ok(r) => Answer {
                status: r.status,
                json: serde_json::from_slice(&r.body).unwrap_or(Value::Null),
            },
            Err(e) => {
                losos_plugin::log::warn(&format!("{method} {path} did not answer: {e}"));
                Answer {
                    status: 502,
                    json: Value::Null,
                }
            }
        }
    }

    fn login(
        &mut self,
        method: &str,
        path: &str,
        forwarded: &Forwarded,
        session: Option<&str>,
        body: Option<&Value>,
    ) -> Option<LoginAnswer> {
        let base = config::get("login")?;
        let mut req = http::Request::new(method, &format!("{base}/auth/api/{path}"))
            .header("x-forwarded-host", &forwarded.host)
            .header("x-forwarded-proto", &forwarded.proto)
            .header("x-forwarded-for", &forwarded.address)
            .header("accept", "application/json")
            .timeout_ms(15_000);
        if let Some(cookie) = session {
            req = req.header("cookie", &format!("authelia_session={cookie}"));
        }
        if let Some(body) = body {
            req = req.json(body);
        }
        if method == "GET" && path.starts_with("authz/") {
            req = req
                .header("x-original-url", &format!("{}/basic/", forwarded.origin()))
                .header("x-original-method", "GET");
        }
        match req.fetch(256 * 1024) {
            Ok(r) => Some(LoginAnswer {
                status: r.status,
                json: serde_json::from_slice(&r.body).unwrap_or(Value::Null),
                cookies: r
                    .headers
                    .iter()
                    .filter(|(k, _)| k.eq_ignore_ascii_case("set-cookie"))
                    .map(|(_, v)| v.clone())
                    .collect(),
            }),
            Err(e) => {
                losos_plugin::log::warn(&format!("LosOS Login did not answer {path}: {e}"));
                None
            }
        }
    }

    fn key_opens(&mut self, key: &str) -> bool {
        let base = config::get_or("api", "http://127.0.0.1:8082");
        http::Request::get(&format!("{base}/api/me"))
            .header("authorization", &format!("Bearer {key}"))
            .timeout_ms(10_000)
            .fetch(64 * 1024)
            .is_ok_and(|r| {
                r.is_success()
                    && r.json::<Value>()
                        .is_some_and(|v| v["spareKey"] == Value::Bool(true))
            })
    }

    fn ui_file(&mut self, path: &str) -> Option<Vec<u8>> {
        let root = config::get("ui")?;
        fs::read_optional(&fs::join(&root, path)).ok().flatten()
    }

    fn apps_running(&mut self) -> (bool, bool) {
        let up = |key: &str| config::get(key).is_some_and(|p| fs::exists(&p).unwrap_or(false));
        (up("cloudSocket"), up("gitSocket"))
    }

    fn office_folder(&mut self) -> bool {
        config::get("officeSocket").is_some_and(|p| fs::exists(&p).unwrap_or(false))
    }
}
