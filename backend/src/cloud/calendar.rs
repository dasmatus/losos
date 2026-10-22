//! LosOS Calendar: a person's calendars in LosOS Cloud, read and written
//! as events for the Calendar page (`/calendar` in the admin UI,
//! `admin-ui/app/src/calendar/`), under `/cloud/api/calendar/…`.
//!
//! There is one store. The calendars are the collections
//! [`crate::cloud::pim`] keeps and [`crate::cloud::caldav`] serves, so a
//! phone or desktop calendar synced with the box and the Calendar page see
//! the same events, and the calendars LosOS Cloud took over from Nextcloud
//! are there from the start. An event's ID is its calendar object's row in
//! `dav_object`; an edit made here rewrites only the lines the page has
//! fields for ([`ical::rewrite`]).
//!
//! Three more kinds of calendar sit beside the person's own:
//!
//! * a **subscription** is a calendar filled from a feed (`webcal://` or
//!   `https://`, tax deadlines, an EduPage timetable), read again on the
//!   schedule the person picked ([`refresh_due`], through the fetcher in
//!   [`crate::cloud::webcal`]). Its events are the feed's: the page and
//!   CalDAV both refuse to change them ([`read_only`]), and phones see
//!   them like any other calendar;
//! * an **import** is a one-off `.ics` file, its events written into one of
//!   the person's calendars, a second import of the same file updating
//!   rather than doubling them;
//! * a **published** calendar is one of the person's own served to anyone
//!   with its secret address, `/cloud/feed/<token>.ics` ([`feed`]), which
//!   a phone or another calendar app subscribes to as `webcal://`.
//!
//! Invitations travel by LosOS Mail as iCalendar (RFC 5546): the page asks
//! here for the `METHOD:REQUEST` text of an event ([`invite`]) and mails
//! it; a guest's page reads one out of a message ([`read_invitation`]),
//! keeps their answer in their own calendar and mails back the
//! `METHOD:REPLY` ([`answer`]), which the organizer's page records
//! ([`apply`]).

use crate::cloud::auth::Who;
use crate::cloud::pim::{self, Collection, Expect, Kind, Object, PutError};
use crate::cloud::store::{Store, StoreError};
use crate::cloud::util::{now, random_token, sha1_hex};
use crate::cloud::webcal;
use crate::cloud::Cloud;
use crate::ical::{self, Event, Fields, Person, Repeat, Time};
use actix_web::http::StatusCode;
use actix_web::{web, HttpRequest, HttpResponse};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;

pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS calendar_feed (
    collection    INTEGER PRIMARY KEY REFERENCES dav_collection (id) ON DELETE CASCADE,
    url           TEXT NOT NULL,
    refresh       INTEGER NOT NULL,
    fetched       INTEGER,
    ok            INTEGER,
    error         TEXT NOT NULL DEFAULT '',
    etag          TEXT NOT NULL DEFAULT '',
    last_modified TEXT NOT NULL DEFAULT ''
);
CREATE TABLE IF NOT EXISTS calendar_publish (
    collection INTEGER PRIMARY KEY REFERENCES dav_collection (id) ON DELETE CASCADE,
    token      TEXT NOT NULL UNIQUE,
    created    INTEGER NOT NULL
);
"#;

/// The most events one import or one feed brings.
pub const MAX_EVENTS: usize = 5000;
/// The largest `.ics` an import reads.
pub const MAX_FILE_BYTES: usize = 10 * 1024 * 1024;
/// The most calendars, subscriptions included, one person keeps.
pub const MAX_CALENDARS: usize = 50;
/// The longest window one events call covers, in days.
pub const MAX_WINDOW_DAYS: i64 = 400;
/// How often a subscription may be read, at most: every 15 minutes.
pub const MIN_REFRESH: i64 = 15 * 60;
/// What a new subscription is read again after, unless the person picks.
pub const DEFAULT_REFRESH: i64 = 86_400;
const MAX_JSON: usize = 1 << 20;

/// Why Calendar could not do what was asked.
#[derive(Debug, thiserror::Error)]
pub enum CalendarError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("there is no such event")]
    NotFound,
    #[error("there is no such calendar")]
    NoCalendar,
    #[error("the event changed since it was opened; open it again")]
    Changed,
    #[error("this calendar comes from a subscription; its events change only in the feed")]
    ReadOnly,
    #[error("{0}")]
    Bad(String),
    #[error("{0}")]
    Feed(String),
}

impl CalendarError {
    pub fn status(&self) -> StatusCode {
        match self {
            CalendarError::Store(StoreError::NotFound)
            | CalendarError::NotFound
            | CalendarError::NoCalendar => StatusCode::NOT_FOUND,
            CalendarError::Store(StoreError::Precondition) | CalendarError::Changed => {
                StatusCode::PRECONDITION_FAILED
            }
            CalendarError::Store(StoreError::Exists) => StatusCode::CONFLICT,
            CalendarError::Store(_) => StatusCode::INTERNAL_SERVER_ERROR,
            CalendarError::ReadOnly => StatusCode::FORBIDDEN,
            CalendarError::Bad(_) => StatusCode::BAD_REQUEST,
            CalendarError::Feed(_) => StatusCode::BAD_GATEWAY,
        }
    }
}

impl From<PutError> for CalendarError {
    fn from(e: PutError) -> Self {
        match e {
            PutError::Store(StoreError::Precondition) => CalendarError::Changed,
            PutError::Store(e) => CalendarError::Store(e),
            PutError::Invalid | PutError::Unsupported => {
                CalendarError::Bad("the event could not be written".into())
            }
            PutError::UidConflict(_) => {
                CalendarError::Bad("another event in this calendar has the same UID".into())
            }
        }
    }
}

type Result<T> = std::result::Result<T, CalendarError>;

pub fn init(store: &Store) -> std::result::Result<(), StoreError> {
    store.with_db(|db| Ok(db.execute_batch(SCHEMA)?))
}

// ── Calendars ──────────────────────────────────────────────────────────────

/// A subscription's feed, as the page shows it.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Feed {
    pub url: String,
    /// Seconds between reads.
    pub refresh: i64,
    /// The last read, tried and good, in milliseconds since the epoch.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fetched: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ok: Option<i64>,
    /// Why the last read failed; empty when it did not.
    pub error: String,
    #[serde(skip)]
    etag: String,
    #[serde(skip)]
    last_modified: String,
}

/// One calendar as the page sees it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarView {
    pub uri: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    pub count: i64,
    /// Events here change only in the feed.
    pub read_only: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feed: Option<Feed>,
    /// `/cloud/feed/<token>.ics` while it is published.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published: Option<String>,
}

fn feed_of(store: &Store, id: i64) -> Result<Option<Feed>> {
    Ok(store.with_db(|db| {
        Ok(db
            .query_row(
                "SELECT url, refresh, fetched, ok, error, etag, last_modified FROM calendar_feed WHERE collection = ?1",
                params![id],
                |r| {
                    Ok(Feed {
                        url: r.get(0)?,
                        refresh: r.get(1)?,
                        fetched: r.get::<_, Option<i64>>(2)?.map(|s| s * 1000),
                        ok: r.get::<_, Option<i64>>(3)?.map(|s| s * 1000),
                        error: r.get(4)?,
                        etag: r.get(5)?,
                        last_modified: r.get(6)?,
                    })
                },
            )
            .optional()?)
    })?)
}

fn token_of(store: &Store, id: i64) -> Result<Option<String>> {
    Ok(store.with_db(|db| {
        Ok(db
            .query_row(
                "SELECT token FROM calendar_publish WHERE collection = ?1",
                params![id],
                |r| r.get(0),
            )
            .optional()?)
    })?)
}

/// Whether a calendar's events come from a feed, and so are not to be
/// changed here or over CalDAV.
pub fn read_only(store: &Store, collection: i64) -> bool {
    store
        .with_db(|db| {
            Ok(db
                .query_row(
                    "SELECT 1 FROM calendar_feed WHERE collection = ?1",
                    params![collection],
                    |_| Ok(()),
                )
                .optional()?
                .is_some())
        })
        .unwrap_or(false)
}

