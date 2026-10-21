//! CalDAV and CardDAV, at the addresses Nextcloud gives them, so a phone
//! that synced calendars and contacts with Nextcloud (DAVx5, iOS, the
//! Nextcloud apps' account setup, Thunderbird) finds them in the same place:
//!
//! - `/remote.php/dav/principals/users/<login>/`, the person;
//! - `/remote.php/dav/calendars/<login>/<calendar>/<object>.ics`;
//! - `/remote.php/dav/addressbooks/users/<login>/<book>/<object>.vcf`.
//!
//! Reports: calendar-multiget, calendar-query (component and time range),
//! addressbook-multiget, addressbook-query, sync-collection. No scheduling
//! and no sharing yet: a person sees their own collections. The data is in
//! [`crate::cloud::pim`].

use crate::cloud::auth::Who;
use crate::cloud::dav::{dav_error, read_body, store_error};
use crate::cloud::pim::{self, Collection, CollectionFields, Expect, Kind, Object, PutError};
use crate::cloud::store::StoreError;
use crate::cloud::util::{decode_path, encode_path, http_date, xml_escape};
use crate::cloud::xml::{
    self, Element, Name, PropChange, PropRequest, Response, APPLE, CALDAV, CARDDAV, CS, DAV, OC,
    SABRE,
};
use crate::cloud::Cloud;
use actix_web::http::StatusCode;
use actix_web::{web, HttpRequest, HttpResponse};

const MAX_XML: usize = 1 << 20;
const MAX_OBJECT: usize = 10 << 20;
const SYNC_PREFIX: &str = "http://sabre.io/ns/sync/";

/// Where under the person's DAV space a request points.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Place {
    /// `/remote.php/dav/principals/`, `/calendars/`, `/addressbooks/`
    /// (and `/addressbooks/users/`): a list with one entry, the person's.
    Top(&'static str),
    Principal(String),
    Home(Kind, String),
    Collection(Kind, String, String),
    Object(Kind, String, String, String),
}

/// `None`: not a calendar or contacts path; the files' WebDAV takes it.
pub fn route(raw: &str) -> Option<Result<Place, ()>> {
    let path = decode_path(raw)?;
    let rest = path.strip_prefix("/remote.php/dav/")?.trim_end_matches('/');
    let parts: Vec<&str> = rest.split('/').collect();
    let place = match parts.as_slice() {
        ["principals"] => Place::Top("principals"),
        ["principals", "users"] => Place::Top("principals/users"),
        ["principals", "users", login] => Place::Principal((*login).into()),
        ["calendars"] => Place::Top("calendars"),
        ["calendars", login] => Place::Home(Kind::Calendar, (*login).into()),
        ["calendars", login, c] => Place::Collection(Kind::Calendar, (*login).into(), (*c).into()),
        ["calendars", login, c, o] => {
            Place::Object(Kind::Calendar, (*login).into(), (*c).into(), (*o).into())
        }
        ["addressbooks"] => Place::Top("addressbooks"),
        ["addressbooks", "users"] => Place::Top("addressbooks/users"),
        ["addressbooks", "users", login] => Place::Home(Kind::AddressBook, (*login).into()),
        ["addressbooks", "users", login, c] => {
            Place::Collection(Kind::AddressBook, (*login).into(), (*c).into())
        }
        ["addressbooks", "users", login, c, o] => {
            Place::Object(Kind::AddressBook, (*login).into(), (*c).into(), (*o).into())
        }
        [first, ..] if ["principals", "calendars", "addressbooks"].contains(first) => {
            return Some(Err(()))
        }
        _ => return None,
    };
    Some(Ok(place))
}

pub fn principal_href(login: &str) -> String {
    format!("/remote.php/dav/principals/users/{}/", encode_path(login))
}

pub fn home_href(kind: Kind, login: &str) -> String {
    match kind {
        Kind::Calendar => format!("/remote.php/dav/calendars/{}/", encode_path(login)),
        Kind::AddressBook => format!("/remote.php/dav/addressbooks/users/{}/", encode_path(login)),
    }
}

fn collection_href(c: &Collection) -> String {
    format!("{}{}/", home_href(c.kind, &c.owner), encode_path(&c.uri))
}

fn object_href(c: &Collection, uri: &str) -> String {
    format!("{}{}", collection_href(c), encode_path(uri))
}

fn sync_token(c: &Collection) -> String {
    format!("{SYNC_PREFIX}{}", c.sync)
}

fn href(h: &str) -> String {
    format!("<d:href>{}</d:href>", xml_escape(h))
}

