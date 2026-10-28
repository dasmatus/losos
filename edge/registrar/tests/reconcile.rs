//! The reconciler's two contracts: it keeps running when one tenant is broken,
//! and the operator whitelist — not the registry — decides hostnames.
//!
//! Both are asserted against what a real `serve` publishes: the document its
//! provider listener hands Traefik, which is what Traefik routes on and what
//! it requests certificates for, and the `server.toml` it writes, which is
//! what rathole binds tunnels from.

mod common;

use common::{Edge, TenantSpec, GOOD_TOKEN, OTHER_TOKEN};

/// One unreadable token file must cost exactly one tenant, not the whole pass.
///
/// The enrichment loop used to `?`-propagate the token read, so a single
/// missing `/var/secrets` file aborted reconciliation before either config
/// file was written — and, because the next tick read the same missing file,
/// it aborted identically every 15 seconds forever. Registration, hostname
/// changes and pruning froze for *every* tenant on the edge until an operator
/// noticed. Here `beta` loses its secret mid-run and `alpha` must keep both
/// its Traefik router and its rathole service.
#[tokio::test]
async fn one_broken_tenant_does_not_stop_the_pass() {
    let edge = Edge::start(
        "reconcile-partial",
        &[
            TenantSpec::new("alpha", "alpha.losos.cfd", GOOD_TOKEN),
            TenantSpec::new("beta", "beta.losos.cfd", OTHER_TOKEN),
        ],
    )
    .await;

    let (status, body) = edge.register("alpha", "alpha.losos.cfd", GOOD_TOKEN).await;
    assert_eq!(status, 200, "body: {body}");
    let (status, body) = edge.register("beta", "beta.losos.cfd", OTHER_TOKEN).await;
    assert_eq!(status, 200, "body: {body}");

    assert!(
        edge.wait_for(|| edge
            .rathole_toml()
            .is_some_and(|t| t.contains("[server.services.beta]")))
            .await,
        "beta never made it into the rathole config to begin with"
    );

    // The operator's secret disappears — a truncated deploy, a wiped /var.
    std::fs::remove_file(edge.token_path("beta")).expect("remove beta's token file");

    assert!(
        edge.wait_for(|| edge
            .rathole_toml()
            .is_some_and(|t| !t.contains("[server.services.beta]")))
            .await,
        "the reconciler never ran again after beta's token file vanished: {:?}",
        edge.rathole_toml()
    );

    let rathole = edge.rathole_toml().expect("server.toml exists");
    assert!(
        rathole.contains("[server.services.alpha]"),
        "alpha lost its tunnel because of beta's fault: {rathole}"
    );
    let traefik = edge.traefik_yaml().expect("routes are published");
    assert!(
        traefik.contains("alpha.losos.cfd"),
        "alpha lost its router because of beta's fault: {traefik}"
    );
    assert!(
        !traefik.contains("beta.losos.cfd"),
        "beta kept a router with no usable token: {traefik}"
    );

    // A tenant the reconciler skipped is still a tenant: it authenticates
    // again the moment the operator puts the secret back.
    assert_eq!(edge.heartbeat("alpha", GOOD_TOKEN).await.0, 204);

    edge.shutdown().await;
}

