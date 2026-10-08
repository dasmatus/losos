//! Integration tests for custom domains on an official edge.
//!
//! A real registrar on a real socket, with its market store seeded (the
//! Stripe data the domains hang off) and a stand-in DNS-over-HTTPS resolver
//! the test fills in, as the owner's DNS provider would. What is asserted is
//! what reaches Traefik and the zone file, and what each box is told.

mod common;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use common::{Edge, TenantSpec, DNS_ZONE, EDGE_IPV4, GOOD_TOKEN, OTHER_TOKEN};
use losos_registrar::domains::{box_label, challenge_token};
use losos_registrar::identity;
use serde_json::{json, Value};
use tokio::sync::oneshot;

const THIRD_TOKEN: &str = "1111222233334444555566667777888811112222333344445555666677778888";
const MATT_UUID: &str = "0b1c2d3e-4f50-4617-8899-aabbccddeeff";
const THIRD_UUID: &str = "11111111-2222-4333-8444-555555555555";

/// `(name, type) → answers`, as the owner's DNS provider would publish them.
type Records = Arc<Mutex<HashMap<(String, String), Vec<(u16, String)>>>>;

struct Doh {
    url: String,
    records: Records,
    stop: Option<oneshot::Sender<()>>,
}

impl Doh {
    async fn start() -> Self {
        let records: Records = Arc::default();
        let app = Router::new()
            .route("/dns-query", get(answer))
            .with_state(records.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let (stop, rx) = oneshot::channel::<()>();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    let _ = rx.await;
                })
                .await;
        });
        Self {
            url: format!("http://127.0.0.1:{port}/dns-query"),
            records,
            stop: Some(stop),
        }
    }

    fn set(&self, name: &str, rtype: &str, answers: &[(u16, &str)]) {
        self.records.lock().unwrap().insert(
            (name.to_string(), rtype.to_string()),
            answers
                .iter()
                .map(|(t, d)| (*t, (*d).to_string()))
                .collect(),
        );
    }
}

impl Drop for Doh {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
}

async fn answer(
    State(records): State<Records>,
    Query(q): Query<HashMap<String, String>>,
) -> Json<Value> {
    let name = q.get("name").cloned().unwrap_or_default();
    let rtype = q.get("type").cloned().unwrap_or_default();
    let found = records.lock().unwrap().get(&(name.clone(), rtype)).cloned();
    match found {
        None => Json(json!({ "Status": 3 })),
        Some(answers) => Json(json!({
            "Status": 0,
            "Answer": answers
                .iter()
                .map(|(t, d)| json!({ "name": name, "type": t, "TTL": 300, "data": d }))
                .collect::<Vec<_>>(),
        })),
    }
}

fn tenants() -> Vec<TenantSpec> {
    vec![
        TenantSpec::new("mattbox", "mattbox.boxes.example.test", GOOD_TOKEN).with_market(),
        TenantSpec::new("otherbox", "otherbox.example.test", OTHER_TOKEN).with_market(),
        TenantSpec::new("thirdbox", "thirdbox.example.test", THIRD_TOKEN).with_market(),
    ]
}

fn market_state() -> Value {
    json!({
        "sellers": {
            "mattbox": { "account_id": "acct_test_1", "ready": true, "box_uuid": MATT_UUID },
            // Onboarding started, Stripe has not finished checking.
            "otherbox": { "account_id": "acct_test_2", "ready": false, "box_uuid": "22222222-3333-4444-8555-666666666666" },
            "thirdbox": { "account_id": "acct_test_3", "ready": true, "box_uuid": THIRD_UUID },
        }
    })
}

/// An identity key and a certificate for it, signed by a throwaway root,
/// written where the edge will read them. Domains need no more than "a
/// certificate is installed and unexpired"; the root signature is the
/// boxes' check.
fn official_identity(dir: &std::path::Path) -> (String, String) {
    let (root_pkcs8, _) = identity::keygen().unwrap();
    let root = identity::load_key(&root_pkcs8).unwrap();
    let (edge_pkcs8, edge_public) = identity::keygen().unwrap();
    let key_file = dir.join("identity.key");
    let cert_file = dir.join("identity.cert.json");
    std::fs::write(&key_file, edge_pkcs8).unwrap();
    let cert = identity::issue(
        &root,
        "test edge",
        "https://register.example.test",
        &edge_public,
        4_000_000_000,
    )
    .unwrap();
    std::fs::write(&cert_file, serde_json::to_vec(&cert).unwrap()).unwrap();
    (
        key_file.to_string_lossy().into_owned(),
        cert_file.to_string_lossy().into_owned(),
    )
}

