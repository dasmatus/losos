//! Pure config generation. `desired_config` takes the live tenant set and
//! returns the exact bytes to write to Traefik's dynamic file-provider dir and
//! the tunnel's service file (`crate::tunnel::ServerFile`). No IO, no time, no
//! randomness — fully deterministic and unit-testable.
//!
//! Both files are regenerated in full on every reconcile (the tenant set is
//! small), so the reconciler byte-compares old-vs-new and only writes when
//! something actually changed.
//!
//! The Traefik dynamic config is modeled as typed serde structs ([`TraefikConfig`]
//! and friends) so its shape — routers and services nested under `http`, each
//! router's `tls` block, the load-balancer server list — is enforced at
//! compile time. The earlier hand-rolled YAML could produce a structurally
//! invalid file (e.g. an empty `routers: {}`) that Traefik only rejected at
//! runtime ("routers cannot be a standalone element"); the type model makes
//! that unrepresentable.

use std::collections::BTreeMap;
use std::net::{Ipv4Addr, SocketAddr};

use serde::{Deserialize, Serialize};

use crate::tunnel::{ClientFile, ClientService, ServerFile, ServerService};

/// A live tenant, from the registry's point of view, reduced to what the
/// config needs. `last_seen` is deliberately absent here — it is a runtime
/// concern of `registry`, not of config generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TenantView {
    pub id: String,
    pub hostname: String,
    pub tunnel_port: u16,
    /// The per-appliance tunnel service token. Populated by the reconciler
    /// (not the registry) by reading the tenant's token file, so the on-disk
    /// token file stays the single source of truth and tokens auto-rotate.
    pub token: String,
    /// The edge port of the box's second tunnel service, `<id>:llm`, when the
    /// box lends memory to AI models (`crate::inference`). No tenant's own
    /// service can take that name: `modules/edge.nix` refuses a tenant id
    /// with a colon, a relayed box is `<spoke>.<label>` and an enrolled one a
    /// DNS label, and neither label may hold one. The service
    /// forwards to the box's llama.cpp RPC server; only the model head on this
    /// edge connects to it. Populated by the reconciler like `token`.
    pub llm_port: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Files {
    /// The Traefik dynamic config, or `None` when there are no live tenants.
    /// Traefik's file provider **rejects** an empty dynamic config — both
    /// `http: {}` and `http:\n  routers: {}\n  services: {}` error as
    /// "<key> cannot be a standalone element" — so the zero-tenant state is
    /// "no `losos.yml`", represented as `None`: the reconciler removes the
    /// file and Traefik drops every appliance router. The tunnel's file, by
    /// contrast, is always written: no services is a valid file.
    pub traefik_yaml: Option<String>,
    pub tunnel_json: String,
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

/// A per-appliance service: a load balancer with one server — the tunnel's
/// edge port for this appliance (`http://127.0.0.1:<tunnel_port>`), which
/// the tunnel (`crate::tunnel`) carries to the appliance's Nginx.
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

/// Render the Traefik dynamic file-provider config and the tunnel's service
/// file for the given live tenants.
///
/// Traefik: one router + one service per tenant, `Host(<hostname>)` →
/// `http://127.0.0.1:<tunnel_port>`, with per-router `tls.certResolver: le`
/// and `tls.domains: [{main: <hostname>}]`. Traefik obtains the Let's Encrypt
/// cert lazily on the first hit for that router — so the *registrar* is the
/// gatekeeper of which hostnames ever get certs: it only writes routers for
/// authenticated tenants.
///
/// Tunnel: one service per tenant, named by its id, on the loopback port
/// Traefik forwards to, with the per-appliance token. The appliance's tunnel
/// client presents the same token to take the service, and the tunnel
/// carries each connection on that port to the appliance's local Nginx.
#[must_use]
pub fn desired_config(tenants: &[TenantView]) -> Files {
    desired_config_with_hosts(tenants, &BTreeMap::new())
}

/// [`desired_config`], plus more hostnames for some tenants: the box's name
/// in the edge's own zone and the custom domains its owner proved
/// ([`crate::domains`]), keyed by tenant id.
///
/// Each extra hostname is a router of its own, `<id>-host-<n>` in sorted
/// order, pointing at the tenant's one service, with its own certificate.
/// One router per name rather than one `Host(a) || Host(b)` rule keeps a name
/// whose certificate cannot be issued from touching the others. Hosts for an
/// id that is not among `tenants` (not live) are ignored: no router may point
/// at a service that does not exist.
#[must_use]
pub fn desired_config_with_hosts(
    tenants: &[TenantView],
    extra_hosts: &BTreeMap<String, Vec<String>>,
) -> Files {
    let tunnel_json = tunnel_file(tenants);
    if tenants.is_empty() {
        // Traefik rejects an empty dynamic config (see `Files`), so the
        // zero-tenant Traefik state is `None` — no losos.yml — not an empty
        // file. The reconciler removes the file.
        return Files {
            traefik_yaml: None,
            tunnel_json,
        };
    }

    // BTreeMap gives deterministic ordering by id so byte-compare is stable
    // across runs regardless of input iteration order. serde_yaml quotes the
    // map keys only when the id needs it.
    let mut routers: BTreeMap<String, TraefikRouter> = BTreeMap::new();
    let mut services: BTreeMap<String, TraefikService> = BTreeMap::new();

    for t in tenants {
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
        if let Some(hosts) = extra_hosts.get(&t.id) {
            let mut hosts: Vec<&String> = hosts.iter().filter(|h| **h != host).collect();
            hosts.sort();
            hosts.dedup();
            for (n, extra) in hosts.into_iter().enumerate() {
                routers.insert(
                    format!("{id}-host-{n}"),
                    TraefikRouter {
                        rule: format!("Host(`{extra}`)"),
                        service: id.clone(),
                        entry_points: vec!["websecure".to_string()],
                        tls: TraefikTls {
                            cert_resolver: "le".to_string(),
                            domains: vec![TraefikDomain {
                                main: extra.clone(),
                            }],
                        },
                    },
                );
            }
        }
        services.insert(
            id.clone(),
            TraefikService {
                load_balancer: TraefikLoadBalancer {
                    servers: vec![TraefikServer {
                        url: format!("http://127.0.0.1:{port}", port = t.tunnel_port),
                    }],
                },
            },
        );
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
        tunnel_json,
    }
}

/// The tunnel's service file: `<id>` on the tenant's port, and `<id>:llm` on
/// its model port when it lends memory. JSON from a `BTreeMap`, so the same
/// tenants in any order give the same bytes.
fn tunnel_file(tenants: &[TenantView]) -> String {
    let loopback = |port| SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let mut file = ServerFile::default();
    for t in tenants {
        file.services.insert(
            t.id.clone(),
            ServerService {
                token: t.token.clone(),
                bind: loopback(t.tunnel_port),
            },
        );
        if let Some(port) = t.llm_port {
            file.services.insert(
                format!("{}:llm", t.id),
                ServerService {
                    token: t.token.clone(),
                    bind: loopback(port),
                },
            );
        }
    }
    let mut out = serde_json::to_string_pretty(&file).expect("a tunnel file serializes");
    out.push('\n');
    out
}

/// The Traefik file for the virtual machines the market published
/// (`crate::vms`): one router per machine order, `Host(<host>)`, with its own
/// certificate, to that order's Service in the mesh. `routes` is
/// `(hostname, service URL)`; the router is named after the hostname's first
/// label, the order's stem. `None` for no routes, for the same reason as
/// [`Files::traefik_yaml`]: Traefik refuses an empty file.
#[must_use]
pub fn vm_routes_yaml(routes: &[(String, String)]) -> Option<String> {
    if routes.is_empty() {
        return None;
    }
    let mut routers = BTreeMap::new();
    let mut services = BTreeMap::new();
    for (host, url) in routes {
        let name = host.split('.').next().unwrap_or(host).to_string();
        routers.insert(
            name.clone(),
            TraefikRouter {
                rule: format!("Host(`{host}`)"),
                service: name.clone(),
                entry_points: vec!["websecure".to_string()],
                tls: TraefikTls {
                    cert_resolver: "le".to_string(),
                    domains: vec![TraefikDomain { main: host.clone() }],
                },
            },
        );
        services.insert(
            name,
            TraefikService {
                load_balancer: TraefikLoadBalancer {
                    servers: vec![TraefikServer { url: url.clone() }],
                },
            },
        );
    }
    Some(
        serde_yaml::to_string(&TraefikConfig {
            http: TraefikHttp { routers, services },
        })
        .expect("serializing TraefikConfig is infallible for String/Vec/BTreeMap fields"),
    )
}

/// The hub a spoke relays through, as the uplink renderer needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UplinkTarget {
    /// The hub's tunnel, `host:port` (UDP).
    pub tunnel_endpoint: String,
    /// SHA-256 of the hub's tunnel certificate, pinned.
    pub pin: String,
    /// This spoke's own tenant token on the hub: every relayed service
    /// authenticates with it, because the spoke is the authenticated party.
    pub token: String,
}

/// One relayed box as the uplink carries it: the service name the hub
/// expects (`<spoke>.<box>`) and the spoke-local tunnel port that reaches
/// the box's own tunnel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UplinkService {
    pub name: String,
    pub local_port: u16,
}

/// Render the tunnel *client* file of a spoke's uplink: one service per
/// relayed box, `<spoke>.<box>`, delivered to the spoke-local port that box's
/// own tunnel serves, with the spoke's token. Deterministic (a `BTreeMap`),
/// so the reconciler's byte-compare is stable, and always a complete file.
#[must_use]
pub fn uplink_config(target: &UplinkTarget, services: &[UplinkService]) -> String {
    let file = ClientFile {
        remote: target.tunnel_endpoint.clone(),
        pin: target.pin.clone(),
        services: services
            .iter()
            .map(|s| {
                (
                    s.name.clone(),
                    ClientService {
                        token: target.token.clone(),
                        local: format!("127.0.0.1:{}", s.local_port),
                    },
                )
            })
            .collect(),
    };
    let mut out = serde_json::to_string_pretty(&file).expect("a tunnel file serializes");
    out.push('\n');
    out
}
