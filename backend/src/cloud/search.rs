//! `SEARCH` (RFC 5323 basicsearch) on `/remote.php/dav/`, which the phone
//! apps use for their photo and media views and the web app for search.
//!
//! The query is evaluated over the rows below the scope: the comparisons
//! (`eq`, `like`, `gt`, `gte`, `lt`, `lte`), `and`, `or`, `not` and
//! `is-collection`, on the properties a client can sort and filter files by.

use crate::cloud::dav::{dav_error, read_body, store_error, Ctx, Resolve};
use crate::cloud::store::{clean_path, Entry};
use crate::cloud::xml::{self, Element, Name, PropRequest, DAV, NC, OC};
use actix_web::http::StatusCode;
use actix_web::{web, HttpResponse};

/// Most results one search returns.
const MAX_RESULTS: usize = 10_000;

/// A value a property has, for comparison.
#[derive(Debug, Clone, PartialEq, PartialOrd)]
enum Value {
    Num(f64),
    Text(String),
}

fn value(e: &Entry, n: &Name) -> Option<Value> {
    Some(match (n.ns.as_str(), n.local.as_str()) {
        (DAV, "displayname") => Value::Text(e.name.clone()),
        (DAV, "getcontenttype") => Value::Text(if e.dir {
            "httpd/unix-directory".into()
        } else {
            e.mime.clone()
        }),
        (DAV, "getlastmodified") => Value::Num(e.mtime as f64),
        (DAV, "getcontentlength") | (OC, "size") => Value::Num(e.size as f64),
        (OC, "fileid") => Value::Num(e.id as f64),
        (OC, "favorite") => Value::Num(if e.favorite { 1.0 } else { 0.0 }),
        (NC, "upload_time") | (NC, "creation_time") => Value::Num(e.mtime as f64),
        _ => return None,
    })
}

fn literal(n: &Name, text: &str) -> Value {
    match (n.ns.as_str(), n.local.as_str()) {
        (DAV, "getlastmodified") => text
            .parse::<f64>()
            .ok()
            .or_else(|| {
                httpdate::parse_http_date(text)
                    .ok()
                    .or_else(|| {
                        chrono::DateTime::parse_from_rfc3339(text).ok().map(|d| {
                            std::time::UNIX_EPOCH
                                + std::time::Duration::from_secs(d.timestamp().max(0) as u64)
                        })
                    })
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as f64)
            })
            .map(Value::Num)
            .unwrap_or(Value::Text(text.into())),
        _ => match text.parse::<f64>() {
            Ok(v) if value_is_numeric(n) => Value::Num(v),
            _ => Value::Text(text.into()),
        },
    }
}

fn value_is_numeric(n: &Name) -> bool {
    matches!(
        (n.ns.as_str(), n.local.as_str()),
        (DAV, "getcontentlength")
            | (OC, "size")
            | (OC, "fileid")
            | (OC, "favorite")
            | (NC, "upload_time")
            | (NC, "creation_time")
    )
}

/// SQL LIKE with `%` and `_`, case-insensitive like Nextcloud's.
///
/// The usual wildcard walk: on a mismatch it goes back to the last `%` and
/// lets it take one more character, never further back, so a pattern of
/// many `%` costs at most pattern times text steps rather than exponential
/// time.
fn like(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().flat_map(char::to_lowercase).collect();
    let t: Vec<char> = text.chars().flat_map(char::to_lowercase).collect();
    let (mut pi, mut ti) = (0, 0);
    // The last `%` seen, and where in the text it started matching.
    let mut star: Option<(usize, usize)> = None;
    while ti < t.len() {
        match p.get(pi) {
            Some('%') => {
                star = Some((pi, ti));
                pi += 1;
            }
            Some(&c) if c == '_' || c == t[ti] => {
                pi += 1;
                ti += 1;
            }
            _ => match star {
                Some((sp, st)) => {
                    pi = sp + 1;
                    ti = st + 1;
                    star = Some((sp, st + 1));
                }
                None => return false,
            },
        }
    }
    p[pi..].iter().all(|c| *c == '%')
}

fn prop_of(el: &Element) -> Option<Name> {
    el.child(DAV, "prop")?
        .children
        .first()
        .map(|c| c.name.clone())
}

