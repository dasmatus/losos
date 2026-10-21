//! LosOS Cloud's own apps, managed from the admin UI's Apps pane.
//!
//! The owner sees every app LosOS Cloud (Nextcloud) has, turns each one on or
//! off, adds apps from Nextcloud's app store and removes the ones added that
//! way. The work is Nextcloud's own: every change is an `occ app:*` run by
//! `losos-nextcloud-apps` (modules/nextcloud-stack.nix, `appsTool`), which is
//! in the pod in container mode and on lososd's unit path natively. This
//! module is the rules around it, pure, plus the three commands the HTTP layer
//! calls:
//!
//!   * `GET /api/cloud/apps` lists the apps ([`cmd_cloud_apps`]);
//!   * `POST /api/cloud/apps/{install,enable,disable,remove}` `{id}` changes
//!     one and answers with the new list ([`cmd_cloud_app`]);
//!   * `GET /api/cloud/apps/search?q=` searches the app store
//!     ([`cmd_cloud_store_search`]). The store is fetched by lososd for the
//!     same reason the chart catalogue is (`crate::catalogue`): the page runs
//!     under `connect-src 'self'`.
//!
//! An app turned off here stays off. Both modes switch every bundled app on
//! at each start, so the tool keeps the owner's off list in Nextcloud's state
//! root (`offFile` in nextcloud-stack.nix) and start-up honours it.
//!
//! A few apps are LosOS's, not the owner's: sign-in with LosOS Login, the
//! sharing switch, the LosOS Cloud look, and the welcome wizard LosOS turns
//! off. They are listed with the reason and cannot be changed here
//! ([`MANAGED`]), and neither can the ones Nextcloud itself will not run
//! without (`alwaysEnabled` in core/shipped.json).

use serde::{Deserialize, Serialize};

use crate::apps::{Conflict, Invalid};
use crate::losos::Losos;
use crate::setup::{NcMode, OccOutcome, Target};

/// The tool on lososd's unit path, natively (modules/daemon.nix).
pub const NATIVE_TOOL: &str = "losos-nextcloud-apps";

/// The same tool inside the pod (nix/images.nix, `nextcloudApps`).
pub const IMAGE_TOOL: &str = "/bin/losos-nextcloud-apps";

/// Where the app store lists apps for one Nextcloud version.
const STORE_ENDPOINT: &str = "https://apps.nextcloud.com/api/v1/platform";

/// An app's page in the store, for a row whose entry names no website.
const STORE_PAGE: &str = "https://apps.nextcloud.com/apps";

/// Rows a search returns. The same bound as the chart catalogue's.
const LIMIT: usize = 20;

/// Apps LosOS sets itself, with the reason the page shows. Changing one
/// here would be undone at the next start, or would break what LosOS uses
/// it for.
pub const MANAGED: &[(&str, Managed)] = &[
    // Sign-in with LosOS Login (modules/app-sign-in.nix).
    ("user_saml", Managed::SignIn),
    // lososd asks it whether the owner's password is right, and device
    // setup mints app passwords through it (backend/src/signin.rs,
    // backend/src/devices.rs).
    ("provisioning_api", Managed::SignIn),
    // On only while the box shares its storage (lososInternal.federation).
    ("federation", Managed::Sharing),
    // The LosOS Cloud look (admin-ui/themes/).
    ("theming", Managed::Look),
    // Off on every start: it would open inside the setup wizard.
    ("firstrunwizard", Managed::Wizard),
];

/// Why LosOS keeps an app as it is. The page words each one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Managed {
    SignIn,
    Sharing,
    Look,
    Wizard,
}

/// What the owner can do to an app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verb {
    Install,
    Enable,
    Disable,
    Remove,
}

