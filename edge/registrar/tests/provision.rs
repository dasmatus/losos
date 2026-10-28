//! The operator gate, against a fake GitHub on loopback.
//!
//! What is real: the device-flow client and the allowlist check. What is
//! fake: GitHub, which here is a small axum app that hands out a device
//! code, answers the poll with `authorization_pending` and `slow_down`
//! before the token, and maps tokens to accounts. The allowlist under test
//! is the committed one, so this also pins that Matus's id is on it and
//! that a stranger's is not.

mod common;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Form, Json, Router};
use losos_registrar::opts::{parse, Mode, ProvisionOpts};
use losos_registrar::provision::{self, Github, Operators, ProvisionGithub, Refused, Session};
use serde::Deserialize;

const MATUS_ID: u64 = 330_471_626;

/// What the fake GitHub saw and does.
#[derive(Default)]
struct Fake {
    /// Polls to answer `authorization_pending` before the token.
    pending_polls: usize,
    /// Whether to answer one `slow_down` after the pending ones.
    slow_down_once: bool,
    /// The token the flow ends in; `None` answers `access_denied`.
    token: Option<&'static str>,
    polls: AtomicUsize,
    /// `GET /user` calls.
    user_calls: AtomicUsize,
    scopes_asked: Mutex<Vec<String>>,
}

#[derive(Deserialize)]
struct DeviceForm {
    client_id: String,
    #[serde(default)]
    scope: String,
}

#[derive(Deserialize)]
struct PollForm {
    client_id: String,
    device_code: String,
    grant_type: String,
}

fn user_of(token: &str) -> Option<(u64, &'static str)> {
    match token {
        "gho_matus" => Some((MATUS_ID, "dasmatus")),
        // The login the same account had before 2026-09-30.
        "gho_renamed" => Some((MATUS_ID, "dichhead")),
        "gho_stranger" => Some((42, "stranger")),
        _ => None,
    }
}

