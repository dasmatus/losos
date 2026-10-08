//! `registry.json` durability: the state the edge re-attaches from.
//!
//! A torn or lost write here is not a cosmetic bug. `Registry::load` fails on
//! malformed JSON, `serve` propagates that failure, and systemd's
//! `Restart=always` then restarts the registrar into the same failure every
//! five seconds — every appliance offline until an operator deletes the file
//! by hand. A silently *lost* write is worse, because nothing looks broken: a
//! tenant simply vanishes from the registry and its route disappears at the
//! next reconcile.

mod common;

use std::collections::HashSet;
use std::sync::Arc;

use common::TempDir;
use losos_registrar::Registry;

const RANGE: (u16, u16) = (50000, 50100);

/// 32 registrations issued concurrently must all be readable back from disk.
///
/// `persist` used to snapshot the tenant map under the lock, *release* the
/// lock, and only then write — so two handlers could both snapshot and then
/// race their writes at the same path, with the later rename carrying the
/// earlier snapshot. Holding one lock across snapshot+write is what makes this
/// hold; the count and the distinctness of the ports are the two things a lost
/// update would break.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_registrations_all_survive_a_load_round_trip() {
    let dir = TempDir::new("registry-concurrent");
    let path = dir.join("registry.json");
    let registry = Arc::new(Registry::new(path.clone(), RANGE));

    let writers: Vec<_> = (0..32)
        .map(|i| {
            let registry = Arc::clone(&registry);
            tokio::spawn(async move {
                let id = format!("box{i:02}");
                registry.register(&id, &format!("{id}.losos.cfd")).await
            })
        })
        .collect();
    for writer in writers {
        writer
            .await
            .expect("registration task runs to completion")
            .expect("registration succeeds");
    }

    // A fresh Registry over the same file is exactly what the next boot does.
    let reloaded = Registry::new(path, RANGE);
    reloaded
        .load()
        .await
        .expect("registry.json parses after 32 concurrent writes");
    let views = reloaded.views().await;

    assert_eq!(views.len(), 32, "a registration was lost: {views:?}");
    let ports: HashSet<u16> = views.iter().map(|v| v.rathole_port).collect();
    assert_eq!(ports.len(), 32, "two tenants share a rathole port");
    assert!(
        ports.iter().all(|p| (RANGE.0..=RANGE.1).contains(p)),
        "a port landed outside {RANGE:?}"
    );
    for (i, view) in views.iter().enumerate() {
        assert_eq!(view.id, format!("box{i:02}"));
        assert_eq!(view.hostname, format!("box{i:02}.losos.cfd"));
    }
}

/// Two independent writers pounding the same path must never leave a file that
/// fails to parse.
///
/// This is the second half of the same defect: the atomic-write helper derived
/// its scratch name with `path.with_extension("tmp")`, so *every* writer of
/// `registry.json` opened one shared `registry.tmp` with `truncate` — one
/// writer could rename another's half-written buffer over the target. Unique
/// `create_new` temp names make the writers independent; whichever wins the
/// last rename, the file is always a complete registry.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn racing_writers_never_leave_a_torn_registry() {
    let dir = TempDir::new("registry-torn");
    let path = dir.join("registry.json");

    let first = Arc::new(Registry::new(path.clone(), RANGE));
    let second = Arc::new(Registry::new(path.clone(), RANGE));
    // Different tenant counts, so the two serialisations differ in length and
    // a torn write is a parse failure rather than a coincidence.
    for i in 0..8 {
        first
            .register(&format!("a{i}"), &format!("a{i}.losos.cfd"))
            .await
            .expect("seed first writer");
    }
    second
        .register("solo", "solo.losos.cfd")
        .await
        .expect("seed second writer");

    let mut writers = Vec::new();
    for round in 0..24 {
        let first = Arc::clone(&first);
        let second = Arc::clone(&second);
        writers.push(tokio::spawn(async move {
            first
                .register(&format!("a{}", round % 8), "a.losos.cfd")
                .await
                .expect("first writer");
        }));
        writers.push(tokio::spawn(async move {
            second
                .register("solo", "solo.losos.cfd")
                .await
                .expect("second writer");
        }));
    }
    for writer in writers {
        writer.await.expect("writer task runs to completion");
    }

    let reloaded = Registry::new(path.clone(), RANGE);
    reloaded
        .load()
        .await
        .expect("registry.json is always a complete document");
    assert!(
        !reloaded.views().await.is_empty(),
        "the surviving document must still hold tenants"
    );

    // No scratch files left behind next to the target.
    let strays: Vec<_> = std::fs::read_dir(dir.path())
        .expect("read temp dir")
        .filter_map(std::result::Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".tmp"))
        .collect();
    assert!(
        strays.is_empty(),
        "atomic write left scratch files: {strays:?}"
    );
}

