//! The official-edge key ceremony, run from the developer's own machine.
//!
//! `identity` (the sibling module) holds the primitives: a key, a signature,
//! a certificate, the four checks. This module is what an operator actually
//! runs, and the one thing it adds is **who**: every step that makes or uses
//! the root key first signs the operator in with GitHub's OAuth *device flow*
//! and refuses unless the signed-in account is on the allowlist compiled into
//! the binary (`operators.json` beside `Cargo.toml`).
//!
//! The device flow was chosen because it needs no secret anywhere: the tool
//! carries only the OAuth App's public client id, prints a one-time code, the
//! operator types it into github.com in any browser, and GitHub hands back a
//! token for that account. The tool then asks `GET /user` who that is and
//! compares the **numeric id** (logins can be renamed; ids cannot) with the
//! list. No token is stored: each gated command signs in afresh, or takes a
//! token from `LOSOS_GITHUB_TOKEN` (for example `gh auth token`), which is
//! checked the same way.
//!
//! What the gate is and is not: the root key is the whole secret, and anyone
//! holding `root.key` can sign a certificate with any Ed25519 tool. The
//! allowlist decides whom *this* tooling serves and makes the ceremony an
//! auditable, named act; it does not replace keeping the key offline.
//!
//! The steps, in the order an operator runs them (`provisioning/edge-identity/README.md`):
//!
//! 1. `provision root-keygen --out root.key [--publish]`: makes the root key
//!    (0600, never overwritten) and, with `--publish`, opens the pull
//!    request that writes the public half into `keys/official-edge-root.pub`
//!    using the same sign-in (scope `public_repo`; the default sign-in asks
//!    for no scope at all).
//! 2. `provision edge --name … --url … --ssh root@edge --root-key root.key`:
//!    makes the edge's key pair *in memory*, signs its certificate, ships
//!    both over one SSH session on stdin (nothing secret in argv, nothing
//!    written to this machine's disk), restarts the registrar, and runs the
//!    box's four checks against `GET /identity?nonce=`.
//! 3. `provision verify --url … --root-key root.key`: the checks alone,
//!    after the edge's configuration names the two files.
//!
//! Previously this was a pair of Forgejo Actions workflows on the box with an
//! on-box runner; that put the root key inside the appliance's `/var`, where
//! a reinstall lost it, and let anyone with a LosOS Git account run code on
//! the box. Both are gone.

use std::path::Path;
use std::process::ExitCode;
use std::time::Duration;

use miette::{miette, Context, IntoDiagnostic, Result};
use ring::rand::SecureRandom;
use serde::Deserialize;

use crate::action::Action;
use crate::identity::{self, Cert, Rejected};
use crate::opts::ProvisionOpts;

/// The committed allowlist, compiled in. Edited by pull request, like the
/// root public key: who may run the ceremony is a reviewed fact of the tree.
const OPERATORS_JSON: &str = include_str!("../operators.json");

/// Where GitHub's device flow and REST API live. Overridable only so the
/// tests can stand up a fake GitHub on loopback.
pub const GITHUB_OAUTH_URL: &str = "https://github.com";
pub const GITHUB_API_URL: &str = "https://api.github.com";

/// Exit code of a refused sign-in (not on the list, or denied on GitHub),
/// distinct from a plain failure so a script can tell the two apart.
pub const EXIT_REFUSED: u8 = 3;
/// Exit code of `provision edge` when the files are installed but the edge
/// does not answer `/identity` yet (its configuration has to name them).
pub const EXIT_NOT_YET: u8 = 2;

const HTTP_TIMEOUT: Duration = Duration::from_secs(20);
/// GitHub's minimum polling interval; a `slow_down` adds five seconds.
const SLOW_DOWN_EXTRA: Duration = Duration::from_secs(5);

// ── the allowlist ──────────────────────────────────────────────────────────

/// `operators.json`.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Operators {
    /// The OAuth App's public client id; empty until the owner creates the
    /// App and fills it in (`--client-id` / `LOSOS_GITHUB_CLIENT_ID` override).
    #[serde(default)]
    pub github_oauth_client_id: String,
    pub operators: Vec<Operator>,
}

