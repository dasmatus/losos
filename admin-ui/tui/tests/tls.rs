//! The certificate check: a box's self-signed certificate is taken at login,
//! pinned with the host, and every later call must see the same one.

mod common;

use std::sync::Arc;

use common::{mock_tls, owner, Cert, Client, TOKEN};
use losos::http::{Http, HttpError, Trust};
use losos::keychain::MemoryKeychain;
use losos::vault::Vault;
use serde_json::json;

fn plain(cert: &Cert) -> String {
    let server = tiny_http::Server::https("127.0.0.1:0", cert.config()).unwrap();
    let url = format!("https://{}", server.server_addr().to_ip().unwrap());
    std::thread::spawn(move || {
        for request in server.incoming_requests() {
            let _ = request.respond(tiny_http::Response::from_string("{}"));
        }
    });
    url
}

#[test]
fn a_self_signed_certificate_is_taken_only_when_asked_and_then_pinned() {
    let a = Cert::new();
    let b = Cert::new();
    let (url_a, url_b) = (plain(&a), plain(&b));

    // No public authority vouches for it.
    let refused = Http::new(Trust::Public).get(&url_a, "application/json");
    assert!(
        matches!(refused, Err(HttpError::Unreachable(_))),
        "{refused:?}"
    );

    // Login takes it as shown and says which one it took.
    let http = Http::new(Trust::FirstUse);
    assert_eq!(http.get(&url_a, "application/json").unwrap().status, 200);
    assert_eq!(http.taken_certificate(), Some(a.pin()));

    // Every call after it must see that one.
    let pinned = Http::new(Trust::Pinned(a.pin()));
    assert_eq!(pinned.get(&url_a, "application/json").unwrap().status, 200);
    let other = Http::new(Trust::Pinned(a.pin())).get(&url_b, "application/json");
    assert_eq!(other.unwrap_err(), HttpError::CertificateChanged);
}

#[test]
fn login_pins_a_box_certificate_and_calls_go_through_it() {
    let me = owner();
    let cert = Cert::new();
    let host = mock_tls(
        false,
        me.key.armored_public(),
        Box::new(|_, _, _| (200, json!({ "mode": "local" }))),
        &cert,
    );
    let keychain = Arc::new(MemoryKeychain::new());
    let client = Client::with(keychain.clone());
    client.run(&["key", "import", me.file.path().to_str().unwrap()], "");
    let r = client.run(
        &["login", &host.url, "--fingerprint", &host.fingerprint()],
        TOKEN,
    );
    assert_eq!(r.code, 0, "{}", r.err);
    assert!(r.out.contains("TLS"), "{}", r.out);

    let record = Vault::new(keychain).get_host(&host.url).unwrap().unwrap();
    assert_eq!(record.tls_pin, Some(cert.pin()));

    let r = client.run(&["call", "GET", "/api/state"], "");
    assert_eq!(r.code, 0, "{}", r.err);
}
