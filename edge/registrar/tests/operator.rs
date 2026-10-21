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
  if [ -f "$dir/sealed-stripe-webhook-secret" ]; then
    printf 'stripe-webhook-secret\t1\t1790000000\n'
  else
    printf 'stripe-webhook-secret\t0\t\n'
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
            delegation: None,
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
    assert_eq!(secrets.body[0]["name"], "stripe-webhook-secret");
    assert_eq!(secrets.body[0]["sealed"], false);

    // A value goes to the script's stdin and nowhere else.
    let set = term
        .call(
            "POST",
            "/secrets",
            serde_json::json!({"name": "stripe-webhook-secret", "value": "whsec_secret_value"}),
        )
        .await
        .unwrap();
    assert_eq!(set.status, 200, "{:?}", set.body);
    assert_eq!(set.body["sealed"], true);
    assert_eq!(
        std::fs::read_to_string(edge.dir.join("sealed-stripe-webhook-secret")).unwrap(),
        "whsec_secret_value"
    );
    let args = std::fs::read_to_string(edge.dir.join("seal-args")).unwrap();
    assert!(
        !args.contains("whsec_"),
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

/// The seal script's stand-in for the releaser: `--list` reports whether
/// `sealed-usdc-releaser-key` exists, and a name writes stdin beside it.
const RELEASER_SEAL_SCRIPT: &str = r#"#!/bin/sh
dir=$(dirname "$0")
echo "$*" >> "$dir/seal-args"
if [ "$1" = "--list" ]; then
  if [ -f "$dir/sealed-usdc-releaser-key" ]; then
    printf 'usdc-releaser-key\t1\t1790000000\n'
  else
    printf 'usdc-releaser-key\t0\t\n'
  fi
  exit 0
fi
cat > "$dir/sealed-$1"
echo "$1: sealed"
"#;

/// A chain that knows two things: the releaser's gas and the escrow's
/// `releaser()`, which the test points at whichever key it likes.
async fn releaser_chain(escrow_releaser: std::sync::Arc<std::sync::Mutex<String>>) -> String {
    use axum::extract::State;
    async fn rpc(
        State(releaser): State<std::sync::Arc<std::sync::Mutex<String>>>,
        Json(req): Json<serde_json::Value>,
    ) -> Json<serde_json::Value> {
        let result = match req["method"].as_str().unwrap_or("") {
            "eth_getBalance" => serde_json::json!("0x2386f26fc10000"),
            "eth_call" if req["params"][0]["data"] == "0x02691bcb" => {
                let address = releaser.lock().unwrap().clone();
                serde_json::json!(format!("0x000000000000000000000000{}", &address[2..]))
            }
            _ => serde_json::Value::Null,
        };
        Json(serde_json::json!({"jsonrpc": "2.0", "id": req["id"], "result": result}))
    }
    let app = Router::new()
        .route("/", axum::routing::post(rpc))
        .with_state(escrow_releaser);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    url
}

#[tokio::test]
async fn the_edge_makes_the_escrow_key_and_shows_only_its_address() {
    use losos_registrar::evm_tx::Releaser;
    use losos_registrar::stablecoin::{network, UsdcChain, UsdcOpts};

    let github = fake_github().await;
    let escrow_releaser =
        std::sync::Arc::new(std::sync::Mutex::new(format!("0x{}", "99".repeat(20))));
    let rpc_url = releaser_chain(escrow_releaser.clone()).await;
    let stripe = {
        let app = Router::new().fallback(|| async { StatusCode::NOT_FOUND });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        url
    };
    let owner = HostKey::generate("Operator <op@example.com>").unwrap();
    let public = owner.armored_public().to_string();
    let edge = Edge::start_market_full(
        "operator-releaser",
        &[TenantSpec::new("seller-box", "seller.example", common::GOOD_TOKEN).with_market()],
        &stripe,
        &[("seller-box", false)],
        None,
        move |opts, dir| {
            std::fs::write(dir.join("edge-access-keys.asc"), &public).unwrap();
            let script = dir.join("seal");
            std::fs::write(&script, RELEASER_SEAL_SCRIPT).unwrap();
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
            std::fs::write(dir.join("usdc-rpc-url"), &rpc_url).unwrap();
            opts.github_api_url = github;
            opts.operator = Some(OperatorOpts {
                access_dir: dir.path_str("access"),
                keys_file: dir.path_str("edge-access-keys.asc"),
                seal_script: Some(dir.path_str("seal")),
                systemd_run: None,
            });
            let market = opts.market.as_mut().expect("market on");
            market.currency = "usd".to_string();
            market.usdc = Some(UsdcOpts {
                chains: vec![UsdcChain {
                    network: network("base-sepolia").unwrap(),
                    rpc_file: dir.path_str("usdc-rpc-url"),
                    pay: Some(format!("0x{}", "11".repeat(20))),
                    treasury: format!("0x{}", "44".repeat(20)),
                    genesis: None,
                }],
                ens_rpc_file: None,
                price_sources: [
                    losos_registrar::coins::PriceSource::Kraken {
                        base: "http://127.0.0.1:9".to_string(),
                    },
                    losos_registrar::coins::PriceSource::CoinGecko {
                        base: "http://127.0.0.1:9".to_string(),
                    },
                ],
                // Where systemd would put the credential once the
                // restarted registrar loads it; read per request.
                releaser_file: Some(dir.path_str("sealed-usdc-releaser-key")),
            });
        },
    )
    .await;

    let hk: serde_json::Value = edge
        .client
        .get(format!("{}/operator/host-key", edge.base))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let server = Keyring::parse(hk["publicKey"].as_str().unwrap())
        .unwrap()
        .keys()[0]
        .clone();
    let new = |scope: &str| serde_json::json!({"name": "laptop", "fingerprint": owner.fingerprint(), "scope": scope, "expiresInDays": 30});
    let (_, full) = tokens_post(&edge, "/operator/tokens", Some("gho_matus"), new("full")).await;
    let (_, read) = tokens_post(&edge, "/operator/tokens", Some("gho_matus"), new("read")).await;
    let term = Terminal {
        edge: &edge,
        key: &owner,
        server: server.clone(),
        fingerprint: hk["fingerprint"].as_str().unwrap().to_string(),
        token: full["token"].as_str().unwrap().to_string(),
    };
    let reader = Terminal {
        token: read["token"].as_str().unwrap().to_string(),
        server,
        fingerprint: term.fingerprint.clone(),
        ..term
    };

    // No key yet: no address, and nothing to ask the chain about.
    let before = reader
        .call("GET", "/escrow/releaser", serde_json::Value::Null)
        .await
        .unwrap();
    assert_eq!(before.status, 200, "{:?}", before.body);
    assert!(before.body["address"].is_null());
    assert_eq!(before.body["networks"][0]["network"], "base-sepolia");
    assert!(before.body["networks"][0]["balanceWei"].is_null());

    // A key cannot be handed in, and a read token cannot make one.
    let pasted = term
        .call(
            "POST",
            "/secrets",
            serde_json::json!({"name": "usdc-releaser-key", "value": "ab".repeat(32)}),
        )
        .await
        .unwrap();
    assert_eq!(pasted.status, 400);
    assert_eq!(
        reader
            .call("POST", "/escrow/releaser", serde_json::Value::Null)
            .await
            .unwrap_err()
            .0,
        403
    );

    // The edge makes it, seals it from stdin, and answers its address.
    let made = term
        .call("POST", "/escrow/releaser", serde_json::Value::Null)
        .await
        .unwrap();
    assert_eq!(made.status, 200, "{:?}", made.body);
    let first = made.body["address"].as_str().unwrap().to_string();
    let sealed = std::fs::read_to_string(edge.dir.join("sealed-usdc-releaser-key")).unwrap();
    assert_eq!(Releaser::from_hex(&sealed).unwrap().address_hex(), first);
    assert!(
        !made.body.to_string().contains(&sealed),
        "the key came back"
    );
    let args = std::fs::read_to_string(edge.dir.join("seal-args")).unwrap();
    assert!(!args.contains(&sealed), "the key reached an argument");

    // A second one would orphan the first; it takes `replace`.
    let again = term
        .call("POST", "/escrow/releaser", serde_json::Value::Null)
        .await
        .unwrap();
    assert_eq!(again.status, 409);

    // The chain side: gas, and whether the escrow takes orders from it yet.
    let shown = reader
        .call("GET", "/escrow/releaser", serde_json::Value::Null)
        .await
        .unwrap();
    assert_eq!(shown.body["address"], first.as_str());
    let row = &shown.body["networks"][0];
    assert_eq!(row["balanceWei"], "10000000000000000");
    assert_eq!(row["escrowAgrees"], false);
    *escrow_releaser.lock().unwrap() = first.clone();
    let shown = reader
        .call("GET", "/escrow/releaser", serde_json::Value::Null)
        .await
        .unwrap();
    assert_eq!(shown.body["networks"][0]["escrowAgrees"], true);

    let replaced = term
        .call(
            "POST",
            "/escrow/releaser",
            serde_json::json!({"replace": true}),
        )
        .await
        .unwrap();
    assert_eq!(replaced.status, 200);
    assert_eq!(replaced.body["replaced"], true);
    assert_ne!(replaced.body["address"], first.as_str());
}
