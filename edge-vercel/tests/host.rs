//! The host's one job: make the registrar behave on a platform that forgets.
//!
//! Every test drives the real router through the real wrapper, with the real
//! `Store` talking HTTP to a fake Upstash. What is asserted is the serverless
//! property the self-hosted tests cannot: that a *second* instance, built from
//! nothing, knows what the first one was told.

mod common;

use axum::http::Method;
use common::{call, fake_upstash, heartbeat_body, host, register_body, TempDir, GOOD_TOKEN};

#[tokio::test]
async fn health_and_status_answer_with_nothing_registered() {
    let dir = TempDir::new();
    let host = host(&dir, None).await;

    let (status, body) = call(&host, Method::GET, "/health", None).await;
    assert_eq!((status, body.as_str()), (200, "ok"));

    let (status, body) = call(&host, Method::GET, "/status", None).await;
    assert_eq!(status, 200, "body: {body}");
    let parsed: serde_json::Value = serde_json::from_str(&body).expect("status is JSON");
    assert_eq!(parsed["host"], "vercel");
    assert_eq!(parsed["store"], "memory");
    assert_eq!(parsed["heartbeat_ttl_secs"], 120);
    assert_eq!(parsed["tenants"].as_array().map(Vec::len), Some(0));
    assert_eq!(parsed["not_on_this_host"].as_array().map(Vec::len), Some(4));

    let (status, body) = call(&host, Method::GET, "/status/traefik", None).await;
    assert_eq!(status, 200);
    assert!(body.starts_with("# No live tenants"), "body: {body}");

    // No tunnel, so no key to pin.
    let (status, _) = call(&host, Method::GET, "/noise-public-key", None).await;
    assert_eq!(status, 404);
}

/// The registrar's own authentication runs unchanged behind the wrapper.
#[tokio::test]
async fn enrollment_is_closed_here_too() {
    let dir = TempDir::new();
    let (url, _kv) = fake_upstash().await;
    let host = host(&dir, Some(&url)).await;

    let wrong = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";
    for (id, token) in [
        ("demo-box", wrong),
        ("demo-box", ""),
        ("stranger", GOOD_TOKEN),
    ] {
        let body = register_body(id, "demo.losos.example", token);
        let (status, _) = call(&host, Method::POST, "/register", Some(&body)).await;
        assert_eq!(status, 401, "{id:?} with token {token:?} was not refused");
    }
    // The whitelisted hostname is the only one the tenant may claim.
    let body = register_body("demo-box", "evil.example", GOOD_TOKEN);
    let (status, _) = call(&host, Method::POST, "/register", Some(&body)).await;
    assert_eq!(status, 403);

    let (_, body) = call(&host, Method::GET, "/status", None).await;
    let parsed: serde_json::Value = serde_json::from_str(&body).expect("JSON");
    assert_eq!(parsed["tenants"].as_array().map(Vec::len), Some(0));
}

#[tokio::test]
async fn a_registered_box_shows_up_in_status_and_in_the_traefik_config() {
    let dir = TempDir::new();
    let (url, kv) = fake_upstash().await;
    let host = host(&dir, Some(&url)).await;

    let body = register_body("demo-box", "demo.losos.example", GOOD_TOKEN);
    let (status, body) = call(&host, Method::POST, "/register", Some(&body)).await;
    assert_eq!(status, 200, "body: {body}");
    let parsed: serde_json::Value = serde_json::from_str(&body).expect("JSON");
    assert_eq!(parsed["rathole_port"], 50000);

    let body = heartbeat_body("demo-box", GOOD_TOKEN, true);
    let (status, _) = call(&host, Method::POST, "/heartbeat", Some(&body)).await;
    assert_eq!(status, 204);

    let (_, body) = call(&host, Method::GET, "/status", None).await;
    let parsed: serde_json::Value = serde_json::from_str(&body).expect("JSON");
    assert_eq!(parsed["store"], "redis");
    let tenants = parsed["tenants"].as_array().expect("tenants");
    assert_eq!(tenants.len(), 1);
    assert_eq!(tenants[0]["id"], "demo-box");
    assert_eq!(tenants[0]["hostname"], "demo.losos.example");
    assert_eq!(tenants[0]["rathole_port"], 50000);
    assert_eq!(tenants[0]["idle"], true);
    assert!(tenants[0]["seen_ago_secs"].as_u64().expect("age") <= 2);

    let (_, yaml) = call(&host, Method::GET, "/status/traefik", None).await;
    assert!(yaml.contains("Host(`demo.losos.example`)"), "yaml: {yaml}");
    assert!(yaml.contains("http://127.0.0.1:50000"), "yaml: {yaml}");
    assert!(
        !yaml.contains(GOOD_TOKEN),
        "a token reached the public route"
    );

    // What went to the store: the snapshot, with no token in it anywhere.
    let saved = kv
        .lock()
        .expect("kv")
        .get("losos:test")
        .cloned()
        .expect("snapshot saved");
    assert!(saved.contains("demo.losos.example"));
    assert!(!saved.contains(GOOD_TOKEN), "a token reached the store");
}

