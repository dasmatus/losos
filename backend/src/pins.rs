//! What the owner pinned to the admin page's sidebar.
//!
//! A pin is a reference, never a link: an app tile of the Overview by its
//! id in the page's catalogue, an app installed from the Apps search by its
//! release name, a file or folder in LosOS cloud by its file id and path,
//! or a repository on LosOS Git by owner and name. The page turns each one
//! into an address (`admin-ui/app/src/lib/pins.ts`), so nothing stored here
//! can send a click anywhere but this box. The list is ordered, and the
//! order is the owner's: `PUT /api/pins` replaces it whole, which is also
//! how a pin moves.
//!
//! Like the look (`crate::look`), the list is appliance state beside
//! `state.json`, so a phone and a laptop show the same sidebar, and writing
//! it starts no rebuild.
//!
//! The page also needs something to pin from, and this module carries the
//! two lists it offers, each one searchable so the page can look things up
//! by name: a folder of LosOS cloud (or the files whose name matches, from
//! anywhere in it) and the owner's repositories on LosOS Git.
//!
//! - **LosOS Git** is asked as the bot administrator lososd already talks
//!   to it as (`crate::config_repo`), for the owner's repositories,
//!   private ones included.
//! - **LosOS cloud** is asked over WebDAV with an *app password* of the
//!   owner's (Nextcloud's "devices & sessions" kind). lososd has no other
//!   key to the owner's files and keeps no copy of their password, so it
//!   asks LosOS cloud for one the moment it holds the password anyway: a
//!   sign-in with it, or the first password at setup. The app password is
//!   kept at `$LOSOS_STATE_DIR/cloud-key`, mode 0600, and shows up in the
//!   owner's LosOS cloud security settings as "LosOS admin page", where they
//!   can revoke it. A revoked or missing one makes the folder list say
//!   "sign in with your password once", nothing worse.
//!
//! Everything here is pure: the request bytes, the WebDAV answer, the
//! chunked body. The socket is [`crate::losos::Losos::cloud_request`].

use crate::losos::Losos;
use crate::setup::Secret;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// How many pins the sidebar keeps. A sidebar of more is a second app grid.
pub const MAX_PINS: usize = 24;
/// Longest LosOS cloud path a pin or a listing takes, in bytes.
pub const MAX_PATH_BYTES: usize = 1024;
/// Longest catalogue id of a LosOS cloud app.
pub const MAX_APP_ID_CHARS: usize = 24;
/// Longest owner or repository name on LosOS Git (`crate::forge::names::MAX_REPO`).
pub const MAX_REPO_PART_CHARS: usize = 100;
/// Most entries one folder listing returns; the rest are left out and the
/// answer says so.
pub const MAX_ENTRIES: usize = 500;
/// Most repositories one listing returns (one page).
pub const MAX_REPOS: usize = 50;
/// Most results one search of LosOS cloud returns.
pub const MAX_HITS: usize = 50;
/// Longest search text, in characters.
pub const MAX_QUERY_CHARS: usize = 100;
/// The name LosOS cloud shows for the app password in its security settings.
pub const KEY_CLIENT: &str = "LosOS admin page";

/// One thing on the sidebar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum Pin {
    /// An app tile of the Overview, by its id in the page's catalogue
    /// (`admin-ui/app/src/lib/apps.ts`): the apps of LosOS cloud, LosOS
    /// Git and this page's settings.
    Tile { app: String },
    /// An app installed from the Apps search, by release name.
    App { release: String },
    /// A file or folder in LosOS cloud. `id` opens it; `path` names it.
    File { id: u64, path: String, folder: bool },
    /// A repository on LosOS Git.
    Repo { owner: String, name: String },
}

impl Pin {
    /// What makes two pins the same pin.
    fn key(&self) -> String {
        match self {
            Pin::Tile { app } => format!("tile:{app}"),
            Pin::App { release } => format!("app:{release}"),
            Pin::File { id, .. } => format!("file:{id}"),
            Pin::Repo { owner, name } => format!("repo:{owner}/{name}"),
        }
    }
}

/// The document on disk.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pins {
    pub pins: Vec<Pin>,
}

/// A request the owner can fix by sending a different one; the HTTP layer
/// answers it as a 400 carrying the sentence.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct Invalid(pub String);

fn invalid<T>(message: impl Into<String>) -> anyhow::Result<T> {
    Err(anyhow::Error::new(Invalid(message.into())))
}

// ── The list ──────────────────────────────────────────────────────────────

fn render(pins: &Pins) -> Value {
    json!({ "pins": pins.pins, "limit": MAX_PINS })
}

