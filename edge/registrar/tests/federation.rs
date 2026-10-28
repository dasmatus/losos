//! Edge federation (handbook/docs/in-depth/edge-federation.md): a spoke relays
//! its boxes through a hub, and a LAN edge enrols boxes on first contact.
//!
//! Real registrars on real sockets, as in every other integration test here:
//! the hub is one `Edge`, the spoke another whose `--uplink-file` points at
//! the hub, and the box is a `/register` call against the spoke.

mod common;

use common::{Edge, TenantSpec, GOOD_TOKEN, OTHER_TOKEN};
use losos_registrar::tunnel::{ClientFile, ServerFile};
use serde_json::json;

const ZONE: &str = "acme.losos.cfd";
const HUB_PIN: &str = "4b1d4b1d4b1d4b1d4b1d4b1d4b1d4b1d4b1d4b1d4b1d4b1d4b1d4b1d4b1d4b1d";
/// The spoke's own tenant token on the hub.
const SPOKE_TOKEN: &str = "5p0ke5p0ke5p0ke5p0ke5p0ke5p0ke5p0ke5p0ke5p0ke5p0ke5p0ke5p0ke5p0k";

fn relay_body(tenants: serde_json::Value) -> serde_json::Value {
    json!({ "appliance_id": "acme", "token": SPOKE_TOKEN, "tenants": tenants })
}

