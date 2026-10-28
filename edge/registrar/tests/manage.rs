//! The manage page's routes end to end (`server::manage`): the key, the box
//! list, and a call relayed into a box that said its owner lets the edge
//! manage it, with the box's own token and the name its nginx serves the
//! edge on. The box is a stand-in on the port the registrar gave it, where
//! the tunnel would land: the test binds a port the OS picks first and makes
//! it the edge's whole range, since a fixed port in the ephemeral range can
//! already be some other socket's.

mod common;

use std::sync::{Arc, Mutex};

use axum::http::{HeaderMap, Method, Uri};
use axum::routing::any;
use axum::{Json, Router};
use common::{Edge, TenantSpec};
use losos_registrar::opts::ManageOpts;

const TOKEN: &str = "box-token-0123456789abcdef0123456789abcdef";

/// What the stand-in box saw: method, path and query, Host, X-Losos-Edge,
/// body.
type Seen = Arc<Mutex<Vec<(String, String, String, String, String)>>>;

async fn fake_box(listener: tokio::net::TcpListener) -> Seen {
    let seen: Seen = Arc::default();
    let log = seen.clone();
    let app = Router::new().fallback(any(
        move |method: Method, uri: Uri, headers: HeaderMap, body: String| {
            let log = log.clone();
            async move {
                let h = |n: &str| {
                    headers
                        .get(n)
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("")
                        .to_string()
                };
                log.lock().unwrap().push((
                    method.to_string(),
                    uri.to_string(),
                    h("host"),
                    h("x-losos-edge"),
                    body,
                ));
                Json(serde_json::json!({ "apps": [{ "release": "pihole" }] }))
            }
        },
    ));
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    seen
}

async fn start(ui: Option<String>, port_range: Option<(u16, u16)>) -> Edge {
    start_with(ui, port_range, None).await
}

async fn start_with(
    ui: Option<String>,
    port_range: Option<(u16, u16)>,
    traefik_metrics: Option<String>,
) -> Edge {
    Edge::start_custom(
        "manage",
        &[TenantSpec::new(
            "office-box",
            "office-box.example.test",
            TOKEN,
        )],
        move |opts, dir| {
            if let Some(range) = port_range {
                opts.port_range = range;
            }
            opts.manage = Some(ManageOpts {
                key_file: dir.path_str("manage-key"),
                ui_dir: ui,
                traefik_metrics,
            });
        },
    )
    .await
}

fn key(edge: &Edge) -> String {
    std::fs::read_to_string(edge.dir.join("manage-key"))
        .expect("the key was made on start")
        .trim()
        .to_string()
}

async fn get(edge: &Edge, path: &str, key: Option<&str>) -> (u16, String) {
    let mut req = edge.client.get(format!("{}{path}", edge.base));
    if let Some(k) = key {
        req = req.bearer_auth(k);
    }
    let resp = req.send().await.unwrap();
    (resp.status().as_u16(), resp.text().await.unwrap())
}

