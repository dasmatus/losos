//! Secondary clusters (`crate::secondary`): two gateways on one LAN that
//! serve the same boxes, relaying through one hub.
//!
//! Real registrars on real sockets: the hub, gateway `gw-a` (the founder,
//! which knows no peer) and gateway `gw-b` (joined through `gw-a`). A box is
//! a `/register` call against whichever gateway it uses.

mod common;

use std::time::Duration;

use common::{Edge, TempDir, TenantSpec, GOOD_TOKEN, OTHER_TOKEN};
use losos_registrar::opts::ServeOpts;
use losos_registrar::secondary::{self, SecondaryOpts};
use serde_json::{json, Value};

const ZONE: &str = "acme.losos.cfd";
const SITE_TOKEN: &str = "517e517e517e517e517e517e517e517e517e517e517e517e517e517e517e517e";
/// The pin a gateway starts with for its hub, as if fetched once already.
const HUB_PIN: &str = "5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e";
const SECRET: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

async fn listener() -> (tokio::net::TcpListener, String) {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://127.0.0.1:{}", l.local_addr().unwrap().port());
    (l, url)
}

/// A gateway: open enrolment, an uplink to `hub` under the site's tenant
/// row, and the cluster file naming it `member` of `office` with `peers`.
fn gateway(
    hub: String,
    self_url: String,
    member: &'static str,
    peers: Vec<String>,
) -> impl FnOnce(&mut ServeOpts, &TempDir) + Send + 'static {
    move |opts, dir| {
        opts.enrol_dir = Some(dir.path_str("enrolled"));
        std::fs::write(dir.join("uplink.token"), SITE_TOKEN).unwrap();
        std::fs::write(dir.join("hub.pin"), HUB_PIN).unwrap();
        std::fs::write(
            dir.join("uplink.json"),
            serde_json::to_vec(&json!({
                "registrar_url": hub,
                "tunnel_endpoint": "hub.losos.cfd:2333",
                "id": "acme",
                "token_file": dir.path_str("uplink.token"),
                "pin_file": dir.path_str("hub.pin"),
            }))
            .unwrap(),
        )
        .unwrap();
        opts.uplink = Some(losos_registrar::relay::UplinkOpts {
            file: dir.path_str("uplink.json"),
            tunnel_config: dir.path_str("uplink-tunnel.json"),
            interval: Duration::from_millis(100),
        });
        std::fs::write(
            dir.join("cluster.json"),
            serde_json::to_vec(&json!({
                "name": "office",
                "member": member,
                "secret": SECRET,
                "peers": peers,
            }))
            .unwrap(),
        )
        .unwrap();
        opts.secondary = Some(SecondaryOpts {
            file: dir.path_str("cluster.json"),
            self_url,
            self_tunnel: format!("{member}.local:2333"),
            interval: Duration::from_millis(100),
        });
    }
}

