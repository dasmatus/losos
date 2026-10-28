//! The operator gate, run from the developer's own machine.
//!
//! Every operator action that touches an edge (minting an edge access token,
//! today) first signs the operator in with GitHub's OAuth *device flow* and
//! refuses unless the signed-in account is on the allowlist compiled into the
//! binary (`operators.json` beside `Cargo.toml`). The edge checks the same
//! token against the same list (`crate::server::github_operator`), so the
//! gate holds at both ends.
//!
//! The device flow was chosen because it needs no secret anywhere: the tool
//! carries only the OAuth App's public client id, prints a one-time code, the
//! operator types it into github.com in any browser, and GitHub hands back a
//! token for that account. The tool then asks `GET /user` who that is and
//! compares the **numeric id** (logins can be renamed; ids cannot) with the
//! list. No token is stored: each gated command signs in afresh, or takes a
//! token from `LOSOS_GITHUB_TOKEN` (for example `gh auth token`), which is
//! checked the same way.

use std::process::ExitCode;
use std::time::Duration;

use miette::{miette, Context, IntoDiagnostic, Result};
use serde::Deserialize;

use crate::action::Action;
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
                 edge/registrar/operators.json, or pass --client-id / LOSOS_GITHUB_CLIENT_ID"
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
    #[error("the edge refused the push: {0}")]
    EdgeRefused(String),
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

// ── the CLI ────────────────────────────────────────────────────────────────

/// `provision token`: one call to the edge's `/operator/tokens*` with the
/// sign-in's GitHub token. A new token goes to stdout and nowhere else.
async fn token_verb(
    client: &reqwest::Client,
    edge: &str,
    github_token: &str,
    verb: crate::opts::TokenVerb,
) -> Result<ExitCode> {
    use crate::opts::TokenVerb;
    let (path, body) = match &verb {
        TokenVerb::Create {
            name,
            fingerprint,
            scope,
            expires_in_days,
        } => (
            "/operator/tokens",
            serde_json::json!({
                "name": name,
                "fingerprint": fingerprint.replace(' ', ""),
                "scope": scope,
                "expiresInDays": expires_in_days,
            }),
        ),
        TokenVerb::List => ("/operator/tokens/list", serde_json::json!({})),
        TokenVerb::Revoke { id } => ("/operator/tokens/revoke", serde_json::json!({ "id": id })),
    };
    let resp = client
        .post(format!("{edge}{path}"))
        .bearer_auth(github_token)
        .json(&body)
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .into_diagnostic()
        .wrap_err_with(|| format!("reach {edge}"))?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if status == reqwest::StatusCode::FORBIDDEN {
        return Err(Refused::EdgeRefused(text.trim().to_string()).into());
    }
    if !status.is_success() {
        let why = serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|v| v["error"].as_str().map(str::to_string))
            .unwrap_or(text);
        return Err(miette!("{edge}{path} answered {status}: {}", why.trim()));
    }
    let answer: serde_json::Value = serde_json::from_str(&text)
        .into_diagnostic()
        .wrap_err("the edge's answer is not JSON")?;
    match verb {
        TokenVerb::Create { .. } => {
            eprintln!(
                "  token {} made; it is shown once. Give it to `losos login {edge}`.",
                answer["record"]["id"].as_str().unwrap_or("?")
            );
            println!("{}", answer["token"].as_str().unwrap_or_default());
        }
        TokenVerb::List => {
            for t in answer["tokens"].as_array().into_iter().flatten() {
                println!(
                    "{}\t{}\t{}\t{}",
                    t["id"].as_str().unwrap_or("?"),
                    t["scope"].as_str().unwrap_or("?"),
                    t["fingerprint"].as_str().unwrap_or("?"),
                    t["name"].as_str().unwrap_or("?"),
                );
            }
        }
        TokenVerb::Revoke { id } => eprintln!("  token {id} revoked"),
    }
    Ok(ExitCode::SUCCESS)
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
            ProvisionOpts::Token { github, edge, verb } => {
                let (_, session) = gate(&github, "").await?;
                token_verb(&client, &edge, &session.token, verb).await
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
}