/// One allowed account. The numeric id is what is compared.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Operator {
    pub github_id: u64,
    pub github_login: String,
    #[serde(default)]
    pub note: String,
}

impl Operators {
    /// The list compiled into this binary.
    pub fn committed() -> Result<Self> {
        Self::parse(OPERATORS_JSON).wrap_err("operators.json compiled into this binary")
    }

    /// Parse and validate: at least one operator, every id non-zero and
    /// unique, every login non-empty. A malformed list is an error, never an
    /// empty list that admits nobody by accident (or everybody by a bug).
    pub fn parse(json: &str) -> Result<Self> {
        let list: Self = serde_json::from_str(json).into_diagnostic()?;
        if list.operators.is_empty() {
            return Err(miette!("the operator list is empty"));
        }
        let mut seen = std::collections::BTreeSet::new();
        for op in &list.operators {
            if op.github_id == 0 {
                return Err(miette!("operator {:?} has no GitHub id", op.github_login));
            }
            if op.github_login.trim().is_empty() {
                return Err(miette!("operator {} has no login", op.github_id));
            }
            if !seen.insert(op.github_id) {
                return Err(miette!("GitHub id {} is listed twice", op.github_id));
            }
        }
        Ok(list)
    }

    /// The listed operator with this GitHub id, if any.
    #[must_use]
    pub fn find(&self, github_id: u64) -> Option<&Operator> {
        self.operators.iter().find(|o| o.github_id == github_id)
    }
}

// ── GitHub ─────────────────────────────────────────────────────────────────

/// Where to talk to GitHub, and as which OAuth App.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Github {
    pub oauth_url: String,
    pub api_url: String,
    pub client_id: String,
}

impl Github {
    /// The client id from the flag, else the environment, else the committed
    /// list; an empty one is an error that says where to put it.
    pub fn resolve(
        oauth_url: &str,
        api_url: &str,
        client_id: Option<&str>,
        operators: &Operators,
    ) -> Result<Self> {
        let from_env = std::env::var("LOSOS_GITHUB_CLIENT_ID").ok();
        let client_id = client_id
            .map(str::to_string)
            .or(from_env)
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| operators.github_oauth_client_id.clone());
        if client_id.trim().is_empty() {
            return Err(miette!(
                "no GitHub OAuth App client id: create the App (Settings > Developer settings > \
                 OAuth Apps, Device Flow enabled), then put its client id into \
                 backend-registrar/operators.json, or pass --client-id / LOSOS_GITHUB_CLIENT_ID"
            ));
        }
        Ok(Self {
            oauth_url: oauth_url.trim_end_matches('/').to_string(),
            api_url: api_url.trim_end_matches('/').to_string(),
            client_id: client_id.trim().to_string(),
        })
    }
}

/// The account GitHub says a token belongs to.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct GithubUser {
    pub id: u64,
    pub login: String,
}

/// A sign-in that went through: who, the token (for `--publish`), and the
/// allowlist row that admitted them.
#[derive(Debug, Clone)]
pub struct Session {
    pub user: GithubUser,
    pub token: String,
    pub operator: Operator,
}

/// Why a sign-in was refused. A distinct type so `run` can map it to
/// [`EXIT_REFUSED`] and a caller can tell "not you" from "GitHub is down".
#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Refused {
    #[error(
        "{login} (GitHub id {id}) is not on the operator allowlist; nothing was made or signed"
    )]
    NotListed { login: String, id: u64 },
    #[error("the sign-in was denied on GitHub")]
    Denied,
    #[error("the device code expired before the sign-in was completed")]
    Expired,
}

#[derive(Debug, Deserialize)]
struct DeviceCode {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    #[serde(default)]
    interval: u64,
}

#[derive(Debug, Deserialize)]
struct TokenAnswer {
    access_token: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
    interval: Option<u64>,
}

fn http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(HTTP_TIMEOUT)
        .user_agent(concat!("losos-registrar/", env!("CARGO_PKG_VERSION")))
        .build()
        .into_diagnostic()
        .wrap_err("build HTTP client")
}

