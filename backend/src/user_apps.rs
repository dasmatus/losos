//! The owner's own apps: small React apps kept on the box, with their data.
//!
//! An app is one JSX source (`app.jsx`) that the box compiles once, when it
//! is saved (`crate::jsx`), into the script the app frame runs. The builder
//! writes the first version from the owner's description; the owner can
//! change it by hand or ask the builder again. Nothing in an app runs on the
//! box itself: the script runs in the person's browser, in a sandboxed frame
//! (`admin-ui/app/public/app-frame/`), and reaches the box only through the
//! admin page, which asks this module for the app's data. So an app gets no
//! process, no port and no privilege, and needs no runtime of its own.
//!
//! # Data
//!
//! Each app keeps its own data in one JSON document beside the list of apps:
//! named collections of objects (`useCollection` in the frame) and named
//! values (`useStored`). Everyone who may open the app shares its data, as a
//! family shares a shopping list. Every write is one operation ([`DataOp`])
//! applied under lososd's lock and saved atomically, and the limits below
//! keep one app from filling `/persist`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::losos::Losos;

/// A request the owner can fix by sending a different one; the HTTP layer
/// answers it as a 400 carrying the sentence.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct Invalid(pub String);

/// Apps a box keeps.
pub const MAX_APPS: usize = 24;
/// Longest app name, in characters.
pub const MAX_NAME_CHARS: usize = 60;
/// Longest summary, in characters.
pub const MAX_SUMMARY_CHARS: usize = 600;
/// Largest source, in bytes.
pub const MAX_SOURCE_BYTES: usize = 64 * 1024;
/// Largest data document of one app, in bytes, as JSON.
pub const MAX_DATA_BYTES: usize = 2 * 1024 * 1024;
/// Largest single object or value, in bytes, as JSON.
pub const MAX_ITEM_BYTES: usize = 32 * 1024;
/// Collections and values one app may name, each.
pub const MAX_NAMES: usize = 32;
/// Longest collection or value name.
pub const MAX_KEY_CHARS: usize = 40;

/// One app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserApp {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub summary: Option<String>,
    /// What the owner asked the builder for, when the builder wrote it.
    #[serde(default)]
    pub prompt: Option<String>,
    /// The JSX the owner keeps and edits.
    pub source: String,
    /// The script the frame runs, compiled from `source`.
    pub code: String,
    #[serde(rename = "createdAt")]
    pub created_at: u64,
    #[serde(rename = "updatedAt")]
    pub updated_at: u64,
}

/// Every app, in the order they were made.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserApps {
    #[serde(default)]
    pub apps: Vec<UserApp>,
    #[serde(default = "one", rename = "nextId")]
    pub next_id: u64,
}

fn one() -> u64 {
    1
}

impl Default for UserApps {
    fn default() -> Self {
        UserApps {
            apps: Vec::new(),
            next_id: 1,
        }
    }
}

impl Default for AppData {
    fn default() -> Self {
        AppData {
            collections: BTreeMap::new(),
            stored: BTreeMap::new(),
            next_id: 1,
        }
    }
}

/// What one app keeps.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppData {
    /// Collections by name; every object has a string `id`.
    #[serde(default)]
    pub collections: BTreeMap<String, Vec<Map<String, Value>>>,
    /// Values by name.
    #[serde(default)]
    pub stored: BTreeMap<String, Value>,
    #[serde(default = "one", rename = "nextId")]
    pub next_id: u64,
}

/// A new app, or a new version of one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draft {
    pub id: Option<String>,
    pub name: String,
    pub summary: Option<String>,
    pub prompt: Option<String>,
    pub source: String,
}

/// One write to an app's data.
#[derive(Debug, Clone, PartialEq)]
pub enum DataOp {
    /// Add an object to a collection; the box gives it its `id`.
    Add {
        collection: String,
        value: Map<String, Value>,
    },
    /// Change some fields of one object.
    Update {
        collection: String,
        id: String,
        value: Map<String, Value>,
    },
    /// Delete one object.
    Remove { collection: String, id: String },
    /// Put a whole list in a collection's place, keeping the ids it has and
    /// giving the rest new ones.
    Replace {
        collection: String,
        items: Vec<Map<String, Value>>,
    },
    /// Keep one value.
    Set { key: String, value: Value },
}

