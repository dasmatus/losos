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
    /// Its repository on LosOS Git and how that stands (`crate::app_repo`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<crate::app_repo::RepoState>,
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

/// What a save or a pushed version must pass: a name, a size, and a source
/// that compiles. The sentence is what the owner reads.
fn validate(name: &str, source: &str) -> Result<crate::jsx::Checked, String> {
    if name.is_empty() {
        return Err("give the app a name".to_string());
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(format!(
            "the name is longer than {MAX_NAME_CHARS} characters"
        ));
    }
    if name.chars().any(char::is_control) {
        return Err("the name has a control character in it".to_string());
    }
    if source.len() > MAX_SOURCE_BYTES {
        return Err(format!(
            "the app is larger than {} KiB",
            MAX_SOURCE_BYTES / 1024
        ));
    }
    let checked = crate::jsx::check(source);
    if checked.code.is_none() {
        let first = checked.problems.first().cloned().unwrap_or_default();
        return Err(format!("the app does not compile. {first}"));
    }
    Ok(checked)
}

/// `POST /api/user-apps`: add an app, or save a new version of the one the
/// draft's id names. The source must compile; problems that would show only
/// when it runs are saved with it and returned, so the owner sees them.
pub fn cmd_put<L: Losos>(l: &mut L, draft: &Draft) -> anyhow::Result<Value> {
    let name = draft.name.trim();
    let checked = match validate(name, &draft.source) {
        Ok(c) => c,
        Err(sentence) => return invalid(sentence),
    };
    let code = checked.code.clone().unwrap_or_default();
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
                repo: None,
            };
            apps.next_id += 1;
            apps.apps.push(app.clone());
            app
        }
    };
    l.save_user_apps(&apps)?;
    // The version goes into the app's repository now; LosOS Git gets it on
    // the next sync, which the HTTP layer asks for at once. A commit that
    // fails costs the history one step, not the save: the next sync commits
    // whatever the app is then.
    if let Err(e) = commit_save(l, &app, &checked.problems) {
        tracing::warn!(app = %app.id, error = ?e, "could not commit the app's version");
    }
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
        // The copy beside the list; the repository on LosOS Git is the
        // owner's to delete (`crate::app_repo`).
        l.app_repo_remove(id)?;
    }
    Ok(json!({ "deleted": deleted }))
}

// ── Each app's repository ──────────────────────────────────────────────────

/// Commit a save to the app's repository: its first version gets the README.
fn commit_save<L: Losos>(l: &mut L, app: &UserApp, problems: &[String]) -> anyhow::Result<()> {
    let first = l.app_repo_head(&app.id)?.is_none();
    let (subject, body) = crate::app_repo::save_message(app, first, problems);
    let files = if first {
        crate::app_repo::first_files(app)
    } else {
        crate::app_repo::files(app)
    };
    l.app_repo_commit(&app.id, &files, &subject, &body)?;
    Ok(())
}

/// Keep every app's repository in step with LosOS Git (`crate::app_repo`):
/// commit what each app is, push what LosOS Git lacks, and take a version a
/// clone pushed once it passes the checks a save goes through. Run by the
/// reconciler thread every half minute and right after a save.
pub fn cmd_sync<L: Losos>(l: &mut L) -> anyhow::Result<Value> {
    let mut apps = l.load_user_apps()?;
    let remote = l.config_repo().filter(|r| r.via_server());
    let mut taken: Option<Vec<String>> = None;
    let mut changed = false;
    for app in &mut apps.apps {
        let before = app.clone();
        if let Err(e) = sync_one(l, app, remote.as_ref(), &mut taken) {
            let name = app.repo.as_ref().and_then(|r| r.name.clone());
            let detail = if let Some(not_up) = e.downcast_ref::<crate::config_repo::NotUp>() {
                format!("{}. The box tries again in half a minute.", not_up.0)
            } else {
                tracing::warn!(app = %app.id, error = ?e, "could not sync the app's repository");
                format!("The box could not sync it with LosOS Git: {e}")
            };
            app.repo = Some(crate::app_repo::RepoState::new(name, "local", detail));
        }
        changed |= *app != before;
    }
    if changed {
        l.save_user_apps(&apps)?;
    }
    Ok(json!({
        "apps": apps
            .apps
            .iter()
            .map(|a| json!({ "id": a.id, "repo": a.repo.as_ref().map(crate::app_repo::RepoState::view) }))
            .collect::<Vec<_>>(),
    }))
}

