//! Closed-enrollment authentication, driven over a real socket.
//!
//! The registrar's `/register`, `/heartbeat` and `/deregister` routes are
//! reachable from the public internet (`modules/edge.nix` puts a Traefik
//! router on `register.<domain>` with no path rule and no middleware), so
//! "who is allowed through" is the crate's single most load-bearing property.
//! Every case below asserts against the real HTTP response of a real running
//! `serve`, not against a helper's return value.

mod common;

use common::{Edge, TenantSpec, GOOD_TOKEN, OTHER_TOKEN};

/// The whole point of the whitelist: a valid id + its real token gets a port.
#[tokio::test]
async fn a_whitelisted_appliance_with_its_token_registers() {
    let edge = Edge::start(
        "auth-ok",
        &[TenantSpec::new("mattbox", "mattbox.losos.cfd", GOOD_TOKEN)],
    )
    .await;

    let (status, body) = edge
        .register("mattbox", "mattbox.losos.cfd", GOOD_TOKEN)
        .await;
    assert_eq!(status, 200, "body: {body}");
    let parsed: serde_json::Value = serde_json::from_str(&body).expect("register returns JSON");
    let port = parsed["rathole_port"].as_u64().expect("rathole_port");
    assert!(
        (50000..=50100).contains(&port),
        "port {port} outside the configured range"
    );

    let (status, _) = edge.heartbeat("mattbox", GOOD_TOKEN).await;
    assert_eq!(status, 204);

    edge.shutdown().await;
}

/// A zero-byte token file must never authenticate anyone.
///
/// Token files are hand-placed; nothing in the flake creates them, so a
/// truncated or not-yet-written file is a realistic state. Before this guard,
/// `expected.trim()` was `""` and a request supplying `"token": ""` compared
/// equal — an unauthenticated caller on the public internet could register as
/// any tenant whose secret had not landed, taking over its Traefik router,
/// its Let's Encrypt cert and its rathole service.
#[tokio::test]
async fn an_empty_token_file_authenticates_nobody() {
    let edge = Edge::start(
        "auth-empty",
        &[TenantSpec::new("mattbox", "mattbox.losos.cfd", "")],
    )
    .await;

    for supplied in ["", " ", "\n", GOOD_TOKEN] {
        let (status, body) = edge
            .register("mattbox", "mattbox.losos.cfd", supplied)
            .await;
        assert_eq!(
            status, 401,
            "empty token file accepted token {supplied:?}; body: {body}"
        );
    }

    assert!(
        edge.registry_json().is_none(),
        "a rejected registration must not reach the registry"
    );
    edge.shutdown().await;
}

/// Whitespace-only is the same fault as empty: `trim()` reduces both to "".
#[tokio::test]
async fn a_whitespace_only_token_file_authenticates_nobody() {
    let edge = Edge::start(
        "auth-blank",
        &[TenantSpec::new("mattbox", "mattbox.losos.cfd", "   \n\t  ")],
    )
    .await;

    let (status, body) = edge
        .register("mattbox", "mattbox.losos.cfd", "   \n\t  ")
        .await;
    assert_eq!(status, 401, "body: {body}");
    let (status, _) = edge.register("mattbox", "mattbox.losos.cfd", "").await;
    assert_eq!(status, 401);

    edge.shutdown().await;
}

/// A short token file is an operator fault, not a credential — even when the
/// caller supplies exactly what the file contains.
#[tokio::test]
async fn a_short_token_file_is_refused_even_when_it_matches() {
    let edge = Edge::start(
        "auth-short",
        &[TenantSpec::new("mattbox", "mattbox.losos.cfd", "hunter2")],
    )
    .await;

    let (status, body) = edge
        .register("mattbox", "mattbox.losos.cfd", "hunter2")
        .await;
    assert_eq!(status, 401, "body: {body}");

    edge.shutdown().await;
}

/// Closed enrollment: an id that is not in `tenants.json` is refused whatever
/// token it presents, and — because the unknown-id branch now pays the same
/// token-file read the known-id branch does — without an obvious timing tell.
#[tokio::test]
async fn an_unknown_appliance_id_is_refused() {
    let edge = Edge::start(
        "auth-unknown",
        &[TenantSpec::new("mattbox", "mattbox.losos.cfd", GOOD_TOKEN)],
    )
    .await;

    for token in [GOOD_TOKEN, OTHER_TOKEN, ""] {
        let (status, body) = edge.register("intruder", "intruder.example", token).await;
        assert_eq!(status, 401, "unknown id accepted; body: {body}");
    }
    let (status, _) = edge.heartbeat("intruder", GOOD_TOKEN).await;
    assert_eq!(status, 401);

    edge.shutdown().await;
}