/// GitHub's OAuth device flow: ask for a code, tell the operator where to
/// type it, poll until GitHub answers with a token. `scope` is empty for a
/// plain sign-in (public profile only) and `public_repo` when the token has
/// to open the pull request too.
pub async fn device_flow(client: &reqwest::Client, gh: &Github, scope: &str) -> Result<String> {
    let code: DeviceCode = client
        .post(format!("{}/login/device/code", gh.oauth_url))
        .header("Accept", "application/json")
        .form(&[("client_id", gh.client_id.as_str()), ("scope", scope)])
        .send()
        .await
        .into_diagnostic()
        .wrap_err("ask GitHub for a device code")?
        .error_for_status()
        .into_diagnostic()
        .wrap_err("GitHub refused the device-code request (is the client id right, and Device Flow enabled on the App?)")?
        .json()
        .await
        .into_diagnostic()
        .wrap_err("read GitHub's device-code answer")?;

    eprintln!();
    eprintln!("  Sign in with GitHub: open {}", code.verification_uri);
    eprintln!("  and enter the code   {}", code.user_code);
    eprintln!();
    eprintln!(
        "  (waiting; the code is good for {} minutes)",
        code.expires_in / 60
    );

    let deadline = tokio::time::Instant::now() + Duration::from_secs(code.expires_in);
    let mut interval = Duration::from_secs(code.interval);
    loop {
        tokio::time::sleep(interval).await;
        if tokio::time::Instant::now() > deadline {
            return Err(Refused::Expired.into());
        }
        let answer: TokenAnswer = client
            .post(format!("{}/login/oauth/access_token", gh.oauth_url))
            .header("Accept", "application/json")
            .form(&[
                ("client_id", gh.client_id.as_str()),
                ("device_code", code.device_code.as_str()),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send()
            .await
            .into_diagnostic()
            .wrap_err("poll GitHub for the token")?
            .json()
            .await
            .into_diagnostic()
            .wrap_err("read GitHub's token answer")?;
        if let Some(token) = answer.access_token.filter(|t| !t.is_empty()) {
            return Ok(token);
        }
        match answer.error.as_deref() {
            Some("authorization_pending") => {}
            Some("slow_down") => {
                interval = answer
                    .interval
                    .map_or(interval + SLOW_DOWN_EXTRA, Duration::from_secs);
            }
            Some("access_denied") => return Err(Refused::Denied.into()),
            Some("expired_token") => return Err(Refused::Expired.into()),
            Some(other) => {
                return Err(miette!(
                    "GitHub ended the sign-in: {other}{}",
                    answer
                        .error_description
                        .map(|d| format!(" ({d})"))
                        .unwrap_or_default()
                ))
            }
            None => {
                return Err(miette!(
                    "GitHub answered the poll with neither a token nor an error"
                ))
            }
        }
    }
}

/// Who a token belongs to, from `GET /user`.
pub async fn whoami(client: &reqwest::Client, gh: &Github, token: &str) -> Result<GithubUser> {
    client
        .get(format!("{}/user", gh.api_url))
        .bearer_auth(token)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
        .into_diagnostic()
        .wrap_err("ask GitHub who the token belongs to")?
        .error_for_status()
        .into_diagnostic()
        .wrap_err("GitHub did not accept the token")?
        .json()
        .await
        .into_diagnostic()
        .wrap_err("read GitHub's /user answer")
}

/// The gate: a token (given, or from the device flow), the account behind
/// it, and the allowlist row for that account's id, or [`Refused`].
pub async fn sign_in(
    client: &reqwest::Client,
    gh: &Github,
    operators: &Operators,
    token: Option<String>,
    scope: &str,
) -> Result<Session> {
    let token = match token
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
    {
        Some(t) => t,
        None => device_flow(client, gh, scope).await?,
    };
    let user = whoami(client, gh, &token).await?;
    let Some(operator) = operators.find(user.id).cloned() else {
        return Err(Refused::NotListed {
            login: user.login,
            id: user.id,
        }
        .into());
    };
    if operator.github_login != user.login {
        tracing::info!(
            target: Action::Provision.target(),
            "GitHub id {} is listed as {} and now signs in as {}; the id is what counts",
            user.id, operator.github_login, user.login
        );
    }
    Ok(Session {
        user,
        token,
        operator,
    })
}

// ── the edge: ship an identity over SSH ────────────────────────────────────

/// Everything `provision edge` needs besides the root key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeSpec {
    pub name: String,
    pub url: String,
    pub ssh_target: String,
    pub days: u64,
    pub key_path: String,
    pub cert_path: String,
    pub ssh_key: Option<String>,
    pub known_hosts: Option<String>,
    /// The `ssh` binary; the tests point it at a script.
    pub ssh_command: String,
}

/// A path that is safe inside the single quotes of the remote script.
fn remote_path_ok(p: &str) -> Result<()> {
    if !p.starts_with('/') || p.contains('\'') || p.contains('\n') || p.ends_with('/') {
        return Err(miette!(
            "{p:?} is not an absolute path without quotes or newlines"
        ));
    }
    Ok(())
}

/// The script the remote shell runs. The two files arrive on stdin as two
/// lines, the key's hex and the certificate's one-line JSON, which is why
/// `read -r` (one byte at a time on a pipe, by POSIX) is enough and no
/// archive format is needed. Nothing secret is in argv; only the two
/// installation paths are interpolated, single-quoted, and checked above.
#[must_use]
pub fn remote_script(key_path: &str, cert_path: &str) -> String {
    format!(
        "set -e\n\
         if [ \"$(id -u)\" != 0 ] && command -v sudo >/dev/null 2>&1; then sudo=sudo; else sudo=; fi\n\
         d=$(mktemp -d)\n\
         umask 077\n\
         IFS= read -r key\n\
         IFS= read -r cert\n\
         printf '%s\\n' \"$key\" > \"$d/edge.key\"\n\
         printf '%s\\n' \"$cert\" > \"$d/edge.cert.json\"\n\
         $sudo install -D -m 0600 \"$d/edge.key\" '{key_path}'\n\
         $sudo install -D -m 0644 \"$d/edge.cert.json\" '{cert_path}'\n\
         rm -rf \"$d\"\n\
         if command -v systemctl >/dev/null 2>&1; then $sudo systemctl try-restart losos-registrar.service || true; fi\n\
         echo \"installed {key_path} and {cert_path}\"\n"
    )
}

/// Run one SSH session with the two lines on its stdin.
fn ship(spec: &EdgeSpec, key_hex: &str, cert_json: &str) -> Result<()> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    remote_path_ok(&spec.key_path)?;
    remote_path_ok(&spec.cert_path)?;
    let mut cmd = Command::new(&spec.ssh_command);
    cmd.arg("-o").arg("BatchMode=yes");
    if let Some(k) = &spec.ssh_key {
        cmd.arg("-i").arg(k);
    }
    if let Some(kh) = &spec.known_hosts {
        cmd.arg("-o")
            .arg(format!("UserKnownHostsFile={kh}"))
            .arg("-o")
            .arg("StrictHostKeyChecking=yes");
    }
    cmd.arg(&spec.ssh_target)
        .arg(remote_script(&spec.key_path, &spec.cert_path))
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    let mut child = cmd
        .spawn()
        .into_diagnostic()
        .wrap_err_with(|| format!("run {}", spec.ssh_command))?;
    {
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| miette!("no stdin on ssh"))?;
        stdin
            .write_all(format!("{key_hex}\n{cert_json}\n").as_bytes())
            .into_diagnostic()
            .wrap_err("write the identity to ssh's stdin")?;
    }
    let status = child.wait().into_diagnostic().wrap_err("wait for ssh")?;
    if !status.success() {
        return Err(miette!("ssh to {} exited with {status}", spec.ssh_target));
    }
    Ok(())
}

