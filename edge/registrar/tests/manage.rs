//! The manage page's routes end to end (`server::manage`): the key, the box
//! list, and a call relayed into a box that said its owner lets the edge
//! manage it, with the box's own token and the name its nginx serves the
//! edge on. The box is a stand-in on the port the registrar gave it, where
//! the tunnel would land.

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

async fn fake_box(port: u16) -> Seen {
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
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
        .await
        .expect("the box's port is free");
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    seen
}

async fn start(ui: Option<String>) -> Edge {
    Edge::start_custom(
        "manage",
        &[TenantSpec::new(
            "office-box",
            "office-box.example.test",
            TOKEN,
        )],
        move |opts, dir| {
            opts.manage = Some(ManageOpts {
                key_file: dir.path_str("manage-key"),
                ui_dir: ui,
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
    let edge = start(None).await;
    let key = key(&edge);
    assert_eq!(key.len(), 64);

    let (status, body) = edge
        .register("office-box", "office-box.example.test", TOKEN)
        .await;
    assert_eq!(status, 200, "{body}");
    let port = serde_json::from_str::<serde_json::Value>(&body).unwrap()["rathole_port"]
        .as_u64()
        .unwrap() as u16;
    let seen = fake_box(port).await;

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
    let edge = start(Some(ui.path_str(""))).await;

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
