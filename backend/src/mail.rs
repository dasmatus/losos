//! LosOS Mail: a mailbox for every person LosOS Login knows.
//!
//! The mail server is Stalwart (`modules/mail.nix`), a pod in this box's own
//! cluster that runs as the `shared` user and keeps its mailboxes in the
//! shared data domain, so it runs only while the box shares its storage;
//! with sharing off there is no mail here at all, page and API included.
//! It listens on loopback only. This daemon is the one client it has: the
//! Mail page (`admin-ui/app/mail/`) calls `/api/mail/*`, and each call is
//! made here, in JMAP, as the person Authelia signed in.
//!
//! **Who a call is made as.** Stalwart's master user lets a client sign in
//! as `<name>%<master>` with the master secret and act as `<name>`. Only
//! this daemon and the pod read that secret (`$LOSOS_MAIL_SECRET_FILE`), so
//! a person reaches their own mailbox through LosOS Login and nothing else:
//! no second password, and taking someone out of the people file takes
//! their mail away at the next call, because `crate::http` no longer
//! recognises them.
//!
//! **Accounts.** A person's account is made on their first call, with the
//! address `<name>@<domain>`: the mail domain, then the person. The same
//! secret opens Stalwart's management API as its fallback administrator,
//! which is how the domain and the account are made. Making them twice is
//! harmless (`fieldAlreadyExists`), so nothing has to remember it across
//! restarts; a set in memory saves the call after the first.
//!
//! **HTML.** A message's HTML is never put into the Mail page. It is served
//! on its own URL (`html_response` in `crate::http`) under a policy that
//! sandboxes it and refuses every script and every load off the message,
//! and the page shows that URL in an `<iframe sandbox>`. Remote pictures
//! are therefore not loaded at all: they are how a sender learns that
//! someone read their mail.
//!
//! Everything that turns a request into JMAP and JMAP into an answer is a
//! pure function, tested below; the one place that talks to the server is
//! [`Transport`].

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use serde_json::{json, Value};

/// The master user's name in Stalwart's configuration. `modules/mail.nix`
/// writes the same literal.
pub const MASTER_USER: &str = "losos";
/// The fallback administrator's name, for the management API.
pub const ADMIN_USER: &str = "losos-admin";
/// The JMAP capabilities every call uses.
const USING: [&str; 3] = [
    "urn:ietf:params:jmap:core",
    "urn:ietf:params:jmap:mail",
    "urn:ietf:params:jmap:submission",
];
/// Messages per page of a mailbox.
pub const PAGE: usize = 50;
/// Recipients one message may have.
const MAX_RECIPIENTS: usize = 50;
/// The longest subject: the line limit of RFC 5322, less the header name.
const MAX_SUBJECT: usize = 900;
/// The largest body a person may write.
const MAX_BODY: usize = 1024 * 1024;
/// The largest answer read from the server: a message with its bodies, or
/// an attachment.
const MAX_ANSWER: u64 = 64 * 1024 * 1024;

/// What the daemon needs to reach the mail server, from the environment
/// `modules/daemon.nix` sets.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// `127.0.0.1:<port>`, Stalwart's HTTP listener.
    pub addr: String,
    /// The master secret, shared with the pod.
    pub secret_file: PathBuf,
    /// The domain addresses are made in.
    pub domain: String,
}

impl Config {
    /// `$LOSOS_MAIL_ADDR`, `$LOSOS_MAIL_SECRET_FILE` and
    /// `$LOSOS_MAIL_DOMAIN`, which `modules/mail.nix` sets only while mail
    /// is live; `None` otherwise.
    pub fn from_env() -> Option<Config> {
        let var = |name: &str| std::env::var(name).ok().filter(|v| !v.trim().is_empty());
        Some(Config {
            addr: var("LOSOS_MAIL_ADDR")?,
            secret_file: PathBuf::from(var("LOSOS_MAIL_SECRET_FILE")?),
            domain: var("LOSOS_MAIL_DOMAIN")?.trim().to_ascii_lowercase(),
        })
    }
}

/// A request the Mail page cannot have, in words it shows as they are.
#[derive(Debug, Clone, PartialEq)]
pub struct Refused {
    pub status: u16,
    pub message: String,
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Refused {}

fn refuse<T>(status: u16, message: impl Into<String>) -> Result<T, Refused> {
    Err(Refused {
        status,
        message: message.into(),
    })
}

/// The server is not answering: the pod is starting, or sharing went off.
fn not_up() -> Refused {
    Refused {
        status: 503,
        message: "the mail server is starting; try again in a minute".into(),
    }
}

/// Mail is not live on this box: off, or the box does not share its
/// storage. The page does not exist then either.
fn closed() -> Refused {
    Refused {
        status: 404,
        message: "there is no mail on this box".into(),
    }
}

/// The server answered something this module did not expect.
fn odd(what: &str) -> Refused {
    tracing::warn!(what, "the mail server answered something unexpected");
    Refused {
        status: 502,
        message: "the mail server gave an answer LosOS Mail does not understand".into(),
    }
}

// ── The wire to Stalwart ───────────────────────────────────────────────────

/// One HTTP request to the mail server.
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    pub method: &'static str,
    pub path: String,
    /// Basic credentials: the user and the secret.
    pub user: String,
    pub secret: String,
    pub body: Option<Vec<u8>>,
}

/// Its answer.
#[derive(Debug, Clone, PartialEq)]
pub struct Response {
    pub status: u16,
    pub content_type: String,
    pub body: Vec<u8>,
}

/// Where requests go: the server over loopback, or a fake in the tests.
pub trait Transport: Send + Sync {
    /// `Err` only when nothing answered.
    fn send(&self, req: &Request) -> Result<Response, Refused>;
}

/// HTTP/1.0 over a TCP connection to the server's loopback listener. One
/// connection per request, read to the end: no chunked bodies, no keep
/// alive, nothing to get wrong.
pub struct Loopback {
    pub addr: String,
}

impl Transport for Loopback {
    fn send(&self, req: &Request) -> Result<Response, Refused> {
        use std::net::{TcpStream, ToSocketAddrs};
        let to = self
            .addr
            .to_socket_addrs()
            .ok()
            .and_then(|mut a| a.next())
            .ok_or_else(not_up)?;
        let mut stream =
            TcpStream::connect_timeout(&to, Duration::from_secs(3)).map_err(|_| not_up())?;
        let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(10)));
        stream
            .write_all(&request_bytes(&self.addr, req))
            .map_err(|_| not_up())?;
        let mut raw = Vec::new();
        (&mut stream)
            .take(MAX_ANSWER)
            .read_to_end(&mut raw)
            .map_err(|_| not_up())?;
        parse_response(&raw).ok_or_else(|| odd("not HTTP"))
    }
}

/// The bytes of `req`. The credentials go in a header, never a URL.
pub fn request_bytes(host: &str, req: &Request) -> Vec<u8> {
    let auth = base64(format!("{}:{}", req.user, req.secret).as_bytes());
    let mut out = format!(
        "{} {} HTTP/1.0\r\nHost: {host}\r\nAuthorization: Basic {auth}\r\nAccept: application/json\r\n",
        req.method, req.path
    )
    .into_bytes();
    if let Some(body) = &req.body {
        out.extend_from_slice(
            format!(
                "Content-Type: application/json\r\nContent-Length: {}\r\n",
                body.len()
            )
            .as_bytes(),
        );
    }
    out.extend_from_slice(b"\r\n");
    if let Some(body) = &req.body {
        out.extend_from_slice(body);
    }
    out
}