async fn info(edge: &Edge) -> Value {
    reqwest::get(format!("{}/cluster/secondary", edge.base))
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

fn uplink(edge: &Edge) -> String {
    std::fs::read_to_string(edge.dir.join("uplink-tunnel.json")).unwrap_or_default()
}

fn enrolled(edge: &Edge) -> Value {
    std::fs::read_to_string(edge.dir.join("enrolled/tenants.json"))
        .map(|t| serde_json::from_str(&t).unwrap())
        .unwrap_or(Value::Null)
}

#[tokio::test]
async fn two_gateways_share_their_boxes_and_one_relays_each() {
    let hub = Edge::start(
        "sec-hub",
        &[TenantSpec::new("acme", "acme.losos.cfd", SITE_TOKEN).with_relay_zone(ZONE)],
    )
    .await;
    let (la, url_a) = listener().await;
    let (lb, url_b) = listener().await;
    let gw_a = Edge::start_custom_on(
        "sec-gw-a",
        &[],
        la,
        gateway(hub.base.clone(), url_a.clone(), "gw-a", vec![]),
    )
    .await;
    let gw_b = Edge::start_custom_on(
        "sec-gw-b",
        &[],
        lb,
        gateway(hub.base.clone(), url_b.clone(), "gw-b", vec![url_a.clone()]),
    )
    .await;

    // The hub is a hub: a box's link to it is tertiary. Each gateway knows
    // the other once gw-b's first sync reaches the founder.
    let hub_info = info(&hub).await;
    assert_eq!(hub_info["role"], "hub", "{hub_info}");
    assert_eq!(hub_info["tier"], "tertiary", "{hub_info}");
    for gw in [&gw_a, &gw_b] {
        assert!(
            gw.wait_for_async(|| async {
                info(gw).await["gateways"].as_array().unwrap().len() == 2
            })
            .await,
            "{}",
            info(gw).await
        );
        let i = info(gw).await;
        assert_eq!(i["role"], "gateway");
        assert_eq!(i["tier"], "secondary");
        assert_eq!(i["cluster"], "office");
        assert_eq!(i["main"], true, "the uplink makes it a primary member");
    }
    assert_eq!(info(&gw_a).await["gateways"][0]["member"], "gw-a");
    assert_eq!(
        info(&gw_a).await["gateways"][1]["tunnel"],
        "gw-b.local:2333"
    );

    // The box enrols with gw-a on first contact; gw-b takes it in with the
    // same token from the sync, without ever hearing from the box.
    let (status, body) = gw_a
        .register("box1", "box1.acme.losos.cfd", GOOD_TOKEN)
        .await;
    assert_eq!(status, 200, "{body}");
    assert!(
        gw_b.wait_for(|| enrolled(&gw_b).get("box1").is_some())
            .await,
        "{}",
        enrolled(&gw_b)
    );
    assert_eq!(
        std::fs::read_to_string(gw_b.dir.join("enrolled/box1.token"))
            .unwrap()
            .trim(),
        GOOD_TOKEN
    );

    // gw-a heard it, so gw-a alone relays it.
    assert!(
        gw_a.wait_for(|| uplink(&gw_a).contains("acme.box1")).await,
        "{}",
        uplink(&gw_a)
    );
    assert!(
        hub.wait_for(|| hub.tunnel_json().is_some_and(|t| t.contains("acme.box1")))
            .await
    );
    // gw-b knows the box but has not heard it: not on its uplink.
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert!(!uplink(&gw_b).contains("acme.box1"), "{}", uplink(&gw_b));
    let hub_port_before = hub_port(&hub);

    // The hub lists both gateways in the main cluster, and tells each.
    assert!(
        hub.wait_for(|| main_cluster(&hub).as_array().is_some_and(|a| a.len() == 2))
            .await,
        "{}",
        main_cluster(&hub)
    );
    let rows = main_cluster(&hub);
    assert_eq!(rows[0]["site"], "acme");
    assert_eq!(rows[0]["gateway"], "gw-a");
    assert_eq!(rows[0]["cluster"], "office");
    assert_eq!(rows[0]["tier"], "primary");
    assert_eq!(rows[1]["gateway"], "gw-b");
    assert!(
        gw_a.wait_for(|| main_cluster(&gw_a).as_array().is_some_and(|a| a.len() == 2))
            .await
    );

    // A wrong token is still wrong at gw-b: it learned the box's token, not
    // an open door.
    let (status, _) = gw_b
        .register("box1", "box1.acme.losos.cfd", OTHER_TOKEN)
        .await;
    assert_eq!(status, 401);

    // gw-a goes quiet for the box (the box moved): it registers with gw-b,
    // which now heard it last. The route moves to gw-b's uplink and leaves
    // gw-a's; the hub keeps the same service and port.
    let (status, body) = gw_b
        .register("box1", "box1.acme.losos.cfd", GOOD_TOKEN)
        .await;
    assert_eq!(status, 200, "{body}");
    assert!(
        gw_b.wait_for(|| uplink(&gw_b).contains("acme.box1")).await,
        "{}",
        uplink(&gw_b)
    );
    assert!(
        gw_a.wait_for(|| !uplink(&gw_a).contains("acme.box1")).await,
        "{}",
        uplink(&gw_a)
    );
    assert!(hub.tunnel_json().unwrap().contains("acme.box1"));
    assert_eq!(hub_port(&hub), hub_port_before);
    let registry = hub.registry_json().unwrap();
    assert!(registry.contains("\"acme.box1\""), "{registry}");

    // A sync signed with another secret is refused.
    let body = serde_json::to_vec(&json!({
        "cluster": "office", "member": "gw-x", "url": "http://10.0.0.9:8443",
        "tunnel": "10.0.0.9:2333", "at": 0, "boxes": [], "peers": []
    }))
    .unwrap();
    let r = reqwest::Client::new()
        .post(format!("{}{}", gw_a.base, secondary::SYNC_ROUTE))
        .header(
            secondary::MAC_HEADER,
            secondary::mac(&"f".repeat(64), &body),
        )
        .body(body)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 401);

    gw_b.shutdown().await;
    gw_a.shutdown().await;
    hub.shutdown().await;
}

/// A box enrolled at a gateway may join the mesh only when the gateway is in
/// a secondary cluster: that is what the cluster is for. Without one, open
/// enrolment stays proxy-only.
#[tokio::test]
async fn an_enrolled_box_may_join_the_mesh_of_its_secondary_cluster() {
    let lone = Edge::start_custom("sec-lone", &[], |opts, dir| {
        opts.enrol_dir = Some(dir.path_str("enrolled"));
    })
    .await;
    let (status, _) = lone.register("box1", "box1.local", GOOD_TOKEN).await;
    assert_eq!(status, 200);
    let (status, _) = lone.join("box1", "box1", GOOD_TOKEN).await;
    assert_eq!(status, 403, "a lone gateway's enrolled box is proxy-only");
    lone.shutdown().await;

    let (l, url) = listener().await;
    let hub = Edge::start("sec-hub-2", &[]).await;
    let gw = Edge::start_custom_on(
        "sec-member",
        &[],
        l,
        gateway(hub.base.clone(), url, "gw-a", vec![]),
    )
    .await;
    let (status, _) = gw.register("box1", "box1.acme.losos.cfd", GOOD_TOKEN).await;
    assert_eq!(status, 200);
    let (status, body) = gw.join("box1", "box1", GOOD_TOKEN).await;
    // Past the membership check: this gateway runs no mesh, so the answer
    // is that, not a refusal.
    assert_eq!(status, 503, "{body}");
    gw.shutdown().await;
    hub.shutdown().await;
}

