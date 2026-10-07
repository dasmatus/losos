//! The box's own NixOS configuration as a repository on LosOS Git.
//!
//! `/etc/nixos` (`$LOSOS_CONFIG_DIR`) is the flake the box rebuilds from: the
//! published tree plus this box's `modules/install-target.nix` and
//! `modules/overrides.nix`. The installer makes it a git repository with one
//! commit; from then on every Apply, storage switch and factory reset commits
//! the change ([`crate::overrides::commit_message`] names the setting), and
//! lososd keeps the repository in step with one on the box's own Forgejo,
//! owned by the owner's account. The owner can read the history there, clone
//! it, and push: a push that moves the branch ahead of the box is fetched,
//! its `overrides.nix` put through the same gates an Apply goes through, and
//! fast-forwarded into `/etc/nixos`, which then rebuilds.
//!
//! Nothing secret is in `/etc/nixos` — the LUKS keyfile is under `/etc/keys`,
//! the tokens under `/var/secrets` — and the repository is created private
//! anyway, because `/forgejo/` is published through the master-proxy tunnel
//! whenever the owner turns that on.
//!
//! This module is the pure half: the sync decision ([`plan_sync`]), the
//! Forgejo API request shapes ([`ForgejoOp`]) and how their answers read
//! ([`classify`]), and the status document the History pane draws. The
//! effects are [`crate::losos::Losos`] methods, so [`crate::losos::
//! cmd_config_sync`] is tested against the fake with no git and no Forgejo.
//!
//! One account does the talking: `losos`, a site administrator the Forgejo
//! bootstrap (`flake/forgejo-bootstrap.nix`) creates with a random password
//! and an access token at `$LOSOS_FORGEJO_TOKEN_FILE`. lososd never reads the
//! owner's Forgejo password back; it *sets* it, from the one password the
//! owner proves at claim, set-password and sign-in time, so LosOS Git opens
//! with the same password as everything else.

use crate::setup::Secret;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fmt;

/// Where the flake the box rebuilds from lives.
pub const DEFAULT_CONFIG_DIR: &str = "/etc/nixos";
/// Forgejo's loopback listener, the same in both modes (`modules/workloads.nix`
/// and `services.forgejo` in `modules/services.nix`).
pub const DEFAULT_FORGEJO_URL: &str = "http://127.0.0.1:3000";
/// The bot account's access token, written by the Forgejo bootstrap.
pub const DEFAULT_TOKEN_FILE: &str = "/var/lib/forgejo/.losos-token";
/// The site-administrator account the bootstrap creates for lososd.
pub const BOT_USER: &str = "losos";
/// The commit identity lososd signs its commits with.
pub const COMMITTER_NAME: &str = "losos";
pub const COMMITTER_EMAIL: &str = "lososd@localhost";
/// What a fresh repository on Forgejo is described as.
pub const REPO_DESCRIPTION: &str =
    "This box's NixOS configuration. LosOS commits every change here; push to this branch and the box picks it up.";

/// Where the configuration repository lives, from the daemon's environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoConfig {
    /// `$LOSOS_CONFIG_REPO`, as `owner/name`.
    pub owner: String,
    pub name: String,
    /// `$LOSOS_FORGEJO_URL`.
    pub forgejo_url: String,
    /// `$LOSOS_FORGEJO_TOKEN_FILE`.
    pub token_file: String,
    /// `$LOSOS_CONFIG_REMOTE_URL`: a remote to use as it is, with no Forgejo
    /// in front of it. The VM test points this at a bare repository on disk;
    /// an owner who wants their configuration mirrored elsewhere could too.
    pub remote_url: Option<String>,
}