impl Verb {
    pub fn parse(raw: &str) -> Option<Verb> {
        Some(match raw {
            "install" => Verb::Install,
            "enable" => Verb::Enable,
            "disable" => Verb::Disable,
            "remove" => Verb::Remove,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Verb::Install => "install",
            Verb::Enable => "enable",
            Verb::Disable => "disable",
            Verb::Remove => "remove",
        }
    }
}

/// Where an app lives, which decides whether it can be removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    /// Shipped with Nextcloud.
    Core,
    /// Shipped with LosOS (modules/nextcloud-stack.nix, `apps`).
    Bundled,
    /// Added from the app store: the only kind that can be removed.
    Store,
}

/// Why an app cannot be changed here, when it cannot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "by", content = "reason")]
pub enum Locked {
    /// Nextcloud does not run without it.
    Nextcloud,
    /// LosOS sets it itself.
    Losos(Managed),
}

/// One app, as `GET /api/cloud/apps` lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudApp {
    pub id: String,
    pub name: String,
    pub summary: Option<String>,
    pub version: Option<String>,
    pub kind: Kind,
    pub enabled: bool,
    pub locked: Option<Locked>,
}

/// One row of `losos-nextcloud-apps list` (modules/nextcloud-apps-list.php).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListedApp {
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    version: String,
    kind: Kind,
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    always_enabled: bool,
}

fn non_empty(s: String) -> Option<String> {
    let t = s.trim();
    (!t.is_empty()).then(|| t.to_string())
}

/// Nextcloud's own rule for an app id: lower-case letters, digits and `_`.
/// Checked before the id goes into an argv; the tool checks it again.
pub fn validate_id(raw: &str) -> Result<&str, String> {
    let id = raw.trim();
    if id.is_empty() || id.len() > 64 {
        return Err("An app id is 1 to 64 characters.".to_string());
    }
    if !id
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
    {
        return Err("An app id has only lower-case letters, digits and underscores.".to_string());
    }
    Ok(id)
}

/// Why LosOS keeps `id` as it is, if it does.
pub fn managed(id: &str) -> Option<Managed> {
    MANAGED.iter().find(|(m, _)| *m == id).map(|(_, why)| *why)
}

/// The argv that runs the tool on `target`: `call` is `None` for the list.
pub fn plan(target: &Target, call: Option<(Verb, &str)>) -> Vec<String> {
    let mut argv: Vec<String> = match target {
        Target::Native => vec![NATIVE_TOOL.into()],
        Target::Container { socket, id } => vec![
            "crictl".into(),
            // Global flag, before the subcommand; see setup::resolve_argv.
            "--runtime-endpoint".into(),
            socket.clone(),
            "exec".into(),
            "--sync".into(),
            id.clone(),
            IMAGE_TOOL.into(),
        ],
    };
    match call {
        None => argv.push("list".into()),
        Some((verb, id)) => {
            argv.push(verb.as_str().into());
            argv.push(id.into());
        }
    }
    argv
}

/// The tool's list, with what the page may do to each app. Sorted by name,
/// as a reader scans for one.
pub fn parse_list(stdout: &str) -> anyhow::Result<Vec<CloudApp>> {
    let rows: Vec<ListedApp> = serde_json::from_str(stdout.trim())
        .map_err(|e| anyhow::anyhow!("losos-nextcloud-apps list printed no app list: {e}"))?;
    let mut apps: Vec<CloudApp> = rows
        .into_iter()
        .filter(|row| validate_id(&row.id).is_ok())
        .map(|row| {
            let locked = match managed(&row.id) {
                Some(why) => Some(Locked::Losos(why)),
                None if row.always_enabled => Some(Locked::Nextcloud),
                None => None,
            };
            CloudApp {
                name: non_empty(row.name).unwrap_or_else(|| row.id.clone()),
                id: row.id,
                summary: non_empty(row.summary),
                version: non_empty(row.version),
                kind: row.kind,
                enabled: row.enabled,
                locked,
            }
        })
        .collect();
    apps.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok(apps)
}

