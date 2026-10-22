//! Moving a Nextcloud data folder's files into the cloud, once, when a box
//! switches over.
//!
//! Nextcloud keeps each account's files at `<data>/<login>/files/`. They are
//! moved (a rename, so no second copy on disk) into the person's home and
//! then scanned. A name the person already has in the cloud keeps the cloud's
//! file; the Nextcloud one lands beside it as "<name> (from Nextcloud)".
//! The apps keep a journal of file ids, and these files get new ones, so
//! each computer does one full pass after the switch. Nothing is lost by it:
//! the content and the mtimes come across, and the desktop app matches
//! files by both before it transfers anything.
//!
//! The rest of what Nextcloud held lives in its database: calendars and
//! tasks, address books, Deck boards, and where each file in its trash bin
//! came from. `modules/cloud.nix` dumps those tables to JSON (one file per
//! table, `row_to_json` of every row) before Nextcloud's server is gone, and
//! [`database`] reads the dump: calendars and address books into
//! [`crate::cloud::pim`], boards into [`crate::cloud::boards`]. Whatever has
//! no place here yet (a calendar subscription, a board shared with a group)
//! is written into the person's files as `From Nextcloud/Not moved.txt`, so
//! nothing goes without a trace.

use crate::cloud::boards::{self, NewBoard, NewCard, NewColumn};
use crate::cloud::pim::{self, CollectionFields, Expect, Kind, PutError};
use crate::cloud::store::{join, valid_name, valid_owner, StoreError};
use crate::cloud::Cloud;
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;

/// Import every account found under `data`. Returns each login and how
/// many entries its scan picked up.
pub fn nextcloud(cloud: &Cloud, data: &Path) -> Result<Vec<(String, usize)>, StoreError> {
    let mut out = Vec::new();
    for e in fs::read_dir(data)? {
        let e = e?;
        let Some(login) = e.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let files = e.path().join("files");
        if !valid_owner(&login) || !files.is_dir() {
            continue;
        }
        cloud.store.ensure_home(&login)?;
        let home = cloud.store.disk_path(&login, "");
        move_tree(&files, &home, "")?;
        let n = cloud.store.scan(&login)?;
        out.push((login, n));
    }
    Ok(out)
}

fn move_tree(from: &Path, to: &Path, rel: &str) -> Result<(), StoreError> {
    for e in fs::read_dir(from.join(rel))? {
        let e = e?;
        let Some(name) = e.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if !valid_name(&name) {
            continue;
        }
        let child = join(rel, &name);
        let src = from.join(&child);
        let dst = to.join(&child);
        let meta = fs::symlink_metadata(&src)?;
        if meta.file_type().is_symlink() {
            continue;
        }
        if !dst.exists() {
            fs::rename(&src, &dst)?;
        } else if meta.is_dir() && dst.is_dir() {
            move_tree(from, to, &child)?;
        } else {
            let (stem, ext) = match name.rsplit_once('.') {
                Some((s, x)) if !s.is_empty() && !meta.is_dir() => (s.to_owned(), format!(".{x}")),
                _ => (name.clone(), String::new()),
            };
            let mut alt = dst.with_file_name(format!("{stem} (from Nextcloud){ext}"));
            let mut n = 2;
            while alt.exists() {
                alt = dst.with_file_name(format!("{stem} (from Nextcloud {n}){ext}"));
                n += 1;
            }
            fs::rename(&src, &alt)?;
        }
    }
    Ok(())
}

/// What one person got from Nextcloud.
#[derive(Debug, Default, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Moved {
    pub files: usize,
    pub trash: usize,
    pub calendars: usize,
    pub events: usize,
    pub tasks: usize,
    pub address_books: usize,
    pub contacts: usize,
    pub boards: usize,
    pub cards: usize,
    pub app_passwords: usize,
    /// What stayed behind, one line each, also in `From Nextcloud/Not moved.txt`.
    pub not_moved: Vec<String>,
}

/// The whole import, per login.
#[derive(Debug, Default, Clone, Serialize, PartialEq, Eq)]
pub struct Report {
    pub people: BTreeMap<String, Moved>,
}

const IMPORT_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS import_done (
    kind    TEXT NOT NULL,
    source  TEXT NOT NULL,
    PRIMARY KEY (kind, source)
);
"#;

/// One table of the dump: every row as a JSON object. A missing file is a
/// table Nextcloud never made (an app never enabled): no rows.
fn table(dump: &Path, name: &str) -> Result<Vec<Value>, StoreError> {
    let path = dump.join(format!("{name}.json"));
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    match serde_json::from_str::<Value>(&text) {
        Ok(Value::Array(rows)) => Ok(rows),
        Ok(Value::Null) => Ok(Vec::new()),
        _ => Err(StoreError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("{} is not a JSON list of rows", path.display()),
        ))),
    }
}

