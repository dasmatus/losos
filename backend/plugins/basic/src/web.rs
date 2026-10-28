//! The plumbing of a page with no script: form bodies, answers that are
//! HTML or a redirect, the page's cookies, and the check that a form was
//! sent from this box's own pages.

use crate::host::Forwarded;
use losos_page::html::Html;
use losos_plugin::{Request, Response};

/// `application/x-www-form-urlencoded`, decoded. Later values of a name
/// come after earlier ones.
#[must_use]
pub fn form(body: &[u8]) -> Vec<(String, String)> {
    String::from_utf8_lossy(body)
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|pair| {
            let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
            (decode(k), decode(v))
        })
        .collect()
}

/// The last value of `name` in a form.
#[must_use]
pub fn field<'a>(form: &'a [(String, String)], name: &str) -> Option<&'a str> {
    form.iter()
        .rev()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
}

/// One percent-encoded value, `+` as a space.
#[must_use]
pub fn decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => match (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                (Some(hi), Some(lo)) => {
                    out.push(hi * 16 + lo);
                    i += 2;
                }
                _ => out.push(b'%'),
            },
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex(b: u8) -> Option<u8> {
    (b as char).to_digit(16).and_then(|d| u8::try_from(d).ok())
}

/// A query string's `name`, decoded.
#[must_use]
pub fn query(req: &Request, name: &str) -> Option<String> {
    form(req.query.as_bytes())
        .into_iter()
        .rev()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v)
}

/// A page. Never cached, never framed, never sniffed: nginx adds the
/// policy, these are for a box reached past it.
#[must_use]
pub fn page(status: u16, html: Html) -> Response {
    Response::bytes(
        status,
        "text/html; charset=utf-8",
        html.into_string().into_bytes(),
    )
    .header("cache-control", "no-store")
    .header("x-content-type-options", "nosniff")
}

/// After a form: back to a page with GET, so a reload sends nothing again.
#[must_use]
pub fn see_other(to: &str) -> Response {
    Response::new(303)
        .header("location", to)
        .header("cache-control", "no-store")
}

/// Where a form may send the reader back to: a page of these, never
/// another site.
#[must_use]
pub fn safe_back(to: Option<&str>) -> String {
    match to {
        Some(t)
            if (t == "/basic/" || t.starts_with("/basic/"))
                && !t.contains("//")
                && !t.contains('\\')
                && t.bytes().all(|b| (0x21..0x7f).contains(&b)) =>
        {
            t.to_string()
        }
        _ => "/basic/".to_string(),
    }
}

/// The page's own cookie: the reader's choices, readable by nothing but
/// the server, a year long.
#[must_use]
pub fn prefs_cookie(value: &str) -> String {
    format!("losos_basic={value}; Path=/basic; Max-Age=31536000; Secure; HttpOnly; SameSite=Lax")
}

/// A word for the next page only: `rail`, `applied`, `signed-out`.
#[must_use]
pub fn flash_cookie(word: &str) -> String {
    format!("losos_basic_flash={word}; Path=/basic; Max-Age=30; Secure; HttpOnly; SameSite=Lax")
}

#[must_use]
pub fn flash_spent() -> String {
    "losos_basic_flash=; Path=/basic; Max-Age=0; Secure; HttpOnly; SameSite=Lax".to_string()
}

/// The spare key, for this browser's session: only nginx reads it back,
/// as the Bearer header of a request under /basic/.
#[must_use]
pub fn spare_cookie(key: &str) -> String {
    format!("losos_spare={key}; Path=/basic; Secure; HttpOnly; SameSite=Strict")
}

#[must_use]
pub fn spare_gone() -> String {
    "losos_spare=; Path=/basic; Max-Age=0; Secure; HttpOnly; SameSite=Strict".to_string()
}

/// Where the browser's request came from, as nginx passed it.
#[must_use]
pub fn forwarded(req: &Request) -> Forwarded {
    let h = |n: &str| req.header(n).unwrap_or_default().to_string();
    Forwarded {
        host: h("x-forwarded-host"),
        proto: if h("x-forwarded-proto") == "http" {
            "http".into()
        } else {
            "https".into()
        },
        address: h("x-real-ip"),
    }
}

/// A form sent from another site is refused. The cookies are SameSite
/// already; this holds for a browser that ignores that, the way
/// `server::cross_site` holds for LosOS Git.
#[must_use]
pub fn same_site(req: &Request) -> bool {
    if let Some(site) = req.header("sec-fetch-site") {
        return matches!(site, "same-origin" | "none");
    }
    match req.header("origin") {
        None => true,
        Some(origin) => origin == forwarded(req).origin(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forms_decode() {
        let f = form(b"name=a+b&pw=%26%3D%25&x=%zz&x=2");
        assert_eq!(field(&f, "name"), Some("a b"));
        assert_eq!(field(&f, "pw"), Some("&=%"));
        assert_eq!(field(&f, "x"), Some("2"));
        assert_eq!(form(b"t=%"), vec![("t".to_string(), "%".to_string())]);
        assert_eq!(form(b"t=%4"), vec![("t".to_string(), "%4".to_string())]);
        assert_eq!(field(&form("a=%é".as_bytes()), "a"), Some("%é"));
    }

    #[test]
    fn back_stays_on_these_pages() {
        assert_eq!(safe_back(Some("/basic/storage")), "/basic/storage");
        assert_eq!(
            safe_back(Some("/basic/settings/network")),
            "/basic/settings/network"
        );
        for bad in [
            "https://evil/",
            "//evil/basic/",
            "/basic//evil",
            "/basic/\\evil",
            "/api/x",
            "/basic/a b",
            "/basic/\r\nx",
        ] {
            assert_eq!(safe_back(Some(bad)), "/basic/", "{bad}");
        }
        assert_eq!(safe_back(None), "/basic/");
    }
}
