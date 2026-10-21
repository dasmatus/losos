//! Calendars and address books: the collections, the iCalendar and vCard
//! objects in them, and a change log per collection for sync tokens.
//!
//! The objects are kept as the text the client sent, byte for byte; only
//! what a query needs (component, UID, first and last moment) is read out
//! of it. [`crate::cloud::caldav`] serves this over CalDAV and CardDAV,
//! and [`crate::cloud::import`] fills it from a Nextcloud database.

use crate::cloud::store::{Result, Store, StoreError};
use crate::cloud::util::{now, sha1_hex};
use rusqlite::{params, OptionalExtension};

pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS dav_collection (
    id          INTEGER PRIMARY KEY,
    owner       TEXT NOT NULL,
    kind        TEXT NOT NULL CHECK (kind IN ('calendar', 'addressbook')),
    uri         TEXT NOT NULL,
    name        TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    color       TEXT,
    components  TEXT NOT NULL DEFAULT 'VEVENT,VTODO',
    timezone    TEXT,
    position    INTEGER NOT NULL DEFAULT 0,
    sync        INTEGER NOT NULL DEFAULT 1,
    created     INTEGER NOT NULL,
    UNIQUE (owner, kind, uri)
);
CREATE TABLE IF NOT EXISTS dav_object (
    id          INTEGER PRIMARY KEY,
    collection  INTEGER NOT NULL REFERENCES dav_collection (id) ON DELETE CASCADE,
    uri         TEXT NOT NULL,
    data        TEXT NOT NULL,
    etag        TEXT NOT NULL,
    component   TEXT,
    uid         TEXT,
    first       INTEGER,
    last        INTEGER,
    modified    INTEGER NOT NULL,
    UNIQUE (collection, uri)
);
CREATE INDEX IF NOT EXISTS dav_object_uid ON dav_object (collection, uid);
CREATE TABLE IF NOT EXISTS dav_change (
    collection  INTEGER NOT NULL REFERENCES dav_collection (id) ON DELETE CASCADE,
    sync        INTEGER NOT NULL,
    uri         TEXT NOT NULL,
    deleted     INTEGER NOT NULL,
    PRIMARY KEY (collection, sync, uri)
);
"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Calendar,
    AddressBook,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Calendar => "calendar",
            Kind::AddressBook => "addressbook",
        }
    }
    fn parse(s: &str) -> Kind {
        if s == "addressbook" {
            Kind::AddressBook
        } else {
            Kind::Calendar
        }
    }
    /// The file extension clients give objects of this kind.
    pub fn ext(self) -> &'static str {
        match self {
            Kind::Calendar => "ics",
            Kind::AddressBook => "vcf",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Collection {
    pub id: i64,
    pub owner: String,
    pub kind: Kind,
    pub uri: String,
    pub name: String,
    pub description: String,
    pub color: Option<String>,
    /// Calendars: which components it takes, `VEVENT,VTODO`.
    pub components: String,
    pub timezone: Option<String>,
    pub position: i64,
    /// The current sync token's number; it grows with every change.
    pub sync: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Object {
    pub id: i64,
    pub uri: String,
    pub data: String,
    pub etag: String,
    pub component: Option<String>,
    pub uid: Option<String>,
    pub first: Option<i64>,
    pub last: Option<i64>,
    pub modified: i64,
}

/// What came back from a sync-token query: changed and deleted names.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Changes {
    pub changed: Vec<String>,
    pub deleted: Vec<String>,
}

/// Fields for a new or changed collection.
#[derive(Debug, Clone, Default)]
pub struct CollectionFields {
    pub name: Option<String>,
    pub description: Option<String>,
    pub color: Option<String>,
    pub components: Option<String>,
    pub timezone: Option<String>,
    pub position: Option<i64>,
}

const COLS: &str =
    "id, owner, kind, uri, name, description, color, components, timezone, position, sync";

fn collection_row(r: &rusqlite::Row) -> rusqlite::Result<Collection> {
    Ok(Collection {
        id: r.get(0)?,
        owner: r.get(1)?,
        kind: Kind::parse(&r.get::<_, String>(2)?),
        uri: r.get(3)?,
        name: r.get(4)?,
        description: r.get(5)?,
        color: r.get(6)?,
        components: r.get(7)?,
        timezone: r.get(8)?,
        position: r.get(9)?,
        sync: r.get(10)?,
    })
}

const OBJ_COLS: &str = "id, uri, data, etag, component, uid, first, last, modified";

fn object_row(r: &rusqlite::Row) -> rusqlite::Result<Object> {
    Ok(Object {
        id: r.get(0)?,
        uri: r.get(1)?,
        data: r.get(2)?,
        etag: r.get(3)?,
        component: r.get(4)?,
        uid: r.get(5)?,
        first: r.get(6)?,
        last: r.get(7)?,
        modified: r.get(8)?,
    })
}

/// A name a collection or object may have: what Nextcloud and the apps
/// make (letters, digits, `-`, `_`, `.`, `@`, `+`), nothing that walks.
pub fn valid_uri(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 255
        && s != "."
        && s != ".."
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.@+=~ ".contains(c))
}

pub fn init(store: &Store) -> Result<()> {
    store.with_db(|db| Ok(db.execute_batch(SCHEMA)?))
}

/// Everyone's collections of one kind, in the order they show.
pub fn collections(store: &Store, owner: &str, kind: Kind) -> Result<Vec<Collection>> {
    store.with_db(|db| {
        let mut st = db.prepare(&format!(
            "SELECT {COLS} FROM dav_collection WHERE owner = ?1 AND kind = ?2 ORDER BY position, id"
        ))?;
        let rows = st.query_map(params![owner, kind.as_str()], collection_row)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    })
}

/// The person's collections, with the first calendar and address book made
/// for them if they have none, as Nextcloud does.
pub fn ensure_defaults(store: &Store, owner: &str) -> Result<()> {
    if collections(store, owner, Kind::Calendar)?.is_empty() {
        create(
            store,
            owner,
            Kind::Calendar,
            "personal",
            &CollectionFields {
                name: Some("Personal".into()),
                color: Some("#0082C9".into()),
                ..Default::default()
            },
        )?;
    }
    if collections(store, owner, Kind::AddressBook)?.is_empty() {
        create(
            store,
            owner,
            Kind::AddressBook,
            "contacts",
            &CollectionFields {
                name: Some("Contacts".into()),
                ..Default::default()
            },
        )?;
    }
    Ok(())
}

pub fn collection(store: &Store, owner: &str, kind: Kind, uri: &str) -> Result<Collection> {
    store.with_db(|db| {
        db.query_row(
            &format!(
                "SELECT {COLS} FROM dav_collection WHERE owner = ?1 AND kind = ?2 AND uri = ?3"
            ),
            params![owner, kind.as_str(), uri],
            collection_row,
        )
        .optional()?
        .ok_or(StoreError::NotFound)
    })
}

pub fn create(
    store: &Store,
    owner: &str,
    kind: Kind,
    uri: &str,
    f: &CollectionFields,
) -> Result<Collection> {
    if !valid_uri(uri) {
        return Err(StoreError::BadName);
    }
    let r = store.with_db(|db| {
        db.execute(
            "INSERT INTO dav_collection (owner, kind, uri, name, description, color, components, timezone, position, created)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                owner,
                kind.as_str(),
                uri,
                f.name.clone().unwrap_or_else(|| uri.to_owned()),
                f.description.clone().unwrap_or_default(),
                f.color,
                f.components.clone().unwrap_or_else(|| "VEVENT,VTODO".into()),
                f.timezone,
                f.position.unwrap_or(0),
                now()
            ],
        )
        .map_err(|e| match e {
            rusqlite::Error::SqliteFailure(ref s, _)
                if s.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                StoreError::Exists
            }
            e => e.into(),
        })
    });
    r?;
    collection(store, owner, kind, uri)
}

