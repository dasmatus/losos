//! Collabora's discovery document: which file types it opens, and where.
//!
//! coolwsd answers `GET /hosting/discovery` with XML listing, per file
//! extension, the actions it offers (`edit`, `view`, `view_comment`, …) and
//! the address of its editor (`urlsrc`). The address carries whatever host
//! the request came in on, which for a fetch over loopback is
//! `127.0.0.1:9980`; only the path is kept, and the browser opens it on the
//! box's own origin, where nginx routes `/browser/` to coolwsd.
//!
//! The document is coolwsd's own output on this box, so a small scanner over
//! its `<action>` tags does instead of an XML parser.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// What Collabora does with one extension.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action {
    /// Whether it edits files of this type, or only shows them.
    pub edit: bool,
    /// The editor's path and query start, ending in `?` or `&`, ready for
    /// `WOPISrc=`.
    pub url_path: String,
    /// Whether coolwsd writes https addresses into the editor page (it does
    /// behind nginx's TLS, `ssl.termination`). The page then works only when
    /// it is itself opened over https.
    pub https: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Discovery {
    actions: BTreeMap<String, Action>,
}

/// Pictures Collabora would show in Draw. A file list shows them itself, so
/// they are not offered as documents.
const PICTURES: &[&str] = &[
    "bmp", "emf", "gif", "jpeg", "jpg", "png", "svg", "tif", "tiff", "webp", "wmf",
];

impl Discovery {
    pub fn parse(xml: &str) -> Self {
        let mut actions: BTreeMap<String, Action> = BTreeMap::new();
        for tag in xml.split("<action").skip(1) {
            let tag = match tag.find('>') {
                Some(end) => &tag[..end],
                None => continue,
            };
            let (Some(name), Some(ext), Some(src)) = (
                attribute(tag, "name"),
                attribute(tag, "ext"),
                attribute(tag, "urlsrc"),
            ) else {
                continue;
            };
            let ext = ext.to_ascii_lowercase();
            if ext.is_empty() || PICTURES.contains(&ext.as_str()) {
                continue;
            }
            let edit = name == "edit";
            if !edit && !name.starts_with("view") {
                continue;
            }
            let Some(url_path) = path_of(&src) else {
                continue;
            };
            let https = src.starts_with("https://");
            // Edit wins over view for an extension listed under both.
            if actions.get(&ext).is_some_and(|a| a.edit) {
                continue;
            }
            actions.insert(
                ext,
                Action {
                    edit,
                    url_path,
                    https,
                },
            );
        }
        Self { actions }
    }

    /// Fetch and parse the discovery document from a plain-http coolwsd on
    /// loopback, e.g. `http://127.0.0.1:9980`, or read it from a file a
    /// copy was saved to (an absolute path). LosOS Cloud has no network at
    /// all, so the box saves coolwsd's answer where the cloud can read it
    /// (modules/office.nix, `losos-office-discovery`).
    pub fn fetch(base: &str) -> std::io::Result<Self> {
        let body = if base.starts_with('/') {
            std::fs::read_to_string(base)?
        } else {
            http_get(base, "/hosting/discovery")?
        };
        let found = Self::parse(&body);
        if found.actions.is_empty() {
            return Err(std::io::Error::other(
                "Collabora's discovery lists no file types",
            ));
        }
        Ok(found)
    }

    pub fn action(&self, ext: &str) -> Option<&Action> {
        self.actions.get(&ext.to_ascii_lowercase())
    }

    /// Every extension Collabora opens, and whether it edits it: what the
    /// file view needs to offer "Open in Office".
    pub fn extensions(&self) -> impl Iterator<Item = (&str, bool)> {
        self.actions.iter().map(|(ext, a)| (ext.as_str(), a.edit))
    }
}