fn n(ns: &str, local: &str) -> Name {
    Name::new(ns, local)
}

fn privileges() -> String {
    [
        "<d:all/>",
        "<d:read/>",
        "<d:write/>",
        "<d:write-properties/>",
        "<d:write-content/>",
        "<d:bind/>",
        "<d:unbind/>",
        "<d:read-current-user-privilege-set/>",
        "<cal:read-free-busy/>",
    ]
    .iter()
    .map(|p| format!("<d:privilege>{p}</d:privilege>"))
    .collect()
}

fn reports(kind: Kind) -> String {
    let list: &[&str] = match kind {
        Kind::Calendar => &[
            "<cal:calendar-multiget/>",
            "<cal:calendar-query/>",
            "<d:sync-collection/>",
        ],
        Kind::AddressBook => &[
            "<card:addressbook-multiget/>",
            "<card:addressbook-query/>",
            "<d:sync-collection/>",
        ],
    };
    list.iter()
        .map(|r| format!("<d:supported-report><d:report>{r}</d:report></d:supported-report>"))
        .collect()
}

/// The answer for one resource: what it has, picked by what was asked.
/// `lazy` supplies the costly properties (the object's text) only when
/// they are asked for by name.
fn answer(
    at: String,
    all: Vec<(Name, String)>,
    req: &PropRequest,
    lazy: impl Fn(&Name) -> Option<String>,
) -> Response {
    let mut r = Response::new(at);
    match req {
        PropRequest::PropName => {
            r.found = all.into_iter().map(|(k, _)| (k, String::new())).collect();
        }
        PropRequest::AllProp(include) => {
            r.found = all;
            for name in include {
                if let Some(v) = lazy(name) {
                    r.found.push((name.clone(), v));
                }
            }
        }
        PropRequest::Props(names) => {
            for name in names {
                if let Some((_, v)) = all.iter().find(|(k, _)| k == name) {
                    r.found.push((name.clone(), v.clone()));
                } else if let Some(v) = lazy(name) {
                    r.found.push((name.clone(), v));
                } else {
                    r.missing.push(name.clone());
                }
            }
        }
    }
    r
}

fn principal_props(who: &Who) -> Vec<(Name, String)> {
    let me = principal_href(&who.login);
    let mut addresses = String::new();
    if !who.email.is_empty() {
        addresses.push_str(&href(&format!("mailto:{}", who.email)));
    }
    addresses.push_str(&href(&me));
    let name = if who.name.is_empty() {
        &who.login
    } else {
        &who.name
    };
    vec![
        (n(DAV, "resourcetype"), "<d:principal/>".into()),
        (n(DAV, "displayname"), xml_escape(name)),
        (n(DAV, "principal-URL"), href(&me)),
        (n(DAV, "current-user-principal"), href(&me)),
        (n(DAV, "group-membership"), String::new()),
        (
            n(CALDAV, "calendar-home-set"),
            href(&home_href(Kind::Calendar, &who.login)),
        ),
        (
            n(CARDDAV, "addressbook-home-set"),
            href(&home_href(Kind::AddressBook, &who.login)),
        ),
        (n(CALDAV, "calendar-user-address-set"), addresses),
        (n(CALDAV, "calendar-user-type"), "INDIVIDUAL".into()),
        (n(SABRE, "email-address"), xml_escape(&who.email)),
    ]
}

fn plain_collection_props(who: &Who, name: &str) -> Vec<(Name, String)> {
    vec![
        (n(DAV, "resourcetype"), "<d:collection/>".into()),
        (n(DAV, "displayname"), xml_escape(name)),
        (
            n(DAV, "current-user-principal"),
            href(&principal_href(&who.login)),
        ),
        (n(DAV, "owner"), href(&principal_href(&who.login))),
        (n(DAV, "current-user-privilege-set"), privileges()),
    ]
}