/// Whether `verb` may be done to `id`, given the current list. `Ok(false)`
/// is a change already made, which runs nothing.
pub fn check(apps: &[CloudApp], verb: Verb, id: &str) -> anyhow::Result<bool> {
    let app = apps.iter().find(|a| a.id == id);
    if let Some(why) = managed(id) {
        let what = match why {
            Managed::SignIn => "signing in",
            Managed::Sharing => "the sharing switch",
            Managed::Look => "the LosOS Cloud look",
            Managed::Wizard => "the setup wizard",
        };
        return Err(Invalid(format!("LosOS keeps {id} as it is for {what}.")).into());
    }
    match (verb, app) {
        (Verb::Install, Some(_)) => {
            Err(Conflict(format!("{id} is already in LosOS Cloud.")).into())
        }
        (Verb::Install, None) => Ok(true),
        (_, None) => Err(Invalid(format!("LosOS Cloud has no app called {id}.")).into()),
        (_, Some(app)) if app.locked == Some(Locked::Nextcloud) => {
            Err(Invalid(format!("LosOS Cloud does not run without {}.", app.name)).into())
        }
        (Verb::Enable, Some(app)) => Ok(!app.enabled),
        (Verb::Disable, Some(app)) => Ok(app.enabled),
        (Verb::Remove, Some(app)) if app.kind != Kind::Store => Err(Invalid(format!(
            "{} came with LosOS Cloud, so it can be turned off but not removed.",
            app.name
        ))
        .into()),
        (Verb::Remove, Some(_)) => Ok(true),
    }
}

/// A change Nextcloud turned down or could not make, with its own words:
/// the app store unreachable, an app that does not fit this version. The
/// HTTP layer answers it as a 502.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct Failed(pub String);

/// The last line worth reading of what occ printed.
fn last_line(s: &str) -> Option<String> {
    s.lines()
        .map(str::trim)
        .rev()
        .find(|l| !l.is_empty())
        .map(ToString::to_string)
}

/// Turn a finished tool run into success or the owner-readable reason.
pub fn interpret(outcome: &OccOutcome, verb: Verb, id: &str) -> Result<(), Failed> {
    if outcome.code == 0 {
        return Ok(());
    }
    let combined = format!("{}\n{}", outcome.stdout, outcome.stderr);
    if combined.contains("not installed") && combined.contains("Nextcloud") {
        return Err(Failed(
            "LosOS Cloud is still installing itself; try again in a few minutes.".to_string(),
        ));
    }
    let detail = last_line(&outcome.stderr).or_else(|| last_line(&outcome.stdout));
    let what = match verb {
        Verb::Install => "add",
        Verb::Enable => "turn on",
        Verb::Disable => "turn off",
        Verb::Remove => "remove",
    };
    Err(Failed(match detail {
        Some(d) => format!("LosOS Cloud could not {what} {id}: {d}"),
        None => format!("LosOS Cloud could not {what} {id} (exit {}).", outcome.code),
    }))
}

// ── The app store ─────────────────────────────────────────────────────────

/// One app the store offers, slimmed down to what a search shows. The
/// store's list for one version is about 17 MB, nearly all of it
/// descriptions and certificates, so the real backend keeps only these.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoreApp {
    pub id: String,
    pub name: String,
    pub summary: Option<String>,
    pub version: Option<String>,
    pub homepage: String,
    pub featured: bool,
    #[serde(skip)]
    pub rating: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawStoreApp {
    id: String,
    #[serde(default)]
    website: Option<String>,
    #[serde(default)]
    is_featured: bool,
    #[serde(default)]
    rating_overall: Option<f64>,
    #[serde(default)]
    translations: RawTranslations,
    #[serde(default)]
    releases: Vec<RawRelease>,
}

#[derive(Default, Deserialize)]
struct RawTranslations {
    #[serde(default)]
    en: Option<RawText>,
}

