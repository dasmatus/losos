//! Pure config generation. `desired_config` takes the live tenant set and
//! edge options and returns the exact bytes to write to Traefik's dynamic
//! file-provider dir and rathole's server config. No IO, no time, no
//! randomness — fully deterministic and unit-testable.
//!
//! Both files are regenerated in full on every reconcile (the tenant set is
//! small), so the reconciler byte-compares old-vs-new and only writes/SIGHUPs
//! when something actually changed.
//!
//! The Traefik dynamic config is modeled as typed serde structs ([`TraefikConfig`]
//! and friends) so its shape — routers and services nested under `http`, each
//! router's `tls` block, the load-balancer server list — is enforced at
//! compile time. The earlier hand-rolled YAML could produce a structurally
//! invalid file (e.g. an empty `routers: {}`) that Traefik only rejected at
//! runtime ("routers cannot be a standalone element"); the type model makes
//! that unrepresentable.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// A live tenant, from the registry's point of view, reduced to what the
/// config needs. `last_seen` is deliberately absent here — it is a runtime
/// concern of `registry`, not of config generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TenantView {
    pub id: String,
    pub hostname: String,
    pub rathole_port: u16,
    /// The per-appliance rathole service token. Populated by the reconciler
    /// (not the registry) by reading the tenant's token file, so the on-disk
    /// token file stays the single source of truth and tokens auto-rotate.
    pub token: String,
}

/// Edge-wide options baked into the generated config.
#[derive(Debug, Clone)]
pub struct EdgeOpts {
    /// rathole `[server] bind_addr` — the address appliance clients dial.
    pub rathole_bind_addr: String,
    pub rathole_bind_port: u16,
    /// rathole `default_token` (also the bootstrap shared secret).
    pub bootstrap_token: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Files {
    /// The Traefik dynamic config, or `None` when there are no live tenants.
    /// Traefik's file provider **rejects** an empty dynamic config — both
    /// `http: {}` and `http:\n  routers: {}\n  services: {}` error as
    /// "<key> cannot be a standalone element" — so the zero-tenant state is
    /// "no `losos.yml`", represented as `None`: the reconciler removes the
    /// file and Traefik drops every appliance router. Contrast rathole, which
    /// accepts an empty `[server.services]` table and is always written.
    pub traefik_yaml: Option<String>,
    pub rathole_toml: String,
}

// ──────────────────────────────────────────────────────────────────────────
// Traefik dynamic file-provider config — typed model.
//
// These mirror the subset of Traefik's dynamic config the registrar emits
// (https://doc.traefik.io/traefik/reference/dynamic-configuration/file/).
// `#[serde(rename_all = "camelCase")]` maps the snake_case Rust fields to the
// camelCase keys Traefik expects (entryPoints, certResolver, loadBalancer).
// `BTreeMap` gives deterministic key order so the serialized output is stable
// across runs (the reconciler byte-compares old vs new).
//
// Only the parts the registrar actually writes are modeled — adding a field
// Traefik doesn't understand is a compile error against these structs, not a
// runtime "unknown field" from Traefik.

/// Root of a Traefik dynamic config file (`losos.yml`).
#[derive(Debug, Serialize, Deserialize)]
struct TraefikConfig {
    http: TraefikHttp,
}

/// The `http:` top-level section: one router and one service per appliance.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TraefikHttp {
    routers: BTreeMap<String, TraefikRouter>,
    services: BTreeMap<String, TraefikService>,
}

/// A per-appliance router: match `Host(<hostname>)`, forward to the service of
/// the same id, terminate TLS with the on-demand Let's Encrypt resolver.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TraefikRouter {
    rule: String,
    service: String,
    entry_points: Vec<String>,
    tls: TraefikTls,
}

/// Per-router TLS: the `le` cert resolver and the hostname to obtain a cert
/// for. The registrar is the gatekeeper of which hostnames ever get certs —
/// Traefik obtains lazily on first hit for a router that exists.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TraefikTls {
    cert_resolver: String,
    domains: Vec<TraefikDomain>,
}

/// A `domains:` entry — `main` is the cert's primary SAN.
#[derive(Debug, Serialize, Deserialize)]
struct TraefikDomain {
    main: String,
}

