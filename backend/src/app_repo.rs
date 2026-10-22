//! Each of the owner's apps as a private repository on LosOS Git.
//!
//! An app ([`crate::user_apps`]) is one JSX source. Every save commits it to
//! a repository of its own, kept by lososd beside the list of apps
//! (`user-apps/<id>.git`, bare), so the app has a history even on a box
//! with LosOS Git off. With LosOS Git on, lososd keeps that repository in
//! step with a private one under the owner's account, `app-<name>`: it pushes
//! each save, and a push from a clone that moves the branch ahead is taken
//! back into the app, after the checks a save goes through. So the owner can
//! clone an app, change it in their own editor, push, and the box runs the
//! new version. A push the checks refuse leaves the app as it was and says
//! why on the app's page.
//!
//! The repository is made private, as the configuration repository is,
//! because `/git/` goes through the master-proxy tunnel whenever the owner
//! turns that on. Making it private only happens when lososd creates it:
//! if the owner changes the visibility in LosOS Git later, that choice stays.
//!
//! Deleting an app deletes the copy beside the list, not the repository on
//! LosOS Git: that is the owner's history, and they delete it there.
//!
//! A commit writes `app.jsx` and `losos-app.json` and leaves every other
//! file as it is, so a README the owner rewrote or notes they added stay.
//!
//! This module is the pure half: the repository's name ([`repo_name`]), the
//! files a commit writes ([`files`]), the commit messages, how a pushed
//! version reads ([`read_pushed`]) and the sync decision ([`plan`]). The
//! effects are [`crate::losos::Losos`] methods, so
//! [`crate::user_apps::cmd_sync`] is tested against the fake with no git.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::user_apps::UserApp;

/// The branch every app repository keeps its versions on.
pub const BRANCH: &str = "main";
/// The app's source in the repository.
pub const SOURCE_FILE: &str = "app.jsx";
/// The app's name, summary and the request the builder was given.
pub const META_FILE: &str = "losos-app.json";
/// What the repository is and how to change the app from a clone.
pub const README_FILE: &str = "README.md";
/// Longest repository name lososd makes, before a `-2` for a name in use.
pub const MAX_NAME_CHARS: usize = 40;
/// What a fresh repository on LosOS Git is described as.
pub const REPO_DESCRIPTION: &str =
    "An app of yours on this LosOS box. Push a change to app.jsx and the box checks it and runs it.";

/// How an app's repository stands with LosOS Git.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoState {
    /// `owner/name` on LosOS Git, once lososd has picked the name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// `synced`, `local` (LosOS Git is off or not reached yet), `diverged`
    /// (both sides moved; nothing is touched) or `refused` (a push the checks
    /// turned down).
    pub state: String,
    /// The sentence the app's page shows for anything but `synced`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub detail: String,
}

impl RepoState {
    pub fn new(name: Option<String>, state: &str, detail: impl Into<String>) -> RepoState {
        RepoState {
            name,
            state: state.to_string(),
            detail: detail.into(),
        }
    }

    /// The repository's page on the box, relative to the front vhost.
    pub fn page_url(&self) -> Option<String> {
        self.name.as_ref().map(|n| format!("/git/{n}"))
    }

    /// What the app's JSON carries for the page.
    pub fn view(&self) -> Value {
        json!({
            "name": self.name,
            "url": self.page_url(),
            "state": self.state,
            "detail": self.detail,
        })
    }
}

/// `app-<name>`, lower case, with anything but letters and digits as one
/// dash, made unique against `taken` (the owner's repository names, lower
/// case) with `-2`, `-3`, … The prefix keeps an app from taking a name the
/// owner might give a project of their own.
pub fn repo_name(app_name: &str, taken: &[String]) -> String {
    let mut slug = String::new();
    for c in app_name.chars().flat_map(char::to_lowercase).map(fold) {
        if c.is_ascii_alphanumeric() {
            slug.push(c);
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
        if slug.len() >= MAX_NAME_CHARS {
            break;
        }
    }
    let slug = slug.trim_end_matches('-');
    let base = if slug.is_empty() {
        "app".to_string()
    } else {
        format!("app-{slug}")
    };
    let free = |n: &str| !taken.iter().any(|t| t.eq_ignore_ascii_case(n));
    if free(&base) {
        return base;
    }
    (2..)
        .map(|i| format!("{base}-{i}"))
        .find(|n| free(n))
        .expect("an unbounded range finds a free name")
}

/// The plain letter under the accented ones Slovak, Czech and German
/// write, so "Účty" is `app-ucty` rather than `app-ty`.
fn fold(c: char) -> char {
    match c {
        'á' | 'ä' | 'à' | 'â' => 'a',
        'č' | 'ç' => 'c',
        'ď' => 'd',
        'é' | 'ě' | 'è' | 'ê' | 'ë' => 'e',
        'í' | 'î' | 'ï' => 'i',
        'ĺ' | 'ľ' => 'l',
        'ň' | 'ñ' => 'n',
        'ó' | 'ô' | 'ö' | 'ò' => 'o',
        'ŕ' | 'ř' => 'r',
        'š' | 'ß' => 's',
        'ť' => 't',
        'ú' | 'ů' | 'ü' | 'ù' | 'û' => 'u',
        'ý' | 'ÿ' => 'y',
        'ž' => 'z',
        other => other,
    }
}

/// What the app's metadata file holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Meta {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
}