fn scratch(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!(
        "losos-domains-{tag}-{}-{}",
        std::process::id(),
        losos_registrar::market::now_secs()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn auth(id: &str, token: &str) -> Value {
    json!({ "appliance_id": id, "token": token })
}

fn with(mut v: Value, k: &str, x: &str) -> Value {
    v[k] = json!(x);
    v
}

fn zone(edge: &Edge) -> String {
    std::fs::read_to_string(edge.dir.join("dns/boxes.example.test.zone")).unwrap_or_default()
}

async fn list(edge: &Edge, id: &str, token: &str) -> Value {
    let (status, body) = edge.post("/domains/list", auth(id, token)).await;
    assert_eq!(status, 200, "{body}");
    serde_json::from_str(&body).unwrap()
}

#[tokio::test]
async fn an_edge_that_is_not_official_offers_no_domains() {
    let doh = Doh::start().await;
    let edge = Edge::start_with_domains(
        "dom-unofficial",
        &tenants(),
        &market_state(),
        None,
        &doh.url,
    )
    .await;
    let (status, body) = edge
        .post("/domains/list", auth("mattbox", GOOD_TOKEN))
        .await;
    assert_eq!(status, 503, "{body}");
    assert!(body.contains("not offered"));
    // Unauthenticated callers learn nothing either way.
    let (status, _) = edge
        .post("/domains/list", auth("mattbox", OTHER_TOKEN))
        .await;
    assert_eq!(status, 401);
    // The zone is served, so its delegation answers, but an edge that may
    // not hand out names publishes none: no box label, no tenant hostname.
    edge.register("mattbox", "mattbox.boxes.example.test", GOOD_TOKEN)
        .await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    let z = zone(&edge);
    assert!(z.contains("IN SOA ns1.boxes.example.test."), "{z}");
    assert!(
        !z.contains("mattbox") && !z.contains(&box_label(MATT_UUID)),
        "{z}"
    );
    edge.shutdown().await;
}

#[tokio::test]
async fn a_box_stripe_vouches_for_gets_a_name_and_a_proved_domain_goes_live() {
    let doh = Doh::start().await;
    let scratch = scratch("live");
    let (key, cert) = official_identity(&scratch);
    let edge = Edge::start_with_domains(
        "dom-live",
        &tenants(),
        &market_state(),
        Some((&key, &cert)),
        &doh.url,
    )
    .await;
    let (status, _) = edge
        .register("mattbox", "mattbox.boxes.example.test", GOOD_TOKEN)
        .await;
    assert_eq!(status, 200);

    // The box's name: the label of its UUID, in the zone, at the edge.
    let label = box_label(MATT_UUID);
    let target = format!("{label}.{DNS_ZONE}");
    let v = list(&edge, "mattbox", GOOD_TOKEN).await;
    assert_eq!(v["eligible"], true);
    assert_eq!(v["target"], target.as_str());
    assert_eq!(v["addresses"], json!([EDGE_IPV4]));
    assert!(
        edge.wait_for(|| zone(&edge).contains(&format!("{label} IN A {EDGE_IPV4}")))
            .await,
        "zone:\n{}",
        zone(&edge)
    );
    let z = zone(&edge);
    assert!(z.contains("$ORIGIN boxes.example.test."), "{z}");
    // The whitelisted hostname inside the zone is assigned too, and a box
    // whose Stripe account is not ready gets no label.
    assert!(z.contains(&format!("mattbox IN A {EDGE_IPV4}")), "{z}");
    assert!(
        !z.contains(&box_label("22222222-3333-4444-8555-666666666666")),
        "{z}"
    );
    // Nothing from Stripe is in the zone.
    assert!(!z.contains("acct_") && !z.contains(MATT_UUID), "{z}");
    assert!(
        edge.wait_for(|| edge
            .traefik_yaml()
            .is_some_and(|y| y.contains(&format!("Host(`{target}`)"))))
            .await
    );

    // The owner publishes both records, then adds the domain.
    let domain = "cloud.example.org";
    let token = challenge_token(domain, MATT_UUID, "acct_test_1");
    doh.set(
        &format!("_losos-challenge.{domain}"),
        "TXT",
        &[(16, &format!("\"{token}\""))],
    );
    doh.set(domain, "A", &[(5, &format!("{target}.")), (1, EDGE_IPV4)]);
    let (status, body) = edge
        .post(
            "/domains/add",
            with(auth("mattbox", GOOD_TOKEN), "domain", "Cloud.Example.ORG."),
        )
        .await;
    assert_eq!(status, 201, "{body}");
    let v: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["domains"][0]["domain"], domain);
    assert_eq!(
        v["domains"][0]["txt_name"],
        "_losos-challenge.cloud.example.org"
    );
    assert_eq!(v["domains"][0]["txt_value"], token.as_str());

    assert!(
        edge.wait_for(|| edge
            .traefik_yaml()
            .is_some_and(|y| y.contains("Host(`cloud.example.org`)")))
            .await,
        "{:?}",
        edge.traefik_yaml()
    );
    let v = list(&edge, "mattbox", GOOD_TOKEN).await;
    assert_eq!(v["domains"][0]["status"], "live");
    assert_eq!(v["domains"][0]["txt_found"], true);
    assert_eq!(v["domains"][0]["points_here"], true);
    assert_eq!(v["domains"][0]["problem"], Value::Null);
    let y = edge.traefik_yaml().unwrap();
    assert!(y.contains("mattbox-host-"), "{y}");
    assert!(y.contains("main: cloud.example.org"), "{y}");

    // Another box Stripe also vouches for cannot take it while it is live.
    let (status, body) = edge
        .post(
            "/domains/add",
            with(auth("thirdbox", THIRD_TOKEN), "domain", domain),
        )
        .await;
    assert_eq!(status, 409, "{body}");

    // Removing it takes the router away.
    let (status, body) = edge
        .post(
            "/domains/remove",
            with(auth("mattbox", GOOD_TOKEN), "domain", domain),
        )
        .await;
    assert_eq!(status, 200, "{body}");
    assert!(
        edge.wait_for(|| edge
            .traefik_yaml()
            .is_some_and(|y| !y.contains("cloud.example.org")))
            .await
    );
    edge.shutdown().await;
    let _ = std::fs::remove_dir_all(scratch);
}

#[tokio::test]
async fn a_domain_waits_until_both_records_are_there_and_says_which_is_missing() {
    let doh = Doh::start().await;
    let scratch = scratch("wait");
    let (key, cert) = official_identity(&scratch);
    let edge = Edge::start_with_domains(
        "dom-wait",
        &tenants(),
        &market_state(),
        Some((&key, &cert)),
        &doh.url,
    )
    .await;
    edge.register("mattbox", "mattbox.boxes.example.test", GOOD_TOKEN)
        .await;

    // TXT only: the name still points somewhere else.
    let domain = "www.example.net";
    let token = challenge_token(domain, MATT_UUID, "acct_test_1");
    doh.set(
        &format!("_losos-challenge.{domain}"),
        "TXT",
        &[(16, &format!("\"{token}\""))],
    );
    doh.set(domain, "A", &[(1, "198.51.100.20")]);
    let (status, body) = edge
        .post(
            "/domains/add",
            with(auth("mattbox", GOOD_TOKEN), "domain", domain),
        )
        .await;
    assert_eq!(status, 201, "{body}");
    let mut last = Value::Null;
    let checked = edge
        .wait_for(|| {
            let doms = std::fs::read_to_string(edge.dir.join("domains.json")).unwrap_or_default();
            doms.contains("notPointing")
        })
        .await;
    if checked {
        last = list(&edge, "mattbox", GOOD_TOKEN).await;
    }
    assert_eq!(last["domains"][0]["status"], "waiting", "{last}");
    assert_eq!(last["domains"][0]["problem"], "notPointing", "{last}");
    assert_eq!(last["domains"][0]["txt_found"], true);
    assert!(edge.traefik_yaml().is_some_and(|y| !y.contains(domain)));

    // Someone else's token in the TXT record is not this box's proof.
    let other = "shop.example.net";
    doh.set(
        &format!("_losos-challenge.{other}"),
        "TXT",
        &[(
            16,
            &format!("\"{}\"", challenge_token(other, THIRD_UUID, "acct_test_3")),
        )],
    );
    let (status, _) = edge
        .post(
            "/domains/add",
            with(auth("mattbox", GOOD_TOKEN), "domain", other),
        )
        .await;
    assert_eq!(status, 201);
    assert!(
        edge.wait_for(|| std::fs::read_to_string(edge.dir.join("domains.json"))
            .unwrap_or_default()
            .contains("txtWrong"))
            .await
    );
    edge.shutdown().await;
    let _ = std::fs::remove_dir_all(scratch);
}

#[tokio::test]
async fn without_a_ready_stripe_account_a_box_is_told_why_and_cannot_add() {
    let doh = Doh::start().await;
    let scratch = scratch("stripe");
    let (key, cert) = official_identity(&scratch);
    let edge = Edge::start_with_domains(
        "dom-stripe",
        &tenants(),
        &market_state(),
        Some((&key, &cert)),
        &doh.url,
    )
    .await;
    let v = list(&edge, "otherbox", OTHER_TOKEN).await;
    assert_eq!(v["eligible"], false);
    assert_eq!(v["reason"], "stripeAccount");
    assert_eq!(v["target"], Value::Null);
    let (status, body) = edge
        .post(
            "/domains/add",
            with(auth("otherbox", OTHER_TOKEN), "domain", "a.example.org"),
        )
        .await;
    assert_eq!(status, 409, "{body}");
    assert!(body.contains("Stripe"), "{body}");

    // Names the edge owns, and nonsense, are refused before anything is stored.
    for bad in [
        "thirdbox.example.test",
        "x.boxes.example.test",
        "10.0.0.1",
        "not a name",
    ] {
        let (status, body) = edge
            .post(
                "/domains/add",
                with(auth("mattbox", GOOD_TOKEN), "domain", bad),
            )
            .await;
        assert_eq!(status, 400, "{bad}: {body}");
    }
    edge.shutdown().await;
    let _ = std::fs::remove_dir_all(scratch);
}
