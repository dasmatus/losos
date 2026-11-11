//! The box's own NixOS configuration as a repository on LosOS Git.
//!
//! `/etc/nixos` (`$LOSOS_CONFIG_DIR`) is the flake the box rebuilds from: the
//! published tree plus this box's `modules/install-target.nix` and
//! `modules/overrides.nix`. The installer makes it a git repository with one
//! commit; from then on every Apply, storage switch and factory reset commits
//! the change ([`crate::overrides::commit_message`] names the setting), and
//! lososd keeps the repository in step with one on the box's own LosOS Git
//! (`losos-git`, [`crate::forge`]), owned by the owner's account. The owner
//! can read the history there, clone it, and push: a push that moves the
//! branch ahead of the box is fetched, its `overrides.nix` put through the
//! same gates an Apply goes through, and fast-forwarded into `/etc/nixos`,
//! which then rebuilds.
//!
//! Nothing secret is in `/etc/nixos` — the LUKS keyfile is under `/etc/keys`,
//! the tokens under `/var/secrets` — and the repository is created private
//! anyway, because `/git/` is published through the master-proxy tunnel
//! whenever the owner turns that on.
//!
//! This module is the pure half: the sync decision ([`plan_sync`]), the
//! LosOS Git API request shapes ([`GitOp`]) and how their answers read
//! ([`classify`]), and the status document the History pane draws. The
//! effects are [`crate::losos::Losos`] methods, so [`crate::losos::
//! cmd_config_sync`] is tested against the fake with no git and no server.
//!
//! lososd talks to LosOS Git as its administrator, with the token `losos-git`
//! writes on its first start (`/var/lib/losos-git/admin-token`), over its
//! loopback listener: the API calls below, and git's own fetch and push.
//! LosOS Git holds no passwords; people sign in to it through LosOS Login,
//! and computers with the tokens [`GitOp::CreateToken`] mints.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Where the flake the box rebuilds from lives.
pub const DEFAULT_CONFIG_DIR: &str = "/etc/nixos";
/// LosOS Git's loopback listener for callers with a token (`modules/git.nix`).
pub const DEFAULT_GIT_URL: &str = "http://127.0.0.1:3000";
/// The admin token, written by `losos-git` on its first start.
pub const DEFAULT_TOKEN_FILE: &str = "/var/lib/losos-git/admin-token";
/// The user name lososd's pushes carry beside the admin token. Any name
/// would do; this one reads well in a log.
pub const BOT_USER: &str = "losos";
/// The commit identity lososd signs its commits with.
pub const COMMITTER_NAME: &str = "losos";
pub const COMMITTER_EMAIL: &str = "lososd@localhost";
/// What a fresh repository on LosOS Git is described as.
pub const REPO_DESCRIPTION: &str =
    "This box's NixOS configuration. LosOS commits every change here; push to this branch and the box picks it up.";

/// Where the configuration repository lives, from the daemon's environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoConfig {
    /// `$LOSOS_CONFIG_REPO`, as `owner/name`.
    pub owner: String,
    pub name: String,
    /// `$LOSOS_GIT_URL`.
    pub git_url: String,
    /// `$LOSOS_GIT_TOKEN_FILE`.
    pub token_file: String,
    /// `$LOSOS_CONFIG_REMOTE_URL`: a remote to use as it is, with no LosOS
    /// Git in front of it. The VM test points this at a bare repository on disk;
    /// an owner who wants their configuration mirrored elsewhere could too.
    pub remote_url: Option<String>,
}

impl RepoConfig {
    /// `None` when neither `$LOSOS_CONFIG_REPO` nor `$LOSOS_CONFIG_REMOTE_URL`
    /// is set: the feature is off (`losos.configRepo.enable = false`, or
    /// LosOS Git is off), and the daemon still commits locally.
    pub fn from_env() -> Option<RepoConfig> {
        let remote_url = std::env::var("LOSOS_CONFIG_REMOTE_URL")
            .ok()
            .filter(|s| !s.trim().is_empty());
        let repo = std::env::var("LOSOS_CONFIG_REPO").ok();
        let (owner, name) = match repo.as_deref().and_then(|r| r.split_once('/')) {
            Some((o, n)) if !o.is_empty() && !n.is_empty() => (o.to_string(), n.to_string()),
            _ if remote_url.is_some() => ("local".to_string(), "losos-config".to_string()),
            _ => return None,
        };
        Some(RepoConfig {
            owner,
            name,
            git_url: std::env::var("LOSOS_GIT_URL").unwrap_or_else(|_| DEFAULT_GIT_URL.to_string()),
            token_file: std::env::var("LOSOS_GIT_TOKEN_FILE")
                .unwrap_or_else(|_| DEFAULT_TOKEN_FILE.to_string()),
            remote_url,
        })
    }