/// Status, content type and body of a whole HTTP/1.x answer.
pub fn parse_response(raw: &[u8]) -> Option<Response> {
    let at = raw.windows(4).position(|w| w == b"\r\n\r\n")?;
    let head = std::str::from_utf8(&raw[..at]).ok()?;
    let mut lines = head.split("\r\n");
    let status = lines.next()?.split(' ').nth(1)?.parse().ok()?;
    let content_type = lines
        .filter_map(|l| l.split_once(':'))
        .find(|(k, _)| k.trim().eq_ignore_ascii_case("content-type"))
        .map(|(_, v)| v.trim().to_string())
        .unwrap_or_default();
    Some(Response {
        status,
        content_type,
        body: raw[at + 4..].to_vec(),
    })
}

/// Standard base64 with padding, for one Basic header. The crate does not
/// otherwise need an encoder.
pub fn base64(input: &[u8]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(char::from(ABC[(n >> (18 - 6 * i)) as usize & 63]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

// ── What the Mail page sees ────────────────────────────────────────────────

/// One folder.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Mailbox {
    pub id: String,
    pub name: String,
    /// `inbox`, `drafts`, `sent`, `junk`, `trash`, `archive`, or `null`.
    pub role: Option<String>,
    pub total: u64,
    pub unread: u64,
}

/// A name and an address.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Address {
    pub name: Option<String>,
    pub email: String,
}

/// One line of a mailbox's list.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub id: String,
    pub thread_id: String,
    pub subject: String,
    pub from: Vec<Address>,
    pub to: Vec<Address>,
    pub received_at: String,
    pub preview: String,
    pub seen: bool,
    pub flagged: bool,
    pub has_attachment: bool,
}

/// A page of a mailbox.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Page {
    pub messages: Vec<Summary>,
    pub position: usize,
    pub total: Option<u64>,
}

/// A file attached to a message.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    pub blob_id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub size: u64,
}

/// One message, opened.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    #[serde(flatten)]
    pub summary: Summary,
    pub cc: Vec<Address>,
    pub reply_to: Vec<Address>,
    pub mailbox_ids: Vec<String>,
    /// The plain text body, joined.
    pub text: String,
    /// Whether there is an HTML body to show in the frame.
    pub has_html: bool,
    pub attachments: Vec<Attachment>,
}

/// Someone on this box the page can offer as a recipient.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Neighbour {
    pub name: String,
    pub display_name: String,
    pub address: String,
}

/// A message the person wrote.
#[derive(Debug, Clone, PartialEq)]
pub struct Draft {
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub subject: String,
    pub text: String,
    /// The id of the message this answers, for the threading headers.
    pub in_reply_to: Option<String>,
}

/// What a message may be changed to.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Change {
    pub seen: Option<bool>,
    pub flagged: Option<bool>,
    /// Move to this mailbox.
    pub mailbox: Option<String>,
}

// ── Reading requests ───────────────────────────────────────────────────────

/// Whether `addr` is an address mail can be sent to: one `@`, a local part,
/// a dotted domain, nothing that could break a header.
pub fn valid_address(addr: &str) -> bool {
    let Some((local, domain)) = addr.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && local.len() <= 64
        && domain.len() <= 253
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !domain.contains("..")
        && addr
            .chars()
            .all(|c| c.is_ascii_graphic() || (!c.is_control() && !c.is_whitespace()))
        && !addr.contains(['<', '>', ',', ';', '"', '(', ')', '[', ']', '\\'])
        && !domain.contains('@')
}

fn strings(doc: &Value, key: &str) -> Vec<String> {
    match doc.get(key) {
        Some(Value::String(s)) => s
            .split([',', ';'])
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect(),
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

/// `POST /api/mail/send`'s body: `to`, `cc` (lists or comma-separated
/// strings), `subject`, `text`, `inReplyTo`.
pub fn parse_draft(doc: &Value) -> Result<Draft, Refused> {
    let to = strings(doc, "to");
    let cc = strings(doc, "cc");
    if to.is_empty() {
        return refuse(400, "add someone to send this to");
    }
    if to.len() + cc.len() > MAX_RECIPIENTS {
        return refuse(
            400,
            format!("one message goes to at most {MAX_RECIPIENTS} people"),
        );
    }
    if let Some(bad) = to.iter().chain(&cc).find(|a| !valid_address(a)) {
        return refuse(400, format!("{bad} is not an email address"));
    }
    let subject = doc
        .get("subject")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    if subject.len() > MAX_SUBJECT || subject.contains(['\r', '\n']) {
        return refuse(400, "the subject is one line of at most 900 characters");
    }
    let text = doc
        .get("text")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if text.len() > MAX_BODY {
        return refuse(400, "the message is longer than 1 MiB");
    }
    let in_reply_to = doc
        .get("inReplyTo")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    if let Some(id) = &in_reply_to {
        check_id(id)?;
    }
    Ok(Draft {
        to,
        cc,
        subject,
        text,
        in_reply_to,
    })
}

/// Mail leaves the box only once an edge carries it (the edge's port 25
/// and its reputation, not a home connection's), so for now every
/// recipient has to be someone here.
pub fn local_only(domain: &str, draft: &Draft) -> Result<(), Refused> {
    let suffix = format!("@{domain}");
    match draft
        .to
        .iter()
        .chain(&draft.cc)
        .find(|a| !a.to_ascii_lowercase().ends_with(&suffix))
    {
        Some(a) => refuse(
            422,
            format!("{a} is not on this box; for now LosOS Mail delivers only to addresses ending in {suffix}"),
        ),
        None => Ok(()),
    }
}

/// `POST /api/mail/messages/{id}`'s body: `seen`, `flagged`, `mailbox`.
pub fn parse_change(doc: &Value) -> Result<Change, Refused> {
    let change = Change {
        seen: doc.get("seen").and_then(Value::as_bool),
        flagged: doc.get("flagged").and_then(Value::as_bool),
        mailbox: doc
            .get("mailbox")
            .and_then(Value::as_str)
            .map(str::to_string),
    };
    if let Some(m) = &change.mailbox {
        check_id(m)?;
    }
    if change == Change::default() {
        return refuse(400, "nothing to change");
    }
    Ok(change)
}

/// A JMAP id as Stalwart makes them: short, letters and digits. Anything
/// else is refused before it reaches a request.
pub fn check_id(id: &str) -> Result<&str, Refused> {
    if !id.is_empty()
        && id.len() <= 255
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        Ok(id)
    } else {
        refuse(400, "no such message")
    }
}

// ── JMAP: building calls and reading answers ───────────────────────────────

/// A JMAP request body.
pub fn envelope(calls: Vec<Value>) -> Value {
    json!({ "using": USING, "methodCalls": calls })
}

/// The answer to call number `tag` in `resp`, or the reason it failed.
fn answer<'a>(resp: &'a Value, name: &str, tag: &str) -> Result<&'a Value, Refused> {
    let calls = resp
        .get("methodResponses")
        .and_then(Value::as_array)
        .ok_or_else(|| odd("no methodResponses"))?;
    for call in calls {
        let (Some(n), Some(args), Some(t)) = (
            call.get(0).and_then(Value::as_str),
            call.get(1),
            call.get(2).and_then(Value::as_str),
        ) else {
            continue;
        };
        if t != tag {
            continue;
        }
        if n == "error" {
            let kind = args.get("type").and_then(Value::as_str).unwrap_or("");
            tracing::warn!(call = name, kind, ?args, "the mail server refused a call");
            return if kind == "notFound" || kind == "invalidArguments" {
                refuse(404, "no such message")
            } else {
                Err(odd(name))
            };
        }
        if n == name {
            return Ok(args);
        }
    }
    Err(odd(name))
}

/// The first reason a `/set` call gave for not doing what was asked.
fn set_failure(args: &Value) -> Option<String> {
    ["notCreated", "notUpdated", "notDestroyed"]
        .iter()
        .filter_map(|k| args.get(*k).and_then(Value::as_object))
        .flat_map(|m| m.values())
        .next()
        .map(|e| {
            e.get("description")
                .and_then(Value::as_str)
                .or_else(|| e.get("type").and_then(Value::as_str))
                .unwrap_or("refused")
                .to_string()
        })
}

