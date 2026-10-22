//! LosOS Cloud over HTTP: the routes the Nextcloud apps, file managers and
//! the web app use, against the real server on a temporary folder.

use actix_web::http::StatusCode;
use actix_web::test::{call_service, init_service, read_body, TestRequest};
use actix_web::{App, HttpResponse};
use base64::Engine;
use losos_ctl::cloud::{server, Cloud, Config};
use std::sync::Arc;

macro_rules! app {
    ($cloud:expr) => {
        init_service(App::new().configure(server::configure($cloud.clone(), true))).await
    };
}

fn cloud() -> (tempfile::TempDir, Arc<Cloud>) {
    let d = tempfile::tempdir().unwrap();
    let c = Arc::new(Cloud::open(d.path(), Config::default()).unwrap());
    (d, c)
}

fn as_alice(r: TestRequest) -> TestRequest {
    r.insert_header(("x-losos-cloud-user", "alice"))
        .insert_header(("x-losos-cloud-name", "Alice Example"))
        .insert_header(("host", "box.local"))
}

fn method(m: &str, path: &str) -> TestRequest {
    TestRequest::default()
        .method(actix_web::http::Method::from_bytes(m.as_bytes()).unwrap())
        .uri(path)
}

fn h(r: &actix_web::dev::ServiceResponse, name: &str) -> String {
    r.headers()
        .get(name)
        .unwrap_or_else(|| panic!("no {name} header"))
        .to_str()
        .unwrap()
        .to_owned()
}

async fn text(resp: actix_web::dev::ServiceResponse) -> String {
    String::from_utf8(read_body(resp).await.to_vec()).unwrap()
}

fn basic(u: &str, p: &str) -> String {
    format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD.encode(format!("{u}:{p}"))
    )
}

const PROPS: &str = r#"<?xml version="1.0"?>
<d:propfind xmlns:d="DAV:" xmlns:oc="http://owncloud.org/ns" xmlns:nc="http://nextcloud.org/ns">
 <d:prop><d:getetag/><d:resourcetype/><d:getcontentlength/><oc:fileid/><oc:id/><oc:permissions/><oc:size/><oc:checksums/><nc:has-preview/><oc:favorite/><x:nope xmlns:x="urn:x"/></d:prop>
</d:propfind>"#;

#[actix_web::test]
async fn status_and_capabilities_answer_without_sign_in() {
    let (_d, c) = cloud();
    let app = app!(c);
    let r = call_service(&app, TestRequest::get().uri("/status.php").to_request()).await;
    assert_eq!(r.status(), 200);
    let v: serde_json::Value = serde_json::from_str(&text(r).await).unwrap();
    assert_eq!(v["installed"], true);
    assert_eq!(v["productname"], "LosOS Cloud");
    let r = call_service(
        &app,
        TestRequest::get()
            .uri("/ocs/v1.php/cloud/capabilities?format=json")
            .to_request(),
    )
    .await;
    let v: serde_json::Value = serde_json::from_str(&text(r).await).unwrap();
    assert_eq!(v["ocs"]["meta"]["statuscode"], 100);
    assert_eq!(v["ocs"]["data"]["capabilities"]["dav"]["chunking"], "1.0");
    let r = call_service(
        &app,
        TestRequest::get()
            .uri("/ocs/v2.php/cloud/capabilities")
            .to_request(),
    )
    .await;
    let body = text(r).await;
    assert!(body.contains("<statuscode>200</statuscode>"), "{body}");
}

#[actix_web::test]
async fn webdav_needs_a_person() {
    let (_d, c) = cloud();
    let app = app!(c);
    let r = call_service(&app, method("PROPFIND", "/remote.php/webdav/").to_request()).await;
    assert_eq!(r.status(), 401);
    assert!(r.headers().contains_key("www-authenticate"));
    let r = call_service(
        &app,
        method("PROPFIND", "/remote.php/webdav/")
            .insert_header(("authorization", basic("alice", "wrong")))
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 401);
}