fn collection_props(who: &Who, c: &Collection) -> Vec<(Name, String)> {
    let token = xml_escape(&sync_token(c));
    let mut p = vec![
        (n(DAV, "displayname"), xml_escape(&c.name)),
        (n(CS, "getctag"), token.clone()),
        (n(DAV, "sync-token"), token),
        (n(DAV, "owner"), href(&principal_href(&c.owner))),
        (
            n(OC, "owner-principal"),
            xml_escape(&format!("principals/users/{}", c.owner)),
        ),
        (
            n(DAV, "current-user-principal"),
            href(&principal_href(&who.login)),
        ),
        (n(DAV, "current-user-privilege-set"), privileges()),
        (n(DAV, "supported-report-set"), reports(c.kind)),
    ];
    match c.kind {
        Kind::Calendar => {
            p.push((
                n(DAV, "resourcetype"),
                "<d:collection/><cal:calendar/>".into(),
            ));
            p.push((
                n(CALDAV, "calendar-description"),
                xml_escape(&c.description),
            ));
            if let Some(color) = &c.color {
                p.push((n(APPLE, "calendar-color"), xml_escape(color)));
            }
            p.push((n(APPLE, "calendar-order"), c.position.to_string()));
            let comps: String = c
                .components
                .split(',')
                .filter(|s| !s.trim().is_empty())
                .map(|s| format!("<cal:comp name=\"{}\"/>", xml_escape(s.trim())))
                .collect();
            p.push((n(CALDAV, "supported-calendar-component-set"), comps));
            p.push((
                n(CALDAV, "supported-calendar-data"),
                "<cal:calendar-data content-type=\"text/calendar\" version=\"2.0\"/>".into(),
            ));
            if let Some(tz) = &c.timezone {
                p.push((n(CALDAV, "calendar-timezone"), xml_escape(tz)));
            }
            p.push((n(CALDAV, "max-resource-size"), MAX_OBJECT.to_string()));
        }
        Kind::AddressBook => {
            p.push((
                n(DAV, "resourcetype"),
                "<d:collection/><card:addressbook/>".into(),
            ));
            p.push((
                n(CARDDAV, "addressbook-description"),
                xml_escape(&c.description),
            ));
            p.push((
                n(CARDDAV, "supported-address-data"),
                "<card:address-data-type content-type=\"text/vcard\" version=\"3.0\"/><card:address-data-type content-type=\"text/vcard\" version=\"4.0\"/>".into(),
            ));
            p.push((n(CARDDAV, "max-resource-size"), MAX_OBJECT.to_string()));
        }
    }
    p
}

fn content_type(kind: Kind, o: &Object) -> String {
    match kind {
        Kind::Calendar => match &o.component {
            Some(c) => format!(
                "text/calendar; charset=utf-8; component={}",
                c.to_ascii_lowercase()
            ),
            None => "text/calendar; charset=utf-8".into(),
        },
        Kind::AddressBook => "text/vcard; charset=utf-8".into(),
    }
}

fn object_response(c: &Collection, o: &Object, req: &PropRequest) -> Response {
    let all = vec![
        (n(DAV, "getetag"), xml_escape(&o.etag)),
        (n(DAV, "getcontenttype"), content_type(c.kind, o)),
        (n(DAV, "getcontentlength"), o.data.len().to_string()),
        (n(DAV, "getlastmodified"), http_date(o.modified)),
        (n(DAV, "resourcetype"), String::new()),
    ];
    answer(object_href(c, &o.uri), all, req, |name| {
        (name.is(CALDAV, "calendar-data") && c.kind == Kind::Calendar
            || name.is(CARDDAV, "address-data") && c.kind == Kind::AddressBook)
            .then(|| xml_escape(&o.data))
    })
}

fn multistatus(responses: &[Response], token: Option<&str>) -> HttpResponse {
    let mut body = xml::multistatus(responses);
    if let Some(t) = token {
        let at = body.rfind("</d:multistatus>").unwrap_or(body.len());
        body.insert_str(
            at,
            &format!("<d:sync-token>{}</d:sync-token>", xml_escape(t)),
        );
    }
    HttpResponse::build(StatusCode::MULTI_STATUS)
        .content_type("application/xml; charset=utf-8")
        .body(body)
}

/// A DAV precondition failure: `<d:error><p:cond/></d:error>`.
fn condition(status: StatusCode, cond: &str) -> HttpResponse {
    HttpResponse::build(status)
        .content_type("application/xml; charset=utf-8")
        .body(format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
<d:error {}>{cond}</d:error>"#,
            xml::ROOT_NAMESPACES
        ))
}

fn not_found() -> HttpResponse {
    dav_error(
        StatusCode::NOT_FOUND,
        "Sabre\\DAV\\Exception\\NotFound",
        "no such calendar, address book or object",
    )
}

fn forbidden(message: &str) -> HttpResponse {
    dav_error(
        StatusCode::FORBIDDEN,
        "Sabre\\DAV\\Exception\\Forbidden",
        message,
    )
}

fn method_not_allowed() -> HttpResponse {
    dav_error(
        StatusCode::METHOD_NOT_ALLOWED,
        "Sabre\\DAV\\Exception\\MethodNotAllowed",
        "not here",
    )
}

fn depth0(req: &HttpRequest) -> bool {
    req.headers()
        .get("depth")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        == Some("0")
}