/// The property this crate exists for.
#[tokio::test]
async fn a_fresh_instance_knows_what_the_previous_one_was_told() {
    let (url, _kv) = fake_upstash().await;

    let first_dir = TempDir::new();
    let first = host(&first_dir, Some(&url)).await;
    let body = register_body("demo-box", "demo.losos.example", GOOD_TOKEN);
    let (status, _) = call(&first, Method::POST, "/register", Some(&body)).await;
    assert_eq!(status, 200);
    drop(first);
    drop(first_dir);

    // A new instance: new scratch disk, nothing in memory, same store.
    let second_dir = TempDir::new();
    let second = host(&second_dir, Some(&url)).await;
    let body = heartbeat_body("demo-box", GOOD_TOKEN, false);
    let (status, _) = call(&second, Method::POST, "/heartbeat", Some(&body)).await;
    assert_eq!(status, 204, "the second instance should know demo-box");

    let (_, body) = call(&second, Method::GET, "/status", None).await;
    let parsed: serde_json::Value = serde_json::from_str(&body).expect("JSON");
    let tenants = parsed["tenants"].as_array().expect("tenants");
    assert_eq!(tenants.len(), 1);
    // The port the first instance allocated is the port the second reports.
    assert_eq!(tenants[0]["rathole_port"], 50000);
    assert_eq!(tenants[0]["idle"], false);
}

/// Without a store, memory is all there is — and a new instance has none.
#[tokio::test]
async fn without_a_store_a_fresh_instance_starts_empty() {
    let first_dir = TempDir::new();
    let first = host(&first_dir, None).await;
    let body = register_body("demo-box", "demo.losos.example", GOOD_TOKEN);
    let (status, _) = call(&first, Method::POST, "/register", Some(&body)).await;
    assert_eq!(status, 200);
    // Warm instance: still there.
    let body = heartbeat_body("demo-box", GOOD_TOKEN, false);
    let (status, _) = call(&first, Method::POST, "/heartbeat", Some(&body)).await;
    assert_eq!(status, 204);

    let second_dir = TempDir::new();
    let second = host(&second_dir, None).await;
    let (status, _) = call(&second, Method::POST, "/heartbeat", Some(&body)).await;
    assert_eq!(
        status, 404,
        "a heartbeat to an instance that never saw the box re-enrols it"
    );
}

/// The reconciler's prune never ran while no instance was alive, so the
/// import applies the TTL to what it loads.
#[tokio::test]
async fn a_box_whose_heartbeats_stopped_is_pruned_on_load() {
    let (url, kv) = fake_upstash().await;
    kv.lock().expect("kv").insert(
        "losos:test".to_string(),
        serde_json::json!({
            "tenants": {
                "demo-box": {
                    "hostname": "demo.losos.example",
                    "rathole_port": 50000,
                    "seen_ago_secs": 3600,
                    "idle": [true, 3600]
                },
                "other-box": {
                    "hostname": "other.losos.example",
                    "rathole_port": 50001,
                    "seen_ago_secs": 10,
                    "idle": null
                }
            },
            "compute_windows": {}
        })
        .to_string(),
    );
    let dir = TempDir::new();
    let host = host(&dir, Some(&url)).await;

    let (_, body) = call(&host, Method::GET, "/status", None).await;
    let parsed: serde_json::Value = serde_json::from_str(&body).expect("JSON");
    let tenants = parsed["tenants"].as_array().expect("tenants");
    // `other-box` is not whitelisted here, so the reconcile pass after the
    // read drops it as stale; `demo-box` is whitelisted but an hour silent.
    assert!(
        tenants.is_empty(),
        "expected both boxes gone, got {tenants:?}"
    );
    let saved = kv
        .lock()
        .expect("kv")
        .get("losos:test")
        .cloned()
        .expect("snapshot saved after the read");
    let saved: serde_json::Value = serde_json::from_str(&saved).expect("JSON");
    assert_eq!(saved["tenants"].as_object().map(|m| m.len()), Some(0));
}

/// A store that cannot be reached must not be papered over with memory.
#[tokio::test]
async fn an_unreachable_store_fails_the_request_rather_than_forgetting() {
    // Bind and drop: the port is now closed, so the connect is refused fast.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let url = format!("http://{}", listener.local_addr().expect("addr"));
    drop(listener);

    let dir = TempDir::new();
    let host = host(&dir, Some(&url)).await;
    let body = register_body("demo-box", "demo.losos.example", GOOD_TOKEN);
    let (status, body) = call(&host, Method::POST, "/register", Some(&body)).await;
    assert_eq!(status, 503, "body: {body}");
}
