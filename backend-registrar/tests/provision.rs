//! The key ceremony end to end, against a fake GitHub on loopback.
//!
//! What is real: the device-flow client, the allowlist check, the edge key
//! and certificate, the SSH shipment (an `ssh` stand-in that executes the
//! remote script locally, so the script's `read -r` framing is what is
//! tested), the registrar that loads what landed, and the four checks over
//! `GET /identity`. What is fake: GitHub, which here is a small axum app
//! that hands out a device code, answers the poll with `authorization_pending`
//! and `slow_down` before the token, and maps tokens to accounts. The allowlist
//! under test is the committed one, so this also pins that Matus's id is on it
//! and that a stranger's is not.

mod common;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use axum::extract::{Query, State};
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
    scopes_asked: Mutex<Vec<String>>,
    /// The contents API: the file on each branch, and the open PRs.
    file: Mutex<String>,
    branches: Mutex<Vec<String>>,
    pulls: Mutex<Vec<serde_json::Value>>,
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
        "gho_matus" => Some((MATUS_ID, "dichhead")),
        "gho_renamed" => Some((MATUS_ID, "matus-renamed")),
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
            get(|headers: HeaderMap| async move {
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
        // The contents API, enough for `publish`.
        .route(
            "/repos/{owner}/{repo}/git/ref/heads/{branch}",
            get(
                |State(f): State<Arc<Fake>>,
                 axum::extract::Path((_, _, branch)): axum::extract::Path<(String, String, String)>| async move {
                    if branch == "main" || f.branches.lock().unwrap().contains(&branch) {
                        (StatusCode::OK, Json(serde_json::json!({"object": {"sha": "abc123"}})))
                            .into_response()
                    } else {
                        StatusCode::NOT_FOUND.into_response()
                    }
                },
            ),
        )
        .route(
            "/repos/{owner}/{repo}/git/refs",
            post(
                |State(f): State<Arc<Fake>>, Json(body): Json<serde_json::Value>| async move {
                    let r = body["ref"].as_str().unwrap().trim_start_matches("refs/heads/");
                    f.branches.lock().unwrap().push(r.to_string());
                    (StatusCode::CREATED, Json(serde_json::json!({"ref": body["ref"]})))
                },
            ),
        )
        .route(
            "/repos/{owner}/{repo}/contents/{*path}",
            get(|State(f): State<Arc<Fake>>, Query(q): Query<std::collections::HashMap<String, String>>| async move {
                assert_eq!(q.get("ref").map(String::as_str), Some("official-edge-root-key"));
                let text = f.file.lock().unwrap().clone();
                Json(serde_json::json!({
                    "sha": "filesha",
                    "content": provision::b64_encode(text.as_bytes()),
                    "encoding": "base64",
                }))
            })
            .put(|State(f): State<Arc<Fake>>, Json(body): Json<serde_json::Value>| async move {
                assert_eq!(body["sha"], "filesha");
                assert_eq!(body["branch"], "official-edge-root-key");
                let bytes = provision::b64_decode(body["content"].as_str().unwrap()).unwrap();
                *f.file.lock().unwrap() = String::from_utf8(bytes).unwrap();
                Json(serde_json::json!({"content": {"sha": "newsha"}}))
            }),
        )
        .route(
            "/repos/{owner}/{repo}/pulls",
            get(|State(f): State<Arc<Fake>>| async move { Json(f.pulls.lock().unwrap().clone()) })
                .post(|State(f): State<Arc<Fake>>, Json(body): Json<serde_json::Value>| async move {
                    let pr = serde_json::json!({
                        "html_url": "https://github.com/dasmatus/losos/pull/999",
                        "title": body["title"], "head": body["head"], "base": body["base"],
                    });
                    f.pulls.lock().unwrap().push(pr.clone());
                    (StatusCode::CREATED, Json(pr))
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
    assert_eq!(session.user.login, "dichhead");
    assert_eq!(session.user.id, MATUS_ID);
    assert_eq!(session.operator.github_login, "dichhead");
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
    assert_eq!(session.user.login, "matus-renamed");
    assert_eq!(session.operator.github_login, "dichhead");
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

/// A stand-in for `ssh`: records its arguments, then runs the remote script
/// (its last argument) locally with stdin passed through, under a `sudo`
/// shim so the script's privilege step is exercised without privileges.
fn fake_ssh(dir: &common::TempDir) -> String {
    use std::os::unix::fs::PermissionsExt;
    let bin = dir.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::write(bin.join("sudo"), "#!/bin/sh\nexec \"$@\"\n").unwrap();
    std::fs::set_permissions(bin.join("sudo"), std::fs::Permissions::from_mode(0o755)).unwrap();
    let script = dir.join("ssh");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > {args}\nfor last; do :; done\nexport PATH={bin}:$PATH\nexec sh -c \"$last\"\n",
            args = dir.path_str("ssh.args"),
            bin = bin.display(),
        ),
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    script.to_string_lossy().into_owned()
}

#[tokio::test]
async fn provision_edge_ships_an_identity_the_registrar_serves_as_official() {
    let (base, _) = start_fake(Fake {
        token: Some("gho_matus"),
        ..Fake::default()
    })
    .await;
    let dir = common::TempDir::new("provision-edge");
    let ssh = fake_ssh(&dir);
    let key_path = dir.path_str("installed/edge.key");
    let cert_path = dir.path_str("installed/edge.cert.json");
    let gh_flags = [
        "--github-oauth-url",
        &base,
        "--github-api-url",
        &base,
        "--client-id",
        "Iv1.test",
    ];
    let run = |a: &[&str]| {
        let args: Vec<String> = a
            .iter()
            .chain(gh_flags.iter())
            .map(|s| s.to_string())
            .collect();
        let Mode::Provision(opts) = parse(args).unwrap() else {
            panic!("parse")
        };
        provision::run(opts)
    };

    // The root key, made the way the operator makes it.
    let root_out = dir.path_str("root.key");
    assert_eq!(
        run(&["provision", "root-keygen", "--out", &root_out])
            .await
            .unwrap(),
        std::process::ExitCode::SUCCESS
    );
    assert!(
        format!(
            "{:?}",
            run(&["provision", "root-keygen", "--out", &root_out]).await
        )
        .contains("not overwriting"),
        "a second keygen over the same file must refuse"
    );

    // The edge's URL is chosen before anything listens there: the first run
    // installs the files and reports "not official yet" (exit 2).
    let free = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = free.local_addr().unwrap();
    drop(free);
    let url = format!("http://{addr}");
    let code = run(&[
        "provision",
        "edge",
        "--name",
        "LosOS edge test",
        "--url",
        &url,
        "--ssh",
        "root@edge.test",
        "--root-key",
        &root_out,
        "--days",
        "30",
        "--key-path",
        &key_path,
        "--cert-path",
        &cert_path,
        "--ssh-key",
        "/nonexistent/deploy",
        "--known-hosts",
        "/nonexistent/known_hosts",
        "--ssh-command",
        &ssh,
    ])
    .await
    .unwrap();
    assert_eq!(code, std::process::ExitCode::from(provision::EXIT_NOT_YET));

    // ssh was called the careful way: batch mode, the deploy key, a pinned
    // known_hosts, the target, and nothing secret in argv.
    let argv = std::fs::read_to_string(dir.join("ssh.args")).unwrap();
    for expected in [
        "BatchMode=yes",
        "/nonexistent/deploy",
        "UserKnownHostsFile=/nonexistent/known_hosts",
        "StrictHostKeyChecking=yes",
        "root@edge.test",
    ] {
        assert!(argv.contains(expected), "ssh argv lacks {expected}: {argv}");
    }
    let key_hex = std::fs::read_to_string(&key_path).unwrap();
    assert!(!argv.contains(key_hex.trim()), "the key travelled in argv");
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        std::fs::metadata(&key_path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let cert: losos_registrar::identity::Cert =
        serde_json::from_str(&std::fs::read_to_string(&cert_path).unwrap()).unwrap();
    assert_eq!(cert.name, "LosOS edge test");
    assert_eq!(cert.url, url);
    assert!(
        losos_registrar::identity::load_key(&key_hex).is_ok(),
        "the shipped key loads"
    );

    // Now the edge's configuration names the files and the registrar is
    // (re)started: the four checks pass, from `verify` and from a renewal
    // run of `edge` alike.
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let edge = common::Edge::start_with_identity_on(
        "provision-edge-reg",
        &[],
        &key_path,
        &cert_path,
        Some(listener),
    )
    .await;
    assert_eq!(edge.base, url);
    assert_eq!(
        run(&[
            "provision",
            "verify",
            "--url",
            &url,
            "--root-key",
            &root_out
        ])
        .await
        .unwrap(),
        std::process::ExitCode::SUCCESS
    );
    let code = run(&[
        "provision",
        "edge",
        "--name",
        "LosOS edge test",
        "--url",
        &url,
        "--ssh",
        "root@edge.test",
        "--root-key",
        &root_out,
        "--key-path",
        &key_path,
        "--cert-path",
        &cert_path,
        "--ssh-command",
        &ssh,
    ])
    .await
    .unwrap();
    assert_eq!(
        code,
        std::process::ExitCode::SUCCESS,
        "renewal: the live edge is still official"
    );
    let renewed = std::fs::read_to_string(&key_path).unwrap();
    assert_ne!(renewed, key_hex, "renewal issued a fresh key");

    // Someone else's root cannot verify it.
    let (_, other_public) = losos_registrar::identity::keygen().unwrap();
    let err = run(&[
        "provision",
        "verify",
        "--url",
        &url,
        "--root-public",
        &other_public,
    ])
    .await
    .expect_err("another root's view");
    assert!(
        format!("{err:?}").contains("not signed by the LosOS root key"),
        "{err:?}"
    );
    edge.shutdown().await;
}

#[tokio::test]
async fn provision_verify_runs_the_four_checks_against_a_live_edge() {
    let dir = common::TempDir::new("provision-verify");
    let (root_pkcs8, root_public) = losos_registrar::identity::keygen().unwrap();
    let root = losos_registrar::identity::load_key(&root_pkcs8).unwrap();
    let (edge_pkcs8, edge_public) = losos_registrar::identity::keygen().unwrap();
    // The registrar's URL is only known once it listens, and the certificate
    // names it: pick the port first, then issue, then start on that port.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let cert =
        losos_registrar::identity::issue(&root, "LosOS edge test", &url, &edge_public, now + 3600)
            .unwrap();
    let key_path = dir.path_str("edge.key");
    let cert_path = dir.path_str("edge.cert.json");
    std::fs::write(&key_path, format!("{edge_pkcs8}\n")).unwrap();
    std::fs::write(&cert_path, serde_json::to_string(&cert).unwrap()).unwrap();
    let edge = common::Edge::start_with_identity_on(
        "provision-verify-reg",
        &[],
        &key_path,
        &cert_path,
        Some(listener),
    )
    .await;
    assert_eq!(edge.base, url);

    let root_file = dir.path_str("root.key");
    std::fs::write(&root_file, format!("{root_pkcs8}\n")).unwrap();
    let args: Vec<String> = [
        "provision",
        "verify",
        "--url",
        &url,
        "--root-key",
        &root_file,
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let Mode::Provision(opts) = parse(args).unwrap() else {
        panic!("parse")
    };
    assert_eq!(
        provision::run(opts).await.unwrap(),
        std::process::ExitCode::SUCCESS
    );

    // Another root: the certificate is not its.
    let (_, other_public) = losos_registrar::identity::keygen().unwrap();
    let args: Vec<String> = [
        "provision",
        "verify",
        "--url",
        &url,
        "--root-public",
        &other_public,
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let Mode::Provision(opts) = parse(args).unwrap() else {
        panic!("parse")
    };
    let err = provision::run(opts).await.expect_err("another root's view");
    assert!(
        format!("{err:?}").contains("not signed by the LosOS root key"),
        "{err:?}"
    );
    let _ = root_public;
    edge.shutdown().await;
}

#[tokio::test]
async fn publish_writes_the_key_line_and_opens_one_pull_request() {
    let (base, fake) = start_fake(Fake {
        token: Some("gho_matus"),
        ..Fake::default()
    })
    .await;
    *fake.file.lock().unwrap() = "# The LosOS root public key.\n#\n# comment\n".to_string();
    let public = "c".repeat(64);
    let args: Vec<String> = [
        "provision",
        "publish",
        "--root-public",
        &public,
        "--repo",
        "dasmatus/losos",
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
        std::process::ExitCode::SUCCESS
    );
    assert_eq!(
        *fake.scopes_asked.lock().unwrap(),
        vec!["public_repo".to_string()],
        "publishing asks for public_repo, no more"
    );
    assert_eq!(
        *fake.branches.lock().unwrap(),
        vec!["official-edge-root-key".to_string()]
    );
    assert_eq!(
        *fake.file.lock().unwrap(),
        format!("# The LosOS root public key.\n#\n# comment\n{public}\n")
    );
    assert_eq!(fake.pulls.lock().unwrap().len(), 1);

    // A second publish with a new key updates the branch and reuses the PR.
    let public2 = "d".repeat(64);
    let args: Vec<String> = [
        "provision",
        "publish",
        "--root-public",
        &public2,
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
        std::process::ExitCode::SUCCESS
    );
    assert_eq!(
        *fake.branches.lock().unwrap(),
        vec!["official-edge-root-key".to_string()]
    );
    assert_eq!(
        *fake.file.lock().unwrap(),
        format!("# The LosOS root public key.\n#\n# comment\n{public2}\n")
    );
    assert_eq!(
        fake.pulls.lock().unwrap().len(),
        1,
        "one PR, updated, not a second"
    );
}

#[tokio::test]
async fn a_refused_sign_in_exits_3_and_makes_no_key() {
    let (base, _) = start_fake(Fake {
        token: Some("gho_stranger"),
        ..Fake::default()
    })
    .await;
    let dir = common::TempDir::new("provision-refused");
    let out = dir.path_str("root.key");
    let args: Vec<String> = [
        "provision",
        "root-keygen",
        "--out",
        &out,
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
    assert!(
        !std::path::Path::new(&out).exists(),
        "a refused operator got a key"
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
    assert!(p(&["provision", "root-keygen"]).is_err(), "no --out");
    assert!(p(&["provision", "publish"]).is_err(), "no root");
    assert!(
        p(&["provision", "verify", "--url", "http://e"]).is_err(),
        "no root"
    );
    assert!(
        p(&[
            "provision",
            "edge",
            "--name",
            "n",
            "--url",
            "http://e",
            "--root-key",
            "r"
        ])
        .is_err(),
        "no --ssh"
    );
    assert!(p(&[
        "provision",
        "edge",
        "--name",
        "n",
        "--url",
        "http://e",
        "--root-key",
        "r",
        "--ssh",
        "t",
        "--days",
        "0"
    ])
    .is_err());
    match p(&[
        "provision",
        "edge",
        "--name",
        "n",
        "--url",
        "http://e",
        "--root-key",
        "r",
        "--ssh",
        "root@e",
    ]) {
        Ok(Mode::Provision(ProvisionOpts::Edge {
            spec,
            root_key,
            github,
        })) => {
            assert_eq!(root_key, "r");
            assert_eq!(spec.days, 365);
            assert_eq!(spec.key_path, "/var/secrets/losos-edge-identity.key");
            assert_eq!(spec.cert_path, "/etc/losos/edge-identity.cert.json");
            assert_eq!(spec.ssh_command, "ssh");
            assert_eq!(github, ProvisionGithub::default());
        }
        Ok(_) => panic!("parsed as another mode"),
        Err(e) => panic!("{e:?}"),
    }
    assert!(p(&["provision", "nonsense"]).is_err());
}