const MAILBOX_PROPS: [&str; 6] = [
    "name",
    "role",
    "totalEmails",
    "unreadEmails",
    "parentId",
    "sortOrder",
];

const SUMMARY_PROPS: [&str; 10] = [
    "threadId",
    "subject",
    "from",
    "to",
    "receivedAt",
    "preview",
    "keywords",
    "hasAttachment",
    "mailboxIds",
    "size",
];

pub fn mailboxes_call(account: &str) -> Vec<Value> {
    vec![json!(["Mailbox/get", { "accountId": account, "properties": MAILBOX_PROPS }, "m"])]
}

/// Stalwart makes no Archive of its own: make one, then read the folders
/// again in the same request.
pub fn archive_calls(account: &str) -> Vec<Value> {
    vec![
        json!(["Mailbox/set", {
            "accountId": account,
            "create": { "a": { "name": "Archive", "role": "archive" } },
        }, "s"]),
        json!(["Mailbox/get", { "accountId": account, "properties": MAILBOX_PROPS }, "m"]),
    ]
}

/// Where a role sorts in the folder list: the inbox first, the bin last.
fn role_rank(role: Option<&str>) -> u8 {
    match role {
        Some("inbox") => 0,
        Some("drafts") => 1,
        Some("sent") => 2,
        Some("archive") => 3,
        None => 4,
        Some("junk") => 5,
        Some("trash") => 6,
        Some(_) => 4,
    }
}

pub fn parse_mailboxes(resp: &Value) -> Result<Vec<Mailbox>, Refused> {
    let list = answer(resp, "Mailbox/get", "m")?
        .get("list")
        .and_then(Value::as_array)
        .ok_or_else(|| odd("Mailbox/get list"))?;
    let mut boxes: Vec<Mailbox> = list
        .iter()
        .filter_map(|m| {
            Some(Mailbox {
                id: m.get("id")?.as_str()?.to_string(),
                name: m.get("name")?.as_str()?.to_string(),
                role: m.get("role").and_then(Value::as_str).map(str::to_string),
                total: m.get("totalEmails").and_then(Value::as_u64).unwrap_or(0),
                unread: m.get("unreadEmails").and_then(Value::as_u64).unwrap_or(0),
            })
        })
        .collect();
    boxes.sort_by(|a, b| {
        role_rank(a.role.as_deref())
            .cmp(&role_rank(b.role.as_deref()))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(boxes)
}

/// The id of the mailbox with `role`.
pub fn with_role<'a>(boxes: &'a [Mailbox], role: &str) -> Option<&'a str> {
    boxes
        .iter()
        .find(|m| m.role.as_deref() == Some(role))
        .map(|m| m.id.as_str())
}

pub fn list_calls(
    account: &str,
    mailbox: &str,
    query: Option<&str>,
    position: usize,
) -> Vec<Value> {
    let mut filter = json!({ "inMailbox": mailbox });
    if let Some(q) = query.map(str::trim).filter(|q| !q.is_empty()) {
        filter = json!({ "operator": "AND", "conditions": [filter, { "text": q }] });
    }
    vec![
        json!(["Email/query", {
            "accountId": account,
            "filter": filter,
            "sort": [{ "property": "receivedAt", "isAscending": false }],
            "position": position,
            "limit": PAGE,
            "calculateTotal": true,
        }, "q"]),
        json!(["Email/get", {
            "accountId": account,
            "#ids": { "resultOf": "q", "name": "Email/query", "path": "/ids" },
            "properties": SUMMARY_PROPS,
        }, "g"]),
    ]
}