#[actix_web::test]
async fn put_propfind_get_and_the_root_etag() {
    let (_d, c) = cloud();
    let app = app!(c);
    let r = call_service(
        &app,
        as_alice(method("PROPFIND", "/remote.php/dav/files/alice/"))
            .insert_header(("depth", "0"))
            .set_payload(PROPS)
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), StatusCode::MULTI_STATUS);
    let before = text(r).await;
    assert!(
        before.contains("<oc:permissions>RGCK</oc:permissions>"),
        "{before}"
    );

    let r = call_service(
        &app,
        as_alice(method("MKCOL", "/remote.php/dav/files/alice/Docs")).to_request(),
    )
    .await;
    assert_eq!(r.status(), 201);
    let r = call_service(
        &app,
        as_alice(method("PUT", "/remote.php/dav/files/alice/Docs/a%20b.txt"))
            .insert_header(("x-oc-mtime", "1700000000"))
            .insert_header((
                "oc-checksum",
                "SHA1:a9993e364706816aba3e25717850c26c9cd0d89d",
            ))
            .set_payload("abc")
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 201);
    assert_eq!(h(&r, "x-oc-mtime"), "accepted");
    let etag = h(&r, "oc-etag");
    let fileid = h(&r, "oc-fileid");
    assert!(fileid.ends_with(c.store.instance()));

    let r = call_service(
        &app,
        as_alice(method("PROPFIND", "/remote.php/dav/files/alice/"))
            .insert_header(("depth", "infinity"))
            .set_payload(PROPS)
            .to_request(),
    )
    .await;
    let after = text(r).await;
    assert!(
        after.contains("<d:href>/remote.php/dav/files/alice/Docs/a%20b.txt</d:href>"),
        "{after}"
    );
    assert!(after.contains(&format!(
        "<d:getetag>{}</d:getetag>",
        etag.replace('"', "&quot;")
    )));
    assert!(after.contains("<oc:checksums><oc:checksum>SHA1:a9993e364706816aba3e25717850c26c9cd0d89d</oc:checksum></oc:checksums>"));
    assert!(after.contains("<oc:permissions>RGDNVW</oc:permissions>"));
    assert!(
        after.contains(r#"<x:nope xmlns:x="urn:x"/>"#),
        "unknown props are 404s"
    );
    let root_etag = |s: &str| {
        s.split("<d:getetag>")
            .nth(1)
            .unwrap()
            .split("</d:getetag>")
            .next()
            .unwrap()
            .to_owned()
    };
    assert_ne!(
        root_etag(&before),
        root_etag(&after),
        "the root's ETag moved"
    );

    let r = call_service(
        &app,
        as_alice(TestRequest::get().uri("/remote.php/webdav/Docs/a%20b.txt")).to_request(),
    )
    .await;
    assert_eq!(r.status(), 200);
    assert_eq!(h(&r, "etag"), etag);
    assert!(h(&r, "content-security-policy").contains("sandbox"));
    assert_eq!(text(r).await, "abc");

    let r = call_service(
        &app,
        as_alice(TestRequest::get().uri("/remote.php/webdav/Docs/a%20b.txt"))
            .insert_header(("if-none-match", etag.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 304);

    let r = call_service(
        &app,
        as_alice(TestRequest::get().uri("/remote.php/webdav/Docs/a%20b.txt"))
            .insert_header(("range", "bytes=1-"))
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 206);
    assert_eq!(text(r).await, "bc");
}

#[actix_web::test]
async fn a_bad_checksum_or_a_short_body_is_refused() {
    let (_d, c) = cloud();
    let app = app!(c);
    let r = call_service(
        &app,
        as_alice(method("PUT", "/remote.php/webdav/x.txt"))
            .insert_header((
                "oc-checksum",
                "SHA1:0000000000000000000000000000000000000000",
            ))
            .set_payload("abc")
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 400);
    let r = call_service(
        &app,
        as_alice(method("PUT", "/remote.php/webdav/x.txt"))
            .insert_header(("oc-total-length", "10"))
            .set_payload("abc")
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 400);
    assert!(c.store.stat("alice", "x.txt").is_err());
    let r = call_service(
        &app,
        as_alice(method("PUT", "/remote.php/webdav/missing/x.txt"))
            .set_payload("abc")
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 409);
}

#[actix_web::test]
async fn if_match_protects_a_concurrent_edit() {
    let (_d, c) = cloud();
    let app = app!(c);
    let r = call_service(
        &app,
        as_alice(method("PUT", "/remote.php/webdav/f.txt"))
            .set_payload("one")
            .to_request(),
    )
    .await;
    let etag = h(&r, "etag");
    let r = call_service(
        &app,
        as_alice(method("PUT", "/remote.php/webdav/f.txt"))
            .insert_header(("if-match", "\"stale\""))
            .set_payload("two")
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 412);
    let r = call_service(
        &app,
        as_alice(method("PUT", "/remote.php/webdav/f.txt"))
            .insert_header(("if-match", etag.as_str()))
            .set_payload("two")
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 204);
}

#[actix_web::test]
async fn move_copy_delete_and_the_trash_bin() {
    let (_d, c) = cloud();
    let app = app!(c);
    for (m, p) in [
        ("MKCOL", "/remote.php/webdav/A"),
        ("MKCOL", "/remote.php/webdav/B"),
    ] {
        assert_eq!(
            call_service(&app, as_alice(method(m, p)).to_request())
                .await
                .status(),
            201
        );
    }
    call_service(
        &app,
        as_alice(method("PUT", "/remote.php/webdav/A/f.txt"))
            .set_payload("x")
            .to_request(),
    )
    .await;
    let id = c.store.stat("alice", "A/f.txt").unwrap().id;
    let r = call_service(
        &app,
        as_alice(method("MOVE", "/remote.php/webdav/A/f.txt"))
            .insert_header(("destination", "https://box.local/remote.php/webdav/B/g.txt"))
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 201);
    assert_eq!(c.store.stat("alice", "B/g.txt").unwrap().id, id);

    let r = call_service(
        &app,
        as_alice(method("COPY", "/remote.php/webdav/B/g.txt"))
            .insert_header(("destination", "/remote.php/webdav/A/f.txt"))
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 201);
    let r = call_service(
        &app,
        as_alice(method("MOVE", "/remote.php/webdav/B/g.txt"))
            .insert_header(("destination", "/remote.php/webdav/A/f.txt"))
            .insert_header(("overwrite", "F"))
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 412);
    let r = call_service(
        &app,
        as_alice(method("MOVE", "/remote.php/webdav/B/g.txt"))
            .insert_header(("destination", "/remote.php/dav/files/bob/x"))
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 403, "never into another person's files");

    let r = call_service(
        &app,
        as_alice(method("DELETE", "/remote.php/webdav/A")).to_request(),
    )
    .await;
    assert_eq!(r.status(), 204);
    let r = call_service(
        &app,
        as_alice(method("PROPFIND", "/remote.php/dav/trashbin/alice/trash"))
            .insert_header(("depth", "1"))
            .to_request(),
    )
    .await;
    let body = text(r).await;
    assert!(
        body.contains("<nc:trashbin-original-location>A</nc:trashbin-original-location>"),
        "{body}"
    );
    let stored = c.store.trash("alice").unwrap()[0].stored.clone();
    let r = call_service(
        &app,
        as_alice(method(
            "MOVE",
            &format!(
                "/remote.php/dav/trashbin/alice/trash/{}",
                losos_ctl::cloud::util::encode_path(&stored)
            ),
        ))
        .insert_header((
            "destination",
            format!("/remote.php/dav/trashbin/alice/restore/{stored}"),
        ))
        .to_request(),
    )
    .await;
    assert_eq!(r.status(), 201);
    assert!(c.store.stat("alice", "A/f.txt").is_ok());
}

#[actix_web::test]
async fn chunked_upload_v2_joins_in_order() {
    let (_d, c) = cloud();
    let app = app!(c);
    let dest = "https://box.local/remote.php/dav/files/alice/big.bin";
    let r = call_service(
        &app,
        as_alice(method("MKCOL", "/remote.php/dav/uploads/alice/t123"))
            .insert_header(("destination", dest))
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 201);
    for (n, part) in [("2", "world"), ("1", "hello "), ("10", "!")] {
        let r = call_service(
            &app,
            as_alice(method(
                "PUT",
                &format!("/remote.php/dav/uploads/alice/t123/{n}"),
            ))
            .insert_header(("destination", dest))
            .insert_header(("oc-total-length", "12"))
            .set_payload(part)
            .to_request(),
        )
        .await;
        assert_eq!(r.status(), 201);
    }
    let r = call_service(
        &app,
        as_alice(method("PROPFIND", "/remote.php/dav/uploads/alice/t123"))
            .insert_header(("depth", "1"))
            .to_request(),
    )
    .await;
    assert!(text(r)
        .await
        .contains("/remote.php/dav/uploads/alice/t123/10"));
    let r = call_service(
        &app,
        as_alice(method("MOVE", "/remote.php/dav/uploads/alice/t123/.file"))
            .insert_header(("destination", dest))
            .insert_header(("oc-total-length", "12"))
            .insert_header(("x-oc-mtime", "1600000000"))
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 201, "{:?}", r.headers());
    assert!(r.headers().contains_key("oc-fileid"));
    assert_eq!(
        c.store.read_bytes("alice", "big.bin").unwrap(),
        b"hello world!"
    );
    assert_eq!(c.store.stat("alice", "big.bin").unwrap().mtime, 1600000000);
}

#[actix_web::test]
async fn login_flow_v2_end_to_end() {
    let (_d, c) = cloud();
    let app = app!(c);
    let r = call_service(
        &app,
        TestRequest::post()
            .uri("/index.php/login/v2")
            .insert_header(("host", "box.local"))
            .insert_header(("x-forwarded-proto", "https"))
            .insert_header((
                "user-agent",
                "Mozilla/5.0 (Linux) mirall/3.16.0 (Nextcloud)",
            ))
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 200);
    let v: serde_json::Value = serde_json::from_str(&text(r).await).unwrap();
    let login_url = v["login"].as_str().unwrap().to_owned();
    let poll = v["poll"]["token"].as_str().unwrap().to_owned();
    assert_eq!(
        v["poll"]["endpoint"],
        "https://box.local/index.php/login/v2/poll"
    );
    let token = login_url.rsplit('/').next().unwrap().to_owned();

    let poll_req = || {
        TestRequest::post()
            .uri("/index.php/login/v2/poll")
            .insert_header(("host", "box.local"))
            .insert_header(("content-type", "application/x-www-form-urlencoded"))
            .set_payload(format!("token={poll}"))
            .to_request()
    };
    assert_eq!(call_service(&app, poll_req()).await.status(), 404);

    let r = call_service(
        &app,
        TestRequest::get()
            .uri(login_url.trim_start_matches("https://box.local"))
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 302);
    assert_eq!(h(&r, "location"), format!("/cloud/connect/{token}"));

    let r = call_service(
        &app,
        as_alice(TestRequest::get().uri(&format!("/cloud/api/connect/{token}"))).to_request(),
    )
    .await;
    let v: serde_json::Value = serde_json::from_str(&text(r).await).unwrap();
    assert_eq!(v["client"], "Nextcloud desktop on Linux");
    let r = call_service(
        &app,
        as_alice(TestRequest::post().uri(&format!("/cloud/api/connect/{token}")))
            .insert_header(("origin", "https://evil.example"))
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 403, "another site cannot grant it");
    let r = call_service(
        &app,
        as_alice(TestRequest::post().uri(&format!("/cloud/api/connect/{token}")))
            .insert_header(("origin", "https://box.local"))
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 200);

    let r = call_service(&app, poll_req()).await;
    assert_eq!(r.status(), 200);
    let v: serde_json::Value = serde_json::from_str(&text(r).await).unwrap();
    assert_eq!(v["loginName"], "alice");
    assert_eq!(v["server"], "https://box.local");
    let pw = v["appPassword"].as_str().unwrap().to_owned();

    let r = call_service(
        &app,
        TestRequest::get()
            .uri("/ocs/v1.php/cloud/user?format=json")
            .insert_header(("authorization", basic("alice", &pw)))
            .to_request(),
    )
    .await;
    let v: serde_json::Value = serde_json::from_str(&text(r).await).unwrap();
    assert_eq!(v["ocs"]["data"]["id"], "alice");
    assert_eq!(v["ocs"]["data"]["display-name"], "Alice Example");

    let r = call_service(
        &app,
        method("PROPFIND", "/remote.php/dav/files/alice/")
            .insert_header(("authorization", basic("alice", &pw)))
            .insert_header(("depth", "0"))
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 207);

    let r = call_service(
        &app,
        TestRequest::delete()
            .uri("/ocs/v2.php/core/apppassword")
            .insert_header(("authorization", basic("alice", &pw)))
            .insert_header(("ocs-apirequest", "true"))
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 200);
    let r = call_service(
        &app,
        method("PROPFIND", "/remote.php/dav/files/alice/")
            .insert_header(("authorization", basic("alice", &pw)))
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 401, "signed out");
}

#[actix_web::test]
async fn the_web_apps_api() {
    let (_d, c) = cloud();
    let app = app!(c);
    let r = call_service(
        &app,
        as_alice(TestRequest::post().uri("/cloud/api/files/folder"))
            .set_json(serde_json::json!({ "path": "Photos" }))
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 200);
    let r = call_service(
        &app,
        as_alice(TestRequest::put().uri("/cloud/api/files/content?path=Photos%2Fa.jpg"))
            .set_payload("jpeg")
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 200);
    let r = call_service(
        &app,
        as_alice(TestRequest::get().uri("/cloud/api/files?path=Photos")).to_request(),
    )
    .await;
    let v: serde_json::Value = serde_json::from_str(&text(r).await).unwrap();
    assert_eq!(v["entries"][0]["name"], "a.jpg");
    assert_eq!(v["entries"][0]["mime"], "image/jpeg");
    let r = call_service(
        &app,
        as_alice(TestRequest::get().uri("/cloud/api/files/media")).to_request(),
    )
    .await;
    let v: serde_json::Value = serde_json::from_str(&text(r).await).unwrap();
    assert_eq!(v["entries"].as_array().unwrap().len(), 1);
    let r = call_service(
        &app,
        as_alice(TestRequest::post().uri("/cloud/api/files/delete"))
            .set_json(serde_json::json!({ "paths": ["Photos/a.jpg"] }))
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 200);
    let r = call_service(
        &app,
        as_alice(TestRequest::get().uri("/cloud/api/trash")).to_request(),
    )
    .await;
    let v: serde_json::Value = serde_json::from_str(&text(r).await).unwrap();
    assert_eq!(v["entries"][0]["original"], "Photos/a.jpg");
    let r = call_service(
        &app,
        as_alice(TestRequest::get().uri("/cloud/api/files?path=..%2Fbob")).to_request(),
    )
    .await;
    assert_eq!(r.status(), 400);
}

#[actix_web::test]
async fn the_header_is_ignored_unless_trusted() {
    let (_d, c) = cloud();
    let app = init_service(App::new().configure(server::configure(c.clone(), false))).await;
    let r = call_service(
        &app,
        as_alice(method("PROPFIND", "/remote.php/webdav/")).to_request(),
    )
    .await;
    assert_eq!(r.status(), 401);
}

#[actix_web::test]
async fn locks_create_and_guard_a_file() {
    let (_d, c) = cloud();
    let app = app!(c);
    let lockinfo = r#"<?xml version="1.0"?><d:lockinfo xmlns:d="DAV:"><d:lockscope><d:exclusive/></d:lockscope><d:locktype><d:write/></d:locktype><d:owner>finder</d:owner></d:lockinfo>"#;
    let r = call_service(
        &app,
        as_alice(method("LOCK", "/remote.php/webdav/new.txt"))
            .insert_header(("timeout", "Second-600"))
            .set_payload(lockinfo)
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 201);
    let token = h(&r, "lock-token");
    assert!(c.store.stat("alice", "new.txt").is_ok());
    let r = call_service(
        &app,
        as_alice(method("PUT", "/remote.php/webdav/new.txt"))
            .set_payload("x")
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 423);
    let r = call_service(
        &app,
        as_alice(method("PUT", "/remote.php/webdav/new.txt"))
            .insert_header(("if", format!("({token})")))
            .set_payload("x")
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 204);
    let r = call_service(
        &app,
        as_alice(method("UNLOCK", "/remote.php/webdav/new.txt"))
            .insert_header(("lock-token", token.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 204);
}

#[actix_web::test]
async fn search_finds_photos_for_the_phone_app() {
    let (_d, c) = cloud();
    let app = app!(c);
    c.store.ensure_home("alice").unwrap();
    c.store.write_bytes("alice", "a.jpg", b"1").unwrap();
    c.store.write_bytes("alice", "b.txt", b"1").unwrap();
    let q = r#"<?xml version="1.0"?>
<d:searchrequest xmlns:d="DAV:" xmlns:oc="http://owncloud.org/ns">
 <d:basicsearch>
  <d:select><d:prop><d:displayname/><oc:fileid/></d:prop></d:select>
  <d:from><d:scope><d:href>/files/alice</d:href><d:depth>infinity</d:depth></d:scope></d:from>
  <d:where><d:like><d:prop><d:getcontenttype/></d:prop><d:literal>image/%</d:literal></d:like></d:where>
  <d:orderby/>
 </d:basicsearch>
</d:searchrequest>"#;
    let r = call_service(
        &app,
        as_alice(method("SEARCH", "/remote.php/dav/"))
            .insert_header(("content-type", "text/xml"))
            .set_payload(q)
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 207);
    let body = text(r).await;
    assert!(body.contains("a.jpg"));
    assert!(!body.contains("b.txt"));
}

#[actix_web::test]
async fn login_flow_v1_hands_the_phone_app_a_password() {
    let (_d, c) = cloud();
    let app = app!(c);
    // The address the phone app opens in its web view: the connect page.
    let r = call_service(
        &app,
        TestRequest::get()
            .uri("/index.php/login/flow")
            .insert_header(("ocs-apirequest", "true"))
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 302);
    assert_eq!(h(&r, "location"), "/cloud/connect/v1");

    let grant = |origin: &'static str| {
        as_alice(TestRequest::post().uri("/cloud/api/connect-v1"))
            .insert_header(("origin", origin))
            .insert_header(("x-forwarded-proto", "https"))
            .set_json(
                serde_json::json!({ "client": "Mozilla/5.0 (Android) Nextcloud-android/3.30.0" }),
            )
            .to_request()
    };
    assert_eq!(
        call_service(&app, grant("https://evil.example"))
            .await
            .status(),
        403,
        "another site cannot mint a password"
    );
    let r = call_service(&app, grant("https://box.local")).await;
    assert_eq!(r.status(), 200);
    let v: serde_json::Value = serde_json::from_str(&text(r).await).unwrap();
    assert_eq!(v["server"], "https://box.local");
    assert_eq!(v["login"], "alice");
    let pw = v["appPassword"].as_str().unwrap().to_owned();

    let r = call_service(
        &app,
        as_alice(TestRequest::get().uri("/cloud/api/devices")).to_request(),
    )
    .await;
    let v: serde_json::Value = serde_json::from_str(&text(r).await).unwrap();
    assert_eq!(v["devices"][0]["name"], "Nextcloud on Android");

    // An app password cannot mint another one.
    let r = call_service(
        &app,
        TestRequest::post()
            .uri("/cloud/api/connect-v1")
            .insert_header(("host", "box.local"))
            .insert_header(("authorization", basic("alice", &pw)))
            .set_json(serde_json::json!({ "client": "x" }))
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 403);
}

#[actix_web::test]
async fn an_extension_route_gets_the_same_person_or_the_same_401() {
    use losos_ctl::cloud::auth::Who;
    let (_d, c) = cloud();
    let app = init_service(
        App::new()
            .configure(server::configure(c.clone(), true))
            .route(
                "/cloud/api/x/whoami",
                actix_web::web::get()
                    .to(|who: Who| async move { HttpResponse::Ok().body(who.login) }),
            ),
    )
    .await;
    let r = call_service(
        &app,
        as_alice(TestRequest::get().uri("/cloud/api/x/whoami")).to_request(),
    )
    .await;
    assert_eq!(r.status(), 200);
    assert_eq!(text(r).await, "alice");
    let r = call_service(
        &app,
        TestRequest::get().uri("/cloud/api/x/whoami").to_request(),
    )
    .await;
    assert_eq!(r.status(), 401);
    let v: serde_json::Value = serde_json::from_str(&text(r).await).unwrap();
    assert_eq!(v["code"], "signIn");
}

#[actix_web::test]
async fn name_search_is_per_person_newest_first_and_capped() {
    let (_d, c) = cloud();
    let app = app!(c);
    for (who, name) in [
        ("alice", "budget 2025.ods"),
        ("alice", "budget 2026.ods"),
        ("bob", "budget bob.ods"),
    ] {
        let r = call_service(
            &app,
            TestRequest::put()
                .uri(&format!(
                    "/cloud/api/files/content?path={}",
                    name.replace(' ', "%20")
                ))
                .insert_header(("x-losos-cloud-user", who))
                .insert_header(("host", "box.local"))
                .set_payload("x")
                .to_request(),
        )
        .await;
        assert!(r.status().is_success(), "{name}: {}", r.status());
        std::thread::sleep(std::time::Duration::from_millis(1100));
    }
    let r = call_service(
        &app,
        as_alice(TestRequest::get().uri("/cloud/api/files/search?q=budget&limit=1")).to_request(),
    )
    .await;
    let v: serde_json::Value = serde_json::from_str(&text(r).await).unwrap();
    let names: Vec<&str> = v["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["budget 2026.ods"]);
    let r = call_service(
        &app,
        as_alice(TestRequest::get().uri("/cloud/api/files/search?q=budget")).to_request(),
    )
    .await;
    let v: serde_json::Value = serde_json::from_str(&text(r).await).unwrap();
    assert_eq!(
        v["entries"].as_array().unwrap().len(),
        2,
        "bob's file is bob's"
    );
}

#[actix_web::test]
async fn a_name_with_accents_reaches_the_person() {
    let (_d, c) = cloud();
    let app = app!(c);
    let r = call_service(
        &app,
        TestRequest::get()
            .uri("/cloud/api/me")
            .insert_header(("x-losos-cloud-user", "eva"))
            .insert_header((
                "x-losos-cloud-name",
                actix_web::http::header::HeaderValue::from_bytes("Eva Kováč".as_bytes()).unwrap(),
            ))
            .insert_header(("host", "box.local"))
            .to_request(),
    )
    .await;
    let v: serde_json::Value = serde_json::from_str(&text(r).await).unwrap();
    assert_eq!(v["name"], "Eva Kováč");
}

/// Office over the cloud's own files: the web app opens one of the person's
/// documents, and coolwsd reads and saves it on `/wopi/files/` of the same
/// listener with the token alone. Collabora's discovery is a saved copy, as
/// on the box.
#[actix_web::test]
async fn office_opens_and_saves_a_cloud_document() {
    use losos_ctl::cloud::office::CloudFiles;
    use losos_ctl::office::actix::Sessions;
    use losos_ctl::office::discovery::Coolwsd;
    use losos_ctl::office::{TokenKey, Wopi};

    let (d, c) = cloud();
    let discovery = d.path().join("discovery.xml");
    std::fs::write(
        &discovery,
        r#"<wopi-discovery><net-zone name="external-http"><app name="writer">
<action default="true" ext="docx" name="edit" urlsrc="https://box.local/browser/f00d/cool.html?"/>
</app></net-zone></wopi-discovery>"#,
    )
    .unwrap();
    let files = Arc::new(CloudFiles { cloud: c.clone() });
    let _ = c.office.set(Arc::new(Sessions {
        wopi: Arc::new(Wopi::new(
            files,
            TokenKey::from_bytes(&[7; 32]),
            "http://127.0.0.1:8093",
        )),
        coolwsd: Coolwsd::new(discovery.to_str().unwrap()),
    }));
    c.store.ensure_home("alice").unwrap();
    let doc = c
        .store
        .write_bytes("alice", "Plan.docx", b"version one")
        .unwrap()
        .entry;
    c.store.write_bytes("alice", "notes.txt", b"x").unwrap();
    let app = app!(c);

    // The types Collabora opens.
    let r = call_service(
        &app,
        as_alice(TestRequest::get().uri("/cloud/api/office/extensions")).to_request(),
    )
    .await;
    assert_eq!(r.status(), 200);
    assert_eq!(
        text(r).await,
        r#"{"extensions":[{"extension":"docx","edit":true}]}"#
    );

    // Nobody signed in, another site, someone else's file, a type it does
    // not open: refused.
    let open = |id: String| {
        TestRequest::post()
            .uri("/cloud/api/office/open")
            .insert_header(("origin", "https://box.local"))
            .set_json(serde_json::json!({ "fileId": id }))
    };
    let r = call_service(&app, open(doc.id.to_string()).to_request()).await;
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
    let r = call_service(
        &app,
        as_alice(open(doc.id.to_string()))
            .insert_header(("origin", "https://evil.example"))
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), StatusCode::FORBIDDEN);
    let r = call_service(
        &app,
        open(doc.id.to_string())
            .insert_header(("x-losos-cloud-user", "bob"))
            .insert_header(("host", "box.local"))
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), StatusCode::NOT_FOUND);
    let txt = c.store.stat("alice", "notes.txt").unwrap();
    let r = call_service(&app, as_alice(open(txt.id.to_string())).to_request()).await;
    assert_eq!(r.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // Alice's own document: a session on this origin.
    let r = call_service(&app, as_alice(open(doc.id.to_string())).to_request()).await;
    assert_eq!(r.status(), 200);
    let session: serde_json::Value = serde_json::from_str(&text(r).await).unwrap();
    assert_eq!(session["name"], "Plan.docx");
    assert_eq!(session["writable"], true);
    assert_eq!(session["https"], true);
    let url = session["url"].as_str().unwrap();
    assert!(url.starts_with("/browser/f00d/cool.html?WOPISrc="), "{url}");
    let token = session["accessToken"].as_str().unwrap();

    // coolwsd: the file's facts, its contents, a save. No sign-in, only
    // the token, and only for that file.
    let wopi = format!("/wopi/files/{}", doc.id);
    let r = call_service(
        &app,
        TestRequest::get()
            .uri(&format!("{wopi}?access_token={token}"))
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 200);
    let info: serde_json::Value = serde_json::from_str(&text(r).await).unwrap();
    assert_eq!(info["BaseFileName"], "Plan.docx");
    assert_eq!(info["UserFriendlyName"], "Alice Example");
    let r = call_service(
        &app,
        TestRequest::get()
            .uri(&format!("{wopi}/contents?access_token={token}"))
            .to_request(),
    )
    .await;
    assert_eq!(text(r).await, "version one");
    // Collabora locks the file before its first save, as WOPI asks.
    let r = call_service(
        &app,
        TestRequest::post()
            .uri(&format!("{wopi}?access_token={token}"))
            .insert_header(("X-WOPI-Override", "LOCK"))
            .insert_header(("X-WOPI-Lock", "cool-1"))
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 200);
    let r = call_service(
        &app,
        TestRequest::post()
            .uri(&format!("{wopi}/contents?access_token={token}"))
            .insert_header(("X-WOPI-Override", "PUT"))
            .insert_header(("X-WOPI-Lock", "cool-1"))
            .set_payload("version two")
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 200);
    assert_eq!(
        c.store.read_bytes("alice", "Plan.docx").unwrap(),
        b"version two"
    );
    let r = call_service(
        &app,
        TestRequest::get()
            .uri(&format!("{wopi}/contents?access_token=nope"))
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
    let r = call_service(
        &app,
        TestRequest::get()
            .uri(&format!("/wopi/files/{}?access_token={token}", txt.id))
            .to_request(),
    )
    .await;
    assert_ne!(r.status(), 200, "the token is for one file");
}