fn hub_port(hub: &Edge) -> String {
    let file: Value = serde_json::from_str(&hub.tunnel_json().unwrap()).unwrap();
    let bind = file["services"]["acme.box1"]["bind"].as_str().unwrap();
    bind.strip_prefix("127.0.0.1:").unwrap().to_string()
}

/// A hub's tunnel pin, distinct per official cluster.
fn pin_of(cluster: &str) -> String {
    let byte = if cluster == "us" { "a5" } else { "e5" };
    byte.repeat(32)
}

fn main_cluster(edge: &Edge) -> Value {
    std::fs::read_to_string(edge.dir.join("main-cluster.json"))
        .map(|t| serde_json::from_str(&t).unwrap())
        .unwrap_or(Value::Null)
}

/// Official clusters: two hubs behind two load balancers, each listing both.
/// A gateway's uplink stays on its hub until that hub's whole official
/// cluster is banned, then moves by itself to the other cluster's hub, which
/// takes the box's route, and the uplink file says so from then on.
#[tokio::test]
async fn a_gateway_leaves_a_banned_official_cluster() {
    let (la, url_a) = listener().await;
    let (lb, url_b) = listener().await;
    let peers = vec![
        losos_registrar::official::OfficialPeer::parse(&format!("eu,{url_a},127.0.0.1:2333"))
            .unwrap(),
        losos_registrar::official::OfficialPeer::parse(&format!("us,{url_b},127.0.0.1:2334"))
            .unwrap(),
    ];
    let site = [TenantSpec::new("acme", "acme.losos.cfd", SITE_TOKEN).with_relay_zone(ZONE)];
    // Each hub serves its own tunnel pin, which a gateway that moves to it
    // fetches afresh.
    let official = |cluster: &'static str, peers: Vec<losos_registrar::official::OfficialPeer>| {
        move |opts: &mut ServeOpts, dir: &TempDir| {
            opts.official_cluster = Some(cluster.to_string());
            opts.official_peers = peers;
            std::fs::write(dir.join("tunnel.pin"), pin_of(cluster)).unwrap();
            opts.tunnel_pin_file = Some(dir.path_str("tunnel.pin"));
        }
    };
    let hub_eu =
        Edge::start_custom_on("ban-hub-eu", &site, la, official("eu", peers.clone())).await;
    let hub_us =
        Edge::start_custom_on("ban-hub-us", &site, lb, official("us", peers.clone())).await;
    let i = info(&hub_eu).await;
    assert_eq!(i["officialCluster"], "eu", "{i}");
    assert_eq!(i["officials"][1]["cluster"], "us", "{i}");

    let (lg, url_g) = listener().await;
    let eu_base = hub_eu.base.clone();
    let gw = Edge::start_custom_on("ban-gw", &[], lg, move |opts, dir| {
        gateway(eu_base, url_g, "gw-a", vec![])(opts, dir);
        opts.bans_file = Some(dir.path_str("bans.json"));
    })
    .await;
    let (status, body) = gw.register("box1", "box1.acme.losos.cfd", GOOD_TOKEN).await;
    assert_eq!(status, 200, "{body}");
    assert!(
        hub_eu
            .wait_for(|| hub_eu
                .tunnel_json()
                .is_some_and(|t| t.contains("acme.box1")))
            .await
    );
    // A ban on one edge of the other cluster moves nothing.
    std::fs::write(
        gw.dir.join("bans.json"),
        format!(r#"{{"edges":["{}"]}}"#, hub_us.base),
    )
    .unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(std::fs::read_to_string(gw.dir.join("uplink.json"))
        .unwrap()
        .contains(&hub_eu.base));

    // The eu cluster banned: the uplink moves to us, by itself.
    std::fs::write(gw.dir.join("bans.json"), r#"{"clusters":["eu"]}"#).unwrap();
    assert!(
        hub_us
            .wait_for(|| hub_us
                .tunnel_json()
                .is_some_and(|t| t.contains("acme.box1")))
            .await,
        "{}",
        uplink(&gw)
    );
    let file = std::fs::read_to_string(gw.dir.join("uplink.json")).unwrap();
    assert!(file.contains(&hub_us.base), "{file}");
    assert!(file.contains("127.0.0.1:2334"), "{file}");
    // The hub takes the route on the gateway's `/relay`, and the gateway
    // writes its tunnel file only after fetching the new hub's pin, so the
    // file can still name the old hub for a moment after the route moved.
    assert!(
        gw.wait_for(|| uplink(&gw).contains("127.0.0.1:2334")).await,
        "{}",
        uplink(&gw)
    );
    // The old hub's pin went with it: the uplink pins the new hub's.
    assert!(uplink(&gw).contains(&pin_of("us")), "{}", uplink(&gw));

    gw.shutdown().await;
    hub_us.shutdown().await;
    hub_eu.shutdown().await;
}