    /// Whether LosOS Git is in the picture at all, or the remote is used as
    /// is.
    pub fn via_server(&self) -> bool {
        self.remote_url.is_none()
    }

    /// The URL git fetches from and pushes to. No credentials in it: those
    /// come from the credential store the daemon writes beside its state.
    pub fn remote(&self) -> String {
        self.remote_url
            .clone()
            .unwrap_or_else(|| format!("{}/{}/{}.git", self.git_url, self.owner, self.name))
    }

    /// The repository's page on the box, relative to the front vhost.
    pub fn page_url(&self) -> String {
        format!("/git/{}/{}", self.owner, self.name)
    }
}

// ── LosOS Git's API ─────────────────────────────────────────────────────────

/// One request to LosOS Git's API, made with the admin token
/// (`backend/src/forge/api.rs`, the `/api/v1/admin/` routes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitOp {
    /// Make sure an account exists, with this name and address when they
    /// are given (an empty one leaves what is there), and an administrator
    /// or not when `admin` says.
    EnsureUser {
        login: String,
        full_name: String,
        email: String,
        admin: Option<bool>,
    },
    /// Make sure a private repository exists under `owner`.
    EnsureRepo {
        owner: String,
        name: String,
        branch: String,
    },
    /// Make sure an app's private repository exists under `owner`
    /// ([`crate::app_repo`]). An existing one is left as it is, visibility
    /// included.
    EnsureAppRepo { owner: String, name: String },
    /// The repositories an account owns, private ones included. The
    /// sidebar's pin picker reads it.
    ListRepos { login: String },
    /// The repositories `login` may read whose owner, name or description
    /// holds `query`, newest first, at most `limit`. Asked as that person
    /// (`X-Losos-As`), so LosOS Git applies their permissions, not the
    /// administrator's.
    SearchRepos {
        login: String,
        query: String,
        limit: u32,
    },
    /// A token for one of `login`'s computers, replacing one of the same
    /// name: clone and push, and read who it belongs to.
    CreateToken { login: String, name: String },
    /// One repository as `login` sees it, with their permissions on it.
    RepoAs {
        login: String,
        owner: String,
        name: String,
    },
    /// Link a repository to a chat room (`crate::git_rooms`), as `login`,
    /// who must be its administrator. An empty `room` unlinks it.
    SetChatRoom {
        login: String,
        owner: String,
        name: String,
        room: String,
    },
}

impl GitOp {
    /// Method and path under LosOS Git's origin.
    pub fn route(&self) -> (&'static str, String) {
        match self {
            GitOp::EnsureUser { login, .. } => ("PUT", format!("/api/v1/admin/users/{login}")),
            GitOp::EnsureRepo { owner, name, .. } | GitOp::EnsureAppRepo { owner, name } => {
                ("PUT", format!("/api/v1/admin/users/{owner}/repos/{name}"))
            }
            GitOp::ListRepos { login } => ("GET", format!("/api/v1/users/{login}/repos")),
            GitOp::SearchRepos { query, limit, .. } => (
                "GET",
                format!(
                    "/api/v1/repos/search?q={}&limit={limit}",
                    crate::catalogue::percent_encode(query)
                ),
            ),
            GitOp::CreateToken { login, .. } => {
                ("POST", format!("/api/v1/admin/users/{login}/tokens"))
            }
            GitOp::RepoAs { owner, name, .. } => ("GET", format!("/api/v1/repos/{owner}/{name}")),
            GitOp::SetChatRoom { owner, name, .. } => {
                ("PATCH", format!("/api/v1/repos/{owner}/{name}"))
            }
        }
    }

