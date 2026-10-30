//! LosOS Mail: a mailbox for every person LosOS Login knows.
//!
//! The mail server is Stalwart 1.0 (`modules/mail.nix`), a service of its
//! own that runs as the `shared` user with its store in the shared data, so
//! it runs only while the box shares its storage; with sharing off there is
//! no mail here at all, page and API included. It listens on loopback only.
//! This daemon is the one client it has: the Mail page calls
//! `/api/mail/*`, and each call is made here, in JMAP, as the person
//! Authelia signed in.
//!
//! **Who a call is made as.** Stalwart's administrator, named in its
//! environment, may sign in as `<address>%<admin>` and act as the account
//! with that address (`crate::mail_setup::as_person`). Only this daemon and
//! the server read its secret (`$LOSOS_MAIL_SECRET_FILE`), so a person
//! reaches their own mailbox through LosOS Login and nothing else: no
//! second password, and taking someone out of the people file takes their
//! mail away at the next call, because `crate::http` no longer recognises
//! them.
//!
//! **Accounts.** A person's account is made on their first call, with the
//! address `<name>@<domain>`. The domain is part of the server's settings
//! (`crate::mail_setup`); the account is made in its registry over JMAP
//! with the same administrator. Making it twice is harmless, so nothing has
//! to remember it across restarts; a map in memory saves the call after the
//! first.
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

/// The administrator's name, `STALWART_RECOVERY_ADMIN` in
/// `modules/mail.nix`. It manages the server's registry and acts as each
/// person.
pub const MASTER_USER: &str = "losos";
/// The fallback administrator of a Stalwart 0.15 store, which the upgrade's
/// copy (`losos-mail migrate`) reads accounts with.
pub const ADMIN_USER: &str = "losos-admin";
/// The JMAP capabilities every call uses.
const USING: [&str; 3] = [
    "urn:ietf:params:jmap:core",
    "urn:ietf:params:jmap:mail",
    "urn:ietf:params:jmap:submission",
];
/// Files one message may carry.
pub const MAX_ATTACHMENTS: usize = 20;
/// Their bytes together. The send route's body limit follows from it.
pub const MAX_ATTACHMENT_BYTES: usize = 20 * 1024 * 1024;
/// The largest body `POST /api/mail/send` takes: the files in base64, the
/// text and room for the rest.
pub const MAX_SEND_BYTES: usize = MAX_ATTACHMENT_BYTES / 3 * 4 + MAX_BODY + 64 * 1024;
/// Messages per page of a mailbox.
pub const PAGE: usize = 50;
/// Recipients one message may have.
const MAX_RECIPIENTS: usize = 50;
/// The longest subject: the line limit of RFC 5322, less the header name.
const MAX_SUBJECT: usize = 900;
/// The largest body a person may write.
const MAX_BODY: usize = 1024 * 1024;
/// The largest OpenPGP body: an encrypted 1 MiB message, armoured, with
/// room to spare.
const MAX_ARMORED: usize = 3 * 1024 * 1024;
/// The largest request body the mail routes take.
pub const MAX_REQUEST: usize = MAX_ARMORED + 64 * 1024;
/// The largest answer read from the server: a message with its bodies, or
/// an attachment.
const MAX_ANSWER: u64 = 64 * 1024 * 1024;

/// What the daemon needs to reach the mail server, from the environment
/// `modules/daemon.nix` sets.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// `127.0.0.1:<port>`, Stalwart's HTTP listener.
    pub addr: String,
    /// The administrator's secret, shared with the server.
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

/// The server is not answering: the server is starting, or sharing went off.
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
    /// The body's type; `None` is JSON, which every call but an upload is.
    pub content_type: Option<String>,
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
                "Content-Type: {}\r\nContent-Length: {}\r\n",
                req.content_type.as_deref().unwrap_or("application/json"),
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
    /// The folder it sits in, or `null` at the top. A person's own
    /// folders nest (backend/plugins/mail); LosOS Mail's stay at the top.
    pub parent_id: Option<String>,
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
    /// `encrypted` or `signed` for OpenPGP mail (PGP/MIME, or an armoured
    /// block in the text), `null` for anything else.
    pub pgp: Option<&'static str>,
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
    /// The raw `Autocrypt:` header, which `crate::mail_pgp` learns a key
    /// from; not for the page.
    #[serde(skip)]
    pub autocrypt: Option<String>,
    /// When the sender says they sent it, RFC 3339; Autocrypt's clock.
    #[serde(skip)]
    pub sent_at: Option<String>,
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
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Draft {
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub subject: String,
    pub text: String,
    /// The id of the message this answers, for the threading headers.
    pub in_reply_to: Option<String>,
    /// An OpenPGP/MIME body the browser made, in place of `text`.
    pub pgp: Option<PgpBody>,
    /// Files to attach, as the page sent them.
    pub files: Vec<Upload>,
    /// The same files once the server holds them, which is what the
    /// message points at.
    pub attached: Vec<Attachment>,
}

/// A file the person attached: from their computer, or from LosOS Cloud,
/// which the page read with their own session before sending.
#[derive(Clone, PartialEq, Default)]
pub struct Upload {
    pub name: String,
    pub kind: String,
    pub data: Vec<u8>,
}

impl std::fmt::Debug for Upload {
    // The bytes are the person's; a log line names the file and its size.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Upload")
            .field("name", &self.name)
            .field("kind", &self.kind)
            .field("bytes", &self.data.len())
            .finish()
    }
}

/// What the browser encrypted or signed (`admin-ui/app/src/mail/pgp.ts`).
/// The box puts it inside the RFC 3156 structure and adds the headers; it
/// never sees the plain text of an encrypted message, and it cannot change
/// a signed part without the signature failing.
#[derive(Debug, Clone, PartialEq)]
pub enum PgpBody {
    /// `multipart/encrypted`: the armoured OpenPGP message.
    Encrypted { armored: String },
    /// `multipart/signed`: the MIME part as it was signed, CRLF line ends,
    /// the armoured detached signature over it, and the hash it used as
    /// `micalg` names it (`pgp-sha256`).
    Signed {
        part: String,
        signature: String,
        micalg: &'static str,
    },
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
    let pgp = match doc.get("pgp") {
        None | Some(Value::Null) => None,
        Some(p) => Some(parse_pgp(p)?),
    };
    let files = parse_files(doc)?;
    if pgp.is_some() && !files.is_empty() {
        return refuse(
            400,
            "an OpenPGP message carries its files inside the encrypted or signed part",
        );
    }
    Ok(Draft {
        to,
        cc,
        subject,
        text,
        in_reply_to,
        pgp,
        files,
        attached: Vec::new(),
    })
}

/// A file name that cannot break a header or name a path: no control
/// characters, quotes, slashes or backslashes, at most 200 characters.
pub fn clean_file_name(name: &str) -> String {
    let name: String = name
        .chars()
        .filter(|c| !c.is_control() && !matches!(c, '"' | '\\' | '/'))
        .take(200)
        .collect();
    let name = name.trim();
    if name.is_empty() || name == "." || name == ".." {
        "attachment".to_string()
    } else {
        name.to_string()
    }
}