/// Read a `PUT /api/pins` body: `{"pins": [...]}`.
pub fn parse_pins(doc: &Value) -> anyhow::Result<Vec<Pin>> {
    let Some(list) = doc.get("pins").and_then(Value::as_array) else {
        return invalid("Send the pins as a list under \"pins\".");
    };
    list.iter()
        .map(|item| {
            serde_json::from_value::<Pin>(item.clone()).map_err(|_| {
                anyhow::Error::new(Invalid("That is not a pin this box knows.".into()))
            })
        })
        .collect()
}

/// Check one pin's fields.
pub fn validate_pin(pin: &Pin) -> anyhow::Result<()> {
    match pin {
        Pin::Tile { app } => {
            let ok = !app.is_empty()
                && app.chars().count() <= MAX_APP_ID_CHARS
                && app.chars().all(|c| c.is_ascii_lowercase());
            if !ok {
                return invalid("That is not an app on the Overview.");
            }
        }
        Pin::App { release } => {
            if crate::apps::validate_release(release).is_err() {
                return invalid("That is not the name of an installed app.");
            }
        }
        Pin::File { id, path, .. } => {
            if *id == 0 {
                return invalid("That file has no id in LosOS cloud.");
            }
            validate_path(path)?;
            if path == "/" {
                return invalid("Pin a folder inside LosOS cloud, not the whole of it.");
            }
        }
        Pin::Repo { owner, name } => {
            if !repo_part(owner) || !repo_part(name) {
                return invalid("That is not a repository on LosOS Git.");
            }
        }
    }
    Ok(())
}

fn repo_part(part: &str) -> bool {
    !part.is_empty()
        && part != "."
        && part != ".."
        && part.chars().count() <= MAX_REPO_PART_CHARS
        && part
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// A path inside LosOS cloud: from the top (`/`), no empty, `.` or `..`
/// part, nothing that is not printable.
pub fn validate_path(path: &str) -> anyhow::Result<()> {
    if !path.starts_with('/') || path.len() > MAX_PATH_BYTES {
        return invalid("That is not a folder in LosOS cloud.");
    }
    if path == "/" {
        return Ok(());
    }
    let ok = path
        .trim_end_matches('/')
        .split('/')
        .skip(1)
        .all(|part| !part.is_empty() && part != "." && part != ".." && !part.contains(['\\']))
        && !path.chars().any(char::is_control);
    if ok {
        Ok(())
    } else {
        invalid("That is not a folder in LosOS cloud.")
    }
}

/// `GET /api/pins`.
pub fn cmd_pins<L: Losos>(l: &mut L) -> anyhow::Result<Value> {
    Ok(render(&l.load_pins()?))
}

/// `PUT /api/pins`: the whole list, in the owner's order.
pub fn cmd_put_pins<L: Losos>(l: &mut L, pins: &[Pin]) -> anyhow::Result<Value> {
    if pins.len() > MAX_PINS {
        return invalid(format!("The sidebar keeps {MAX_PINS} pins at most."));
    }
    let mut seen = std::collections::HashSet::new();
    for pin in pins {
        validate_pin(pin)?;
        if !seen.insert(pin.key()) {
            return invalid("That is pinned already.");
        }
    }
    let doc = Pins {
        pins: pins.to_vec(),
    };
    l.save_pins(&doc)?;
    Ok(render(&doc))
}

// ── LosOS Git: the owner's repositories ───────────────────────────────────

/// Search text: printable, at most [`MAX_QUERY_CHARS`]. `None` for none.
pub fn validate_query(query: Option<&str>) -> anyhow::Result<Option<String>> {
    let Some(query) = query.map(str::trim).filter(|q| !q.is_empty()) else {
        return Ok(None);
    };
    if query.chars().count() > MAX_QUERY_CHARS || query.chars().any(char::is_control) {
        return invalid(format!(
            "Search for {MAX_QUERY_CHARS} characters at most, on one line."
        ));
    }
    Ok(Some(query.to_string()))
}

/// `GET /api/git/repos[?q=...]`: every repository, or those whose name or
/// description holds the text.
pub fn cmd_git_repos<L: Losos>(l: &mut L, query: Option<&str>) -> anyhow::Result<Value> {
    let query = validate_query(query)?;
    use crate::config_repo::{classify, Answer, GitOp, NotUp};
    let unavailable =
        |reason: &str| Ok(json!({ "available": false, "reason": reason, "repos": [] }));
    let Some(repo) = l.config_repo() else {
        return unavailable("noGit");
    };
    if !repo.via_server() {
        return unavailable("noGit");
    }
    let op = GitOp::ListRepos {
        login: crate::setup::DEFAULT_ADMIN_USER.to_string(),
    };
    let (status, body) = match l.git_request(&op) {
        Ok(answer) => answer,
        Err(e) if e.downcast_ref::<NotUp>().is_some() => return unavailable("notUp"),
        Err(e) => return Err(e),
    };
    match classify(status, &body) {
        Answer::Ok(doc) => {
            let mut repos = parse_repos(&doc);
            if let Some(q) = &query {
                let q = q.to_lowercase();
                repos.retain(|r| {
                    ["name", "description"]
                        .iter()
                        .any(|k| r[k].as_str().is_some_and(|v| v.to_lowercase().contains(&q)))
                });
            }
            Ok(json!({ "available": true, "repos": repos }))
        }
        // No account yet: the reconciler makes it within a minute of LosOS
        // Git coming up, so this is "not yet", not "none".
        Answer::Missing => unavailable("notUp"),
        other => anyhow::bail!("LosOS Git refused the repository list: {other:?}"),
    }
}

/// The rows of a LosOS Git repository list, sorted by name.
pub fn parse_repos(doc: &Value) -> Vec<Value> {
    let mut rows: Vec<Value> = doc
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .filter_map(|r| {
            let name = r.get("name")?.as_str()?;
            let owner = r.get("owner")?.get("login")?.as_str()?;
            if !repo_part(owner) || !repo_part(name) {
                return None;
            }
            Some(json!({
                "owner": owner,
                "name": name,
                "private": r.get("private").and_then(Value::as_bool).unwrap_or(false),
                "description": r.get("description").and_then(Value::as_str).unwrap_or(""),
            }))
        })
        .take(MAX_REPOS)
        .collect();
    rows.sort_by_key(|r| r["name"].as_str().unwrap_or("").to_lowercase());
    rows
}

// ── LosOS cloud: a folder ─────────────────────────────────────────────────

/// The app password lososd lists the owner's files with.
#[derive(Debug, Clone)]
pub struct CloudKey {
    pub user: String,
    pub key: Secret,
}

impl CloudKey {
    /// The file's text: the user on the first line, the key on the second.
    pub fn encode(&self) -> String {
        format!("{}\n{}\n", self.user, self.key.expose())
    }

    pub fn decode(text: &str) -> Option<CloudKey> {
        let mut lines = text.lines();
        let user = lines.next()?.trim();
        let key = lines.next()?.trim();
        if user.is_empty() || key.is_empty() {
            return None;
        }
        Some(CloudKey {
            user: user.to_string(),
            key: Secret::new(key),
        })
    }
}

/// One request to LosOS cloud.
#[derive(Debug)]
pub enum CloudRequest<'a> {
    /// Trade the owner's password for an app password.
    MintKey { user: &'a str, password: &'a Secret },
    /// One folder, one level deep.
    List { key: &'a CloudKey, path: &'a str },
    /// Files and folders anywhere whose name holds `query`.
    Search { key: &'a CloudKey, query: &'a str },
    /// Sign `user` in through LosOS Login's headers, as nginx does for a
    /// browser, so LosOS cloud makes their account if it has none yet
    /// (`crate::cloud_share`).
    Arrive {
        user: &'a str,
        display_name: &'a str,
    },
}

