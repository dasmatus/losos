//! `desired_config` — the pure core that decides exactly what Traefik and
//! the tunnel are told.
//!
//! No IO, no clock, no randomness, so these assert on bytes and on the parsed
//! documents. The registrar is the sole writer of both files, which is what
//! makes the odd-input cases below severe rather than pedantic: one
//! unparseable tunnel file would keep every tunnel on the edge at its last
//! state, not just the tenant that caused it.

use losos_registrar::tunnel::{ClientFile, ServerFile};
use losos_registrar::{desired_config, TenantView};

fn tenant(id: &str, hostname: &str, port: u16, token: &str) -> TenantView {
    TenantView {
        id: id.to_string(),
        hostname: hostname.to_string(),
        tunnel_port: port,
        token: token.to_string(),
        llm_port: None,
    }
}

fn tunnel(files: &losos_registrar::Files) -> ServerFile {
    serde_json::from_str(&files.tunnel_json).expect("the tunnel file is JSON it reads back")
}

#[test]
fn zero_tenants_yield_no_traefik_file_and_a_tunnel_file_with_no_services() {
    let files = desired_config(&[]);
    // Traefik's file provider rejects an empty dynamic config, so the
    // zero-tenant state is "no losos.yml", not "an empty losos.yml".
    assert!(files.traefik_yaml.is_none());
    assert!(tunnel(&files).services.is_empty());
    assert_eq!(files.tunnel_json, "{\n  \"services\": {}\n}\n");
}

#[test]
fn one_tenant_renders_a_router_a_service_and_a_tunnel() {
    let files = desired_config(&[tenant("mattbox", "mattbox.losos.cfd", 50000, "TOK")]);
    let yaml = files
        .traefik_yaml
        .clone()
        .expect("one tenant yields a traefik config");

    // Assert on the parsed document rather than substrings: this is the shape
    // Traefik actually reads, and it is checked without leaning on the
    // crate's own structs agreeing with themselves.
    let doc: serde_yaml::Value = serde_yaml::from_str(&yaml).expect("losos.yml is valid YAML");
    let router = &doc["http"]["routers"]["mattbox"];
    assert_eq!(router["rule"].as_str(), Some("Host(`mattbox.losos.cfd`)"));
    assert_eq!(router["service"].as_str(), Some("mattbox"));
    assert_eq!(router["entryPoints"][0].as_str(), Some("websecure"));
    assert_eq!(router["tls"]["certResolver"].as_str(), Some("le"));
    assert_eq!(
        router["tls"]["domains"][0]["main"].as_str(),
        Some("mattbox.losos.cfd")
    );
    let servers = &doc["http"]["services"]["mattbox"]["loadBalancer"]["servers"];
    assert_eq!(servers[0]["url"].as_str(), Some("http://127.0.0.1:50000"));

    let file = tunnel(&files);
    let svc = &file.services["mattbox"];
    assert_eq!(svc.token, "TOK");
    assert_eq!(svc.bind.to_string(), "127.0.0.1:50000");
    assert_eq!(file.services.len(), 1);
}

/// The same tenants in another order give the same bytes, so the
/// reconciler's byte-compare does not rewrite the file for nothing.
#[test]
fn the_tunnel_file_does_not_depend_on_tenant_order() {
    let a = tenant("alpha", "a.losos.cfd", 50000, "A");
    let b = tenant("beta", "b.losos.cfd", 50001, "B");
    let one = desired_config(&[a.clone(), b.clone()]);
    let two = desired_config(&[b, a]);
    assert_eq!(one, two);
}

/// Ids and tokens carrying quotes, newlines or control characters cannot
/// break the file or smuggle a service in: they come back exactly as given.
#[test]
fn odd_ids_and_tokens_round_trip() {
    let raw = "a\"b\\c\nd\u{1}e";
    let files = desired_config(&[tenant(raw, "odd.losos.cfd", 50000, raw)]);
    let file = tunnel(&files);
    assert_eq!(file.services.len(), 1);
    assert_eq!(file.services[raw].token, raw);
}

/// The spoke's uplink client file: the hub's endpoint and pin, one service
/// per relayed box named `<spoke>.<box>` with the spoke's own token and the
/// spoke-local port; a complete file with no services when nothing is
/// relayed.
#[test]
fn uplink_config_renders_the_relayed_services() {
    use losos_registrar::{uplink_config, UplinkService, UplinkTarget};
    let target = UplinkTarget {
        tunnel_endpoint: "hub.losos.cfd:2333".to_string(),
        pin: "5e".repeat(32),
        token: "spoke-token".to_string(),
    };
    let empty: ClientFile = serde_json::from_str(&uplink_config(&target, &[])).unwrap();
    assert_eq!(empty.remote, "hub.losos.cfd:2333");
    assert_eq!(empty.pin, "5e".repeat(32));
    assert!(empty.services.is_empty());
    let two = uplink_config(
        &target,
        &[
            UplinkService {
                name: "acme.zed".to_string(),
                local_port: 50002,
            },
            UplinkService {
                name: "acme.alpha".to_string(),
                local_port: 50001,
            },
        ],
    );
    assert!(
        two.find("acme.alpha").unwrap() < two.find("acme.zed").unwrap(),
        "{two}"
    );
    let two: ClientFile = serde_json::from_str(&two).unwrap();
    assert_eq!(two.services["acme.alpha"].token, "spoke-token");
    assert_eq!(two.services["acme.alpha"].local, "127.0.0.1:50001");
    assert_eq!(two.services["acme.zed"].local, "127.0.0.1:50002");
}

#[test]
fn a_lending_box_gets_a_second_tunnel_service_on_its_llm_port() {
    let mut lender = tenant("box-a", "a.example.org", 20001, "tok-a");
    lender.llm_port = Some(23001);
    let other = tenant("box-b", "b.example.org", 20002, "tok-b");
    // Tenants whose ids end the way older service names did: a direct one,
    // and a box called `llm` relayed by a spoke called `box-a`.
    let lookalike = tenant("box-a-llm", "c.example.org", 20003, "tok-c");
    let relayed = tenant("box-a.llm", "llm.box-a.example.org", 20004, "tok-d");
    let files = desired_config(&[lender, other, lookalike, relayed]);
    let file = tunnel(&files);
    let llm = &file.services["box-a:llm"];
    assert_eq!(llm.token, "tok-a");
    assert_eq!(llm.bind.to_string(), "127.0.0.1:23001");
    assert!(!file.services.contains_key("box-b:llm"));
    assert_eq!(file.services["box-a-llm"].bind.port(), 20003);
    assert_eq!(file.services["box-a.llm"].token, "tok-d");
    assert_eq!(file.services.len(), 5);
    // The model port never becomes a web route.
    let yaml = files.traefik_yaml.unwrap();
    assert!(!yaml.contains("23001"), "{yaml}");
    assert!(!yaml.contains("box-a:llm"), "{yaml}");
}