/// What `GET /identity` said: the edge is official, it answered but failed
/// a check, or it does not serve the route (yet).
#[derive(Debug)]
pub enum Probe {
    Official(Cert),
    Rejected(Rejected),
    NotServing(String),
}

/// A fresh nonce, the box's four checks, the verdict.
pub async fn probe(client: &reqwest::Client, url: &str, root_public: &str) -> Result<Probe> {
    let url = url.trim().trim_end_matches('/');
    let mut raw = [0u8; 32];
    ring::rand::SystemRandom::new()
        .fill(&mut raw)
        .map_err(|_| miette!("the system random source failed"))?;
    let nonce = identity::to_hex(&raw);
    let resp = match client
        .get(format!("{url}/identity?nonce={nonce}"))
        .timeout(Duration::from_secs(10))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return Ok(Probe::NotServing(format!("{url}: {e}"))),
    };
    if !resp.status().is_success() {
        return Ok(Probe::NotServing(format!(
            "{url}/identity answered {}",
            resp.status()
        )));
    }
    let answer: identity::Answer = resp
        .json()
        .await
        .into_diagnostic()
        .wrap_err("the /identity answer is not the document the registrar writes")?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    Ok(
        match identity::check_answer(root_public, &answer, url, &nonce, now) {
            Ok(()) => Probe::Official(answer.cert),
            Err(why) => Probe::Rejected(why),
        },
    )
}