/// A known id with the wrong token is refused, and is indistinguishable from
/// an unknown id in both status and body.
#[tokio::test]
async fn a_known_id_with_the_wrong_token_is_refused() {
    let edge = Edge::start(
        "auth-wrong",
        &[TenantSpec::new("mattbox", "mattbox.losos.cfd", GOOD_TOKEN)],
    )
    .await;

    let (known_status, known_body) = edge
        .register("mattbox", "mattbox.losos.cfd", OTHER_TOKEN)
        .await;
    assert_eq!(known_status, 401);

    // A near-miss (one byte short) must not be treated as a prefix match.
    let truncated = &GOOD_TOKEN[..GOOD_TOKEN.len() - 1];
    let (status, _) = edge
        .register("mattbox", "mattbox.losos.cfd", truncated)
        .await;
    assert_eq!(status, 401);

    let (unknown_status, unknown_body) = edge
        .register("nosuchbox", "mattbox.losos.cfd", OTHER_TOKEN)
        .await;
    assert_eq!(
        (known_status, known_body),
        (unknown_status, unknown_body),
        "a wrong token and an unknown id must be indistinguishable to the caller"
    );

    edge.shutdown().await;
}

/// A missing token file is an operator fault; it must read as "unauthorized",
/// not as a 500 that tells the caller the secret is absent on the edge.
#[tokio::test]
async fn a_missing_token_file_reads_as_unauthorized() {
    let edge = Edge::start(
        "auth-missing",
        &[TenantSpec::without_token_file(
            "mattbox",
            "mattbox.losos.cfd",
        )],
    )
    .await;

    let (status, body) = edge
        .register("mattbox", "mattbox.losos.cfd", GOOD_TOKEN)
        .await;
    assert_eq!(status, 401, "body: {body}");
    assert!(
        !body.contains("No such file") && !body.contains(".token"),
        "the response leaked a server-side path: {body}"
    );

    edge.shutdown().await;
}

/// Authentication is not authorisation over hostnames: a tenant may only
/// claim the hostname the operator whitelisted for it.
#[tokio::test]
async fn a_tenant_cannot_claim_another_hostname() {
    let edge = Edge::start(
        "auth-hostname",
        &[TenantSpec::new("mattbox", "mattbox.losos.cfd", GOOD_TOKEN)],
    )
    .await;

    let (status, body) = edge
        .register("mattbox", "victim.losos.cfd", GOOD_TOKEN)
        .await;
    assert_eq!(status, 403, "body: {body}");
    assert!(
        edge.traefik_yaml().is_none(),
        "a refused hostname must never reach the Traefik config (and so never request a cert)"
    );

    edge.shutdown().await;
}

/// `/health` is the one unauthenticated route, and stays that way.
#[tokio::test]
async fn health_needs_no_token() {
    let edge = Edge::start(
        "auth-health",
        &[TenantSpec::new("mattbox", "mattbox.losos.cfd", GOOD_TOKEN)],
    )
    .await;

    let response = edge
        .client
        .get(format!("{}/health", edge.base))
        .send()
        .await
        .expect("health responds");
    assert_eq!(response.status().as_u16(), 200);
    assert_eq!(response.text().await.unwrap_or_default(), "ok");

    edge.shutdown().await;
}

/// The upload endpoints are gone. They were never in the approved design, they
/// fed an untrusted body to `serde_yaml` on the public edge, and they wrote it
/// to disk as root — all to return a summary of what the caller had just sent.
#[tokio::test]
async fn the_removed_upload_endpoints_are_not_routed() {
    let edge = Edge::start(
        "auth-noupload",
        &[TenantSpec::new("mattbox", "mattbox.losos.cfd", GOOD_TOKEN)],
    )
    .await;

    for path in ["/config", "/tahoe"] {
        let response = edge
            .client
            .post(format!("{}{path}", edge.base))
            .header("x-appliance-id", "mattbox")
            .header("x-appliance-token", GOOD_TOKEN)
            .body("http: {}")
            .send()
            .await
            .expect("request reaches the registrar");
        assert_eq!(response.status().as_u16(), 404, "{path} is still routed");
    }

    edge.shutdown().await;
}
