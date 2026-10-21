//! The two lists the sidebar's pin picker offers, each one searchable so
//! the page can look things up by name. The pins themselves, and what makes
//! one, are the pins plugin's (`plugins/pins/`); what the picker offers is
//! still lososd's, because it speaks to LosOS Cloud and LosOS Git in ways
//! only lososd may.
//!
//! - a folder of LosOS Cloud, or the files whose name matches, from
//!   anywhere in it;
//! - the owner's repositories on LosOS Git.
//!
//! - **LosOS Git** is asked as the bot administrator lososd already talks
//!   to it as (`crate::config_repo`), for the owner's repositories,
//!   private ones included.
//! - **LosOS Cloud** is asked over WebDAV on its socket, naming the owner
//!   in the sign-in header the way nginx does for a signed-in person
//!   (`modules/cloud.nix`): root can open the socket, and nothing else but
//!   nginx can.
//!
//! Everything here is pure: the request bytes, the WebDAV answer, the
//! chunked body. The socket is [`crate::losos::Losos::cloud_request`].

use crate::losos::Losos;
use serde::Serialize;
use serde_json::{json, Value};

/// Most entries one folder listing returns; the rest are left out and the
/// answer says so.
pub const MAX_ENTRIES: usize = 500;
/// Most repositories one listing returns (Forgejo's page size).
pub const MAX_REPOS: usize = 50;
/// Most results one search of LosOS Cloud returns.
pub const MAX_HITS: usize = 50;
/// Longest search text, in characters.
pub const MAX_QUERY_CHARS: usize = 100;
/// The client name lososd gives LosOS Cloud.
pub const KEY_CLIENT: &str = "LosOS admin page";

/// A request the owner can fix by sending a different one; the HTTP layer
/// answers it as a 400 carrying the sentence.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct Invalid(pub String);

fn invalid<T>(message: impl Into<String>) -> anyhow::Result<T> {
    Err(anyhow::Error::new(Invalid(message.into())))
}

