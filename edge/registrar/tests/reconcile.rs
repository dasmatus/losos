//! The reconciler's two contracts: it keeps running when one tenant is broken,
//! and the operator whitelist — not the registry — decides hostnames.
//!
//! Both are asserted against the files a real `serve` writes to a real
//! directory, because those files *are* the product: `losos.yml` is what
//! Traefik routes on and what it requests certificates for, and `tunnel.json`
//! is what the tunnel binds each box's port from.

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
/// its Traefik router and its tunnel service.
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
        edge.wait_for(|| edge.tunnel_json().is_some_and(|t| t.contains("\"beta\"")))
            .await,
        "beta never made it into the tunnel file to begin with"
    );

    // The operator's secret disappears — a truncated deploy, a wiped /var.
    std::fs::remove_file(edge.token_path("beta")).expect("remove beta's token file");

    assert!(
        edge.wait_for(|| edge.tunnel_json().is_some_and(|t| !t.contains("\"beta\"")))
            .await,
        "the reconciler never ran again after beta's token file vanished: {:?}",
        edge.tunnel_json()
    );

    let tunnel = edge.tunnel_json().expect("tunnel.json exists");
    assert!(
        tunnel.contains("\"alpha\""),
        "alpha lost its tunnel because of beta's fault: {tunnel}"
    );
    let traefik = edge.traefik_yaml().expect("losos.yml exists");
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
    let traefik = edge.traefik_yaml().expect("losos.yml exists");
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

/// With no live tenants, `losos.yml` must be absent rather than empty —
/// Traefik's file provider rejects an empty dynamic config — while the
/// tunnel still gets a complete file with no services.
#[tokio::test]
async fn the_zero_tenant_state_removes_the_traefik_file() {
    let edge = Edge::start(
        "reconcile-zero",
        &[TenantSpec::new("mattbox", "mattbox.losos.cfd", GOOD_TOKEN)],
    )
    .await;

    assert!(
        edge.traefik_yaml().is_none(),
        "no appliance has registered, so there must be no losos.yml"
    );
    let tunnel = edge
        .tunnel_json()
        .expect("tunnel.json is written at startup");
    let tunnel: losos_registrar::tunnel::ServerFile =
        serde_json::from_str(&tunnel).expect("a tunnel file");
    assert!(tunnel.services.is_empty(), "got: {tunnel:?}");

    assert_eq!(
        edge.register("mattbox", "mattbox.losos.cfd", GOOD_TOKEN)
            .await
            .0,
        200
    );
    assert!(
        edge.wait_for(|| edge.traefik_yaml().is_some()).await,
        "registering did not produce a Traefik config"
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
        "losos.yml survived the last tenant leaving: {:?}",
        edge.traefik_yaml()
    );

    edge.shutdown().await;
}