fn addresses(v: Option<&Value>) -> Vec<Address> {
    v.and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|x| {
                    Some(Address {
                        name: x
                            .get("name")
                            .and_then(Value::as_str)
                            .filter(|s| !s.is_empty())
                            .map(str::to_string),
                        email: x.get("email")?.as_str()?.to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn keyword(m: &Value, k: &str) -> bool {
    m.pointer(&format!("/keywords/{}", k.replace('/', "~1")))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn summary(m: &Value) -> Option<Summary> {
    Some(Summary {
        id: m.get("id")?.as_str()?.to_string(),
        thread_id: m
            .get("threadId")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        subject: m
            .get("subject")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        from: addresses(m.get("from")),
        to: addresses(m.get("to")),
        received_at: m
            .get("receivedAt")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        preview: m
            .get("preview")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        seen: keyword(m, "$seen"),
        flagged: keyword(m, "$flagged"),
        has_attachment: m
            .get("hasAttachment")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

pub fn parse_list(resp: &Value, position: usize) -> Result<Page, Refused> {
    let query = answer(resp, "Email/query", "q")?;
    let order: Vec<&str> = query
        .get("ids")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let got: HashMap<String, Summary> = answer(resp, "Email/get", "g")?
        .get("list")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(summary)
                .map(|s| (s.id.clone(), s))
                .collect()
        })
        .unwrap_or_default();
    Ok(Page {
        // The query's order, which Email/get does not promise to keep.
        messages: order
            .iter()
            .filter_map(|id| got.get(*id).cloned())
            .collect(),
        position,
        total: query.get("total").and_then(Value::as_u64),
    })
}

/// Open message `id`: read it with its bodies, and mark it read.
pub fn message_calls(account: &str, id: &str) -> Vec<Value> {
    let mut props: Vec<&str> = SUMMARY_PROPS.to_vec();
    props.extend([
        "cc",
        "replyTo",
        "textBody",
        "htmlBody",
        "attachments",
        "bodyValues",
    ]);
    vec![
        json!(["Email/get", {
            "accountId": account,
            "ids": [id],
            "properties": props,
            "fetchTextBodyValues": true,
            "maxBodyValueBytes": MAX_BODY,
        }, "g"]),
        json!(["Email/set", {
            "accountId": account,
            "update": { id: { "keywords/$seen": true } },
        }, "s"]),
    ]
}

/// Read only the mailbox membership needed to decide how to delete a message.
pub fn delete_check_calls(account: &str, id: &str) -> Vec<Value> {
    vec![json!(["Email/get", {
        "accountId": account,
        "ids": [id],
        "properties": ["mailboxIds"],
    }, "g"])]
}

pub fn parse_delete_mailbox_ids(resp: &Value) -> Result<Vec<String>, Refused> {
    let m = answer(resp, "Email/get", "g")?
        .get("list")
        .and_then(Value::as_array)
        .and_then(|a| a.first())
        .ok_or_else(|| Refused {
            status: 404,
            message: "no such message".into(),
        })?;
    let ids = m
        .get("mailboxIds")
        .and_then(Value::as_object)
        .ok_or_else(|| odd("Email/get mailboxIds"))?;
    Ok(ids.keys().cloned().collect())
}

/// The text of the parts `key` lists, from `bodyValues`.
fn joined(m: &Value, key: &str) -> String {
    let values = m.get("bodyValues");
    m.get(key)
        .and_then(Value::as_array)
        .map(|parts| {
            parts
                .iter()
                .filter_map(|p| {
                    let id = p.get("partId")?.as_str()?;
                    values?.get(id)?.get("value")?.as_str()
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

pub fn parse_message(resp: &Value) -> Result<Message, Refused> {
    let m = answer(resp, "Email/get", "g")?
        .get("list")
        .and_then(Value::as_array)
        .and_then(|a| a.first())
        .ok_or_else(|| Refused {
            status: 404,
            message: "no such message".into(),
        })?;
    let mut summary = summary(m).ok_or_else(|| odd("Email/get item"))?;
    // It was opened: the second call marked it read.
    summary.seen = true;
    let has_html = m
        .get("htmlBody")
        .and_then(Value::as_array)
        .is_some_and(|parts| {
            parts
                .iter()
                .any(|p| p.get("type").and_then(Value::as_str) == Some("text/html"))
        });
    let attachments = m
        .get("attachments")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|p| {
                    Some(Attachment {
                        blob_id: p.get("blobId")?.as_str()?.to_string(),
                        name: p
                            .get("name")
                            .and_then(Value::as_str)
                            .filter(|s| !s.is_empty())
                            .unwrap_or("attachment")
                            .to_string(),
                        kind: p
                            .get("type")
                            .and_then(Value::as_str)
                            .unwrap_or("application/octet-stream")
                            .to_string(),
                        size: p.get("size").and_then(Value::as_u64).unwrap_or(0),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let mut mailbox_ids: Vec<String> = m
        .get("mailboxIds")
        .and_then(Value::as_object)
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default();
    mailbox_ids.sort();
    Ok(Message {
        summary,
        cc: addresses(m.get("cc")),
        reply_to: addresses(m.get("replyTo")),
        mailbox_ids,
        text: joined(m, "textBody"),
        has_html,
        attachments,
    })
}

pub fn html_calls(account: &str, id: &str) -> Vec<Value> {
    vec![json!(["Email/get", {
        "accountId": account,
        "ids": [id],
        "properties": ["htmlBody", "bodyValues"],
        "fetchHTMLBodyValues": true,
        "maxBodyValueBytes": MAX_BODY,
    }, "g"])]
}

/// The message's HTML, ready for the frame: links open in a new tab,
/// because a sandboxed frame cannot navigate itself or the page.
pub fn parse_html(resp: &Value) -> Result<String, Refused> {
    let m = answer(resp, "Email/get", "g")?
        .get("list")
        .and_then(Value::as_array)
        .and_then(|a| a.first())
        .ok_or_else(|| Refused {
            status: 404,
            message: "no such message".into(),
        })?;
    Ok(format!(
        "<!doctype html><meta charset=\"utf-8\"><base target=\"_blank\">{}",
        joined(m, "htmlBody")
    ))
}

/// What sending needs to know first: the person's identities, the
/// folders, and (for a reply) the message answered.
pub fn send_prepare_calls(account: &str, in_reply_to: Option<&str>) -> Vec<Value> {
    let mut calls = vec![
        json!(["Identity/get", { "accountId": account }, "i"]),
        mailboxes_call(account).remove(0),
    ];
    if let Some(id) = in_reply_to {
        calls.push(json!(["Email/get", {
            "accountId": account,
            "ids": [id],
            "properties": ["messageId", "references"],
        }, "r"]));
    }
    calls
}

/// What [`send_prepare_calls`] found.
#[derive(Debug, Clone, PartialEq)]
pub struct Prepared {
    pub identity: Option<String>,
    pub drafts: String,
    pub sent: String,
    /// `Message-ID`s of the answered message, and its references.
    pub message_id: Vec<String>,
    pub references: Vec<String>,
}

pub fn parse_prepared(resp: &Value, address: &str) -> Result<Prepared, Refused> {
    let identity = answer(resp, "Identity/get", "i")?
        .get("list")
        .and_then(Value::as_array)
        .and_then(|a| {
            a.iter().find(|i| {
                i.get("email")
                    .and_then(Value::as_str)
                    .is_some_and(|e| e.eq_ignore_ascii_case(address))
            })
        })
        .and_then(|i| i.get("id"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let boxes = parse_mailboxes(resp)?;
    let drafts = with_role(&boxes, "drafts").ok_or_else(|| odd("no Drafts"))?;
    let sent = with_role(&boxes, "sent").ok_or_else(|| odd("no Sent"))?;
    let list = |key: &str| -> Vec<String> {
        resp.get("methodResponses")
            .and_then(Value::as_array)
            .and_then(|calls| {
                calls
                    .iter()
                    .find(|c| c.get(2).and_then(Value::as_str) == Some("r"))
            })
            .and_then(|c| c.pointer("/1/list/0"))
            .and_then(|m| m.get(key))
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default()
    };
    Ok(Prepared {
        identity,
        drafts: drafts.to_string(),
        sent: sent.to_string(),
        message_id: list("messageId"),
        references: list("references"),
    })
}

/// Write the message into Drafts and submit it; on success the server
/// moves it to Sent. A missing identity is made in the same request.
pub fn send_calls(account: &str, from: &Address, prep: &Prepared, draft: &Draft) -> Vec<Value> {
    let mut calls = Vec::new();
    let identity = match &prep.identity {
        Some(id) => id.clone(),
        None => {
            calls.push(json!(["Identity/set", {
                "accountId": account,
                "create": { "me": { "name": from.name.clone().unwrap_or_default(), "email": from.email } },
            }, "i"]));
            "#me".to_string()
        }
    };
    let to: Vec<Value> = draft.to.iter().map(|e| json!({ "email": e })).collect();
    let cc: Vec<Value> = draft.cc.iter().map(|e| json!({ "email": e })).collect();
    let mut email = json!({
        "mailboxIds": { prep.drafts.clone(): true },
        "keywords": { "$draft": true, "$seen": true },
        "from": [{ "name": from.name, "email": from.email }],
        "to": to,
        "subject": draft.subject,
        "bodyValues": { "t": { "value": draft.text } },
        "textBody": [{ "partId": "t", "type": "text/plain" }],
    });
    if !cc.is_empty() {
        email["cc"] = Value::Array(cc);
    }
    if !prep.message_id.is_empty() {
        let mut references = prep.references.clone();
        references.extend(prep.message_id.iter().cloned());
        email["inReplyTo"] = json!(prep.message_id);
        email["references"] = json!(references);
    }
    calls.push(json!(["Email/set", { "accountId": account, "create": { "draft": email } }, "e"]));
    let mut moved = serde_json::Map::new();
    moved.insert(format!("mailboxIds/{}", prep.drafts), Value::Null);
    moved.insert(format!("mailboxIds/{}", prep.sent), Value::Bool(true));
    moved.insert("keywords/$draft".into(), Value::Null);
    calls.push(json!(["EmailSubmission/set", {
        "accountId": account,
        "create": { "send": { "identityId": identity, "emailId": "#draft" } },
        "onSuccessUpdateEmail": { "#send": moved },
    }, "s"]));
    calls
}

/// Whether the message went: every step's `/set` succeeded.
pub fn parse_sent(resp: &Value, made_identity: bool) -> Result<(), Refused> {
    let mut steps = vec![("Email/set", "e"), ("EmailSubmission/set", "s")];
    if made_identity {
        steps.insert(0, ("Identity/set", "i"));
    }
    for (name, tag) in steps {
        if let Some(why) = set_failure(answer(resp, name, tag)?) {
            tracing::warn!(step = name, why, "the mail server would not send a message");
            return refuse(422, format!("the message was not sent: {why}"));
        }
    }
    Ok(())
}

pub fn change_calls(account: &str, id: &str, change: &Change) -> Vec<Value> {
    let mut patch = serde_json::Map::new();
    if let Some(seen) = change.seen {
        patch.insert(
            "keywords/$seen".into(),
            if seen { Value::Bool(true) } else { Value::Null },
        );
    }
    if let Some(flagged) = change.flagged {
        patch.insert(
            "keywords/$flagged".into(),
            if flagged {
                Value::Bool(true)
            } else {
                Value::Null
            },
        );
    }
    if let Some(to) = &change.mailbox {
        // A move replaces every folder the message was in.
        patch.insert("mailboxIds".into(), json!({ to.clone(): true }));
    }
    vec![json!(["Email/set", { "accountId": account, "update": { id: patch } }, "s"])]
}

pub fn destroy_calls(account: &str, id: &str) -> Vec<Value> {
    vec![json!(["Email/set", { "accountId": account, "destroy": [id] }, "s"])]
}

pub fn parse_changed(resp: &Value) -> Result<(), Refused> {
    let args = answer(resp, "Email/set", "s")?;
    match set_failure(args) {
        Some(why) if why.contains("not found") || why == "notFound" => {
            refuse(404, "no such message")
        }
        Some(why) => refuse(422, why),
        None => Ok(()),
    }
}

// ── The service ────────────────────────────────────────────────────────────

/// Mail as the HTTP handlers use it.
pub struct Mail {
    config: Option<Config>,
    transport: Box<dyn Transport>,
    /// People whose account exists, and the JMAP account id of each.
    accounts: Mutex<BTreeMap<String, String>>,
    /// Whether the domain exists.
    domain_made: Mutex<bool>,
    /// Secrets read once. A new secret means a new pod, which means a
    /// restart of this daemon too (`modules/mail.nix`).
    secret: Mutex<Option<String>>,
}

impl Mail {
    pub fn new(config: Option<Config>, transport: Box<dyn Transport>) -> Mail {
        Mail {
            config,
            transport,
            accounts: Mutex::new(BTreeMap::new()),
            domain_made: Mutex::new(false),
            secret: Mutex::new(None),
        }
    }

    pub fn from_env() -> Mail {
        let config = Config::from_env();
        let addr = config.as_ref().map(|c| c.addr.clone()).unwrap_or_default();
        Mail::new(config, Box::new(Loopback { addr }))
    }

    /// The configuration, if mail is live on this box: on, and the box
    /// sharing its storage (`modules/mail.nix` sets the environment only
    /// then).
    pub fn open(&self) -> Option<&Config> {
        self.config.as_ref()
    }

    /// `name`'s address.
    pub fn address(&self, name: &str) -> Option<String> {
        self.config.as_ref().map(|c| format!("{name}@{}", c.domain))
    }

    fn secret(&self, config: &Config) -> Result<String, Refused> {
        let mut slot = self.secret.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(s) = slot.as_ref() {
            return Ok(s.clone());
        }
        let s = std::fs::read_to_string(&config.secret_file)
            .map(|s| s.trim().to_string())
            .ok()
            .filter(|s| !s.is_empty())
            .ok_or_else(not_up)?;
        *slot = Some(s.clone());
        Ok(s)
    }

    fn admin(
        &self,
        config: &Config,
        method: &'static str,
        path: &str,
        body: Value,
    ) -> Result<Response, Refused> {
        self.transport.send(&Request {
            method,
            path: path.to_string(),
            user: ADMIN_USER.to_string(),
            secret: self.secret(config)?,
            body: Some(serde_json::to_vec(&body).unwrap_or_default()),
        })
    }

    /// Make `doc` with the management API; one that already exists is fine.
    fn create(&self, config: &Config, doc: Value) -> Result<(), Refused> {
        let resp = self.admin(config, "POST", "/api/principal", doc)?;
        let body: Value = serde_json::from_slice(&resp.body).unwrap_or_default();
        if resp.status == 200
            && (body.get("data").is_some()
                || body.get("error").and_then(Value::as_str) == Some("fieldAlreadyExists"))
        {
            return Ok(());
        }
        tracing::warn!(
            status = resp.status,
            ?body,
            "the mail server would not make an account"
        );
        Err(odd("/api/principal"))
    }

    /// Make sure `person` has a mailbox, and return their JMAP account id.
    fn account(&self, config: &Config, person: &crate::people::Person) -> Result<String, Refused> {
        if let Some(id) = self
            .accounts
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&person.name)
        {
            return Ok(id.clone());
        }
        {
            let mut made = self.domain_made.lock().unwrap_or_else(|e| e.into_inner());
            if !*made {
                self.create(config, json!({ "type": "domain", "name": config.domain }))?;
                *made = true;
            }
        }
        self.create(
            config,
            json!({
                "type": "individual",
                "name": person.name,
                "description": person.display_name,
                "emails": [format!("{}@{}", person.name, config.domain)],
                "roles": ["user"],
            }),
        )?;
        let resp = self.transport.send(&Request {
            method: "GET",
            path: "/jmap/session".into(),
            user: format!("{}%{MASTER_USER}", person.name),
            secret: self.secret(config)?,
            body: None,
        })?;
        if resp.status != 200 {
            return Err(odd("/jmap/session"));
        }
        let session: Value = serde_json::from_slice(&resp.body).map_err(|_| odd("session JSON"))?;
        let id = session
            .pointer("/primaryAccounts/urn:ietf:params:jmap:mail")
            .and_then(Value::as_str)
            .ok_or_else(|| odd("primaryAccounts"))?
            .to_string();
        self.accounts
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(person.name.clone(), id.clone());
        Ok(id)
    }

    /// One JMAP request as `person`.
    fn jmap(
        &self,
        person: &crate::people::Person,
        calls: impl FnOnce(&str) -> Vec<Value>,
    ) -> Result<(String, Value), Refused> {
        let config = self.open().ok_or_else(closed)?;
        let account = self.account(config, person)?;
        let resp = self.transport.send(&Request {
            method: "POST",
            path: "/jmap/".into(),
            user: format!("{}%{MASTER_USER}", person.name),
            secret: self.secret(config)?,
            body: Some(serde_json::to_vec(&envelope(calls(&account))).unwrap_or_default()),
        })?;
        if resp.status != 200 {
            tracing::warn!(
                status = resp.status,
                "the mail server refused a JMAP request"
            );
            return Err(odd("JMAP status"));
        }
        let doc = serde_json::from_slice(&resp.body).map_err(|_| odd("JMAP JSON"))?;
        Ok((account, doc))
    }

    pub fn mailboxes(&self, person: &crate::people::Person) -> Result<Vec<Mailbox>, Refused> {
        let (_, resp) = self.jmap(person, mailboxes_call)?;
        let boxes = parse_mailboxes(&resp)?;
        if with_role(&boxes, "archive").is_some() {
            return Ok(boxes);
        }
        let (_, resp) = self.jmap(person, archive_calls)?;
        parse_mailboxes(&resp)
    }

    pub fn list(
        &self,
        person: &crate::people::Person,
        mailbox: &str,
        query: Option<&str>,
        position: usize,
    ) -> Result<Page, Refused> {
        check_id(mailbox)?;
        let (_, resp) = self.jmap(person, |a| list_calls(a, mailbox, query, position))?;
        parse_list(&resp, position)
    }

    pub fn message(&self, person: &crate::people::Person, id: &str) -> Result<Message, Refused> {
        check_id(id)?;
        let (_, resp) = self.jmap(person, |a| message_calls(a, id))?;
        parse_message(&resp)
    }

    pub fn html(&self, person: &crate::people::Person, id: &str) -> Result<String, Refused> {
        check_id(id)?;
        let (_, resp) = self.jmap(person, |a| html_calls(a, id))?;
        parse_html(&resp)
    }

    pub fn send(&self, person: &crate::people::Person, draft: &Draft) -> Result<(), Refused> {
        let config = self.open().ok_or_else(closed)?;
        local_only(&config.domain, draft)?;
        let address = self.address(&person.name).unwrap_or_default();
        let (account, resp) = self.jmap(person, |a| {
            send_prepare_calls(a, draft.in_reply_to.as_deref())
        })?;
        let prep = parse_prepared(&resp, &address)?;
        let from = Address {
            name: Some(person.display_name.clone()).filter(|n| !n.is_empty()),
            email: address,
        };
        let (_, resp) = self.jmap(person, |_| send_calls(&account, &from, &prep, draft))?;
        parse_sent(&resp, prep.identity.is_none())
    }

    pub fn change(
        &self,
        person: &crate::people::Person,
        id: &str,
        change: &Change,
    ) -> Result<(), Refused> {
        check_id(id)?;
        let (_, resp) = self.jmap(person, |a| change_calls(a, id, change))?;
        parse_changed(&resp)
    }

    /// Move to the bin, or, from the bin, delete for good.
    pub fn delete(&self, person: &crate::people::Person, id: &str) -> Result<(), Refused> {
        check_id(id)?;
        let (_, resp) = self.jmap(person, |a| delete_check_calls(a, id))?;
        let mailbox_ids = parse_delete_mailbox_ids(&resp)?;
        let boxes = self.mailboxes(person)?;
        let trash = with_role(&boxes, "trash").ok_or_else(|| odd("no Deleted Items"))?;
        let (_, resp) = if mailbox_ids.iter().any(|m| m == trash) {
            self.jmap(person, |a| destroy_calls(a, id))?
        } else {
            let change = Change {
                mailbox: Some(trash.to_string()),
                ..Change::default()
            };
            self.jmap(person, |a| change_calls(a, id, &change))?
        };
        parse_changed(&resp)
    }

    /// An attachment's bytes and type.
    pub fn download(
        &self,
        person: &crate::people::Person,
        blob: &str,
    ) -> Result<(String, Vec<u8>), Refused> {
        check_id(blob)?;
        let config = self.open().ok_or_else(closed)?;
        let account = self.account(config, person)?;
        let resp = self.transport.send(&Request {
            method: "GET",
            path: format!("/jmap/download/{account}/{blob}/file"),
            user: format!("{}%{MASTER_USER}", person.name),
            secret: self.secret(config)?,
            body: None,
        })?;
        match resp.status {
            200 => Ok((resp.content_type, resp.body)),
            404 => refuse(404, "no such attachment"),
            _ => Err(odd("download")),
        }
    }

    /// The people on this box, with their addresses, for the To field.
    pub fn neighbours(&self, people: &[crate::people::Listed]) -> Vec<Neighbour> {
        let Some(config) = &self.config else {
            return Vec::new();
        };
        let mut seen = HashSet::new();
        people
            .iter()
            .filter(|p| !p.disabled && seen.insert(p.name.clone()))
            .map(|p| Neighbour {
                name: p.name.clone(),
                display_name: p.display_name.clone(),
                address: format!("{}@{}", p.name, config.domain),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn person(name: &str) -> crate::people::Person {
        crate::people::Person {
            name: name.into(),
            display_name: name.to_uppercase(),
            groups: vec![],
        }
    }

    /// A server that answers from a script and records what it was asked.
    #[derive(Default)]
    struct Fake {
        seen: Arc<Mutex<Vec<Request>>>,
        answers: Mutex<Vec<Response>>,
    }

    fn ok(body: Value) -> Response {
        Response {
            status: 200,
            content_type: "application/json".into(),
            body: serde_json::to_vec(&body).unwrap(),
        }
    }

    impl Transport for Fake {
        fn send(&self, req: &Request) -> Result<Response, Refused> {
            self.seen.lock().unwrap().push(req.clone());
            let mut answers = self.answers.lock().unwrap();
            if answers.is_empty() {
                return Err(not_up());
            }
            Ok(answers.remove(0))
        }
    }

    fn config(dir: &std::path::Path) -> Config {
        let secret_file = dir.join("secret");
        std::fs::write(&secret_file, "s3cret\n").unwrap();
        Config {
            addr: "127.0.0.1:1".into(),
            secret_file,
            domain: "mattbox.home.arpa".into(),
        }
    }

    fn tempdir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "losos-mail-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn base64_is_the_standard_alphabet_with_padding() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"matus%losos:pw"), "bWF0dXMlbG9zb3M6cHc=");
    }

    #[test]
    fn the_secret_rides_in_a_header_and_the_body_is_counted() {
        let req = Request {
            method: "POST",
            path: "/jmap/".into(),
            user: "eva%losos".into(),
            secret: "pw".into(),
            body: Some(b"{}".to_vec()),
        };
        let text = String::from_utf8(request_bytes("127.0.0.1:8089", &req)).unwrap();
        assert!(text.starts_with("POST /jmap/ HTTP/1.0\r\n"));
        assert!(text.contains(&format!(
            "Authorization: Basic {}\r\n",
            base64(b"eva%losos:pw")
        )));
        assert!(text.contains("Content-Length: 2\r\n"));
        assert!(text.ends_with("\r\n\r\n{}"));
        assert!(!text.contains("pw\r\n"));
    }

    #[test]
    fn an_answer_is_split_into_status_type_and_bytes() {
        let raw = b"HTTP/1.1 200 OK\r\nContent-Type: image/png\r\ncontent-length: 3\r\n\r\n\x89PN";
        let resp = parse_response(raw).unwrap();
        assert_eq!(resp.status, 200);
        assert_eq!(resp.content_type, "image/png");
        assert_eq!(resp.body, b"\x89PN");
        assert!(parse_response(b"garbage").is_none());
    }

    #[test]
    fn addresses_are_checked_before_they_reach_a_header() {
        for good in [
            "eva@mattbox.home.arpa",
            "a.b+c@example.org",
            "ján@príklad.sk",
        ] {
            assert!(valid_address(good), "{good}");
        }
        for bad in [
            "eva",
            "@x.org",
            "eva@localhost",
            "eva@x..org",
            "eva@.org",
            "eva@x.org.",
            "a b@x.org",
            "eva@x.org\r\nBcc: all@x.org",
            "a\u{2028}@x.org",
            "eva@\u{2029}x.org",
            "a\u{00a0}@x.org",
            "<eva@x.org>",
            "eva@x.org,bob@y.org",
            "a@b@c.org",
        ] {
            assert!(!valid_address(bad), "{bad}");
        }
    }

    #[test]
    fn a_draft_needs_a_recipient_and_takes_lists_or_commas() {
        let d = parse_draft(&json!({
            "to": "eva@mattbox.home.arpa, bob@example.org",
            "cc": ["ann@example.org"],
            "subject": " Hello ",
            "text": "Hi",
        }))
        .unwrap();
        assert_eq!(d.to, ["eva@mattbox.home.arpa", "bob@example.org"]);
        assert_eq!(d.cc, ["ann@example.org"]);
        assert_eq!(d.subject, "Hello");
        assert_eq!(d.in_reply_to, None);

        let r = parse_draft(&json!({ "to": [], "text": "x" })).unwrap_err();
        assert_eq!(
            (r.status, r.message.as_str()),
            (400, "add someone to send this to")
        );
        let r = parse_draft(&json!({ "to": "nobody" })).unwrap_err();
        assert_eq!(r.message, "nobody is not an email address");
        let r =
            parse_draft(&json!({ "to": "a@b.org", "subject": "a\r\nBcc: x@y.org" })).unwrap_err();
        assert_eq!(r.status, 400);
        let many: Vec<String> = (0..51).map(|i| format!("p{i}@x.org")).collect();
        assert_eq!(parse_draft(&json!({ "to": many })).unwrap_err().status, 400);
        let r = parse_draft(&json!({ "to": "a@b.org", "inReplyTo": "../x" })).unwrap_err();
        assert_eq!(r.status, 400);
    }

    #[test]
    fn a_change_says_what_and_names_a_real_mailbox() {
        assert_eq!(
            parse_change(&json!({ "seen": false })).unwrap(),
            Change {
                seen: Some(false),
                ..Change::default()
            }
        );
        assert_eq!(parse_change(&json!({})).unwrap_err().status, 400);
        assert_eq!(
            parse_change(&json!({ "mailbox": "a/b" }))
                .unwrap_err()
                .status,
            400
        );
    }

    fn mailboxes_answer() -> Value {
        json!({ "methodResponses": [["Mailbox/get", { "list": [
            { "id": "c", "name": "Junk Mail", "role": "junk", "totalEmails": 0, "unreadEmails": 0 },
            { "id": "b", "name": "Deleted Items", "role": "trash", "totalEmails": 1, "unreadEmails": 0 },
            { "id": "z", "name": "Receipts", "role": null, "totalEmails": 2, "unreadEmails": 1 },
            { "id": "d", "name": "Drafts", "role": "drafts", "totalEmails": 0, "unreadEmails": 0 },
            { "id": "e", "name": "Sent Items", "role": "sent", "totalEmails": 3, "unreadEmails": 0 },
            { "id": "a", "name": "Inbox", "role": "inbox", "totalEmails": 5, "unreadEmails": 2 },
        ]}, "m"]]})
    }

    /// The same folders after the Archive was made, as Stalwart answers
    /// [Mailbox/set, Mailbox/get].
    fn archived_answer() -> Value {
        let mut answer = mailboxes_answer();
        let get = answer["methodResponses"][0].clone();
        answer["methodResponses"][0] =
            json!(["Mailbox/set", { "created": { "a": { "id": "f" } } }, "s"]);
        answer["methodResponses"].as_array_mut().unwrap().push(get);
        answer["methodResponses"][1][1]["list"]
            .as_array_mut()
            .unwrap()
            .push(json!({ "id": "f", "name": "Archive", "role": "archive", "totalEmails": 0, "unreadEmails": 0 }));
        answer
    }

    #[test]
    fn folders_come_inbox_first_and_bin_last() {
        let boxes = parse_mailboxes(&mailboxes_answer()).unwrap();
        let names: Vec<_> = boxes.iter().map(|b| b.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "Inbox",
                "Drafts",
                "Sent Items",
                "Receipts",
                "Junk Mail",
                "Deleted Items"
            ]
        );
        assert_eq!(boxes[0].unread, 2);
        assert_eq!(with_role(&boxes, "sent"), Some("e"));
    }

    #[test]
    fn a_page_keeps_the_query_order_and_reads_keywords() {
        let resp = json!({ "methodResponses": [
            ["Email/query", { "ids": ["m2", "m1"], "total": 2 }, "q"],
            ["Email/get", { "list": [
                { "id": "m1", "threadId": "t1", "subject": "Old", "from": [{ "name": null, "email": "a@x.org" }],
                  "to": [], "receivedAt": "2026-10-20T10:00:00Z", "preview": "p1", "keywords": { "$seen": true },
                  "hasAttachment": false },
                { "id": "m2", "threadId": "t2", "subject": "New", "from": [{ "name": "Eva", "email": "eva@x.org" }],
                  "to": [], "receivedAt": "2026-10-21T10:00:00Z", "preview": "p2", "keywords": { "$flagged": true },
                  "hasAttachment": true },
            ]}, "g"],
        ]});
        let page = parse_list(&resp, 0).unwrap();
        assert_eq!(page.total, Some(2));
        assert_eq!(page.messages[0].subject, "New");
        assert!(
            !page.messages[0].seen && page.messages[0].flagged && page.messages[0].has_attachment
        );
        assert_eq!(page.messages[0].from[0].name.as_deref(), Some("Eva"));
        assert!(page.messages[1].seen);
        assert_eq!(page.messages[1].from[0].name, None);
    }

    #[test]
    fn a_search_narrows_the_folder_rather_than_replacing_it() {
        let calls = list_calls("c", "a", Some(" invoice "), 50);
        assert_eq!(calls[0][1]["position"], 50);
        assert_eq!(
            calls[0][1]["filter"],
            json!({ "operator": "AND", "conditions": [{ "inMailbox": "a" }, { "text": "invoice" }] })
        );
        assert_eq!(
            list_calls("c", "a", Some("  "), 0)[0][1]["filter"],
            json!({ "inMailbox": "a" })
        );
        assert_eq!(calls[1][1]["#ids"]["resultOf"], "q");
    }

    #[test]
    fn opening_a_message_marks_it_read_and_reads_its_parts() {
        let calls = message_calls("c", "m1");
        assert_eq!(
            calls[1],
            json!(["Email/set", { "accountId": "c", "update": { "m1": { "keywords/$seen": true } } }, "s"])
        );
        let resp = json!({ "methodResponses": [
            ["Email/get", { "list": [{
                "id": "m1", "threadId": "t", "subject": "S", "from": [], "to": [], "cc": [{ "email": "c@x.org" }],
                "receivedAt": "2026-10-21T10:00:00Z", "preview": "", "keywords": {}, "hasAttachment": true,
                "mailboxIds": { "a": true },
                "textBody": [{ "partId": "1", "type": "text/plain" }, { "partId": "3", "type": "text/plain" }],
                "htmlBody": [{ "partId": "2", "type": "text/html" }],
                "attachments": [{ "blobId": "b1", "name": "plan.pdf", "type": "application/pdf", "size": 1200 },
                                 { "blobId": "b2", "name": null, "size": 4 }],
                "bodyValues": { "1": { "value": "line one" }, "3": { "value": "line two" } },
            }]}, "g"],
            ["Email/set", { "updated": { "m1": null } }, "s"],
        ]});
        let m = parse_message(&resp).unwrap();
        assert!(m.summary.seen);
        assert_eq!(m.text, "line one\nline two");
        assert!(m.has_html);
        assert_eq!(m.mailbox_ids, ["a"]);
        assert_eq!(m.cc[0].email, "c@x.org");
        assert_eq!(m.attachments[1].name, "attachment");
        assert_eq!(m.attachments[1].kind, "application/octet-stream");

        let missing =
            json!({ "methodResponses": [["Email/get", { "list": [], "notFound": ["m9"] }, "g"]] });
        assert_eq!(parse_message(&missing).unwrap_err().status, 404);
    }

    #[test]
    fn html_opens_links_in_a_new_tab() {
        let resp = json!({ "methodResponses": [["Email/get", { "list": [{
            "id": "m1", "htmlBody": [{ "partId": "2", "type": "text/html" }],
            "bodyValues": { "2": { "value": "<p><a href=\"https://x.org\">x</a></p>" } },
        }]}, "g"]]});
        let html = parse_html(&resp).unwrap();
        assert!(html.starts_with("<!doctype html><meta charset=\"utf-8\"><base target=\"_blank\">"));
        assert!(html.ends_with("<p><a href=\"https://x.org\">x</a></p>"));
    }

    #[test]
    fn a_reply_carries_the_threading_headers_and_a_first_send_makes_an_identity() {
        let resp = json!({ "methodResponses": [
            ["Identity/get", { "list": [] }, "i"],
            mailboxes_answer()["methodResponses"][0].clone(),
            ["Email/get", { "list": [{ "id": "m1", "messageId": ["abc@x.org"], "references": ["root@x.org"] }] }, "r"],
        ]});
        let prep = parse_prepared(&resp, "matus@mattbox.home.arpa").unwrap();
        assert_eq!(prep.identity, None);
        assert_eq!((prep.drafts.as_str(), prep.sent.as_str()), ("d", "e"));
        let from = Address {
            name: Some("Matus".into()),
            email: "matus@mattbox.home.arpa".into(),
        };
        let draft = Draft {
            to: vec!["eva@mattbox.home.arpa".into()],
            cc: vec![],
            subject: "Re: plan".into(),
            text: "Yes.".into(),
            in_reply_to: Some("m1".into()),
        };
        let calls = send_calls("c", &from, &prep, &draft);
        assert_eq!(calls[0][0], "Identity/set");
        let email = &calls[1][1]["create"]["draft"];
        assert_eq!(email["mailboxIds"], json!({ "d": true }));
        assert_eq!(email["inReplyTo"], json!(["abc@x.org"]));
        assert_eq!(email["references"], json!(["root@x.org", "abc@x.org"]));
        assert!(email.get("cc").is_none());
        let sub = &calls[2][1];
        assert_eq!(
            sub["create"]["send"],
            json!({ "identityId": "#me", "emailId": "#draft" })
        );
        assert_eq!(
            sub["onSuccessUpdateEmail"]["#send"],
            json!({ "mailboxIds/d": null, "mailboxIds/e": true, "keywords/$draft": null })
        );

        let known = json!({ "methodResponses": [
            ["Identity/get", { "list": [{ "id": "i7", "email": "MATUS@mattbox.home.arpa" }] }, "i"],
            mailboxes_answer()["methodResponses"][0].clone(),
        ]});
        let prep = parse_prepared(&known, "matus@mattbox.home.arpa").unwrap();
        assert_eq!(prep.identity.as_deref(), Some("i7"));
        let calls = send_calls(
            "c",
            &from,
            &prep,
            &Draft {
                in_reply_to: None,
                ..draft
            },
        );
        assert_eq!(calls[0][0], "Email/set");
        assert_eq!(calls[1][1]["create"]["send"]["identityId"], "i7");
    }

    #[test]
    fn a_refused_send_says_why() {
        let resp = json!({ "methodResponses": [
            ["Email/set", { "created": { "draft": { "id": "x" } } }, "e"],
            ["EmailSubmission/set", { "notCreated": { "send": { "type": "forbiddenFrom", "description": "You are not allowed to send from this address." } } }, "s"],
        ]});
        let r = parse_sent(&resp, false).unwrap_err();
        assert_eq!(r.status, 422);
        assert_eq!(
            r.message,
            "the message was not sent: You are not allowed to send from this address."
        );
        let ok = json!({ "methodResponses": [
            ["Email/set", { "created": { "draft": { "id": "x" } } }, "e"],
            ["EmailSubmission/set", { "created": { "send": { "id": "s" } } }, "s"],
        ]});
        assert!(parse_sent(&ok, false).is_ok());
    }

    #[test]
    fn a_call_the_server_refused_is_not_mistaken_for_an_answer() {
        let resp = json!({ "methodResponses": [["error", { "type": "serverFail" }, "m"]] });
        assert_eq!(parse_mailboxes(&resp).unwrap_err().status, 502);
        let resp = json!({ "methodResponses": [["error", { "type": "invalidArguments" }, "s"]] });
        assert_eq!(parse_changed(&resp).unwrap_err().status, 404);
    }

    #[test]
    fn mail_stays_on_the_box_until_an_edge_carries_it() {
        let draft = |to: &str| Draft {
            to: vec![to.into()],
            cc: vec![],
            subject: String::new(),
            text: String::new(),
            in_reply_to: None,
        };
        assert!(local_only("mattbox.home.arpa", &draft("Eva@MattBox.home.arpa")).is_ok());
        let r = local_only("mattbox.home.arpa", &draft("eva@gmail.com")).unwrap_err();
        assert_eq!(r.status, 422);
        assert!(r.message.starts_with("eva@gmail.com is not on this box"));
        assert!(local_only("mattbox.home.arpa", &draft("eva@evilmattbox.home.arpa")).is_err());
    }

    #[test]
    fn flags_are_set_and_cleared_and_a_move_replaces_the_folders() {
        let c = Change {
            seen: Some(false),
            flagged: Some(true),
            mailbox: Some("b".into()),
        };
        let calls = change_calls("c", "m1", &c);
        assert_eq!(
            calls[0][1]["update"]["m1"],
            json!({ "keywords/$seen": null, "keywords/$flagged": true, "mailboxIds": { "b": true } })
        );
    }

    #[test]
    fn deleting_checks_only_mailbox_membership_without_marking_read() {
        let calls = delete_check_calls("c", "m1");
        assert_eq!(
            calls,
            vec![json!(["Email/get", {
                "accountId": "c",
                "ids": ["m1"],
                "properties": ["mailboxIds"],
            }, "g"])]
        );
        assert_eq!(
            parse_delete_mailbox_ids(&json!({ "methodResponses": [
                ["Email/get", { "list": [{ "mailboxIds": { "inbox": true, "trash": true } }] }, "g"]
            ]}))
            .unwrap(),
            ["inbox", "trash"]
        );
        assert_eq!(
            parse_delete_mailbox_ids(&json!({ "methodResponses": [
                ["Email/get", { "list": [] }, "g"]
            ]}))
            .unwrap_err()
            .status,
            404
        );
    }

    #[test]
    fn without_mail_on_this_box_every_call_is_not_found() {
        let mail = Mail::new(None, Box::new(Fake::default()));
        assert!(mail.open().is_none());
        let r = mail.mailboxes(&person("eva")).unwrap_err();
        assert_eq!(
            (r.status, r.message.as_str()),
            (404, "there is no mail on this box")
        );
    }

    #[test]
    fn the_first_call_makes_the_domain_and_the_account_as_the_person() {
        let dir = tempdir();
        let fake = Fake::default();
        let seen = fake.seen.clone();
        *fake.answers.lock().unwrap() = vec![
            ok(json!({ "data": 1 })),
            ok(json!({ "error": "fieldAlreadyExists", "field": "name", "value": "eva" })),
            ok(json!({ "primaryAccounts": { "urn:ietf:params:jmap:mail": "d" } })),
            ok(mailboxes_answer()),
            ok(archived_answer()),
            ok(archived_answer()),
        ];
        let mail = Mail::new(Some(config(&dir)), Box::new(fake));
        let boxes = mail.mailboxes(&person("eva")).unwrap();
        // Stalwart had no Archive; there is one now, after Sent.
        assert_eq!(boxes.len(), 7);
        assert_eq!(boxes[3].role.as_deref(), Some("archive"));
        mail.mailboxes(&person("eva")).unwrap();

        let seen = seen.lock().unwrap();
        assert_eq!(
            seen.len(),
            6,
            "the account and the Archive are made once: {seen:#?}"
        );
        let make: Value = serde_json::from_slice(seen[4].body.as_ref().unwrap()).unwrap();
        assert_eq!(make["methodCalls"][0][0], "Mailbox/set");
        assert_eq!(make["methodCalls"][0][1]["create"]["a"]["role"], "archive");
        assert_eq!(
            (seen[0].method, seen[0].path.as_str(), seen[0].user.as_str()),
            ("POST", "/api/principal", ADMIN_USER)
        );
        let domain: Value = serde_json::from_slice(seen[0].body.as_ref().unwrap()).unwrap();
        assert_eq!(
            domain,
            json!({ "type": "domain", "name": "mattbox.home.arpa" })
        );
        let account: Value = serde_json::from_slice(seen[1].body.as_ref().unwrap()).unwrap();
        assert_eq!(account["emails"], json!(["eva@mattbox.home.arpa"]));
        assert_eq!(account["description"], "EVA");
        assert_eq!(seen[2].path, "/jmap/session");
        for req in &seen[2..] {
            assert_eq!(req.user, "eva%losos");
            assert_eq!(req.secret, "s3cret");
        }
        let jmap: Value = serde_json::from_slice(seen[3].body.as_ref().unwrap()).unwrap();
        assert_eq!(jmap["methodCalls"][0][1]["accountId"], "d");
    }

    #[test]
    fn a_server_that_does_not_answer_is_starting() {
        let dir = tempdir();
        let mail = Mail::new(Some(config(&dir)), Box::new(Fake::default()));
        let r = mail.mailboxes(&person("eva")).unwrap_err();
        assert_eq!(r.status, 503);
    }

    #[test]
    fn ids_from_the_page_are_checked_before_any_request() {
        let dir = tempdir();
        let fake = Fake::default();
        let seen = fake.seen.clone();
        let mail = Mail::new(Some(config(&dir)), Box::new(fake));
        assert_eq!(
            mail.message(&person("eva"), "../../api")
                .unwrap_err()
                .status,
            400
        );
        assert_eq!(
            mail.download(&person("eva"), "x y").unwrap_err().status,
            400
        );
        assert!(seen.lock().unwrap().is_empty());
    }

    #[test]
    fn neighbours_are_the_people_who_can_sign_in() {
        let dir = tempdir();
        let mail = Mail::new(Some(config(&dir)), Box::new(Fake::default()));
        let listed = |name: &str, disabled| crate::people::Listed {
            name: name.into(),
            display_name: name.into(),
            email: String::new(),
            groups: vec![],
            disabled,
        };
        let n = mail.neighbours(&[listed("eva", false), listed("bob", true)]);
        assert_eq!(n.len(), 1);
        assert_eq!(n[0].address, "eva@mattbox.home.arpa");
    }
}