/// Does `e` match the condition `cond`?
fn matches(e: &Entry, cond: &Element) -> bool {
    match (cond.name.ns.as_str(), cond.name.local.as_str()) {
        (DAV, "and") => cond.children.iter().all(|c| matches(e, c)),
        (DAV, "or") => cond.children.iter().any(|c| matches(e, c)),
        (DAV, "not") => !cond.children.iter().all(|c| matches(e, c)),
        (DAV, "is-collection") => e.dir,
        (DAV, op @ ("eq" | "like" | "gt" | "gte" | "lt" | "lte")) => {
            let Some(n) = prop_of(cond) else {
                return false;
            };
            let Some(lit) = cond.child(DAV, "literal").map(|l| l.deep_text()) else {
                return false;
            };
            let Some(v) = value(e, &n) else {
                return false;
            };
            match op {
                "like" => match &v {
                    Value::Text(t) => like(&lit, t),
                    Value::Num(x) => like(&lit, &x.to_string()),
                },
                _ => {
                    let l = literal(&n, &lit);
                    let ord = match (&v, &l) {
                        (Value::Num(a), Value::Num(b)) => a.partial_cmp(b),
                        (Value::Text(a), Value::Text(b)) => {
                            Some(a.to_lowercase().cmp(&b.to_lowercase()))
                        }
                        _ => None,
                    };
                    let Some(ord) = ord else {
                        return false;
                    };
                    match op {
                        "eq" => ord.is_eq(),
                        "gt" => ord.is_gt(),
                        "gte" => ord.is_ge(),
                        "lt" => ord.is_lt(),
                        _ => ord.is_le(),
                    }
                }
            }
        }
        _ => false,
    }
}

/// The parsed query.
pub struct Query {
    pub props: PropRequest,
    pub scope: String,
    pub depth_one: bool,
    pub cond: Option<Element>,
    pub order: Vec<(Name, bool)>,
    pub limit: usize,
    pub offset: usize,
}

pub fn parse(body: &str, login: &str) -> Result<Query, String> {
    let root = xml::parse(body).map_err(|e| e.to_string())?;
    if !root.is(DAV, "searchrequest") {
        return Err("not a searchrequest".into());
    }
    let bs = root
        .child(DAV, "basicsearch")
        .ok_or("only basicsearch is supported")?;
    let props = bs
        .find(&[(DAV, "select"), (DAV, "prop")])
        .map(|p| PropRequest::Props(p.children.iter().map(|c| c.name.clone()).collect()))
        .unwrap_or(PropRequest::AllProp(Vec::new()));
    let scope_el = bs
        .find(&[(DAV, "from"), (DAV, "scope")])
        .ok_or("no scope")?;
    let href = scope_el
        .child(DAV, "href")
        .map(|h| h.deep_text())
        .unwrap_or_default();
    // `/files/<login>/sub`, with or without the `/remote.php/dav` prefix.
    let href = crate::cloud::util::decode_path(&href).unwrap_or(href);
    let href = href
        .split_once("/files/")
        .map(|(_, r)| r)
        .ok_or("the scope is not a files path")?;
    let (owner, rest) = href.split_once('/').unwrap_or((href, ""));
    if owner.trim_end_matches('/') != login {
        return Err("not your files".into());
    }
    let scope = clean_path(rest).map_err(|_| "bad scope")?;
    let depth_one = scope_el
        .child(DAV, "depth")
        .is_some_and(|d| d.deep_text() == "1");
    let cond = bs
        .child(DAV, "where")
        .and_then(|w| w.children.first().cloned());
    let mut order = Vec::new();
    if let Some(ob) = bs.child(DAV, "orderby") {
        for o in ob.children_named(DAV, "order") {
            if let Some(n) = prop_of(o) {
                order.push((n, o.child(DAV, "descending").is_some()));
            }
        }
    }
    let limit_el = bs.child(DAV, "limit");
    let limit = limit_el
        .and_then(|l| l.child(DAV, "nresults"))
        .and_then(|n| n.deep_text().parse().ok())
        .unwrap_or(MAX_RESULTS)
        .min(MAX_RESULTS);
    let offset = limit_el
        .and_then(|l| l.child(NC, "firstresult"))
        .and_then(|n| n.deep_text().parse().ok())
        .unwrap_or(0);
    Ok(Query {
        props,
        scope,
        depth_one,
        cond,
        order,
        limit,
        offset,
    })
}

/// Run a query over a set of entries.
pub fn run(q: &Query, entries: Vec<Entry>) -> Vec<Entry> {
    let mut hits: Vec<Entry> = entries
        .into_iter()
        .filter(|e| {
            if q.depth_one {
                crate::cloud::store::parent_of(&e.path) == Some(q.scope.as_str())
            } else {
                true
            }
        })
        .filter(|e| q.cond.as_ref().is_none_or(|c| matches(e, c)))
        .collect();
    if !q.order.is_empty() {
        hits.sort_by(|a, b| {
            for (n, desc) in &q.order {
                let ord = value(a, n)
                    .partial_cmp(&value(b, n))
                    .unwrap_or(std::cmp::Ordering::Equal);
                let ord = if *desc { ord.reverse() } else { ord };
                if !ord.is_eq() {
                    return ord;
                }
            }
            std::cmp::Ordering::Equal
        });
    }
    hits.into_iter().skip(q.offset).take(q.limit).collect()
}