/// An ETag precondition as stored: quoted, never weak.
fn etag_header(req: &HttpRequest, name: &str) -> Option<String> {
    let v = req.headers().get(name)?.to_str().ok()?.trim();
    let v = v.strip_prefix("W/").unwrap_or(v);
    Some(v.to_owned())
}

async fn xml_body(body: web::Payload) -> Result<Option<Element>, HttpResponse> {
    let bytes = read_body(body, MAX_XML).await?;
    let text = String::from_utf8_lossy(&bytes);
    if text.trim().is_empty() {
        return Ok(None);
    }
    xml::parse(&text).map(Some).map_err(|e| {
        dav_error(
            StatusCode::BAD_REQUEST,
            "Sabre\\DAV\\Exception\\BadRequest",
            &e.to_string(),
        )
    })
}

fn prop_request(root: &Element) -> PropRequest {
    if root.child(DAV, "allprop").is_some() {
        return PropRequest::AllProp(Vec::new());
    }
    match root.child(DAV, "prop") {
        Some(p) => PropRequest::Props(p.children.iter().map(|c| c.name.clone()).collect()),
        None => PropRequest::Props(vec![n(DAV, "getetag")]),
    }
}

/// Handle a calendar or contacts request, or `None` to leave it to the
/// files' WebDAV.
pub async fn handle(
    cloud: &Cloud,
    who: &Who,
    req: &HttpRequest,
    body: web::Payload,
) -> Option<HttpResponse> {
    let place = match route(req.path())? {
        Ok(p) => p,
        Err(()) => return Some(not_found()),
    };
    let owner = match &place {
        Place::Top(_) => who.login.clone(),
        Place::Principal(o)
        | Place::Home(_, o)
        | Place::Collection(_, o, _)
        | Place::Object(_, o, _, _) => o.clone(),
    };
    if owner != who.login {
        return Some(forbidden("not your calendars or contacts"));
    }
    if let Err(e) = pim::ensure_defaults(&cloud.store, &owner) {
        return Some(store_error(&e));
    }
    let method = req.method().as_str().to_owned();
    Some(match (&place, method.as_str()) {
        (_, "PROPFIND") => propfind(cloud, who, req, &place, body).await,
        (Place::Collection(..), "REPORT") => report(cloud, who, &place, body).await,
        (_, "REPORT") => {
            // principal-property-search and the like: nobody to find.
            let _ = read_body(body, MAX_XML).await;
            multistatus(&[], None)
        }
        (Place::Collection(..), "PROPPATCH") => proppatch(cloud, &place, req, body).await,
        (Place::Collection(..), "MKCALENDAR" | "MKCOL") => mkcol(cloud, &place, body).await,
        (Place::Collection(..), "DELETE") => match collection_of(cloud, &place) {
            Ok(c) => match pim::delete_collection(&cloud.store, &c) {
                Ok(()) => HttpResponse::NoContent().finish(),
                Err(e) => store_error(&e),
            },
            Err(r) => r,
        },
        (Place::Object(..), "GET" | "HEAD") => get(cloud, &place, method == "HEAD"),
        (Place::Object(..), "PUT") => put(cloud, &place, req, body).await,
        (Place::Object(..), "DELETE") => delete(cloud, &place, req),
        _ => method_not_allowed(),
    })
}

fn collection_of(cloud: &Cloud, place: &Place) -> Result<Collection, HttpResponse> {
    let (kind, owner, uri) = match place {
        Place::Collection(k, o, c) | Place::Object(k, o, c, _) => (*k, o, c),
        _ => return Err(not_found()),
    };
    pim::collection(&cloud.store, owner, kind, uri).map_err(|e| match e {
        StoreError::NotFound => not_found(),
        e => store_error(&e),
    })
}