fn view(store: &Store, c: &Collection) -> Result<CalendarView> {
    let feed = feed_of(store, c.id)?;
    Ok(CalendarView {
        uri: c.uri.clone(),
        name: c.name.clone(),
        color: c.color.clone(),
        count: pim::count(store, c)?,
        read_only: feed.is_some(),
        feed,
        published: token_of(store, c.id)?.map(|t| format!("/cloud/feed/{t}.ics")),
    })
}

/// The person's calendars that hold events, the first made if they have
/// none.
pub fn calendars(store: &Store, login: &str) -> Result<Vec<Collection>> {
    pim::ensure_defaults(store, login)?;
    Ok(pim::collections(store, login, Kind::Calendar)?
        .into_iter()
        .filter(|c| c.components.split(',').any(|x| x.trim() == "VEVENT"))
        .collect())
}

pub fn list(store: &Store, login: &str) -> Result<Vec<CalendarView>> {
    calendars(store, login)?
        .iter()
        .map(|c| view(store, c))
        .collect()
}

fn calendar(store: &Store, login: &str, uri: &str) -> Result<Collection> {
    calendars(store, login)?
        .into_iter()
        .find(|c| c.uri == uri)
        .ok_or(CalendarError::NoCalendar)
}

/// A colour the page may set: `#rrggbb`.
fn valid_color(c: &str) -> bool {
    c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|x| x.is_ascii_hexdigit())
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CalendarFields {
    pub name: Option<String>,
    pub color: Option<String>,
    /// Subscriptions: seconds between reads.
    pub refresh: Option<i64>,
}

fn clean_name(name: &str) -> Result<String> {
    let n: String = name
        .trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(100)
        .collect();
    if n.is_empty() {
        return Err(CalendarError::Bad("a calendar needs a name".into()));
    }
    Ok(n)
}

fn clean_color(color: Option<&str>) -> Result<Option<String>> {
    match color.map(str::trim) {
        None | Some("") => Ok(None),
        Some(c) if valid_color(c) => Ok(Some(c.to_ascii_lowercase())),
        Some(_) => Err(CalendarError::Bad("a colour is #rrggbb".into())),
    }
}

fn make(
    store: &Store,
    login: &str,
    prefix: &str,
    name: &str,
    color: Option<String>,
) -> Result<Collection> {
    if calendars(store, login)?.len() >= MAX_CALENDARS {
        return Err(CalendarError::Bad(format!(
            "a person keeps at most {MAX_CALENDARS} calendars"
        )));
    }
    let uri = format!("{prefix}-{}", random_token(8).to_ascii_lowercase());
    Ok(pim::create(
        store,
        login,
        Kind::Calendar,
        &uri,
        &pim::CollectionFields {
            name: Some(name.to_owned()),
            color,
            components: Some("VEVENT,VTODO".into()),
            ..Default::default()
        },
    )?)
}

pub fn create(store: &Store, login: &str, f: &CalendarFields) -> Result<CalendarView> {
    let name = clean_name(f.name.as_deref().unwrap_or(""))?;
    let c = make(
        store,
        login,
        "calendar",
        &name,
        clean_color(f.color.as_deref())?,
    )?;
    view(store, &c)
}

pub fn change(store: &Store, login: &str, uri: &str, f: &CalendarFields) -> Result<CalendarView> {
    let c = calendar(store, login, uri)?;
    let fields = pim::CollectionFields {
        name: f.name.as_deref().map(clean_name).transpose()?,
        color: match f.color.as_deref() {
            None => None,
            Some(c) => Some(clean_color(Some(c))?.unwrap_or_default()),
        },
        ..Default::default()
    };
    pim::update(store, &c, &fields)?;
    if let Some(r) = f.refresh {
        if !read_only(store, c.id) {
            return Err(CalendarError::Bad(
                "only a subscription is read again".into(),
            ));
        }
        store.with_db(|db| {
            db.execute(
                "UPDATE calendar_feed SET refresh = ?1 WHERE collection = ?2",
                params![r.clamp(MIN_REFRESH, 30 * 86_400), c.id],
            )?;
            Ok(())
        })?;
    }
    view(store, &calendar(store, login, uri)?)
}

pub fn remove(store: &Store, login: &str, uri: &str) -> Result<()> {
    let c = calendar(store, login, uri)?;
    if calendars(store, login)?.len() <= 1 {
        return Err(CalendarError::Bad("the last calendar stays".into()));
    }
    pim::delete_collection(store, &c)?;
    Ok(())
}

pub fn publish(store: &Store, login: &str, uri: &str, on: bool) -> Result<CalendarView> {
    let c = calendar(store, login, uri)?;
    if on && read_only(store, c.id) {
        return Err(CalendarError::Bad(
            "a subscription is someone else's calendar; publish your own".into(),
        ));
    }
    store.with_db(|db| {
        if on {
            db.execute(
                "INSERT OR IGNORE INTO calendar_publish (collection, token, created) VALUES (?1, ?2, ?3)",
                params![c.id, random_token(32), now()],
            )?;
        } else {
            db.execute("DELETE FROM calendar_publish WHERE collection = ?1", params![c.id])?;
        }
        Ok(())
    })?;
    view(store, &c)
}

/// A published calendar as one `.ics`, for anyone with its address.
pub fn feed(store: &Store, token: &str) -> Result<String> {
    if token.len() < 16 || !token.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(CalendarError::NotFound);
    }
    let c: Option<Collection> = store.with_db(|db| {
        Ok(db
            .query_row(
                "SELECT c.owner, c.uri FROM calendar_publish p JOIN dav_collection c ON c.id = p.collection WHERE p.token = ?1",
                params![token],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )
            .optional()?)
    })?
    .map(|(owner, uri)| pim::collection(store, &owner, Kind::Calendar, &uri))
    .transpose()?;
    let c = c.ok_or(CalendarError::NotFound)?;
    let objects: Vec<String> = pim::objects(store, &c)?
        .into_iter()
        .filter(|o| o.component.as_deref() == Some("VEVENT"))
        .map(|o| o.data)
        .collect();
    Ok(ical::bundle(
        &c.name,
        c.color.as_deref().unwrap_or(""),
        &objects,
    ))
}

// ── Subscriptions ──────────────────────────────────────────────────────────

/// What a read of a feed did to its calendar.
#[derive(Debug, Default, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub added: usize,
    pub updated: usize,
    pub removed: usize,
    /// Events that could not be written.
    pub skipped: usize,
    /// Events past [`MAX_EVENTS`].
    pub left_out: usize,
}

/// The name an object of a UID is stored under in a feed's calendar: the
/// same every read, whatever the feed's own UIDs look like.
fn feed_uri(uid: &str) -> String {
    format!("{}.ics", &sha1_hex(uid.as_bytes())[..32])
}

/// Put a feed's text into its calendar: new events in, changed ones
/// rewritten, ones the feed no longer has out. Nothing changes when the
/// text is not a calendar.
fn apply_feed(store: &Store, c: &Collection, body: &[u8]) -> Result<Report> {
    let text = String::from_utf8_lossy(body);
    let total = ical::split(&text, usize::MAX).len();
    let pieces = ical::split(&text, MAX_EVENTS);
    if pieces.is_empty()
        && !text
            .trim_start()
            .to_ascii_uppercase()
            .starts_with("BEGIN:VCALENDAR")
    {
        return Err(CalendarError::Feed(
            "the address did not answer with a calendar".into(),
        ));
    }
    let mut report = Report {
        left_out: total.saturating_sub(pieces.len()),
        ..Report::default()
    };
    let old: HashMap<String, String> = pim::objects(store, c)?
        .into_iter()
        .map(|o| (o.uri, o.etag))
        .collect();
    let mut kept = std::collections::HashSet::new();
    for p in &pieces {
        let uri = feed_uri(&p.uid);
        kept.insert(uri.clone());
        let etag = format!("\"{}\"", sha1_hex(p.text.as_bytes()));
        if old.get(&uri) == Some(&etag) {
            continue;
        }
        match pim::put(store, c, &uri, &p.text, Expect::Any) {
            Ok((_, true)) => report.added += 1,
            Ok((_, false)) => report.updated += 1,
            Err(PutError::Store(e)) => return Err(e.into()),
            Err(_) => report.skipped += 1,
        }
    }
    for uri in old.keys() {
        if !kept.contains(uri) {
            pim::delete_object(store, c, uri, None)?;
            report.removed += 1;
        }
    }
    Ok(report)
}

