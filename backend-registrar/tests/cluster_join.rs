//! `/cluster/join` — mesh enrolment, driven over a real socket.
//!
//! The route is on the public internet for the same reason every other route
//! here is (`modules/edge.nix` gives Traefik a `Host(register.<domain>)` router
//! with no path rule), and it hands out the credential that lets a box run a
//! kubelet on the edge's cluster. So the cases below are mostly about who is
//! refused, and about what has to happen *before* anyone is accepted.
//!
//! The one thing these tests cannot cover is the real rke2 apiserver: the
//! cleanup is asserted against a stub that records the requests
//! (`common::KubeStub`). Whether rke2 actually stores a node-password `Secret`
//! under that name, and whether deleting it really lets a wiped box rejoin,
//! belongs to `tests/edge-vm.nix` and `tests/cluster-vm.nix`.

mod common;

use common::{Edge, KubeStub, MeshFixture, TenantSpec, GOOD_TOKEN, MESH_AGENT_TOKEN, OTHER_TOKEN};

/// The happy path, and the two things it must do in order.
///
/// The response carries the supervisor address and the node token; the stale
/// `Node` and node-password `Secret` are deleted first. The ordering is the
/// whole point of the route: handing out a token *without* the cleanup is what
/// permanently locks a factory-reset box out of the cluster, because the box
/// regenerates `/etc/rancher/node/password` on its tmpfs root and rke2 then
/// refuses the rejoin as a duplicate hostname.
#[tokio::test]
async fn a_cleared_tenant_joins_and_the_stale_node_is_removed_first() {
    // 404 from the apiserver: nothing to clean, which is the first-join case
    // and must read as success.
    let kube = KubeStub::expecting_bearer(404, common::KUBE_TOKEN).await;
    let edge = Edge::start_with_mesh(
        "join-ok",
        &[TenantSpec::new("mattbox-01", "mattbox-01.losos.cfd", GOOD_TOKEN).with_cluster()],
        MeshFixture::enabled(&kube.base),
    )
    .await;

    let (status, body) = edge.join("mattbox-01", "mattbox-01", GOOD_TOKEN).await;
    assert_eq!(status, 200, "body: {body}");
    let parsed: serde_json::Value = serde_json::from_str(&body).expect("join returns JSON");
    assert_eq!(parsed["token"], MESH_AGENT_TOKEN);
    assert_eq!(parsed["server_addr"], "https://198.51.100.7:9345");
    assert_eq!(parsed["node_name"], "mattbox-01");

    assert_eq!(
        kube.seen(),
        vec![
            "DELETE /api/v1/nodes/mattbox-01".to_string(),
            "DELETE /api/v1/namespaces/kube-system/secrets/mattbox-01.node-password.rke2"
                .to_string(),
        ],
        "the cleanup did not delete both objects, in order"
    );

    edge.shutdown().await;
    kube.shutdown().await;
}

/// The reinstall path: joining twice with the same id succeeds, and the second
/// join cleans up again. This is what `losos-ctl factory-reset` and a reinstall
/// from the ISO both look like from the edge's side.
#[tokio::test]
async fn a_second_join_for_the_same_id_succeeds() {
    // 200 from the apiserver: the node object existed and was deleted.
    let kube = KubeStub::start(200).await;
    let edge = Edge::start_with_mesh(
        "join-again",
        &[TenantSpec::new("mattbox-01", "mattbox-01.losos.cfd", GOOD_TOKEN).with_cluster()],
        MeshFixture::enabled(&kube.base),
    )
    .await;

    assert_eq!(
        edge.join("mattbox-01", "mattbox-01", GOOD_TOKEN).await.0,
        200
    );
    let (status, body) = edge.join("mattbox-01", "mattbox-01", GOOD_TOKEN).await;
    assert_eq!(status, 200, "the rejoin was refused; body: {body}");
    assert_eq!(
        kube.seen().len(),
        4,
        "each join must delete both objects: {:?}",
        kube.seen()
    );

    edge.shutdown().await;
    kube.shutdown().await;
}

/// Proxy membership does not imply mesh membership. A tenant without
/// `losos.edge.tenants.<id>.cluster` authenticates perfectly well and is still
/// refused — and gets no node token out of the exchange.
#[tokio::test]
async fn a_tenant_without_the_cluster_flag_is_refused() {
    let kube = KubeStub::start(404).await;
    let edge = Edge::start_with_mesh(
        "join-noflag",
        &[TenantSpec::new(
            "mattbox-01",
            "mattbox-01.losos.cfd",
            GOOD_TOKEN,
        )],
        MeshFixture::enabled(&kube.base),
    )
    .await;

    let (status, body) = edge.join("mattbox-01", "mattbox-01", GOOD_TOKEN).await;
    assert_eq!(status, 403, "body: {body}");
    assert!(
        !body.contains(MESH_AGENT_TOKEN),
        "a refused join leaked the node token: {body}"
    );
    assert!(
        kube.seen().is_empty(),
        "a refused join touched the apiserver: {:?}",
        kube.seen()
    );

    edge.shutdown().await;
    kube.shutdown().await;
}