#[derive(Deserialize)]
struct RawText {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    summary: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawRelease {
    version: String,
    #[serde(default)]
    is_nightly: bool,
}

/// The store's list for Nextcloud `version`, or `None` for a version that is
/// not plain digits and dots (it goes into a URL).
pub fn store_url(version: &str) -> Option<String> {
    let v = version.trim();
    let plain = !v.is_empty()
        && v.len() <= 32
        && v.bytes().all(|b| b.is_ascii_digit() || b == b'.')
        && !v.starts_with('.');
    plain.then(|| format!("{STORE_ENDPOINT}/{v}/apps.json"))
}

fn is_https(u: &str) -> bool {
    u.starts_with("https://") && !u.chars().any(|c| c.is_whitespace() || c.is_control())
}

/// A release version as numbers, for ordering: the store lists releases as
/// strings, so "5.9.3" sorts above "5.12.4" there.
fn version_key(v: &str) -> Vec<u64> {
    v.split('.').map(|part| part.parse().unwrap_or(0)).collect()
}

/// The newest release that is neither a nightly nor a pre-release
/// ("2.0.0-beta.2"); a nightly-only or pre-release-only app has none.
fn newest_stable(releases: &[RawRelease]) -> Option<String> {
    releases
        .iter()
        .filter(|r| !r.is_nightly && !r.version.contains('-') && !r.version.is_empty())
        .max_by(|a, b| version_key(&a.version).cmp(&version_key(&b.version)))
        .map(|r| r.version.clone())
}

/// Parse the store's list. An entry without a usable id, or one the store
/// has no stable release of for this version, is left out.
pub fn parse_store(body: &str) -> anyhow::Result<Vec<StoreApp>> {
    let raw: Vec<RawStoreApp> = serde_json::from_str(body)
        .map_err(|e| anyhow::anyhow!("the app store sent a list this box cannot read: {e}"))?;
    Ok(raw
        .into_iter()
        .filter(|a| validate_id(&a.id).is_ok())
        .filter_map(|a| {
            let version = newest_stable(&a.releases)?;
            let en = a.translations.en;
            let name = en
                .as_ref()
                .and_then(|t| t.name.clone())
                .and_then(non_empty)
                .unwrap_or_else(|| a.id.clone());
            let summary = en.and_then(|t| t.summary).and_then(non_empty);
            let homepage = a
                .website
                .filter(|w| is_https(w))
                .unwrap_or_else(|| format!("{STORE_PAGE}/{}", a.id));
            Some(StoreApp {
                id: a.id,
                name,
                summary,
                version: Some(version),
                homepage,
                featured: a.is_featured,
                rating: a.rating_overall.unwrap_or(0.0),
            })
        })
        .collect())
}

/// The best matches for `query`: an id or name that is the query, then one
/// that starts with it, then one that contains it, then a summary that
/// does; featured and better-rated apps first within each.
pub fn search(index: &[StoreApp], query: &str) -> Vec<StoreApp> {
    let q = query.trim().to_lowercase();
    let rank = |a: &StoreApp| -> Option<u8> {
        let id = a.id.to_lowercase();
        let name = a.name.to_lowercase();
        if id == q || name == q {
            Some(0)
        } else if name.starts_with(&q) || id.starts_with(&q) {
            Some(1)
        } else if name.contains(&q) || id.contains(&q) {
            Some(2)
        } else if a
            .summary
            .as_deref()
            .is_some_and(|s| s.to_lowercase().contains(&q))
        {
            Some(3)
        } else {
            None
        }
    };
    let mut hits: Vec<(u8, &StoreApp)> = index
        .iter()
        .filter_map(|a| rank(a).map(|r| (r, a)))
        .collect();
    hits.sort_by(|(ra, a), (rb, b)| {
        ra.cmp(rb)
            .then_with(|| b.featured.cmp(&a.featured))
            .then_with(|| b.rating.total_cmp(&a.rating))
            .then_with(|| a.name.cmp(&b.name))
    });
    hits.into_iter()
        .take(LIMIT)
        .map(|(_, a)| a.clone())
        .collect()
}

// ── Commands ──────────────────────────────────────────────────────────────

/// Find the running LosOS Cloud, or say it is not running yet.
fn locate<L: Losos>(l: &mut L) -> anyhow::Result<Target> {
    let mode = l.nextcloud_mode()?;
    l.nextcloud_target(mode).map_err(|e| {
        tracing::info!(error = %e, "cloud apps: Nextcloud not found");
        Conflict(match mode {
            NcMode::Container => "LosOS Cloud is not running yet.".to_string(),
            NcMode::Native => "LosOS Cloud cannot be reached.".to_string(),
        })
        .into()
    })
}

fn list<L: Losos>(l: &mut L, target: &Target) -> anyhow::Result<Vec<CloudApp>> {
    let out = l.run_cloud_apps(&plan(target, None))?;
    if out.code != 0 {
        let detail = last_line(&out.stderr)
            .or_else(|| last_line(&out.stdout))
            .unwrap_or_else(|| format!("exit {}", out.code));
        tracing::warn!(detail = detail.as_str(), "losos-nextcloud-apps list failed");
        return Err(Failed(format!("LosOS Cloud did not list its apps: {detail}")).into());
    }
    parse_list(&out.stdout)
}

/// `GET /api/cloud/apps`. A LosOS Cloud that is not running, or not
/// answering yet, is `available: false` with the reason, never an error:
/// on a box in its first minutes that is the normal state.
pub fn cmd_cloud_apps<L: Losos>(l: &mut L) -> anyhow::Result<serde_json::Value> {
    let store = l.cloud_store_offered();
    let outcome = locate(l).and_then(|target| list(l, &target));
    Ok(match outcome {
        Ok(apps) => serde_json::json!({ "available": true, "store": store, "apps": apps }),
        Err(e) => {
            serde_json::json!({ "available": false, "store": store, "reason": e.to_string() })
        }
    })
}

/// `POST /api/cloud/apps/<verb>` `{id}`: one change, answered with the list
/// after it.
pub fn cmd_cloud_app<L: Losos>(
    l: &mut L,
    verb: Verb,
    raw_id: &str,
) -> anyhow::Result<serde_json::Value> {
    let id = validate_id(raw_id).map_err(Invalid)?;
    if verb == Verb::Install && !l.cloud_store_offered() {
        return Err(Conflict("This box does not offer the app store.".to_string()).into());
    }
    let target = locate(l)?;
    let before = list(l, &target)?;
    if check(&before, verb, id)? {
        let out = l.run_cloud_apps(&plan(&target, Some((verb, id))))?;
        interpret(&out, verb, id)?;
        tracing::info!(app = id, verb = verb.as_str(), "cloud app changed");
    }
    let apps = list(l, &target)?;
    Ok(serde_json::json!({ "available": true, "store": true, "apps": apps }))
}

/// `GET /api/cloud/apps/search?q=`: the app store's best matches. Whether
/// each is already in LosOS Cloud is the page's to tell from its list.
pub fn cmd_cloud_store_search<L: Losos>(
    l: &mut L,
    query: &str,
) -> anyhow::Result<serde_json::Value> {
    let query = crate::catalogue::validate_query(query).map_err(Invalid)?;
    if !l.cloud_store_offered() {
        return Err(Conflict("This box does not offer the app store.".to_string()).into());
    }
    let index = l.cloud_store()?;
    Ok(serde_json::json!({ "results": search(&index, query) }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::FakeLosos;

    const LIST: &str = r#"[
      {"id":"files","name":"Files","summary":"File management","version":"2.4.0","kind":"core","enabled":true,"alwaysEnabled":true},
      {"id":"activity","name":"Activity","summary":"","version":"5.0.0","kind":"core","enabled":true,"alwaysEnabled":false},
      {"id":"deck","name":"Deck","summary":"Kanban boards","version":"1.18.4","kind":"bundled","enabled":true,"alwaysEnabled":false},
      {"id":"user_saml","name":"SSO & SAML","summary":"","version":"7.0","kind":"bundled","enabled":true,"alwaysEnabled":false},
      {"id":"whiteboard","name":"Whiteboard","summary":"Draw together","version":"1.4","kind":"store","enabled":false,"alwaysEnabled":false},
      {"id":"Bad-Id","name":"Not an app","summary":"","version":"","kind":"store","enabled":false,"alwaysEnabled":false}
    ]"#;

    fn apps() -> Vec<CloudApp> {
        parse_list(LIST).unwrap()
    }

    fn container() -> Target {
        Target::Container {
            socket: "unix:///run/k3s/containerd/containerd.sock".into(),
            id: "abc123".into(),
        }
    }

    #[test]
    fn the_list_is_sorted_by_name_and_says_what_is_locked() {
        let apps = apps();
        let ids: Vec<&str> = apps.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(
            ids,
            ["activity", "deck", "files", "user_saml", "whiteboard"]
        );
        let by = |id: &str| apps.iter().find(|a| a.id == id).unwrap().clone();
        assert_eq!(by("files").locked, Some(Locked::Nextcloud));
        assert_eq!(by("user_saml").locked, Some(Locked::Losos(Managed::SignIn)));
        assert_eq!(by("deck").locked, None);
        assert_eq!(
            by("activity").summary,
            None,
            "an empty summary is no summary"
        );
        assert_eq!(by("whiteboard").kind, Kind::Store);
    }

    #[test]
    fn a_list_that_is_not_json_is_an_error() {
        assert!(parse_list("Nextcloud is not installed").is_err());
    }

    #[test]
    fn the_locked_reason_serialises_as_the_page_reads_it() {
        let v = serde_json::to_value(apps()).unwrap();
        let files = v
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["id"] == "files")
            .unwrap();
        assert_eq!(files["locked"], serde_json::json!({ "by": "nextcloud" }));
        let saml = v
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["id"] == "user_saml")
            .unwrap();
        assert_eq!(
            saml["locked"],
            serde_json::json!({ "by": "losos", "reason": "signIn" })
        );
        assert_eq!(saml["kind"], "bundled");
    }