    /// The person LosOS Git should answer as, when not the administrator.
    /// `None` also for a login that could not travel in a header.
    pub fn acting_as(&self) -> Option<&str> {
        match self {
            GitOp::SearchRepos { login, .. }
            | GitOp::RepoAs { login, .. }
            | GitOp::SetChatRoom { login, .. }
                if !login.is_empty()
                    && login
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b)) =>
            {
                Some(login)
            }
            _ => None,
        }
    }

    /// The JSON body, or `None` for a read.
    pub fn body(&self) -> Option<String> {
        let doc = match self {
            GitOp::ListRepos { .. } | GitOp::SearchRepos { .. } | GitOp::RepoAs { .. } => {
                return None
            }
            GitOp::SetChatRoom { room, .. } => json!({ "chat_room": room }),
            GitOp::EnsureUser {
                full_name,
                email,
                admin,
                ..
            } => {
                let mut doc = json!({ "full_name": full_name, "email": email });
                if let Some(a) = admin {
                    doc["admin"] = json!(a);
                }
                doc
            }
            GitOp::EnsureRepo { branch, .. } => json!({
                "private": true,
                "default_branch": branch,
                "description": REPO_DESCRIPTION,
            }),
            GitOp::EnsureAppRepo { .. } => json!({
                "private": true,
                "default_branch": crate::app_repo::BRANCH,
                "description": crate::app_repo::REPO_DESCRIPTION,
            }),
            GitOp::CreateToken { name, .. } => json!({
                "name": name,
                "scopes": crate::devices::GIT_SCOPES,
                "replace": true,
            }),
        };
        Some(doc.to_string())
    }
}

/// How LosOS Git answered.
#[derive(Debug, Clone, PartialEq)]
pub enum Answer {
    /// 2xx, with the body as JSON (or `Null` when it is not).
    Ok(Value),
    /// 404: no such user or repository.
    Missing,
    /// 409: the thing is there, which for "ensure" is as good as having
    /// made it.
    Exists,
    /// Anything else, with LosOS Git's own sentence when it gave one.
    Refused { status: u16, message: String },
}

/// Read one LosOS Git reply.
pub fn classify(status: u16, body: &str) -> Answer {
    let doc: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let message = doc
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    match status {
        200..=299 => Answer::Ok(doc),
        404 => Answer::Missing,
        409 => Answer::Exists,
        _ => Answer::Refused {
            status,
            message: if message.is_empty() {
                format!("LosOS Git answered {status}")
            } else {
                message
            },
        },
    }
}

/// LosOS Git could not be asked: not started yet, no token yet, or the
/// request never connected. Typed so [`crate::losos::cmd_config_sync`] can
/// report it as "unavailable" — the normal state of a box in its first
/// minutes — rather than as a fault.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct NotUp(pub String);

// ── The sync decision ───────────────────────────────────────────────────────

/// What one look at the local and remote branch heads calls for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncPlan {
    /// Same commit on both sides, or nothing committed locally yet.
    Nothing,
    /// The box is ahead (or the remote branch does not exist): push.
    Push,
    /// The remote is ahead and the box's head is in its history: take it.
    FastForward,
    /// Both moved. Nobody is merged on an appliance with no shell; say so.
    Diverged,
}

/// Decide from the two heads and the two ancestry questions.
///
/// `local_in_remote` is "is the local head an ancestor of the remote head";
/// `remote_in_local` the reverse. Both are asked only when the heads differ.
pub fn plan_sync(
    local: Option<&str>,
    remote: Option<&str>,
    local_in_remote: bool,
    remote_in_local: bool,
) -> SyncPlan {
    match (local, remote) {
        (None, _) => SyncPlan::Nothing,
        (Some(_), None) => SyncPlan::Push,
        (Some(l), Some(r)) if l == r => SyncPlan::Nothing,
        _ if remote_in_local => SyncPlan::Push,
        _ if local_in_remote => SyncPlan::FastForward,
        _ => SyncPlan::Diverged,
    }
}

// ── What the History pane reads ─────────────────────────────────────────────

/// The branch head of the configuration repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Head {
    pub sha: String,
    pub branch: String,
}

/// One line of `git log`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogEntry {
    pub sha: String,
    /// RFC 3339, the committer date.
    pub when: String,
    pub subject: String,
}