/// The gate that makes the delete safe. Without it any cleared tenant could
/// name a neighbour's node and have the edge evict it from the cluster on the
/// neighbour's behalf.
#[tokio::test]
async fn a_tenant_cannot_join_under_another_nodes_name() {
    let kube = KubeStub::start(404).await;
    let edge = Edge::start_with_mesh(
        "join-nodename",
        &[
            TenantSpec::new("mattbox-01", "mattbox-01.losos.cfd", GOOD_TOKEN).with_cluster(),
            TenantSpec::new("mattbox-02", "mattbox-02.losos.cfd", OTHER_TOKEN).with_cluster(),
        ],
        MeshFixture::enabled(&kube.base),
    )
    .await;

    let (status, body) = edge.join("mattbox-01", "mattbox-02", GOOD_TOKEN).await;
    assert_eq!(status, 403, "body: {body}");
    assert!(
        kube.seen().is_empty(),
        "the victim's node object was touched anyway: {:?}",
        kube.seen()
    );

    edge.shutdown().await;
    kube.shutdown().await;
}

/// Authentication is unchanged: the join route reuses the per-appliance token
/// and mints nothing new, so an unknown id and a wrong token are 401 here for
/// exactly the reasons they are on `/register`.
#[tokio::test]
async fn an_unauthenticated_join_is_refused() {
    let kube = KubeStub::start(404).await;
    let edge = Edge::start_with_mesh(
        "join-auth",
        &[TenantSpec::new("mattbox-01", "mattbox-01.losos.cfd", GOOD_TOKEN).with_cluster()],
        MeshFixture::enabled(&kube.base),
    )
    .await;

    for (id, token) in [
        ("mattbox-01", OTHER_TOKEN),
        ("mattbox-01", ""),
        ("intruder", GOOD_TOKEN),
    ] {
        let (status, body) = edge.join(id, id, token).await;
        assert_eq!(status, 401, "{id:?} got in; body: {body}");
    }
    assert!(kube.seen().is_empty());

    edge.shutdown().await;
    kube.shutdown().await;
}

/// A browser is not a trust boundary, and neither is lososd. The window bounds
/// end up in a file the edge's timer turns into `kubectl taint` arguments, so
/// a malformed one is a 400 rather than something clamped into range.
#[tokio::test]
async fn a_malformed_window_is_rejected_not_clamped() {
    let kube = KubeStub::start(404).await;
    let edge = Edge::start_with_mesh(
        "join-window",
        &[TenantSpec::new("mattbox-01", "mattbox-01.losos.cfd", GOOD_TOKEN).with_cluster()],
        MeshFixture::enabled(&kube.base),
    )
    .await;

    for (start, end) in [
        ("24:00", "07:00"),
        ("23:00", "23:60"),
        ("7:00", "07:00"),
        ("23:00", "0700"),
        ("", "07:00"),
        ("23:00 ; kubectl delete node mattbox-02", "07:00"),
    ] {
        let (status, body) = edge
            .post(
                "/cluster/join",
                serde_json::json!({
                    "appliance_id": "mattbox-01",
                    "token": GOOD_TOKEN,
                    "node_name": "mattbox-01",
                    "share_compute": true,
                    "window_start": start,
                    "window_end": end,
                }),
            )
            .await;
        assert_eq!(status, 400, "{start:?}-{end:?} was accepted; body: {body}");
    }
    assert!(
        kube.seen().is_empty(),
        "a malformed window still reached the apiserver: {:?}",
        kube.seen()
    );

    edge.shutdown().await;
    kube.shutdown().await;
}

/// An edge with `losos.edge.cluster.enable = false` runs `serve` without the
/// mesh flags. The route still exists — one binary, one argument list — and
/// answers 503, while everything the master proxy does keeps working.
#[tokio::test]
async fn an_edge_without_a_mesh_answers_503_and_still_proxies() {
    let edge = Edge::start(
        "join-nomesh",
        &[TenantSpec::new("mattbox-01", "mattbox-01.losos.cfd", GOOD_TOKEN).with_cluster()],
    )
    .await;

    let (status, body) = edge.join("mattbox-01", "mattbox-01", GOOD_TOKEN).await;
    assert_eq!(status, 503, "body: {body}");

    // The proxy half is untouched.
    let (status, body) = edge
        .register("mattbox-01", "mattbox-01.losos.cfd", GOOD_TOKEN)
        .await;
    assert_eq!(status, 200, "body: {body}");

    edge.shutdown().await;
}