fn s(row: &Value, key: &str) -> Option<String> {
    match row.get(key)? {
        Value::String(v) => Some(v.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn i(row: &Value, key: &str) -> Option<i64> {
    match row.get(key)? {
        Value::Number(n) => n.as_i64(),
        Value::String(v) => v.trim().parse().ok(),
        Value::Bool(b) => Some(*b as i64),
        _ => None,
    }
}

fn b(row: &Value, key: &str) -> bool {
    match row.get(key) {
        Some(Value::Bool(v)) => *v,
        Some(Value::Number(n)) => n.as_i64().unwrap_or(0) != 0,
        Some(Value::String(v)) => matches!(v.as_str(), "1" | "t" | "true"),
        _ => false,
    }
}

/// A bytea column as `row_to_json` writes it (`\x` and hex), or text.
fn text_of(row: &Value, key: &str) -> Option<String> {
    let v = s(row, key)?;
    if let Some(hex) = v.strip_prefix("\\x") {
        if hex.len() % 2 == 0 && hex.bytes().all(|c| c.is_ascii_hexdigit()) {
            let bytes: Vec<u8> = (0..hex.len())
                .step_by(2)
                .filter_map(|k| u8::from_str_radix(&hex[k..k + 2], 16).ok())
                .collect();
            return Some(String::from_utf8_lossy(&bytes).into_owned());
        }
    }
    Some(v)
}

/// A timestamp column: Unix seconds, or a date and time as Postgres writes
/// it in JSON (`2026-10-25T12:00:00`, maybe with a zone, read as UTC).
fn time_of(row: &Value, key: &str) -> Option<i64> {
    match row.get(key)? {
        Value::Number(n) => n.as_i64().filter(|v| *v > 0),
        Value::String(v) => {
            if let Ok(n) = v.trim().parse::<i64>() {
                return (n > 0).then_some(n);
            }
            let core: String = v
                .chars()
                .take(19)
                .filter(|c| c.is_ascii_digit() || *c == 'T' || *c == ' ')
                .map(|c| if c == ' ' { 'T' } else { c })
                .collect();
            pim::parse_time(&core).map(|t| t.0)
        }
        _ => None,
    }
}

/// `principals/users/<login>` to the login, if it is a person's.
fn principal_login(row: &Value) -> Option<String> {
    let p = s(row, "principaluri")?;
    let login = p.strip_prefix("principals/users/")?;
    valid_owner(login).then(|| login.to_owned())
}

fn done(cloud: &Cloud, kind: &str, source: &str) -> Result<bool, StoreError> {
    cloud.store.with_db(|db| {
        Ok(db
            .query_row(
                "SELECT 1 FROM import_done WHERE kind = ?1 AND source = ?2",
                rusqlite::params![kind, source],
                |_| Ok(()),
            )
            .is_ok())
    })
}

fn mark(cloud: &Cloud, kind: &str, source: &str) -> Result<(), StoreError> {
    cloud.store.with_db(|db| {
        db.execute(
            "INSERT OR IGNORE INTO import_done (kind, source) VALUES (?1, ?2)",
            rusqlite::params![kind, source],
        )?;
        Ok(())
    })
}

/// Read a Nextcloud database dump into the cloud. Safe to run again: what
/// came across once is not brought twice.
pub fn database(cloud: &Cloud, dump: &Path) -> Result<Report, StoreError> {
    cloud
        .store
        .with_db(|db| Ok(db.execute_batch(IMPORT_SCHEMA)?))?;
    let mut report = Report::default();
    for u in table(dump, "users")? {
        if let Some(uid) = s(&u, "uid").filter(|l| valid_owner(l)) {
            report.people.entry(uid).or_default();
        }
    }
    calendars(cloud, dump, &mut report)?;
    address_books(cloud, dump, &mut report)?;
    deck(cloud, dump, &mut report)?;
    for sub in table(dump, "calendarsubscriptions")? {
        if let Some(login) = principal_login(&sub) {
            let name = s(&sub, "displayname").unwrap_or_default();
            let source = s(&sub, "source").unwrap_or_default();
            report
                .people
                .entry(login)
                .or_default()
                .not_moved
                .push(format!(
                    "Calendar subscription \"{name}\": {source} (add it again in your calendar app)"
                ));
        }
    }
    Ok(report)
}

fn calendars(cloud: &Cloud, dump: &Path, report: &mut Report) -> Result<(), StoreError> {
    let mut by_id = HashMap::new();
    for row in table(dump, "calendars")? {
        let (Some(login), Some(id), Some(uri)) =
            (principal_login(&row), i(&row, "id"), s(&row, "uri"))
        else {
            continue;
        };
        // Nextcloud makes the birthdays calendar out of the contacts; the
        // contacts come across, the copy does not need to.
        if uri == "contact_birthdays" || time_of(&row, "deleted_at").is_some() {
            continue;
        }
        let fields = CollectionFields {
            name: s(&row, "displayname").filter(|n| !n.is_empty()),
            description: s(&row, "description"),
            color: s(&row, "calendarcolor").filter(|c| !c.is_empty()),
            components: s(&row, "components").filter(|c| !c.is_empty()),
            timezone: s(&row, "timezone").filter(|t| !t.is_empty()),
            position: i(&row, "calendarorder"),
        };
        cloud.store.ensure_home(&login)?;
        let c = match pim::collection(&cloud.store, &login, Kind::Calendar, &uri) {
            Ok(c) => c,
            Err(StoreError::NotFound) => {
                report.people.entry(login.clone()).or_default().calendars += 1;
                pim::create(&cloud.store, &login, Kind::Calendar, &uri, &fields)?
            }
            Err(e) => return Err(e),
        };
        by_id.insert(id, c);
    }
    for row in table(dump, "calendarobjects")? {
        if i(&row, "calendartype").unwrap_or(0) != 0 || time_of(&row, "deleted_at").is_some() {
            continue;
        }
        let Some(c) = i(&row, "calendarid").and_then(|id| by_id.get(&id)) else {
            continue;
        };
        let (Some(uri), Some(data)) = (s(&row, "uri"), text_of(&row, "calendardata")) else {
            continue;
        };
        let moved = report.people.entry(c.owner.clone()).or_default();
        match put_once(cloud, c, &uri, &data) {
            Ok(()) => {
                if pim::facts(Kind::Calendar, &data)
                    .and_then(|f| f.component)
                    .as_deref()
                    == Some("VTODO")
                {
                    moved.tasks += 1;
                } else {
                    moved.events += 1;
                }
            }
            Err(why) => keep_aside(cloud, &c.owner, &c.name, &uri, &data, &why, moved)?,
        }
    }
    Ok(())
}

fn address_books(cloud: &Cloud, dump: &Path, report: &mut Report) -> Result<(), StoreError> {
    let mut by_id = HashMap::new();
    for row in table(dump, "addressbooks")? {
        let (Some(login), Some(id), Some(uri)) =
            (principal_login(&row), i(&row, "id"), s(&row, "uri"))
        else {
            continue;
        };
        let fields = CollectionFields {
            name: s(&row, "displayname").filter(|n| !n.is_empty()),
            description: s(&row, "description"),
            ..Default::default()
        };
        cloud.store.ensure_home(&login)?;
        let c = match pim::collection(&cloud.store, &login, Kind::AddressBook, &uri) {
            Ok(c) => c,
            Err(StoreError::NotFound) => {
                report
                    .people
                    .entry(login.clone())
                    .or_default()
                    .address_books += 1;
                pim::create(&cloud.store, &login, Kind::AddressBook, &uri, &fields)?
            }
            Err(e) => return Err(e),
        };
        by_id.insert(id, c);
    }
    for row in table(dump, "cards")? {
        let Some(c) = i(&row, "addressbookid").and_then(|id| by_id.get(&id)) else {
            continue;
        };
        let (Some(uri), Some(data)) = (s(&row, "uri"), text_of(&row, "carddata")) else {
            continue;
        };
        let moved = report.people.entry(c.owner.clone()).or_default();
        match put_once(cloud, c, &uri, &data) {
            Ok(()) => moved.contacts += 1,
            Err(why) => keep_aside(cloud, &c.owner, &c.name, &uri, &data, &why, moved)?,
        }
    }
    Ok(())
}

/// Put an object unless the same text is already there; a name taken by
/// different text gets the Nextcloud copy under a new name.
fn put_once(cloud: &Cloud, c: &pim::Collection, uri: &str, data: &str) -> Result<(), String> {
    if let Ok(o) = pim::object(&cloud.store, c, uri) {
        if o.data == data {
            return Ok(());
        }
    }
    let mut name = uri.to_owned();
    if !pim::valid_uri(&name) {
        name = format!(
            "{}.{}",
            crate::cloud::util::sha1_hex(uri.as_bytes()),
            c.kind.ext()
        );
    }
    for attempt in 0..2 {
        let expect = if attempt == 0 {
            Expect::New
        } else {
            Expect::Any
        };
        match pim::put(&cloud.store, c, &name, data, expect) {
            Ok(_) => return Ok(()),
            Err(PutError::Store(StoreError::Precondition)) => {
                name = format!("nextcloud-{name}");
            }
            Err(PutError::Invalid) => {
                return Err("not a calendar or contact the cloud can read".into())
            }
            Err(PutError::Unsupported) => {
                return Err("a kind of entry this calendar does not take".into())
            }
            Err(PutError::UidConflict(other)) => {
                // Same UID as an entry already here: the same event twice.
                if let Ok(o) = pim::object(&cloud.store, c, &other) {
                    if o.data == data {
                        return Ok(());
                    }
                }
                return Err(format!("the same entry as {other}, with different details"));
            }
            Err(PutError::Store(e)) => return Err(e.to_string()),
        }
    }
    Err("the name was taken twice".into())
}

/// An entry that could not go in: its text goes into the person's files.
fn keep_aside(
    cloud: &Cloud,
    owner: &str,
    collection: &str,
    uri: &str,
    data: &str,
    why: &str,
    moved: &mut Moved,
) -> Result<(), StoreError> {
    let folder = "From Nextcloud";
    cloud.store.mkdir_all(owner, folder)?;
    let safe: String = uri
        .chars()
        .map(|c| if c == '/' || c == '\\' { '_' } else { c })
        .collect();
    let path = join(folder, &safe);
    cloud.store.write_bytes(owner, &path, data.as_bytes())?;
    moved.not_moved.push(format!(
        "\"{collection}\": {uri} ({why}); its text is in {path}"
    ));
    Ok(())
}

fn deck(cloud: &Cloud, dump: &Path, report: &mut Report) -> Result<(), StoreError> {
    let boards_rows = table(dump, "deck_boards")?;
    if boards_rows.is_empty() {
        return Ok(());
    }
    let stacks = table(dump, "deck_stacks")?;
    let cards = table(dump, "deck_cards")?;
    let labels = table(dump, "deck_labels")?;
    let assigned_labels = table(dump, "deck_assigned_labels")?;
    let assigned_users = table(dump, "deck_assigned_users")?;
    let acl = table(dump, "deck_board_acl")?;
    let comments = table(dump, "comments")?;
    let alive = |row: &Value| time_of(row, "deleted_at").is_none();
    for row in boards_rows.iter().filter(|r| alive(r)) {
        let (Some(id), Some(owner)) = (i(row, "id"), s(row, "owner").filter(|o| valid_owner(o)))
        else {
            continue;
        };
        if done(cloud, "deck-board", &id.to_string())? {
            continue;
        }
        let title = s(row, "title").unwrap_or_else(|| "Board".into());
        let moved = report.people.entry(owner.clone()).or_default();
        let mut board = NewBoard {
            title: title.clone(),
            color: s(row, "color").filter(|c| !c.is_empty()),
            owner: owner.clone(),
            archived: b(row, "archived"),
            updated: time_of(row, "last_modified"),
            ..Default::default()
        };
        for a in acl.iter().filter(|a| i(a, "board_id") == Some(id)) {
            let who = s(a, "participant").unwrap_or_default();
            if i(a, "type").unwrap_or(0) == 0 && valid_owner(&who) {
                board.members.push((
                    who,
                    b(a, "permission_edit"),
                    b(a, "permission_share"),
                    b(a, "permission_manage"),
                ));
            } else {
                moved.not_moved.push(format!(
                    "Board \"{title}\" was shared with the group or circle \"{who}\"; share it with each person again"
                ));
            }
        }
        let mut label_index = HashMap::new();
        for l in labels.iter().filter(|l| i(l, "board_id") == Some(id)) {
            if let Some(lid) = i(l, "id") {
                label_index.insert(lid, board.labels.len());
                board.labels.push((
                    s(l, "title").unwrap_or_default(),
                    s(l, "color").filter(|c| !c.is_empty()),
                ));
            }
        }
        let mut cols: Vec<&Value> = stacks
            .iter()
            .filter(|st| i(st, "board_id") == Some(id) && alive(st))
            .collect();
        cols.sort_by_key(|st| (i(st, "order").unwrap_or(0), i(st, "id").unwrap_or(0)));
        for st in cols {
            let sid = i(st, "id");
            let mut col = NewColumn {
                title: s(st, "title").unwrap_or_default(),
                cards: Vec::new(),
            };
            let mut list: Vec<&Value> = cards
                .iter()
                .filter(|c| i(c, "stack_id") == sid && alive(c))
                .collect();
            list.sort_by_key(|c| (i(c, "order").unwrap_or(0), i(c, "id").unwrap_or(0)));
            for c in list {
                let cid = i(c, "id");
                let cid_text = cid.map(|v| v.to_string()).unwrap_or_default();
                let mut card = NewCard {
                    title: s(c, "title").unwrap_or_default(),
                    description: s(c, "description").unwrap_or_default(),
                    due: time_of(c, "duedate"),
                    archived: b(c, "archived"),
                    done_at: time_of(c, "done"),
                    created: time_of(c, "created_at"),
                    updated: time_of(c, "last_modified"),
                    ..Default::default()
                };
                for al in assigned_labels.iter().filter(|al| i(al, "card_id") == cid) {
                    if let Some(ix) = i(al, "label_id").and_then(|l| label_index.get(&l)) {
                        card.labels.push(*ix);
                    }
                }
                for au in assigned_users.iter().filter(|au| i(au, "card_id") == cid) {
                    let who = s(au, "participant").unwrap_or_default();
                    if i(au, "type").unwrap_or(0) == 0 && valid_owner(&who) {
                        card.assignees.push(who);
                    }
                }
                let mut notes: Vec<&Value> = comments
                    .iter()
                    .filter(|m| {
                        s(m, "object_type").as_deref() == Some("deckCard")
                            && s(m, "object_id").as_deref() == Some(cid_text.as_str())
                            && s(m, "verb").as_deref().unwrap_or("comment") == "comment"
                    })
                    .collect();
                notes.sort_by_key(|m| time_of(m, "creation_timestamp").unwrap_or(0));
                for m in notes {
                    card.comments.push((
                        s(m, "actor_id").unwrap_or_default(),
                        s(m, "message").unwrap_or_default(),
                        time_of(m, "creation_timestamp").unwrap_or(0),
                    ));
                }
                moved.cards += 1;
                col.cards.push(card);
            }
            board.columns.push(col);
        }
        cloud.store.ensure_home(&owner)?;
        boards::insert(&cloud.store, &board)?;
        mark(cloud, "deck-board", &id.to_string())?;
        moved.boards += 1;
    }
    Ok(())
}

/// Nextcloud's trash bin (`<data>/<login>/files_trashbin/files/`) into the
/// cloud's, keeping when and from where each thing was deleted, read from
/// the dump's `files_trash` table.
pub fn trash(
    cloud: &Cloud,
    data: &Path,
    dump: &Path,
) -> Result<BTreeMap<String, usize>, StoreError> {
    let mut whence: HashMap<(String, String), String> = HashMap::new();
    for row in table(dump, "files_trash")? {
        if let (Some(user), Some(id), Some(ts)) =
            (s(&row, "user"), s(&row, "id"), s(&row, "timestamp"))
        {
            let location = s(&row, "location").unwrap_or_default();
            let location = location.trim_matches('/');
            let original = if location.is_empty() || location == "." {
                id.clone()
            } else {
                format!("{location}/{id}")
            };
            whence.insert((user, format!("{id}.d{ts}")), original);
        }
    }
    let mut out = BTreeMap::new();
    for e in fs::read_dir(data)? {
        let e = e?;
        let Some(login) = e.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let bin = e.path().join("files_trashbin/files");
        if !valid_owner(&login) || !bin.is_dir() {
            continue;
        }
        cloud.store.ensure_home(&login)?;
        let mut n = 0;
        for item in fs::read_dir(&bin)? {
            let item = item?;
            let Some(stored) = item.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let (name, deleted) = match stored.rsplit_once(".d") {
                Some((name, ts)) if ts.bytes().all(|c| c.is_ascii_digit()) && !ts.is_empty() => {
                    (name.to_owned(), ts.parse().unwrap_or(0))
                }
                _ => (stored.clone(), crate::cloud::util::now()),
            };
            let original = whence
                .get(&(login.clone(), stored.clone()))
                .cloned()
                .unwrap_or(name);
            match cloud
                .store
                .adopt_trashed(&login, &item.path(), &original, deleted)
            {
                Ok(_) => n += 1,
                Err(StoreError::BadName) => {}
                Err(e) => return Err(e),
            }
        }
        out.insert(login, n);
    }
    Ok(out)
}

/// The `'secret' => '...'` of a Nextcloud `config.php`.
pub fn config_secret(php: &str) -> Option<String> {
    let at = php.find("'secret'")?;
    let rest = &php[at + "'secret'".len()..];
    let rest = rest.trim_start().strip_prefix("=>")?.trim_start();
    let quote = rest.chars().next().filter(|c| *c == '\'' || *c == '"')?;
    let value = &rest[1..];
    let end = value.find(quote)?;
    Some(value[..end].to_owned())
}

/// The app passwords Nextcloud gave the desktop and phone apps (permanent
/// tokens), so they go on syncing with the same password. Their hash only
/// works with the instance's secret, read from its `config.php`.
pub fn passwords(
    cloud: &Cloud,
    dump: &Path,
    config_php: &Path,
) -> Result<BTreeMap<String, usize>, StoreError> {
    let mut out = BTreeMap::new();
    let Some(secret) = fs::read_to_string(config_php)
        .ok()
        .and_then(|p| config_secret(&p))
    else {
        return Ok(out);
    };
    let mut tokens = Vec::new();
    for row in table(dump, "authtoken")? {
        if i(&row, "type") != Some(1) || i(&row, "expires").unwrap_or(0) != 0 {
            continue;
        }
        let login = s(&row, "login_name")
            .filter(|l| valid_owner(l))
            .or_else(|| s(&row, "uid").filter(|l| valid_owner(l)));
        let (Some(login), Some(hash)) = (login, s(&row, "token")) else {
            continue;
        };
        let name = s(&row, "name").unwrap_or_else(|| "Nextcloud app".into());
        let last = i(&row, "last_activity").unwrap_or(0);
        *out.entry(login.clone()).or_default() += 1;
        tokens.push((login, name, hash, last));
    }
    cloud
        .auth
        .adopt_nextcloud_passwords(&cloud.store, &secret, &tokens)?;
    Ok(out)
}

/// Write each person's `From Nextcloud/Not moved.txt`, when something was.
pub fn write_notes(cloud: &Cloud, report: &Report) -> Result<(), StoreError> {
    for (login, moved) in &report.people {
        if moved.not_moved.is_empty() {
            continue;
        }
        cloud.store.ensure_home(login)?;
        cloud.store.mkdir_all(login, "From Nextcloud")?;
        let mut text = String::from(
            "These things from Nextcloud have no place in LosOS Cloud yet.\nNothing here was deleted.\n\n",
        );
        for line in &moved.not_moved {
            text.push_str("- ");
            text.push_str(line);
            text.push('\n');
        }
        cloud
            .store
            .write_bytes(login, "From Nextcloud/Not moved.txt", text.as_bytes())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloud::Config;

    #[test]
    fn a_nextcloud_folder_moves_in_and_clashes_keep_both() {
        let data = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let nc = data.path().join("notshared/files");
        fs::create_dir_all(nc.join("Photos")).unwrap();
        fs::write(nc.join("Photos/a.jpg"), b"jpeg").unwrap();
        fs::write(nc.join("notes.txt"), b"from nc").unwrap();
        fs::create_dir_all(data.path().join("appdata_oc123")).unwrap();
        let cloud = Cloud::open(root.path(), Config::default()).unwrap();
        cloud.store.ensure_home("notshared").unwrap();
        cloud
            .store
            .write_bytes("notshared", "notes.txt", b"ours")
            .unwrap();
        let r = nextcloud(&cloud, data.path()).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(
            cloud.store.read_bytes("notshared", "notes.txt").unwrap(),
            b"ours"
        );
        assert_eq!(
            cloud
                .store
                .read_bytes("notshared", "notes (from Nextcloud).txt")
                .unwrap(),
            b"from nc"
        );
        assert_eq!(
            cloud.store.stat("notshared", "Photos/a.jpg").unwrap().size,
            4
        );
        assert!(!nc.join("Photos/a.jpg").exists(), "moved, not copied");
    }

    fn hex(t: &str) -> String {
        format!("\\x{}", crate::cloud::util::hex(t.as_bytes()))
    }

    #[test]
    fn a_nextcloud_database_dump_comes_across_and_a_second_run_adds_nothing() {
        let dump = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let w = |name: &str, v: serde_json::Value| {
            fs::write(dump.path().join(format!("{name}.json")), v.to_string()).unwrap()
        };
        let event = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:e1\r\nDTSTART:20261023T090000Z\r\nSUMMARY:Dentist\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let task = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VTODO\r\nUID:t1\r\nSUMMARY:Milk\r\nEND:VTODO\r\nEND:VCALENDAR\r\n";
        let card = "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:c1\r\nFN:Eva\r\nEND:VCARD\r\n";
        w(
            "users",
            serde_json::json!([{"uid": "notshared"}, {"uid": "eva"}]),
        );
        w(
            "calendars",
            serde_json::json!([
                {"id": 1, "principaluri": "principals/users/notshared", "uri": "personal", "displayname": "Personal", "calendarcolor": "#0082c9", "components": "VEVENT,VTODO", "deleted_at": null},
                {"id": 2, "principaluri": "principals/users/notshared", "uri": "contact_birthdays", "displayname": "Birthdays", "components": "VEVENT"},
                {"id": 3, "principaluri": "principals/system/system", "uri": "x", "components": "VEVENT"}
            ]),
        );
        w(
            "calendarobjects",
            serde_json::json!([
                {"calendarid": 1, "uri": "e1.ics", "calendardata": hex(event), "calendartype": 0, "deleted_at": null},
                {"calendarid": 1, "uri": "t1.ics", "calendardata": hex(task), "calendartype": 0},
                {"calendarid": 1, "uri": "gone.ics", "calendardata": hex(event), "calendartype": 0, "deleted_at": 1700000000},
                {"calendarid": 1, "uri": "bad.ics", "calendardata": hex("not a calendar"), "calendartype": 0}
            ]),
        );
        w(
            "addressbooks",
            serde_json::json!([{"id": 7, "principaluri": "principals/users/eva", "uri": "contacts", "displayname": "Contacts"}]),
        );
        w(
            "cards",
            serde_json::json!([{"addressbookid": 7, "uri": "c1.vcf", "carddata": hex(card)}]),
        );
        w(
            "deck_boards",
            serde_json::json!([
                {"id": 10, "title": "Garden shed", "owner": "notshared", "color": "31CC7C", "archived": false, "deleted_at": 0, "last_modified": 1760000000},
                {"id": 11, "title": "Deleted", "owner": "notshared", "deleted_at": 1760000000}
            ]),
        );
        w(
            "deck_stacks",
            serde_json::json!([
                {"id": 21, "board_id": 10, "title": "Done", "order": 2, "deleted_at": 0},
                {"id": 20, "board_id": 10, "title": "To do", "order": 1, "deleted_at": 0}
            ]),
        );
        w(
            "deck_cards",
            serde_json::json!([
                {"id": 30, "stack_id": 20, "title": "Buy boards", "description": "48 larch", "order": 0, "duedate": "2026-10-25T12:00:00", "archived": false, "deleted_at": 0, "done": null},
                {"id": 31, "stack_id": 21, "title": "Pour the base", "order": 0, "deleted_at": 0, "done": "2026-10-01T08:00:00"}
            ]),
        );
        w(
            "deck_labels",
            serde_json::json!([{"id": 40, "board_id": 10, "title": "Wood", "color": "31CC7C"}]),
        );
        w(
            "deck_assigned_labels",
            serde_json::json!([{"label_id": 40, "card_id": 30}]),
        );
        w(
            "deck_assigned_users",
            serde_json::json!([{"card_id": 30, "participant": "eva", "type": 0}]),
        );
        w(
            "deck_board_acl",
            serde_json::json!([
                {"board_id": 10, "type": 0, "participant": "eva", "permission_edit": true, "permission_share": false, "permission_manage": false},
                {"board_id": 10, "type": 1, "participant": "family", "permission_edit": true}
            ]),
        );
        w(
            "comments",
            serde_json::json!([{"object_type": "deckCard", "object_id": "30", "actor_id": "eva", "message": "I'll pick them up", "verb": "comment", "creation_timestamp": "2026-10-20T10:00:00"}]),
        );
        w(
            "calendarsubscriptions",
            serde_json::json!([{"principaluri": "principals/users/notshared", "displayname": "Holidays", "source": "https://example.org/h.ics"}]),
        );

        let cloud = Cloud::open(root.path(), Config::default()).unwrap();
        let r = database(&cloud, dump.path()).unwrap();
        let me = &r.people["notshared"];
        assert_eq!((me.calendars, me.events, me.tasks), (1, 1, 1));
        assert_eq!((me.boards, me.cards), (1, 2));
        assert_eq!(me.not_moved.len(), 3, "{:?}", me.not_moved);
        assert!(me.not_moved.iter().any(|l| l.contains("bad.ics")));
        assert!(me.not_moved.iter().any(|l| l.contains("family")));
        assert!(me
            .not_moved
            .iter()
            .any(|l| l.contains("https://example.org/h.ics")));
        assert_eq!(
            cloud
                .store
                .read_bytes("notshared", "From Nextcloud/bad.ics")
                .unwrap(),
            b"not a calendar"
        );
        assert_eq!(r.people["eva"].contacts, 1);

        let cal = pim::collection(&cloud.store, "notshared", Kind::Calendar, "personal").unwrap();
        assert_eq!(
            pim::object(&cloud.store, &cal, "e1.ics").unwrap().data,
            event
        );
        assert!(pim::collection(
            &cloud.store,
            "notshared",
            Kind::Calendar,
            "contact_birthdays"
        )
        .is_err());

        let list = boards::list(&cloud.store, "eva").unwrap();
        assert_eq!(list.len(), 1, "shared with eva");
        let full = boards::get(&cloud.store, "notshared", list[0].id).unwrap();
        assert_eq!(full.columns[0].title, "To do");
        let c = &full.columns[0].cards[0];
        assert_eq!(c.labels, vec!["Wood".to_owned()]);
        assert_eq!(c.assignees, vec!["eva".to_owned()]);
        assert_eq!(c.comments[0].body, "I'll pick them up");
        assert_eq!(c.due, pim::parse_time("20261025T120000").map(|t| t.0));
        assert!(full.columns[1].cards[0].done_at.is_some());

        write_notes(&cloud, &r).unwrap();
        let note = String::from_utf8(
            cloud
                .store
                .read_bytes("notshared", "From Nextcloud/Not moved.txt")
                .unwrap(),
        )
        .unwrap();
        assert!(note.contains("Holidays"));

        // Again: nothing doubles.
        let again = database(&cloud, dump.path()).unwrap();
        assert_eq!(again.people["notshared"].boards, 0);
        assert_eq!(boards::list(&cloud.store, "notshared").unwrap().len(), 1);
        assert_eq!(pim::count(&cloud.store, &cal).unwrap(), 2);
    }

    #[test]
    fn nextclouds_trash_lands_in_the_trash_with_its_time_and_place() {
        let data = tempfile::tempdir().unwrap();
        let dump = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let bin = data.path().join("notshared/files_trashbin/files");
        fs::create_dir_all(bin.join("Old folder.d1760000000")).unwrap();
        fs::write(bin.join("Old folder.d1760000000/a.txt"), b"aaaa").unwrap();
        fs::write(bin.join("draft.odt.d1760000500"), b"draft").unwrap();
        fs::write(
            dump.path().join("files_trash.json"),
            serde_json::json!([
                {"user": "notshared", "id": "draft.odt", "timestamp": "1760000500", "location": "Documents"},
                {"user": "notshared", "id": "Old folder", "timestamp": "1760000000", "location": "."}
            ])
            .to_string(),
        )
        .unwrap();
        let cloud = Cloud::open(root.path(), Config::default()).unwrap();
        let n = trash(&cloud, data.path(), dump.path()).unwrap();
        assert_eq!(n["notshared"], 2);
        let mut t = cloud.store.trash("notshared").unwrap();
        t.sort_by_key(|x| x.deleted);
        assert_eq!(t[0].original, "Old folder");
        assert!(t[0].dir);
        assert_eq!(t[0].size, 4);
        assert_eq!(t[1].original, "Documents/draft.odt");
        assert_eq!(t[1].deleted, 1_760_000_500);
        // It can come back.
        cloud.store.ensure_home("notshared").unwrap();
        cloud.store.mkdir_all("notshared", "Documents").unwrap();
        let e = cloud.store.restore("notshared", t[1].id, None).unwrap();
        assert_eq!(e.path, "Documents/draft.odt");
    }

    #[test]
    fn nextclouds_app_passwords_keep_working() {
        let dump = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let secret = "s3cr3t+/=";
        let token = "AbCdEfGhIj".repeat(7) + "Kl";
        let user_made = "Abcde-Fghij-Klmno-Pqrst-Uvwxy";
        let hash = |t: &str| crate::cloud::util::sha512_hex(format!("{t}{secret}").as_bytes());
        fs::write(
            dump.path().join("authtoken.json"),
            serde_json::json!([
                {"uid": "notshared", "login_name": "notshared", "name": "Pixel 8", "token": hash(&token), "type": 1, "expires": null, "last_activity": 1760000000},
                {"uid": "notshared", "login_name": "notshared", "name": "Laptop", "token": hash(user_made), "type": 1},
                {"uid": "notshared", "login_name": "notshared", "name": "Firefox session", "token": hash("session"), "type": 0}
            ])
            .to_string(),
        )
        .unwrap();
        let php = dump.path().join("config.php");
        fs::write(&php, format!("<?php\n$CONFIG = array (\n  'instanceid' => 'oc123',\n  'secret' => '{secret}',\n);\n")).unwrap();
        assert_eq!(
            config_secret(&fs::read_to_string(&php).unwrap()).as_deref(),
            Some(secret)
        );
        let cloud = Cloud::open(root.path(), Config::default()).unwrap();
        let n = passwords(&cloud, dump.path(), &php).unwrap();
        assert_eq!(n["notshared"], 2);
        let basic = |u: &str, p: &str| {
            use base64::Engine;
            format!(
                "Basic {}",
                base64::engine::general_purpose::STANDARD.encode(format!("{u}:{p}"))
            )
        };
        let ok = |p: &str| {
            cloud
                .auth
                .identify(&cloud.store, None, None, None, Some(&basic("notshared", p)))
                .is_ok()
        };
        assert!(ok(&token));
        assert!(ok(user_made));
        assert!(!ok("session"), "a browser session is not an app password");
        assert!(
            cloud
                .auth
                .identify(&cloud.store, None, None, None, Some(&basic("eva", &token)))
                .is_err(),
            "only for its own login"
        );
        let names: Vec<String> = cloud
            .auth
            .app_passwords(&cloud.store, "notshared")
            .unwrap()
            .into_iter()
            .map(|p| p.name)
            .collect();
        assert!(names.contains(&"Pixel 8".to_owned()));
    }
}