/// A media type as a header may carry it, else `application/octet-stream`.
pub fn clean_media_type(kind: &str) -> String {
    let token = |s: &str| {
        !s.is_empty()
            && s.len() <= 100
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"!#$&-^_.+".contains(&b))
    };
    let kind = kind
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    match kind.split_once('/') {
        Some((a, b)) if token(a) && token(b) => kind,
        _ => "application/octet-stream".to_string(),
    }
}

/// `attachments`: `[{"name", "type", "data": base64}]`.
fn parse_files(doc: &Value) -> Result<Vec<Upload>, Refused> {
    use base64::Engine;
    let Some(list) = doc.get("attachments") else {
        return Ok(Vec::new());
    };
    let Some(list) = list.as_array() else {
        return refuse(400, "attachments is a list");
    };
    if list.len() > MAX_ATTACHMENTS {
        return refuse(
            400,
            format!("one message carries at most {MAX_ATTACHMENTS} files"),
        );
    }
    let mut total = 0usize;
    let mut files = Vec::with_capacity(list.len());
    for item in list {
        let data = item
            .get("data")
            .and_then(Value::as_str)
            .and_then(|d| base64::engine::general_purpose::STANDARD.decode(d).ok());
        let Some(data) = data else {
            return refuse(400, "an attachment's data is not base64");
        };
        total += data.len();
        if total > MAX_ATTACHMENT_BYTES {
            return refuse(413, "the attachments come to more than 20 MiB");
        }
        files.push(Upload {
            name: clean_file_name(item.get("name").and_then(Value::as_str).unwrap_or("")),
            kind: clean_media_type(item.get("type").and_then(Value::as_str).unwrap_or("")),
            data,
        });
    }
    Ok(files)
}

/// What the server answered an upload with: the blob the message points at.
pub fn parse_uploaded(resp: &Response, file: &Upload) -> Result<Attachment, Refused> {
    if !(200..300).contains(&resp.status) {
        tracing::warn!(status = resp.status, "the mail server refused an upload");
        return Err(odd("upload"));
    }
    let doc: Value = serde_json::from_slice(&resp.body).map_err(|_| odd("upload JSON"))?;
    let blob_id = doc
        .get("blobId")
        .and_then(Value::as_str)
        .ok_or_else(|| odd("upload blobId"))?;
    check_id(blob_id)?;
    Ok(Attachment {
        blob_id: blob_id.to_string(),
        name: file.name.clone(),
        kind: file.kind.clone(),
        size: doc
            .get("size")
            .and_then(Value::as_u64)
            .unwrap_or(file.data.len() as u64),
    })
}

/// Lines of `text` end in CRLF, and no line is longer than RFC 5322's 998.
fn crlf_only(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.iter().enumerate().all(|(i, &b)| match b {
        b'\n' => i > 0 && bytes[i - 1] == b'\r',
        b'\r' => bytes.get(i + 1) == Some(&b'\n'),
        _ => true,
    }) && text.split("\r\n").all(|l| l.len() <= 998)
}

/// `pgp`: `{"mime": "encrypted", "armored"}` or `{"mime": "signed",
/// "part", "signature"}`. Armour and a signed part are 7-bit text; nothing
/// else is taken, so nothing here can add a header or a part.
pub fn parse_pgp(doc: &Value) -> Result<PgpBody, Refused> {
    let field = |k: &str| doc.get(k).and_then(Value::as_str).unwrap_or("");
    let armor = |text: &str, label: &str| -> Result<String, Refused> {
        let text = text.trim().replace("\r\n", "\n").replace('\n', "\r\n");
        let begin = format!("-----BEGIN PGP {label}-----");
        let end = format!("-----END PGP {label}-----");
        if text.len() > MAX_ARMORED
            || !text.starts_with(&begin)
            || !text.ends_with(&end)
            || !text.is_ascii()
            || !crlf_only(&text)
        {
            return refuse(400, "the encrypted or signed message is not OpenPGP armour");
        }
        Ok(text)
    };
    match field("mime") {
        "encrypted" => Ok(PgpBody::Encrypted {
            armored: armor(field("armored"), "MESSAGE")?,
        }),
        "signed" => {
            let part = field("part");
            if part.is_empty()
                || part.len() > MAX_ARMORED
                || !part.is_ascii()
                || !crlf_only(part)
                || !part.to_ascii_lowercase().starts_with("content-type:")
                || !part.contains("\r\n\r\n")
            {
                return refuse(400, "the signed part is not a MIME part in 7-bit text");
            }
            let micalg = match field("micalg") {
                "" | "pgp-sha256" => "pgp-sha256",
                "pgp-sha384" => "pgp-sha384",
                "pgp-sha512" => "pgp-sha512",
                "pgp-sha224" => "pgp-sha224",
                _ => return refuse(400, "pgp.micalg names a SHA-2 hash"),
            };
            Ok(PgpBody::Signed {
                part: part.to_string(),
                signature: armor(field("signature"), "SIGNATURE")?,
                micalg,
            })
        }
        _ => refuse(400, "pgp.mime is encrypted or signed"),
    }
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

const SUMMARY_PROPS: [&str; 11] = [
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
    "header:Content-Type:asRaw",
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

/// The folder mail a moderator holds goes to (`crate::moderation`). It has
/// no JMAP role, so it is found by name, and the page never lists it.
pub const QUARANTINE: &str = "Quarantine";

/// Make the Quarantine folder, then read the folders again.
pub fn quarantine_calls(account: &str) -> Vec<Value> {
    vec![
        json!(["Mailbox/set", {
            "accountId": account,
            "create": { "q": { "name": QUARANTINE } },
        }, "s"]),
        json!(["Mailbox/get", { "accountId": account, "properties": MAILBOX_PROPS }, "m"]),
    ]
}

/// The id of the Quarantine folder.
pub fn quarantine_of(boxes: &[Mailbox]) -> Option<&str> {
    boxes
        .iter()
        .find(|m| m.role.is_none() && m.name == QUARANTINE)
        .map(|m| m.id.as_str())
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
                parent_id: m
                    .get("parentId")
                    .and_then(Value::as_str)
                    .map(str::to_string),
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
        pgp: pgp_kind(
            raw_header(m, "Content-Type").unwrap_or(""),
            m.get("preview").and_then(Value::as_str).unwrap_or(""),
        ),
    })
}

/// A header asked for as `header:<name>:asRaw`. Stalwart answers it under
/// `header:<name>`, without the form, so both keys are read.
fn raw_header<'a>(m: &'a Value, name: &str) -> Option<&'a str> {
    m.get(format!("header:{name}:asRaw"))
        .or_else(|| m.get(format!("header:{name}")))
        .and_then(Value::as_str)
}

