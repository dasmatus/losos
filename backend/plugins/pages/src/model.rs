//! What the pages plugin keeps and answers, against a [`Store`], so the
//! rules are tested without a repository.
//!
//! The pages' repository (`/var/lib/losos/pages`, a Git repository of its
//! own, apart from `/etc/nixos`'s) holds, for each page an administrator
//! changed:
//!
//! - `<page>.mdx`, their page;
//! - `defaults/<page>.mdx`, LosOS's default as it was when they saved, so
//!   the editor can say when the default has moved on since;
//!
//! and `style.css`, their stylesheet, and `pages.json`, who saved each and
//! when. A page with no file draws LosOS's default. Every save and reset is
//! a commit by the administrator who made it.

use losos_mdx::page::{page_id_ok, render_page, Registry};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// The largest page an administrator may save.
pub const MAX_PAGE: usize = 256 * 1024;
/// The largest stylesheet.
pub const MAX_STYLE: usize = 256 * 1024;
/// How many commits a page's history lists.
pub const HISTORY: usize = 20;

pub const META_FILE: &str = "pages.json";
pub const STYLE_FILE: &str = "style.css";

/// A request the rules turn down, with the sentence the editor shows.
#[derive(Debug)]
pub struct Invalid(pub String);

impl std::fmt::Display for Invalid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Invalid {}

/// No such page in `blocks.json`.
#[derive(Debug)]
pub struct NoSuchPage(pub String);

impl std::fmt::Display for NoSuchPage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "there is no page {}", self.0)
    }
}

impl std::error::Error for NoSuchPage {}

/// Who saved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Author {
    /// The sign-in name, or `spare-key`.
    pub login: String,
    pub name: String,
}

/// One commit of the repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Commit {
    pub commit: String,
    pub at: String,
    pub by: String,
    pub message: String,
}

/// A file to write into the repository, or to take out of it.
pub type Write = (String, Option<String>);

/// The repository and LosOS's own files, as the commands need them.
pub trait Store {
    /// `blocks.json`, as the build wrote it.
    fn registry(&mut self) -> anyhow::Result<String>;
    /// LosOS's default for a page, as the build wrote it.
    fn default_source(&mut self, id: &str) -> anyhow::Result<Option<String>>;
    /// A file of the repository.
    fn read(&mut self, file: &str) -> anyhow::Result<Option<String>>;
    /// Writes and removes files and commits them, with `message`.
    fn commit(&mut self, writes: &[Write], author: &Author, message: &str) -> anyhow::Result<()>;
    /// The last commits that touched `file`, newest first.
    fn history(&mut self, file: &str, max: usize) -> anyhow::Result<Vec<Commit>>;
    /// Seconds since the epoch.
    fn now(&mut self) -> u64;
}

/// Who saved a page, and when.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Saved {
    pub at: String,
    pub by: String,
}

/// `pages.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Meta {
    #[serde(default)]
    pub pages: BTreeMap<String, Saved>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<Saved>,
    /// Bumped on every save of the stylesheet, for its URL.
    #[serde(default)]
    pub style_version: u64,
}

fn page_file(id: &str) -> String {
    format!("{id}.mdx")
}

fn base_file(id: &str) -> String {
    format!("defaults/{id}.mdx")
}