pub fn update(store: &Store, c: &Collection, f: &CollectionFields) -> Result<()> {
    store.with_db(|db| {
        let tx = db.transaction()?;
        if let Some(v) = &f.name {
            tx.execute(
                "UPDATE dav_collection SET name = ?1 WHERE id = ?2",
                params![v, c.id],
            )?;
        }
        if let Some(v) = &f.description {
            tx.execute(
                "UPDATE dav_collection SET description = ?1 WHERE id = ?2",
                params![v, c.id],
            )?;
        }
        if let Some(v) = &f.color {
            let v = (!v.is_empty()).then_some(v);
            tx.execute(
                "UPDATE dav_collection SET color = ?1 WHERE id = ?2",
                params![v, c.id],
            )?;
        }
        if let Some(v) = &f.timezone {
            tx.execute(
                "UPDATE dav_collection SET timezone = ?1 WHERE id = ?2",
                params![v, c.id],
            )?;
        }
        if let Some(v) = f.position {
            tx.execute(
                "UPDATE dav_collection SET position = ?1 WHERE id = ?2",
                params![v, c.id],
            )?;
        }
        // A property change is a change to the collection: clients that
        // compare the ctag reread it.
        tx.execute(
            "UPDATE dav_collection SET sync = sync + 1 WHERE id = ?1",
            params![c.id],
        )?;
        tx.commit()?;
        Ok(())
    })
}