/// Where LosOS cloud answers on loopback: the socket, the `Host` header and
/// the path its web root is at (`/nextcloud` for the workload, empty for
/// the native vhost). Derived from the sign-in probe's URL, so the two can
/// never disagree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudEndpoint {
    pub addr: String,
    pub host: String,
    pub root: String,
}

impl CloudEndpoint {
    pub fn from_login(endpoint: &crate::signin::LoginEndpoint, host: &str) -> CloudEndpoint {
        let root = endpoint
            .path
            .strip_suffix("/ocs/v2.php/cloud/user")
            .unwrap_or("")
            .trim_end_matches('/')
            .to_string();
        CloudEndpoint {
            addr: endpoint.addr.clone(),
            host: host.to_string(),
            root,
        }
    }
}

const PROPFIND_BODY: &str = concat!(
    r#"<?xml version="1.0"?>"#,
    r#"<d:propfind xmlns:d="DAV:" xmlns:oc="http://owncloud.org/ns">"#,
    "<d:prop><d:resourcetype/><oc:fileid/><oc:size/></d:prop>",
    "</d:propfind>",
);

/// The bytes of one request. HTTP/1.1 with `Connection: close`, so the
/// answer ends where the socket does.
pub fn request_bytes(endpoint: &CloudEndpoint, request: &CloudRequest<'_>) -> Vec<u8> {
    let host: String = endpoint.host.chars().filter(|c| !c.is_control()).collect();
    let mut out = String::new();
    match request {
        CloudRequest::MintKey { user, password } => {
            out.push_str(&format!(
                "GET {}/ocs/v2.php/core/getapppassword HTTP/1.1\r\n",
                endpoint.root
            ));
            out.push_str(&format!("Host: {host}\r\n"));
            out.push_str(&basic(user, password.expose()));
            out.push_str("OCS-APIRequest: true\r\n");
            out.push_str("Accept: application/json\r\n");
            // LosOS cloud names the app password after the client.
            out.push_str(&format!("User-Agent: {KEY_CLIENT}\r\n"));
            out.push_str("Connection: close\r\n\r\n");
        }
        CloudRequest::List { key, path } => {
            out.push_str(&format!(
                "PROPFIND {} HTTP/1.1\r\n",
                dav_path(&endpoint.root, &key.user, path)
            ));
            out.push_str(&format!("Host: {host}\r\n"));
            out.push_str(&basic(&key.user, key.key.expose()));
            out.push_str("Depth: 1\r\n");
            out.push_str("Content-Type: application/xml; charset=utf-8\r\n");
            out.push_str(&format!("Content-Length: {}\r\n", PROPFIND_BODY.len()));
            out.push_str(&format!("User-Agent: {KEY_CLIENT}\r\n"));
            out.push_str("Connection: close\r\n\r\n");
            out.push_str(PROPFIND_BODY);
        }
        CloudRequest::Search { key, query } => {
            let body = search_body(&key.user, query);
            out.push_str(&format!(
                "SEARCH {}/remote.php/dav/ HTTP/1.1\r\n",
                endpoint.root
            ));
            out.push_str(&format!("Host: {host}\r\n"));
            out.push_str(&basic(&key.user, key.key.expose()));
            out.push_str("Content-Type: text/xml; charset=utf-8\r\n");
            out.push_str(&format!("Content-Length: {}\r\n", body.len()));
            out.push_str(&format!("User-Agent: {KEY_CLIENT}\r\n"));
            out.push_str("Connection: close\r\n\r\n");
            out.push_str(&body);
        }
        CloudRequest::Arrive { user, display_name } => {
            let clean = |v: &str| v.chars().filter(|c| !c.is_control()).collect::<String>();
            out.push_str(&format!(
                "GET {}/index.php/apps/user_saml/saml/login HTTP/1.1\r\n",
                endpoint.root
            ));
            out.push_str(&format!("Host: {host}\r\n"));
            out.push_str(&format!("X-Losos-Cloud-User: {}\r\n", clean(user)));
            out.push_str(&format!("X-Losos-Cloud-Name: {}\r\n", clean(display_name)));
            out.push_str(&format!("User-Agent: {KEY_CLIENT}\r\n"));
            out.push_str("Connection: close\r\n\r\n");
        }
    }
    out.into_bytes()
}