fn meta(store: &mut dyn Store) -> Meta {
    store
        .read(META_FILE)
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn registry(store: &mut dyn Store) -> anyhow::Result<Registry> {
    let text = store.registry()?;
    Registry::from_json(&text).map_err(|e| anyhow::anyhow!("blocks.json: {e}"))
}

/// The id a route names, if `blocks.json` lists it.
fn known(registry: &Registry, id: &str) -> Result<(), NoSuchPage> {
    if page_id_ok(id) && registry.page(id).is_some() {
        Ok(())
    } else {
        Err(NoSuchPage(id.to_string()))
    }
}

/// `GET /api/pages`: every changed page as its HTML, for every signed-in
/// person. A page whose file no longer reads (a block LosOS dropped since)
/// is drawn with what still reads; a page `blocks.json` no longer lists is
/// left out.
pub fn cmd_list(store: &mut dyn Store) -> anyhow::Result<Value> {
    let registry = registry(store)?;
    let meta = meta(store);
    let mut pages = serde_json::Map::new();
    for (id, saved) in &meta.pages {
        let Some(spec) = registry.page(id) else {
            continue;
        };
        let Some(source) = store.read(&page_file(id))? else {
            continue;
        };
        // A file that read when it was saved and no longer does (a build
        // since moved the syntax) leaves the default drawn until an
        // administrator saves it again.
        if let Ok(rendered) = render_page(&source, spec) {
            pages.insert(
                id.clone(),
                json!({ "html": rendered.html, "at": saved.at, "by": saved.by }),
            );
        }
    }
    let style = meta
        .style
        .as_ref()
        .map(|_| Value::String(meta.style_version.to_string()))
        .unwrap_or(Value::Null);
    Ok(json!({ "pages": pages, "style": style }))
}

/// `GET /api/pages/{page}`: what the editor needs, for administrators.
pub fn cmd_get(store: &mut dyn Store, id: &str) -> anyhow::Result<Value> {
    let registry = registry(store)?;
    known(&registry, id)?;
    let default = store.default_source(id)?.unwrap_or_default();
    let custom = store.read(&page_file(id))?;
    let base = store.read(&base_file(id))?;
    let default_changed = custom.is_some() && base.as_deref() != Some(default.as_str());
    let history = store.history(&page_file(id), HISTORY)?;
    Ok(json!({
        "id": id,
        "source": custom.clone().unwrap_or_else(|| default.clone()),
        "default": default,
        "custom": custom.is_some(),
        "defaultChanged": default_changed,
        "history": history,
    }))
}

/// `PUT /api/pages/{page}`: saves a page that reads with no problem.
pub fn cmd_put(
    store: &mut dyn Store,
    id: &str,
    source: &str,
    author: &Author,
) -> anyhow::Result<Value> {
    let registry = registry(store)?;
    known(&registry, id)?;
    if source.len() > MAX_PAGE {
        return Err(Invalid(format!("the page is over {} KiB", MAX_PAGE / 1024)).into());
    }
    let spec = registry.page(id).expect("checked above");
    let rendered = render_page(source, spec).map_err(|e| Invalid(e.to_string()))?;
    if let Some(p) = rendered.problems.first() {
        return Err(Invalid(format!(
            "line {}, column {}: {}",
            p.place.line, p.place.column, p.message
        ))
        .into());
    }
    let default = store.default_source(id)?.unwrap_or_default();
    let mut meta = meta(store);
    let first = !meta.pages.contains_key(id);
    meta.pages.insert(
        id.to_string(),
        Saved {
            at: rfc3339(store.now()),
            by: author.login.clone(),
        },
    );
    let title = &spec.title;
    let message = if first {
        format!("Change the {title} page")
    } else {
        format!("Edit the {title} page")
    };
    store.commit(
        &[
            (page_file(id), Some(normalize(source))),
            (base_file(id), Some(default)),
            (META_FILE.to_string(), Some(meta_text(&meta))),
        ],
        author,
        &message,
    )?;
    cmd_list(store)
}

/// `DELETE /api/pages/{page}`: LosOS's default again. The page's history
/// stays in the repository.
pub fn cmd_reset(store: &mut dyn Store, id: &str, author: &Author) -> anyhow::Result<Value> {
    let registry = registry(store)?;
    known(&registry, id)?;
    let mut meta = meta(store);
    if meta.pages.remove(id).is_some() || store.read(&page_file(id))?.is_some() {
        let title = &registry.page(id).expect("checked above").title;
        store.commit(
            &[
                (page_file(id), None),
                (base_file(id), None),
                (META_FILE.to_string(), Some(meta_text(&meta))),
            ],
            author,
            &format!("Put the {title} page back to LosOS's default"),
        )?;
    }
    cmd_list(store)
}

/// `GET /api/pages/style.css`: the administrators' stylesheet, if any.
pub fn cmd_style(store: &mut dyn Store) -> anyhow::Result<Option<String>> {
    if meta(store).style.is_none() {
        return Ok(None);
    }
    store.read(STYLE_FILE)
}

/// `PUT /api/pages/style.css`.
pub fn cmd_put_style(store: &mut dyn Store, css: &str, author: &Author) -> anyhow::Result<Value> {
    if css.len() > MAX_STYLE {
        return Err(Invalid(format!("the stylesheet is over {} KiB", MAX_STYLE / 1024)).into());
    }
    if css.contains('\0') {
        return Err(Invalid("the stylesheet holds a NUL character".into()).into());
    }
    if css.trim().is_empty() {
        return cmd_reset_style(store, author);
    }
    let mut meta = meta(store);
    let first = meta.style.is_none();
    meta.style = Some(Saved {
        at: rfc3339(store.now()),
        by: author.login.clone(),
    });
    meta.style_version += 1;
    store.commit(
        &[
            (STYLE_FILE.to_string(), Some(normalize(css))),
            (META_FILE.to_string(), Some(meta_text(&meta))),
        ],
        author,
        if first {
            "Add a stylesheet for the pages"
        } else {
            "Edit the pages' stylesheet"
        },
    )?;
    cmd_list(store)
}

/// `DELETE /api/pages/style.css`.
pub fn cmd_reset_style(store: &mut dyn Store, author: &Author) -> anyhow::Result<Value> {
    let mut meta = meta(store);
    if meta.style.take().is_some() {
        meta.style_version += 1;
        store.commit(
            &[
                (STYLE_FILE.to_string(), None),
                (META_FILE.to_string(), Some(meta_text(&meta))),
            ],
            author,
            "Remove the pages' stylesheet",
        )?;
    }
    cmd_list(store)
}

/// Text as the repository keeps it: Unix line ends and a last one.
fn normalize(text: &str) -> String {
    let mut out = text.replace("\r\n", "\n");
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn meta_text(meta: &Meta) -> String {
    let mut s = serde_json::to_string_pretty(meta).unwrap_or_else(|_| "{}".into());
    s.push('\n');
    s
}

/// What the repository says it is, written when it is made.
pub const README: &str = "# The pages of this LosOS box\n\n\
Each page of the admin UI and of the apps is an MDX file that places the page's blocks. \
LosOS ships a default for every page; a file here replaces it.\n\n\
- `<page>.mdx` is a page an administrator changed.\n\
- `defaults/<page>.mdx` is LosOS's default as it was when they saved, so a later change to the default shows.\n\
- `style.css` is their stylesheet, loaded after LosOS's own on every page. Each page names itself on the root element, `html[data-page=\"overview\"]`.\n\
- `pages.json` says who saved each and when.\n\n\
Change them in Settings, Look, Pages. Every save and reset is a commit here.\n";

/// Seconds since the epoch as RFC 3339, in UTC.
#[must_use]
pub fn rfc3339(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rest = secs % 86_400;
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const REGISTRY: &str = r#"{"pages":{
        "overview":{"title":"Overview","blocks":{"BoxName":{},"Apps":{"props":{"hideStorage":{"type":"flag"}}}}},
        "settings.storage":{"title":"Storage","blocks":{"Disk":{}}}
    }}"#;

    #[derive(Default)]
    struct Fake {
        files: BTreeMap<String, String>,
        defaults: BTreeMap<String, String>,
        commits: Vec<(String, String, Vec<String>)>,
        clock: u64,
    }

    impl Fake {
        fn new() -> Fake {
            let mut f = Fake {
                clock: 1_800_000_000,
                ..Fake::default()
            };
            f.defaults
                .insert("overview".into(), "<BoxName />\n\n<Apps />\n".into());
            f.defaults
                .insert("settings.storage".into(), "<Disk />\n".into());
            f
        }
    }

    impl Store for Fake {
        fn registry(&mut self) -> anyhow::Result<String> {
            Ok(REGISTRY.into())
        }
        fn default_source(&mut self, id: &str) -> anyhow::Result<Option<String>> {
            Ok(self.defaults.get(id).cloned())
        }
        fn read(&mut self, file: &str) -> anyhow::Result<Option<String>> {
            Ok(self.files.get(file).cloned())
        }
        fn commit(
            &mut self,
            writes: &[Write],
            author: &Author,
            message: &str,
        ) -> anyhow::Result<()> {
            let mut touched = Vec::new();
            for (file, content) in writes {
                match content {
                    Some(c) => {
                        self.files.insert(file.clone(), c.clone());
                    }
                    None => {
                        self.files.remove(file);
                    }
                }
                touched.push(file.clone());
            }
            self.commits
                .push((author.login.clone(), message.into(), touched));
            Ok(())
        }
        fn history(&mut self, file: &str, max: usize) -> anyhow::Result<Vec<Commit>> {
            Ok(self
                .commits
                .iter()
                .rev()
                .filter(|(_, _, t)| t.iter().any(|f| f == file))
                .take(max)
                .enumerate()
                .map(|(n, (by, message, _))| Commit {
                    commit: format!("c{n}"),
                    at: String::new(),
                    by: by.clone(),
                    message: message.clone(),
                })
                .collect())
        }
        fn now(&mut self) -> u64 {
            self.clock
        }
    }

    fn admin() -> Author {
        Author {
            login: "matus".into(),
            name: "Matus".into(),
        }
    }

    #[test]
    fn nothing_changed_is_an_empty_list() {
        let mut s = Fake::new();
        assert_eq!(
            cmd_list(&mut s).unwrap(),
            json!({ "pages": {}, "style": null })
        );
    }

    #[test]
    fn a_saved_page_is_listed_as_its_html_and_committed_with_its_default() {
        let mut s = Fake::new();
        let list = cmd_put(
            &mut s,
            "overview",
            "# Hi\r\n\r\n<Apps hideStorage />",
            &admin(),
        )
        .unwrap();
        let html = list["pages"]["overview"]["html"].as_str().unwrap();
        assert!(html.contains("<h1 id=\"hi\">Hi</h1>"), "{html}");
        assert!(html.contains("data-block=\"Apps\""), "{html}");
        assert_eq!(list["pages"]["overview"]["by"], "matus");
        assert_eq!(list["pages"]["overview"]["at"], "2027-01-15T08:00:00Z");
        assert_eq!(s.files["overview.mdx"], "# Hi\n\n<Apps hideStorage />\n");
        assert_eq!(
            s.files["defaults/overview.mdx"],
            "<BoxName />\n\n<Apps />\n"
        );
        assert_eq!(s.commits.len(), 1);
        assert_eq!(s.commits[0].1, "Change the Overview page");
        cmd_put(&mut s, "overview", "<Apps />", &admin()).unwrap();
        assert_eq!(s.commits[1].1, "Edit the Overview page");
    }

    #[test]
    fn a_page_with_a_problem_is_refused_and_nothing_is_written() {
        let mut s = Fake::new();
        let e = cmd_put(&mut s, "overview", "<Apps />\n\n<Script />\n", &admin()).unwrap_err();
        assert!(e.downcast_ref::<Invalid>().is_some());
        assert!(e.to_string().contains("line 3, column 1"), "{e}");
        let e = cmd_put(&mut s, "overview", "{alert(1)}", &admin()).unwrap_err();
        assert!(e.to_string().contains("expressions"), "{e}");
        let e = cmd_put(&mut s, "overview", "<Apps>\n\ntext\n", &admin()).unwrap_err();
        assert!(e.downcast_ref::<Invalid>().is_some(), "{e}");
        assert!(s.commits.is_empty());
    }

    #[test]
    fn only_pages_blocks_json_lists_are_taken() {
        let mut s = Fake::new();
        for id in ["mail", "../etc/passwd", "Overview", "a/b", ""] {
            let e = cmd_put(&mut s, id, "x", &admin()).unwrap_err();
            assert!(e.downcast_ref::<NoSuchPage>().is_some(), "{id}: {e}");
            assert!(cmd_get(&mut s, id)
                .unwrap_err()
                .downcast_ref::<NoSuchPage>()
                .is_some());
        }
        assert!(s.commits.is_empty());
    }

    #[test]
    fn the_editor_sees_the_default_until_a_save_and_when_it_moves_on() {
        let mut s = Fake::new();
        let got = cmd_get(&mut s, "overview").unwrap();
        assert_eq!(got["custom"], false);
        assert_eq!(got["source"], got["default"]);
        assert_eq!(got["defaultChanged"], false);
        cmd_put(&mut s, "overview", "<Apps />\n", &admin()).unwrap();
        let got = cmd_get(&mut s, "overview").unwrap();
        assert_eq!(got["custom"], true);
        assert_eq!(got["source"], "<Apps />\n");
        assert_eq!(got["defaultChanged"], false);
        assert_eq!(got["history"][0]["message"], "Change the Overview page");
        s.defaults.insert("overview".into(), "<BoxName />\n".into());
        assert_eq!(cmd_get(&mut s, "overview").unwrap()["defaultChanged"], true);
    }

    #[test]
    fn a_reset_removes_the_page_and_keeps_its_history() {
        let mut s = Fake::new();
        cmd_put(&mut s, "settings.storage", "Hello\n\n<Disk />\n", &admin()).unwrap();
        let list = cmd_reset(&mut s, "settings.storage", &admin()).unwrap();
        assert_eq!(list["pages"], json!({}));
        assert!(!s.files.contains_key("settings.storage.mdx"));
        assert!(!s.files.contains_key("defaults/settings.storage.mdx"));
        assert_eq!(
            s.commits[1].1,
            "Put the Storage page back to LosOS's default"
        );
        // A second reset has nothing to commit.
        cmd_reset(&mut s, "settings.storage", &admin()).unwrap();
        assert_eq!(s.commits.len(), 2);
        assert_eq!(
            cmd_get(&mut s, "settings.storage").unwrap()["history"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn the_stylesheet_is_saved_versioned_and_removed() {
        let mut s = Fake::new();
        assert_eq!(cmd_style(&mut s).unwrap(), None);
        let list = cmd_put_style(&mut s, ":root { --accent: red }", &admin()).unwrap();
        assert_eq!(list["style"], "1");
        assert_eq!(
            cmd_style(&mut s).unwrap().as_deref(),
            Some(":root { --accent: red }\n")
        );
        let list = cmd_put_style(&mut s, "body { margin: 0 }", &admin()).unwrap();
        assert_eq!(list["style"], "2");
        let list = cmd_reset_style(&mut s, &admin()).unwrap();
        assert_eq!(list["style"], Value::Null);
        assert_eq!(cmd_style(&mut s).unwrap(), None);
        // An empty stylesheet is none.
        cmd_put_style(&mut s, "a{}", &admin()).unwrap();
        assert_eq!(
            cmd_put_style(&mut s, "  \n", &admin()).unwrap()["style"],
            Value::Null
        );
        assert!(cmd_put_style(&mut s, "a\0", &admin()).is_err());
        assert!(cmd_put_style(&mut s, &"a".repeat(MAX_STYLE + 1), &admin()).is_err());
    }

    #[test]
    fn a_page_blocks_json_dropped_is_left_out_of_the_list() {
        let mut s = Fake::new();
        cmd_put(&mut s, "overview", "<Apps />\n", &admin()).unwrap();
        let mut meta = meta(&mut s);
        meta.pages.insert("gone".into(), Saved::default());
        s.files.insert(META_FILE.into(), meta_text(&meta));
        s.files.insert("gone.mdx".into(), "x".into());
        let list = cmd_list(&mut s).unwrap();
        assert_eq!(list["pages"].as_object().unwrap().len(), 1);
    }
}