/// Read a subscription's feed now and put what it says in its calendar.
pub fn refresh(cloud: &Cloud, c: &Collection) -> Result<Report> {
    let store = &cloud.store;
    let f = feed_of(store, c.id)?.ok_or(CalendarError::NoCalendar)?;
    let answer = webcal::fetch(
        cloud.config.webcal_socket.as_deref(),
        &webcal::Request {
            url: f.url.clone(),
            etag: f.etag.clone(),
            last_modified: f.last_modified.clone(),
        },
    );
    let t = now();
    let outcome = if !answer.error.is_empty() {
        Err(CalendarError::Feed(answer.error.clone()))
    } else if answer.status == 304 {
        Ok(Report::default())
    } else if !(200..300).contains(&answer.status) {
        Err(CalendarError::Feed(format!(
            "the server answered {}",
            answer.status
        )))
    } else {
        apply_feed(store, c, &answer.body)
    };
    store.with_db(|db| {
        match &outcome {
            Ok(_) => db.execute(
                "UPDATE calendar_feed SET fetched = ?1, ok = ?1, error = '', etag = ?2, last_modified = ?3 WHERE collection = ?4",
                params![t, answer.etag, answer.last_modified, c.id],
            )?,
            Err(e) => db.execute(
                "UPDATE calendar_feed SET fetched = ?1, error = ?2 WHERE collection = ?3",
                params![t, e.to_string(), c.id],
            )?,
        };
        Ok(())
    })?;
    outcome
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Subscription {
    pub url: String,
    pub name: Option<String>,
    pub color: Option<String>,
    pub refresh: Option<i64>,
}

/// Subscribe to a feed: read it once, and keep it only if that worked.
pub fn subscribe(cloud: &Cloud, login: &str, s: &Subscription) -> Result<(CalendarView, Report)> {
    let store = &cloud.store;
    let url = webcal::normalize(&s.url).map_err(CalendarError::Bad)?;
    let color = clean_color(s.color.as_deref())?;
    // The same address twice is the same subscription, read again now.
    if let Some(c) = subscribed_to(store, login, &url)? {
        let report = refresh(cloud, &c)?;
        return Ok((view(store, &c)?, report));
    }
    let given = s.name.as_deref().map(str::trim).filter(|n| !n.is_empty());
    let c = make(
        store,
        login,
        "webcal",
        given.unwrap_or("Subscription"),
        color,
    )?;
    let refresh_secs = s
        .refresh
        .unwrap_or(DEFAULT_REFRESH)
        .clamp(MIN_REFRESH, 30 * 86_400);
    store.with_db(|db| {
        db.execute(
            "INSERT INTO calendar_feed (collection, url, refresh) VALUES (?1, ?2, ?3)",
            params![c.id, url, refresh_secs],
        )?;
        Ok(())
    })?;
    let first = webcal::fetch(
        cloud.config.webcal_socket.as_deref(),
        &webcal::Request {
            url: url.clone(),
            ..Default::default()
        },
    );
    let failed = |e: CalendarError| -> Result<(CalendarView, Report)> {
        pim::delete_collection(store, &c)?;
        Err(e)
    };
    if !first.error.is_empty() {
        return failed(CalendarError::Feed(first.error));
    }
    if !(200..300).contains(&first.status) {
        return failed(CalendarError::Feed(format!(
            "the server answered {}",
            first.status
        )));
    }
    let report = match apply_feed(store, &c, &first.body) {
        Ok(r) => r,
        Err(e) => return failed(e),
    };
    // The feed's own name, when the person gave none.
    if given.is_none() {
        let cal = ical::parse(&String::from_utf8_lossy(&first.body));
        let name: String = cal
            .name
            .chars()
            .filter(|c| !c.is_control())
            .take(100)
            .collect();
        let host = url::Url::parse(&url.replacen("webcal", "https", 1))
            .ok()
            .and_then(|u| u.host_str().map(str::to_owned))
            .unwrap_or_default();
        let name = if name.trim().is_empty() { host } else { name };
        if !name.trim().is_empty() {
            pim::update(
                store,
                &c,
                &pim::CollectionFields {
                    name: Some(name.trim().to_owned()),
                    ..Default::default()
                },
            )?;
        }
    }
    let t = now();
    store.with_db(|db| {
        db.execute(
            "UPDATE calendar_feed SET fetched = ?1, ok = ?1, etag = ?2, last_modified = ?3 WHERE collection = ?4",
            params![t, first.etag, first.last_modified, c.id],
        )?;
        Ok(())
    })?;
    let c = pim::collection(store, login, Kind::Calendar, &c.uri)?;
    Ok((view(store, &c)?, report))
}

/// Keep a subscription without reading it now: the next pass of
/// [`refresh_due`] does. For one carried over from another server; the
/// person's subscription to the same address, when there is one.
/// The person's subscription to `url` (normalized), if they have one.
fn subscribed_to(store: &Store, login: &str, url: &str) -> Result<Option<Collection>> {
    let held: Option<String> = store.with_db(|db| {
        Ok(db
            .query_row(
                "SELECT c.uri FROM calendar_feed f JOIN dav_collection c ON c.id = f.collection
                 WHERE c.owner = ?1 AND f.url = ?2",
                params![login, url],
                |r| r.get(0),
            )
            .optional()?)
    })?;
    match held {
        Some(uri) => Ok(Some(pim::collection(store, login, Kind::Calendar, &uri)?)),
        None => Ok(None),
    }
}

pub fn adopt(store: &Store, login: &str, s: &Subscription) -> Result<Collection> {
    let url = webcal::normalize(&s.url).map_err(CalendarError::Bad)?;
    if let Some(c) = subscribed_to(store, login, &url)? {
        return Ok(c);
    }
    let color = clean_color(s.color.as_deref()).unwrap_or(None);
    let name = s
        .name
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .unwrap_or("Subscription");
    let c = make(store, login, "webcal", name, color)?;
    let refresh = s
        .refresh
        .unwrap_or(DEFAULT_REFRESH)
        .clamp(MIN_REFRESH, 30 * 86_400);
    store.with_db(|db| {
        db.execute(
            "INSERT INTO calendar_feed (collection, url, refresh) VALUES (?1, ?2, ?3)",
            params![c.id, url, refresh],
        )?;
        Ok(())
    })?;
    Ok(c)
}

/// Read every subscription whose time has come. The serve loop calls it
/// once a minute; one feed's failure is kept on that feed and the rest go on.
pub fn refresh_due(cloud: &Cloud) -> usize {
    let t = now();
    let due: Vec<(String, String)> = cloud
        .store
        .with_db(|db| {
            let mut st = db.prepare(
                "SELECT c.owner, c.uri FROM calendar_feed f JOIN dav_collection c ON c.id = f.collection
                 WHERE f.fetched IS NULL OR f.fetched + f.refresh <= ?1 ORDER BY f.fetched",
            )?;
            let rows = st.query_map(params![t], |r| Ok((r.get(0)?, r.get(1)?)))?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
        .unwrap_or_default();
    let mut n = 0;
    for (owner, uri) in due {
        let Ok(c) = pim::collection(&cloud.store, &owner, Kind::Calendar, &uri) else {
            continue;
        };
        match refresh(cloud, &c) {
            Ok(r) => {
                n += 1;
                if r != Report::default() {
                    tracing::info!(
                        owner,
                        calendar = uri,
                        added = r.added,
                        updated = r.updated,
                        removed = r.removed,
                        "calendar: subscription read"
                    );
                }
            }
            Err(e) => tracing::warn!(owner, calendar = uri, "calendar: subscription: {e}"),
        }
    }
    n
}

// ── Imports ────────────────────────────────────────────────────────────────

fn object_by_uid(store: &Store, c: &Collection, uid: &str) -> Result<Option<String>> {
    Ok(store.with_db(|db| {
        Ok(db
            .query_row(
                "SELECT uri FROM dav_object WHERE collection = ?1 AND uid = ?2",
                params![c.id, uid],
                |r| r.get(0),
            )
            .optional()?)
    })?)
}

/// A name for a new object of `uid`: the UID itself where it is a valid
/// name, else a digest of it.
fn object_uri(uid: &str) -> String {
    let plain = format!("{uid}.ics");
    if pim::valid_uri(&plain) && !uid.contains(' ') && plain.len() <= 200 {
        plain
    } else {
        feed_uri(uid)
    }
}

/// An `.ics` file's events into one of the person's calendars. An event
/// whose UID is there already is rewritten, so the same file twice adds
/// nothing.
pub fn import(
    store: &Store,
    login: &str,
    uri: Option<&str>,
    bytes: &[u8],
) -> Result<(CalendarView, Report)> {
    let c = match uri {
        Some(u) => calendar(store, login, u)?,
        None => calendars(store, login)?
            .into_iter()
            .find(|c| !read_only(store, c.id))
            .ok_or(CalendarError::NoCalendar)?,
    };
    if read_only(store, c.id) {
        return Err(CalendarError::ReadOnly);
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|_| CalendarError::Bad("the file is not UTF-8 text".into()))?;
    if !text
        .trim_start()
        .to_ascii_uppercase()
        .starts_with("BEGIN:VCALENDAR")
    {
        return Err(CalendarError::Bad(
            "the file is not an iCalendar (.ics) file".into(),
        ));
    }
    let total = ical::split(text, usize::MAX).len();
    let pieces = ical::split(text, MAX_EVENTS);
    let mut report = Report {
        left_out: total.saturating_sub(pieces.len()),
        ..Report::default()
    };
    for p in &pieces {
        let at = object_by_uid(store, &c, &p.uid)?.unwrap_or_else(|| object_uri(&p.uid));
        match pim::put(store, &c, &at, &p.text, Expect::Any) {
            Ok((_, true)) => report.added += 1,
            Ok((_, false)) => report.updated += 1,
            Err(PutError::Store(e)) => return Err(e.into()),
            Err(_) => report.skipped += 1,
        }
    }
    Ok((view(store, &calendar(store, login, &c.uri)?)?, report))
}

// ── Events ─────────────────────────────────────────────────────────────────

/// An event as the page sees it: one occurrence of it, when listed.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventView {
    pub id: String,
    pub calendar: String,
    pub uid: String,
    pub summary: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub location: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// `2026-10-22` or `2026-10-22T08:00:00`, in `zone`.
    pub start: String,
    pub end: String,
    /// `UTC`, an IANA zone, or none: the reader's own (and all-day).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zone: Option<String>,
    pub all_day: bool,
    /// How it repeats, where the page's fields can say it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repeat: Option<Repeat>,
    /// It repeats, in a way the page shows but does not edit.
    pub repeats: bool,
    /// The start of this occurrence, for a repeat: what a delete of just
    /// this one names.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub occurrence: Option<String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organizer: Option<Person>,
    pub attendees: Vec<Person>,
    /// Reminders, in minutes before the start.
    pub alarms: Vec<i64>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub call: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub url: String,
    pub read_only: bool,
    /// The stored object's ETag, which a change sends back as `If-Match`.
    pub etag: String,
}

fn event_view(c: &Collection, ro: bool, o: &Object, e: &Event, at: Option<i64>) -> EventView {
    let start = Time {
        at: at.unwrap_or(e.start.at),
        zone: e.start.zone.clone(),
    };
    let end = Time {
        at: start.at + e.length,
        zone: e.start.zone.clone(),
    };
    let recurring = e.rule.is_some() || e.rrule.is_some() || !e.rdates.is_empty();
    EventView {
        id: o.id.to_string(),
        calendar: c.uri.clone(),
        uid: e.uid.clone(),
        summary: e.summary.clone(),
        location: e.location.clone(),
        description: e.description.clone(),
        start: start.local(),
        end: end.local(),
        zone: e.start.zone_name(),
        all_day: e.start.is_date(),
        repeat: e.rule.as_ref().and_then(Repeat::of),
        repeats: recurring,
        occurrence: (recurring && at.is_some()).then(|| start.local()),
        status: e.status.clone(),
        organizer: e.organizer.clone(),
        attendees: e.attendees.clone(),
        alarms: e
            .alarms
            .iter()
            .filter(|s| **s >= 0)
            .map(|s| s / 60)
            .collect(),
        call: e.call.clone(),
        url: e.url.clone(),
        read_only: ro,
        etag: o.etag.clone(),
    }
}

/// A day `2026-10-22` as wall-clock seconds.
fn day_of(s: &str) -> Result<i64> {
    Time::from_page(s, None)
        .filter(Time::is_date)
        .map(|t| t.at)
        .ok_or_else(|| CalendarError::Bad(format!("{s} is not a day (2026-10-22)")))
}

/// Every occurrence of every event in the person's calendars that falls
/// in `[from, to)` (days), give or take a day of zone offset, which the
/// page trims. Cancelled ones are left out.
pub fn occurrences(store: &Store, login: &str, from: &str, to: &str) -> Result<Vec<EventView>> {
    let (from, to) = (day_of(from)?, day_of(to)?);
    if to <= from || (to - from) / 86_400 > MAX_WINDOW_DAYS {
        return Err(CalendarError::Bad(format!(
            "a window is one to {MAX_WINDOW_DAYS} days"
        )));
    }
    let (wide_from, wide_to) = (from - ical::ZONE_SLACK, to + ical::ZONE_SLACK);
    let mut out = Vec::new();
    for c in calendars(store, login)? {
        let ro = read_only(store, c.id);
        for o in pim::objects(store, &c)? {
            if o.component.as_deref() != Some("VEVENT") {
                continue;
            }
            // `first`/`last` are wall clock read as UTC, the same scale.
            if o.first.is_some_and(|f| f >= wide_to) || o.last.is_some_and(|l| l < wide_from) {
                continue;
            }
            let cal = ical::parse(&o.data);
            let instances: Vec<&Event> = cal
                .events
                .iter()
                .filter(|e| e.recurrence_id.is_some())
                .collect();
            let skip: Vec<i64> = instances
                .iter()
                .filter_map(|e| e.recurrence_id.as_ref().map(|r| r.at))
                .collect();
            for e in &cal.events {
                if e.status == "CANCELLED" {
                    continue;
                }
                if e.recurrence_id.is_some() {
                    if e.start.at < wide_to && e.start.at + e.length.max(1) > wide_from {
                        let mut v = event_view(&c, ro, &o, e, Some(e.start.at));
                        v.repeats = true;
                        v.occurrence = e.recurrence_id.as_ref().map(Time::local);
                        out.push(v);
                    }
                    continue;
                }
                for at in ical::starts(e, wide_from, wide_to, &skip, ical::MAX_OCCURRENCES) {
                    out.push(event_view(&c, ro, &o, e, Some(at)));
                }
            }
            if out.len() > 20_000 {
                return Err(CalendarError::Bad("too many events in that window".into()));
            }
        }
    }
    out.sort_by(|a, b| a.start.cmp(&b.start).then(a.summary.cmp(&b.summary)));
    Ok(out)
}

/// One event by ID, with its calendar and object.
fn find(store: &Store, login: &str, id: &str) -> Result<(Collection, Object)> {
    let id: i64 = id.parse().map_err(|_| CalendarError::NotFound)?;
    for c in calendars(store, login)? {
        let found: Option<String> = store.with_db(|db| {
            Ok(db
                .query_row(
                    "SELECT uri FROM dav_object WHERE id = ?1 AND collection = ?2",
                    params![id, c.id],
                    |r| r.get(0),
                )
                .optional()?)
        })?;
        if let Some(uri) = found {
            let o = pim::object(store, &c, &uri)?;
            return Ok((c, o));
        }
    }
    Err(CalendarError::NotFound)
}

fn main_event(o: &Object) -> Result<Event> {
    ical::parse(&o.data)
        .events
        .into_iter()
        .find(|e| e.recurrence_id.is_none())
        .ok_or(CalendarError::NotFound)
}

pub fn get(store: &Store, login: &str, id: &str) -> Result<EventView> {
    let (c, o) = find(store, login, id)?;
    let e = main_event(&o)?;
    Ok(event_view(&c, read_only(store, c.id), &o, &e, None))
}

/// An event as the page saves it: its fields, and which calendar.
#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Save {
    pub calendar: String,
    #[serde(flatten)]
    pub fields: Fields,
}

fn writable(store: &Store, login: &str, uri: &str) -> Result<Collection> {
    let c = if uri.is_empty() {
        calendars(store, login)?
            .into_iter()
            .find(|c| !read_only(store, c.id))
            .ok_or(CalendarError::NoCalendar)?
    } else {
        calendar(store, login, uri)?
    };
    if read_only(store, c.id) {
        return Err(CalendarError::ReadOnly);
    }
    Ok(c)
}

pub fn save(
    store: &Store,
    login: &str,
    id: Option<&str>,
    s: &Save,
    if_match: Option<&str>,
) -> Result<EventView> {
    let target = writable(store, login, &s.calendar)?;
    let t = now();
    // The ETag the stored copy had, which the write must still find.
    let (c, uri, text, held) = match id {
        None => {
            let uid = format!("{}@losos", random_token(24).to_ascii_lowercase());
            let text = ical::create(&uid, &s.fields, t).map_err(CalendarError::Bad)?;
            (target, object_uri(&uid), text, None)
        }
        Some(id) => {
            let (c, o) = find(store, login, id)?;
            if read_only(store, c.id) {
                return Err(CalendarError::ReadOnly);
            }
            if if_match.is_some_and(|m| m != o.etag) {
                return Err(CalendarError::Changed);
            }
            let text = ical::rewrite(&o.data, &s.fields, t).map_err(CalendarError::Bad)?;
            if c.id != target.id {
                // Moved to another calendar: written there, then gone here.
                let uid = main_event(&o)?.uid;
                let at = object_uri(&uid);
                pim::put(store, &target, &at, &text, Expect::New)?;
                pim::delete_object(store, &c, &o.uri, Some(&o.etag))?;
                let o = pim::object(store, &target, &at)?;
                let e = main_event(&o)?;
                return Ok(event_view(&target, false, &o, &e, None));
            }
            (c, o.uri, text, Some(o.etag))
        }
    };
    let expect = match &held {
        None => Expect::New,
        Some(e) => Expect::Etag(e),
    };
    pim::put(store, &c, &uri, &text, expect)?;
    let o = pim::object(store, &c, &uri)?;
    let e = main_event(&o)?;
    Ok(event_view(&c, false, &o, &e, None))
}

/// Delete an event, or with `occurrence` (its start, as listed) only that
/// occurrence of a repeat.
pub fn delete(store: &Store, login: &str, id: &str, occurrence: Option<&str>) -> Result<()> {
    let (c, o) = find(store, login, id)?;
    if read_only(store, c.id) {
        return Err(CalendarError::ReadOnly);
    }
    match occurrence {
        None => Ok(pim::delete_object(store, &c, &o.uri, Some(&o.etag))?),
        Some(at) => {
            let e = main_event(&o)?;
            let local = Time::from_page(at, None)
                .ok_or_else(|| CalendarError::Bad("that is not an occurrence's start".into()))?;
            let at = Time {
                at: local.at,
                zone: e.start.zone.clone(),
            };
            let text = ical::exclude(&o.data, &at).map_err(CalendarError::Bad)?;
            pim::put(store, &c, &o.uri, &text, Expect::Etag(&o.etag))?;
            Ok(())
        }
    }
}

// ── Invitations ────────────────────────────────────────────────────────────

/// An event's invitation, `METHOD:REQUEST`, for the page to mail.
pub fn invite(store: &Store, login: &str, id: &str) -> Result<String> {
    let (_, o) = find(store, login, id)?;
    let e = main_event(&o)?;
    if e.attendees.is_empty() {
        return Err(CalendarError::Bad(
            "the event has no guests to invite".into(),
        ));
    }
    Ok(ical::request(&o.data))
}

/// What a message's `.ics` says, and where it stands in the person's own
/// calendars.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Invitation {
    /// `REQUEST`, `REPLY`, `CANCEL`, `PUBLISH`, …
    pub method: String,
    pub summary: String,
    pub start: String,
    pub end: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zone: Option<String>,
    pub all_day: bool,
    pub repeats: bool,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub location: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organizer: Option<Person>,
    pub attendees: Vec<Person>,
    /// The event already in one of the person's calendars, by UID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stored: Option<EventView>,
    /// `email`'s answer as the stored copy has it, or as the invitation
    /// asks (`NEEDS-ACTION`).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub status: String,
}