async fn start_fake(fake: Fake) -> (String, Arc<Fake>) {
    let fake = Arc::new(fake);
    let app = Router::new()
        .route(
            "/login/device/code",
            post(
                |State(f): State<Arc<Fake>>, Form(form): Form<DeviceForm>| async move {
                    assert_eq!(form.client_id, "Iv1.test");
                    f.scopes_asked.lock().unwrap().push(form.scope);
                    Json(serde_json::json!({
                        "device_code": "dc-1",
                        "user_code": "ABCD-1234",
                        "verification_uri": "https://github.com/login/device",
                        "expires_in": 900,
                        "interval": 0,
                    }))
                },
            ),
        )
        .route(
            "/login/oauth/access_token",
            post(
                |State(f): State<Arc<Fake>>, Form(form): Form<PollForm>| async move {
                    assert_eq!(form.client_id, "Iv1.test");
                    assert_eq!(form.device_code, "dc-1");
                    assert_eq!(form.grant_type, "urn:ietf:params:oauth:grant-type:device_code");
                    let n = f.polls.fetch_add(1, Ordering::SeqCst);
                    let body = if n < f.pending_polls {
                        serde_json::json!({"error": "authorization_pending"})
                    } else if f.slow_down_once && n == f.pending_polls {
                        serde_json::json!({"error": "slow_down", "interval": 0})
                    } else {
                        match f.token {
                            Some(t) => serde_json::json!({"access_token": t, "token_type": "bearer", "scope": ""}),
                            None => serde_json::json!({"error": "access_denied"}),
                        }
                    };
                    Json(body)
                },
            ),
        )
        .route(
            "/user",
            get(|State(f): State<Arc<Fake>>, headers: HeaderMap| async move {
                f.user_calls.fetch_add(1, Ordering::SeqCst);
                let auth = headers
                    .get("authorization")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("");
                match auth.strip_prefix("Bearer ").and_then(user_of) {
                    Some((id, login)) => {
                        (StatusCode::OK, Json(serde_json::json!({"id": id, "login": login})))
                            .into_response()
                    }
                    None => StatusCode::UNAUTHORIZED.into_response(),
                }
            }),
        )
        .with_state(fake.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    (base, fake)
}

fn github(base: &str) -> Github {
    Github {
        oauth_url: base.to_string(),
        api_url: base.to_string(),
        client_id: "Iv1.test".to_string(),
    }
}

fn client() -> reqwest::Client {
    reqwest::Client::builder().build().unwrap()
}

async fn sign_in(base: &str, token: Option<&str>, scope: &str) -> miette::Result<Session> {
    provision::sign_in(
        &client(),
        &github(base),
        &Operators::committed().unwrap(),
        token.map(str::to_string),
        scope,
    )
    .await
}

#[tokio::test]
async fn the_device_flow_signs_a_listed_operator_in() {
    let (base, fake) = start_fake(Fake {
        pending_polls: 2,
        slow_down_once: true,
        token: Some("gho_matus"),
        ..Fake::default()
    })
    .await;
    let session = sign_in(&base, None, "").await.expect("Matus is listed");
    assert_eq!(session.user.login, "dasmatus");
    assert_eq!(session.user.id, MATUS_ID);
    assert_eq!(session.operator.github_login, "dasmatus");
    assert_eq!(session.token, "gho_matus");
    // Two pending answers, one slow_down, then the token: four polls.
    assert_eq!(fake.polls.load(Ordering::SeqCst), 4);
    // A plain sign-in asks GitHub for nothing beyond the public profile.
    assert_eq!(*fake.scopes_asked.lock().unwrap(), vec![String::new()]);
}

#[tokio::test]
async fn a_stranger_is_refused_by_id_and_the_refusal_is_typed() {
    let (base, _) = start_fake(Fake {
        token: Some("gho_stranger"),
        ..Fake::default()
    })
    .await;
    let err = sign_in(&base, None, "")
        .await
        .expect_err("42 is not listed");
    match err.downcast_ref::<Refused>() {
        Some(Refused::NotListed { login, id }) => {
            assert_eq!(login, "stranger");
            assert_eq!(*id, 42);
        }
        other => panic!("expected NotListed, got {other:?}: {err:?}"),
    }
}

#[tokio::test]
async fn a_renamed_login_still_passes_because_the_id_is_the_key() {
    let (base, _) = start_fake(Fake {
        token: Some("gho_renamed"),
        ..Fake::default()
    })
    .await;
    let session = sign_in(&base, None, "").await.expect("same id, new login");
    assert_eq!(session.user.login, "dichhead");
    assert_eq!(session.operator.github_login, "dasmatus");
}

#[tokio::test]
async fn a_denied_sign_in_is_refused() {
    let (base, _) = start_fake(Fake {
        token: None,
        ..Fake::default()
    })
    .await;
    let err = sign_in(&base, None, "").await.expect_err("denied");
    assert!(matches!(
        err.downcast_ref::<Refused>(),
        Some(Refused::Denied)
    ));
}

#[tokio::test]
async fn a_given_token_skips_the_browser_but_not_the_list() {
    let (base, fake) = start_fake(Fake::default()).await;
    let session = sign_in(&base, Some("gho_matus"), "")
        .await
        .expect("token for Matus");
    assert_eq!(session.user.id, MATUS_ID);
    assert_eq!(fake.polls.load(Ordering::SeqCst), 0, "no device flow ran");
    let err = sign_in(&base, Some("gho_stranger"), "")
        .await
        .expect_err("token for 42");
    assert!(matches!(
        err.downcast_ref::<Refused>(),
        Some(Refused::NotListed { .. })
    ));
    let err = sign_in(&base, Some("gho_nobody"), "")
        .await
        .expect_err("unknown token");
    assert!(
        err.downcast_ref::<Refused>().is_none(),
        "a bad token is a failure, not a refusal"
    );
}

#[tokio::test]
async fn a_refused_sign_in_exits_3() {
    let (base, _) = start_fake(Fake {
        token: Some("gho_stranger"),
        ..Fake::default()
    })
    .await;
    let args: Vec<String> = [
        "provision",
        "whoami",
        "--github-oauth-url",
        &base,
        "--github-api-url",
        &base,
        "--client-id",
        "Iv1.test",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let Mode::Provision(opts) = parse(args).unwrap() else {
        panic!("parse")
    };
    assert_eq!(
        provision::run(opts).await.unwrap(),
        std::process::ExitCode::from(provision::EXIT_REFUSED)
    );
}

#[test]
fn the_binary_has_the_committed_client_id_or_says_where_it_goes() {
    // Until the owner fills operators.json, resolve() must say so rather
    // than start a device flow against an empty client id.
    let ops = Operators::committed().unwrap();
    let r = Github::resolve(
        provision::GITHUB_OAUTH_URL,
        provision::GITHUB_API_URL,
        None,
        &ops,
    );
    if ops.github_oauth_client_id.is_empty() {
        let err = r.expect_err("no client id yet");
        assert!(format!("{err}").contains("operators.json"), "{err}");
    } else {
        assert_eq!(r.unwrap().client_id, ops.github_oauth_client_id);
    }
    let given = Github::resolve(
        "https://github.com/",
        "https://api.github.com/",
        Some("Iv1.x"),
        &ops,
    )
    .unwrap();
    assert_eq!(given.oauth_url, "https://github.com");
    assert_eq!(given.client_id, "Iv1.x");
    let _ = ProvisionGithub::default();
}

#[test]
fn provision_parsing_requires_what_each_verb_needs() {
    let p = |a: &[&str]| parse(a.iter().map(|s| s.to_string()).collect());
    assert!(matches!(
        p(&["provision", "whoami"]),
        Ok(Mode::Provision(ProvisionOpts::Whoami { .. }))
    ));
    assert!(p(&["provision", "token", "list"]).is_err(), "no --edge");
    assert!(matches!(
        p(&["provision", "token", "list", "--edge", "https://e/"]),
        Ok(Mode::Provision(ProvisionOpts::Token { ref edge, .. })) if edge == "https://e"
    ));
    // The certificate ceremony is gone with the root key.
    for gone in ["root-keygen", "publish", "edge", "verify"] {
        assert!(p(&["provision", gone]).is_err(), "{gone} still parses");
    }
    assert!(p(&["provision", "nonsense"]).is_err());
}