async fn propfind(
    cloud: &Cloud,
    who: &Who,
    req: &HttpRequest,
    place: &Place,
    body: web::Payload,
) -> HttpResponse {
    let bytes = match read_body(body, MAX_XML).await {
        Ok(b) => b,
        Err(r) => return r,
    };
    let preq = match xml::parse_propfind(&String::from_utf8_lossy(&bytes)) {
        Ok(p) => p,
        Err(e) => {
            return dav_error(
                StatusCode::BAD_REQUEST,
                "Sabre\\DAV\\Exception\\BadRequest",
                &e.to_string(),
            )
        }
    };
    let deep = !depth0(req);
    let none = |_: &Name| None;
    let mut out = Vec::new();
    match place {
        Place::Top(what) => {
            let at = format!("/remote.php/dav/{what}/");
            out.push(answer(at, plain_collection_props(who, what), &preq, none));
            if deep {
                match *what {
                    "principals" => out.push(answer(
                        "/remote.php/dav/principals/users/".into(),
                        plain_collection_props(who, "users"),
                        &preq,
                        none,
                    )),
                    "addressbooks" => out.push(answer(
                        "/remote.php/dav/addressbooks/users/".into(),
                        plain_collection_props(who, "users"),
                        &preq,
                        none,
                    )),
                    "principals/users" => out.push(answer(
                        principal_href(&who.login),
                        principal_props(who),
                        &preq,
                        none,
                    )),
                    "calendars" => out.push(answer(
                        home_href(Kind::Calendar, &who.login),
                        plain_collection_props(who, &who.login),
                        &preq,
                        none,
                    )),
                    _ => out.push(answer(
                        home_href(Kind::AddressBook, &who.login),
                        plain_collection_props(who, &who.login),
                        &preq,
                        none,
                    )),
                }
            }
        }
        Place::Principal(_) => {
            out.push(answer(
                principal_href(&who.login),
                principal_props(who),
                &preq,
                none,
            ));
        }
        Place::Home(kind, owner) => {
            out.push(answer(
                home_href(*kind, owner),
                plain_collection_props(who, owner),
                &preq,
                none,
            ));
            if deep {
                match pim::collections(&cloud.store, owner, *kind) {
                    Ok(list) => {
                        for c in list {
                            out.push(answer(
                                collection_href(&c),
                                collection_props(who, &c),
                                &preq,
                                none,
                            ));
                        }
                    }
                    Err(e) => return store_error(&e),
                }
            }
        }
        Place::Collection(..) => {
            let c = match collection_of(cloud, place) {
                Ok(c) => c,
                Err(r) => return r,
            };
            out.push(answer(
                collection_href(&c),
                collection_props(who, &c),
                &preq,
                none,
            ));
            if deep {
                match pim::objects(&cloud.store, &c) {
                    Ok(list) => out.extend(list.iter().map(|o| object_response(&c, o, &preq))),
                    Err(e) => return store_error(&e),
                }
            }
        }
        Place::Object(_, _, _, uri) => {
            let c = match collection_of(cloud, place) {
                Ok(c) => c,
                Err(r) => return r,
            };
            match pim::object(&cloud.store, &c, uri) {
                Ok(o) => out.push(object_response(&c, &o, &preq)),
                Err(StoreError::NotFound) => return not_found(),
                Err(e) => return store_error(&e),
            }
        }
    }
    multistatus(&out, None)
}

/// The object names an `<d:href>` list points at, inside collection `c`.
fn hrefs_in(root: &Element, c: &Collection) -> Vec<(String, Option<String>)> {
    let base = collection_href(c);
    root.children_named(DAV, "href")
        .map(|h| {
            let raw = h.deep_text();
            let path = match raw.find("://") {
                Some(i) => raw[i + 3..]
                    .find('/')
                    .map(|j| raw[i + 3 + j..].to_owned())
                    .unwrap_or_default(),
                None => raw.clone(),
            };
            let decoded = decode_path(&path).unwrap_or_default();
            let base_decoded = decode_path(&base).unwrap_or_default();
            let uri = decoded
                .strip_prefix(&base_decoded)
                .filter(|u| !u.is_empty() && !u.contains('/'))
                .map(str::to_owned);
            (raw, uri)
        })
        .collect()
}

/// `<cal:comp-filter>` and `<cal:time-range>` of a calendar-query: the
/// component asked for and the window, if any.
fn query_filter(root: &Element) -> (Option<String>, Option<i64>, Option<i64>) {
    let Some(vcal) = root.find(&[(CALDAV, "filter"), (CALDAV, "comp-filter")]) else {
        return (None, None, None);
    };
    let Some(inner) = vcal.child(CALDAV, "comp-filter") else {
        return (None, None, None);
    };
    let attr = |e: &Element, k: &str| e.attrs.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());
    let comp = attr(inner, "name").map(|s| s.to_ascii_uppercase());
    let (mut start, mut end) = (None, None);
    if let Some(tr) = inner.child(CALDAV, "time-range") {
        start = attr(tr, "start")
            .and_then(|v| pim::parse_time(&v))
            .map(|t| t.0);
        end = attr(tr, "end")
            .and_then(|v| pim::parse_time(&v))
            .map(|t| t.0);
    }
    (comp, start, end)
}