/// The files one version of an app writes. Any other file in the
/// repository (a README the owner rewrote, notes, tests) is left as the
/// head has it; the first version also gets a README ([`first_files`]).
pub fn files(app: &UserApp) -> Vec<(&'static str, String)> {
    let meta = Meta {
        name: app.name.clone(),
        summary: app.summary.clone(),
        prompt: app.prompt.clone(),
    };
    let mut meta_text = serde_json::to_string_pretty(&meta).expect("plain strings serialize");
    meta_text.push('\n');
    vec![(SOURCE_FILE, app.source.clone()), (META_FILE, meta_text)]
}

/// [`files`] and the README, for an app's first commit.
pub fn first_files(app: &UserApp) -> Vec<(&'static str, String)> {
    let mut all = vec![(README_FILE, readme(app))];
    all.extend(files(app));
    all
}

fn readme(app: &UserApp) -> String {
    let mut text = format!("# {}\n\n", app.name);
    if let Some(summary) = &app.summary {
        text.push_str(summary);
        text.push_str("\n\n");
    }
    text.push_str(
        "An app on a LosOS box, kept here by the box. `app.jsx` is the app: one React \
         component, with the LosOS components, `useCollection` and `useStored` in scope. \
         `losos-app.json` holds its name.\n\n\
         Push a change to `main` and the box checks it, compiles it and runs it within a \
         minute. If the checks turn it down, the app stays as it was and its page on the \
         box says why. The app's data is not in this repository; it stays on the box.\n",
    );
    text
}

/// The commit message of a save: a subject and a body.
pub fn save_message(app: &UserApp, first: bool, problems: &[String]) -> (String, String) {
    let subject = if first {
        format!("Make {}", app.name)
    } else {
        format!("Change {}", app.name)
    };
    let mut body = String::new();
    if first {
        if let Some(prompt) = &app.prompt {
            body.push_str("Written by Build with AI, asked for:\n\n");
            for line in prompt.lines() {
                body.push_str("> ");
                body.push_str(line);
                body.push('\n');
            }
        }
    }
    if !problems.is_empty() {
        if !body.is_empty() {
            body.push('\n');
        }
        body.push_str("Saved with what the checks found:\n\n");
        for p in problems {
            body.push_str("- ");
            body.push_str(p);
            body.push('\n');
        }
    }
    (subject, body.trim_end().to_string())
}

/// The message of a commit a sync makes: an app from before its
/// repository's first commit, or files the box writes differently from how
/// a clone pushed them.
pub fn sync_message(app: &UserApp, first: bool) -> (String, String) {
    if first {
        return save_message(app, true, &[]);
    }
    (
        format!("Keep {} as the box has it", app.name),
        String::new(),
    )
}

/// The message of the commit that puts an app back after a push the
/// checks turned down, so the history says why.
pub fn refused_message(app: &UserApp, pushed: &str, why: &str) -> (String, String) {
    let short: String = pushed.chars().take(7).collect();
    (
        format!("Keep {} as it was", app.name),
        format!("The box did not take {short}: {why}"),
    )
}

/// A version someone pushed, read back from its files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pushed {
    pub name: String,
    pub summary: Option<String>,
    pub prompt: Option<String>,
    pub source: String,
}

/// Read a pushed version: `app.jsx` must be there; `losos-app.json`, when it
/// is there, must be the object [`files`] writes, and its name wins over the
/// app's. The checks a save makes come after, in
/// [`crate::user_apps::cmd_sync`].
pub fn read_pushed(
    current: &UserApp,
    source: Option<String>,
    meta: Option<String>,
) -> Result<Pushed, String> {
    let Some(source) = source else {
        return Err(format!("the pushed version has no {SOURCE_FILE}"));
    };
    let (name, summary, prompt) = match meta {
        None => (
            current.name.clone(),
            current.summary.clone(),
            current.prompt.clone(),
        ),
        Some(text) => match serde_json::from_str::<Meta>(&text) {
            Ok(m) => (m.name, m.summary, m.prompt),
            Err(e) => return Err(format!("{META_FILE} is not what the box writes ({e})")),
        },
    };
    Ok(Pushed {
        name,
        summary,
        prompt,
        source,
    })
}

/// What one sync of an app's repository does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    /// LosOS Git has this version, or a newer one the box already took.
    Nothing,
    /// LosOS Git is behind the box, or has nothing yet: push.
    Push,
    /// A clone pushed ahead of the box: check the new version and take it.
    Take(String),
    /// Both moved since they last agreed. Reported, never touched.
    Diverged,
}