fn sync_one<L: Losos>(
    l: &mut L,
    app: &mut UserApp,
    remote: Option<&crate::config_repo::RepoConfig>,
    taken: &mut Option<Vec<String>>,
) -> anyhow::Result<()> {
    use crate::app_repo::{Plan, RepoState};

    // The box's own copy first: an app from before its repository, or one a
    // failed commit left behind, gets its version now.
    let first = l.app_repo_head(&app.id)?.is_none();
    let (subject, body) = crate::app_repo::sync_message(app, first);
    let files = if first {
        crate::app_repo::first_files(app)
    } else {
        crate::app_repo::files(app)
    };
    l.app_repo_commit(&app.id, &files, &subject, &body)?;

    let Some(remote) = remote else {
        app.repo = Some(RepoState::new(
            None,
            "local",
            "LosOS Git is off on this box, so the app's versions are kept on the box only.",
        ));
        return Ok(());
    };
    let name = match app.repo.as_ref().and_then(|r| r.name.clone()) {
        Some(name) => name,
        None => {
            let full = make_repo(l, app, remote, taken)?;
            app.repo = Some(RepoState::new(Some(full.clone()), "local", ""));
            full
        }
    };
    let url = format!("{}/{name}.git", remote.git_url.trim_end_matches('/'));
    let local = l
        .app_repo_head(&app.id)?
        .ok_or_else(|| anyhow::anyhow!("the app's repository has no commit after one was made"))?;
    let fetched = l.app_repo_fetch(&app.id, &url)?;
    let (remote_ahead, local_ahead) = match fetched.as_deref() {
        Some(r) if r != local => (
            l.app_repo_is_ancestor(&app.id, &local, r)?,
            l.app_repo_is_ancestor(&app.id, r, &local)?,
        ),
        _ => (false, false),
    };
    let was = app.repo.clone();
    let state = match crate::app_repo::plan(&local, fetched.as_deref(), remote_ahead, local_ahead) {
        // A refusal stays on the page until the next version lands.
        Plan::Nothing => match was {
            Some(r) if r.state == "refused" => r,
            _ => RepoState::new(Some(name), "synced", ""),
        },
        Plan::Push => {
            l.app_repo_push(&app.id, &url)?;
            RepoState::new(Some(name), "synced", "")
        }
        Plan::Diverged => RepoState::new(
            Some(name),
            "diverged",
            "The box and LosOS Git each have a version the other lacks. Pull, merge and push, and the box takes the merge.",
        ),
        Plan::Take(sha) => take(l, app, &sha, &url, name)?,
    };
    app.repo = Some(state);
    Ok(())
}

/// Pick the repository's name and make it on LosOS Git, private.
fn make_repo<L: Losos>(
    l: &mut L,
    app: &UserApp,
    remote: &crate::config_repo::RepoConfig,
    taken: &mut Option<Vec<String>>,
) -> anyhow::Result<String> {
    use crate::config_repo::{classify, Answer, GitOp, NotUp};

    if taken.is_none() {
        crate::losos::ensure_owner(l, remote)?;
        let (status, body) = l.git_request(&GitOp::ListRepos {
            login: remote.owner.clone(),
        })?;
        let names = match classify(status, &body) {
            Answer::Ok(Value::Array(repos)) => repos
                .iter()
                .filter_map(|r| r.get("name").and_then(Value::as_str))
                .map(str::to_string)
                .collect(),
            Answer::Ok(_) => Vec::new(),
            Answer::Missing => {
                return Err(NotUp("The owner's account on LosOS Git is not made yet".into()).into())
            }
            Answer::Exists | Answer::Refused { .. } => {
                anyhow::bail!("LosOS Git would not list the owner's repositories ({status})")
            }
        };
        *taken = Some(names);
    }
    let names = taken.as_mut().expect("filled above");
    let short = crate::app_repo::repo_name(&app.name, names);
    let (status, body) = l.git_request(&GitOp::EnsureAppRepo {
        owner: remote.owner.clone(),
        name: short.clone(),
    })?;
    match classify(status, &body) {
        Answer::Ok(_) | Answer::Exists => {}
        Answer::Missing => {
            return Err(NotUp("The owner's account on LosOS Git is not made yet".into()).into())
        }
        Answer::Refused { status, message } => {
            anyhow::bail!("LosOS Git would not make the repository ({status}): {message}")
        }
    }
    names.push(short.clone());
    Ok(format!("{}/{short}", remote.owner))
}