/// Does an object fall in `[start, end)`? Without a start it is unbounded
/// (a task with no dates, a repeating event) and always does.
fn in_window(o: &Object, start: Option<i64>, end: Option<i64>) -> bool {
    let Some(first) = o.first else {
        return true;
    };
    let last = o.last.unwrap_or(i64::MAX);
    let before_end = end.is_none_or(|e| first < e);
    let after_start = start.is_none_or(|s| last > s || (last == first && first >= s));
    before_end && after_start
}

async fn report(cloud: &Cloud, _who: &Who, place: &Place, body: web::Payload) -> HttpResponse {
    let c = match collection_of(cloud, place) {
        Ok(c) => c,
        Err(r) => return r,
    };
    let root = match xml_body(body).await {
        Ok(Some(r)) => r,
        Ok(None) => {
            return dav_error(
                StatusCode::BAD_REQUEST,
                "Sabre\\DAV\\Exception\\BadRequest",
                "an empty report",
            )
        }
        Err(r) => return r,
    };
    let preq = prop_request(&root);
    let name = (root.name.ns.as_str(), root.name.local.as_str());
    match name {
        (CALDAV, "calendar-multiget") | (CARDDAV, "addressbook-multiget") => {
            let mut out = Vec::new();
            for (raw, uri) in hrefs_in(&root, &c) {
                let found = uri.and_then(|u| pim::object(&cloud.store, &c, &u).ok());
                match found {
                    Some(o) => out.push(object_response(&c, &o, &preq)),
                    None => {
                        let mut r = Response::new(raw);
                        r.status = Some("404 Not Found");
                        out.push(r);
                    }
                }
            }
            multistatus(&out, None)
        }
        (CALDAV, "calendar-query") | (CARDDAV, "addressbook-query") => {
            let (comp, start, end) = if name.0 == CALDAV {
                query_filter(&root)
            } else {
                (None, None, None)
            };
            let list = match pim::objects(&cloud.store, &c) {
                Ok(l) => l,
                Err(e) => return store_error(&e),
            };
            let out: Vec<Response> = list
                .iter()
                .filter(|o| comp.is_none() || o.component == comp)
                .filter(|o| in_window(o, start, end))
                .map(|o| object_response(&c, o, &preq))
                .collect();
            multistatus(&out, None)
        }
        (DAV, "sync-collection") => {
            let token = root
                .child(DAV, "sync-token")
                .map(|t| t.deep_text())
                .unwrap_or_default();
            let mut out = Vec::new();
            if token.is_empty() {
                match pim::objects(&cloud.store, &c) {
                    Ok(list) => out.extend(list.iter().map(|o| object_response(&c, o, &preq))),
                    Err(e) => return store_error(&e),
                }
            } else {
                let since = token
                    .strip_prefix(SYNC_PREFIX)
                    .and_then(|v| v.parse::<i64>().ok());
                let changes = match since.map(|s| pim::changes_since(&cloud.store, &c, s)) {
                    Some(Ok(Some(ch))) => ch,
                    Some(Err(e)) => return store_error(&e),
                    _ => return condition(StatusCode::FORBIDDEN, "<d:valid-sync-token/>"),
                };
                for uri in changes.changed {
                    if let Ok(o) = pim::object(&cloud.store, &c, &uri) {
                        out.push(object_response(&c, &o, &preq));
                    }
                }
                for uri in changes.deleted {
                    let mut r = Response::new(object_href(&c, &uri));
                    r.status = Some("404 Not Found");
                    out.push(r);
                }
            }
            multistatus(&out, Some(&sync_token(&c)))
        }
        _ => dav_error(
            StatusCode::FORBIDDEN,
            "Sabre\\DAV\\Exception\\ReportNotSupported",
            "this report is not supported here",
        ),
    }
}

/// Collection properties a client may set, out of a `<d:prop>`.
fn fields_from(prop: &Element) -> (CollectionFields, Vec<Name>) {
    let mut f = CollectionFields::default();
    let mut refused = Vec::new();
    for el in &prop.children {
        let text = el.deep_text();
        match (el.name.ns.as_str(), el.name.local.as_str()) {
            (DAV, "displayname") => f.name = Some(text),
            (CALDAV, "calendar-description") | (CARDDAV, "addressbook-description") => {
                f.description = Some(text)
            }
            (APPLE, "calendar-color") => f.color = Some(text),
            (APPLE, "calendar-order") => f.position = text.parse().ok(),
            (CALDAV, "calendar-timezone") => f.timezone = Some(text),
            (CALDAV, "supported-calendar-component-set") => {
                let comps: Vec<String> = el
                    .children_named(CALDAV, "comp")
                    .filter_map(|c| {
                        c.attrs
                            .iter()
                            .find(|(k, _)| k == "name")
                            .map(|(_, v)| v.to_ascii_uppercase())
                    })
                    .collect();
                if !comps.is_empty() {
                    f.components = Some(comps.join(","));
                }
            }
            (DAV, "resourcetype") => {}
            _ => refused.push(el.name.clone()),
        }
    }
    (f, refused)
}