/// A path inside LosOS Cloud: from the top (`/`), no empty, `.` or `..`
/// part, nothing that is not printable.
pub fn validate_path(path: &str) -> anyhow::Result<()> {
    if losos_names::cloud_path(path) {
        Ok(())
    } else {
        invalid("That is not a folder in LosOS Cloud.")
    }
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
    use crate::config_repo::{classify, Answer, ForgejoOp, NotUp};
    let unavailable =
        |reason: &str| Ok(json!({ "available": false, "reason": reason, "repos": [] }));
    let Some(repo) = l.config_repo() else {
        return unavailable("noGit");
    };
    if !repo.via_forgejo() {
        return unavailable("noGit");
    }
    let op = ForgejoOp::ListRepos {
        login: crate::setup::DEFAULT_ADMIN_USER.to_string(),
    };
    let (status, body) = match l.forgejo_request(&op, None) {
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

/// The rows of a Forgejo repository list, sorted by name.
pub fn parse_repos(doc: &Value) -> Vec<Value> {
    let mut rows: Vec<Value> = doc
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .filter_map(|r| {
            let name = r.get("name")?.as_str()?;
            let owner = r.get("owner")?.get("login")?.as_str()?;
            if !losos_names::repo_part(owner) || !losos_names::repo_part(name) {
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

// ── LosOS Cloud: a folder ─────────────────────────────────────────────────

/// One request to LosOS Cloud, as the person it names.
#[derive(Debug)]
pub enum CloudRequest<'a> {
    /// A new app password for `user`'s computer named `device`.
    MintKey { user: &'a str, device: &'a str },
    /// One folder, one level deep.
    List { user: &'a str, path: &'a str },
    /// Files and folders anywhere whose name holds `query`.
    Search { user: &'a str, query: &'a str },
    /// Sign `user` in once, so LosOS Cloud makes their account if it has
    /// none yet (`crate::cloud_share`).
    Arrive {
        user: &'a str,
        display_name: &'a str,
    },
    /// `user`'s contacts that match `query`, for a people picker
    /// (`crate::contacts`): `want` is `any`, `email`, `matrix` or `mesh`.
    ContactMatches {
        user: &'a str,
        query: &'a str,
        want: &'a str,
    },
}

const PROPFIND_BODY: &str = concat!(
    r#"<?xml version="1.0"?>"#,
    r#"<d:propfind xmlns:d="DAV:" xmlns:oc="http://owncloud.org/ns">"#,
    "<d:prop><d:resourcetype/><oc:fileid/><oc:size/></d:prop>",
    "</d:propfind>",
);

/// The bytes of one request. HTTP/1.1 with `Connection: close`, so the
/// answer ends where the socket does. Every request names its person in
/// the sign-in header (`modules/app-sign-in.nix`), with anything that could
/// end the line removed.
pub fn request_bytes(request: &CloudRequest<'_>) -> Vec<u8> {
    let clean = |v: &str| v.chars().filter(|c| !c.is_control()).collect::<String>();
    let person = |user: &str| format!("X-Losos-Cloud-User: {}\r\n", clean(user));
    let mut out = String::new();
    match request {
        CloudRequest::MintKey { user, device } => {
            out.push_str("GET /ocs/v2.php/core/getapppassword HTTP/1.1\r\n");
            out.push_str("Host: localhost\r\n");
            out.push_str(&person(user));
            out.push_str("OCS-APIRequest: true\r\n");
            out.push_str("Accept: application/json\r\n");
            // LosOS Cloud names the app password after the client.
            out.push_str(&format!("User-Agent: {}\r\n", clean(device)));
            out.push_str("Connection: close\r\n\r\n");
        }
        CloudRequest::List { user, path } => {
            out.push_str(&format!("PROPFIND {} HTTP/1.1\r\n", dav_path(user, path)));
            out.push_str("Host: localhost\r\n");
            out.push_str(&person(user));
            out.push_str("Depth: 1\r\n");
            out.push_str("Content-Type: application/xml; charset=utf-8\r\n");
            out.push_str(&format!("Content-Length: {}\r\n", PROPFIND_BODY.len()));
            out.push_str(&format!("User-Agent: {KEY_CLIENT}\r\n"));
            out.push_str("Connection: close\r\n\r\n");
            out.push_str(PROPFIND_BODY);
        }
        CloudRequest::Search { user, query } => {
            let body = search_body(user, query);
            out.push_str("SEARCH /remote.php/dav/ HTTP/1.1\r\n");
            out.push_str("Host: localhost\r\n");
            out.push_str(&person(user));
            out.push_str("Content-Type: text/xml; charset=utf-8\r\n");
            out.push_str(&format!("Content-Length: {}\r\n", body.len()));
            out.push_str(&format!("User-Agent: {KEY_CLIENT}\r\n"));
            out.push_str("Connection: close\r\n\r\n");
            out.push_str(&body);
        }
        CloudRequest::Arrive { user, display_name } => {
            out.push_str("GET /ocs/v2.php/cloud/user HTTP/1.1\r\n");
            out.push_str("Host: localhost\r\n");
            out.push_str(&person(user));
            out.push_str(&format!("X-Losos-Cloud-Name: {}\r\n", clean(display_name)));
            out.push_str("OCS-APIRequest: true\r\n");
            out.push_str(&format!("User-Agent: {KEY_CLIENT}\r\n"));
            out.push_str("Connection: close\r\n\r\n");
        }
        CloudRequest::ContactMatches { user, query, want } => {
            let q = url::form_urlencoded::Serializer::new(String::new())
                .append_pair("q", query)
                .append_pair("want", want)
                .finish();
            out.push_str(&format!("GET /cloud/api/contacts/suggest?{q} HTTP/1.1\r\n"));
            out.push_str("Host: localhost\r\n");
            out.push_str(&person(user));
            out.push_str("Accept: application/json\r\n");
            out.push_str(&format!("User-Agent: {KEY_CLIENT}\r\n"));
            out.push_str("Connection: close\r\n\r\n");
        }
    }
    out.into_bytes()
}

/// A WebDAV SEARCH (RFC 5323, as LosOS Cloud speaks it) for names that
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

/// `/remote.php/dav/files/<user>/<path>`, each part percent-encoded.
pub fn dav_path(user: &str, path: &str) -> String {
    let mut out = format!("/remote.php/dav/files/{}", encode_part(user));
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

/// One row of a folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Entry {
    pub name: String,
    /// From the top of LosOS Cloud, `/Documents/plan.pdf`.
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
    let user = crate::setup::DEFAULT_ADMIN_USER;
    let request = match &query {
        Some(q) => CloudRequest::Search { user, query: q },
        None => CloudRequest::List { user, path },
    };
    let (status, body) = match l.cloud_request(&request) {
        Ok(answer) => answer,
        Err(e) => {
            tracing::info!(error = ?e, "LosOS Cloud could not be asked for a folder");
            return unavailable("notUp");
        }
    };
    match status {
        207 => {
            let text = String::from_utf8_lossy(&body);
            let mut entries = parse_multistatus(&text, user, path);
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
        404 => invalid("That folder is not in LosOS Cloud any more."),
        _ => unavailable("notUp"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::FakeLosos;

    #[test]
    fn a_listing_is_one_propfind_as_the_owner() {
        let bytes = request_bytes(&CloudRequest::List {
            user: "notshared",
            path: "/My Files/ü",
        });
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.starts_with(
            "PROPFIND /remote.php/dav/files/notshared/My%20Files/%C3%BC/ HTTP/1.1\r\n"
        ));
        assert!(text.contains("X-Losos-Cloud-User: notshared\r\n"));
        assert!(!text.contains("Authorization"));
        assert!(text.contains("Depth: 1\r\n"));
        assert!(text.ends_with("</d:propfind>"));
        let len = PROPFIND_BODY.len();
        assert!(text.contains(&format!("Content-Length: {len}\r\n")));
    }

    #[test]
    fn minting_names_the_app_password_after_the_computer() {
        let text = String::from_utf8(request_bytes(&CloudRequest::MintKey {
            user: "notshared",
            device: "LosOS terminal on laptop\r\nX-Evil: 1",
        }))
        .unwrap();
        assert!(text.starts_with("GET /ocs/v2.php/core/getapppassword HTTP/1.1\r\n"));
        assert!(text.contains("OCS-APIRequest: true\r\n"));
        assert!(text.contains("User-Agent: LosOS terminal on laptopX-Evil: 1\r\n"));
        assert!(!text.contains("\r\nX-Evil"));
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
 <d:response><d:href>/remote.php/dav/files/notshared/Documents/</d:href>
  <d:propstat><d:prop><d:resourcetype><d:collection/></d:resourcetype><oc:fileid>10</oc:fileid><oc:size>300</oc:size></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>
 <d:response><d:href>/remote.php/dav/files/notshared/Documents/zeta.pdf</d:href>
  <d:propstat><d:prop><d:resourcetype/><oc:fileid>12</oc:fileid><oc:size>100</oc:size></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>
 <d:response><d:href>/remote.php/dav/files/notshared/Documents/Tax%20%26%20bills/</d:href>
  <d:propstat><d:prop><d:resourcetype><d:collection/></d:resourcetype><oc:fileid>11</oc:fileid><oc:size>200</oc:size></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>
 <d:response><d:href>/remote.php/dav/files/notshared/Documents/alpha.md</d:href>
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
    fn a_folder_is_listed_as_the_owner_and_a_cloud_that_is_down_says_so() {
        let mut f = FakeLosos::new();
        f.cloud_answers
            .push_back((207, MULTISTATUS.as_bytes().to_vec()));
        let out = cmd_cloud_files(&mut f, "/Documents", None).unwrap();
        assert_eq!(out["available"], true);
        assert_eq!(out["entries"].as_array().unwrap().len(), 3);
        assert_eq!(f.cloud_calls, ["list notshared /Documents"]);

        // Nothing queued: the fake is not answering.
        assert_eq!(
            cmd_cloud_files(&mut f, "/", None).unwrap()["reason"],
            "notUp"
        );
        f.cloud_answers.push_back((404, Vec::new()));
        assert!(cmd_cloud_files(&mut f, "/Gone", None).is_err());
        assert!(cmd_cloud_files(&mut f, "/../x", None).is_err());
    }

    #[test]
    fn a_search_asks_for_names_anywhere_and_reads_like_a_folder() {
        let mut f = FakeLosos::new();
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
        let text = String::from_utf8(request_bytes(&CloudRequest::Search {
            user: "notshared",
            query: "a<b>%&",
        }))
        .unwrap();
        assert!(text.starts_with("SEARCH /remote.php/dav/ HTTP/1.1\r\n"));
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
        f.forgejo_routes.insert(
            "GET /api/v1/users/notshared/repos?limit=50".to_string(),
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
