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
