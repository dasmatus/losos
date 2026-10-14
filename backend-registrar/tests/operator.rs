//! The edge's terminal routes end to end (`docs/edge-terminal.md`): tokens
//! made behind the GitHub gate, calls sealed as `losos` seals them, and the
//! seal script handed each value on its stdin.

mod common;

use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use common::{Edge, TenantSpec};
use losos_access::{open_answer, seal_request, Answer, HostKey, Keyring, Request, SeededKey};
use losos_registrar::operator::OperatorOpts;

/// The listed operator's id in `operators.json`.
const MATUS_ID: u64 = 330_471_626;

async fn fake_github() -> String {
    let app = Router::new().route(
        "/user",
        get(|headers: HeaderMap| async move {
            let auth = headers
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            match auth.strip_prefix("Bearer ") {
                Some("gho_matus") => {
                    Json(serde_json::json!({"id": MATUS_ID, "login": "dasmatus"})).into_response()
                }
                Some("gho_stranger") => {
                    Json(serde_json::json!({"id": 42, "login": "stranger"})).into_response()
                }
                _ => StatusCode::UNAUTHORIZED.into_response(),
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    url
}

/// A stand-in for `losos-seal-credential`: `--list` prints the two rows,
/// a name reads the value from stdin into `sealed-<name>` beside it and
/// records its arguments, and `bad` fails the way a shape check does.
const SEAL_SCRIPT: &str = r#"#!/bin/sh
dir=$(dirname "$0")
echo "$*" >> "$dir/seal-args"
echo "$LOSOS_SEAL_DEFER" > "$dir/seal-defer"
if [ "$1" = "--list" ]; then
  if [ -f "$dir/sealed-claude-api-key" ]; then
    printf 'claude-api-key\t1\t1790000000\n'
  else
    printf 'claude-api-key\t0\t\n'
  fi
  printf 'stripe-secret-key\t0\t\n'
  exit 0
fi
if [ "$1" = "bad" ]; then
  echo "losos-seal-credential: bad is not shaped like one; nothing was sealed" >&2
  exit 1
fi
cat > "$dir/sealed-$1"
echo "$1: sealed"
"#;

struct Terminal<'a> {
    edge: &'a Edge,
    key: &'a HostKey,
    server: SeededKey,
    fingerprint: String,
    token: String,
}

fn nonce() -> String {
    use std::io::Read;
    let mut b = [0u8; 16];
    std::fs::File::open("/dev/urandom")
        .unwrap()
        .read_exact(&mut b)
        .unwrap();
    b.iter().map(|x| format!("{x:02x}")).collect()
}

impl Terminal<'_> {
    /// Seal and send one call. `Ok` the opened answer, `Err` a plain refusal.
    async fn call(
        &self,
        method: &str,
        path: &str,
        body: serde_json::Value,
    ) -> Result<Answer, (u16, String)> {
        let n = nonce();
        let req = Request {
            v: 1,
            host: self.fingerprint.clone(),
            token: self.token.clone(),
            method: method.into(),
            path: path.into(),
            ts: losos_access::now_secs(),
            nonce: n.clone(),
            body,
        };
        let sealed = seal_request(self.key, &self.server, &req).unwrap();
        let resp = self
            .edge
            .client
            .post(format!("{}/operator/call", self.edge.base))
            .header("Content-Type", "application/pgp-encrypted")
            .body(sealed)
            .send()
            .await
            .unwrap();
        let status = resp.status().as_u16();
        let sealed_answer = resp
            .headers()
            .get("content-type")
            .is_some_and(|v| v == "application/pgp-encrypted");
        let text = resp.text().await.unwrap();
        if !sealed_answer {
            return Err((status, text));
        }
        let answer = open_answer(self.key, &self.server, text.as_bytes()).unwrap();
        assert_eq!(status, answer.status, "the outer status is the inner one");
        assert_eq!(answer.nonce, n);
        Ok(answer)
    }
}

async fn tokens_post(
    edge: &Edge,
    path: &str,
    github: Option<&str>,
    body: serde_json::Value,
) -> (u16, serde_json::Value) {
    let mut req = edge.client.post(format!("{}{path}", edge.base)).json(&body);
    if let Some(t) = github {
        req = req.bearer_auth(t);
    }
    let resp = req.send().await.unwrap();
    let status = resp.status().as_u16();
    let text = resp.text().await.unwrap();
    (
        status,
        serde_json::from_str(&text).unwrap_or(serde_json::Value::String(text)),
    )
}

#[tokio::test]
async fn an_operator_reaches_the_four_paths_with_a_sealed_call() {
    let github = fake_github().await;
    let owner = HostKey::generate("Operator <op@example.com>").unwrap();
    let stranger = HostKey::generate("Stranger").unwrap();
    let public = owner.armored_public().to_string();
    let edge = Edge::start_custom(
        "operator",
        &[TenantSpec::new(
            "box-1",
            "box-1.example.test",
            "t".repeat(64).as_str(),
        )],
        move |opts, dir| {
            std::fs::write(dir.join("edge-access-keys.asc"), &public).unwrap();
            let script = dir.join("seal");
            std::fs::write(&script, SEAL_SCRIPT).unwrap();
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
            opts.github_api_url = github;
            opts.operator = Some(OperatorOpts {
                access_dir: dir.path_str("access"),
                keys_file: dir.path_str("edge-access-keys.asc"),
                seal_script: Some(dir.path_str("seal")),
                systemd_run: None,
            });
        },
    )
    .await;
    let (status, _) = edge
        .register("box-1", "box-1.example.test", &"t".repeat(64))
        .await;
    assert_eq!(status, 200);

    // The host key is public; `losos login` pins it.
    let hk: serde_json::Value = edge
        .client
        .get(format!("{}/operator/host-key", edge.base))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let fingerprint = hk["fingerprint"].as_str().unwrap().to_string();
    let server = Keyring::parse(hk["publicKey"].as_str().unwrap())
        .unwrap()
        .keys()[0]
        .clone();

    // Tokens are behind the GitHub gate.
    let new = |scope: &str| serde_json::json!({"name": "laptop", "fingerprint": owner.fingerprint(), "scope": scope, "expiresInDays": 30});
    assert_eq!(
        tokens_post(&edge, "/operator/tokens", None, new("full"))
            .await
            .0,
        401
    );
    assert_eq!(
        tokens_post(&edge, "/operator/tokens", Some("gho_nobody"), new("full"))
            .await
            .0,
        401
    );
    assert_eq!(
        tokens_post(&edge, "/operator/tokens", Some("gho_stranger"), new("full"))
            .await
            .0,
        403
    );
    let (status, unseeded) = tokens_post(
        &edge,
        "/operator/tokens",
        Some("gho_matus"),
        serde_json::json!({"name": "x", "fingerprint": stranger.fingerprint(), "scope": "full", "expiresInDays": null}),
    )
    .await;
    assert_eq!(status, 400, "{unseeded}");
    let (status, made) =
        tokens_post(&edge, "/operator/tokens", Some("gho_matus"), new("full")).await;
    assert_eq!(status, 200, "{made}");
    let full = made["token"].as_str().unwrap().to_string();
    let (_, made) = tokens_post(&edge, "/operator/tokens", Some("gho_matus"), new("read")).await;
    let read = made["token"].as_str().unwrap().to_string();
    let read_id = made["record"]["id"].as_str().unwrap().to_string();

    let term = Terminal {
        edge: &edge,
        key: &owner,
        server: server.clone(),
        fingerprint: fingerprint.clone(),
        token: full.clone(),
    };

    let overview = term
        .call("GET", "/overview", serde_json::Value::Null)
        .await
        .unwrap();
    assert_eq!(overview.status, 200);
    assert_eq!(overview.body["health"], true);
    assert_eq!(overview.body["tenants"][0]["id"], "box-1");
    assert!(overview.body["tenants"][0]["seenAgoSecs"].is_u64());

    let secrets = term
        .call("GET", "/secrets", serde_json::Value::Null)
        .await
        .unwrap();
    assert_eq!(secrets.body[0]["name"], "claude-api-key");
    assert_eq!(secrets.body[0]["sealed"], false);

    // A value goes to the script's stdin and nowhere else.
    let set = term
        .call(
            "POST",
            "/secrets",
            serde_json::json!({"name": "claude-api-key", "value": "sk-ant-secret-value"}),
        )
        .await
        .unwrap();
    assert_eq!(set.status, 200, "{:?}", set.body);
    assert_eq!(set.body["sealed"], true);
    assert_eq!(
        std::fs::read_to_string(edge.dir.join("sealed-claude-api-key")).unwrap(),
        "sk-ant-secret-value"
    );
    let args = std::fs::read_to_string(edge.dir.join("seal-args")).unwrap();
    assert!(
        !args.contains("sk-ant"),
        "the value reached an argument: {args}"
    );
    assert_eq!(
        std::fs::read_to_string(edge.dir.join("seal-defer"))
            .unwrap()
            .trim(),
        "losos-registrar.service"
    );
    let secrets = term
        .call("GET", "/secrets", serde_json::Value::Null)
        .await
        .unwrap();
    assert_eq!(secrets.body[0]["sealed"], true);
    assert_eq!(secrets.body[0]["updated"], 1_790_000_000);

    // The script's refusal comes back sealed, with its words.
    let bad = term
        .call(
            "POST",
            "/secrets",
            serde_json::json!({"name": "bad", "value": "x"}),
        )
        .await
        .unwrap();
    assert_eq!(bad.status, 400);
    assert!(bad.body["error"].as_str().unwrap().contains("not shaped"));

    let reconcile = term
        .call("POST", "/reconcile", serde_json::Value::Null)
        .await
        .unwrap();
    assert_eq!(reconcile.status, 200);

    // Nothing else is served, and a box's path is not an edge's.
    let other = term
        .call("GET", "/api/state", serde_json::Value::Null)
        .await
        .unwrap();
    assert_eq!(other.status, 404);

    // A read token reads and writes nothing.
    let reader = Terminal {
        token: read.clone(),
        server: server.clone(),
        fingerprint: fingerprint.clone(),
        ..term
    };
    assert_eq!(
        reader
            .call("GET", "/overview", serde_json::Value::Null)
            .await
            .unwrap()
            .status,
        200
    );
    assert_eq!(
        reader
            .call("POST", "/reconcile", serde_json::Value::Null)
            .await
            .unwrap_err()
            .0,
        403
    );

    // The right token signed by another key is refused before anything runs.
    let thief = Terminal {
        edge: &edge,
        key: &stranger,
        server: server.clone(),
        fingerprint: fingerprint.clone(),
        token: full.clone(),
    };
    assert_eq!(
        thief
            .call("GET", "/overview", serde_json::Value::Null)
            .await
            .unwrap_err()
            .0,
        401
    );

    // The list shows both tokens without hashes; revoking one ends it.
    let (status, list) = tokens_post(
        &edge,
        "/operator/tokens/list",
        Some("gho_matus"),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, 200, "{list}");
    assert_eq!(list["tokens"].as_array().unwrap().len(), 2);
    assert!(list["tokens"][0].get("sha256").is_none());
    assert_eq!(list["hostKey"]["fingerprint"], fingerprint.as_str());
    let (status, _) = tokens_post(
        &edge,
        "/operator/tokens/revoke",
        Some("gho_matus"),
        serde_json::json!({"id": read_id}),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(
        reader
            .call("GET", "/overview", serde_json::Value::Null)
            .await
            .unwrap_err()
            .0,
        401
    );
}

#[tokio::test]
async fn an_edge_without_access_serves_no_terminal() {
    let edge = Edge::start("no-operator", &[]).await;
    for (path, get) in [("/operator/host-key", true), ("/operator/call", false)] {
        let req = if get {
            edge.client.get(format!("{}{path}", edge.base))
        } else {
            edge.client.post(format!("{}{path}", edge.base)).body("x")
        };
        assert_eq!(req.send().await.unwrap().status().as_u16(), 404, "{path}");
    }
}