/// A WebDAV SEARCH (RFC 5323, as LosOS cloud speaks it) for names that
/// hold `query`, anywhere under the owner's files. `%` is the wildcard, so
/// one typed in the text is dropped rather than matched as one.
fn search_body(user: &str, query: &str) -> String {
    let literal = xml_escape(&query.replace('%', ""));
    let scope = xml_escape(&format!("/files/{user}"));
    format!(
        concat!(
            r#"<?xml version="1.0" encoding="UTF-8"?>"#,
            r#"<d:searchrequest xmlns:d="DAV:" xmlns:oc="http://owncloud.org/ns">"#,
            "<d:basicsearch>",
            "<d:select><d:prop><d:resourcetype/><oc:fileid/><oc:size/></d:prop></d:select>",
            "<d:from><d:scope><d:href>{scope}</d:href><d:depth>infinity</d:depth></d:scope></d:from>",
            "<d:where><d:like><d:prop><d:displayname/></d:prop><d:literal>%{literal}%</d:literal></d:like></d:where>",
            "<d:orderby/>",
            "<d:limit><d:nresults>{limit}</d:nresults></d:limit>",
            "</d:basicsearch>",
            "</d:searchrequest>",
        ),
        scope = scope,
        literal = literal,
        limit = MAX_HITS,
    )
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn basic(user: &str, password: &str) -> String {
    format!(
        "Authorization: Basic {}\r\n",
        crate::signin::base64(format!("{user}:{password}").as_bytes())
    )
}

/// `<root>/remote.php/dav/files/<user>/<path>`, each part percent-encoded.
pub fn dav_path(root: &str, user: &str, path: &str) -> String {
    let mut out = format!("{root}/remote.php/dav/files/{}", encode_part(user));
    for part in path.split('/').filter(|p| !p.is_empty()) {
        out.push('/');
        out.push_str(&encode_part(part));
    }
    out.push('/');
    out
}

fn encode_part(part: &str) -> String {
    let mut out = String::new();
    for b in part.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn decode_percent(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            if let Some(b) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn decode_entities(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// Split an HTTP/1.x response into its status and its body, undoing
/// `Transfer-Encoding: chunked` when the server used it.
pub fn split_response(raw: &[u8]) -> Option<(u16, Vec<u8>)> {
    let status = crate::signin::status_code(raw)?;
    let end = raw.windows(4).position(|w| w == b"\r\n\r\n")?;
    let head = String::from_utf8_lossy(&raw[..end]).to_ascii_lowercase();
    let body = &raw[end + 4..];
    let chunked = head
        .lines()
        .any(|l| l.starts_with("transfer-encoding:") && l.contains("chunked"));
    Some((
        status,
        if chunked {
            dechunk(body)?
        } else {
            body.to_vec()
        },
    ))
}

fn dechunk(mut body: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    loop {
        let line_end = body.windows(2).position(|w| w == b"\r\n")?;
        let size_text = std::str::from_utf8(&body[..line_end]).ok()?;
        let size = usize::from_str_radix(size_text.split(';').next()?.trim(), 16).ok()?;
        body = &body[line_end + 2..];
        if size == 0 {
            return Some(out);
        }
        out.extend_from_slice(body.get(..size)?);
        body = body.get(size + 2..)?;
    }
}

/// The app password out of `getapppassword`'s OCS answer.
pub fn parse_minted(body: &[u8]) -> Option<String> {
    let doc: Value = serde_json::from_slice(body).ok()?;
    let key = doc.pointer("/ocs/data/apppassword")?.as_str()?;
    (!key.is_empty() && !key.chars().any(char::is_control)).then(|| key.to_string())
}

/// One row of a folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Entry {
    pub name: String,
    /// From the top of LosOS cloud, `/Documents/plan.pdf`.
    pub path: String,
    pub id: u64,
    pub folder: bool,
    pub size: Option<u64>,
}

/// The inner text of every element named `local` (any namespace prefix).
fn elements<'a>(xml: &'a str, local: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find('<') {
        rest = &rest[start + 1..];
        let Some(close) = rest.find('>') else { break };
        let tag = &rest[..close];
        let name = tag
            .split(|c: char| c.is_whitespace() || c == '/')
            .next()
            .unwrap_or("");
        let bare = name.rsplit(':').next().unwrap_or("");
        if tag.starts_with('/') || bare != local {
            continue;
        }
        let after = &rest[close + 1..];
        if tag.ends_with('/') {
            out.push("");
            rest = after;
            continue;
        }
        let end_tag = format!("</{name}>");
        let Some(end) = after.find(&end_tag) else {
            break;
        };
        out.push(&after[..end]);
        rest = &after[end + end_tag.len()..];
    }
    out
}

/// Read a WebDAV multistatus for the folder at `path`: every entry but the
/// folder itself, folders first, then by name.
pub fn parse_multistatus(xml: &str, user: &str, path: &str) -> Vec<Entry> {
    let marker = format!("/remote.php/dav/files/{user}/");
    let own = path.trim_matches('/');
    let mut rows: Vec<Entry> = elements(xml, "response")
        .into_iter()
        .filter_map(|response| {
            let href = decode_percent(&decode_entities(elements(response, "href").first()?.trim()));
            let at = href.find(&marker)?;
            let inner = href[at + marker.len()..].trim_matches('/').to_string();
            if inner == own {
                return None;
            }
            let id = elements(response, "fileid")
                .into_iter()
                .find_map(|v| v.trim().parse::<u64>().ok())?;
            let folder = elements(response, "resourcetype")
                .first()
                .is_some_and(|t| !elements(t, "collection").is_empty());
            let size = elements(response, "size")
                .into_iter()
                .find_map(|v| v.trim().parse::<u64>().ok());
            let name = inner.rsplit('/').next().unwrap_or(&inner).to_string();
            if name.is_empty() {
                return None;
            }
            Some(Entry {
                name,
                path: format!("/{inner}"),
                id,
                folder,
                size,
            })
        })
        .collect();
    rows.sort_by(|a, b| {
        b.folder
            .cmp(&a.folder)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    rows
}

/// Ask LosOS cloud for an app password with the owner's password, unless
/// lososd holds one already. Best effort, like the LosOS Git password sync
/// beside it: a sign-in LosOS cloud accepted is never refused for this.
pub fn keep_cloud_key<L: Losos>(l: &mut L, user: &str, password: &Secret) {
    match l.load_cloud_key() {
        Ok(Some(key)) if key.user == user => return,
        Ok(_) => {}
        Err(e) => {
            tracing::warn!(error = ?e, "could not read the LosOS cloud key");
            return;
        }
    }
    let answer = l.cloud_request(&CloudRequest::MintKey { user, password });
    let minted = match answer {
        Ok((200, body)) => parse_minted(&body),
        Ok((status, _)) => {
            tracing::info!(status, "LosOS cloud did not hand out an app password");
            None
        }
        Err(e) => {
            tracing::info!(error = ?e, "LosOS cloud could not be asked for an app password");
            None
        }
    };
    if let Some(key) = minted {
        let key = CloudKey {
            user: user.to_string(),
            key: Secret::new(&key),
        };
        match l.save_cloud_key(Some(&key)) {
            Ok(()) => tracing::info!(user, "kept an app password for listing LosOS cloud"),
            Err(e) => tracing::warn!(error = ?e, "could not keep the LosOS cloud key"),
        }
    }
}

/// `GET /api/cloud/files?path=/...`: one folder. With `q=...` instead, the
/// files and folders anywhere whose name holds the text (`path` is then
/// `/`, where the results are relative to).
pub fn cmd_cloud_files<L: Losos>(
    l: &mut L,
    path: &str,
    query: Option<&str>,
) -> anyhow::Result<Value> {
    let query = validate_query(query)?;
    let path = if query.is_some() { "/" } else { path };
    validate_path(path)?;
    let unavailable = |reason: &str| {
        Ok(json!({
            "available": false,
            "reason": reason,
            "path": path,
            "query": query,
            "entries": [],
        }))
    };
    let Some(key) = l.load_cloud_key()? else {
        return unavailable("signIn");
    };
    let request = match &query {
        Some(q) => CloudRequest::Search {
            key: &key,
            query: q,
        },
        None => CloudRequest::List { key: &key, path },
    };
    let (status, body) = match l.cloud_request(&request) {
        Ok(answer) => answer,
        Err(e) => {
            tracing::info!(error = ?e, "LosOS cloud could not be asked for a folder");
            return unavailable("notUp");
        }
    };
    match status {
        207 => {
            let text = String::from_utf8_lossy(&body);
            let mut entries = parse_multistatus(&text, &key.user, path);
            let more = entries.len() > MAX_ENTRIES;
            entries.truncate(MAX_ENTRIES);
            Ok(json!({
                "available": true,
                "path": path,
                "query": query,
                "entries": entries,
                "more": more,
            }))
        }
        // Revoked in LosOS cloud's security settings, or the account is new:
        // forget it, and the next sign-in with the password asks for another.
        401 => {
            l.save_cloud_key(None)?;
            unavailable("signIn")
        }
        404 => invalid("That folder is not in LosOS cloud any more."),
        _ => unavailable("notUp"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::FakeLosos;

    fn file(id: u64, path: &str) -> Pin {
        Pin::File {
            id,
            path: path.into(),
            folder: false,
        }
    }

    #[test]
    fn a_list_round_trips_in_the_owners_order() {
        let mut f = FakeLosos::new();
        let pins = vec![
            Pin::Repo {
                owner: "notshared".into(),
                name: "site".into(),
            },
            Pin::Tile {
                app: "calendar".into(),
            },
            file(42, "/Documents/plan.pdf"),
            Pin::App {
                release: "jellyfin".into(),
            },
        ];
        let out = cmd_put_pins(&mut f, &pins).unwrap();
        assert_eq!(out["limit"], MAX_PINS);
        assert_eq!(out["pins"][0]["kind"], "repo");
        assert_eq!(
            out["pins"][2],
            json!({"kind":"file","id":42,"path":"/Documents/plan.pdf","folder":false})
        );
        assert_eq!(cmd_pins(&mut f).unwrap(), out);
        assert_eq!(parse_pins(&out).unwrap(), pins);
    }

    #[test]
    fn duplicates_bad_fields_and_too_many_are_refused_and_nothing_is_written() {
        let mut f = FakeLosos::new();
        let refuse = |f: &mut FakeLosos, pins: &[Pin]| {
            let e = cmd_put_pins(f, pins).unwrap_err();
            assert!(e.downcast_ref::<Invalid>().is_some(), "untyped: {e}");
        };
        refuse(&mut f, &[file(1, "/a"), file(1, "/b")]);
        refuse(&mut f, &[file(0, "/a")]);
        refuse(&mut f, &[file(1, "/")]);
        refuse(&mut f, &[file(1, "/a/../etc")]);
        refuse(&mut f, &[file(1, "relative")]);
        refuse(
            &mut f,
            &[Pin::Tile {
                app: "Files".into(),
            }],
        );
        refuse(
            &mut f,
            &[Pin::App {
                release: "Bad Name".into(),
            }],
        );
        refuse(
            &mut f,
            &[Pin::Repo {
                owner: "..".into(),
                name: "x".into(),
            }],
        );
        let many: Vec<Pin> = (1..=(MAX_PINS as u64 + 1)).map(|i| file(i, "/a")).collect();
        refuse(&mut f, &many);
        assert!(cmd_pins(&mut f).unwrap()["pins"]
            .as_array()
            .unwrap()
            .is_empty());
    }

    #[test]
    fn a_pin_with_a_field_of_its_own_choosing_is_not_a_pin() {
        // No href, no label: a stored pin cannot carry an address.
        let doc =
            json!({"pins": [{"kind": "repo", "owner": "a", "name": "b", "href": "https://x"}]});
        assert!(parse_pins(&doc).is_err());
        assert!(parse_pins(&json!({"pins": [{"kind": "link"}]})).is_err());
        assert!(parse_pins(&json!({})).is_err());
    }

    #[test]
    fn the_cloud_endpoint_follows_the_sign_in_probe() {
        let login =
            crate::signin::parse_login_url("http://127.0.0.1:8080/nextcloud/ocs/v2.php/cloud/user")
                .unwrap();
        let e = CloudEndpoint::from_login(&login, "localhost");
        assert_eq!(e.root, "/nextcloud");
        let native =
            crate::signin::parse_login_url("http://127.0.0.1:80/ocs/v2.php/cloud/user").unwrap();
        assert_eq!(CloudEndpoint::from_login(&native, "box.local").root, "");
    }

    #[test]
    fn a_listing_is_one_propfind_with_the_app_password() {
        let e = CloudEndpoint {
            addr: "127.0.0.1:8080".into(),
            host: "localhost".into(),
            root: "/nextcloud".into(),
        };
        let key = CloudKey {
            user: "notshared".into(),
            key: Secret::new("k"),
        };
        let bytes = request_bytes(
            &e,
            &CloudRequest::List {
                key: &key,
                path: "/My Files/ü",
            },
        );
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.starts_with(
            "PROPFIND /nextcloud/remote.php/dav/files/notshared/My%20Files/%C3%BC/ HTTP/1.1\r\n"
        ));
        assert!(text.contains(&format!(
            "Authorization: Basic {}\r\n",
            crate::signin::base64(b"notshared:k")
        )));
        assert!(text.contains("Depth: 1\r\n"));
        assert!(text.ends_with("</d:propfind>"));
        let len = PROPFIND_BODY.len();
        assert!(text.contains(&format!("Content-Length: {len}\r\n")));
    }

    #[test]
    fn minting_trades_the_password_for_a_named_app_password() {
        let e = CloudEndpoint {
            addr: "127.0.0.1:80".into(),
            host: "box.local".into(),
            root: String::new(),
        };
        let pw = Secret::new("Correct-Horse-1");
        let text = String::from_utf8(request_bytes(
            &e,
            &CloudRequest::MintKey {
                user: "notshared",
                password: &pw,
            },
        ))
        .unwrap();
        assert!(text.starts_with("GET /ocs/v2.php/core/getapppassword HTTP/1.1\r\n"));
        assert!(text.contains("OCS-APIRequest: true\r\n"));
        assert!(text.contains(&format!("User-Agent: {KEY_CLIENT}\r\n")));
        let body = br#"{"ocs":{"meta":{"status":"ok"},"data":{"apppassword":"abc-def"}}}"#;
        assert_eq!(parse_minted(body).as_deref(), Some("abc-def"));
        assert_eq!(parse_minted(b"{}"), None);
    }

    #[test]
    fn chunked_and_plain_bodies_both_read() {
        let plain = b"HTTP/1.1 207 Multi-Status\r\nContent-Length: 2\r\n\r\nok";
        assert_eq!(split_response(plain), Some((207, b"ok".to_vec())));
        let chunked =
            b"HTTP/1.1 207 Multi-Status\r\nTransfer-Encoding: chunked\r\n\r\n4\r\nWiki\r\n5;x=y\r\npedia\r\n0\r\n\r\n";
        assert_eq!(split_response(chunked), Some((207, b"Wikipedia".to_vec())));
        assert_eq!(split_response(b"garbage"), None);
    }

    const MULTISTATUS: &str = r#"<?xml version="1.0"?>
<d:multistatus xmlns:d="DAV:" xmlns:s="http://sabredav.org/ns" xmlns:oc="http://owncloud.org/ns">
 <d:response><d:href>/nextcloud/remote.php/dav/files/notshared/Documents/</d:href>
  <d:propstat><d:prop><d:resourcetype><d:collection/></d:resourcetype><oc:fileid>10</oc:fileid><oc:size>300</oc:size></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>
 <d:response><d:href>/nextcloud/remote.php/dav/files/notshared/Documents/zeta.pdf</d:href>
  <d:propstat><d:prop><d:resourcetype/><oc:fileid>12</oc:fileid><oc:size>100</oc:size></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>
 <d:response><d:href>/nextcloud/remote.php/dav/files/notshared/Documents/Tax%20%26%20bills/</d:href>
  <d:propstat><d:prop><d:resourcetype><d:collection/></d:resourcetype><oc:fileid>11</oc:fileid><oc:size>200</oc:size></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>
 <d:response><d:href>/nextcloud/remote.php/dav/files/notshared/Documents/alpha.md</d:href>
  <d:propstat><d:prop><d:resourcetype/><oc:fileid>13</oc:fileid></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat>
  <d:propstat><d:prop><oc:size/></d:prop><d:status>HTTP/1.1 404 Not Found</d:status></d:propstat></d:response>
</d:multistatus>"#;

    #[test]
    fn a_folder_reads_as_its_entries_folders_first() {
        let rows = parse_multistatus(MULTISTATUS, "notshared", "/Documents");
        let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["Tax & bills", "alpha.md", "zeta.pdf"]);
        assert_eq!(rows[0].path, "/Documents/Tax & bills");
        assert!(rows[0].folder);
        assert_eq!(rows[0].id, 11);
        assert_eq!(rows[1].size, None);
        assert_eq!(rows[2].size, Some(100));
    }

    #[test]
    fn the_folder_list_needs_a_key_and_forgets_a_revoked_one() {
        let mut f = FakeLosos::new();
        assert_eq!(
            cmd_cloud_files(&mut f, "/", None).unwrap()["reason"],
            "signIn"
        );

        // A sign-in with the password mints one; a second keeps it.
        f.cloud_answers.push_back((
            200,
            br#"{"ocs":{"data":{"apppassword":"minted"}}}"#.to_vec(),
        ));
        keep_cloud_key(&mut f, "notshared", &Secret::new("pw"));
        keep_cloud_key(&mut f, "notshared", &Secret::new("pw"));
        assert_eq!(f.cloud_calls, ["mint notshared"]);

        f.cloud_answers
            .push_back((207, MULTISTATUS.as_bytes().to_vec()));
        let out = cmd_cloud_files(&mut f, "/Documents", None).unwrap();
        assert_eq!(out["available"], true);
        assert_eq!(out["entries"].as_array().unwrap().len(), 3);
        assert_eq!(f.cloud_calls[1], "list notshared:minted /Documents");

        f.cloud_answers.push_back((401, Vec::new()));
        assert_eq!(
            cmd_cloud_files(&mut f, "/", None).unwrap()["reason"],
            "signIn"
        );
        assert!(f.cloud_key.is_none());

        assert!(cmd_cloud_files(&mut f, "/../x", None).is_err());
    }

    #[test]
    fn a_search_asks_for_names_anywhere_and_reads_like_a_folder() {
        let mut f = FakeLosos::new();
        f.cloud_key = Some(CloudKey {
            user: "notshared".into(),
            key: Secret::new("k"),
        });
        f.cloud_answers
            .push_back((207, MULTISTATUS.as_bytes().to_vec()));
        let out = cmd_cloud_files(&mut f, "/ignored", Some(" tax ")).unwrap();
        assert_eq!(out["query"], "tax");
        assert_eq!(out["path"], "/");
        assert_eq!(f.cloud_calls, ["search notshared tax"]);
        // Relative to the top, the folder the hits are in is part of them.
        assert_eq!(out["entries"][0]["path"], "/Documents");
        assert_eq!(out["entries"][1]["path"], "/Documents/Tax & bills");

        let long = "x".repeat(MAX_QUERY_CHARS + 1);
        assert!(cmd_cloud_files(&mut f, "/", Some(&long)).is_err());
        assert!(cmd_cloud_files(&mut f, "/", Some("a\nb")).is_err());
    }

    #[test]
    fn the_search_body_escapes_the_text_and_drops_wildcards() {
        let e = CloudEndpoint {
            addr: "127.0.0.1:8080".into(),
            host: "localhost".into(),
            root: "/nextcloud".into(),
        };
        let key = CloudKey {
            user: "notshared".into(),
            key: Secret::new("k"),
        };
        let text = String::from_utf8(request_bytes(
            &e,
            &CloudRequest::Search {
                key: &key,
                query: "a<b>%&",
            },
        ))
        .unwrap();
        assert!(text.starts_with("SEARCH /nextcloud/remote.php/dav/ HTTP/1.1\r\n"));
        assert!(text.contains("<d:literal>%a&lt;b&gt;&amp;%</d:literal>"));
        assert!(text.contains("<d:href>/files/notshared</d:href>"));
        let body = text.split("\r\n\r\n").nth(1).unwrap();
        assert!(text.contains(&format!("Content-Length: {}\r\n", body.len())));
    }

    #[test]
    fn repositories_come_from_the_owners_account_on_losos_git() {
        let mut f = FakeLosos::new();
        assert_eq!(cmd_git_repos(&mut f, None).unwrap()["reason"], "noGit");
        let mut f = FakeLosos::new().with_config_repo();
        assert_eq!(cmd_git_repos(&mut f, None).unwrap()["reason"], "notUp");
        f.git_routes.insert(
            "GET /api/v1/users/notshared/repos".to_string(),
            (
                200,
                r#"[{"name":"site","owner":{"login":"notshared"},"private":false,"description":"web"},
                   {"name":"losos-config","owner":{"login":"notshared"},"private":true}]"#
                    .to_string(),
            ),
        );
        let out = cmd_git_repos(&mut f, None).unwrap();
        assert_eq!(out["available"], true);
        assert_eq!(out["repos"][0]["name"], "losos-config");
        assert_eq!(out["repos"][0]["private"], true);
        assert_eq!(out["repos"][1]["description"], "web");
        let found = cmd_git_repos(&mut f, Some("WEB")).unwrap();
        assert_eq!(found["repos"].as_array().unwrap().len(), 1);
        assert_eq!(found["repos"][0]["name"], "site");
    }
}