fn stored_by_uid(store: &Store, login: &str, uid: &str) -> Result<Option<(Collection, Object)>> {
    for c in calendars(store, login)? {
        if let Some(uri) = object_by_uid(store, &c, uid)? {
            return Ok(Some((c.clone(), pim::object(store, &c, &uri)?)));
        }
    }
    Ok(None)
}

fn invitation_event(text: &str) -> Result<(ical::Calendar, Event)> {
    let cal = ical::parse(text);
    let e = cal
        .events
        .iter()
        .find(|e| e.recurrence_id.is_none())
        .or(cal.events.first())
        .cloned()
        .ok_or_else(|| CalendarError::Bad("the attachment holds no event".into()))?;
    if e.uid.is_empty() {
        return Err(CalendarError::Bad("the event has no UID".into()));
    }
    Ok((cal, e))
}

pub fn read_invitation(store: &Store, login: &str, text: &str, email: &str) -> Result<Invitation> {
    let (cal, e) = invitation_event(text)?;
    let email = email.trim().to_ascii_lowercase();
    let stored = stored_by_uid(store, login, &e.uid)?;
    let stored_view = stored.as_ref().and_then(|(c, o)| {
        main_event(o)
            .ok()
            .map(|se| event_view(c, read_only(store, c.id), o, &se, None))
    });
    let status = stored_view
        .as_ref()
        .and_then(|v| v.attendees.iter().find(|a| a.email == email))
        .or_else(|| e.attendees.iter().find(|a| a.email == email))
        .map(|a| a.status.clone())
        .unwrap_or_default();
    let end = Time {
        at: e.start.at + e.length,
        zone: e.start.zone.clone(),
    };
    Ok(Invitation {
        method: if cal.method.is_empty() {
            "PUBLISH".into()
        } else {
            cal.method.clone()
        },
        summary: e.summary.clone(),
        start: e.start.local(),
        end: end.local(),
        zone: e.start.zone_name(),
        all_day: e.start.is_date(),
        repeats: e.rrule.is_some(),
        location: e.location.clone(),
        organizer: e.organizer.clone(),
        attendees: e.attendees.clone(),
        stored: stored_view,
        status,
    })
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AnswerBody {
    /// The invitation as it came.
    pub ics: String,
    /// The person's own address, as the invitation names them.
    pub email: String,
    /// `ACCEPTED`, `TENTATIVE` or `DECLINED`.
    pub status: String,
    /// Where an accepted event goes; the first of the person's own if none.
    pub calendar: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Answered {
    /// The `METHOD:REPLY` to mail the organizer.
    pub reply: String,
    /// Who it goes to.
    pub to: String,
    pub subject: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event: Option<EventView>,
}

/// Answer an invitation: the event goes into (or stays in) the person's
/// calendar with their answer on it, a decline included, so a later
/// update finds it; the reply comes back for the page to mail.
pub fn answer(store: &Store, login: &str, a: &AnswerBody) -> Result<Answered> {
    let status = a.status.trim().to_ascii_uppercase();
    if !["ACCEPTED", "TENTATIVE", "DECLINED"].contains(&status.as_str()) {
        return Err(CalendarError::Bad(
            "an answer is ACCEPTED, TENTATIVE or DECLINED".into(),
        ));
    }
    let (cal, e) = invitation_event(&a.ics)?;
    if cal.method != "REQUEST" {
        return Err(CalendarError::Bad(
            "only an invitation (METHOD:REQUEST) is answered".into(),
        ));
    }
    let organizer = e
        .organizer
        .clone()
        .ok_or_else(|| CalendarError::Bad("the invitation names no organizer".into()))?;
    let mine = ical::with_partstat(&a.ics, &a.email, &status)
        .ok_or_else(|| CalendarError::Bad("the invitation is not addressed to you".into()))?;
    let reply = ical::reply(&a.ics, &a.email, &status, now())
        .ok_or_else(|| CalendarError::Bad("the invitation is not addressed to you".into()))?;
    let (c, uri, expect_old) = match stored_by_uid(store, login, &e.uid)? {
        Some((c, o)) => (c, o.uri, Some(o.etag)),
        None => {
            let c = writable(store, login, &a.calendar)?;
            (c, object_uri(&e.uid), None)
        }
    };
    if read_only(store, c.id) {
        return Err(CalendarError::ReadOnly);
    }
    let expect = match &expect_old {
        Some(tag) => Expect::Etag(tag),
        None => Expect::New,
    };
    pim::put(store, &c, &uri, &mine, expect)?;
    let o = pim::object(store, &c, &uri)?;
    let stored = main_event(&o)?;
    let word = match status.as_str() {
        "ACCEPTED" => "Accepted",
        "TENTATIVE" => "Tentative",
        _ => "Declined",
    };
    Ok(Answered {
        reply,
        to: organizer.email,
        subject: format!("{word}: {}", e.summary),
        event: Some(event_view(&c, false, &o, &stored, None)),
    })
}

/// Record what a guest's reply says, or an organizer's cancellation, on
/// the person's own copy of the event.
pub fn apply(store: &Store, login: &str, text: &str) -> Result<EventView> {
    let (cal, e) = invitation_event(text)?;
    let (c, o) = stored_by_uid(store, login, &e.uid)?.ok_or(CalendarError::NotFound)?;
    if read_only(store, c.id) {
        return Err(CalendarError::ReadOnly);
    }
    let updated = match cal.method.as_str() {
        "REPLY" => {
            let who = e
                .attendees
                .first()
                .ok_or_else(|| CalendarError::Bad("the reply names nobody".into()))?;
            let status = if who.status.is_empty() {
                "NEEDS-ACTION"
            } else {
                &who.status
            };
            ical::with_partstat(&o.data, &who.email, status).ok_or_else(|| {
                CalendarError::Bad(format!("{} is not a guest of this event", who.email))
            })?
        }
        "CANCEL" => {
            let mine = main_event(&o)?;
            if mine.organizer.as_ref().map(|p| &p.email) != e.organizer.as_ref().map(|p| &p.email) {
                return Err(CalendarError::Bad(
                    "only the organizer cancels an event".into(),
                ));
            }
            set_status(&o.data, "CANCELLED")
        }
        _ => {
            return Err(CalendarError::Bad(
                "only a reply or a cancellation is recorded".into(),
            ))
        }
    };
    pim::put(store, &c, &o.uri, &updated, Expect::Etag(&o.etag))?;
    let o = pim::object(store, &c, &o.uri)?;
    let me = main_event(&o)?;
    Ok(event_view(&c, false, &o, &me, None))
}

fn set_status(text: &str, status: &str) -> String {
    let mut out = Vec::new();
    let mut depth = 0usize;
    for l in ical::unfold(text) {
        let p = ical::Line::parse(&l);
        match p.name.as_str() {
            "BEGIN" => depth += 1,
            "END" => {
                if depth == 2 {
                    out.push(format!("STATUS:{status}"));
                }
                depth = depth.saturating_sub(1);
            }
            "STATUS" if depth == 2 => continue,
            _ => {}
        }
        out.push(l);
    }
    // Only a component's own STATUS was replaced: put each END:VEVENT's
    // new line before it, not before a VTIMEZONE's end.
    let mut fixed = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    for l in out {
        let p = ical::Line::parse(&l);
        if p.name == "BEGIN" {
            stack.push(p.value.trim().to_ascii_uppercase());
        }
        if l.starts_with("STATUS:") && stack.last().map(String::as_str) != Some("VEVENT") {
            continue;
        }
        if p.name == "END" {
            stack.pop();
        }
        fixed.push(l);
    }
    fixed.iter().map(|l| ical::fold(l)).collect()
}

// ── The routes ─────────────────────────────────────────────────────────────

fn refused(e: &CalendarError) -> HttpResponse {
    let status = e.status();
    if status.is_server_error() && status != StatusCode::BAD_GATEWAY {
        tracing::warn!(error = %e, "calendar");
    }
    HttpResponse::build(status).json(json!({ "error": e.to_string() }))
}

fn answer_json<T: Serialize>(r: Result<T>) -> HttpResponse {
    match r {
        Ok(v) => HttpResponse::Ok()
            .insert_header(("Cache-Control", "no-store"))
            .json(v),
        Err(e) => refused(&e),
    }
}

fn query(req: &HttpRequest) -> HashMap<String, String> {
    url::form_urlencoded::parse(req.query_string().as_bytes())
        .into_owned()
        .collect()
}

fn json_body<T: serde::de::DeserializeOwned>(raw: &[u8]) -> Result<T> {
    serde_json::from_slice(raw)
        .map_err(|e| CalendarError::Bad(format!("the body is not what was expected: {e}")))
}

/// A blocking piece of work (a feed read) off the server's threads.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    web::block(f)
        .await
        .unwrap_or_else(|_| Err(CalendarError::Feed("the read was cut short".into())))
}

/// The public feed of a published calendar: `/cloud/feed/<token>.ics`.
pub fn public(cloud: &Cloud, method: &str, path: &str) -> Option<HttpResponse> {
    let token = path.strip_prefix("/cloud/feed/")?.strip_suffix(".ics")?;
    if method != "GET" && method != "HEAD" {
        return Some(HttpResponse::MethodNotAllowed().finish());
    }
    Some(match feed(&cloud.store, token) {
        Ok(text) => {
            let etag = format!("\"{}\"", sha1_hex(text.as_bytes()));
            let mut b = HttpResponse::Ok();
            b.content_type("text/calendar; charset=utf-8")
                .insert_header(("ETag", etag))
                .insert_header(("Cache-Control", "private, max-age=300"))
                .insert_header(("X-Content-Type-Options", "nosniff"))
                .insert_header(("Content-Disposition", "inline; filename=\"calendar.ics\""));
            if method == "HEAD" {
                b.finish()
            } else {
                b.body(text)
            }
        }
        Err(_) => HttpResponse::NotFound().finish(),
    })
}

/// One call under `/cloud/api/calendar`. `rest` is what follows it.
///
/// ```text
/// GET    /calendar/calendars                {"calendars": [...]}
/// POST   /calendar/calendars                {name, color}: a new one
/// PUT    /calendar/calendars/<uri>          {name?, color?, refresh?}
/// DELETE /calendar/calendars/<uri>
/// POST   /calendar/calendars/<uri>/publish  its feed address, made
/// DELETE /calendar/calendars/<uri>/publish  and taken back
/// POST   /calendar/calendars/<uri>/refresh  a subscription, read now
/// POST   /calendar/subscriptions            {url, name?, color?, refresh?}
/// POST   /calendar/import?calendar=         an .ics as the body
/// GET    /calendar/events?from=&to=         occurrences, days
/// POST   /calendar/events                   {calendar, ...fields}
/// GET    /calendar/events/<id>              one, as it repeats
/// PUT    /calendar/events/<id>              If-Match: its ETag
/// DELETE /calendar/events/<id>?occurrence=  all of it, or one occurrence
/// GET    /calendar/events/<id>/invite       METHOD:REQUEST, to mail
/// POST   /calendar/invitations/read?email=  an .ics: what it asks
/// POST   /calendar/invitations/answer       {ics, email, status}
/// POST   /calendar/invitations/apply        a reply or cancellation
/// ```
pub async fn api(
    req: &HttpRequest,
    cloud: &std::sync::Arc<Cloud>,
    who: &Who,
    rest: &[&str],
    body: web::Payload,
) -> HttpResponse {
    let store = &cloud.store;
    let login = who.login.clone();
    let raw = |max: usize| crate::cloud::dav::read_body(body, max);
    match (req.method().as_str(), rest) {
        ("GET", ["calendars"]) => {
            answer_json(list(store, &login).map(|c| json!({ "calendars": c })))
        }
        ("POST", ["calendars"]) => match raw(MAX_JSON).await {
            Ok(b) => answer_json(json_body(&b).and_then(|f| create(store, &login, &f))),
            Err(r) => r,
        },
        ("PUT", ["calendars", uri]) => match raw(MAX_JSON).await {
            Ok(b) => answer_json(json_body(&b).and_then(|f| change(store, &login, uri, &f))),
            Err(r) => r,
        },
        ("DELETE", ["calendars", uri]) => {
            answer_json(remove(store, &login, uri).map(|()| json!({ "removed": uri })))
        }
        ("POST", ["calendars", uri, "publish"]) => answer_json(publish(store, &login, uri, true)),
        ("DELETE", ["calendars", uri, "publish"]) => {
            answer_json(publish(store, &login, uri, false))
        }
        ("POST", ["calendars", uri, "refresh"]) => {
            let (cloud, uri) = (cloud.clone(), uri.to_string());
            answer_json(
                blocking(move || {
                    let c = calendar(&cloud.store, &login, &uri)?;
                    if !read_only(&cloud.store, c.id) {
                        return Err(CalendarError::Bad(
                            "only a subscription is read again".into(),
                        ));
                    }
                    let report = refresh(&cloud, &c)?;
                    Ok(json!({ "calendar": view(&cloud.store, &c)?, "report": report }))
                })
                .await,
            )
        }
        ("POST", ["subscriptions"]) => match raw(MAX_JSON).await {
            Ok(b) => {
                let cloud = cloud.clone();
                answer_json(
                    blocking(move || {
                        let s: Subscription = json_body(&b)?;
                        let (c, report) = subscribe(&cloud, &login, &s)?;
                        Ok(json!({ "calendar": c, "report": report }))
                    })
                    .await,
                )
            }
            Err(r) => r,
        },
        ("POST", ["import"]) => match raw(MAX_FILE_BYTES).await {
            Ok(b) => {
                let q = query(req);
                let uri = q.get("calendar").filter(|s| !s.is_empty()).cloned();
                answer_json(
                    import(store, &login, uri.as_deref(), &b)
                        .map(|(c, report)| json!({ "calendar": c, "report": report })),
                )
            }
            Err(r) => r,
        },
        ("GET", ["events"]) => {
            let q = query(req);
            let from = q.get("from").map(String::as_str).unwrap_or("");
            let to = q.get("to").map(String::as_str).unwrap_or("");
            answer_json(occurrences(store, &login, from, to).map(|e| json!({ "events": e })))
        }
        ("POST", ["events"]) => match raw(MAX_JSON).await {
            Ok(b) => answer_json(json_body(&b).and_then(|s| save(store, &login, None, &s, None))),
            Err(r) => r,
        },
        ("GET", ["events", id]) => match get(store, &login, id) {
            Ok(v) => HttpResponse::Ok()
                .insert_header(("Cache-Control", "no-store"))
                .insert_header(("ETag", v.etag.clone()))
                .json(v),
            Err(e) => refused(&e),
        },
        ("PUT", ["events", id]) => {
            let if_match = req
                .headers()
                .get("if-match")
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned);
            match raw(MAX_JSON).await {
                Ok(b) => answer_json(
                    json_body(&b)
                        .and_then(|s| save(store, &login, Some(id), &s, if_match.as_deref())),
                ),
                Err(r) => r,
            }
        }
        ("DELETE", ["events", id]) => {
            let q = query(req);
            answer_json(
                delete(store, &login, id, q.get("occurrence").map(String::as_str))
                    .map(|()| json!({ "removed": id })),
            )
        }
        ("GET", ["events", id, "invite"]) => match invite(store, &login, id) {
            Ok(text) => HttpResponse::Ok()
                .content_type("text/calendar; charset=utf-8; method=REQUEST")
                .insert_header(("Cache-Control", "no-store"))
                .body(text),
            Err(e) => refused(&e),
        },
        ("POST", ["invitations", "read"]) => match raw(MAX_FILE_BYTES).await {
            Ok(b) => {
                let q = query(req);
                let email = q.get("email").cloned().unwrap_or_default();
                answer_json(
                    std::str::from_utf8(&b)
                        .map_err(|_| CalendarError::Bad("the attachment is not text".into()))
                        .and_then(|t| read_invitation(store, &login, t, &email)),
                )
            }
            Err(r) => r,
        },
        ("POST", ["invitations", "answer"]) => match raw(MAX_FILE_BYTES).await {
            Ok(b) => answer_json(json_body(&b).and_then(|a| answer(store, &login, &a))),
            Err(r) => r,
        },
        ("POST", ["invitations", "apply"]) => match raw(MAX_FILE_BYTES).await {
            Ok(b) => answer_json(
                std::str::from_utf8(&b)
                    .map_err(|_| CalendarError::Bad("the attachment is not text".into()))
                    .and_then(|t| apply(store, &login, t)),
            ),
            Err(r) => r,
        },
        _ => crate::cloud::api::error(StatusCode::NOT_FOUND, "notFound", "no such call"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cloud() -> (tempfile::TempDir, Cloud) {
        let dir = tempfile::tempdir().unwrap();
        let cloud = Cloud::open(dir.path(), crate::cloud::Config::default()).unwrap();
        (dir, cloud)
    }

    fn fields(summary: &str, start: &str, end: &str) -> Fields {
        Fields {
            summary: summary.into(),
            start: start.into(),
            end: end.into(),
            zone: Some("Europe/Bratislava".into()),
            ..Default::default()
        }
    }

    const FEED: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nX-WR-CALNAME:Tax deadlines\r\nBEGIN:VEVENT\r\nUID:income@tax\r\nDTSTART;VALUE=DATE:20270331\r\nRRULE:FREQ=YEARLY\r\nSUMMARY:Income tax return\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:vat@tax\r\nDTSTART;VALUE=DATE:20261025\r\nRRULE:FREQ=MONTHLY;BYMONTHDAY=25\r\nSUMMARY:VAT return\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";

    #[test]
    fn an_event_made_here_is_an_object_a_phone_reads() {
        let (_d, cloud) = cloud();
        let s = &cloud.store;
        let saved = save(
            s,
            "eva",
            None,
            &Save {
                calendar: String::new(),
                fields: fields("Dentist", "2026-10-23T09:00", "2026-10-23T10:00"),
            },
            None,
        )
        .unwrap();
        assert_eq!(saved.calendar, "personal");
        assert_eq!(saved.start, "2026-10-23T09:00:00");
        assert_eq!(saved.zone.as_deref(), Some("Europe/Bratislava"));
        let c = pim::collection(s, "eva", Kind::Calendar, "personal").unwrap();
        let objects = pim::objects(s, &c).unwrap();
        assert_eq!(objects.len(), 1);
        assert!(objects[0].data.contains("SUMMARY:Dentist\r\n"));
        // Listed in its week, and nobody else's.
        let week = occurrences(s, "eva", "2026-10-19", "2026-10-26").unwrap();
        assert_eq!(week.len(), 1);
        assert!(occurrences(s, "matus", "2026-10-19", "2026-10-26")
            .unwrap()
            .is_empty());
        assert!(matches!(
            get(s, "matus", &saved.id),
            Err(CalendarError::NotFound)
        ));
        // An edit with an old ETag is refused; with the right one it lands.
        let mut again = Save {
            calendar: "personal".into(),
            fields: fields("Dentist, Dr. Malá", "2026-10-23T09:30", "2026-10-23T10:00"),
        };
        assert!(matches!(
            save(s, "eva", Some(&saved.id), &again, Some("\"old\"")),
            Err(CalendarError::Changed)
        ));
        let edited = save(s, "eva", Some(&saved.id), &again, Some(&saved.etag)).unwrap();
        assert_eq!(edited.summary, "Dentist, Dr. Malá");
        assert_eq!(edited.id, saved.id);
        // Moved to another calendar.
        let work = create(
            s,
            "eva",
            &CalendarFields {
                name: Some("Work".into()),
                color: Some("#2266AA".into()),
                refresh: None,
            },
        )
        .unwrap();
        assert_eq!(work.color.as_deref(), Some("#2266aa"));
        again.calendar = work.uri.clone();
        let moved = save(s, "eva", Some(&edited.id), &again, None).unwrap();
        assert_eq!(moved.calendar, work.uri);
        assert_eq!(
            occurrences(s, "eva", "2026-10-19", "2026-10-26")
                .unwrap()
                .len(),
            1
        );
        delete(s, "eva", &moved.id, None).unwrap();
        assert!(occurrences(s, "eva", "2026-10-19", "2026-10-26")
            .unwrap()
            .is_empty());
    }

    #[test]
    fn a_feed_fills_a_calendar_nobody_writes_to() {
        let (_d, cloud) = cloud();
        let s = &cloud.store;
        let c = make(s, "matus", "webcal", "Tax", None).unwrap();
        s.with_db(|db| {
            db.execute(
                "INSERT INTO calendar_feed (collection, url, refresh) VALUES (?1, 'webcal://f/t.ics', 86400)",
                params![c.id],
            )?;
            Ok(())
        })
        .unwrap();
        let r = apply_feed(s, &c, FEED.as_bytes()).unwrap();
        assert_eq!((r.added, r.updated, r.removed), (2, 0, 0));
        // The same feed again changes nothing; one event fewer removes it.
        assert_eq!(
            apply_feed(s, &c, FEED.as_bytes()).unwrap(),
            Report::default()
        );
        let shorter = FEED.replace("BEGIN:VEVENT\r\nUID:vat@tax\r\nDTSTART;VALUE=DATE:20261025\r\nRRULE:FREQ=MONTHLY;BYMONTHDAY=25\r\nSUMMARY:VAT return\r\nEND:VEVENT\r\n", "");
        assert_eq!(apply_feed(s, &c, shorter.as_bytes()).unwrap().removed, 1);
        assert!(apply_feed(s, &c, b"<html>not found</html>").is_err());
        assert_eq!(pim::count(s, &c).unwrap(), 1);
        // The page may not write into it.
        assert!(read_only(s, c.id));
        let attempt = Save {
            calendar: c.uri.clone(),
            fields: fields("Mine", "2026-10-23T09:00", "2026-10-23T10:00"),
        };
        assert!(matches!(
            save(s, "matus", None, &attempt, None),
            Err(CalendarError::ReadOnly)
        ));
        let listed = list(s, "matus").unwrap();
        let sub = listed.iter().find(|v| v.uri == c.uri).unwrap();
        assert!(sub.read_only);
        assert_eq!(sub.feed.as_ref().unwrap().url, "webcal://f/t.ics");
        assert!(publish(s, "matus", &c.uri, true).is_err());
        let year = occurrences(s, "matus", "2027-01-01", "2028-01-01").unwrap();
        assert_eq!(
            year.iter().map(|e| e.summary.as_str()).collect::<Vec<_>>(),
            vec!["Income tax return"]
        );
        assert!(year[0].read_only && year[0].all_day && year[0].repeats);
        assert_eq!(year[0].occurrence.as_deref(), Some("2027-03-31"));
    }

    #[test]
    fn an_import_twice_updates_and_a_published_calendar_serves_it() {
        let (_d, cloud) = cloud();
        let s = &cloud.store;
        let (cal, r) = import(s, "matus", None, FEED.as_bytes()).unwrap();
        assert_eq!((r.added, r.updated), (2, 0));
        assert_eq!(cal.uri, "personal");
        let (_, r) = import(s, "matus", None, FEED.as_bytes()).unwrap();
        assert_eq!((r.added, r.updated), (0, 2));
        assert!(import(s, "matus", None, b"BEGIN:VCARD\r\nEND:VCARD\r\n").is_err());
        let published = publish(s, "matus", "personal", true)
            .unwrap()
            .published
            .unwrap();
        let token = published
            .trim_start_matches("/cloud/feed/")
            .trim_end_matches(".ics");
        let text = feed(s, token).unwrap();
        let back = ical::parse(&text);
        assert_eq!(back.events.len(), 2);
        assert_eq!(back.name, "Personal");
        assert!(public(&cloud, "GET", &format!("/cloud/feed/{token}.ics"))
            .unwrap()
            .status()
            .is_success());
        assert_eq!(
            public(&cloud, "GET", "/cloud/feed/nothing-of-the-sort-here.ics")
                .unwrap()
                .status(),
            StatusCode::NOT_FOUND
        );
        // Taken back: the address stops working.
        publish(s, "matus", "personal", false).unwrap();
        assert!(feed(s, token).is_err());
    }

    #[test]
    fn one_occurrence_of_a_repeat_goes() {
        let (_d, cloud) = cloud();
        let s = &cloud.store;
        let mut f = fields("Spanish", "2026-10-19T08:00", "2026-10-19T08:45");
        f.repeat = Some(Repeat {
            freq: ical::Freq::Weekly,
            interval: 1,
            until: None,
            count: None,
            days: vec!["MO".into(), "WE".into()],
            offset: 0,
        });
        let e = save(
            s,
            "matus",
            None,
            &Save {
                calendar: String::new(),
                fields: f,
            },
            None,
        )
        .unwrap();
        // A window is read a day wider each way, for the page to place
        // events in other zones: Sunday 25th ends it short of the next Monday.
        let two = occurrences(s, "matus", "2026-10-19", "2026-10-25").unwrap();
        assert_eq!(two.len(), 2);
        assert_eq!(two[1].occurrence.as_deref(), Some("2026-10-21T08:00:00"));
        delete(s, "matus", &e.id, Some("2026-10-21T08:00:00")).unwrap();
        let one = occurrences(s, "matus", "2026-10-19", "2026-10-25").unwrap();
        assert_eq!(one.len(), 1);
        assert_eq!(one[0].start, "2026-10-19T08:00:00");
    }

    #[test]
    fn an_invitation_goes_out_comes_back_answered_and_is_recorded() {
        let (_d, cloud) = cloud();
        let s = &cloud.store;
        let mut f = fields("Budget review", "2026-10-23T10:00", "2026-10-23T11:00");
        f.organizer = Some(Person {
            email: "matus@mattbox.local".into(),
            name: "Matus".into(),
            ..Default::default()
        });
        f.attendees = vec![Person {
            email: "eva@mattbox.local".into(),
            name: "Eva".into(),
            ..Default::default()
        }];
        let e = save(
            s,
            "matus",
            None,
            &Save {
                calendar: String::new(),
                fields: f,
            },
            None,
        )
        .unwrap();
        let req = invite(s, "matus", &e.id).unwrap();
        // Eva reads it: nothing stored yet, her answer is asked for.
        let seen = read_invitation(s, "eva", &req, "eva@mattbox.local").unwrap();
        assert_eq!(seen.method, "REQUEST");
        assert_eq!(seen.status, "NEEDS-ACTION");
        assert!(seen.stored.is_none());
        let done = answer(
            s,
            "eva",
            &AnswerBody {
                ics: req.clone(),
                email: "eva@mattbox.local".into(),
                status: "accepted".into(),
                calendar: String::new(),
            },
        )
        .unwrap();
        assert_eq!(done.to, "matus@mattbox.local");
        assert_eq!(done.subject, "Accepted: Budget review");
        let hers = occurrences(s, "eva", "2026-10-19", "2026-10-26").unwrap();
        assert_eq!(hers.len(), 1);
        assert_eq!(hers[0].attendees[0].status, "ACCEPTED");
        assert_eq!(
            read_invitation(s, "eva", &req, "eva@mattbox.local")
                .unwrap()
                .status,
            "ACCEPTED"
        );
        // Changing her mind updates the same copy.
        answer(
            s,
            "eva",
            &AnswerBody {
                ics: req.clone(),
                email: "eva@mattbox.local".into(),
                status: "DECLINED".into(),
                calendar: String::new(),
            },
        )
        .unwrap();
        assert_eq!(
            occurrences(s, "eva", "2026-10-19", "2026-10-26").unwrap()[0].attendees[0].status,
            "DECLINED"
        );
        // Someone not invited cannot answer.
        assert!(answer(
            s,
            "jana",
            &AnswerBody {
                ics: req.clone(),
                email: "jana@mattbox.local".into(),
                status: "ACCEPTED".into(),
                calendar: String::new()
            }
        )
        .is_err());
        // Matus records her reply.
        let recorded = apply(s, "matus", &done.reply).unwrap();
        assert_eq!(recorded.attendees[0].status, "ACCEPTED");
        // A cancellation from someone who is not the organizer is refused.
        let fake = req.replace("METHOD:REQUEST", "METHOD:CANCEL").replace(
            "ORGANIZER;CN=Matus:mailto:matus@mattbox.local",
            "ORGANIZER:mailto:x@y.z",
        );
        assert!(apply(s, "eva", &fake).is_err());
        let cancel = req.replace("METHOD:REQUEST", "METHOD:CANCEL");
        let gone = apply(s, "eva", &cancel).unwrap();
        assert_eq!(gone.status, "CANCELLED");
        assert!(occurrences(s, "eva", "2026-10-19", "2026-10-26")
            .unwrap()
            .is_empty());
    }
}