#[tokio::test]
async fn a_managed_box_is_listed_and_its_calls_reach_it_with_its_token() {
    let box_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let box_port = box_listener.local_addr().unwrap().port();
    let edge = start(None, Some((box_port, box_port))).await;
    let key = key(&edge);
    assert_eq!(key.len(), 64);

    let (status, body) = edge
        .register("office-box", "office-box.example.test", TOKEN)
        .await;
    assert_eq!(status, 200, "{body}");
    let port = serde_json::from_str::<serde_json::Value>(&body).unwrap()["tunnel_port"]
        .as_u64()
        .unwrap() as u16;
    assert_eq!(port, box_port);
    let seen = fake_box(box_listener).await;

    // No key, a wrong key.
    assert_eq!(get(&edge, "/manage/api/boxes", None).await.0, 401);
    assert_eq!(
        get(&edge, "/manage/api/boxes", Some(&"0".repeat(64)))
            .await
            .0,
        401
    );

    // Listed, not managed: the owner has not switched it on.
    let (status, body) = get(&edge, "/manage/api/boxes", Some(&key)).await;
    assert_eq!(status, 200, "{body}");
    let list: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(list["boxes"][0]["id"], "office-box");
    assert_eq!(list["boxes"][0]["managed"], false);
    let (status, _) = get(&edge, "/manage/api/boxes/office-box/api/apps", Some(&key)).await;
    assert_eq!(status, 403);
    assert!(seen.lock().unwrap().is_empty(), "nothing reached the box");

    // The owner switched it on: the next heartbeat says so.
    let (status, _) = edge
        .post(
            "/heartbeat",
            serde_json::json!({ "appliance_id": "office-box", "token": TOKEN, "managed": true }),
        )
        .await;
    assert_eq!(status, 204);
    let (_, body) = get(&edge, "/manage/api/boxes", Some(&key)).await;
    assert!(body.contains(r#""managed":true"#), "{body}");

    let (status, body) = get(
        &edge,
        "/manage/api/boxes/office-box/api/apps?all=1",
        Some(&key),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert!(body.contains("pihole"));
    let resp = edge
        .client
        .post(format!(
            "{}/manage/api/boxes/office-box/api/apps/install",
            edge.base
        ))
        .bearer_auth(&key)
        .json(&serde_json::json!({ "release": "pihole" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    {
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 2, "{seen:?}");
        let (method, path, host, token, _) = &seen[0];
        assert_eq!((method.as_str(), path.as_str()), ("GET", "/api/apps?all=1"));
        assert_eq!(host, "losos-manage.invalid");
        assert_eq!(token, TOKEN);
        assert_eq!(seen[1].0, "POST");
        assert_eq!(seen[1].1, "/api/apps/install");
        assert!(seen[1].4.contains("pihole"));
    }

    // Only plain routes, and only boxes this edge routes. (A client resolves
    // dot segments before it sends; the box's lososd decides what the edge
    // may call either way.)
    let (status, _) = get(
        &edge,
        "/manage/api/boxes/office-box/api/apps/a%20b",
        Some(&key),
    )
    .await;
    assert_eq!(status, 400);
    let (status, _) = get(
        &edge,
        "/manage/api/boxes/office-box/api/apps/%2F..%2Fsetup",
        Some(&key),
    )
    .await;
    assert_eq!(status, 400);
    let (status, _) = get(&edge, "/manage/api/boxes/nobody/api/apps", Some(&key)).await;
    assert_eq!(status, 403);

    // Switched off again: the edge stops relaying at once.
    edge.heartbeat("office-box", TOKEN).await;
    let (status, _) = get(&edge, "/manage/api/boxes/office-box/api/apps", Some(&key)).await;
    assert_eq!(status, 403);
    assert_eq!(seen.lock().unwrap().len(), 2);

    edge.shutdown().await;
}

#[tokio::test]
async fn the_page_is_served_with_its_own_policy_and_deep_links_reload() {
    let ui = common::TempDir::new("manage-ui");
    std::fs::create_dir_all(ui.join("assets")).unwrap();
    std::fs::create_dir_all(ui.join("manage")).unwrap();
    std::fs::write(
        ui.join("manage/index.html"),
        "<!doctype html><title>LosOS edge</title>",
    )
    .unwrap();
    std::fs::write(
        ui.join("index.html"),
        "<!doctype html><title>the box's own page</title>",
    )
    .unwrap();
    std::fs::write(ui.join("assets/page-abc.js"), "console.log(1)").unwrap();
    std::fs::write(ui.join("theme-boot.js"), "void 0").unwrap();
    let edge = start(Some(ui.path_str("")), None).await;

    let resp = edge
        .client
        .get(format!("{}/manage/", edge.base))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let csp = resp.headers()["content-security-policy"]
        .to_str()
        .unwrap()
        .to_string();
    assert!(csp.contains("default-src 'none'") && csp.contains("frame-ancestors 'none'"));
    assert!(resp.text().await.unwrap().contains("LosOS edge"));

    let resp = edge
        .client
        .get(format!("{}/manage/boxes/office-box", edge.base))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    assert!(resp.text().await.unwrap().contains("LosOS edge"));

    let resp = edge
        .client
        .get(format!("{}/assets/page-abc.js", edge.base))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    assert!(resp.headers()["content-type"]
        .to_str()
        .unwrap()
        .starts_with("text/javascript"));
    assert!(resp.headers()["cache-control"]
        .to_str()
        .unwrap()
        .contains("immutable"));
    let resp = edge
        .client
        .get(format!("{}/theme-boot.js", edge.base))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let resp = edge
        .client
        .get(format!("{}/assets/missing.js", edge.base))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 404);
    let resp = edge
        .client
        .get(format!("{}/assets/..%2Findex.html", edge.base))
        .send()
        .await
        .unwrap();
    assert_ne!(resp.status().as_u16(), 200);

    edge.shutdown().await;
}

#[tokio::test]
async fn an_edge_without_a_key_manages_nothing() {
    let edge = Edge::start(
        "manage-off",
        &[TenantSpec::new(
            "office-box",
            "office-box.example.test",
            TOKEN,
        )],
    )
    .await;
    assert_eq!(
        get(&edge, "/manage/api/boxes", Some(&"0".repeat(64)))
            .await
            .0,
        404
    );
    assert_eq!(get(&edge, "/manage/", None).await.0, 404);
    // A heartbeat that says "managed" to such an edge is still a heartbeat.
    edge.register("office-box", "office-box.example.test", TOKEN)
        .await;
    let (status, _) = edge
        .post(
            "/heartbeat",
            serde_json::json!({ "appliance_id": "office-box", "token": TOKEN, "managed": true }),
        )
        .await;
    assert_eq!(status, 204);
    edge.shutdown().await;
}

/// Traefik's Prometheus answer as a stand-in serves it: one box's service,
/// Traefik's own, and the metrics entrypoint, which is not traffic.
const METRICS: &str = "\
# TYPE traefik_entrypoint_requests_total counter
traefik_entrypoint_requests_total{code=\"200\",entrypoint=\"websecure\",method=\"GET\",protocol=\"http\"} 40
traefik_entrypoint_requests_total{code=\"404\",entrypoint=\"websecure\",method=\"GET\",protocol=\"http\"} 3
traefik_entrypoint_requests_total{code=\"502\",entrypoint=\"websecure\",method=\"GET\",protocol=\"http\"} 2
traefik_entrypoint_requests_total{code=\"200\",entrypoint=\"metrics\",method=\"GET\",protocol=\"http\"} 70
traefik_entrypoint_responses_bytes_total{code=\"200\",entrypoint=\"websecure\",method=\"GET\",protocol=\"http\"} 81920
traefik_open_connections{entrypoint=\"websecure\",protocol=\"TCP\"} 2
traefik_service_requests_total{code=\"200\",method=\"GET\",protocol=\"http\",service=\"office-box@http\"} 40
traefik_service_requests_total{code=\"502\",method=\"GET\",protocol=\"http\",service=\"office-box@http\"} 2
traefik_service_requests_total{code=\"200\",method=\"GET\",protocol=\"http\",service=\"prometheus@internal\"} 70
process_start_time_seconds 1.7926e+09
";

#[tokio::test]
async fn statistics_are_what_traefik_counted_with_each_box_named() {
    let traefik = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/metrics", traefik.local_addr().unwrap());
    let app = Router::new().route("/metrics", axum::routing::get(|| async { METRICS }));
    tokio::spawn(async move { axum::serve(traefik, app).await.unwrap() });

    let edge = start_with(None, None, Some(url)).await;
    let key = key(&edge);
    assert_eq!(get(&edge, "/manage/api/statistics", None).await.0, 401);
    edge.register("office-box", "office-box.example.test", TOKEN)
        .await;

    // The first reading is taken as the registrar starts, and the box's
    // name joins once a reconcile pass has routed it.
    let mut doc = serde_json::Value::Null;
    for _ in 0..50 {
        let (status, body) = get(&edge, "/manage/api/statistics", Some(&key)).await;
        assert_eq!(status, 200, "{body}");
        doc = serde_json::from_str(&body).unwrap();
        if doc["available"] == true && doc["routes"][0]["host"].is_string() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert_eq!(doc["available"], true, "{doc}");
    assert_eq!(doc["totals"]["requests"], 45.0);
    assert_eq!(doc["totals"]["clientErrors"], 3.0);
    assert_eq!(doc["totals"]["serverErrors"], 2.0);
    assert_eq!(doc["totals"]["openConnections"], 2.0);
    assert_eq!(doc["codes"]["2xx"], 40.0);
    assert_eq!(doc["since"], "2026-10-21T16:26:40Z");
    let routes = doc["routes"].as_array().unwrap();
    assert_eq!(routes.len(), 1, "{doc}");
    assert_eq!(routes[0]["name"], "office-box");
    assert_eq!(routes[0]["host"], "office-box.example.test");
    assert_eq!(routes[0]["serverErrors"], 2.0);
    edge.shutdown().await;
}

#[tokio::test]
async fn statistics_without_traefik_say_why() {
    let edge = start(None, None).await;
    let (status, body) = get(&edge, "/manage/api/statistics", Some(&key(&edge))).await;
    assert_eq!(status, 200);
    let doc: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(doc["available"], false);
    assert_eq!(doc["reason"], "this edge does not read Traefik's counters");
    edge.shutdown().await;
}

/// The gateway's own settings, which only the manage API changes: no key,
/// no settings; with it, an uplink is set and cleared and an enrolled box
/// is forgotten.
#[tokio::test]
async fn the_gateway_is_set_up_from_the_page_with_the_key() {
    let edge = Edge::start_custom("manage-gateway", &[], |opts, dir| {
        opts.manage = Some(ManageOpts {
            key_file: dir.path_str("manage-key"),
            ui_dir: None,
            traefik_metrics: None,
        });
        opts.enrol_dir = Some(dir.path_str("enrolled"));
        opts.uplink = Some(losos_registrar::relay::UplinkOpts {
            file: dir.path_str("edge/uplink.json"),
            tunnel_config: dir.path_str("uplink-tunnel.json"),
            interval: std::time::Duration::from_secs(3600),
        });
    })
    .await;
    let key = key(&edge);
    let send =
        |method: reqwest::Method, path: &str, k: Option<&str>, body: Option<serde_json::Value>| {
            let mut req = edge.client.request(method, format!("{}{path}", edge.base));
            if let Some(k) = k {
                req = req.bearer_auth(k);
            }
            if let Some(b) = body {
                req = req.json(&b);
            }
            async move {
                let resp = req.send().await.unwrap();
                (resp.status().as_u16(), resp.text().await.unwrap())
            }
        };
    let uplink = serde_json::json!({
        "id": "acme",
        "token": "hub-token",
        "registrarUrl": "https://register.example.test",
        "tunnelEndpoint": "relay.example.test:2333",
    });

    assert_eq!(get(&edge, "/manage/api/gateway", None).await.0, 401);
    assert_eq!(
        send(
            reqwest::Method::PUT,
            "/manage/api/gateway/uplink",
            None,
            Some(uplink.clone())
        )
        .await
        .0,
        401
    );
    assert!(!edge.dir.join("edge/uplink.json").exists());

    let (status, body) = get(&edge, "/manage/api/gateway", Some(&key)).await;
    assert_eq!(status, 200, "{body}");
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(v["uplink"]["set"].is_null(), "{v}");
    assert_eq!(v["enrolled"], serde_json::json!([]));

    // A malformed row is refused and writes nothing.
    let mut bad = uplink.clone();
    bad["tunnelEndpoint"] = "relay".into();
    let (status, body) = send(
        reqwest::Method::PUT,
        "/manage/api/gateway/uplink",
        Some(&key),
        Some(bad),
    )
    .await;
    assert_eq!(status, 400, "{body}");
    assert!(!edge.dir.join("edge/uplink.json").exists());

    let (status, body) = send(
        reqwest::Method::PUT,
        "/manage/api/gateway/uplink",
        Some(&key),
        Some(uplink),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    let file: serde_json::Value =
        serde_json::from_slice(&std::fs::read(edge.dir.join("edge/uplink.json")).unwrap()).unwrap();
    assert_eq!(file["id"], "acme");
    assert_eq!(
        std::fs::read_to_string(edge.dir.join("edge/uplink.token")).unwrap(),
        "hub-token"
    );
    let (_, body) = get(&edge, "/manage/api/gateway", Some(&key)).await;
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(
        v["uplink"]["set"]["registrarUrl"], "https://register.example.test",
        "{v}"
    );

    // An unknown box enrols on first contact, and the page forgets it.
    let (status, body) = edge
        .register("newbox", "newbox.example.test", common::GOOD_TOKEN)
        .await;
    assert_eq!(status, 200, "{body}");
    let (_, body) = get(&edge, "/manage/api/gateway", Some(&key)).await;
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["enrolled"], serde_json::json!(["newbox"]), "{v}");
    assert_eq!(
        send(
            reqwest::Method::DELETE,
            "/manage/api/gateway/boxes/newbox",
            None,
            None
        )
        .await
        .0,
        401
    );
    let (status, body) = send(
        reqwest::Method::DELETE,
        "/manage/api/gateway/boxes/newbox",
        Some(&key),
        None,
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(
        send(
            reqwest::Method::DELETE,
            "/manage/api/gateway/boxes/newbox",
            Some(&key),
            None
        )
        .await
        .0,
        404
    );

    // Cleared: the file, its tokens and the rendered tunnel config go.
    std::fs::write(
        edge.dir.join("uplink-tunnel.json"),
        r#"{"remote":"relay.example.test:2333","pin":"","services":{}}"#,
    )
    .unwrap();
    let (status, body) = send(
        reqwest::Method::DELETE,
        "/manage/api/gateway/uplink",
        Some(&key),
        None,
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert!(!edge.dir.join("edge/uplink.json").exists());
    assert!(!edge.dir.join("edge/uplink.token").exists());
    assert!(!edge.dir.join("uplink-tunnel.json").exists());
    edge.shutdown().await;
}