/// The outcome of the last sync, kept beside `state.json` so the pane can
/// show it between runs and after a daemon restart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SyncReport {
    /// `ok`, `unavailable`, `refused`, `diverged`, `waiting` or `error`.
    pub state: String,
    /// One sentence for the owner.
    pub detail: String,
    #[serde(default)]
    pub synced_at: Option<String>,
    /// The remote branch head the last sync saw, when it saw one.
    #[serde(default)]
    pub remote_head: Option<String>,
}

impl SyncReport {
    pub fn new(state: &str, detail: impl Into<String>) -> Self {
        SyncReport {
            state: state.to_string(),
            detail: detail.into(),
            synced_at: Some(now()),
            remote_head: None,
        }
    }
}

// ── Following the published tree ────────────────────────────────────────────
//
// The installer copies the published tree into `/etc/nixos` once, and both
// rebuild paths (the 03:00 `system.autoUpgrade` and every Apply) build from
// there. Left alone, that copy never changes, so the box never gets new
// software. Following moves it to the published branch's newest tree while
// keeping the files that belong to this box, which is all a box ever writes:
// the installer's `install-target.nix` and lososd's `overrides.nix`.
//
// It is a new commit on the box's own branch, not a merge: the installer's
// history and the published one share no commit, and the fetch is shallow, so
// a box never carries the project's history. The commit's parent is the
// box's head, so LosOS Git sees an ordinary push and the History pane an
// ordinary line.

/// The files the box writes itself. A follow keeps the box's copy of each;
/// everything else comes from the published tree.
pub const BOX_FILES: [&str; 2] = ["modules/install-target.nix", "modules/overrides.nix"];

/// Where `/etc/nixos` follows the published tree from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Upstream {
    /// `$LOSOS_UPSTREAM_URL`: a git URL, `https://github.com/dasmatus/losos.git`
    /// on a stock box (losos.configRepo.upstream.url).
    pub url: String,
    /// `$LOSOS_UPSTREAM_BRANCH`, `main` by default.
    pub branch: String,
}

impl Upstream {
    /// `None` when `$LOSOS_UPSTREAM_URL` is unset or empty: following is off
    /// (losos.configRepo.upstream.enable = false).
    pub fn from_env() -> Option<Upstream> {
        let url = std::env::var("LOSOS_UPSTREAM_URL")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())?;
        let branch = std::env::var("LOSOS_UPSTREAM_BRANCH")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "main".to_string());
        Some(Upstream { url, branch })
    }

    /// `dasmatus/losos main`, for the commit subject and the pane: the last
    /// two path segments of the URL without `.git`, then the branch.
    pub fn label(&self) -> String {
        let trimmed = self.url.trim_end_matches('/').trim_end_matches(".git");
        let mut parts = trimmed.rsplit(['/', ':']);
        let repo = parts.next().unwrap_or(trimmed);
        let name = match parts.next() {
            Some(owner) if !owner.is_empty() => format!("{owner}/{repo}"),
            _ => repo.to_string(),
        };
        format!("{name} {}", self.branch)
    }
}

/// What the last follow recorded: the published commit it took and the box
/// commit it made (`refs/losos/upstream` and `refs/losos/followed`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FollowMark {
    pub upstream: String,
    pub followed: String,
}

/// What one look at the published branch calls for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FollowPlan {
    /// The box already has the published commit it would take.
    UpToDate,
    /// Somebody changed files outside [`BOX_FILES`] since the last follow
    /// (a push to LosOS Git from a clone). Following would overwrite them,
    /// so the box keeps its tree and says so.
    Kept,
    /// The published commit was cut from an older main than the one the box
    /// took last: the box moved to a slower channel. Taking it would run the
    /// box's data under older code, and migrations do not run backwards, so
    /// the box keeps its tree until the channel passes it.
    Holding,
    /// Take the published tree.
    Follow,
}