/// The hub side alone: a tenant with a relay zone may list boxes under it,
/// each accepted box becomes a `<spoke>.<box>` router and tunnel service
/// carrying the spoke's token, refusals are per box, and a shorter list
/// drops what it no longer names.
#[tokio::test]
async fn a_spoke_relays_boxes_under_its_zone_and_nothing_else() {
    let hub = Edge::start(
        "hub-relay",
        &[
            TenantSpec::new("acme", "acme.losos.cfd", SPOKE_TOKEN).with_relay_zone(ZONE),
            TenantSpec::new("plain", "plain.losos.cfd", GOOD_TOKEN),
        ],
    )
    .await;

    // A plain tenant may not relay; an unknown id may not do anything.
    let (status, _) = hub
        .post(
            "/relay",
            json!({ "appliance_id": "plain", "token": GOOD_TOKEN, "tenants": [] }),
        )
        .await;
    assert_eq!(status, 403);
    let (status, _) = hub
        .post(
            "/relay",
            json!({ "appliance_id": "nobody", "token": GOOD_TOKEN, "tenants": [] }),
        )
        .await;
    assert_eq!(status, 401);

    let (status, body) = hub
        .post(
            "/relay",
            relay_body(json!([
                { "id": "mattbox", "hostname": "mattbox.acme.losos.cfd" },
                { "id": "deep", "hostname": "a.b.acme.losos.cfd" },
                { "id": "Bad.Id", "hostname": "x.acme.losos.cfd" },
                { "id": "elsewhere", "hostname": "elsewhere.losos.cfd" },
            ])),
        )
        .await;
    assert_eq!(status, 200, "{body}");
    let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
    let accepted = resp["accepted"].as_array().unwrap();
    assert_eq!(accepted.len(), 1, "{body}");
    assert_eq!(accepted[0]["id"], "mattbox");
    let port = accepted[0]["tunnel_port"].as_u64().unwrap();
    assert!((50000..=50100).contains(&port), "{port}");
    let refused: Vec<&str> = resp["refused"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    assert_eq!(refused, ["deep", "Bad.Id", "elsewhere"]);

    assert!(
        hub.wait_for(|| hub
            .tunnel_json()
            .is_some_and(|t| t.contains("\"acme.mattbox\"")))
            .await,
        "{:?}",
        hub.tunnel_json()
    );
    let file: ServerFile = serde_json::from_str(&hub.tunnel_json().unwrap()).unwrap();
    let svc = &file.services["acme.mattbox"];
    assert_eq!(
        svc.token, SPOKE_TOKEN,
        "the relayed service carries the spoke's token"
    );
    assert_eq!(svc.bind.to_string(), format!("127.0.0.1:{port}"));
    let yaml = hub.traefik_yaml().unwrap();
    assert!(yaml.contains("Host(`mattbox.acme.losos.cfd`)"), "{yaml}");
    assert!(yaml.contains("acme.mattbox"), "{yaml}");
    let registry = hub.registry_json().unwrap();
    assert!(registry.contains("\"via\": \"acme\""), "{registry}");

    // A second call is the whole truth: a box not listed is gone.
    let (status, _) = hub.post("/relay", relay_body(json!([]))).await;
    assert_eq!(status, 200);
    assert!(
        hub.wait_for(|| hub
            .tunnel_json()
            .is_some_and(|t| !t.contains("acme.mattbox")))
            .await
    );
    assert!(hub.traefik_yaml().is_none());

    // Too many at once is refused whole, so the spoke's log says so.
    let many: Vec<serde_json::Value> = (0..65)
        .map(|i| json!({ "id": format!("b{i}"), "hostname": format!("b{i}.acme.losos.cfd") }))
        .collect();
    let (status, _) = hub.post("/relay", relay_body(json!(many))).await;
    assert_eq!(status, 400);

    hub.shutdown().await;
}

/// The operator narrows the zone: a relayed box that no longer fits is
/// dropped on the next reconcile, with no new `/relay` call.
#[tokio::test]
async fn narrowing_the_zone_drops_relayed_boxes_on_the_next_tick() {
    let hub = Edge::start(
        "hub-zone-change",
        &[TenantSpec::new("acme", "acme.losos.cfd", SPOKE_TOKEN).with_relay_zone(ZONE)],
    )
    .await;
    let (status, _) = hub
        .post(
            "/relay",
            relay_body(json!([{ "id": "mattbox", "hostname": "mattbox.acme.losos.cfd" }])),
        )
        .await;
    assert_eq!(status, 200);
    assert!(
        hub.wait_for(|| hub
            .tunnel_json()
            .is_some_and(|t| t.contains("acme.mattbox")))
            .await
    );
    hub.rewrite_tenants(&[
        TenantSpec::new("acme", "acme.losos.cfd", SPOKE_TOKEN).with_relay_zone("other.losos.cfd")
    ]);
    assert!(
        hub.wait_for(|| hub
            .tunnel_json()
            .is_some_and(|t| !t.contains("acme.mattbox")))
            .await,
        "{:?}",
        hub.tunnel_json()
    );
    hub.shutdown().await;
}

/// Spoke to hub, end to end: a box registers with the spoke, the spoke's
/// uplink tells the hub, the hub routes `<spoke>.<box>`, and the spoke's
/// uplink client config carries that service pointed at the box's port on
/// the spoke. Removing the uplink file switches the uplink off.
#[tokio::test]
async fn the_uplink_relays_what_is_live_on_the_spoke() {
    let hub = Edge::start_with_tunnel_pin(
        "fed-hub",
        &[TenantSpec::new("acme", "acme.losos.cfd", SPOKE_TOKEN).with_relay_zone(ZONE)],
        HUB_PIN,
    )
    .await;
    let hub_base = hub.base.clone();
    let spoke = Edge::start_custom(
        "fed-spoke",
        &[
            TenantSpec::new("mattbox", "mattbox.acme.losos.cfd", GOOD_TOKEN),
            // Named outside the zone: the hub refuses it, the spoke still
            // serves it locally and does not put it on the uplink.
            TenantSpec::new("stray", "stray.example.org", OTHER_TOKEN),
        ],
        move |opts, dir| {
            std::fs::write(dir.join("uplink.token"), SPOKE_TOKEN).unwrap();
            // The hub's tunnel is not pinned yet, so the first pass fetches
            // its pin.
            std::fs::write(
                dir.join("uplink.json"),
                serde_json::to_vec(&json!({
                    "registrar_url": hub_base,
                    "tunnel_endpoint": "hub.losos.cfd:2333",
                    "id": "acme",
                    "token_file": dir.path_str("uplink.token"),
                    "pin_file": dir.path_str("pins/hub.pin"),
                }))
                .unwrap(),
            )
            .unwrap();
            opts.uplink = Some(losos_registrar::relay::UplinkOpts {
                file: dir.path_str("uplink.json"),
                tunnel_config: dir.path_str("uplink-tunnel.json"),
                interval: std::time::Duration::from_millis(100),
            });
        },
    )
    .await;

    // Nothing live yet: the uplink file exists, so a client config is
    // written, with no services.
    let uplink_file = spoke.dir.join("uplink-tunnel.json");
    let read_uplink = || std::fs::read_to_string(&uplink_file).ok();
    assert!(spoke.wait_for(|| read_uplink().is_some()).await);
    let empty: ClientFile = serde_json::from_str(&read_uplink().unwrap()).unwrap();
    assert_eq!(empty.remote, "hub.losos.cfd:2333");
    assert!(empty.services.is_empty(), "{empty:?}");
    assert_eq!(empty.pin, HUB_PIN);
    assert_eq!(
        std::fs::read_to_string(spoke.dir.join("pins/hub.pin")).unwrap(),
        HUB_PIN,
        "the hub's tunnel is pinned on first contact"
    );

    let (status, body) = spoke
        .register("mattbox", "mattbox.acme.losos.cfd", GOOD_TOKEN)
        .await;
    assert_eq!(status, 200, "{body}");
    let spoke_port = serde_json::from_str::<serde_json::Value>(&body).unwrap()["tunnel_port"]
        .as_u64()
        .unwrap();
    let (status, _) = spoke
        .register("stray", "stray.example.org", OTHER_TOKEN)
        .await;
    assert_eq!(status, 200);

    assert!(
        hub.wait_for(|| hub
            .tunnel_json()
            .is_some_and(|t| t.contains("acme.mattbox")))
            .await,
        "hub: {:?}",
        hub.tunnel_json()
    );
    assert!(!hub.tunnel_json().unwrap().contains("stray"));
    assert!(hub
        .traefik_yaml()
        .unwrap()
        .contains("Host(`mattbox.acme.losos.cfd`)"));
    assert!(
        spoke
            .wait_for(|| read_uplink().is_some_and(|t| t.contains("acme.mattbox")))
            .await,
        "spoke uplink: {:?}",
        read_uplink()
    );
    let file: ClientFile = serde_json::from_str(&read_uplink().unwrap()).unwrap();
    let svc = &file.services["acme.mattbox"];
    assert_eq!(svc.local, format!("127.0.0.1:{spoke_port}"));
    assert_eq!(svc.token, SPOKE_TOKEN);
    assert_eq!(file.services.len(), 1, "{file:?}");

    // The box leaves the spoke: the hub forgets it on the next uplink pass.
    let (status, _) = spoke
        .post(
            "/unregister",
            json!({ "appliance_id": "mattbox", "token": GOOD_TOKEN }),
        )
        .await;
    assert_eq!(status, 204);
    assert!(
        hub.wait_for(|| hub
            .tunnel_json()
            .is_some_and(|t| !t.contains("acme.mattbox")))
            .await
    );
    assert!(
        spoke
            .wait_for(|| read_uplink().is_some_and(|t| !t.contains("acme.mattbox")))
            .await
    );

    // The owner removes the uplink file: the client config goes with it.
    std::fs::remove_file(spoke.dir.join("uplink.json")).unwrap();
    assert!(spoke.wait_for(|| read_uplink().is_none()).await);

    spoke.shutdown().await;
    hub.shutdown().await;
}

/// Open enrolment: an unknown box on a LAN edge is accepted on first contact
/// with the token it presented, later requests must match it, a second box
/// cannot take the id, the enrolled box may rename itself, and the operator
/// whitelist still wins for an id it names. Closed by default: the same
/// registration against an ordinary edge is 401.
#[tokio::test]
async fn a_lan_edge_enrols_boxes_on_first_contact() {
    let closed = Edge::start("closed", &[]).await;
    let (status, _) = closed.register("newbox", "newbox.local", GOOD_TOKEN).await;
    assert_eq!(status, 401);
    closed.shutdown().await;

    let open = Edge::start_custom(
        "open-enrolment",
        &[TenantSpec::new("listed", "listed.losos.cfd", OTHER_TOKEN)],
        |opts, dir| {
            opts.enrol_dir = Some(dir.path_str("enrolled"));
        },
    )
    .await;

    // First contact fixes the token.
    let (status, body) = open.register("newbox", "newbox.local", GOOD_TOKEN).await;
    assert_eq!(status, 200, "{body}");
    let (status, _) = open.heartbeat("newbox", GOOD_TOKEN).await;
    assert_eq!(status, 204);
    let token_file = open.dir.join("enrolled").join("newbox.token");
    assert_eq!(
        std::fs::read_to_string(&token_file).unwrap().trim(),
        GOOD_TOKEN
    );
    assert!(
        open.wait_for(|| open.tunnel_json().is_some_and(|t| t.contains("\"newbox\"")))
            .await
    );

    // Somebody else with the same id, or the right id and a wrong token.
    let (status, _) = open.register("newbox", "newbox.local", OTHER_TOKEN).await;
    assert_eq!(status, 401);
    let (status, _) = open.heartbeat("newbox", OTHER_TOKEN).await;
    assert_eq!(status, 401);

    // A rename by the box that proved its token is recorded.
    let (status, _) = open
        .register("newbox", "newbox.acme.losos.cfd", GOOD_TOKEN)
        .await;
    assert_eq!(status, 200);
    assert!(
        open.wait_for(|| open
            .traefik_yaml()
            .is_some_and(|y| y.contains("Host(`newbox.acme.losos.cfd`)")))
            .await,
        "{:?}",
        open.traefik_yaml()
    );

    // Not everything enrols: ids and names must be DNS, tokens must be real.
    let (status, _) = open.register("Not A Label", "x.local", GOOD_TOKEN).await;
    assert_eq!(status, 401);
    let (status, _) = open.register("shorttoken", "short.local", "abc").await;
    assert_eq!(status, 401);
    assert!(!open.dir.join("enrolled").join("shorttoken.token").exists());

    // The whitelist still decides for an id it lists.
    let (status, _) = open
        .register("listed", "listed.losos.cfd", GOOD_TOKEN)
        .await;
    assert_eq!(status, 401);
    let (status, _) = open
        .register("listed", "listed.losos.cfd", OTHER_TOKEN)
        .await;
    assert_eq!(status, 200);

    open.shutdown().await;
}