/// The whitelist is the hostname authority, and it applies on the next tick.
///
/// The reconciler used to look a tenant up in `tenants.json` purely as a
/// membership check and then build the router from the registry's stored
/// hostname, so an operator hostname change sat inert until the appliance
/// happened to re-register. Worse, it meant anything that reached
/// `registry.json` — a restored backup, a hand-edit — became a live router and
/// an ACME request for a hostname the operator never listed.
#[tokio::test]
async fn a_whitelist_hostname_change_lands_without_re_registration() {
    let edge = Edge::start(
        "reconcile-hostname",
        &[TenantSpec::new("mattbox", "old.losos.cfd", GOOD_TOKEN)],
    )
    .await;

    let (status, body) = edge.register("mattbox", "old.losos.cfd", GOOD_TOKEN).await;
    assert_eq!(status, 200, "body: {body}");
    assert!(
        edge.wait_for(|| edge
            .traefik_yaml()
            .is_some_and(|y| y.contains("old.losos.cfd")))
            .await,
        "the original hostname never reached Traefik"
    );

    // The operator renames the appliance. No appliance-side action follows —
    // the box keeps heartbeating with the id it already has.
    edge.rewrite_tenants(&[TenantSpec::new("mattbox", "renamed.losos.cfd", GOOD_TOKEN)]);

    assert!(
        edge.wait_for(|| edge
            .traefik_yaml()
            .is_some_and(|y| y.contains("renamed.losos.cfd")))
            .await,
        "the whitelist hostname never took effect: {:?}",
        edge.traefik_yaml()
    );
    let traefik = edge.traefik_yaml().expect("routes are published");
    assert!(
        !traefik.contains("old.losos.cfd"),
        "the stale hostname still has a router (and so still requests a cert): {traefik}"
    );
    assert!(
        traefik.contains("Host(`renamed.losos.cfd`)"),
        "the router rule was not rewritten: {traefik}"
    );

    // The registry entry is repaired too, so the disagreement does not come
    // back after a restart.
    assert!(
        edge.wait_for(|| edge
            .registry_json()
            .is_some_and(|r| r.contains("renamed.losos.cfd") && !r.contains("old.losos.cfd")))
            .await,
        "the registry kept the stale hostname: {:?}",
        edge.registry_json()
    );

    // And the appliance may now only claim the new name.
    assert_eq!(
        edge.register("mattbox", "old.losos.cfd", GOOD_TOKEN)
            .await
            .0,
        403
    );
    assert_eq!(
        edge.register("mattbox", "renamed.losos.cfd", GOOD_TOKEN)
            .await
            .0,
        200
    );

    edge.shutdown().await;
}

/// Dropping a tenant from the whitelist removes its route on the next tick.
#[tokio::test]
async fn a_tenant_removed_from_the_whitelist_loses_its_route() {
    let edge = Edge::start(
        "reconcile-dropped",
        &[
            TenantSpec::new("alpha", "alpha.losos.cfd", GOOD_TOKEN),
            TenantSpec::new("beta", "beta.losos.cfd", OTHER_TOKEN),
        ],
    )
    .await;

    assert_eq!(
        edge.register("alpha", "alpha.losos.cfd", GOOD_TOKEN)
            .await
            .0,
        200
    );
    assert_eq!(
        edge.register("beta", "beta.losos.cfd", OTHER_TOKEN).await.0,
        200
    );
    assert!(
        edge.wait_for(|| edge
            .traefik_yaml()
            .is_some_and(|y| y.contains("beta.losos.cfd")))
            .await,
        "beta never got a router"
    );

    edge.rewrite_tenants(&[TenantSpec::new("alpha", "alpha.losos.cfd", GOOD_TOKEN)]);

    assert!(
        edge.wait_for(|| edge
            .traefik_yaml()
            .is_some_and(|y| !y.contains("beta.losos.cfd")))
            .await,
        "beta kept its router after leaving the whitelist: {:?}",
        edge.traefik_yaml()
    );
    assert!(
        edge.traefik_yaml()
            .is_some_and(|y| y.contains("alpha.losos.cfd")),
        "alpha was collateral damage"
    );
    // De-whitelisted means unauthenticated, immediately.
    assert_eq!(edge.heartbeat("beta", OTHER_TOKEN).await.0, 401);

    edge.shutdown().await;
}