/// Whether a message is OpenPGP mail, from its top-level `Content-Type` and
/// the start of its text: PGP/MIME (RFC 3156), or an armoured block.
pub fn pgp_kind(content_type: &str, text: &str) -> Option<&'static str> {
    let ct = content_type.to_ascii_lowercase();
    if ct.contains("multipart/encrypted") && ct.contains("application/pgp-encrypted") {
        return Some("encrypted");
    }
    if ct.contains("multipart/signed") && ct.contains("application/pgp-signature") {
        return Some("signed");
    }
    if text.contains("-----BEGIN PGP MESSAGE-----") {
        return Some("encrypted");
    }
    if text.contains("-----BEGIN PGP SIGNED MESSAGE-----") {
        return Some("signed");
    }
    None
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
/// The calls that open message `id`: its parts, and with `mark_read`, the
/// `$seen` keyword. The LosOS companion opens without marking, so a message
/// an AI tool read still waits for its person.
pub fn message_calls(account: &str, id: &str, mark_read: bool) -> Vec<Value> {
    let mut props: Vec<&str> = SUMMARY_PROPS.to_vec();
    props.extend([
        "cc",
        "replyTo",
        "textBody",
        "htmlBody",
        "attachments",
        "bodyValues",
        "sentAt",
        "header:Autocrypt:asRaw",
    ]);
    let mut calls = vec![json!(["Email/get", {
        "accountId": account,
        "ids": [id],
        "properties": props,
        "fetchTextBodyValues": true,
        "maxBodyValueBytes": MAX_BODY,
    }, "g"])];
    if mark_read {
        calls.push(json!(["Email/set", {
            "accountId": account,
            "update": { id: { "keywords/$seen": true } },
        }, "s"]));
    }
    calls
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

pub fn parse_message(resp: &Value, marked_read: bool) -> Result<Message, Refused> {
    let m = answer(resp, "Email/get", "g")?
        .get("list")
        .and_then(Value::as_array)
        .and_then(|a| a.first())
        .ok_or_else(|| Refused {
            status: 404,
            message: "no such message".into(),
        })?;
    let mut summary = summary(m).ok_or_else(|| odd("Email/get item"))?;
    // Opened by its person: the second call marked it read.
    summary.seen |= marked_read;
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
    let text = joined(m, "textBody");
    if summary.pgp.is_none() {
        summary.pgp = pgp_kind("", &text);
    }
    Ok(Message {
        summary,
        cc: addresses(m.get("cc")),
        reply_to: addresses(m.get("replyTo")),
        mailbox_ids,
        text,
        has_html,
        attachments,
        autocrypt: raw_header(m, "Autocrypt").map(str::to_string),
        sent_at: m.get("sentAt").and_then(Value::as_str).map(str::to_string),
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
    if !draft.attached.is_empty() {
        email["attachments"] = draft
            .attached
            .iter()
            .map(|a| json!({ "blobId": a.blob_id, "type": a.kind, "name": a.name, "disposition": "attachment" }))
            .collect();
    }
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

/// Add a header to the email [`send_calls`] creates.
fn with_header(calls: &mut [Value], name: &str, value: &str) {
    for call in calls.iter_mut() {
        if call.get(0).and_then(Value::as_str) == Some("Email/set") {
            if let Some(email) = call.pointer_mut("/1/create/draft") {
                email[name] = Value::String(value.to_string());
            }
        }
    }
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

// ── OpenPGP/MIME: the whole message, built here ────────────────────────────

/// A header value as RFC 2047 encoded words when it is not plain ASCII.
pub fn encoded(text: &str) -> String {
    if text.chars().all(|c| c.is_ascii() && !c.is_ascii_control()) {
        return text.to_string();
    }
    // At most 45 bytes per word, whole characters only: 60 characters of
    // base64 plus the markers stays under the 75 a word may have.
    let mut words = Vec::new();
    let mut chunk = String::new();
    for c in text.chars().filter(|c| !c.is_control()) {
        if chunk.len() + c.len_utf8() > 45 {
            words.push(std::mem::take(&mut chunk));
        }
        chunk.push(c);
    }
    if !chunk.is_empty() {
        words.push(chunk);
    }
    words
        .iter()
        .map(|w| format!("=?UTF-8?B?{}?=", base64(w.as_bytes())))
        .collect::<Vec<_>>()
        .join("\r\n ")
}

/// `Name <address>`, the name quoted or encoded as it needs.
pub fn mailbox(name: Option<&str>, email: &str) -> String {
    match name.map(str::trim).filter(|n| !n.is_empty()) {
        None => email.to_string(),
        Some(n) if n.chars().all(|c| c.is_ascii_alphanumeric() || c == ' ') => {
            format!("{n} <{email}>")
        }
        Some(n) if n.is_ascii() => format!(
            "\"{}\" <{email}>",
            n.replace(['\\', '"'], "")
                .replace(|c: char| c.is_control(), "")
        ),
        Some(n) => format!("{} <{email}>", encoded(n)),
    }
}

/// The headers and the message ids [`raw_message`] needs besides the body.
#[derive(Debug, Clone)]
pub struct Envelope<'a> {
    pub from: &'a Address,
    pub to: &'a [String],
    pub cc: &'a [String],
    pub subject: &'a str,
    /// RFC 5322 date, e.g. `Thu, 22 Oct 2026 10:00:00 +0000`.
    pub date: &'a str,
    /// `<...@domain>`.
    pub message_id: &'a str,
    pub in_reply_to: &'a [String],
    pub references: &'a [String],
    /// The `Autocrypt:` header's folded value.
    pub autocrypt: Option<&'a str>,
}

/// The whole message for an OpenPGP/MIME body, RFC 3156: headers, then a
/// `multipart/encrypted` with the version part and the armour, or a
/// `multipart/signed` with the part as it was signed and the signature.
/// `boundary` must not occur in the body.
pub fn raw_message(env: &Envelope<'_>, body: &PgpBody, boundary: &str) -> Vec<u8> {
    let ids = |list: &[String]| {
        list.iter()
            .map(|id| format!("<{}>", id.trim_matches(['<', '>'])))
            .collect::<Vec<_>>()
            .join(" ")
    };
    let mut out = String::new();
    out.push_str(&format!(
        "From: {}\r\n",
        mailbox(env.from.name.as_deref(), &env.from.email)
    ));
    out.push_str(&format!("To: {}\r\n", env.to.join(", ")));
    if !env.cc.is_empty() {
        out.push_str(&format!("Cc: {}\r\n", env.cc.join(", ")));
    }
    out.push_str(&format!("Subject: {}\r\n", encoded(env.subject)));
    out.push_str(&format!("Date: {}\r\n", env.date));
    out.push_str(&format!("Message-ID: {}\r\n", env.message_id));
    if !env.in_reply_to.is_empty() {
        out.push_str(&format!("In-Reply-To: {}\r\n", ids(env.in_reply_to)));
        out.push_str(&format!("References: {}\r\n", ids(env.references)));
    }
    if let Some(a) = env.autocrypt {
        out.push_str(&format!("Autocrypt:{a}\r\n"));
    }
    out.push_str("MIME-Version: 1.0\r\n");
    match body {
        PgpBody::Encrypted { armored } => {
            out.push_str(&format!(
                "Content-Type: multipart/encrypted;\r\n protocol=\"application/pgp-encrypted\";\r\n boundary=\"{boundary}\"\r\n\r\n"
            ));
            out.push_str("This is an OpenPGP/MIME encrypted message (RFC 4880 and 3156)\r\n");
            out.push_str(&format!("--{boundary}\r\n"));
            out.push_str("Content-Type: application/pgp-encrypted\r\n");
            out.push_str("Content-Description: PGP/MIME version identification\r\n\r\n");
            out.push_str("Version: 1\r\n\r\n");
            out.push_str(&format!("--{boundary}\r\n"));
            out.push_str("Content-Type: application/octet-stream; name=\"encrypted.asc\"\r\n");
            out.push_str("Content-Description: OpenPGP encrypted message\r\n");
            out.push_str("Content-Disposition: inline; filename=\"encrypted.asc\"\r\n\r\n");
            out.push_str(armored);
            out.push_str(&format!("\r\n\r\n--{boundary}--\r\n"));
        }
        PgpBody::Signed {
            part,
            signature,
            micalg,
        } => {
            out.push_str(&format!(
                "Content-Type: multipart/signed; micalg={micalg};\r\n protocol=\"application/pgp-signature\";\r\n boundary=\"{boundary}\"\r\n\r\n"
            ));
            out.push_str("This is an OpenPGP/MIME signed message (RFC 4880 and 3156)\r\n");
            out.push_str(&format!("--{boundary}\r\n"));
            // Byte for byte: the CRLF before the next delimiter belongs to
            // the delimiter, not to the part (RFC 2046 5.1.1).
            out.push_str(part);
            out.push_str(&format!("\r\n--{boundary}\r\n"));
            out.push_str("Content-Type: application/pgp-signature; name=\"signature.asc\"\r\n");
            out.push_str("Content-Description: OpenPGP digital signature\r\n");
            out.push_str("Content-Disposition: attachment; filename=\"signature.asc\"\r\n\r\n");
            out.push_str(signature);
            out.push_str(&format!("\r\n\r\n--{boundary}--\r\n"));
        }
    }
    out.into_bytes()
}

/// Import an uploaded message into Drafts, as sending a written one does.
pub fn import_calls(account: &str, blob: &str, drafts: &str) -> Vec<Value> {
    vec![json!(["Email/import", {
        "accountId": account,
        "emails": { "m": {
            "blobId": blob,
            "mailboxIds": { drafts: true },
            "keywords": { "$draft": true, "$seen": true },
        } },
    }, "e"])]
}

pub fn parse_imported(resp: &Value) -> Result<String, Refused> {
    let args = answer(resp, "Email/import", "e")?;
    if let Some(why) = args.pointer("/notCreated/m").map(|e| {
        e.get("description")
            .or_else(|| e.get("type"))
            .and_then(Value::as_str)
            .unwrap_or("refused")
            .to_string()
    }) {
        tracing::warn!(why, "the mail server would not take a message");
        return refuse(422, format!("the message was not sent: {why}"));
    }
    args.pointer("/created/m/id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| odd("Email/import created"))
}

/// Submit an email already in Drafts, and move it to Sent once it went.
pub fn submit_calls(account: &str, identity: &str, email: &str, prep: &Prepared) -> Vec<Value> {
    let mut moved = serde_json::Map::new();
    moved.insert(format!("mailboxIds/{}", prep.drafts), Value::Null);
    moved.insert(format!("mailboxIds/{}", prep.sent), Value::Bool(true));
    moved.insert("keywords/$draft".into(), Value::Null);
    vec![json!(["EmailSubmission/set", {
        "accountId": account,
        "create": { "send": { "identityId": identity, "emailId": email } },
        "onSuccessUpdateEmail": { "#send": moved },
    }, "s"])]
}

/// Make the identity sending needs, on its own.
pub fn identity_calls(account: &str, from: &Address) -> Vec<Value> {
    vec![json!(["Identity/set", {
        "accountId": account,
        "create": { "me": { "name": from.name.clone().unwrap_or_default(), "email": from.email } },
    }, "i"])]
}

pub fn parse_identity(resp: &Value) -> Result<String, Refused> {
    let args = answer(resp, "Identity/set", "i")?;
    if let Some(why) = set_failure(args) {
        return refuse(422, format!("the message was not sent: {why}"));
    }
    args.pointer("/created/me/id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| odd("Identity/set created"))
}

pub fn parse_submitted(resp: &Value) -> Result<(), Refused> {
    if let Some(why) = set_failure(answer(resp, "EmailSubmission/set", "s")?) {
        tracing::warn!(why, "the mail server would not send a message");
        return refuse(422, format!("the message was not sent: {why}"));
    }
    Ok(())
}

/// The blob of the whole message, for [`Mail::raw`].
pub fn blob_calls(account: &str, id: &str) -> Vec<Value> {
    vec![json!(["Email/get", {
        "accountId": account,
        "ids": [id],
        "properties": ["blobId"],
    }, "g"])]
}

pub fn parse_blob(resp: &Value) -> Result<String, Refused> {
    answer(resp, "Email/get", "g")?
        .pointer("/list/0/blobId")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| Refused {
            status: 404,
            message: "no such message".into(),
        })
}

/// Sixteen random bytes as hex, for a Message-ID and a boundary.
fn random_hex() -> String {
    use ring::rand::SecureRandom;
    let mut bytes = [0u8; 16];
    let _ = ring::rand::SystemRandom::new().fill(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
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

/// The login that acts as `person`: their whole address, impersonated with
/// the administrator's secret.
fn login(config: &Config, person: &crate::people::Person) -> String {
    crate::mail_setup::as_person(&format!("{}@{}", person.name, config.domain))
}

/// Mail as the HTTP handlers use it.
pub struct Mail {
    config: Option<Config>,
    transport: Box<dyn Transport>,
    /// People whose account exists, and the JMAP account id of each.
    accounts: Mutex<BTreeMap<String, String>>,
    /// The mail domain's id in the server's registry.
    domain_id: Mutex<Option<String>>,
    /// Secrets read once. A new secret means a new server, which means a
    /// restart of this daemon too (`modules/mail.nix`).
    secret: Mutex<Option<String>>,
}

impl Mail {
    pub fn new(config: Option<Config>, transport: Box<dyn Transport>) -> Mail {
        Mail {
            config,
            transport,
            accounts: Mutex::new(BTreeMap::new()),
            domain_id: Mutex::new(None),
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

    /// One registry request as the server's administrator.
    fn admin(&self, config: &Config, calls: Vec<Value>) -> Result<Value, Refused> {
        let resp = self.transport.send(&Request {
            method: "POST",
            path: "/jmap/".into(),
            user: MASTER_USER.to_string(),
            secret: self.secret(config)?,
            body: Some(serde_json::to_vec(&crate::mail_setup::envelope(calls)).unwrap_or_default()),
            content_type: None,
        })?;
        if resp.status != 200 {
            tracing::warn!(
                status = resp.status,
                "the mail server refused an account request"
            );
            return Err(odd("registry status"));
        }
        serde_json::from_slice(&resp.body).map_err(|_| odd("registry JSON"))
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
        // The domain is the server's own setting (modules/mail.nix); only
        // its id is looked up, once.
        let domain_id = {
            let mut slot = self.domain_id.lock().unwrap_or_else(|e| e.into_inner());
            match slot.as_ref() {
                Some(id) => id.clone(),
                None => {
                    let resp = self.admin(config, vec![crate::mail_setup::domains_call()])?;
                    let id = crate::mail_setup::domain_id(&resp, &config.domain).map_err(|e| {
                        tracing::warn!(error = %e, "no mail domain");
                        odd("x:Domain")
                    })?;
                    *slot = Some(id.clone());
                    id
                }
            }
        };
        let resp = self.admin(
            config,
            vec![crate::mail_setup::account_call(
                &domain_id,
                &person.name,
                &person.display_name,
            )],
        )?;
        crate::mail_setup::account_made(&resp).map_err(|e| {
            tracing::warn!(error = %e, "the mail server would not make an account");
            odd("x:Account")
        })?;
        let resp = self.transport.send(&Request {
            method: "GET",
            path: "/jmap/session".into(),
            user: login(config, person),
            secret: self.secret(config)?,
            body: None,
            content_type: None,
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
            user: login(config, person),
            secret: self.secret(config)?,
            body: Some(serde_json::to_vec(&envelope(calls(&account))).unwrap_or_default()),
            content_type: None,
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

    /// The folders the page lists: every one but Quarantine.
    pub fn mailboxes(&self, person: &crate::people::Person) -> Result<Vec<Mailbox>, Refused> {
        let mut boxes = self.all_mailboxes(person)?;
        boxes.retain(|m| !(m.role.is_none() && m.name == QUARANTINE));
        Ok(boxes)
    }

    fn all_mailboxes(&self, person: &crate::people::Person) -> Result<Vec<Mailbox>, Refused> {
        let (_, resp) = self.jmap(person, mailboxes_call)?;
        let boxes = parse_mailboxes(&resp)?;
        if with_role(&boxes, "archive").is_some() {
            return Ok(boxes);
        }
        let (_, resp) = self.jmap(person, archive_calls)?;
        parse_mailboxes(&resp)
    }

    /// Move `person`'s message `id` into Quarantine, making the folder the
    /// first time. Nothing of the message is read.
    pub fn hold(&self, person: &crate::people::Person, id: &str) -> Result<(), Refused> {
        check_id(id)?;
        let boxes = self.all_mailboxes(person)?;
        let quarantine = match quarantine_of(&boxes) {
            Some(q) => q.to_string(),
            None => {
                let (_, resp) = self.jmap(person, quarantine_calls)?;
                let boxes = parse_mailboxes(&resp)?;
                quarantine_of(&boxes)
                    .ok_or_else(|| odd("no Quarantine"))?
                    .to_string()
            }
        };
        self.change(
            person,
            id,
            &Change {
                mailbox: Some(quarantine),
                ..Change::default()
            },
        )
    }

    /// Move a held message back to `person`'s inbox.
    pub fn release(&self, person: &crate::people::Person, id: &str) -> Result<(), Refused> {
        check_id(id)?;
        let boxes = self.all_mailboxes(person)?;
        let inbox = with_role(&boxes, "inbox").ok_or_else(|| odd("no Inbox"))?;
        self.change(
            person,
            id,
            &Change {
                mailbox: Some(inbox.to_string()),
                ..Change::default()
            },
        )
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

    pub fn message(
        &self,
        person: &crate::people::Person,
        id: &str,
        mark_read: bool,
    ) -> Result<Message, Refused> {
        check_id(id)?;
        let (_, resp) = self.jmap(person, |a| message_calls(a, id, mark_read))?;
        parse_message(&resp, mark_read)
    }

    pub fn html(&self, person: &crate::people::Person, id: &str) -> Result<String, Refused> {
        check_id(id)?;
        let (_, resp) = self.jmap(person, |a| html_calls(a, id))?;
        parse_html(&resp)
    }

    /// Send `draft`. `autocrypt` is the person's `Autocrypt:` header value
    /// when they have a key (`crate::mail_pgp::autocrypt_header`).
    pub fn send(
        &self,
        person: &crate::people::Person,
        draft: &Draft,
        autocrypt: Option<&str>,
    ) -> Result<(), Refused> {
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
        let mut draft = draft.clone();
        let Some(body) = draft.pgp.clone() else {
            for file in std::mem::take(&mut draft.files) {
                let resp = self.transport.send(&Request {
                    method: "POST",
                    path: format!("/jmap/upload/{account}/"),
                    user: login(config, person),
                    secret: self.secret(config)?,
                    body: Some(file.data.clone()),
                    content_type: Some(file.kind.clone()),
                })?;
                draft.attached.push(parse_uploaded(&resp, &file)?);
            }
            let (_, resp) = self.jmap(person, |_| {
                let mut calls = send_calls(&account, &from, &prep, &draft);
                if let Some(value) = autocrypt {
                    with_header(&mut calls, "header:Autocrypt:asRaw", value);
                }
                calls
            })?;
            return parse_sent(&resp, prep.identity.is_none());
        };
        let identity = match &prep.identity {
            Some(id) => id.clone(),
            None => {
                let (_, resp) = self.jmap(person, |a| identity_calls(a, &from))?;
                parse_identity(&resp)?
            }
        };
        let boundary = format!("losos-{}", random_hex());
        if matches!(&body, PgpBody::Signed { part, .. } if part.contains(&boundary)) {
            return refuse(400, "the signed part cannot be sent as it is");
        }
        let mut references = prep.references.clone();
        references.extend(prep.message_id.iter().cloned());
        let date = chrono::Utc::now().to_rfc2822();
        let message_id = format!("<{}@{}>", random_hex(), config.domain);
        let raw = raw_message(
            &Envelope {
                from: &from,
                to: &draft.to,
                cc: &draft.cc,
                subject: &draft.subject,
                date: &date,
                message_id: &message_id,
                in_reply_to: &prep.message_id,
                references: &references,
                autocrypt,
            },
            &body,
            &boundary,
        );
        let blob = self.upload(person, &account, raw)?;
        let (_, resp) = self.jmap(person, |a| import_calls(a, &blob, &prep.drafts))?;
        let email = parse_imported(&resp)?;
        let (_, resp) = self.jmap(person, |a| submit_calls(a, &identity, &email, &prep))?;
        parse_submitted(&resp)
    }

    /// Upload a whole message; its blob id.
    fn upload(
        &self,
        person: &crate::people::Person,
        account: &str,
        raw: Vec<u8>,
    ) -> Result<String, Refused> {
        let config = self.open().ok_or_else(closed)?;
        let resp = self.transport.send(&Request {
            method: "POST",
            path: format!("/jmap/upload/{account}/"),
            user: login(config, person),
            secret: self.secret(config)?,
            body: Some(raw),
            content_type: Some("message/rfc822".to_string()),
        })?;
        if !(200..300).contains(&resp.status) {
            tracing::warn!(status = resp.status, "the mail server refused an upload");
            return Err(odd("upload"));
        }
        let doc: Value = serde_json::from_slice(&resp.body).map_err(|_| odd("upload JSON"))?;
        doc.get("blobId")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| odd("upload blobId"))
    }

    /// The whole message as it arrived, for the browser to decrypt or check
    /// a signature on.
    pub fn raw(&self, person: &crate::people::Person, id: &str) -> Result<Vec<u8>, Refused> {
        check_id(id)?;
        let (_, resp) = self.jmap(person, |a| blob_calls(a, id))?;
        let blob = parse_blob(&resp)?;
        self.download(person, &blob).map(|(_, bytes)| bytes)
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
            user: login(config, person),
            secret: self.secret(config)?,
            body: None,
            content_type: None,
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
            content_type: None,
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
            { "id": "y", "name": "2026", "role": null, "parentId": "z", "totalEmails": 0, "unreadEmails": 0 },
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
                "2026",
                "Receipts",
                "Junk Mail",
                "Deleted Items"
            ]
        );
        assert_eq!(boxes[0].unread, 2);
        // The page draws the tree from each folder's parent.
        assert_eq!(boxes[3].parent_id.as_deref(), Some("z"));
        assert_eq!(boxes[4].parent_id, None);
        assert_eq!(
            serde_json::to_value(&boxes[3]).unwrap()["parentId"],
            json!("z")
        );
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
        let calls = message_calls("c", "m1", true);
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
        let m = parse_message(&resp, true).unwrap();
        assert!(m.summary.seen);
        assert_eq!(m.text, "line one\nline two");
        assert!(m.has_html);
        assert_eq!(m.mailbox_ids, ["a"]);
        assert_eq!(m.cc[0].email, "c@x.org");
        assert_eq!(m.attachments[1].name, "attachment");
        assert_eq!(m.attachments[1].kind, "application/octet-stream");

        let missing =
            json!({ "methodResponses": [["Email/get", { "list": [], "notFound": ["m9"] }, "g"]] });
        assert_eq!(parse_message(&missing, true).unwrap_err().status, 404);
    }

    #[test]
    fn the_companion_opens_a_message_and_leaves_it_unread() {
        let calls = message_calls("c", "m1", false);
        assert_eq!(calls.len(), 1, "{calls:?}");
        assert_eq!(calls[0][0], "Email/get");
        let resp = json!({ "methodResponses": [["Email/get", { "list": [{
            "id": "m1", "threadId": "t", "subject": "S", "from": [], "to": [],
            "receivedAt": "2026-10-21T10:00:00Z", "preview": "", "keywords": {},
            "hasAttachment": false, "mailboxIds": { "a": true },
            "textBody": [], "htmlBody": [], "attachments": [], "bodyValues": {},
        }] }, "g"]] });
        assert!(!parse_message(&resp, false).unwrap().summary.seen);
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
            ..Draft::default()
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
    fn attachments_arrive_as_base64_and_are_kept_to_what_a_header_takes() {
        let d = parse_draft(&json!({
            "to": "eva@mattbox.home.arpa",
            "attachments": [
                { "name": "plan \"v2\"/../x.docx", "type": "application/vnd.openxmlformats-officedocument.wordprocessingml.document", "data": "UEsDBA==" },
                { "name": "", "type": "text/plain; charset=utf-8\r\nX: y", "data": "aGk=" },
            ],
        }))
        .unwrap();
        assert_eq!(d.files.len(), 2);
        assert_eq!(d.files[0].name, "plan v2..x.docx");
        assert_eq!(d.files[0].data, b"PK\x03\x04");
        assert_eq!(d.files[1].name, "attachment");
        assert_eq!(d.files[1].kind, "text/plain");
        assert_eq!(
            clean_media_type("image/png\r\nBcc: x"),
            "application/octet-stream"
        );
        assert!(!format!("{:?}", d.files[1]).contains("hi"));

        let r =
            parse_draft(&json!({ "to": "a@b.org", "attachments": [{ "data": "not base64!" }] }))
                .unwrap_err();
        assert_eq!(r.status, 400);
        let many: Vec<Value> = (0..21).map(|_| json!({ "data": "" })).collect();
        assert_eq!(
            parse_draft(&json!({ "to": "a@b.org", "attachments": many }))
                .unwrap_err()
                .status,
            400
        );
        use base64::Engine;
        let big =
            base64::engine::general_purpose::STANDARD
                .encode(vec![0u8; MAX_ATTACHMENT_BYTES / 2 + 1]);
        let r = parse_draft(
            &json!({ "to": "a@b.org", "attachments": [{ "data": big }, { "data": big }] }),
        )
        .unwrap_err();
        assert_eq!(r.status, 413);
    }

    #[test]
    fn an_attached_blob_goes_into_the_message_as_an_attachment() {
        let file = Upload {
            name: "plan.docx".into(),
            kind: "application/octet-stream".into(),
            data: vec![1, 2, 3],
        };
        let resp = Response {
            status: 201,
            content_type: "application/json".into(),
            body: br#"{"accountId":"c","blobId":"b9","type":"application/octet-stream","size":3}"#
                .to_vec(),
        };
        let a = parse_uploaded(&resp, &file).unwrap();
        assert_eq!((a.blob_id.as_str(), a.size), ("b9", 3));
        let bad = Response {
            status: 400,
            content_type: String::new(),
            body: Vec::new(),
        };
        assert!(parse_uploaded(&bad, &file).is_err());

        let prep = Prepared {
            identity: Some("i".into()),
            drafts: "d".into(),
            sent: "e".into(),
            message_id: vec![],
            references: vec![],
        };
        let from = Address {
            name: None,
            email: "matus@mattbox.home.arpa".into(),
        };
        let draft = Draft {
            to: vec!["eva@mattbox.home.arpa".into()],
            attached: vec![a],
            ..Draft::default()
        };
        let calls = send_calls("c", &from, &prep, &draft);
        assert_eq!(
            calls[0][1]["create"]["draft"]["attachments"],
            json!([{ "blobId": "b9", "type": "application/octet-stream", "name": "plan.docx", "disposition": "attachment" }])
        );
        let plain = send_calls(
            "c",
            &from,
            &prep,
            &Draft {
                attached: vec![],
                ..draft
            },
        );
        assert!(plain[0][1]["create"]["draft"].get("attachments").is_none());

        let req = Request {
            method: "POST",
            path: "/jmap/upload/c/".into(),
            user: "u".into(),
            secret: "s".into(),
            body: Some(vec![1]),
            content_type: Some("image/png".into()),
        };
        let text = String::from_utf8_lossy(&request_bytes("h", &req)).into_owned();
        assert!(text.contains("Content-Type: image/png\r\n"));
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
            ..Draft::default()
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
    fn the_first_call_makes_the_account_and_acts_as_the_person() {
        let dir = tempdir();
        let fake = Fake::default();
        let seen = fake.seen.clone();
        *fake.answers.lock().unwrap() = vec![
            ok(json!({ "methodResponses": [["x:Domain/get",
                { "list": [{ "id": "dom", "name": "mattbox.home.arpa" }] }, "d"]] })),
            ok(json!({ "methodResponses": [["x:Account/set",
                { "notCreated": { "a": { "type": "alreadyExists" } } }, "a"]] })),
            ok(json!({ "primaryAccounts": { "urn:ietf:params:jmap:mail": "d" } })),
            ok(mailboxes_answer()),
            ok(archived_answer()),
            ok(archived_answer()),
        ];
        let mail = Mail::new(Some(config(&dir)), Box::new(fake));
        let boxes = mail.mailboxes(&person("eva")).unwrap();
        // Stalwart had no Archive; there is one now, after Sent.
        assert_eq!(boxes.len(), 8);
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
        for req in &seen[..2] {
            assert_eq!(
                (req.method, req.path.as_str(), req.user.as_str()),
                ("POST", "/jmap/", MASTER_USER)
            );
        }
        let domain: Value = serde_json::from_slice(seen[0].body.as_ref().unwrap()).unwrap();
        assert_eq!(domain["methodCalls"][0][0], "x:Domain/get");
        assert_eq!(domain["using"][1], "urn:stalwart:jmap");
        let account: Value = serde_json::from_slice(seen[1].body.as_ref().unwrap()).unwrap();
        assert_eq!(
            account["methodCalls"][0][1]["create"]["a"],
            json!({ "@type": "User", "name": "eva", "domainId": "dom", "description": "EVA" })
        );
        assert_eq!(seen[2].path, "/jmap/session");
        for req in &seen[2..] {
            assert_eq!(req.user, "eva@mattbox.home.arpa%losos");
            assert_eq!(req.secret, "s3cret");
        }
        let jmap: Value = serde_json::from_slice(seen[3].body.as_ref().unwrap()).unwrap();
        assert_eq!(jmap["methodCalls"][0][1]["accountId"], "d");
    }

    #[test]
    fn held_mail_goes_to_a_quarantine_the_page_never_lists() {
        let dir = tempdir();
        let fake = Fake::default();
        let seen = fake.seen.clone();
        let folders = |extra: &[Value]| {
            let mut list = mailboxes_answer()["methodResponses"][0][1]["list"].clone();
            let all = list.as_array_mut().unwrap();
            all.push(json!({ "id": "f", "name": "Archive", "role": "archive", "totalEmails": 0, "unreadEmails": 0 }));
            all.extend(extra.iter().cloned());
            list
        };
        let quarantine = json!({ "id": "q", "name": QUARANTINE, "role": null, "totalEmails": 1, "unreadEmails": 1 });
        let moved =
            json!({ "methodResponses": [["Email/set", { "updated": { "m1": null } }, "s"]] });
        *fake.answers.lock().unwrap() = vec![
            ok(json!({ "methodResponses": [["x:Domain/get",
                { "list": [{ "id": "dom", "name": "mattbox.home.arpa" }] }, "d"]] })),
            ok(json!({ "methodResponses": [["x:Account/set",
                { "created": { "a": { "id": "e" } } }, "a"]] })),
            ok(json!({ "primaryAccounts": { "urn:ietf:params:jmap:mail": "d" } })),
            ok(json!({ "methodResponses": [["Mailbox/get", { "list": folders(&[]) }, "m"]] })),
            ok(json!({ "methodResponses": [
                ["Mailbox/set", { "created": { "q": { "id": "q" } } }, "s"],
                ["Mailbox/get", { "list": folders(std::slice::from_ref(&quarantine)) }, "m"],
            ]})),
            ok(moved.clone()),
            ok(
                json!({ "methodResponses": [["Mailbox/get", { "list": folders(std::slice::from_ref(&quarantine)) }, "m"]] }),
            ),
            ok(
                json!({ "methodResponses": [["Mailbox/get", { "list": folders(&[quarantine]) }, "m"]] }),
            ),
            ok(moved),
        ];
        let mail = Mail::new(Some(config(&dir)), Box::new(fake));
        mail.hold(&person("eva"), "m1").unwrap();
        let boxes = mail.mailboxes(&person("eva")).unwrap();
        assert!(boxes.iter().all(|b| b.name != QUARANTINE), "{boxes:?}");
        mail.release(&person("eva"), "m1").unwrap();

        let seen = seen.lock().unwrap();
        let body =
            |i: usize| -> Value { serde_json::from_slice(seen[i].body.as_ref().unwrap()).unwrap() };
        assert_eq!(
            body(4)["methodCalls"][0][1]["create"]["q"]["name"],
            QUARANTINE
        );
        assert_eq!(
            body(5)["methodCalls"][0][1]["update"]["m1"]["mailboxIds"],
            json!({ "q": true })
        );
        // Nothing of the message was read on the way.
        for i in 3..seen.len() {
            let calls = body(i)["methodCalls"].to_string();
            assert!(!calls.contains("Email/get"), "{calls}");
        }
        assert_eq!(
            body(8)["methodCalls"][0][1]["update"]["m1"]["mailboxIds"],
            json!({ "a": true })
        );
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
            mail.message(&person("eva"), "../../api", true)
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

    #[test]
    fn a_raw_header_is_read_as_stalwart_names_it() {
        // Stalwart answers header:Content-Type:asRaw under header:Content-Type.
        let stalwart = json!({ "header:Content-Type": " multipart/signed; micalg=pgp-sha256" });
        assert_eq!(
            raw_header(&stalwart, "Content-Type"),
            Some(" multipart/signed; micalg=pgp-sha256")
        );
        let spec = json!({ "header:Autocrypt:asRaw": " addr=x" });
        assert_eq!(raw_header(&spec, "Autocrypt"), Some(" addr=x"));
        assert_eq!(raw_header(&spec, "Content-Type"), None);
    }

    #[test]
    fn openpgp_mail_is_told_apart_by_its_type_or_its_armour() {
        assert_eq!(
            pgp_kind(
                "multipart/encrypted; protocol=\"application/pgp-encrypted\"; boundary=x",
                ""
            ),
            Some("encrypted")
        );
        assert_eq!(
            pgp_kind(
                "multipart/signed; micalg=pgp-sha256;\r\n protocol=\"application/pgp-signature\"",
                ""
            ),
            Some("signed")
        );
        assert_eq!(
            pgp_kind(
                "multipart/signed; protocol=\"application/pkcs7-signature\"",
                ""
            ),
            None
        );
        assert_eq!(
            pgp_kind("text/plain", "hi\n-----BEGIN PGP MESSAGE-----\n"),
            Some("encrypted")
        );
        assert_eq!(
            pgp_kind("", "-----BEGIN PGP SIGNED MESSAGE-----"),
            Some("signed")
        );
        assert_eq!(pgp_kind("text/plain", "hello"), None);
    }

    #[test]
    fn a_pgp_body_is_armour_or_a_7bit_part_and_nothing_else() {
        let armored = "-----BEGIN PGP MESSAGE-----\n\nwV4D\n-----END PGP MESSAGE-----\n";
        let body = parse_pgp(&json!({ "mime": "encrypted", "armored": armored })).unwrap();
        assert_eq!(
            body,
            PgpBody::Encrypted {
                armored: "-----BEGIN PGP MESSAGE-----\r\n\r\nwV4D\r\n-----END PGP MESSAGE-----"
                    .into()
            }
        );
        // Anything after the armour could be a header or a part of its own.
        for bad in [
            "-----BEGIN PGP MESSAGE-----\nx\n-----END PGP MESSAGE-----\n--b\nContent-Type: text/html",
            "hello",
            "-----BEGIN PGP MESSAGE-----\nž\n-----END PGP MESSAGE-----",
        ] {
            assert!(parse_pgp(&json!({ "mime": "encrypted", "armored": bad })).is_err(), "{bad}");
        }
        let sig = "-----BEGIN PGP SIGNATURE-----\n\niHUE\n-----END PGP SIGNATURE-----";
        let part = "Content-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: quoted-printable\r\n\r\nAhoj=\r\n";
        let body = parse_pgp(&json!({ "mime": "signed", "part": part, "signature": sig })).unwrap();
        assert!(matches!(&body, PgpBody::Signed { part: p, .. } if p == part));
        // A signed part is kept byte for byte, so it must already be CRLF.
        let lf = part.replace("\r\n", "\n");
        assert!(parse_pgp(&json!({ "mime": "signed", "part": lf, "signature": sig })).is_err());
        assert!(
            parse_pgp(&json!({ "mime": "signed", "part": "hello\r\n\r\n", "signature": sig }))
                .is_err()
        );
        assert!(parse_pgp(&json!({ "mime": "other" })).is_err());
        let draft = parse_draft(&json!({ "to": "eva@x.org", "text": "", "pgp": { "mime": "encrypted", "armored": armored } })).unwrap();
        assert!(draft.pgp.is_some());
        assert!(parse_draft(&json!({ "to": "eva@x.org", "text": "" }))
            .unwrap()
            .pgp
            .is_none());
    }

    #[test]
    fn names_and_subjects_outside_ascii_are_encoded_words() {
        assert_eq!(encoded("Plan"), "Plan");
        let e = encoded("Plán na štvrtok");
        assert!(e.starts_with("=?UTF-8?B?") && e.ends_with("?="), "{e}");
        let long = encoded(&"ľ".repeat(80));
        assert!(long.split("\r\n ").all(|w| w.len() <= 75), "{long}");
        assert_eq!(mailbox(Some("Eva"), "eva@x.org"), "Eva <eva@x.org>");
        assert_eq!(
            mailbox(Some("Eva, M."), "eva@x.org"),
            "\"Eva, M.\" <eva@x.org>"
        );
        assert_eq!(mailbox(None, "eva@x.org"), "eva@x.org");
        assert!(mailbox(Some("Éva"), "eva@x.org").starts_with("=?UTF-8?B?"));
    }

    #[test]
    fn an_encrypted_message_is_rfc_3156_with_the_armour_untouched() {
        let from = Address {
            name: Some("Matúš".into()),
            email: "matus@mattbox.home.arpa".into(),
        };
        let to = vec!["eva@mattbox.home.arpa".to_string()];
        let env = Envelope {
            from: &from,
            to: &to,
            cc: &[],
            subject: "Plan",
            date: "Thu, 22 Oct 2026 10:00:00 +0000",
            message_id: "<abc@mattbox.home.arpa>",
            in_reply_to: &["m1@x".to_string()],
            references: &["m0@x".to_string(), "m1@x".to_string()],
            autocrypt: Some(
                " addr=matus@mattbox.home.arpa; prefer-encrypt=mutual; keydata=\r\n AAAA",
            ),
        };
        let armored = "-----BEGIN PGP MESSAGE-----\r\n\r\nwV4D\r\n-----END PGP MESSAGE-----";
        let raw = String::from_utf8(raw_message(
            &env,
            &PgpBody::Encrypted {
                armored: armored.into(),
            },
            "B1",
        ))
        .unwrap();
        assert!(raw.contains("\r\nTo: eva@mattbox.home.arpa\r\n"));
        assert!(raw.contains("In-Reply-To: <m1@x>\r\nReferences: <m0@x> <m1@x>\r\n"));
        assert!(raw.contains(
            "Autocrypt: addr=matus@mattbox.home.arpa; prefer-encrypt=mutual; keydata=\r\n AAAA\r\n"
        ));
        assert!(raw.contains("Content-Type: multipart/encrypted;\r\n protocol=\"application/pgp-encrypted\";\r\n boundary=\"B1\""));
        assert!(raw.contains("--B1\r\nContent-Type: application/pgp-encrypted\r\n"));
        assert!(raw.contains("Version: 1\r\n"));
        assert!(raw.contains(&format!("\r\n\r\n{armored}\r\n\r\n--B1--\r\n")));
        assert!(raw.lines().all(|l| l.len() <= 998));
        assert!(!raw.replace("\r\n", "").contains('\n'), "bare LF");

        let part = "Content-Type: text/plain; charset=utf-8\r\n\r\nAhoj";
        let raw = String::from_utf8(raw_message(
            &env,
            &PgpBody::Signed {
                part: part.into(),
                signature: "SIG".into(),
                micalg: "pgp-sha256",
            },
            "B2",
        ))
        .unwrap();
        assert!(raw.contains("multipart/signed; micalg=pgp-sha256;"));
        // Exactly the bytes signed, between the delimiter and the next one.
        assert!(raw.contains(&format!(
            "--B2\r\n{part}\r\n--B2\r\nContent-Type: application/pgp-signature"
        )));
    }

    #[test]
    fn an_imported_message_is_submitted_and_moved_to_sent() {
        let prep = Prepared {
            identity: Some("i7".into()),
            drafts: "d".into(),
            sent: "s".into(),
            message_id: vec![],
            references: vec![],
        };
        let calls = import_calls("c", "blob1", "d");
        assert_eq!(calls[0][1]["emails"]["m"]["blobId"], "blob1");
        assert_eq!(
            calls[0][1]["emails"]["m"]["mailboxIds"],
            json!({ "d": true })
        );
        let resp = json!({ "methodResponses": [["Email/import", { "created": { "m": { "id": "e9" } } }, "e"]] });
        assert_eq!(parse_imported(&resp).unwrap(), "e9");
        let resp = json!({ "methodResponses": [["Email/import", { "notCreated": { "m": { "type": "invalidEmail" } } }, "e"]] });
        assert_eq!(parse_imported(&resp).unwrap_err().status, 422);
        let calls = submit_calls("c", "i7", "e9", &prep);
        assert_eq!(calls[0][1]["create"]["send"]["emailId"], "e9");
        assert_eq!(
            calls[0][1]["onSuccessUpdateEmail"]["#send"]["mailboxIds/s"],
            true
        );
    }

    #[test]
    fn a_plain_message_carries_the_autocrypt_header() {
        let prep = Prepared {
            identity: Some("i7".into()),
            drafts: "d".into(),
            sent: "s".into(),
            message_id: vec![],
            references: vec![],
        };
        let from = Address {
            name: None,
            email: "matus@mattbox.home.arpa".into(),
        };
        let draft = parse_draft(&json!({ "to": "eva@mattbox.home.arpa", "text": "hi" })).unwrap();
        let mut calls = send_calls("c", &from, &prep, &draft);
        with_header(&mut calls, "header:Autocrypt:asRaw", " addr=x");
        assert_eq!(
            calls[0][1]["create"]["draft"]["header:Autocrypt:asRaw"],
            " addr=x"
        );
    }
}