/// A coolwsd on loopback and its discovery document, fetched when first
/// needed and again every [`Coolwsd::REFRESH`], so a Collabora update that
/// moves its editor's address is picked up without a restart.
#[derive(Debug)]
pub struct Coolwsd {
    base: String,
    cached: Mutex<Option<(Instant, Arc<Discovery>)>>,
}

impl Coolwsd {
    pub const REFRESH: Duration = Duration::from_secs(10 * 60);

    /// `base` is coolwsd's plain-http address, e.g. `http://127.0.0.1:9980`,
    /// or the path of a saved copy of its discovery document.
    pub fn new(base: &str) -> Self {
        Self {
            base: base.trim_end_matches('/').to_string(),
            cached: Mutex::new(None),
        }
    }

    pub fn discovery(&self) -> std::io::Result<Arc<Discovery>> {
        let mut cached = self.cached.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((at, d)) = cached.as_ref() {
            if at.elapsed() < Self::REFRESH {
                return Ok(d.clone());
            }
        }
        match Discovery::fetch(&self.base) {
            Ok(d) => {
                let d = Arc::new(d);
                *cached = Some((Instant::now(), d.clone()));
                Ok(d)
            }
            // A restarting coolwsd: the last answer is better than none.
            Err(e) => cached.as_ref().map(|(_, d)| d.clone()).ok_or(e),
        }
    }
}

/// The value of `name="…"` in one tag's attributes, entities decoded.
fn attribute(tag: &str, name: &str) -> Option<String> {
    let mut rest = tag;
    loop {
        let at = rest.find(name)?;
        let before = rest[..at].chars().last();
        let after = &rest[at + name.len()..];
        rest = after;
        if !before.is_some_and(char::is_whitespace) {
            continue;
        }
        let Some(after) = after.trim_start().strip_prefix('=') else {
            continue;
        };
        let after = after.trim_start();
        let quote = after.chars().next()?;
        if quote != '"' && quote != '\'' {
            return None;
        }
        let value = &after[1..];
        let end = value.find(quote)?;
        return Some(unescape(&value[..end]));
    }
}

fn unescape(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// `https://host:port/path?q` → `/path?q`. Only an absolute address whose
/// path is coolwsd's `/browser/` is taken.
fn path_of(url: &str) -> Option<String> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let path = &rest[rest.find('/')?..];
    if !path.starts_with("/browser/") {
        return None;
    }
    let mut path = path.to_string();
    if !path.ends_with('?') && !path.ends_with('&') {
        path.push(if path.contains('?') { '&' } else { '?' });
    }
    Some(path)
}

