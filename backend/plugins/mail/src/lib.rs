//! The mail plugin: a person's own folders and their filters in LosOS
//! Mail, beside lososd's `/api/mail` (backend/src/mail.rs), which reads and
//! sends the mail itself.
//!
//! Every call is made to Stalwart as the person LosOS Login signed in,
//! through its administrator, as lososd's own mail calls are:
//! `<name>@<domain>%losos` with the secret `modules/mail.nix` made. The
//! manifest grants that one loopback origin and that one file, and both
//! exist only while mail is live on this box; otherwise every route
//! answers 404, as lososd's do.
//! What the routes do is [`model`]; what a filter is, and the Sieve it
//! becomes, is [`sieve`].

pub mod base64;
pub mod model;
pub mod sieve;

use losos_plugin::{cache, config, fs, http, lock, Plugin, Request, Response};
use model::{odd, refuse, Refused, Server};
use serde_json::{json, Value};

/// lososd's own name for the server's administrator (backend/src/mail.rs).
const MASTER_USER: &str = "losos";
/// The group LosOS Login holds an unfinished account in.
const LOCKED: &str = "losos-locked";
const USING: [&str; 3] = [
    "urn:ietf:params:jmap:core",
    "urn:ietf:params:jmap:mail",
    "urn:ietf:params:jmap:sieve",
];
/// The largest answer read: a folder list, or a script of a hundred
/// filters.
const MAX_ANSWER: u64 = 8 * 1024 * 1024;
/// How long a person's account id is remembered.
const ACCOUNT_SECS: u32 = 3600;

/// Stalwart over its loopback listener.
struct Live {
    url: String,
    user: String,
    secret: String,
    account: String,
}

fn not_up() -> Refused {
    Refused {
        status: 503,
        message: "the mail server is starting; try again in a minute".into(),
        code: None,
        detail: None,
    }
}

impl Live {
    fn open(req: &Request) -> Result<Live, Refused> {
        let url = config::get("url").filter(|u| !u.is_empty());
        let file = config::get("secretFile").filter(|f| !f.is_empty());
        let domain = config::get("domain").filter(|d| !d.is_empty());
        let (Some(url), Some(file), Some(domain)) = (url, file, domain) else {
            return refuse(404, "there is no mail on this box");
        };
        let person = match req.caller.person() {
            Some(p) if p.groups.iter().any(|g| g == LOCKED) => {
                return refuse(403, "finish what LosOS Login asks of your account first")
            }
            Some(p) => p,
            None => {
                return refuse(
                    409,
                    "the spare key has no mailbox; sign in with LosOS Login to read mail",
                )
            }
        };
        let secret = fs::read_string(&file)
            .map(|s| s.trim().to_string())
            .ok()
            .filter(|s| !s.is_empty())
            .ok_or_else(not_up)?;
        let mut live = Live {
            url: url.trim_end_matches('/').to_string(),
            user: format!("{}@{domain}%{MASTER_USER}", person.name),
            secret,
            account: String::new(),
        };
        let key = format!("account:{}", person.name);
        live.account = match cache::get(&key) {
            Some(a) => String::from_utf8_lossy(&a).into_owned(),
            None => {
                let a = live.session()?;
                cache::set(&key, a.as_bytes(), ACCOUNT_SECS);
                a
            }
        };
        Ok(live)
    }

    fn request(&self, method: &str, path: &str) -> http::Request {
        let basic = base64::encode(format!("{}:{}", self.user, self.secret).as_bytes());
        http::Request::new(method, &format!("{}{path}", self.url))
            .header("authorization", &format!("Basic {basic}"))
            .timeout_ms(20_000)
    }