/// `registry.json` outlives reboots and is reachable by anything that can
/// write the edge's state directory, so `load` treats it as data to validate,
/// not as truth. A port outside the configured range would otherwise become a
/// live rathole bind the operator never authorised.
#[tokio::test]
async fn load_drops_entries_it_cannot_trust() {
    let dir = TempDir::new("registry-validate");
    let path = dir.join("registry.json");
    std::fs::write(
        &path,
        serde_json::json!({
            "tenants": {
                "good":        { "hostname": "good.losos.cfd",   "port": 50000 },
                "below-range": { "hostname": "low.losos.cfd",    "port": 22    },
                "above-range": { "hostname": "high.losos.cfd",   "port": 60000 },
                "duplicate":   { "hostname": "dupe.losos.cfd",   "port": 50000 },
                "":            { "hostname": "anon.losos.cfd",   "port": 50001 },
                "no-hostname": { "hostname": "",                 "port": 50002 }
            }
        })
        .to_string(),
    )
    .expect("write registry.json");

    let registry = Registry::new(path, RANGE);
    registry.load().await.expect("load tolerates bad entries");
    let ids: Vec<String> = registry.views().await.into_iter().map(|v| v.id).collect();

    // "duplicate" claims a port "good" already holds; iterating in id order
    // makes "duplicate" the one that survives and "good" the one dropped, and
    // — the property that matters — exactly one of them survives.
    assert_eq!(ids.len(), 1, "expected one surviving tenant, got {ids:?}");
    assert!(
        ids[0] == "duplicate" || ids[0] == "good",
        "the survivor should be one of the two port-50000 claimants, got {ids:?}"
    );
}

/// A registry file that is simply absent is the first-boot case, not an error.
#[tokio::test]
async fn load_treats_a_missing_file_as_empty() {
    let dir = TempDir::new("registry-absent");
    let registry = Registry::new(dir.join("registry.json"), RANGE);
    registry
        .load()
        .await
        .expect("a missing file is not an error");
    assert!(registry.views().await.is_empty());
}

/// Re-registering keeps the assigned port so Traefik and rathole don't churn.
#[tokio::test]
async fn re_registration_preserves_the_assigned_port() {
    let dir = TempDir::new("registry-stable-port");
    let registry = Registry::new(dir.join("registry.json"), RANGE);

    let first = registry
        .register("mattbox", "mattbox.losos.cfd")
        .await
        .expect("first registration");
    let second = registry
        .register("mattbox", "mattbox.losos.cfd")
        .await
        .expect("second registration");
    assert_eq!(first, second);
}

/// Exhausting the range is an error, never a sentinel port.
#[tokio::test]
async fn a_full_port_range_is_an_error() {
    let dir = TempDir::new("registry-exhausted");
    let registry = Registry::new(dir.join("registry.json"), (50000, 50001));

    registry.register("a", "a.losos.cfd").await.expect("first");
    registry.register("b", "b.losos.cfd").await.expect("second");
    let third = registry.register("c", "c.losos.cfd").await;
    assert!(third.is_err(), "expected exhaustion, got {third:?}");
}

