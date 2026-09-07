//! `desired_config` — the pure core that decides exactly what Traefik and
//! rathole are told.
//!
//! No IO, no clock, no randomness, so these assert on bytes. The registrar is
//! the sole writer of both files, which is what makes the escaping cases below
//! severe rather than pedantic: one unparseable `server.toml` drops every
//! tunnel on the edge, not just the tenant that caused it.

use losos_registrar::{desired_config, EdgeOpts, TenantView};

fn opts() -> EdgeOpts {
    EdgeOpts {
        rathole_bind_addr: "0.0.0.0".to_string(),
        rathole_bind_port: 2333,
        bootstrap_token: "BOOT".to_string(),
    }
}

fn tenant(id: &str, hostname: &str, port: u16, token: &str) -> TenantView {
    TenantView {
        id: id.to_string(),
        hostname: hostname.to_string(),
        rathole_port: port,
        token: token.to_string(),
    }
}

#[test]
fn zero_tenants_yield_no_traefik_file_and_an_empty_rathole_services_table() {
    let files = desired_config(&[], &opts());
    // Traefik's file provider rejects an empty dynamic config, so the
    // zero-tenant state is "no losos.yml", not "an empty losos.yml".
    assert!(files.traefik_yaml.is_none());
    // rathole, by contrast, requires the `services` key to be present.
    assert_eq!(
        files.rathole_toml,
        "[server]\nbind_addr = \"0.0.0.0:2333\"\ndefault_token = \"BOOT\"\n\n[server.services]\n"
    );
}

#[test]
fn one_tenant_renders_a_router_a_service_and_a_tunnel() {
    let files = desired_config(
        &[tenant("mattbox", "mattbox.losos.cfd", 50000, "TOK")],
        &opts(),
    );
    let yaml = files
        .traefik_yaml
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

    assert!(files.rathole_toml.contains("[server.services.mattbox]"));
    assert!(files
        .rathole_toml
        .contains("bind_addr = \"127.0.0.1:50000\""));
    assert!(files.rathole_toml.contains("token = \"TOK\""));
}

/// rathole parses `bind_addr` as a `SocketAddr`, and `::2333` is an address
/// literal rather than a socket — the production default `::` (dual-stack)
/// has to be bracketed.
#[test]
fn an_ipv6_bind_address_is_bracketed() {
    let files = desired_config(
        &[],
        &EdgeOpts {
            rathole_bind_addr: "::".to_string(),
            rathole_bind_port: 2333,
            bootstrap_token: "BOOT".to_string(),
        },
    );
    assert!(
        files.rathole_toml.contains("bind_addr = \"[::]:2333\""),
        "got: {}",
        files.rathole_toml
    );
}

/// Byte-identical output for the same tenants in any order, so the
/// reconciler's write-if-changed compare never fires spuriously.
#[test]
fn output_does_not_depend_on_input_order() {
    let a = tenant("alpha", "a.losos.cfd", 50000, "ta");
    let b = tenant("beta", "b.losos.cfd", 50001, "tb");
    assert_eq!(
        desired_config(&[a.clone(), b.clone()], &opts()),
        desired_config(&[b, a], &opts())
    );
}

/// A quote or backslash in a token must not end the TOML string early.
#[test]
fn quotes_and_backslashes_are_escaped() {
    let files = desired_config(
        &[tenant("box", "box.losos.cfd", 50000, "plain")],
        &EdgeOpts {
            rathole_bind_addr: "0.0.0.0".to_string(),
            rathole_bind_port: 2333,
            bootstrap_token: r#"a"b\c"#.to_string(),
        },
    );
    assert!(
        files.rathole_toml.contains(r#"default_token = "a\"b\\c""#),
        "got: {}",
        files.rathole_toml
    );
}

/// TOML basic strings forbid raw control characters, and the registrar is the
/// sole writer of `server.toml` — so a tenant id or token carrying a newline
/// used to emit a file rathole cannot parse, dropping **every** tunnel on the
/// edge at the next reconcile, not just that tenant's. The old escaper handled
/// `"` and `\` and nothing else.
#[test]
fn control_characters_in_a_token_are_escaped() {
    let raw = "line\nfeed\ttab\rcr\u{8}bs\u{c}ff\u{0}nul\u{1f}us\u{7f}del";
    let files = desired_config(
        &[tenant("box", "box.losos.cfd", 50000, raw)],
        &EdgeOpts {
            rathole_bind_addr: "0.0.0.0".to_string(),
            rathole_bind_port: 2333,
            bootstrap_token: raw.to_string(),
        },
    );
    let toml = files.rathole_toml;

    let escaped = r"line\nfeed\ttab\rcr\bbs\fff\u0000nul\u001fus\u007fdel";
    assert!(
        toml.contains(&format!("token = \"{escaped}\"")),
        "token was not fully escaped; got: {toml:?}"
    );
    assert!(
        toml.contains(&format!("default_token = \"{escaped}\"")),
        "bootstrap token was not fully escaped; got: {toml:?}"
    );
    // The decisive property: no value in the emitted file contains a raw
    // control character, so every line is still the line it claims to be.
    for (n, line) in toml.lines().enumerate() {
        assert!(
            !line.chars().any(|c| c.is_control()),
            "line {n} carries a raw control character: {line:?}"
        );
    }
    // A quoted value must contain exactly two unescaped quotes: the delimiters.
    let token_line = toml
        .lines()
        .find(|l| l.starts_with("token = "))
        .expect("a token line");
    assert!(
        token_line.ends_with('"'),
        "the token value did not terminate on its own line: {token_line:?}"
    );
}

/// An id that is not a bare TOML key must be quoted *and* escaped — the same
/// hazard, one layer up, since the id becomes a table header.
#[test]
fn a_non_bare_tenant_id_is_quoted_and_escaped() {
    let files = desired_config(
        &[tenant("odd id\n[server]", "odd.losos.cfd", 50000, "TOK")],
        &opts(),
    );
    let toml = files.rathole_toml;
    assert!(
        toml.contains(r#"[server.services."odd id\n[server]"]"#),
        "got: {toml:?}"
    );
    // The injected `[server]` must not have become a second table header.
    assert_eq!(
        toml.lines().filter(|l| l.trim() == "[server]").count(),
        1,
        "a tenant id smuggled a table header into server.toml: {toml:?}"
    );
}