    fn fetch(&self, req: http::Request) -> Result<http::Response, Refused> {
        let resp = req.fetch(MAX_ANSWER).map_err(|e| Refused {
            detail: Some(format!("reaching the mail server: {e}")),
            ..not_up()
        })?;
        match resp.status {
            200..=299 => Ok(resp),
            // lososd makes the account on the person's first visit to Mail,
            // which every page that reaches these routes has made.
            401 => refuse(409, "open Mail once first; it makes your mailbox"),
            s => Err(odd(&format!("HTTP {s}: {}", resp.text()))),
        }
    }

    /// The person's mail account id.
    fn session(&self) -> Result<String, Refused> {
        let resp = self.fetch(self.request("GET", "/jmap/session"))?;
        let doc: Value = resp.json().ok_or_else(|| odd("session JSON"))?;
        doc.pointer("/primaryAccounts/urn:ietf:params:jmap:mail")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| odd("primaryAccounts"))
    }
}

impl Server for Live {
    fn account(&self) -> &str {
        &self.account
    }

    fn jmap(&mut self, calls: Vec<Value>) -> Result<Value, Refused> {
        let body = json!({ "using": USING, "methodCalls": calls });
        let resp = self.fetch(self.request("POST", "/jmap/").json(&body))?;
        resp.json().ok_or_else(|| odd("JMAP JSON"))
    }

    fn upload(&mut self, bytes: &[u8], kind: &str) -> Result<String, Refused> {
        let path = format!("/jmap/upload/{}/", self.account);
        let req = self
            .request("POST", &path)
            .header("content-type", kind)
            .body(bytes.to_vec());
        let doc: Value = self.fetch(req)?.json().ok_or_else(|| odd("upload JSON"))?;
        doc.get("blobId")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| odd("upload blobId"))
    }

    fn download(&mut self, blob: &str) -> Result<Vec<u8>, Refused> {
        if !sieve::is_id(blob) {
            return Err(odd("a blob id that is not one"));
        }
        let path = format!(
            "/jmap/download/{}/{blob}/{}.sieve?accept=application/sieve",
            self.account,
            sieve::SCRIPT_NAME
        );
        Ok(self.fetch(self.request("GET", &path))?.body)
    }
}

fn answer(r: Result<Value, Refused>) -> Response {
    match r {
        Ok(v) => Response::ok(&v),
        Err(e) => {
            if let Some(d) = &e.detail {
                losos_plugin::log::warn(d);
            }
            Response::json(e.status, &json!({ "error": e.message, "code": e.code }))
        }
    }
}

/// The folder a route names, checked before it reaches a request.
fn folder_id(req: &Request) -> Result<String, Refused> {
    match req.param("id") {
        Some(id) if sieve::is_id(id) => Ok(id.to_string()),
        _ => refuse(404, "there is no such folder"),
    }
}

/// Anything that changes the script or empties a folder, one at a time.
fn locked(work: impl FnOnce() -> Result<Value, Refused>) -> Result<Value, Refused> {
    let _guard = lock::acquire("mail").map_err(|e| Refused {
        detail: Some(format!("the mail lock: {e}")),
        ..not_up()
    })?;
    work()
}

pub struct MailPlugin;

impl Plugin for MailPlugin {
    fn handle(req: Request) -> Response {
        let doc = req.json_value();
        answer(Live::open(&req).and_then(|mut live| {
            let server: &mut dyn Server = &mut live;
            match (req.method.as_str(), req.route.as_str()) {
                ("POST", "/api/mail/folders") => model::create_folder(server, &doc),
                ("PUT", "/api/mail/folders/{id}") => {
                    let id = folder_id(&req)?;
                    model::update_folder(server, &id, &doc)
                }
                ("DELETE", "/api/mail/folders/{id}") => {
                    let id = folder_id(&req)?;
                    locked(|| model::delete_folder(server, &id))
                }
                ("GET", "/api/mail/filters") => model::get_filters(server),
                ("PUT", "/api/mail/filters") => locked(|| model::put_filters(server, &doc)),
                _ => refuse(404, "not found"),
            }
        }))
    }
}

losos_plugin::export!(MailPlugin);