/// A clone pushed ahead: check its version and run it, or put the app back
/// in a commit of its own that says why, so the branch stays one line.
fn take<L: Losos>(
    l: &mut L,
    app: &mut UserApp,
    sha: &str,
    url: &str,
    name: String,
) -> anyhow::Result<crate::app_repo::RepoState> {
    use crate::app_repo::RepoState;

    let source = l.app_repo_show(&app.id, sha, crate::app_repo::SOURCE_FILE)?;
    let meta = l.app_repo_show(&app.id, sha, crate::app_repo::META_FILE)?;
    let verdict = crate::app_repo::read_pushed(app, source, meta).and_then(|pushed| {
        let trimmed = pushed.name.trim().to_string();
        validate(&trimmed, &pushed.source).map(|checked| (pushed, trimmed, checked))
    });
    l.app_repo_set_head(&app.id, sha)?;
    match verdict {
        Ok((pushed, trimmed, checked)) => {
            app.name = trimmed;
            app.summary = pushed
                .summary
                .as_deref()
                .and_then(crate::builder::clean_summary)
                .map(|s| s.chars().take(MAX_SUMMARY_CHARS).collect());
            app.prompt = pushed
                .prompt
                .map(|p| p.chars().take(crate::builder::MAX_PROMPT_CHARS).collect());
            app.source = pushed.source;
            app.code = checked.code.unwrap_or_default();
            app.updated_at = l.now();
            tracing::info!(app = %app.id, sha, "took a version pushed to the app's repository");
            // The box writes losos-app.json its own way; when that differs
            // from what was pushed, it says so in a commit and pushes it.
            let (subject, body) = crate::app_repo::sync_message(app, false);
            if l.app_repo_commit(&app.id, &crate::app_repo::files(app), &subject, &body)?
                .is_some()
            {
                l.app_repo_push(&app.id, url)?;
            }
            Ok(RepoState::new(Some(name), "synced", ""))
        }
        Err(why) => {
            let (subject, body) = crate::app_repo::refused_message(app, sha, &why);
            l.app_repo_commit(&app.id, &crate::app_repo::files(app), &subject, &body)?;
            l.app_repo_push(&app.id, url)?;
            let short: String = sha.chars().take(7).collect();
            Ok(RepoState::new(
                Some(name),
                "refused",
                format!("The version pushed as {short} was not taken: {why}"),
            ))
        }
    }
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

    /// A fake LosOS Git that has the owner, lists `existing` as theirs and
    /// makes any app repository asked for.
    fn with_git(existing: &[&str]) -> FakeLosos {
        let mut l = FakeLosos::default().with_config_repo();
        let repos: Vec<Value> = existing.iter().map(|n| json!({ "name": n })).collect();
        l.git_routes.insert(
            "GET /api/v1/users/notshared/repos".to_string(),
            (200, Value::Array(repos).to_string()),
        );
        for name in ["app-notes", "app-notes-2", "app-chores"] {
            l.git_routes.insert(
                format!("PUT /api/v1/admin/users/notshared/repos/{name}"),
                (201, format!(r#"{{"name": "{name}"}}"#)),
            );
        }
        l
    }

    const URL: &str = "http://127.0.0.1:3000/notshared/app-notes-2.git";

    fn head_files(l: &FakeLosos, id: &str) -> std::collections::BTreeMap<String, String> {
        let head = &l.app_heads[id];
        l.app_commits[head].files.clone()
    }

    #[test]
    fn every_save_is_a_commit_and_the_first_one_has_the_readme() {
        let mut l = FakeLosos::default();
        cmd_put(&mut l, &draft("Notes", SOURCE)).unwrap();
        let first = l.app_heads["u1"].clone();
        let c = &l.app_commits[&first];
        assert_eq!(c.subject, "Make Notes");
        assert!(c.body.contains("> a notes app"), "{}", c.body);
        assert_eq!(c.files["app.jsx"], SOURCE);
        assert!(c.files["README.md"].starts_with("# Notes"));
        let mut next = draft(
            "Notes",
            "function App() { return <Page title=\"Notes\" />; }",
        );
        next.id = Some("u1".into());
        cmd_put(&mut l, &next).unwrap();
        let second = &l.app_commits[&l.app_heads["u1"]];
        assert_eq!(second.subject, "Change Notes");
        assert_eq!(second.parent.as_deref(), Some(first.as_str()));
        assert!(second.files.contains_key("README.md"), "the README is kept");
        // Saving the same thing again makes no commit.
        cmd_put(&mut l, &next).unwrap();
        assert_eq!(l.app_commits.len(), 2);
        cmd_delete(&mut l, "u1").unwrap();
        assert!(!l.app_heads.contains_key("u1"));
    }

    #[test]
    fn with_git_off_the_versions_stay_on_the_box() {
        let mut l = FakeLosos::default();
        cmd_put(&mut l, &draft("Notes", SOURCE)).unwrap();
        let out = cmd_sync(&mut l).unwrap();
        assert_eq!(out["apps"][0]["repo"]["state"], "local");
        assert_eq!(out["apps"][0]["repo"]["url"], Value::Null);
        assert!(l.app_pushes.is_empty());
        assert!(l.git_calls.is_empty());
    }

    #[test]
    fn a_saved_app_gets_a_private_repository_with_a_free_name_and_is_pushed() {
        let mut l = with_git(&["losos-config", "app-notes"]);
        cmd_put(&mut l, &draft("Notes", SOURCE)).unwrap();
        let out = cmd_sync(&mut l).unwrap();
        assert_eq!(out["apps"][0]["repo"]["state"], "synced", "{out}");
        assert_eq!(out["apps"][0]["repo"]["name"], "notshared/app-notes-2");
        assert_eq!(out["apps"][0]["repo"]["url"], "/git/notshared/app-notes-2");
        assert!(l
            .git_calls
            .contains(&"PUT /api/v1/admin/users/notshared/repos/app-notes-2".to_string()));
        assert_eq!(l.app_remotes[URL], l.app_heads["u1"]);
        // Kept: the next sync neither lists nor makes anything, nor pushes.
        let calls = l.git_calls.len();
        cmd_sync(&mut l).unwrap();
        assert_eq!(l.git_calls.len(), calls);
        assert_eq!(l.app_pushes.len(), 1);
        assert_eq!(l.user_apps.apps[0].repo.as_ref().unwrap().state, "synced");
    }

    #[test]
    fn a_pushed_version_that_passes_the_checks_becomes_the_app() {
        let mut l = with_git(&["app-notes"]);
        cmd_put(&mut l, &draft("Notes", SOURCE)).unwrap();
        cmd_sync(&mut l).unwrap();
        let new_source = "function App() { return <Page title=\"Shopping\" />; }";
        let meta = r#"{"name": "Shopping", "summary": "What to buy."}"#;
        let pushed = l.push_from_clone(
            URL,
            &[
                ("app.jsx", new_source.to_string()),
                ("losos-app.json", meta.to_string()),
                ("notes.md", "mine".to_string()),
            ],
        );
        let out = cmd_sync(&mut l).unwrap();
        assert_eq!(out["apps"][0]["repo"]["state"], "synced", "{out}");
        let app = &l.user_apps.apps[0];
        assert_eq!(app.name, "Shopping");
        assert_eq!(app.summary.as_deref(), Some("What to buy."));
        assert_eq!(app.source, new_source);
        assert!(app.code.contains("Shopping"));
        // The box rewrote losos-app.json its own way, on top of the push,
        // and kept the owner's own file.
        let head = l.app_heads["u1"].clone();
        assert_eq!(
            l.app_commits[&head].parent.as_deref(),
            Some(pushed.as_str())
        );
        assert_eq!(
            l.app_commits[&head].subject,
            "Keep Shopping as the box has it"
        );
        assert_eq!(head_files(&l, "u1")["notes.md"], "mine");
        assert_eq!(l.app_remotes[URL], head);
        // The data is the app's still.
        assert!(cmd_data(&mut l, "u1").unwrap().is_some());
    }

    #[test]
    fn a_pushed_version_that_does_not_compile_is_answered_and_the_app_kept() {
        let mut l = with_git(&["app-notes"]);
        cmd_put(&mut l, &draft("Notes", SOURCE)).unwrap();
        cmd_sync(&mut l).unwrap();
        let pushed = l.push_from_clone(
            URL,
            &[("app.jsx", "function App() { return <p>; }".to_string())],
        );
        let out = cmd_sync(&mut l).unwrap();
        assert_eq!(out["apps"][0]["repo"]["state"], "refused", "{out}");
        let detail = out["apps"][0]["repo"]["detail"].as_str().unwrap();
        assert!(
            detail.contains(&pushed[..7]) && detail.contains("does not compile"),
            "{detail}"
        );
        assert_eq!(
            l.user_apps.apps[0].source, SOURCE,
            "the app is kept as it was"
        );
        let head = l.app_heads["u1"].clone();
        let answer = &l.app_commits[&head];
        assert_eq!(answer.subject, "Keep Notes as it was");
        assert_eq!(
            answer.parent.as_deref(),
            Some(pushed.as_str()),
            "one line of history"
        );
        assert_eq!(answer.files["app.jsx"], SOURCE);
        assert_eq!(l.app_remotes[URL], head, "the answer is pushed");
        // The refusal stays on the page until a new version lands.
        cmd_sync(&mut l).unwrap();
        assert_eq!(l.user_apps.apps[0].repo.as_ref().unwrap().state, "refused");
        let mut next = draft(
            "Notes",
            "function App() { return <Page title=\"Notes 2\" />; }",
        );
        next.id = Some("u1".into());
        cmd_put(&mut l, &next).unwrap();
        cmd_sync(&mut l).unwrap();
        assert_eq!(l.user_apps.apps[0].repo.as_ref().unwrap().state, "synced");
    }

    #[test]
    fn both_sides_moving_is_reported_and_nothing_touched() {
        let mut l = with_git(&["app-notes"]);
        cmd_put(&mut l, &draft("Notes", SOURCE)).unwrap();
        cmd_sync(&mut l).unwrap();
        let pushed = l.push_from_clone(URL, &[("notes.md", "x".to_string())]);
        let mut next = draft(
            "Notes",
            "function App() { return <Page title=\"Mine\" />; }",
        );
        next.id = Some("u1".into());
        cmd_put(&mut l, &next).unwrap();
        let out = cmd_sync(&mut l).unwrap();
        assert_eq!(out["apps"][0]["repo"]["state"], "diverged", "{out}");
        assert_eq!(l.app_remotes[URL], pushed);
        assert!(l.user_apps.apps[0].source.contains("Mine"));
    }

    #[test]
    fn git_not_up_leaves_the_app_local_and_tries_again() {
        let mut l = with_git(&["app-notes"]);
        cmd_put(&mut l, &draft("Notes", SOURCE)).unwrap();
        l.app_remote_down = true;
        let out = cmd_sync(&mut l).unwrap();
        assert_eq!(out["apps"][0]["repo"]["state"], "local");
        assert_eq!(
            out["apps"][0]["repo"]["name"], "notshared/app-notes-2",
            "the name is kept"
        );
        assert!(out["apps"][0]["repo"]["detail"]
            .as_str()
            .unwrap()
            .contains("tries again"));
        l.app_remote_down = false;
        let out = cmd_sync(&mut l).unwrap();
        assert_eq!(out["apps"][0]["repo"]["state"], "synced");
        assert_eq!(
            l.git_calls
                .iter()
                .filter(|c| c.starts_with("PUT /api/v1/admin/users/notshared/repos/app-"))
                .count(),
            1,
            "the repository is made once"
        );
    }

    #[test]
    fn an_app_from_before_its_repository_gets_its_first_commit_on_the_next_sync() {
        let mut l = FakeLosos::default();
        cmd_put(&mut l, &draft("Notes", SOURCE)).unwrap();
        l.app_heads.clear();
        cmd_sync(&mut l).unwrap();
        let head = &l.app_commits[&l.app_heads["u1"]];
        assert_eq!(head.subject, "Make Notes");
        assert!(head.files.contains_key("README.md"));
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