    #[test]
    fn ids_are_checked_before_they_reach_an_argv() {
        assert_eq!(validate_id(" deck "), Ok("deck"));
        for bad in [
            "",
            "Deck",
            "deck;rm",
            "../deck",
            "deck app",
            "dé",
            &"a".repeat(65),
        ] {
            assert!(validate_id(bad).is_err(), "{bad:?} should be refused");
        }
    }

    #[test]
    fn the_argv_names_the_tool_in_the_pod_or_on_the_host() {
        assert_eq!(
            plan(&Target::Native, None),
            ["losos-nextcloud-apps", "list"]
        );
        assert_eq!(
            plan(&container(), Some((Verb::Disable, "deck"))),
            [
                "crictl",
                "--runtime-endpoint",
                "unix:///run/k3s/containerd/containerd.sock",
                "exec",
                "--sync",
                "abc123",
                "/bin/losos-nextcloud-apps",
                "disable",
                "deck"
            ]
        );
    }

    #[test]
    fn what_may_be_done_to_which_app() {
        let apps = apps();
        // Ordinary changes.
        assert!(check(&apps, Verb::Disable, "deck").unwrap());
        assert!(check(&apps, Verb::Enable, "whiteboard").unwrap());
        assert!(check(&apps, Verb::Remove, "whiteboard").unwrap());
        assert!(check(&apps, Verb::Install, "mail").unwrap());
        // Already so: nothing to run.
        assert!(!check(&apps, Verb::Enable, "deck").unwrap());
        assert!(!check(&apps, Verb::Disable, "whiteboard").unwrap());
        // Refused, each with its reason.
        let refused = |verb, id| check(&apps, verb, id).unwrap_err().to_string();
        assert!(refused(Verb::Disable, "files").contains("does not run without Files"));
        assert!(refused(Verb::Disable, "user_saml").contains("signing in"));
        assert!(refused(Verb::Install, "theming").contains("look"));
        assert!(refused(Verb::Remove, "deck").contains("turned off but not removed"));
        assert!(refused(Verb::Remove, "activity").contains("turned off but not removed"));
        assert!(refused(Verb::Enable, "nosuchapp").contains("no app called"));
        assert!(refused(Verb::Install, "deck").contains("already in LosOS Cloud"));
    }