/// A per-appliance service: a load balancer with one server — the rathole
/// edge port for this appliance (`http://127.0.0.1:<rathole_port>`), which
/// rathole forwards over the tunnel to the appliance's Nginx.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TraefikService {
    load_balancer: TraefikLoadBalancer,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TraefikLoadBalancer {
    servers: Vec<TraefikServer>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TraefikServer {
    url: String,
}

/// Render the Traefik dynamic file-provider config and the rathole server
/// config for the given live tenants.
///
/// Traefik: one router + one service per tenant, `Host(<hostname>)` →
/// `http://127.0.0.1:<rathole_port>`, with per-router `tls.certResolver: le`
/// and `tls.domains: [{main: <hostname>}]`. Traefik obtains the Let's Encrypt
/// cert lazily on the first hit for that router — so the *registrar* is the
/// gatekeeper of which hostnames ever get certs: it only writes routers for
/// authenticated tenants.
///
/// rathole: one `[server.services.<id>]` per tenant, `bind_addr` loopback
/// (only Traefik, on the same host, reaches it), `token` = the per-appliance
/// token. The appliance client presents the same token; rathole matches it to
/// the service and forwards `127.0.0.1:<rathole_port>` traffic over the tunnel
/// to the appliance's local Nginx.
///
/// Note: rathole's server config keys are `bind_addr` (a scalar string), not
/// `bind` (a list). The `[server]` block emitted here for zero tenants is
/// exactly what `seed` (see [`crate::seed::run`]) writes to disk on first
/// boot, before `serve`'s reconciler has run — both call this same function
/// — so steady-state (no tenants) is zero churn: the reconciler's first
/// compare sees no change.
#[must_use]
pub fn desired_config(tenants: &[TenantView], opts: &EdgeOpts) -> Files {
    // rathole [server] base. The [server.services] table is appended below —
    // either empty (zero tenants) or one [server.services.<id>] per tenant.
    // For zero tenants this is exactly what `seed::run` writes to disk on
    // first boot — it calls this same function, so there is nothing else to
    // keep in sync.
    let mut toml = format!(
        "[server]\nbind_addr = \"{bind}\"\ndefault_token = \"{tok}\"\n",
        bind = format_bind(&opts.rathole_bind_addr, opts.rathole_bind_port),
        tok = toml_escape(&opts.bootstrap_token),
    );

    if tenants.is_empty() {
        // rathole's server config requires a `services` field (serde rejects a
        // `[server]` block with no services table: "missing field `services`"),
        // so emit an empty `[server.services]` table: rathole starts, listens
        // on `bind_addr`, and watches the file for hot-reload — the registrar
        // later rewrites it with real `[server.services.<id>]` blocks as
        // appliances register. This empty rathole form is exactly what `seed`
        // writes to disk on first boot (it calls this same function with no
        // tenants), so a freshly-booted edge produces zero rathole churn on
        // its first reconcile.
        //
        // Traefik, by contrast, rejects an empty dynamic config (see `Files`),
        // so the zero-tenant Traefik state is `None` — no losos.yml — not an
        // empty file. The reconciler removes the file.
        toml.push_str("\n[server.services]\n");
        return Files {
            traefik_yaml: None,
            rathole_toml: toml,
        };
    }

    // Sort by id so the rathole per-service blocks (emitted in iteration
    // order) match the Traefik BTreeMap (key order) — the output is then
    // deterministic regardless of the order callers pass tenants in. The
    // registry already sorts `views()`, but `desired_config` must not lean on
    // that: a caller that passed the same tenants in a different order would
    // otherwise produce byte-identical Traefik config (BTreeMap) but a
    // different rathole block order, byte-compare as "changed", and trigger a
    // spurious rathole SIGHUP + reload.
    let mut sorted: Vec<&TenantView> = tenants.iter().collect();
    sorted.sort_by(|a, b| a.id.cmp(&b.id));

    // BTreeMap gives deterministic ordering by id so byte-compare is stable
    // across runs regardless of input iteration order. serde_yaml quotes the
    // map keys only when the id needs it (it handles YAML key escaping; the
    // old hand-rolled `yaml_key` helper is no longer needed).
    let mut routers: BTreeMap<String, TraefikRouter> = BTreeMap::new();
    let mut services: BTreeMap<String, TraefikService> = BTreeMap::new();

    for t in sorted {
        let id = t.id.clone();
        let host = t.hostname.clone();
        routers.insert(
            id.clone(),
            TraefikRouter {
                rule: format!("Host(`{host}`)"),
                service: id.clone(),
                entry_points: vec!["websecure".to_string()],
                tls: TraefikTls {
                    cert_resolver: "le".to_string(),
                    domains: vec![TraefikDomain { main: host.clone() }],
                },
            },
        );
        services.insert(
            id.clone(),
            TraefikService {
                load_balancer: TraefikLoadBalancer {
                    servers: vec![TraefikServer {
                        url: format!("http://127.0.0.1:{port}", port = t.rathole_port),
                    }],
                },
            },
        );

        // rathole per-service block: token + loopback bind Traefik hits.
        toml.push_str(&format!(
            "\n[server.services.{id}]\ntoken = \"{tok}\"\nbind_addr = \"127.0.0.1:{port}\"\n",
            id = toml_key(&t.id),
            tok = toml_escape(&t.token),
            port = t.rathole_port,
        ));
    }

    // Serializing these structs is infallible in practice: every field is a
    // String/Vec/BTreeMap<String,_>, so there are no enums, no unbounded
    // recursion, and no non-string keys for serde_yaml to choke on.
    let traefik_yaml = serde_yaml::to_string(&TraefikConfig {
        http: TraefikHttp { routers, services },
    })
    .expect("serializing TraefikConfig is infallible for String/Vec/BTreeMap fields");

    Files {
        traefik_yaml: Some(traefik_yaml),
        rathole_toml: toml,
    }
}

/// Format a `host:port` socket string for rathole's `bind_addr`, bracketing
/// IPv6 literals: `::` → `[::]:2333`, `0.0.0.0` → `0.0.0.0:2333`. rathole
/// parses `bind_addr` as a `SocketAddr`, so an unbracketed IPv6 literal
/// (`::2333`) is rejected. The default `rathole_bind_addr` is `::` (dual-stack
/// — Linux accepts IPv4-mapped connections on an `::` bind), so an appliance
/// that resolves the edge over IPv6 reaches the tunnel. Both `seed` and
/// `serve`'s reconciler render through this same helper, so there is no
/// second implementation that could drift out of bracketing-lockstep.
fn format_bind(addr: &str, port: u16) -> String {
    if addr.contains(':') {
        format!("[{addr}]:{port}")
    } else {
        format!("{addr}:{port}")
    }
}

/// Escape `\` and `"` for a TOML basic-string value. (rathole tokens are
/// random; this only matters if one ever contains a quote or backslash.)
fn toml_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn toml_key(s: &str) -> String {
    // Bare TOML keys allow A-Za-z0-9-_ only; quote anything else.
    let bare = !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if bare {
        s.to_string()
    } else {
        format!("\"{}\"", toml_escape(s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> EdgeOpts {
        EdgeOpts {
            rathole_bind_addr: "0.0.0.0".to_string(),
            rathole_bind_port: 2333,
            bootstrap_token: "BOOT".to_string(),
        }
    }

    #[test]
    fn empty_tenants_yield_empty_config() {
        let f = desired_config(&[], &opts());
        // Zero tenants → no losos.yml (Traefik rejects an empty dynamic config).
        assert!(f.traefik_yaml.is_none());
        // rathole still gets an (empty-services) config, byte-identical to
        // what the `seed` subcommand writes on first boot.
        assert_eq!(
            f.rathole_toml,
            "[server]\nbind_addr = \"0.0.0.0:2333\"\ndefault_token = \"BOOT\"\n\n[server.services]\n"
        );
    }

    #[test]
    fn one_tenant_renders_router_service_and_rathole_service() {
        let t = TenantView {
            id: "mattbox".into(),
            hostname: "mattbox.losos.cfd".into(),
            rathole_port: 50000,
            token: "TOK".into(),
        };
        let f = desired_config(&[t], &opts());
        let yaml = f.traefik_yaml.expect("one tenant yields a traefik config");
        // Substring checks are deliberately quote-agnostic — serde_yaml picks
        // the quoting style, which we don't pin (Traefik accepts either).
        assert!(yaml.contains("Host(`mattbox.losos.cfd`)"));
        assert!(yaml.contains("certResolver: le"));
        assert!(yaml.contains("mattbox.losos.cfd"));
        assert!(yaml.contains("http://127.0.0.1:50000"));
        assert!(f.rathole_toml.contains("[server.services.mattbox]"));
        assert!(f.rathole_toml.contains("bind_addr = \"127.0.0.1:50000\""));
        assert!(f.rathole_toml.contains("default_token = \"BOOT\""));
        assert!(f.rathole_toml.contains("token = \"TOK\""));
    }

    /// The serialized `losos.yml` must deserialize back into the typed model —
    /// the payoff for modeling Traefik's config as structs. This round-trip
    /// pins that the shape the registrar emits is exactly the shape the
    /// (compile-time-checked) schema describes: routers and services nested
    /// under `http`, the `tls` block with `certResolver` + `domains`, and the
    /// load-balancer server URL. If a field is misnamed or misplaced, this
    /// fails instead of Traefik rejecting the file at runtime.
    #[test]
    fn traefik_config_round_trips_through_yaml() {
        let t = TenantView {
            id: "mattbox".into(),
            hostname: "mattbox.losos.cfd".into(),
            rathole_port: 50000,
            token: "TOK".into(),
        };
        let f = desired_config(&[t], &opts());
        let yaml = f.traefik_yaml.expect("one tenant yields a traefik config");
        let parsed: TraefikConfig =
            serde_yaml::from_str(&yaml).expect("losos.yml round-trips into TraefikConfig");

        let router = parsed.http.routers.get("mattbox").expect("router present");
        assert_eq!(router.rule, "Host(`mattbox.losos.cfd`)");
        assert_eq!(router.service, "mattbox");
        assert_eq!(router.entry_points, ["websecure"]);
        assert_eq!(router.tls.cert_resolver, "le");
        assert_eq!(router.tls.domains.len(), 1);
        assert_eq!(router.tls.domains[0].main, "mattbox.losos.cfd");

        let svc = parsed.http.services.get("mattbox").expect("service present");
        assert_eq!(svc.load_balancer.servers.len(), 1);
        assert_eq!(svc.load_balancer.servers[0].url, "http://127.0.0.1:50000");
    }

    /// An IPv6 `bind_addr` (the production default `::`, dual-stack) must be
    /// bracketed so rathole parses it as a `SocketAddr` — `::2333` is an IPv6
    /// address literal, not a socket. This pins the bracketing `format_bind`
    /// must produce for both `seed` and `serve`'s reconciler.
    #[test]
    fn ipv6_bind_addr_is_bracketed() {
        let o = EdgeOpts {
            rathole_bind_addr: "::".into(),
            rathole_bind_port: 2333,
            bootstrap_token: "BOOT".into(),
        };
        let f = desired_config(&[], &o);
        assert!(
            f.rathole_toml.contains("bind_addr = \"[::]:2333\""),
            "expected bracketed IPv6 bind_addr, got: {}",
            f.rathole_toml,
        );
        assert!(f.traefik_yaml.is_none());
    }

    #[test]
    fn output_is_deterministic_regardless_of_input_order() {
        let a = TenantView { id: "alpha".into(), hostname: "a.losos.cfd".into(), rathole_port: 50000, token: "ta".into() };
        let b = TenantView { id: "beta".into(), hostname: "b.losos.cfd".into(), rathole_port: 50001, token: "tb".into() };
        let f1 = desired_config(&[a.clone(), b.clone()], &opts());
        let f2 = desired_config(&[b, a], &opts());
        assert_eq!(f1, f2);
    }

    #[test]
    fn special_chars_in_token_are_escaped() {
        let t = TenantView {
            id: "box".into(),
            hostname: "box.losos.cfd".into(),
            rathole_port: 50000,
            token: "plain".into(),
        };
        let o = EdgeOpts {
            rathole_bind_addr: "0.0.0.0".into(),
            rathole_bind_port: 2333,
            bootstrap_token: "a\"b\\c".into(),
        };
        let f = desired_config(&[t], &o);
        assert!(f.rathole_toml.contains("default_token = \"a\\\"b\\\\c\""));
    }
}