// ── publishing the public key ──────────────────────────────────────────────

/// The new contents of `keys/official-edge-root.pub`: the comment block as
/// it is, any old key line dropped, the key appended.
#[must_use]
pub fn with_key_line(current: &str, public_key: &str) -> String {
    let mut out = String::new();
    for line in current.lines() {
        let t = line.trim();
        if t.len() == 64 && t.bytes().all(|b| b.is_ascii_hexdigit()) {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out.push_str(&public_key.to_ascii_lowercase());
    out.push('\n');
    out
}

/// Open (or update) the pull request that writes `public_key` into
/// `keys/official-edge-root.pub` of `repo`, on branch `branch` off `base`.
/// Returns the pull request's URL. The token needs `public_repo`.
pub async fn publish(
    client: &reqwest::Client,
    gh: &Github,
    token: &str,
    repo: &str,
    base: &str,
    branch: &str,
    public_key: &str,
) -> Result<String> {
    const PATH: &str = "keys/official-edge-root.pub";
    const TITLE: &str = "Official edge root key: publish the public half";
    if identity::from_hex(public_key).map(|k| k.len()) != Some(32) {
        return Err(miette!("the public key must be 64 hex characters"));
    }
    let api = format!("{}/repos/{repo}", gh.api_url);
    let req = |m: reqwest::Method, url: String| {
        client
            .request(m, url)
            .bearer_auth(token)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
    };
    let json = |r: reqwest::Response| async move {
        let status = r.status();
        let body = r.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(miette!("GitHub answered {status}: {body}"));
        }
        serde_json::from_str::<serde_json::Value>(&body)
            .into_diagnostic()
            .wrap_err("GitHub's answer is not JSON")
    };

    let base_sha = json(
        req(reqwest::Method::GET, format!("{api}/git/ref/heads/{base}"))
            .send()
            .await
            .into_diagnostic()?,
    )
    .await
    .wrap_err_with(|| format!("read the {base} branch of {repo}"))?;
    let base_sha = base_sha["object"]["sha"]
        .as_str()
        .ok_or_else(|| miette!("no sha for {base}"))?
        .to_string();

    let have_branch = req(
        reqwest::Method::GET,
        format!("{api}/git/ref/heads/{branch}"),
    )
    .send()
    .await
    .into_diagnostic()?
    .status()
    .is_success();
    if !have_branch {
        json(
            req(reqwest::Method::POST, format!("{api}/git/refs"))
                .json(
                    &serde_json::json!({ "ref": format!("refs/heads/{branch}"), "sha": base_sha }),
                )
                .send()
                .await
                .into_diagnostic()?,
        )
        .await
        .wrap_err_with(|| format!("create branch {branch}"))?;
    }

    let current = json(
        req(
            reqwest::Method::GET,
            format!("{api}/contents/{PATH}?ref={branch}"),
        )
        .send()
        .await
        .into_diagnostic()?,
    )
    .await
    .wrap_err_with(|| format!("read {PATH} on {branch}"))?;
    let file_sha = current["sha"]
        .as_str()
        .ok_or_else(|| miette!("no sha for {PATH}"))?
        .to_string();
    let text = b64_decode(current["content"].as_str().unwrap_or(""))
        .and_then(|b| String::from_utf8(b).ok())
        .ok_or_else(|| miette!("{PATH} did not come back as base64 text"))?;
    let new_text = with_key_line(&text, public_key);
    json(
        req(reqwest::Method::PUT, format!("{api}/contents/{PATH}"))
            .json(&serde_json::json!({
                "message": TITLE,
                "content": b64_encode(new_text.as_bytes()),
                "sha": file_sha,
                "branch": branch,
            }))
            .send()
            .await
            .into_diagnostic()?,
    )
    .await
    .wrap_err_with(|| format!("write {PATH} on {branch}"))?;

    let owner = repo.split('/').next().unwrap_or(repo);
    let open = json(
        req(
            reqwest::Method::GET,
            format!("{api}/pulls?head={owner}:{branch}&base={base}&state=open"),
        )
        .send()
        .await
        .into_diagnostic()?,
    )
    .await
    .wrap_err("list open pull requests")?;
    if let Some(url) = open
        .as_array()
        .and_then(|a| a.first())
        .and_then(|p| p["html_url"].as_str())
    {
        return Ok(url.to_string());
    }
    let created = json(
        req(reqwest::Method::POST, format!("{api}/pulls"))
            .json(&serde_json::json!({
                "title": TITLE,
                "head": branch,
                "base": base,
                "body": "The LosOS root public key, published from the operator's machine by \
                         `losos-registrar provision` after a GitHub sign-in against the operator \
                         allowlist. Every box built from this tree after it merges treats edges \
                         certified by this key as official (trading allowed); nothing else \
                         changes. The private half never left the operator's machine.",
            }))
            .send()
            .await
            .into_diagnostic()?,
    )
    .await
    .wrap_err("open the pull request")?;
    created["html_url"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| miette!("the pull request came back without a URL"))
}

// base64, by hand: the contents API speaks it and nothing else here does.

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

#[must_use]
pub fn b64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk.len();
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let v = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(B64[(v >> 18) as usize & 63] as char);
        out.push(B64[(v >> 12) as usize & 63] as char);
        out.push(if n > 1 {
            B64[(v >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if n > 2 {
            B64[v as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// Whitespace (GitHub wraps at 60 columns) is skipped; anything else that is
/// not base64 is `None`.
#[must_use]
pub fn b64_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    let mut acc: u32 = 0;
    let mut bits = 0u32;
    for c in s.bytes() {
        if c.is_ascii_whitespace() || c == b'=' {
            continue;
        }
        let v = B64.iter().position(|&b| b == c)? as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

// ── the CLI ────────────────────────────────────────────────────────────────

fn root_public_of(root_key: Option<&str>, root_public: Option<&str>) -> Result<String> {
    match (root_key, root_public) {
        (Some(file), _) => {
            let hex = std::fs::read_to_string(file)
                .into_diagnostic()
                .wrap_err_with(|| format!("reading the root key {file}"))?;
            let pair = identity::load_key(&hex)?;
            Ok(identity::to_hex(
                ring::signature::KeyPair::public_key(&pair).as_ref(),
            ))
        }
        (None, Some(hex)) => {
            if identity::from_hex(hex.trim()).map(|k| k.len()) != Some(32) {
                return Err(miette!("--root-public must be 64 hex characters"));
            }
            Ok(hex.trim().to_ascii_lowercase())
        }
        (None, None) => Err(miette!("need --root-key FILE or --root-public HEX")),
    }
}

fn report(session: &Session) {
    eprintln!(
        "  signed in as {} (GitHub id {}), listed as {}",
        session.user.login,
        session.user.id,
        if session.operator.note.is_empty() {
            session.operator.github_login.clone()
        } else {
            session.operator.note.clone()
        }
    );
}

/// `losos-registrar provision …`. Stdout carries the one thing a script
/// would want (the public key, the PR URL, the verdict); everything said to
/// the operator goes to stderr.
pub async fn run(opts: ProvisionOpts) -> Result<ExitCode> {
    let client = http_client()?;
    let operators = Operators::committed()?;
    let token_env = std::env::var("LOSOS_GITHUB_TOKEN").ok();

    let gate = |gh: &ProvisionGithub, scope: &'static str| {
        let client = client.clone();
        let operators = operators.clone();
        let token_env = token_env.clone();
        let gh = Github::resolve(
            &gh.oauth_url,
            &gh.api_url,
            gh.client_id.as_deref(),
            &operators,
        );
        async move {
            let gh = gh?;
            let session = sign_in(&client, &gh, &operators, token_env, scope).await?;
            report(&session);
            Ok::<(Github, Session), miette::Report>((gh, session))
        }
    };

    let outcome: Result<ExitCode> = async {
        match opts {
            ProvisionOpts::Whoami { github } => {
                let (_, session) = gate(&github, "").await?;
                println!("{} {}", session.user.login, session.user.id);
                Ok(ExitCode::SUCCESS)
            }
            ProvisionOpts::RootKeygen {
                github,
                out,
                publish: do_publish,
                repo,
                base,
            } => {
                let path = Path::new(&out);
                if path.exists() {
                    return Err(miette!("{out} exists; not overwriting a root key"));
                }
                let (gh, session) =
                    gate(&github, if do_publish { "public_repo" } else { "" }).await?;
                let (pkcs8_hex, public_hex) = identity::keygen()?;
                identity::write_private(path, &pkcs8_hex)?;
                eprintln!("  root key written to {out} (0600); keep it offline");
                println!("{public_hex}");
                if do_publish {
                    let url = publish(
                        &client,
                        &gh,
                        &session.token,
                        &repo,
                        &base,
                        "official-edge-root-key",
                        &public_hex,
                    )
                    .await?;
                    eprintln!("  pull request: {url}");
                }
                Ok(ExitCode::SUCCESS)
            }
            ProvisionOpts::Publish {
                github,
                root_key,
                root_public,
                repo,
                base,
            } => {
                let public = root_public_of(root_key.as_deref(), root_public.as_deref())?;
                let (gh, session) = gate(&github, "public_repo").await?;
                let url = publish(
                    &client,
                    &gh,
                    &session.token,
                    &repo,
                    &base,
                    "official-edge-root-key",
                    &public,
                )
                .await?;
                println!("{url}");
                Ok(ExitCode::SUCCESS)
            }
            ProvisionOpts::Edge {
                github,
                root_key,
                spec,
            } => {
                let hex = std::fs::read_to_string(&root_key)
                    .into_diagnostic()
                    .wrap_err_with(|| format!("reading the root key {root_key}"))?;
                let root = identity::load_key(&hex)?;
                let root_public = identity::to_hex(ring::signature::KeyPair::public_key(&root).as_ref());
                let (_, _session) = gate(&github, "").await?;

                let (edge_pkcs8, edge_public) = identity::keygen()?;
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(|_| miette!("the clock is before 1970"))?
                    .as_secs();
                let cert = identity::issue(
                    &root,
                    &spec.name,
                    &spec.url,
                    &edge_public,
                    now + spec.days * 86_400,
                )?;
                let cert_json = serde_json::to_string(&cert).into_diagnostic()?;
                eprintln!("  edge public key {edge_public}");
                eprintln!(
                    "  certificate for {:?} at {} until {} ({} days)",
                    cert.name, cert.url, cert.not_after, spec.days
                );
                ship(&spec, &edge_pkcs8, &cert_json)?;
                drop(edge_pkcs8);

                match probe(&client, &spec.url, &root_public).await? {
                    Probe::Official(c) => {
                        println!("official: {} at {}", c.name, c.url);
                        Ok(ExitCode::SUCCESS)
                    }
                    Probe::Rejected(why) => Err(miette!(
                        "the edge answered /identity but failed a check: {why}"
                    )),
                    Probe::NotServing(what) => {
                        eprintln!(
                            "  files are in place; the edge is not official yet ({what}).\n  \
                             Set losos.edge.identity.keyFile = \"{}\" and certFile = \"{}\" on it, \
                             rebuild, then run: losos-registrar provision verify --url {} --root-key {root_key}",
                            spec.key_path, spec.cert_path, spec.url
                        );
                        Ok(ExitCode::from(EXIT_NOT_YET))
                    }
                }
            }
            ProvisionOpts::Verify {
                url,
                root_key,
                root_public,
            } => {
                let root_public = root_public_of(root_key.as_deref(), root_public.as_deref())?;
                match probe(&client, &url, &root_public).await? {
                    Probe::Official(c) => {
                        println!("official: {} at {}", c.name, c.url);
                        Ok(ExitCode::SUCCESS)
                    }
                    Probe::Rejected(why) => Err(miette!("not an official edge: {why}")),
                    Probe::NotServing(what) => Err(miette!(
                        "the edge does not answer /identity ({what}); is losos.edge.identity.* set and the registrar restarted?"
                    )),
                }
            }
        }
    }
    .await;

    match outcome {
        Ok(code) => Ok(code),
        Err(e) if e.downcast_ref::<Refused>().is_some() => {
            tracing::error!(target: Action::Provision.target(), "refused: {e}");
            Ok(ExitCode::from(EXIT_REFUSED))
        }
        Err(e) => Err(e),
    }
}

/// The GitHub endpoints and client id as the command line gave them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvisionGithub {
    pub oauth_url: String,
    pub api_url: String,
    pub client_id: Option<String>,
}

impl Default for ProvisionGithub {
    fn default() -> Self {
        Self {
            oauth_url: GITHUB_OAUTH_URL.to_string(),
            api_url: GITHUB_API_URL.to_string(),
            client_id: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_committed_list_parses_and_names_the_owner_by_id() {
        let list = Operators::committed().expect("operators.json");
        let matus = list.find(330_471_626).expect("Matus is listed");
        assert_eq!(matus.github_login, "dasmatus");
        assert!(list.find(1).is_none());
    }

    #[test]
    fn a_malformed_list_is_refused_rather_than_admitting_nobody() {
        assert!(Operators::parse(r#"{"operators": []}"#).is_err());
        assert!(
            Operators::parse(r#"{"operators": [{"github_id": 0, "github_login": "x"}]}"#).is_err()
        );
        assert!(Operators::parse(
            r#"{"operators": [{"github_id": 7, "github_login": "a"}, {"github_id": 7, "github_login": "b"}]}"#
        )
        .is_err());
        assert!(
            Operators::parse(r#"{"operators": [{"github_id": 7, "github_login": " "}]}"#).is_err()
        );
        let ok =
            Operators::parse(r#"{"operators": [{"github_id": 7, "github_login": "a"}]}"#).unwrap();
        assert_eq!(ok.github_oauth_client_id, "");
    }

    #[test]
    fn base64_round_trips_and_skips_githubs_line_wrapping() {
        for input in [&b""[..], b"f", b"fo", b"foo", b"foob", b"fooba", b"foobar"] {
            let enc = b64_encode(input);
            assert_eq!(b64_decode(&enc).as_deref(), Some(input), "{enc}");
        }
        assert_eq!(b64_encode(b"foobar"), "Zm9vYmFy");
        assert_eq!(b64_decode("Zm9v\nYmFy\n").as_deref(), Some(&b"foobar"[..]));
        assert_eq!(b64_decode("Zm9v!"), None);
    }

    #[test]
    fn the_key_line_replaces_an_old_key_and_keeps_the_comments() {
        let old = "# comment\n#\n\
                   aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n";
        let new = "B".repeat(64);
        let out = with_key_line(old, &new);
        assert_eq!(out, format!("# comment\n#\n{}\n", "b".repeat(64)));
        assert_eq!(
            with_key_line("# only\n", &new),
            format!("# only\n{}\n", "b".repeat(64))
        );
    }

    #[test]
    fn remote_paths_are_checked_before_they_are_quoted() {
        assert!(remote_path_ok("/var/secrets/x.key").is_ok());
        assert!(remote_path_ok("relative").is_err());
        assert!(remote_path_ok("/a'b").is_err());
        assert!(remote_path_ok("/a\nb").is_err());
        assert!(remote_path_ok("/dir/").is_err());
        let s = remote_script("/k", "/c");
        assert!(s.contains("install -D -m 0600 \"$d/edge.key\" '/k'"));
        assert!(s.contains("IFS= read -r key"));
    }
}
