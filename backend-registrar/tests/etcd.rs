//! The route table against a real etcd (`crate::routes::Etcd`).
//!
//! The rest of the suite talks to a stand-in that answers the way etcd's v3
//! JSON gateway documents; this file checks the stand-in's assumptions
//! against the real thing: base64 keys, `range_end`, an empty range with no
//! `kvs`, deletes. Gated on `LOSOS_TEST_ETCD_URL` (CI's test job runs an
//! etcd service), so it passes trivially where there is none.

use std::collections::BTreeMap;

use losos_registrar::routes::{row_key, Etcd, RouteRow};

fn etcd() -> Option<Etcd> {
    let url = std::env::var("LOSOS_TEST_ETCD_URL").ok()?;
    Some(Etcd::new(&url).expect("client"))
}

fn row(spoke: &str, tenant: &str, domain: &str) -> RouteRow {
    RouteRow {
        domain: domain.to_string(),
        tenant: tenant.to_string(),
        spoke: spoke.to_string(),
        service: format!("{spoke}.{tenant}"),
    }
}

fn table(prefix: &str, rows: &[RouteRow]) -> BTreeMap<String, RouteRow> {
    rows.iter()
        .map(|r| (row_key(prefix, r), r.clone()))
        .collect()
}

#[tokio::test]
async fn the_table_round_trips_through_a_real_etcd() {
    let Some(etcd) = etcd() else {
        eprintln!("LOSOS_TEST_ETCD_URL unset; skipping");
        return;
    };
    // A prefix of its own per run, and a neighbour that must not be touched.
    let prefix = format!("/losos-test/{}/routes", std::process::id());
    let neighbour = format!("{prefix}x/acme/kept/kept.example.org");
    etcd.put(&neighbour, b"not ours").await.unwrap();

    // Empty: no `kvs` in the reply.
    let (rows, junk) = etcd.rows(&prefix).await.unwrap();
    assert!(rows.is_empty() && junk.is_empty());

    let first = table(
        &prefix,
        &[
            row("acme", "mattbox", "cloud.example.org"),
            row("acme", "mattbox", "0123456789abcdef.boxes.example.test"),
        ],
    );
    assert_eq!(etcd.sync(&prefix, &first).await.unwrap(), first);

    // Junk under the prefix is removed, a moved row is rewritten, a gone one
    // deleted, and the neighbour (a key sharing the prefix's characters but
    // not its directory) survives.
    etcd.put(
        &format!("{prefix}/acme/mattbox/bad.example.org"),
        b"{not json",
    )
    .await
    .unwrap();
    let (_, junk) = etcd.rows(&prefix).await.unwrap();
    assert_eq!(junk.len(), 1);
    let second = table(&prefix, &[row("home", "mattbox", "cloud.example.org")]);
    assert_eq!(etcd.sync(&prefix, &second).await.unwrap(), second);
    let raw = etcd.range_prefix(&format!("{prefix}/")).await.unwrap();
    assert_eq!(raw.len(), 1, "{raw:?}");
    assert!(etcd
        .range_prefix(&neighbour)
        .await
        .unwrap()
        .contains_key(&neighbour));

    assert!(etcd
        .sync(&prefix, &BTreeMap::new())
        .await
        .unwrap()
        .is_empty());
    etcd.delete(&neighbour).await.unwrap();
    assert!(etcd.range_prefix(&neighbour).await.unwrap().is_empty());
}