impl RepoConfig {
    /// `None` when neither `$LOSOS_CONFIG_REPO` nor `$LOSOS_CONFIG_REMOTE_URL`
    /// is set: the feature is off (`losos.configRepo.enable = false`, or
    /// Forgejo is off), and the daemon still commits locally.
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
            forgejo_url: std::env::var("LOSOS_FORGEJO_URL")
                .unwrap_or_else(|_| DEFAULT_FORGEJO_URL.to_string()),
            token_file: std::env::var("LOSOS_FORGEJO_TOKEN_FILE")
                .unwrap_or_else(|_| DEFAULT_TOKEN_FILE.to_string()),
            remote_url,
        })
    }

    /// Whether Forgejo is in the picture at all, or the remote is used as is.
    pub fn via_forgejo(&self) -> bool {
        self.remote_url.is_none()
    }

    /// The URL git fetches from and pushes to. No credentials in it: those
    /// come from the credential store the daemon writes beside its state.
    pub fn remote(&self) -> String {
        self.remote_url
            .clone()
            .unwrap_or_else(|| format!("{}/{}/{}.git", self.forgejo_url, self.owner, self.name))
    }

    /// The repository's page on the box, relative to the front vhost.
    pub fn page_url(&self) -> String {
        format!("/forgejo/{}/{}", self.owner, self.name)
    }
}

// ── Forgejo's API ───────────────────────────────────────────────────────────

/// One request to Forgejo's REST API, made as the bot administrator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForgejoOp {
    GetUser {
        login: String,
    },
    /// Create the owner's account. The password is the `Secret` handed to
    /// [`ForgejoOp::body`], never a field, so it is in no `Debug` output.
    CreateUser {
        login: String,
    },
    /// Make an account a site administrator (the owner is; `losos` is too).
    MakeAdmin {
        login: String,
    },
    /// Replace an account's password with the `Secret` given to `body`.
    SetPassword {
        login: String,
    },
    GetRepo {
        owner: String,
        name: String,
    },
    /// Create a private repository under `owner`, as the administrator.
    CreateRepo {
        owner: String,
        name: String,
        branch: String,
    },
}

impl ForgejoOp {
    /// Method and path under the Forgejo origin.
    pub fn route(&self) -> (&'static str, String) {
        match self {
            ForgejoOp::GetUser { login } => ("GET", format!("/api/v1/users/{login}")),
            ForgejoOp::CreateUser { .. } => ("POST", "/api/v1/admin/users".to_string()),
            ForgejoOp::MakeAdmin { login } | ForgejoOp::SetPassword { login } => {
                ("PATCH", format!("/api/v1/admin/users/{login}"))
            }
            ForgejoOp::GetRepo { owner, name } => ("GET", format!("/api/v1/repos/{owner}/{name}")),
            ForgejoOp::CreateRepo { owner, .. } => {
                ("POST", format!("/api/v1/admin/users/{owner}/repos"))
            }
        }
    }

    /// The JSON body, or `None` for a read. `secret` is required by the two
    /// operations that carry a password and ignored by the rest.
    pub fn body(&self, secret: Option<&Secret>) -> anyhow::Result<Option<String>> {
        let doc = match self {
            ForgejoOp::GetUser { .. } | ForgejoOp::GetRepo { .. } => return Ok(None),
            ForgejoOp::CreateUser { login } => {
                let secret =
                    secret.ok_or_else(|| anyhow::anyhow!("creating a user needs a password"))?;
                json!({
                    "username": login,
                    "email": format!("{login}@localhost"),
                    "password": secret.expose(),
                    "must_change_password": false,
                    "send_notify": false,
                    "visibility": "private",
                })
            }
            // Forgejo's EditUserOption insists on login_name and source_id
            // even when neither changes; 0 is the local authentication source.
            ForgejoOp::MakeAdmin { login } => json!({
                "login_name": login,
                "source_id": 0,
                "admin": true,
            }),
            ForgejoOp::SetPassword { login } => {
                let secret =
                    secret.ok_or_else(|| anyhow::anyhow!("setting a password needs one"))?;
                json!({
                    "login_name": login,
                    "source_id": 0,
                    "password": secret.expose(),
                    "must_change_password": false,
                })
            }
            ForgejoOp::CreateRepo { name, branch, .. } => json!({
                "name": name,
                "private": true,
                "default_branch": branch,
                "description": REPO_DESCRIPTION,
                "auto_init": false,
            }),
        };
        Ok(Some(doc.to_string()))
    }
}