/// `export` → `import` is how a host with no durable disk carries the registry
/// between two processes: ports and liveness must survive the trip, and the
/// TTL must be applied on the way in, because the reconciler that would have
/// pruned did not exist while nobody was running.
#[tokio::test]
async fn a_snapshot_round_trip_keeps_ports_and_ages_and_prunes_by_ttl() {
    let ttl = std::time::Duration::from_secs(120);
    let dir = TempDir::new("snapshot-round-trip");
    let first = Registry::new(dir.join("a.json"), RANGE);
    assert_eq!(
        first
            .register("box-a", "a.losos.cfd")
            .await
            .expect("register a"),
        50000
    );
    assert_eq!(
        first
            .register("box-b", "b.losos.cfd")
            .await
            .expect("register b"),
        50001
    );
    first.record_idle("box-a", true).await;

    let mut snapshot = first.export().await;
    assert_eq!(snapshot.tenants.len(), 2);
    assert_eq!(snapshot.tenants["box-a"].rathole_port, 50000);
    assert!(snapshot.tenants["box-a"].seen_ago_secs <= 1);
    assert_eq!(
        snapshot.tenants["box-a"].idle.map(|(idle, _)| idle),
        Some(true)
    );
    assert_eq!(snapshot.tenants["box-b"].idle, None);
    // No token travels in a snapshot: the whitelist's files stay the only
    // place a secret lives.
    let json = serde_json::to_string(&snapshot).expect("serialise");
    assert!(
        !json.contains("token"),
        "snapshot carries a token field: {json}"
    );

    // Age box-b past the TTL before the trip, as a long silence would.
    snapshot
        .tenants
        .get_mut("box-b")
        .expect("box-b")
        .seen_ago_secs = 121;

    let second = Registry::new(dir.join("b.json"), RANGE);
    second.import(snapshot, ttl).await;
    let views = second.views().await;
    assert_eq!(views.len(), 1, "box-b should have been pruned on import");
    assert_eq!(views[0].id, "box-a");
    assert_eq!(
        views[0].rathole_port, 50000,
        "the port must survive the trip"
    );
    assert!(
        second.idle_nodes(ttl).await.contains("box-a"),
        "a fresh idle report must survive the trip"
    );
    // The next free port is allocated around the imported one.
    assert_eq!(
        second
            .register("box-c", "c.losos.cfd")
            .await
            .expect("register c"),
        50001
    );
}

/// Import validates what it loads the way `load` does: a port outside the
/// range and a malformed window are dropped, not trusted.
#[tokio::test]
async fn import_drops_what_load_would_drop() {
    use losos_registrar::{ComputeWindow, Snapshot, SnapshotTenant};
    let ttl = std::time::Duration::from_secs(120);
    let dir = TempDir::new("snapshot-validate");
    let registry = Registry::new(dir.join("r.json"), RANGE);
    let mut snapshot = Snapshot::default();
    snapshot.tenants.insert(
        "out-of-range".to_string(),
        SnapshotTenant {
            hostname: "x.losos.cfd".to_string(),
            rathole_port: 1,
            seen_ago_secs: 0,
            idle: None,
            via: None,
        },
    );
    snapshot.tenants.insert(
        "fine".to_string(),
        SnapshotTenant {
            hostname: "fine.losos.cfd".to_string(),
            rathole_port: 50007,
            seen_ago_secs: 0,
            idle: Some((true, 0)),
            via: None,
        },
    );
    snapshot.compute_windows.insert(
        "fine".to_string(),
        ComputeWindow {
            share_compute: true,
            window_start: "25:00".to_string(),
            window_end: "07:00".to_string(),
            tz: "UTC".to_string(),
        },
    );
    registry.import(snapshot, ttl).await;
    let views = registry.views().await;
    assert_eq!(views.len(), 1);
    assert_eq!(
        (views[0].id.as_str(), views[0].rathole_port),
        ("fine", 50007)
    );
    assert!(registry.compute_windows().await.is_empty());
}