fn invalid<T>(message: impl Into<String>) -> anyhow::Result<T> {
    Err(anyhow::Error::new(Invalid(message.into())))
}

/// An id this module made: `u` and digits.
#[must_use]
pub fn is_app_id(id: &str) -> bool {
    id.len() > 1
        && id.len() <= 20
        && id.starts_with('u')
        && id[1..].bytes().all(|b| b.is_ascii_digit())
}

/// A collection or value name: letters, digits, `-` and `_`.
#[must_use]
pub fn is_key(name: &str) -> bool {
    !name.is_empty()
        && name.chars().count() <= MAX_KEY_CHARS
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn text(doc: &Value, key: &str) -> Option<String> {
    doc.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Read `{"id"?, "name", "summary"?, "prompt"?, "source"}`.
#[must_use]
pub fn parse_draft(doc: &Value) -> Option<Draft> {
    Some(Draft {
        id: text(doc, "id"),
        name: doc.get("name")?.as_str()?.trim().to_string(),
        summary: text(doc, "summary"),
        prompt: text(doc, "prompt"),
        source: doc.get("source")?.as_str()?.to_string(),
    })
}

fn object(value: Option<&Value>) -> Option<Map<String, Value>> {
    match value {
        Some(Value::Object(map)) => Some(map.clone()),
        _ => None,
    }
}

/// Read one data operation: `{"op": "add" | "update" | "remove" | "replace"
/// | "set", ...}`.
#[must_use]
pub fn parse_op(doc: &Value) -> Option<DataOp> {
    let collection = || text(doc, "collection");
    Some(match doc.get("op")?.as_str()? {
        "add" => DataOp::Add {
            collection: collection()?,
            value: object(doc.get("value"))?,
        },
        "update" => DataOp::Update {
            collection: collection()?,
            id: text(doc, "id")?,
            value: object(doc.get("value"))?,
        },
        "remove" => DataOp::Remove {
            collection: collection()?,
            id: text(doc, "id")?,
        },
        "replace" => DataOp::Replace {
            collection: collection()?,
            items: doc
                .get("items")?
                .as_array()?
                .iter()
                .map(|v| object(Some(v)))
                .collect::<Option<Vec<_>>>()?,
        },
        "set" => DataOp::Set {
            key: text(doc, "key")?,
            value: doc.get("value")?.clone(),
        },
        _ => return None,
    })
}

fn listed(app: &UserApp) -> Value {
    json!({
        "id": app.id,
        "name": app.name,
        "summary": app.summary,
        "createdAt": app.created_at,
        "updatedAt": app.updated_at,
    })
}

fn limits() -> Value {
    json!({
        "apps": MAX_APPS,
        "nameChars": MAX_NAME_CHARS,
        "sourceBytes": MAX_SOURCE_BYTES,
        "dataBytes": MAX_DATA_BYTES,
    })
}

/// `GET /api/user-apps`: the apps, without their code.
pub fn cmd_list<L: Losos>(l: &mut L) -> anyhow::Result<Value> {
    let apps = l.load_user_apps()?;
    Ok(json!({
        "apps": apps.apps.iter().map(listed).collect::<Vec<_>>(),
        "limits": limits(),
    }))
}

/// `GET /api/user-apps/{id}`: one app with its source and its script.
pub fn cmd_get<L: Losos>(l: &mut L, id: &str) -> anyhow::Result<Option<Value>> {
    let apps = l.load_user_apps()?;
    Ok(apps
        .apps
        .iter()
        .find(|a| a.id == id)
        .map(|app| json!({ "app": app })))
}

/// `POST /api/user-apps/check`: what [`crate::jsx::check`] says of a source,
/// and the script it makes. Saves nothing.
pub fn cmd_check(source: &str) -> anyhow::Result<Value> {
    if source.len() > MAX_SOURCE_BYTES {
        return invalid(format!(
            "the app is larger than {} KiB",
            MAX_SOURCE_BYTES / 1024
        ));
    }
    let checked = crate::jsx::check(source);
    Ok(json!({ "code": checked.code, "problems": checked.problems }))
}

/// `POST /api/user-apps`: add an app, or save a new version of the one the
/// draft's id names. The source must compile; problems that would show only
/// when it runs are saved with it and returned, so the owner sees them.
pub fn cmd_put<L: Losos>(l: &mut L, draft: &Draft) -> anyhow::Result<Value> {
    let name = draft.name.trim();
    if name.is_empty() {
        return invalid("give the app a name");
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return invalid(format!(
            "the name is longer than {MAX_NAME_CHARS} characters"
        ));
    }
    if name.chars().any(char::is_control) {
        return invalid("the name has a control character in it");
    }
    if draft.source.len() > MAX_SOURCE_BYTES {
        return invalid(format!(
            "the app is larger than {} KiB",
            MAX_SOURCE_BYTES / 1024
        ));
    }
    let checked = crate::jsx::check(&draft.source);
    let Some(code) = checked.code else {
        let first = checked.problems.first().cloned().unwrap_or_default();
        return invalid(format!("the app does not compile. {first}"));
    };
    let summary = draft
        .summary
        .as_deref()
        .and_then(crate::builder::clean_summary)
        .map(|s| s.chars().take(MAX_SUMMARY_CHARS).collect());
    let prompt = draft
        .prompt
        .as_ref()
        .map(|p| p.chars().take(crate::builder::MAX_PROMPT_CHARS).collect());
    let now = l.now();
    let mut apps = l.load_user_apps()?;
    let app = match &draft.id {
        Some(id) => {
            let Some(existing) = apps.apps.iter_mut().find(|a| &a.id == id) else {
                return invalid("there is no app with that id on this box");
            };
            existing.name = name.to_string();
            existing.source.clone_from(&draft.source);
            existing.code = code;
            if summary.is_some() {
                existing.summary = summary;
            }
            if prompt.is_some() {
                existing.prompt = prompt;
            }
            existing.updated_at = now;
            existing.clone()
        }
        None => {
            if apps.apps.len() >= MAX_APPS {
                return invalid(format!("this box keeps at most {MAX_APPS} apps"));
            }
            let app = UserApp {
                id: format!("u{}", apps.next_id),
                name: name.to_string(),
                summary,
                prompt,
                source: draft.source.clone(),
                code,
                created_at: now,
                updated_at: now,
            };
            apps.next_id += 1;
            apps.apps.push(app.clone());
            app
        }
    };
    l.save_user_apps(&apps)?;
    Ok(json!({ "app": app, "problems": checked.problems }))
}

/// `DELETE /api/user-apps/{id}`: the app and its data. Idempotent.
pub fn cmd_delete<L: Losos>(l: &mut L, id: &str) -> anyhow::Result<Value> {
    if !is_app_id(id) {
        return invalid("that is not an app id");
    }
    let mut apps = l.load_user_apps()?;
    let before = apps.apps.len();
    apps.apps.retain(|a| a.id != id);
    let deleted = apps.apps.len() != before;
    if deleted {
        l.save_user_apps(&apps)?;
        l.remove_app_data(id)?;
    }
    Ok(json!({ "deleted": deleted }))
}

fn known<L: Losos>(l: &mut L, id: &str) -> anyhow::Result<bool> {
    Ok(is_app_id(id) && l.load_user_apps()?.apps.iter().any(|a| a.id == id))
}

/// `GET /api/user-apps/{id}/data`: everything the app keeps, or `None` for
/// an app the box does not have.
pub fn cmd_data<L: Losos>(l: &mut L, id: &str) -> anyhow::Result<Option<Value>> {
    if !known(l, id)? {
        return Ok(None);
    }
    let data = l.load_app_data(id)?;
    Ok(Some(json!({
        "collections": data.collections,
        "stored": data.stored,
    })))
}

fn size(value: &impl Serialize) -> usize {
    serde_json::to_vec(value).map_or(usize::MAX, |v| v.len())
}

fn check_item(value: &impl Serialize) -> anyhow::Result<()> {
    if size(value) > MAX_ITEM_BYTES {
        return invalid(format!(
            "one entry may be at most {} KiB",
            MAX_ITEM_BYTES / 1024
        ));
    }
    Ok(())
}

fn check_key(name: &str, names: usize, exists: bool) -> anyhow::Result<()> {
    if !is_key(name) {
        return invalid("a collection or value name is letters, digits, - and _, at most 40");
    }
    if !exists && names >= MAX_NAMES {
        return invalid(format!(
            "an app keeps at most {MAX_NAMES} collections and {MAX_NAMES} values"
        ));
    }
    Ok(())
}

fn new_id(data: &mut AppData) -> String {
    let id = format!("i{}", data.next_id);
    data.next_id += 1;
    id
}

/// `POST /api/user-apps/{id}/data`: one write. Answers with what changed:
/// the object added or updated, or the collection or value as it now is.
/// `None` for an app the box does not have.
pub fn cmd_data_op<L: Losos>(l: &mut L, id: &str, op: &DataOp) -> anyhow::Result<Option<Value>> {
    if !known(l, id)? {
        return Ok(None);
    }
    let mut data = l.load_app_data(id)?;
    let collections = data.collections.len();
    let answer = match op {
        DataOp::Add { collection, value } => {
            check_key(
                collection,
                collections,
                data.collections.contains_key(collection),
            )?;
            check_item(value)?;
            let mut item = value.clone();
            item.insert("id".into(), Value::String(new_id(&mut data)));
            item.insert("createdAt".into(), json!(l.now() * 1000));
            data.collections
                .entry(collection.clone())
                .or_default()
                .push(item.clone());
            json!({ "item": item })
        }
        DataOp::Update {
            collection,
            id,
            value,
        } => {
            check_item(value)?;
            let Some(item) = data.collections.get_mut(collection).and_then(|items| {
                items
                    .iter_mut()
                    .find(|i| i.get("id").and_then(Value::as_str) == Some(id))
            }) else {
                return invalid("there is no such entry; it may have been deleted");
            };
            for (k, v) in value {
                if k != "id" {
                    item.insert(k.clone(), v.clone());
                }
            }
            check_item(item)?;
            json!({ "item": item })
        }
        DataOp::Remove { collection, id } => {
            let removed = data.collections.get_mut(collection).is_some_and(|items| {
                let before = items.len();
                items.retain(|i| i.get("id").and_then(Value::as_str) != Some(id));
                items.len() != before
            });
            json!({ "removed": removed })
        }
        DataOp::Replace { collection, items } => {
            check_key(
                collection,
                collections,
                data.collections.contains_key(collection),
            )?;
            let mut kept = Vec::with_capacity(items.len());
            for item in items {
                check_item(item)?;
                let mut item = item.clone();
                let has_id = item
                    .get("id")
                    .and_then(Value::as_str)
                    .is_some_and(|i| !i.is_empty());
                if !has_id {
                    item.insert("id".into(), Value::String(new_id(&mut data)));
                }
                kept.push(item);
            }
            data.collections.insert(collection.clone(), kept.clone());
            json!({ "items": kept })
        }
        DataOp::Set { key, value } => {
            check_key(key, data.stored.len(), data.stored.contains_key(key))?;
            check_item(value)?;
            data.stored.insert(key.clone(), value.clone());
            json!({ "value": value })
        }
    };
    if size(&data) > MAX_DATA_BYTES {
        return invalid(format!(
            "the app keeps at most {} MiB; delete something first",
            MAX_DATA_BYTES / (1024 * 1024)
        ));
    }
    l.save_app_data(id, &data)?;
    Ok(Some(answer))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::FakeLosos;

    const SOURCE: &str = "function App() { const notes = useCollection('notes'); return <Page title=\"Notes\">{notes.items.length}</Page>; }";

    fn draft(name: &str, source: &str) -> Draft {
        Draft {
            id: None,
            name: name.into(),
            summary: Some("Notes.".into()),
            prompt: Some("a notes app".into()),
            source: source.into(),
        }
    }

    fn message(e: &anyhow::Error) -> String {
        e.downcast_ref::<Invalid>().expect("an Invalid").0.clone()
    }

    #[test]
    fn an_app_is_saved_compiled_and_listed_without_its_code() {
        let mut l = FakeLosos::default();
        let saved = cmd_put(&mut l, &draft("Notes", SOURCE)).unwrap();
        assert_eq!(saved["app"]["id"], "u1");
        assert!(saved["app"]["code"]
            .as_str()
            .unwrap()
            .contains("window.LosOS.mount(App)"));
        assert_eq!(saved["problems"], json!([]));
        let list = cmd_list(&mut l).unwrap();
        assert_eq!(list["apps"][0]["name"], "Notes");
        assert!(list["apps"][0].get("code").is_none());
        let one = cmd_get(&mut l, "u1").unwrap().unwrap();
        assert_eq!(one["app"]["source"], SOURCE);
    }

    #[test]
    fn a_new_version_keeps_the_id_and_the_data() {
        let mut l = FakeLosos::default();
        cmd_put(&mut l, &draft("Notes", SOURCE)).unwrap();
        let add = parse_op(&json!({"op": "add", "collection": "notes", "value": {"text": "milk"}}))
            .unwrap();
        cmd_data_op(&mut l, "u1", &add).unwrap();
        let mut next = draft("Notes 2", SOURCE);
        next.id = Some("u1".into());
        let saved = cmd_put(&mut l, &next).unwrap();
        assert_eq!(saved["app"]["id"], "u1");
        assert_eq!(saved["app"]["name"], "Notes 2");
        let data = cmd_data(&mut l, "u1").unwrap().unwrap();
        assert_eq!(data["collections"]["notes"][0]["text"], "milk");
    }

    #[test]
    fn source_that_does_not_compile_is_refused_with_the_line() {
        let mut l = FakeLosos::default();
        let e = cmd_put(
            &mut l,
            &draft("Broken", "function App() { return <Page>; }"),
        )
        .unwrap_err();
        assert!(
            message(&e).starts_with("the app does not compile. Line 1"),
            "{}",
            message(&e)
        );
        assert!(cmd_list(&mut l).unwrap()["apps"]
            .as_array()
            .unwrap()
            .is_empty());
    }

    #[test]
    fn source_with_problems_is_saved_and_the_problems_returned() {
        let mut l = FakeLosos::default();
        let saved = cmd_put(
            &mut l,
            &draft(
                "Grid",
                "function App() { return <Page><DataGrid /></Page>; }",
            ),
        )
        .unwrap();
        assert!(saved["problems"][0].as_str().unwrap().contains("DataGrid"));
    }

    #[test]
    fn names_and_counts_are_bounded() {
        let mut l = FakeLosos::default();
        assert_eq!(
            message(&cmd_put(&mut l, &draft(" ", SOURCE)).unwrap_err()),
            "give the app a name"
        );
        for i in 0..MAX_APPS {
            cmd_put(&mut l, &draft(&format!("App {i}"), SOURCE)).unwrap();
        }
        let e = cmd_put(&mut l, &draft("One more", SOURCE)).unwrap_err();
        assert!(message(&e).contains("at most 24 apps"));
    }

    #[test]
    fn data_is_added_updated_replaced_removed_and_kept_per_app() {
        let mut l = FakeLosos::default();
        cmd_put(&mut l, &draft("A", SOURCE)).unwrap();
        cmd_put(&mut l, &draft("B", SOURCE)).unwrap();
        let op = |doc: Value| parse_op(&doc).unwrap();
        let added = cmd_data_op(&mut l, "u1", &op(json!({"op": "add", "collection": "notes", "value": {"text": "milk", "done": false}}))).unwrap().unwrap();
        let id = added["item"]["id"].as_str().unwrap().to_string();
        assert!(added["item"]["createdAt"].is_u64());
        let updated = cmd_data_op(&mut l, "u1", &op(json!({"op": "update", "collection": "notes", "id": id, "value": {"done": true, "id": "x"}}))).unwrap().unwrap();
        assert_eq!(updated["item"]["done"], true);
        assert_eq!(updated["item"]["id"], id.as_str(), "the id is the box's");
        cmd_data_op(
            &mut l,
            "u1",
            &op(json!({"op": "set", "key": "count", "value": 3})),
        )
        .unwrap();
        let replaced = cmd_data_op(&mut l, "u1", &op(json!({"op": "replace", "collection": "cards", "items": [{"id": "keep", "t": 1}, {"t": 2}]}))).unwrap().unwrap();
        assert_eq!(replaced["items"][0]["id"], "keep");
        assert!(replaced["items"][1]["id"]
            .as_str()
            .unwrap()
            .starts_with('i'));
        let data = cmd_data(&mut l, "u1").unwrap().unwrap();
        assert_eq!(data["stored"]["count"], 3);
        assert_eq!(data["collections"]["notes"][0]["done"], true);
        assert_eq!(
            cmd_data(&mut l, "u2").unwrap().unwrap()["collections"],
            json!({})
        );
        let removed = cmd_data_op(
            &mut l,
            "u1",
            &op(json!({"op": "remove", "collection": "notes", "id": id})),
        )
        .unwrap()
        .unwrap();
        assert_eq!(removed["removed"], true);
        assert_eq!(
            cmd_data(&mut l, "u1").unwrap().unwrap()["collections"]["notes"],
            json!([])
        );
    }

    #[test]
    fn data_of_an_app_the_box_does_not_have_is_none_and_deleting_drops_it() {
        let mut l = FakeLosos::default();
        let add = parse_op(&json!({"op": "add", "collection": "notes", "value": {}})).unwrap();
        assert!(cmd_data_op(&mut l, "u1", &add).unwrap().is_none());
        cmd_put(&mut l, &draft("A", SOURCE)).unwrap();
        cmd_data_op(&mut l, "u1", &add).unwrap();
        assert_eq!(cmd_delete(&mut l, "u1").unwrap()["deleted"], true);
        assert!(cmd_data(&mut l, "u1").unwrap().is_none());
        assert!(l.app_data.is_empty());
        assert_eq!(cmd_delete(&mut l, "u1").unwrap()["deleted"], false);
    }

    #[test]
    fn data_writes_are_bounded() {
        let mut l = FakeLosos::default();
        cmd_put(&mut l, &draft("A", SOURCE)).unwrap();
        let big = "x".repeat(MAX_ITEM_BYTES);
        let e = cmd_data_op(
            &mut l,
            "u1",
            &parse_op(&json!({"op": "add", "collection": "notes", "value": {"t": big}})).unwrap(),
        )
        .unwrap_err();
        assert!(message(&e).contains("at most 32 KiB"));
        let e = cmd_data_op(
            &mut l,
            "u1",
            &parse_op(&json!({"op": "add", "collection": "no tes", "value": {}})).unwrap(),
        )
        .unwrap_err();
        assert!(message(&e).contains("letters, digits"));
        let e = cmd_data_op(
            &mut l,
            "u1",
            &parse_op(&json!({"op": "update", "collection": "notes", "id": "i9", "value": {}}))
                .unwrap(),
        )
        .unwrap_err();
        assert!(message(&e).contains("no such entry"));
        let chunk = "y".repeat(30 * 1024);
        let mut last = Ok(None);
        for _ in 0..80 {
            last = cmd_data_op(
                &mut l,
                "u1",
                &parse_op(&json!({"op": "add", "collection": "notes", "value": {"t": chunk}}))
                    .unwrap(),
            );
            if last.is_err() {
                break;
            }
        }
        assert!(message(&last.unwrap_err()).contains("at most 2 MiB"));
    }

    #[test]
    fn an_operation_is_read_or_refused() {
        assert!(parse_op(&json!({"op": "drop"})).is_none());
        assert!(parse_op(&json!({"op": "add", "collection": "a", "value": 3})).is_none());
        assert!(parse_op(&json!({"op": "remove", "collection": "a"})).is_none());
        assert!(matches!(
            parse_op(&json!({"op": "set", "key": "k", "value": null})),
            Some(DataOp::Set { .. })
        ));
    }
}
