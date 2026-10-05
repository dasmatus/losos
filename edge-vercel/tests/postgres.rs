//! The Postgres store against a real Postgres.
//!
//! Gated on `LOSOS_TEST_DATABASE_URL`: without it every test here passes
//! trivially, so `cargo test` stays runnable on a laptop with no database.
//! CI's test job starts one and sets the variable; locally,
//! `docker run -e POSTGRES_USER=losos -e POSTGRES_PASSWORD=losos -p 5433:5432 postgres`
//! and `LOSOS_TEST_DATABASE_URL=postgres://losos:losos@127.0.0.1:5433/losos?sslmode=disable`
//! does the same. The tests share one database, so each uses its own key.

mod common;

use axum::http::Method;
use common::{call, heartbeat_body, host, register_body, TempDir, GOOD_TOKEN};
use losos_edge_vercel::{PgStore, Store, StoreSettings};
use losos_registrar::Snapshot;

const VAR: &str = "LOSOS_TEST_DATABASE_URL";

fn store(key: &str) -> Option<PgStore> {
    let url = std::env::var(VAR).ok()?;
    if url.is_empty() {
        return None;
    }
    Some(
        PgStore::new(&StoreSettings {
            url,
            key: format!("losos:test:{key}:{}", std::process::id()),
        })
        .expect("store from the test database URL"),
    )
}

/// A fresh database has no table; the first use creates it, and a key that
/// was never written reads as nothing rather than as an error.
#[tokio::test]
async fn a_fresh_database_reads_as_empty_and_round_trips_a_snapshot() {
    let Some(store) = store("round-trip") else {
        eprintln!("{VAR} unset; skipping");
        return;
    };
    assert_eq!(store.kind(), "postgres");
    let loaded = store.load().await.expect("load from an empty table");
    assert!(loaded.is_none(), "nothing was ever saved under this key");

    let snapshot: Snapshot = serde_json::from_value(serde_json::json!({
        "tenants": {
            "demo-box": {
                "hostname": "demo.losos.example",
                "rathole_port": 50000,
                "seen_ago_secs": 7,
                "idle": [true, 3]
            }
        },
        "compute_windows": {
            "demo-box": {
                "share_compute": true,
                "window_start": "23:00",
                "window_end": "07:00",
                "tz": "Europe/Berlin"
            }
        }
    }))
    .expect("a snapshot");
    store.save(&snapshot).await.expect("save");
    let loaded = store.load().await.expect("load").expect("saved just now");
    assert_eq!(loaded, snapshot);

    // A second save under the same key replaces, not duplicates.
    let empty = Snapshot::default();
    store.save(&empty).await.expect("save again");
    let loaded = store.load().await.expect("load").expect("saved");
    assert_eq!(loaded, empty);
}

/// The property the crate exists for, over the real store: a second host,
/// built from nothing on the same database, knows what the first was told.
#[tokio::test]
async fn two_hosts_on_one_database_see_one_registry() {
    let Some(first_store) = store("two-hosts") else {
        eprintln!("{VAR} unset; skipping");
        return;
    };
    // Same URL and key, separate connections, as two function instances have.
    let second_store = PgStore::new(&StoreSettings {
        url: std::env::var(VAR).expect("checked above"),
        key: format!("losos:test:two-hosts:{}", std::process::id()),
    })
    .expect("store");

    let first_dir = TempDir::new();
    let first = host(&first_dir, Some(Box::new(first_store))).await;
    let body = register_body("demo-box", "demo.losos.example", GOOD_TOKEN);
    let (status, body) = call(&first, Method::POST, "/register", Some(&body)).await;
    assert_eq!(status, 200, "body: {body}");
    drop(first);
    drop(first_dir);

    let second_dir = TempDir::new();
    let second = host(&second_dir, Some(Box::new(second_store))).await;
    let body = heartbeat_body("demo-box", GOOD_TOKEN, true);
    let (status, _) = call(&second, Method::POST, "/heartbeat", Some(&body)).await;
    assert_eq!(status, 204, "the second instance should know demo-box");

    let (_, body) = call(&second, Method::GET, "/status", None).await;
    let parsed: serde_json::Value = serde_json::from_str(&body).expect("JSON");
    assert_eq!(parsed["store"], "postgres");
    let tenants = parsed["tenants"].as_array().expect("tenants");
    assert_eq!(tenants.len(), 1);
    assert_eq!(tenants[0]["rathole_port"], 50000);
    assert_eq!(tenants[0]["idle"], true);
    let (_, yaml) = call(&second, Method::GET, "/status/traefik", None).await;
    assert!(yaml.contains("Host(`demo.losos.example`)"), "yaml: {yaml}");
    assert!(!yaml.contains(GOOD_TOKEN));
}