/// Decide from the fetched head, the last mark, whether the box's tree was
/// edited outside its own files since that mark (or since the install, when
/// there is no mark yet), and whether the fetched head is older than the
/// marked one by [`base_of`].
pub fn plan_follow(
    fetched: &str,
    mark: Option<&FollowMark>,
    edited_outside: bool,
    older: bool,
) -> FollowPlan {
    match mark {
        Some(m) if m.upstream == fetched && !edited_outside => FollowPlan::UpToDate,
        _ if edited_outside => FollowPlan::Kept,
        Some(_) if older => FollowPlan::Holding,
        _ => FollowPlan::Follow,
    }
}

// ── Release channels ────────────────────────────────────────────────────────
//
// `channel/daily`, `channel/weekly` and `channel/monthly` are branches that
// .github/workflows/channels.yml moves (.github/scripts/channels.py). A cut is
// a commit whose tree is exactly main's at a commit whose CI run passed, and
// a hotfix is a cherry-pick on top. Each of those commits names the main
// commit it was cut from in a trailer:
//
//     Channel-Base: <sha> <unix seconds of that commit>
//
// The box fetches one commit deep and cannot ask git whether one commit
// descends from another, so it compares that time instead. A commit of main
// itself carries no trailer, and its own committer time stands in.

/// The trailer naming the main commit a channel commit was cut from.
pub const CHANNEL_BASE: &str = "Channel-Base:";

/// When the main a published commit was cut from was made, in unix seconds:
/// the last `Channel-Base:` trailer of `message`, else `committed_at`.
pub fn base_of(committed_at: i64, message: &str) -> i64 {
    message
        .lines()
        .rev()
        .filter_map(|line| line.trim().strip_prefix(CHANNEL_BASE))
        .find_map(|rest| rest.split_whitespace().nth(1)?.parse().ok())
        .unwrap_or(committed_at)
}

/// The current time, RFC 3339 to the second.
pub fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

// ── The reconciler ──────────────────────────────────────────────────────────

/// How long after start the first sync runs. LosOS Git may still be
/// starting, and a daemon restarted by a rebuild has no reason to hurry.
pub const FIRST_SYNC_AFTER: std::time::Duration = std::time::Duration::from_secs(20);
/// The gap between syncs. A push from a clone is picked up within this.
pub const SYNC_EVERY: std::time::Duration = std::time::Duration::from_secs(30);

static NUDGED: std::sync::Mutex<bool> = std::sync::Mutex::new(false);
static WAKE: std::sync::Condvar = std::sync::Condvar::new();

/// Run the next sync now rather than at the end of the wait: a saved app
/// reaches LosOS Git in a second, not half a minute.
pub fn nudge() {
    let mut nudged = NUDGED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *nudged = true;
    WAKE.notify_all();
}

fn wait_for_nudge(at_most: std::time::Duration) {
    let nudged = NUDGED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let (mut nudged, _) = WAKE
        .wait_timeout_while(nudged, at_most, |n| !*n)
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *nudged = false;
}