/// A cleanup that fails is a join that fails. Handing out a node token after a
/// failed cleanup reproduces the exact permanent lockout the cleanup exists to
/// prevent, so an apiserver that is down (or refusing the registrar's
/// ServiceAccount) must not be papered over.
#[tokio::test]
async fn a_failed_cleanup_refuses_the_join_rather_than_handing_out_a_token() {
    // 403: the ServiceAccount lost its RBAC binding. Not 404, which is the one
    // non-2xx the handler is allowed to treat as success.
    let kube = KubeStub::start(403).await;
    let edge = Edge::start_with_mesh(
        "join-cleanup-fails",
        &[TenantSpec::new("mattbox-01", "mattbox-01.losos.cfd", GOOD_TOKEN).with_cluster()],
        MeshFixture::enabled(&kube.base),
    )
    .await;

    let (status, body) = edge.join("mattbox-01", "mattbox-01", GOOD_TOKEN).await;
    assert_eq!(status, 503, "body: {body}");
    assert!(
        !body.contains(MESH_AGENT_TOKEN),
        "a failed cleanup still handed out the node token: {body}"
    );
    // The apiserver's URL and status are the operator's diagnostic, not the
    // caller's: they go to the journal, never into the response.
    assert!(
        !body.contains("127.0.0.1") && !body.contains("403") && !body.contains("/api/v1/"),
        "the 503 body leaked the control plane's address: {body}"
    );

    edge.shutdown().await;
    kube.shutdown().await;
}

/// The window an appliance sent has to reach the file the edge's taint timer
/// reads, or the whole opt-in is decoration.
#[tokio::test]
async fn an_accepted_join_publishes_the_compute_window() {
    let kube = KubeStub::start(404).await;
    let edge = Edge::start_with_mesh(
        "join-windowfile",
        &[TenantSpec::new("mattbox-01", "mattbox-01.losos.cfd", GOOD_TOKEN).with_cluster()],
        MeshFixture::enabled(&kube.base),
    )
    .await;

    assert_eq!(
        edge.join("mattbox-01", "mattbox-01", GOOD_TOKEN).await.0,
        200
    );
    assert!(
        edge.wait_for(|| edge
            .compute_windows()
            .is_some_and(|w| !w["nodes"].as_array().expect("nodes is a list").is_empty()))
            .await,
        "the window never reached compute-windows.json: {:?}",
        edge.compute_windows()
    );

    let published = edge.compute_windows().expect("compute-windows.json exists");
    let node = &published["nodes"][0];
    assert_eq!(node["node_name"], "mattbox-01");
    assert_eq!(node["share_compute"], true);
    assert_eq!(node["window_start"], "23:00");
    assert_eq!(node["window_end"], "07:00");

    edge.shutdown().await;
    kube.shutdown().await;
}

/// A window outlives the heartbeat TTL by design — nothing about it expires —
/// so the operator whitelist has to be what ages it out, as it is for
/// hostnames and tunnels. Otherwise a de-whitelisted tenant keeps steering the
/// taint on its node and the map only ever grows.
#[tokio::test]
async fn a_de_whitelisted_tenant_loses_its_published_window() {
    let kube = KubeStub::start(404).await;
    let edge = Edge::start_with_mesh(
        "join-dropwindow",
        &[
            TenantSpec::new("alpha", "alpha.losos.cfd", GOOD_TOKEN).with_cluster(),
            TenantSpec::new("beta", "beta.losos.cfd", OTHER_TOKEN).with_cluster(),
        ],
        MeshFixture::enabled(&kube.base),
    )
    .await;

    assert_eq!(edge.join("alpha", "alpha", GOOD_TOKEN).await.0, 200);
    assert_eq!(edge.join("beta", "beta", OTHER_TOKEN).await.0, 200);
    assert!(
        edge.wait_for(|| edge
            .compute_windows()
            .is_some_and(|w| w["nodes"].as_array().is_some_and(|n| n.len() == 2)))
            .await,
        "both windows never landed: {:?}",
        edge.compute_windows()
    );

    edge.rewrite_tenants(&[TenantSpec::new("alpha", "alpha.losos.cfd", GOOD_TOKEN).with_cluster()]);

    assert!(
        edge.wait_for(|| edge
            .compute_windows()
            .is_some_and(|w| w["nodes"].as_array().is_some_and(|n| n.len() == 1)))
            .await,
        "beta kept its window after leaving the whitelist: {:?}",
        edge.compute_windows()
    );
    let published = edge.compute_windows().expect("compute-windows.json exists");
    assert_eq!(published["nodes"][0]["node_name"], "alpha");

    // And the drop is durable, not just in memory: the next boot re-reads this.
    assert!(
        edge.registry_json()
            .is_some_and(|r| !r.contains("\"beta\"")),
        "the registry kept beta's window: {:?}",
        edge.registry_json()
    );

    edge.shutdown().await;
    kube.shutdown().await;
}