pub fn delete_collection(store: &Store, c: &Collection) -> Result<()> {
    store.with_db(|db| {
        db.execute("DELETE FROM dav_collection WHERE id = ?1", params![c.id])?;
        Ok(())
    })
}

pub fn objects(store: &Store, c: &Collection) -> Result<Vec<Object>> {
    store.with_db(|db| {
        let mut st = db.prepare(&format!(
            "SELECT {OBJ_COLS} FROM dav_object WHERE collection = ?1 ORDER BY uri"
        ))?;
        let rows = st.query_map(params![c.id], object_row)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    })
}

pub fn object(store: &Store, c: &Collection, uri: &str) -> Result<Object> {
    store.with_db(|db| {
        db.query_row(
            &format!("SELECT {OBJ_COLS} FROM dav_object WHERE collection = ?1 AND uri = ?2"),
            params![c.id, uri],
            object_row,
        )
        .optional()?
        .ok_or(StoreError::NotFound)
    })
}

pub fn count(store: &Store, c: &Collection) -> Result<i64> {
    store.with_db(|db| {
        Ok(db.query_row(
            "SELECT COUNT(*) FROM dav_object WHERE collection = ?1",
            params![c.id],
            |r| r.get(0),
        )?)
    })
}

/// Why a PUT was refused before it reached the table.
#[derive(Debug)]
pub enum PutError {
    Store(StoreError),
    /// Not an iCalendar (or vCard) object, or one with no component.
    Invalid,
    /// Another object in the collection has this UID.
    UidConflict(String),
    /// The calendar does not take this component.
    Unsupported,
}

impl From<StoreError> for PutError {
    fn from(e: StoreError) -> PutError {
        PutError::Store(e)
    }
}

/// What a PUT's preconditions say.
#[derive(Debug, Clone, Copy)]
pub enum Expect<'a> {
    Any,
    /// `If-None-Match: *`: only a new object.
    New,
    /// `If-Match`: only this version.
    Etag(&'a str),
}

/// Write an object. Returns its new ETag and whether it was created.
pub fn put(
    store: &Store,
    c: &Collection,
    uri: &str,
    data: &str,
    expect: Expect,
) -> std::result::Result<(String, bool), PutError> {
    if !valid_uri(uri) {
        return Err(PutError::Store(StoreError::BadName));
    }
    let facts = facts(c.kind, data).ok_or(PutError::Invalid)?;
    if c.kind == Kind::Calendar {
        let comp = facts.component.as_deref().unwrap_or_default();
        if !c.components.split(',').any(|x| x.trim() == comp) {
            return Err(PutError::Unsupported);
        }
    }
    let etag = format!("\"{}\"", sha1_hex(data.as_bytes()));
    let created = store.with_db(|db| {
        let tx = db.transaction()?;
        let existing: Option<String> = tx
            .query_row(
                "SELECT etag FROM dav_object WHERE collection = ?1 AND uri = ?2",
                params![c.id, uri],
                |r| r.get(0),
            )
            .optional()?;
        match (expect, &existing) {
            (Expect::New, Some(_)) => return Ok(Err(PutError::Store(StoreError::Precondition))),
            (Expect::Etag(_), None) => return Ok(Err(PutError::Store(StoreError::Precondition))),
            (Expect::Etag(want), Some(have)) if want != "*" && want != have => {
                return Ok(Err(PutError::Store(StoreError::Precondition)))
            }
            _ => {}
        }
        if let Some(uid) = &facts.uid {
            let other: Option<String> = tx
                .query_row(
                    "SELECT uri FROM dav_object WHERE collection = ?1 AND uid = ?2 AND uri != ?3",
                    params![c.id, uid, uri],
                    |r| r.get(0),
                )
                .optional()?;
            if let Some(other) = other {
                return Ok(Err(PutError::UidConflict(other)));
            }
        }
        let sync: i64 = tx.query_row(
            "UPDATE dav_collection SET sync = sync + 1 WHERE id = ?1 RETURNING sync",
            params![c.id],
            |r| r.get(0),
        )?;
        tx.execute(
            "INSERT INTO dav_object (collection, uri, data, etag, component, uid, first, last, modified)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT (collection, uri) DO UPDATE SET
               data = excluded.data, etag = excluded.etag, component = excluded.component,
               uid = excluded.uid, first = excluded.first, last = excluded.last,
               modified = excluded.modified",
            params![
                c.id,
                uri,
                data,
                etag,
                facts.component,
                facts.uid,
                facts.first,
                facts.last,
                now()
            ],
        )?;
        log_change(&tx, c.id, sync, uri, false)?;
        tx.commit()?;
        Ok(Ok(existing.is_none()))
    })??;
    Ok((etag, created))
}