async fn mkcol(cloud: &Cloud, place: &Place, body: web::Payload) -> HttpResponse {
    let Place::Collection(kind, owner, uri) = place else {
        return method_not_allowed();
    };
    let root = match xml_body(body).await {
        Ok(r) => r,
        Err(r) => return r,
    };
    let mut fields = CollectionFields::default();
    if let Some(root) = &root {
        if let Some(prop) = root.find(&[(DAV, "set"), (DAV, "prop")]) {
            if let Some(rt) = prop.child(DAV, "resourcetype") {
                let wants_calendar = rt.child(CALDAV, "calendar").is_some();
                let wants_book = rt.child(CARDDAV, "addressbook").is_some();
                if (*kind == Kind::Calendar && !wants_calendar)
                    || (*kind == Kind::AddressBook && !wants_book)
                {
                    return forbidden("only calendars here, or only address books");
                }
            }
            fields = fields_from(prop).0;
        }
    }
    if *kind == Kind::AddressBook {
        fields.components = None;
    }
    match pim::create(&cloud.store, owner, *kind, uri, &fields) {
        Ok(_) => HttpResponse::Created().finish(),
        Err(StoreError::Exists) => method_not_allowed(),
        Err(e) => store_error(&e),
    }
}

async fn proppatch(
    cloud: &Cloud,
    place: &Place,
    _req: &HttpRequest,
    body: web::Payload,
) -> HttpResponse {
    let c = match collection_of(cloud, place) {
        Ok(c) => c,
        Err(r) => return r,
    };
    let bytes = match read_body(body, MAX_XML).await {
        Ok(b) => b,
        Err(r) => return r,
    };
    let changes = match xml::parse_proppatch(&String::from_utf8_lossy(&bytes)) {
        Ok(c) => c,
        Err(e) => {
            return dav_error(
                StatusCode::BAD_REQUEST,
                "Sabre\\DAV\\Exception\\BadRequest",
                &e.to_string(),
            )
        }
    };
    let mut prop = Element {
        name: n(DAV, "prop"),
        attrs: Vec::new(),
        children: Vec::new(),
        text: String::new(),
        inner_xml: String::new(),
    };
    let mut removed = Vec::new();
    for change in changes {
        match change {
            PropChange::Set(el) => prop.children.push(el),
            PropChange::Remove(name) => removed.push(name),
        }
    }
    let (mut fields, refused) = fields_from(&prop);
    for name in &removed {
        match (name.ns.as_str(), name.local.as_str()) {
            (APPLE, "calendar-color") => fields.color = Some(String::new()),
            (CALDAV, "calendar-description") | (CARDDAV, "addressbook-description") => {
                fields.description = Some(String::new())
            }
            _ => {}
        }
    }
    let mut r = Response::new(collection_href(&c));
    if !refused.is_empty() {
        // All or nothing (RFC 4918 9.2): nothing is set.
        for name in prop.children.iter().map(|e| e.name.clone()) {
            let status = if refused.contains(&name) {
                "403 Forbidden"
            } else {
                "424 Failed Dependency"
            };
            r.other.push((name, status));
        }
        return multistatus(&[r], None);
    }
    if let Err(e) = pim::update(&cloud.store, &c, &fields) {
        return store_error(&e);
    }
    for el in &prop.children {
        r.found.push((el.name.clone(), String::new()));
    }
    for name in removed {
        r.found.push((name, String::new()));
    }
    multistatus(&[r], None)
}

fn get(cloud: &Cloud, place: &Place, head: bool) -> HttpResponse {
    let Place::Object(kind, _, _, uri) = place else {
        return method_not_allowed();
    };
    let c = match collection_of(cloud, place) {
        Ok(c) => c,
        Err(r) => return r,
    };
    match pim::object(&cloud.store, &c, uri) {
        Ok(o) => {
            let mut b = HttpResponse::Ok();
            b.content_type(content_type(*kind, &o))
                .insert_header(("ETag", o.etag.clone()))
                .insert_header(("Last-Modified", http_date(o.modified)));
            if head {
                b.insert_header(("Content-Length", o.data.len().to_string()))
                    .finish()
            } else {
                b.body(o.data)
            }
        }
        Err(StoreError::NotFound) => not_found(),
        Err(e) => store_error(&e),
    }
}