    #[test]
    fn a_failed_change_says_what_occ_said() {
        let out = OccOutcome {
            code: 1,
            stdout: String::new(),
            stderr: "Could not download app whiteboard\n".into(),
        };
        let e = interpret(&out, Verb::Install, "whiteboard").unwrap_err();
        assert_eq!(
            e.0,
            "LosOS Cloud could not add whiteboard: Could not download app whiteboard"
        );
        let quiet = OccOutcome {
            code: 7,
            stdout: String::new(),
            stderr: String::new(),
        };
        assert!(interpret(&quiet, Verb::Remove, "x")
            .unwrap_err()
            .0
            .contains("exit 7"));
        assert!(interpret(&OccOutcome { code: 0, ..quiet }, Verb::Remove, "x").is_ok());
    }

    const STORE: &str = r#"[
      {"id":"whiteboard","website":"https://github.com/nextcloud/whiteboard","isFeatured":true,"ratingOverall":0.8,
       "translations":{"en":{"name":"Whiteboard","summary":"Draw together","description":"long"}},
       "releases":[{"version":"2.0.0-beta.2","isNightly":false},{"version":"1.10.0","isNightly":false},{"version":"1.4.0","isNightly":false}]},
      {"id":"white_noise","website":"javascript:alert(1)","isFeatured":false,"ratingOverall":0.9,
       "translations":{"en":{"name":"White noise","summary":"Sleep sounds"}},
       "releases":[{"version":"2.0.0-nightly","isNightly":true},{"version":"1.9.0","isNightly":false}]},
      {"id":"nightly_only","translations":{"en":{"name":"Nightly"}},"releases":[{"version":"0.1","isNightly":true}]},
      {"id":"drawio","website":"","translations":{"en":{"name":"Draw.io","summary":"Diagrams on a whiteboard"}},
       "releases":[{"version":"3.0.0","isNightly":false}]},
      {"id":"Bad Id","translations":{},"releases":[{"version":"1","isNightly":false}]}
    ]"#;

    #[test]
    fn the_store_list_is_slimmed_to_what_a_search_shows() {
        let index = parse_store(STORE).unwrap();
        let ids: Vec<&str> = index.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(
            ids,
            ["whiteboard", "white_noise", "drawio"],
            "no nightly-only, no bad id"
        );
        assert_eq!(index[1].version.as_deref(), Some("1.9.0"), "no nightly");
        assert_eq!(
            index[0].version.as_deref(),
            Some("1.10.0"),
            "newest by number, no beta"
        );
        assert_eq!(
            index[1].homepage,
            "https://apps.nextcloud.com/apps/white_noise"
        );
        assert_eq!(index[0].homepage, "https://github.com/nextcloud/whiteboard");
    }

    #[test]
    fn search_ranks_names_over_summaries() {
        let index = parse_store(STORE).unwrap();
        let ids = |q: &str| {
            search(&index, q)
                .into_iter()
                .map(|a| a.id)
                .collect::<Vec<_>>()
        };
        assert_eq!(ids("whiteboard"), ["whiteboard", "drawio"]);
        assert_eq!(
            ids("white"),
            ["whiteboard", "white_noise", "drawio"],
            "featured first"
        );
        assert_eq!(ids("SLEEP"), ["white_noise"]);
        assert!(ids("nothing like it").is_empty());
    }

    #[test]
    fn the_store_url_takes_only_a_plain_version() {
        assert_eq!(
            store_url("34.0.2").as_deref(),
            Some("https://apps.nextcloud.com/api/v1/platform/34.0.2/apps.json")
        );
        for bad in ["", "34.0.2/../x", "34.0 2", ".1", "34.0.2?x=1"] {
            assert_eq!(store_url(bad), None, "{bad:?}");
        }
    }

    fn fake() -> FakeLosos {
        let mut f = FakeLosos {
            nextcloud_mode: NcMode::Container,
            nextcloud_container: Some("abc123".into()),
            ..FakeLosos::default()
        };
        f.cloud_apps_list = LIST.to_string();
        f
    }

    #[test]
    fn listing_runs_the_tool_in_the_pod() {
        let mut f = fake();
        let v = cmd_cloud_apps(&mut f).unwrap();
        assert_eq!(v["available"], true);
        assert_eq!(v["apps"].as_array().unwrap().len(), 5);
        assert_eq!(f.cloud_apps_ran.len(), 1);
        assert_eq!(f.cloud_apps_ran[0].last().unwrap(), "list");
    }

    #[test]
    fn a_cloud_that_is_not_running_is_unavailable_not_an_error() {
        let mut f = fake();
        f.nextcloud_container = None;
        let v = cmd_cloud_apps(&mut f).unwrap();
        assert_eq!(v["available"], false);
        assert_eq!(v["reason"], "LosOS Cloud is not running yet.");
        assert!(f.cloud_apps_ran.is_empty());
    }

    #[test]
    fn a_change_checks_first_then_runs_then_lists_again() {
        let mut f = fake();
        let v = cmd_cloud_app(&mut f, Verb::Disable, "deck").unwrap();
        assert_eq!(v["available"], true);
        let verbs: Vec<String> = f
            .cloud_apps_ran
            .iter()
            .map(|argv| argv[7..].join(" "))
            .collect();
        assert_eq!(verbs, ["list", "disable deck", "list"]);
    }

    #[test]
    fn a_change_already_made_runs_nothing_but_the_lists() {
        let mut f = fake();
        cmd_cloud_app(&mut f, Verb::Enable, "deck").unwrap();
        assert!(f
            .cloud_apps_ran
            .iter()
            .all(|argv| argv.last().unwrap() == "list"));
    }

    #[test]
    fn a_refused_change_runs_nothing() {
        let mut f = fake();
        let e = cmd_cloud_app(&mut f, Verb::Disable, "user_saml").unwrap_err();
        assert!(e.downcast_ref::<Invalid>().is_some());
        assert_eq!(f.cloud_apps_ran.len(), 1, "only the list");
        let e = cmd_cloud_app(&mut f, Verb::Enable, "Bad;Id").unwrap_err();
        assert!(e.downcast_ref::<Invalid>().is_some());
    }

    #[test]
    fn a_change_occ_refuses_is_failed_with_its_words() {
        let mut f = fake();
        f.cloud_apps_action = OccOutcome {
            code: 1,
            stdout: String::new(),
            stderr: "App store unreachable".into(),
        };
        let e = cmd_cloud_app(&mut f, Verb::Install, "mail").unwrap_err();
        assert_eq!(
            e.downcast_ref::<Failed>().unwrap().0,
            "LosOS Cloud could not add mail: App store unreachable"
        );
    }

    #[test]
    fn installing_needs_the_store() {
        let mut f = fake();
        f.cloud_store_body = None;
        let e = cmd_cloud_app(&mut f, Verb::Install, "mail").unwrap_err();
        assert!(e.downcast_ref::<Conflict>().is_some());
        assert!(f.cloud_apps_ran.is_empty());
    }

    #[test]
    fn the_store_search_uses_the_parsed_index() {
        let mut f = fake();
        f.cloud_store_body = Some(STORE.to_string());
        let v = cmd_cloud_store_search(&mut f, "white").unwrap();
        let ids: Vec<&str> = v["results"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, ["whiteboard", "white_noise", "drawio"]);
        assert!(
            v["results"][0].get("rating").is_none(),
            "the rating only sorts"
        );
        assert!(cmd_cloud_store_search(&mut f, "w").is_err(), "too short");
    }
}