pub fn delete_object(
    store: &Store,
    c: &Collection,
    uri: &str,
    if_match: Option<&str>,
) -> Result<()> {
    store.with_db(|db| {
        let tx = db.transaction()?;
        let have: Option<String> = tx
            .query_row(
                "SELECT etag FROM dav_object WHERE collection = ?1 AND uri = ?2",
                params![c.id, uri],
                |r| r.get(0),
            )
            .optional()?;
        let Some(have) = have else {
            return Err(StoreError::NotFound);
        };
        if let Some(want) = if_match {
            if want != "*" && want != have {
                return Err(StoreError::Precondition);
            }
        }
        let sync: i64 = tx.query_row(
            "UPDATE dav_collection SET sync = sync + 1 WHERE id = ?1 RETURNING sync",
            params![c.id],
            |r| r.get(0),
        )?;
        tx.execute(
            "DELETE FROM dav_object WHERE collection = ?1 AND uri = ?2",
            params![c.id, uri],
        )?;
        log_change(&tx, c.id, sync, uri, true)?;
        tx.commit()?;
        Ok(())
    })
}

fn log_change(
    tx: &rusqlite::Transaction,
    collection: i64,
    sync: i64,
    uri: &str,
    deleted: bool,
) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT OR REPLACE INTO dav_change (collection, sync, uri, deleted) VALUES (?1, ?2, ?3, ?4)",
        params![collection, sync, uri, deleted],
    )?;
    Ok(())
}

/// Every name changed or deleted after sync token `since`. `None` when the
/// token is not one this collection gave out (the client starts over).
pub fn changes_since(store: &Store, c: &Collection, since: i64) -> Result<Option<Changes>> {
    if since > c.sync || since < 1 {
        return Ok(None);
    }
    store.with_db(|db| {
        let mut st = db.prepare(
            "SELECT uri, deleted FROM dav_change WHERE collection = ?1 AND sync > ?2
             ORDER BY sync",
        )?;
        let mut last: std::collections::BTreeMap<String, bool> = Default::default();
        for row in st.query_map(params![c.id, since], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, bool>(1)?))
        })? {
            let (uri, deleted) = row?;
            last.insert(uri, deleted);
        }
        let mut out = Changes::default();
        for (uri, deleted) in last {
            if deleted {
                out.deleted.push(uri);
            } else {
                out.changed.push(uri);
            }
        }
        Ok(Some(out))
    })
}

/// What a query needs from an object.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Facts {
    pub component: Option<String>,
    pub uid: Option<String>,
    pub first: Option<i64>,
    pub last: Option<i64>,
}

/// The content lines of an iCalendar or vCard text, unfolded.
pub fn unfold(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for raw in text.split('\n') {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        if let Some(cont) = line.strip_prefix([' ', '\t']) {
            if let Some(prev) = out.last_mut() {
                prev.push_str(cont);
                continue;
            }
        }
        if !line.is_empty() {
            out.push(line.to_owned());
        }
    }
    out
}

/// A content line's name (upper case, no parameters) and value.
pub fn split_line(line: &str) -> (String, &str) {
    let (head, value) = line.split_once(':').unwrap_or((line, ""));
    let name = head.split(';').next().unwrap_or(head).to_ascii_uppercase();
    (name, value)
}