async fn put(cloud: &Cloud, place: &Place, req: &HttpRequest, body: web::Payload) -> HttpResponse {
    let Place::Object(kind, _, _, uri) = place else {
        return method_not_allowed();
    };
    let c = match collection_of(cloud, place) {
        Ok(c) => c,
        Err(r) => return r,
    };
    let bytes = match read_body(body, MAX_OBJECT).await {
        Ok(b) => b,
        Err(r) => return r,
    };
    let Ok(text) = String::from_utf8(bytes) else {
        return invalid(*kind);
    };
    let if_match = etag_header(req, "if-match");
    let expect = if etag_header(req, "if-none-match").as_deref() == Some("*") {
        Expect::New
    } else if let Some(e) = &if_match {
        Expect::Etag(e)
    } else {
        Expect::Any
    };
    match pim::put(&cloud.store, &c, uri, &text, expect) {
        Ok((etag, created)) => {
            let mut b = if created {
                HttpResponse::Created()
            } else {
                HttpResponse::NoContent()
            };
            b.insert_header(("ETag", etag)).finish()
        }
        Err(PutError::Invalid) => invalid(*kind),
        Err(PutError::Unsupported) => {
            condition(StatusCode::FORBIDDEN, "<cal:supported-calendar-component/>")
        }
        Err(PutError::UidConflict(other)) => {
            let inner = href(&object_href(&c, &other));
            let cond = match kind {
                Kind::Calendar => format!("<cal:no-uid-conflict>{inner}</cal:no-uid-conflict>"),
                Kind::AddressBook => {
                    format!("<card:no-uid-conflict>{inner}</card:no-uid-conflict>")
                }
            };
            condition(StatusCode::CONFLICT, &cond)
        }
        Err(PutError::Store(e)) => store_error(&e),
    }
}

fn invalid(kind: Kind) -> HttpResponse {
    condition(
        StatusCode::FORBIDDEN,
        match kind {
            Kind::Calendar => "<cal:valid-calendar-data/>",
            Kind::AddressBook => "<card:valid-address-data/>",
        },
    )
}

fn delete(cloud: &Cloud, place: &Place, req: &HttpRequest) -> HttpResponse {
    let Place::Object(_, _, _, uri) = place else {
        return method_not_allowed();
    };
    let c = match collection_of(cloud, place) {
        Ok(c) => c,
        Err(r) => return r,
    };
    let if_match = etag_header(req, "if-match");
    match pim::delete_object(&cloud.store, &c, uri, if_match.as_deref()) {
        Ok(()) => HttpResponse::NoContent().finish(),
        Err(StoreError::NotFound) => not_found(),
        Err(e) => store_error(&e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nextcloud_addresses_route_to_the_right_place() {
        let r = |p: &str| route(p).map(|x| x.ok());
        assert_eq!(
            r("/remote.php/dav/principals/users/eva/"),
            Some(Some(Place::Principal("eva".into())))
        );
        assert_eq!(
            r("/remote.php/dav/calendars/eva/personal/a%20b.ics"),
            Some(Some(Place::Object(
                Kind::Calendar,
                "eva".into(),
                "personal".into(),
                "a b.ics".into()
            )))
        );
        assert_eq!(
            r("/remote.php/dav/addressbooks/users/eva/contacts/"),
            Some(Some(Place::Collection(
                Kind::AddressBook,
                "eva".into(),
                "contacts".into()
            )))
        );
        assert_eq!(r("/remote.php/dav/files/eva/x"), None);
        assert_eq!(r("/remote.php/dav/calendars/eva/a/b/c"), Some(None));
    }

    #[test]
    fn a_time_range_keeps_what_overlaps() {
        let o = |first: Option<i64>, last: Option<i64>| Object {
            id: 1,
            uri: "x".into(),
            data: String::new(),
            etag: String::new(),
            component: None,
            uid: None,
            first,
            last,
            modified: 0,
        };
        assert!(in_window(&o(Some(10), Some(20)), Some(15), Some(30)));
        assert!(!in_window(&o(Some(10), Some(20)), Some(20), Some(30)));
        assert!(!in_window(&o(Some(30), Some(40)), Some(0), Some(30)));
        assert!(in_window(&o(Some(10), None), Some(1000), Some(2000)));
        assert!(in_window(&o(None, None), Some(1000), Some(2000)));
        assert!(in_window(&o(Some(20), Some(20)), Some(20), Some(30)));
    }
}