/// With no live tenants Traefik must be handed `{}` rather than an empty
/// `http` section, which it refuses while keeping the old routes, and rathole
/// still gets a complete zero-tenant `[server]` block so it can start.
#[tokio::test]
async fn the_zero_tenant_state_routes_nothing() {
    let edge = Edge::start(
        "reconcile-zero",
        &[TenantSpec::new("mattbox", "mattbox.losos.cfd", GOOD_TOKEN)],
    )
    .await;

    assert!(
        edge.traefik_yaml().is_none(),
        "no appliance has registered, so there must be no route"
    );
    let rathole = edge
        .rathole_toml()
        .expect("server.toml is written at startup");
    assert!(rathole.contains("[server.services]"), "got: {rathole}");
    assert!(
        rathole.contains(&format!("default_token = \"{}\"", common::BOOTSTRAP_TOKEN)),
        "got: {rathole}"
    );

    assert_eq!(
        edge.register("mattbox", "mattbox.losos.cfd", GOOD_TOKEN)
            .await
            .0,
        200
    );
    assert!(
        edge.wait_for(|| edge.traefik_yaml().is_some()).await,
        "registering did not produce a route"
    );

    let (status, _) = edge
        .post(
            "/unregister",
            serde_json::json!({ "appliance_id": "mattbox", "token": GOOD_TOKEN }),
        )
        .await;
    assert_eq!(status, 204);
    assert!(
        edge.wait_for(|| edge.traefik_yaml().is_none()).await,
        "a route survived the last tenant leaving: {:?}",
        edge.traefik_yaml()
    );

    edge.shutdown().await;
}

/// Traefik's side of it, over the real listener: the routes as JSON, the
/// boxes' routers behind the offline page, and that page in the visitor's
/// language with Traefik's status.
#[tokio::test]
async fn traefik_reads_the_routes_and_the_offline_page_from_the_provider_listener() {
    let edge = Edge::start(
        "reconcile-provider",
        &[TenantSpec::new("mattbox", "mattbox.losos.cfd", GOOD_TOKEN)],
    )
    .await;
    let get = |path: &str| {
        let url = format!("{}{path}", edge.provider_base);
        let client = edge.client.clone();
        async move {
            client
                .get(url)
                .header("host", "mattbox.losos.cfd")
                .header("accept-language", "sk,en;q=0.5")
                .send()
                .await
                .expect("provider answers")
        }
    };

    // No box yet: `{}`, which Traefik takes as "no routes".
    let r = get("/traefik").await;
    assert_eq!(r.status(), 200);
    assert_eq!(r.text().await.unwrap(), "{}");

    let (status, body) = edge
        .register("mattbox", "mattbox.losos.cfd", GOOD_TOKEN)
        .await;
    assert_eq!(status, 200, "body: {body}");
    assert!(edge.wait_for(|| edge.traefik_yaml().is_some()).await);
    let r = get("/traefik").await;
    assert_eq!(r.headers()["content-type"], "application/json");
    let doc: serde_json::Value = r.json().await.expect("the document is JSON");
    let router = &doc["http"]["routers"]["mattbox"];
    assert_eq!(router["rule"], "Host(`mattbox.losos.cfd`)");
    assert_eq!(router["middlewares"][0], "losos_offline");
    let errors = &doc["http"]["middlewares"]["losos_offline"]["errors"];
    assert_eq!(errors["status"], serde_json::json!(["502", "504"]));
    assert_eq!(errors["query"], "/offline/{status}");
    assert_eq!(
        doc["http"]["services"]["losos_offline"]["loadBalancer"]["servers"][0]["url"],
        edge.provider_base.as_str()
    );

    let r = get("/offline/504").await;
    assert_eq!(r.status(), 504);
    assert_eq!(r.headers()["cache-control"], "no-store");
    let page = r.text().await.unwrap();
    assert!(page.contains("<html lang=\"sk\">"), "{page}");
    assert!(
        page.contains("<strong>mattbox.losos.cfd</strong>"),
        "{page}"
    );
    // Anything else is the gateway's own error.
    assert_eq!(get("/offline/418").await.status(), 502);

    edge.shutdown().await;
}