/// Read what [`Facts`] holds out of an object; `None` when it is not one.
pub fn facts(kind: Kind, data: &str) -> Option<Facts> {
    let lines = unfold(data);
    let first = lines.first().map(|l| l.trim().to_ascii_uppercase())?;
    let mut f = Facts::default();
    match kind {
        Kind::AddressBook => {
            if first != "BEGIN:VCARD" {
                return None;
            }
            for l in &lines {
                let (name, value) = split_line(l);
                if name == "UID" && f.uid.is_none() {
                    f.uid = Some(value.trim().to_owned());
                }
            }
            Some(f)
        }
        Kind::Calendar => {
            if first != "BEGIN:VCALENDAR" {
                return None;
            }
            let mut depth = 0;
            let mut in_main = false;
            let mut start: Option<(i64, bool)> = None;
            let mut end: Option<i64> = None;
            let mut duration: Option<i64> = None;
            let mut repeats = false;
            for l in &lines {
                let (name, value) = split_line(l);
                match name.as_str() {
                    "BEGIN" => {
                        depth += 1;
                        let comp = value.trim().to_ascii_uppercase();
                        if depth == 2 && f.component.is_none() && comp != "VTIMEZONE" {
                            f.component = Some(comp);
                            in_main = true;
                        }
                    }
                    "END" => {
                        if depth == 2 {
                            in_main = false;
                        }
                        depth -= 1;
                    }
                    _ if in_main && depth == 2 => match name.as_str() {
                        "UID" if f.uid.is_none() => f.uid = Some(value.trim().to_owned()),
                        "DTSTART" if start.is_none() => start = parse_time(value),
                        "DTEND" | "DUE" if end.is_none() => end = parse_time(value).map(|t| t.0),
                        "DURATION" => duration = parse_duration(value),
                        "RRULE" | "RDATE" => repeats = true,
                        _ => {}
                    },
                    _ => {}
                }
            }
            f.component.as_ref()?;
            if let Some((s, all_day)) = start {
                f.first = Some(s);
                f.last = if repeats {
                    None
                } else {
                    Some(end.or(duration.map(|d| s + d)).unwrap_or(if all_day {
                        s + 86_400
                    } else {
                        s
                    }))
                };
            }
            Some(f)
        }
    }
}

/// An iCalendar DATE or DATE-TIME as Unix seconds, and whether it was a
/// date. A local time is read as UTC: close enough for a query window,
/// which is all this is used for.
pub fn parse_time(value: &str) -> Option<(i64, bool)> {
    let v = value.trim();
    let digits = |s: &str| s.parse::<i64>().ok();
    let (date, time) = match v.split_once('T') {
        Some((d, t)) => (d, Some(t.trim_end_matches('Z'))),
        None => (v, None),
    };
    if date.len() != 8 {
        return None;
    }
    let (y, m, d) = (
        digits(&date[..4])?,
        digits(&date[4..6])?,
        digits(&date[6..8])?,
    );
    let days = days_from_civil(y, m, d);
    let secs = match time {
        Some(t) if t.len() >= 6 => {
            digits(&t[..2])? * 3600 + digits(&t[2..4])? * 60 + digits(&t[4..6])?
        }
        Some(_) => return None,
        None => 0,
    };
    Some((days * 86_400 + secs, time.is_none()))
}

/// `P1DT2H` and the like, in seconds. Weeks, days, hours, minutes, seconds.
pub fn parse_duration(value: &str) -> Option<i64> {
    let v = value.trim();
    let (sign, v) = match v.strip_prefix('-') {
        Some(rest) => (-1, rest),
        None => (1, v.strip_prefix('+').unwrap_or(v)),
    };
    let v = v.strip_prefix('P')?;
    let mut total = 0i64;
    let mut num = String::new();
    let mut in_time = false;
    for c in v.chars() {
        match c {
            'T' => in_time = true,
            '0'..='9' => num.push(c),
            unit => {
                let n: i64 = num.parse().ok()?;
                num.clear();
                total += n * match (unit, in_time) {
                    ('W', _) => 7 * 86_400,
                    ('D', _) => 86_400,
                    ('H', true) => 3600,
                    ('M', true) => 60,
                    ('S', true) => 1,
                    _ => return None,
                };
            }
        }
    }
    Some(sign * total)
}