/// Decide from the two heads and whether one contains the other.
/// `remote_is_ahead`: the local head is an ancestor of the remote one;
/// `local_is_ahead`: the other way round.
pub fn plan(
    local: &str,
    remote: Option<&str>,
    remote_is_ahead: bool,
    local_is_ahead: bool,
) -> Plan {
    match remote {
        None => Plan::Push,
        Some(r) if r == local => Plan::Nothing,
        Some(r) if remote_is_ahead => Plan::Take(r.to_string()),
        Some(_) if local_is_ahead => Plan::Push,
        Some(_) => Plan::Diverged,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> UserApp {
        UserApp {
            id: "u1".into(),
            name: "Chores".into(),
            summary: Some("Who does what this week.".into()),
            prompt: Some("a chore list\nfor the kids".into()),
            source: "function App() { return <p>hi</p>; }".into(),
            code: String::new(),
            created_at: 1,
            updated_at: 1,
            repo: None,
        }
    }

    #[test]
    fn names_are_prefixed_slugs_made_unique() {
        assert_eq!(repo_name("Chores", &[]), "app-chores");
        assert_eq!(
            repo_name("  Kids' chores!! 2026 ", &[]),
            "app-kids-chores-2026"
        );
        assert_eq!(repo_name("Účty a dane", &[]), "app-ucty-a-dane");
        assert_eq!(repo_name("Größe", &[]), "app-grose");
        assert_eq!(repo_name("😀", &[]), "app");
        let taken = vec!["app-chores".to_string(), "APP-CHORES-2".to_string()];
        assert_eq!(repo_name("Chores", &taken), "app-chores-3");
        let long = "a".repeat(100);
        assert_eq!(repo_name(&long, &[]).len(), "app-".len() + MAX_NAME_CHARS);
    }

    #[test]
    fn a_version_is_its_source_and_name_and_reads_back() {
        let a = app();
        let f = first_files(&a);
        let names: Vec<_> = f.iter().map(|(n, _)| *n).collect();
        assert_eq!(names, vec![README_FILE, SOURCE_FILE, META_FILE]);
        assert!(f[0].1.starts_with("# Chores\n"));
        assert_eq!(
            files(&a).len(),
            2,
            "only the first version writes the README"
        );
        assert_eq!(f[1].1, a.source, "the source goes in as the app has it");
        let back = read_pushed(&a, Some(f[1].1.clone()), Some(f[2].1.clone())).unwrap();
        assert_eq!(back.name, "Chores");
        assert_eq!(back.summary.as_deref(), Some("Who does what this week."));
        assert_eq!(back.prompt.as_deref(), Some("a chore list\nfor the kids"));
        assert_eq!(back.source, a.source);
    }

    #[test]
    fn a_pushed_version_needs_its_source_and_a_readable_meta() {
        let a = app();
        assert!(read_pushed(&a, None, None).unwrap_err().contains("app.jsx"));
        assert!(read_pushed(&a, Some("x".into()), Some("{".into()))
            .unwrap_err()
            .contains("losos-app.json"));
        let kept = read_pushed(&a, Some("x".into()), None).unwrap();
        assert_eq!(kept.name, "Chores");
        let renamed = read_pushed(&a, Some("x".into()), Some(r#"{"name":"Jobs"}"#.into())).unwrap();
        assert_eq!(renamed.name, "Jobs");
        assert_eq!(renamed.summary, None);
    }

    #[test]
    fn messages_name_the_app_and_quote_the_request_once() {
        let a = app();
        let (s, b) = save_message(&a, true, &[]);
        assert_eq!(s, "Make Chores");
        assert!(b.contains("> a chore list\n> for the kids"));
        let (s, b) = save_message(&a, false, &["`x` is never made".to_string()]);
        assert_eq!(s, "Change Chores");
        assert!(!b.contains("asked for"));
        assert!(b.contains("- `x` is never made"));
        let (s, b) = refused_message(&a, "0123456789abcdef", "the app does not compile.");
        assert_eq!(s, "Keep Chores as it was");
        assert_eq!(b, "The box did not take 0123456: the app does not compile.");
        assert_eq!(sync_message(&a, false).0, "Keep Chores as the box has it");
    }

    #[test]
    fn the_plan_follows_the_heads() {
        assert_eq!(plan("a", None, false, false), Plan::Push);
        assert_eq!(plan("a", Some("a"), false, false), Plan::Nothing);
        assert_eq!(plan("a", Some("b"), true, false), Plan::Take("b".into()));
        assert_eq!(plan("b", Some("a"), false, true), Plan::Push);
        assert_eq!(plan("b", Some("c"), false, false), Plan::Diverged);
    }

    #[test]
    fn the_state_links_the_page() {
        let s = RepoState::new(Some("notshared/app-chores".into()), "synced", "");
        assert_eq!(s.view()["url"], "/git/notshared/app-chores");
        assert_eq!(
            RepoState::new(None, "local", "x").view()["url"],
            Value::Null
        );
    }
}