/// A GET over plain HTTP/1.0 to a loopback service. No client library: the
/// one caller fetches one document from a server on the same machine.
fn http_get(base: &str, path: &str) -> std::io::Result<String> {
    let authority = base
        .strip_prefix("http://")
        .ok_or_else(|| std::io::Error::other("the discovery address must be http://"))?
        .trim_end_matches('/');
    let mut stream = TcpStream::connect(authority)?;
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    stream.set_write_timeout(Some(Duration::from_secs(30)))?;
    write!(
        stream,
        "GET {path} HTTP/1.0\r\nHost: {authority}\r\nConnection: close\r\n\r\n"
    )?;
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw)?;
    let text = String::from_utf8_lossy(&raw);
    let (head, body) = text
        .split_once("\r\n\r\n")
        .ok_or_else(|| std::io::Error::other("no HTTP answer"))?;
    let status = head.split_whitespace().nth(1).unwrap_or_default();
    if status != "200" {
        return Err(std::io::Error::other(format!("{path} answered {status}")));
    }
    Ok(body.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Trimmed from coolwsd 25.04's own answer.
    const XML: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<wopi-discovery>
    <net-zone name="external-http">
        <app favIconUrl="http://127.0.0.1:9980/browser/f00d/images/x-office-document.svg" name="application/vnd.oasis.opendocument.text">
            <action default="true" ext="" name="edit" urlsrc="http://127.0.0.1:9980/browser/f00d/cool.html?"/>
        </app>
        <app name="writer">
            <action default="true" ext="odt" name="edit" urlsrc="http://127.0.0.1:9980/browser/f00d/cool.html?"/>
            <action default="true" ext="DOCX" name="edit" urlsrc="http://127.0.0.1:9980/browser/f00d/cool.html?"/>
            <action ext="doc" name="view_comment" urlsrc="http://127.0.0.1:9980/browser/f00d/cool.html?"/>
        </app>
        <app name="draw">
            <action ext="pdf" name="view" urlsrc="http://127.0.0.1:9980/browser/f00d/cool.html?"/>
            <action ext="pdf" name="edit" urlsrc="http://127.0.0.1:9980/browser/f00d/cool.html?"/>
            <action ext="vsd" name="view" urlsrc='https://box.local/browser/f00d/cool.html?a=1&amp;b=2'/>
            <action ext="svg" name="convert" urlsrc="http://127.0.0.1:9980/browser/f00d/cool.html?"/>
            <action ext="png" name="view" urlsrc="http://127.0.0.1:9980/browser/f00d/cool.html?"/>
            <action ext="exe" name="edit" urlsrc="http://evil.example/steal?"/>
        </app>
    </net-zone>
    <proof-key value="x" modulus="y" exponent="AQAB"/>
</wopi-discovery>"#;

    #[test]
    fn extensions_map_to_the_editor_path() {
        let d = Discovery::parse(XML);
        let docx = d.action("docx").unwrap();
        assert!(docx.edit);
        assert_eq!(docx.url_path, "/browser/f00d/cool.html?");
        assert!(!docx.https);
        assert!(d.action("vsd").unwrap().https);
        assert_eq!(d.action("DOCX"), d.action("docx"));
        assert!(!d.action("doc").unwrap().edit);
        assert!(d.action("pdf").unwrap().edit, "edit wins over view");
        assert_eq!(
            d.action("vsd").unwrap().url_path,
            "/browser/f00d/cool.html?a=1&b=2&"
        );
    }

    #[test]
    fn mime_rows_other_actions_and_foreign_paths_are_skipped() {
        let d = Discovery::parse(XML);
        let exts: Vec<_> = d.extensions().map(|(e, _)| e).collect();
        assert_eq!(exts, ["doc", "docx", "odt", "pdf", "vsd"]);
    }

    #[test]
    fn attributes_need_a_word_boundary() {
        let tag = r#" default="true" ext="odt" name="edit" urlsrc="u""#;
        assert_eq!(attribute(tag, "ext").as_deref(), Some("odt"));
        assert_eq!(attribute(tag, "src"), None);
        assert_eq!(attribute(tag, "name").as_deref(), Some("edit"));
    }

    #[test]
    fn fetch_reads_a_saved_copy() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("discovery.xml");
        assert!(Discovery::fetch(path.to_str().unwrap()).is_err());
        std::fs::write(&path, XML).unwrap();
        let d = Discovery::fetch(path.to_str().unwrap()).unwrap();
        assert!(d.action("docx").is_some());
    }

    #[test]
    fn fetch_reads_a_loopback_server() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            // Read the complete request headers before replying. A single TCP
            // read is not guaranteed to contain the whole request, and closing
            // the socket with unread request bytes can reset the client while
            // it is reading the response.
            let mut req = Vec::new();
            loop {
                let mut buf = [0u8; 256];
                let n = s.read(&mut buf).unwrap();
                if n == 0 {
                    break;
                }
                req.extend_from_slice(&buf[..n]);
                if req.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            let req = String::from_utf8_lossy(&req).to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{XML}",
                XML.len()
            );
            s.write_all(response.as_bytes()).unwrap();
            req
        });
        let d = Discovery::fetch(&format!("http://{addr}")).unwrap();
        assert!(d.action("odt").is_some());
        assert!(server
            .join()
            .unwrap()
            .starts_with("GET /hosting/discovery HTTP/1.0"));
    }
}