/// How Forgejo answered.
#[derive(Debug, Clone, PartialEq)]
pub enum Answer {
    /// 2xx, with the body as JSON (or `Null` when it is not).
    Ok(Value),
    /// 404: no such user or repository.
    Missing,
    /// 409, or the 422 Forgejo gives a user that already exists: the thing
    /// is there, which for "ensure" is as good as having made it.
    Exists,
    /// Anything else, with Forgejo's own sentence when it gave one.
    Refused { status: u16, message: String },
}

/// Read one Forgejo reply.
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
        422 if message.contains("already exists") => Answer::Exists,
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotUp(pub String);

impl fmt::Display for NotUp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for NotUp {}

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

/// The current time, RFC 3339 to the second.
pub fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

// ── The reconciler ──────────────────────────────────────────────────────────

/// How long after start the first sync runs. Forgejo takes a while on a
/// fresh box, and a daemon restarted by a rebuild has no reason to hurry.
pub const FIRST_SYNC_AFTER: std::time::Duration = std::time::Duration::from_secs(20);
/// The gap between syncs. A push from a clone is picked up within this.
pub const SYNC_EVERY: std::time::Duration = std::time::Duration::from_secs(30);

/// Keep `/etc/nixos` in step with LosOS Git for the life of the daemon.
///
/// One detached thread, running [`crate::losos::cmd_config_sync`] under the
/// state lock every [`SYNC_EVERY`]. Nothing is started when no repository is
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
                std::thread::sleep(SYNC_EVERY);
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
    fn requests_carry_the_password_in_the_body_only() {
        let secret = Secret::new("correct horse battery staple");
        let op = ForgejoOp::CreateUser {
            login: "notshared".into(),
        };
        assert_eq!(op.route(), ("POST", "/api/v1/admin/users".to_string()));
        let body = op.body(Some(&secret)).unwrap().unwrap();
        assert!(body.contains("\"password\":\"correct horse battery staple\""));
        assert!(!format!("{op:?}").contains("horse"));
        assert!(op.body(None).is_err());

        let op = ForgejoOp::SetPassword {
            login: "notshared".into(),
        };
        assert_eq!(
            op.route(),
            ("PATCH", "/api/v1/admin/users/notshared".to_string())
        );
        let body: Value = serde_json::from_str(&op.body(Some(&secret)).unwrap().unwrap()).unwrap();
        assert_eq!(body["login_name"], "notshared");
        assert_eq!(body["source_id"], 0);
        assert_eq!(body["must_change_password"], false);

        let op = ForgejoOp::CreateRepo {
            owner: "notshared".into(),
            name: "losos-config".into(),
            branch: "main".into(),
        };
        assert_eq!(
            op.route(),
            ("POST", "/api/v1/admin/users/notshared/repos".to_string())
        );
        let body: Value = serde_json::from_str(&op.body(None).unwrap().unwrap()).unwrap();
        assert_eq!(body["private"], true, "the tunnel publishes /forgejo/");
        assert_eq!(body["default_branch"], "main");
        assert!(ForgejoOp::GetUser { login: "x".into() }
            .body(None)
            .unwrap()
            .is_none());
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
            classify(
                422,
                r#"{"message":"user already exists [name: notshared]"}"#
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
            forgejo_url: "http://127.0.0.1:3000".into(),
            token_file: "/var/lib/forgejo/.losos-token".into(),
            remote_url: None,
        };
        assert_eq!(
            c.remote(),
            "http://127.0.0.1:3000/notshared/losos-config.git"
        );
        assert_eq!(c.page_url(), "/forgejo/notshared/losos-config");
        assert!(c.via_forgejo());
        let seam = RepoConfig {
            remote_url: Some("/tmp/bare.git".into()),
            ..c
        };
        assert_eq!(seam.remote(), "/tmp/bare.git");
        assert!(!seam.via_forgejo());
    }
}