/// Days since 1970-01-01 of a proleptic Gregorian date.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    const EVENT: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//test//EN\r\nBEGIN:VTIMEZONE\r\nTZID:Europe/Bratislava\r\nEND:VTIMEZONE\r\nBEGIN:VEVENT\r\nUID:abc-1\r\nDTSTART;TZID=Europe/Bratislava:20261023T090000\r\nDURATION:PT1H30M\r\nSUMMARY:Dentist\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";

    fn store() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        init(&store).unwrap();
        (dir, store)
    }

    #[test]
    fn facts_come_out_of_an_event_a_task_and_a_card() {
        let f = facts(Kind::Calendar, EVENT).unwrap();
        assert_eq!(f.component.as_deref(), Some("VEVENT"));
        assert_eq!(f.uid.as_deref(), Some("abc-1"));
        let start = parse_time("20261023T090000").unwrap().0;
        assert_eq!(f.first, Some(start));
        assert_eq!(f.last, Some(start + 5400));

        let task =
            "BEGIN:VCALENDAR\nBEGIN:VTODO\nUID:t1\nSUMMARY:Buy\n  milk\nEND:VTODO\nEND:VCALENDAR\n";
        let f = facts(Kind::Calendar, task).unwrap();
        assert_eq!(f.component.as_deref(), Some("VTODO"));
        assert_eq!(f.first, None);
        assert!(unfold(task).contains(&"SUMMARY:Buy milk".to_owned()));

        let card = "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:c1\r\nFN:Eva\r\nEND:VCARD\r\n";
        assert_eq!(
            facts(Kind::AddressBook, card).unwrap().uid.as_deref(),
            Some("c1")
        );
        assert!(facts(Kind::AddressBook, EVENT).is_none());
        assert!(facts(Kind::Calendar, "hello").is_none());
    }

    #[test]
    fn dates_durations_and_repeats() {
        assert_eq!(parse_time("19700102"), Some((86_400, true)));
        assert_eq!(parse_time("20000301T000000Z").unwrap().0, 951_868_800);
        assert_eq!(parse_duration("P1W"), Some(604_800));
        assert_eq!(parse_duration("-PT15M"), Some(-900));
        let rec = EVENT.replace("SUMMARY", "RRULE:FREQ=WEEKLY\r\nSUMMARY");
        assert_eq!(facts(Kind::Calendar, &rec).unwrap().last, None);
    }

    #[test]
    fn puts_deletes_and_sync_tokens() {
        let (_d, store) = store();
        ensure_defaults(&store, "eva").unwrap();
        let cal = collection(&store, "eva", Kind::Calendar, "personal").unwrap();
        let t0 = cal.sync;
        let (etag, created) = put(&store, &cal, "a.ics", EVENT, Expect::New).unwrap();
        assert!(created);
        assert!(matches!(
            put(&store, &cal, "a.ics", EVENT, Expect::New),
            Err(PutError::Store(StoreError::Precondition))
        ));
        assert!(matches!(
            put(&store, &cal, "b.ics", EVENT, Expect::Any),
            Err(PutError::UidConflict(u)) if u == "a.ics"
        ));
        let (etag2, created) = put(&store, &cal, "a.ics", EVENT, Expect::Etag(&etag)).unwrap();
        assert!(!created);
        assert_eq!(etag, etag2, "same text, same etag");
        let cal = collection(&store, "eva", Kind::Calendar, "personal").unwrap();
        let mid = cal.sync;
        let ch = changes_since(&store, &cal, t0).unwrap().unwrap();
        assert_eq!(ch.changed, vec!["a.ics".to_owned()]);
        assert!(matches!(
            delete_object(&store, &cal, "a.ics", Some("\"nope\"")),
            Err(StoreError::Precondition)
        ));
        delete_object(&store, &cal, "a.ics", None).unwrap();
        let cal = collection(&store, "eva", Kind::Calendar, "personal").unwrap();
        let ch = changes_since(&store, &cal, mid).unwrap().unwrap();
        assert_eq!(ch.deleted, vec!["a.ics".to_owned()]);
        assert!(ch.changed.is_empty());
        assert_eq!(changes_since(&store, &cal, cal.sync + 5).unwrap(), None);

        let tasks = create(
            &store,
            "eva",
            Kind::Calendar,
            "tasks",
            &CollectionFields {
                components: Some("VTODO".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(matches!(
            put(&store, &tasks, "e.ics", EVENT, Expect::Any),
            Err(PutError::Unsupported)
        ));
        assert!(matches!(
            create(&store, "eva", Kind::Calendar, "tasks", &Default::default()),
            Err(StoreError::Exists)
        ));
        assert!(matches!(
            create(&store, "eva", Kind::Calendar, "../x", &Default::default()),
            Err(StoreError::BadName)
        ));
    }
}