/// Keep `/etc/nixos` in step with LosOS Git for the life of the daemon.
///
/// One detached thread, running [`crate::losos::cmd_config_sync`] and then
/// [`crate::user_apps::cmd_sync`] under the state lock every [`SYNC_EVERY`],
/// or sooner after a [`nudge`]. Nothing is started when no repository is
/// configured: a box with LosOS Git off commits locally and that is all.
/// Every outcome is a report the History pane reads, so the thread itself
/// never has anything to say beyond a debug line.
pub fn start_reconciler(backend: &crate::io_backend::IoLosos) {
    if RepoConfig::from_env().is_none() {
        tracing::info!("no configuration repository configured; not syncing with LosOS Git");
        return;
    }
    let backend = backend.clone();
    let spawned = std::thread::Builder::new()
        .name("lososd-config".into())
        .spawn(move || {
            std::thread::sleep(FIRST_SYNC_AFTER);
            loop {
                match backend.serialized(crate::losos::cmd_config_sync) {
                    Ok(doc) => tracing::debug!(
                        state = doc["sync"]["state"].as_str().unwrap_or(""),
                        "configuration sync"
                    ),
                    Err(e) => tracing::warn!(error = ?e, "configuration sync could not run"),
                }
                // The owner's apps, each in a repository of its own
                // (crate::app_repo), ride the same thread.
                if let Err(e) = backend.serialized(crate::user_apps::cmd_sync) {
                    tracing::warn!(error = ?e, "the apps' repositories could not be synced");
                }
                wait_for_nudge(SYNC_EVERY);
            }
        });
    if let Err(e) = spawned {
        tracing::error!(error = ?e, "could not start the configuration sync thread");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_plan_follows_the_heads() {
        assert_eq!(plan_sync(None, None, false, false), SyncPlan::Nothing);
        assert_eq!(plan_sync(Some("a"), None, false, false), SyncPlan::Push);
        assert_eq!(
            plan_sync(Some("a"), Some("a"), false, false),
            SyncPlan::Nothing
        );
        // The remote is behind: its head is in the local history.
        assert_eq!(plan_sync(Some("b"), Some("a"), false, true), SyncPlan::Push);
        // The remote is ahead: the local head is in its history.
        assert_eq!(
            plan_sync(Some("a"), Some("b"), true, false),
            SyncPlan::FastForward
        );
        assert_eq!(
            plan_sync(Some("a"), Some("b"), false, false),
            SyncPlan::Diverged
        );
    }

    #[test]
    fn the_follow_plan_takes_new_commits_and_never_overwrites_edits() {
        let mark = FollowMark {
            upstream: "u1".into(),
            followed: "b1".into(),
        };
        // A box that never followed takes the published tree.
        assert_eq!(plan_follow("u1", None, false, false), FollowPlan::Follow);
        assert_eq!(
            plan_follow("u1", Some(&mark), false, false),
            FollowPlan::UpToDate
        );
        assert_eq!(
            plan_follow("u2", Some(&mark), false, false),
            FollowPlan::Follow
        );
        // An edit outside the box's own files wins, new commit or not.
        assert_eq!(
            plan_follow("u2", Some(&mark), true, false),
            FollowPlan::Kept
        );
        assert_eq!(
            plan_follow("u1", Some(&mark), true, false),
            FollowPlan::Kept
        );
        assert_eq!(plan_follow("u1", None, true, false), FollowPlan::Kept);
        assert_eq!(plan_follow("u2", Some(&mark), true, true), FollowPlan::Kept);
    }

    #[test]
    fn a_slower_channel_never_takes_the_box_back() {
        let mark = FollowMark {
            upstream: "daily".into(),
            followed: "b1".into(),
        };
        assert_eq!(
            plan_follow("weekly", Some(&mark), false, true),
            FollowPlan::Holding
        );
        // Once the channel passes the box, it moves again.
        assert_eq!(
            plan_follow("weekly", Some(&mark), false, false),
            FollowPlan::Follow
        );
        // Without a mark there is nothing to be older than.
        assert_eq!(plan_follow("weekly", None, false, true), FollowPlan::Follow);
    }

    #[test]
    fn the_base_of_a_channel_commit_is_its_trailer() {
        // A commit of main: its own time.
        assert_eq!(
            base_of(1_700_000_000, "Merge pull request #12\n\nA fix"),
            1_700_000_000
        );
        // A cut: the main commit it was cut from.
        let cut =
            "Cut channel/weekly from main 1234567\n\nChannel-Base: 1234567890abcdef 1690000000\n";
        assert_eq!(base_of(1_700_000_000, cut), 1_690_000_000);
        // A hotfix keeps the channel's base; the last trailer counts.
        let hotfix = "Hotfix #40 on channel/weekly: Fix the tunnel\n\n\
                      Channel-Base: aaaa 1680000000\nChannel-Base: bbbb 1690000000\nHotfix: #40 cccc";
        assert_eq!(base_of(1_700_000_000, hotfix), 1_690_000_000);
        // A trailer that does not parse is no trailer.
        assert_eq!(base_of(5, "Channel-Base: nonsense"), 5);
        assert_eq!(base_of(5, "Channel-Base: abc soon"), 5);
    }

    #[test]
    fn the_upstream_label_names_the_repository_and_branch() {
        let up = |url: &str| Upstream {
            url: url.into(),
            branch: "main".into(),
        };
        assert_eq!(
            up("https://github.com/dasmatus/losos.git").label(),
            "dasmatus/losos main"
        );
        assert_eq!(
            up("https://example.org/acme/losos/").label(),
            "acme/losos main"
        );
        assert_eq!(
            up("git@github.com:acme/losos.git").label(),
            "acme/losos main"
        );
        assert_eq!(up("/srv/losos.git").label(), "srv/losos main");
    }

    #[test]
    fn requests_are_the_admin_routes() {
        let op = GitOp::EnsureUser {
            login: "notshared".into(),
            full_name: "Jana Novak".into(),
            email: String::new(),
            admin: Some(true),
        };
        assert_eq!(
            op.route(),
            ("PUT", "/api/v1/admin/users/notshared".to_string())
        );
        let body: Value = serde_json::from_str(&op.body().unwrap()).unwrap();
        assert_eq!(body["admin"], true);
        assert_eq!(body["full_name"], "Jana Novak");
        let op = GitOp::EnsureUser {
            login: "jana".into(),
            full_name: String::new(),
            email: String::new(),
            admin: None,
        };
        let body: Value = serde_json::from_str(&op.body().unwrap()).unwrap();
        assert!(
            body.get("admin").is_none(),
            "an unsaid admin flag is left as it is"
        );

        let op = GitOp::EnsureRepo {
            owner: "notshared".into(),
            name: "losos-config".into(),
            branch: "main".into(),
        };
        assert_eq!(
            op.route(),
            (
                "PUT",
                "/api/v1/admin/users/notshared/repos/losos-config".to_string()
            )
        );
        let body: Value = serde_json::from_str(&op.body().unwrap()).unwrap();
        assert_eq!(body["private"], true, "the tunnel publishes /git/");
        assert_eq!(body["default_branch"], "main");

        let op = GitOp::CreateToken {
            login: "notshared".into(),
            name: "LosOS on laptop".into(),
        };
        let body: Value = serde_json::from_str(&op.body().unwrap()).unwrap();
        assert_eq!(body["replace"], true);
        assert_eq!(body["scopes"], json!(crate::devices::GIT_SCOPES));
        assert!(GitOp::ListRepos { login: "x".into() }.body().is_none());
        let search = GitOp::SearchRepos {
            login: "eva".into(),
            query: "notes & todo".into(),
            limit: 8,
        };
        assert_eq!(
            search.route(),
            (
                "GET",
                "/api/v1/repos/search?q=notes%20%26%20todo&limit=8".to_string()
            )
        );
        assert!(search.body().is_none());
        assert_eq!(search.acting_as(), Some("eva"));
        let smuggled = GitOp::SearchRepos {
            login: "eva\r\nX-Evil: 1".into(),
            query: String::new(),
            limit: 8,
        };
        assert_eq!(smuggled.acting_as(), None, "a header break never travels");
        assert_eq!(GitOp::ListRepos { login: "x".into() }.acting_as(), None);
    }

    #[test]
    fn answers_are_read_by_status() {
        assert_eq!(classify(200, r#"{"id": 1}"#), Answer::Ok(json!({"id": 1})));
        assert_eq!(classify(201, "not json"), Answer::Ok(Value::Null));
        assert_eq!(
            classify(404, r#"{"message":"user does not exist"}"#),
            Answer::Missing
        );
        assert_eq!(
            classify(
                409,
                r#"{"message":"The repository with the same name already exists."}"#
            ),
            Answer::Exists
        );
        assert_eq!(
            classify(422, r#"{"message":"password is too short"}"#),
            Answer::Refused {
                status: 422,
                message: "password is too short".into()
            }
        );
        assert_eq!(
            classify(500, ""),
            Answer::Refused {
                status: 500,
                message: "LosOS Git answered 500".into()
            }
        );
    }

    #[test]
    fn the_repo_config_derives_its_urls() {
        let c = RepoConfig {
            owner: "notshared".into(),
            name: "losos-config".into(),
            git_url: "http://127.0.0.1:3000".into(),
            token_file: "/var/lib/losos-git/admin-token".into(),
            remote_url: None,
        };
        assert_eq!(
            c.remote(),
            "http://127.0.0.1:3000/notshared/losos-config.git"
        );
        assert_eq!(c.page_url(), "/git/notshared/losos-config");
        assert!(c.via_server());
        let seam = RepoConfig {
            remote_url: Some("/tmp/bare.git".into()),
            ..c
        };
        assert_eq!(seam.remote(), "/tmp/bare.git");
        assert!(!seam.via_server());
    }
}