pub async fn handle(ctx: &Ctx<'_>, body: web::Payload) -> HttpResponse {
    let bytes = match read_body(body, 1 << 20).await {
        Ok(b) => b,
        Err(r) => return r,
    };
    let q = match parse(&String::from_utf8_lossy(&bytes), &ctx.who.login) {
        Ok(q) => q,
        Err(m) => {
            return dav_error(
                StatusCode::BAD_REQUEST,
                "Sabre\\DAV\\Exception\\BadRequest",
                &m,
            )
        }
    };
    let entries = match ctx.cloud.store.walk(&ctx.who.login, &q.scope) {
        Ok(e) => e,
        Err(e) => return store_error(&e),
    };
    let hits = run(&q, entries);
    let t = match crate::cloud::dav::resolve(
        &format!(
            "/remote.php/dav/files/{}",
            crate::cloud::util::encode_path(&ctx.who.login)
        ),
        &ctx.who.login,
    ) {
        Resolve::Ok(t) => t,
        _ => return HttpResponse::InternalServerError().finish(),
    };
    let responses: Vec<_> = hits
        .iter()
        .map(|e| ctx.entry_response(&t, e, &q.props))
        .collect();
    HttpResponse::build(StatusCode::MULTI_STATUS)
        .content_type("application/xml; charset=utf-8")
        .body(xml::multistatus(&responses))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, mime: &str, mtime: i64, dir: bool) -> Entry {
        Entry {
            id: mtime,
            owner: "alice".into(),
            path: format!("Photos/{name}"),
            name: name.into(),
            dir,
            size: 10,
            mtime,
            etag: "e".into(),
            mime: mime.into(),
            checksum: None,
            favorite: false,
        }
    }

    const MEDIA: &str = r#"<?xml version="1.0"?>
<d:searchrequest xmlns:d="DAV:" xmlns:oc="http://owncloud.org/ns" xmlns:nc="http://nextcloud.org/ns">
 <d:basicsearch>
  <d:select><d:prop><d:displayname/><oc:fileid/></d:prop></d:select>
  <d:from><d:scope><d:href>/files/alice</d:href><d:depth>infinity</d:depth></d:scope></d:from>
  <d:where><d:and>
    <d:or>
      <d:like><d:prop><d:getcontenttype/></d:prop><d:literal>image/%</d:literal></d:like>
      <d:like><d:prop><d:getcontenttype/></d:prop><d:literal>video/%</d:literal></d:like>
    </d:or>
    <d:gt><d:prop><d:getlastmodified/></d:prop><d:literal>100</d:literal></d:gt>
  </d:and></d:where>
  <d:orderby><d:order><d:prop><d:getlastmodified/></d:prop><d:descending/></d:order></d:orderby>
  <d:limit><d:nresults>2</d:nresults></d:limit>
 </d:basicsearch>
</d:searchrequest>"#;

    #[test]
    fn the_phone_apps_media_search() {
        let q = parse(MEDIA, "alice").unwrap();
        assert_eq!(q.limit, 2);
        let hits = run(
            &q,
            vec![
                entry("a.jpg", "image/jpeg", 300, false),
                entry("b.mp4", "video/mp4", 400, false),
                entry("old.jpg", "image/jpeg", 50, false),
                entry("c.txt", "text/plain", 500, false),
                entry("d.png", "image/png", 200, false),
            ],
        );
        let names: Vec<&str> = hits.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["b.mp4", "a.jpg"]);
    }

    #[test]
    fn another_persons_scope_is_refused() {
        assert!(parse(MEDIA, "bob").is_err());
    }

    #[test]
    fn like_is_case_insensitive_with_wildcards() {
        assert!(like("%.JPG", "holiday.jpg"));
        assert!(like("a_c", "abc"));
        assert!(!like("a_c", "ac"));
        assert!(like("%", ""));
        assert!(like("%ab%cd%", "xxabyycdzz"));
        assert!(!like("%ab%cd", "xxabyycdzz"));
        assert!(like("a%%b", "ab"));
        assert!(!like("", "a"));
    }

    #[test]
    fn many_wildcards_that_fail_are_not_slow() {
        let pattern = format!("{}b", "%a".repeat(40));
        let text = "a".repeat(200);
        let started = std::time::Instant::now();
        assert!(!like(&pattern, &text));
        assert!(started.elapsed() < std::time::Duration::from_secs(1));
    }
}
