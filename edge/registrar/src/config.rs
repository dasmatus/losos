//! Pure config generation. `desired_config` takes the live tenant set and
//! edge options and returns the routes Traefik discovers and the exact bytes
//! of rathole's server config. No IO, no time, no randomness — fully
//! deterministic and unit-testable.
//!
//! Traefik finds the boxes through its HTTP provider: it polls the
//! registrar's loopback provider listener (`server::provider_router`) for
//! [`provider_document`], the JSON form of the [`Routes`] the last reconcile
//! pass decided on. Nothing is written to Traefik's directory any more; the
//! file provider there carries only the NixOS-owned `register*.yml`.
//! rathole's file is regenerated in full on every reconcile (the tenant set
//! is small), so the reconciler byte-compares old-vs-new and only writes when
//! something actually changed.
//!
//! The Traefik dynamic config is modeled as typed serde structs ([`Routes`]
//! and friends) so its shape — routers and services nested under `http`, each
//! router's `tls` block, the load-balancer server list — is enforced at
//! compile time. Traefik decodes an empty section such as `routers: {}` as
//! an error ("routers cannot be a standalone element") and then keeps the
//! routes it had, so every map is skipped when empty and no routes at all is
//! the document `{}`, which Traefik takes as "drop every box".

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
    /// The edge port of the box's second tunnel service, `<id>:llm`, when the
    /// box lends memory to AI models (`crate::inference`). No tenant's own
    /// service can take that name: `modules/edge.nix` refuses a tenant id
    /// with a colon, a relayed box is `<spoke>.<label>` and an enrolled one a
    /// DNS label, and neither label may hold one. The service
    /// forwards to the box's llama.cpp RPC server; only the model head on this
    /// edge connects to it. Populated by the reconciler like `token`.
    pub llm_port: Option<u16>,
}

/// Edge-wide options baked into the generated config.
#[derive(Debug, Clone)]
pub struct EdgeOpts {
    /// rathole `[server] bind_addr` — the address appliance clients dial.
    pub rathole_bind_addr: String,
    pub rathole_bind_port: u16,
    /// rathole `default_token` (also the bootstrap shared secret).
    pub bootstrap_token: String,
    /// The tunnel's Noise (`Noise_NK_25519_ChaChaPoly_BLAKE2s`) private key,
    /// base64, read from a 0600 file at runtime. `Some` turns rathole's Noise
    /// transport on for the server; `None` leaves the tunnel plain TCP.
    pub noise_private_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Files {
    /// The boxes' routes for Traefik's HTTP provider. Empty with no live
    /// tenants, which [`provider_document`] renders as `{}`.
    pub traefik: Routes,
    pub rathole_toml: String,
}

// ──────────────────────────────────────────────────────────────────────────
// Traefik dynamic config — typed model.
//
// These mirror the subset of Traefik's dynamic config the registrar serves
// (https://doc.traefik.io/traefik/reference/dynamic-configuration/file/; the
// HTTP provider decodes the same document). `#[serde(rename_all =
// "camelCase")]` maps the snake_case Rust fields to the camelCase keys
// Traefik expects (entryPoints, certResolver, loadBalancer). `BTreeMap` gives
// deterministic key order, so the document only changes when a route does
// (Traefik hashes each response and ignores one it has seen).
//
// Only the parts the registrar actually serves are modeled — adding a field
// Traefik doesn't understand is a compile error against these structs, not a
// runtime "unknown field" from Traefik.

/// Root of the provider document: `{"http": {...}}`, or `{}` with no routes.
#[derive(Debug, Serialize, Deserialize)]
struct TraefikConfig {
    #[serde(default, skip_serializing_if = "Routes::is_empty")]
    http: Routes,
}

/// The `http:` section of a dynamic config: routers, the middlewares they
/// use and the services they forward to. One value per source (the boxes,
/// the machines), merged by [`provider_document`]; names never collide across
/// sources because each source names its own entries.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Routes {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    routers: BTreeMap<String, TraefikRouter>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    middlewares: BTreeMap<String, TraefikMiddleware>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    services: BTreeMap<String, TraefikService>,
}

impl Routes {
    /// No router, middleware or service.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.routers.is_empty() && self.middlewares.is_empty() && self.services.is_empty()
    }

    /// The hostnames routed, sorted: what the boxes are reachable at.
    #[must_use]
    pub fn hosts(&self) -> Vec<String> {
        let mut hosts: Vec<String> = self
            .routers
            .values()
            .flat_map(|r| {
                r.tls
                    .iter()
                    .flat_map(|t| t.domains.iter().map(|d| d.main.clone()))
            })
            .collect();
        hosts.sort();
        hosts.dedup();
        hosts
    }

    /// This source alone as a provider document, or `None` when it routes
    /// nothing.
    #[must_use]
    pub fn document(&self) -> Option<String> {
        (!self.is_empty()).then(|| provider_document([self]))
    }

    fn extend(&mut self, other: &Routes) {
        self.routers
            .extend(other.routers.iter().map(|(k, v)| (k.clone(), v.clone())));
        self.middlewares.extend(
            other
                .middlewares
                .iter()
                .map(|(k, v)| (k.clone(), v.clone())),
        );
        self.services
            .extend(other.services.iter().map(|(k, v)| (k.clone(), v.clone())));
    }

    /// Put [`OFFLINE_MIDDLEWARE`] in front of every router: a box Traefik
    /// cannot reach (switched off, rebooting, its tunnel down) answers with
    /// the edge's own page instead of a bare 502. `url` is the provider
    /// listener, which serves that page at `/offline/<status>`.
    #[must_use]
    pub fn with_offline_page(&self, url: &str) -> Routes {
        if self.routers.is_empty() {
            return self.clone();
        }
        let mut out = self.clone();
        for r in out.routers.values_mut() {
            r.middlewares.push(OFFLINE_MIDDLEWARE.to_string());
        }
        out.middlewares.insert(
            OFFLINE_MIDDLEWARE.to_string(),
            TraefikMiddleware {
                errors: TraefikErrors {
                    status: OFFLINE_STATUSES.iter().map(|s| (*s).to_string()).collect(),
                    service: OFFLINE_MIDDLEWARE.to_string(),
                    query: "/offline/{status}".to_string(),
                },
            },
        );
        out.services.insert(
            OFFLINE_MIDDLEWARE.to_string(),
            TraefikService {
                load_balancer: TraefikLoadBalancer {
                    servers: vec![TraefikServer {
                        url: url.to_string(),
                    }],
                },
            },
        );
        out
    }
}

/// The middleware (and the service behind it) that answers for an
/// unreachable box. Tenant services are named after the tenant id, and no id
/// can be this one: an enrolled or relayed id is made of DNS labels, which
/// hold no underscore, and `modules/edge.nix` refuses it as a tenant id.
pub const OFFLINE_MIDDLEWARE: &str = "losos_offline";

/// What Traefik answers when it cannot reach a box: 502 when the tunnel
/// refuses or drops the connection, 504 when the box does not answer in
/// time. 503 is left alone, because a box sends it on purpose while an app
/// is starting.
pub const OFFLINE_STATUSES: [&str; 2] = ["502", "504"];

/// The provider document Traefik's HTTP provider reads: every source merged.
/// `{}` when nothing is routed, which Traefik takes as "no routes" (an empty
/// `http` section would be refused, and the old routes kept).
#[must_use]
pub fn provider_document<'a>(parts: impl IntoIterator<Item = &'a Routes>) -> String {
    let mut http = Routes::default();
    for part in parts {
        http.extend(part);
    }
    serde_json::to_string(&TraefikConfig { http })
        .expect("serializing TraefikConfig is infallible for String/Vec/BTreeMap fields")
}

/// A per-appliance router: match `Host(<hostname>)`, forward to the service of
/// the same id, terminate TLS with the on-demand Let's Encrypt resolver.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TraefikRouter {
    rule: String,
    service: String,
    entry_points: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    middlewares: Vec<String>,
    /// `None` for a router on a plain entry point (a box's Tor and I2P
    /// names on `losos-hidden`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tls: Option<TraefikTls>,
}

/// Per-router TLS: the `le` cert resolver and the hostname to obtain a cert
/// for. The registrar is the gatekeeper of which hostnames ever get certs —
/// Traefik obtains lazily on first hit for a router that exists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TraefikTls {
    /// `None` is Traefik's own default certificate: no public authority
    /// issues for a Tor or I2P name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cert_resolver: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    domains: Vec<TraefikDomain>,
}

/// A `domains:` entry — `main` is the cert's primary SAN.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct TraefikDomain {
    main: String,
}

/// The one middleware kind the registrar uses: `errors`, which hands a
/// response with one of `status` to `service` at `query` instead.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct TraefikMiddleware {
    errors: TraefikErrors,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct TraefikErrors {
    status: Vec<String>,
    service: String,
    query: String,
}

/// A per-appliance service: a load balancer with one server — the rathole
/// edge port for this appliance (`http://127.0.0.1:<rathole_port>`), which
/// rathole forwards over the tunnel to the appliance's Nginx.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TraefikService {
    load_balancer: TraefikLoadBalancer,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TraefikLoadBalancer {
    servers: Vec<TraefikServer>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct TraefikServer {
    url: String,
}

/// Render the Traefik routes and the rathole server config for the given
/// live tenants.
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
    desired_config_with_hosts(tenants, opts, &BTreeMap::new())
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
    opts: &EdgeOpts,
    extra_hosts: &BTreeMap<String, Vec<String>>,
) -> Files {
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
    if let Some(key) = &opts.noise_private_key {
        toml.push_str(&format!(
            "\n[server.transport]\ntype = \"noise\"\n\n[server.transport.noise]\nlocal_private_key = \"{}\"\n",
            toml_escape(key)
        ));
    }

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
        // Traefik gets no routes, which the provider serves as `{}`.
        toml.push_str("\n[server.services]\n");
        return Files {
            traefik: Routes::default(),
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
    // across runs regardless of input iteration order. serde_json escapes
    // the map keys.
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
                middlewares: Vec::new(),
                tls: Some(TraefikTls {
                    cert_resolver: Some("le".to_string()),
                    domains: vec![TraefikDomain { main: host.clone() }],
                }),
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
                        middlewares: Vec::new(),
                        tls: Some(TraefikTls {
                            cert_resolver: Some("le".to_string()),
                            domains: vec![TraefikDomain {
                                main: extra.clone(),
                            }],
                        }),
                    },
                );
            }
        }
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
        if let Some(port) = t.llm_port {
            toml.push_str(&format!(
                "\n[server.services.{id}]\ntoken = \"{tok}\"\nbind_addr = \"127.0.0.1:{port}\"\n",
                id = toml_key(&format!("{}:llm", t.id)),
                tok = toml_escape(&t.token),
            ));
        }
    }

    Files {
        traefik: Routes {
            routers,
            middlewares: BTreeMap::new(),
            services,
        },
        rathole_toml: toml,
    }
}

/// The routes of the virtual machines the market published (`crate::vms`):
/// one router per machine order, `Host(<host>)`, with its own certificate, to
/// that order's Service in the mesh. `routes` is `(hostname, service URL)`;
/// the router is named after the hostname's first label, the order's stem.
#[must_use]
pub fn vm_routes(routes: &[(String, String)]) -> Routes {
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
                middlewares: Vec::new(),
                tls: Some(TraefikTls {
                    cert_resolver: Some("le".to_string()),
                    domains: vec![TraefikDomain { main: host.clone() }],
                }),
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
    Routes {
        routers,
        middlewares: BTreeMap::new(),
        services,
    }
}

/// One box behind this edge with Tor or I2P addresses
/// (`crate::hidden`): its id, the edge port of its tunnel, and the names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HiddenRoute {
    pub id: String,
    pub rathole_port: u16,
    pub hosts: Vec<String>,
}

/// Traefik's routes for the boxes' Tor and I2P addresses, a source of the
/// provider document of its own: per box, one router on the plain
/// `losos-hidden` entry point and one on `losos-hidden-tls` with Traefik's
/// default certificate, both `Host(<onion>) || Host(<b32>)`, to a service of
/// their own at the box's tunnel port. Those entry points listen on
/// loopback, where only Tor and I2P connect, and no other router listens
/// there. No offline page: a visitor on Tor gets Traefik's own answer.
#[must_use]
pub fn hidden_routes(routes: &[HiddenRoute]) -> Routes {
    let mut out = Routes::default();
    for r in routes {
        if r.hosts.is_empty() {
            continue;
        }
        let mut hosts = r.hosts.clone();
        hosts.sort();
        hosts.dedup();
        let rule = hosts
            .iter()
            .map(|h| format!("Host(`{h}`)"))
            .collect::<Vec<_>>()
            .join(" || ");
        let service = format!("{}-hidden", r.id);
        out.routers.insert(
            service.clone(),
            TraefikRouter {
                rule: rule.clone(),
                service: service.clone(),
                entry_points: vec![crate::hidden::PLAIN_ENTRY_POINT.to_string()],
                middlewares: Vec::new(),
                tls: None,
            },
        );
        out.routers.insert(
            format!("{}-hidden-tls", r.id),
            TraefikRouter {
                rule,
                service: service.clone(),
                entry_points: vec![crate::hidden::TLS_ENTRY_POINT.to_string()],
                middlewares: Vec::new(),
                tls: Some(TraefikTls {
                    cert_resolver: None,
                    domains: Vec::new(),
                }),
            },
        );
        out.services.insert(
            service,
            TraefikService {
                load_balancer: TraefikLoadBalancer {
                    servers: vec![TraefikServer {
                        url: format!("http://127.0.0.1:{}", r.rathole_port),
                    }],
                },
            },
        );
    }
    out
}

/// The hub a spoke relays through, as the uplink renderer needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UplinkTarget {
    /// rathole `[client] remote_addr` — the hub's tunnel port.
    pub rathole_endpoint: String,
    /// The hub's rathole `default_token` (its bootstrap token).
    pub bootstrap_token: String,
    /// This spoke's own tenant token on the hub: every relayed service
    /// authenticates with it, because the spoke is the authenticated party.
    pub token: String,
    /// The hub's Noise public key, pinned; `None` leaves the uplink plain TCP.
    pub noise_public_key: Option<String>,
}

/// One relayed box as the uplink carries it: the service name the hub
/// expects (`<spoke>.<box>`) and the spoke-local rathole port that reaches
/// the box's own tunnel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UplinkService {
    pub name: String,
    pub local_port: u16,
}

/// Render the rathole *client* config of a spoke's uplink: one
/// `[client.services.<spoke>.<box>]` per relayed box, `local_addr` the
/// spoke-local port the hub's traffic is to be delivered to, token the
/// spoke's own. Deterministic (services sorted by name), so the reconciler's
/// byte-compare is stable, and always a complete file: with no services
/// rathole keeps the connection and idles, which is what a site with no box
/// registered yet wants.
#[must_use]
pub fn uplink_config(target: &UplinkTarget, services: &[UplinkService]) -> String {
    let mut toml = format!(
        "[client]\nremote_addr = \"{addr}\"\ndefault_token = \"{tok}\"\n",
        addr = toml_escape(&target.rathole_endpoint),
        tok = toml_escape(&target.bootstrap_token),
    );
    if let Some(key) = &target.noise_public_key {
        toml.push_str(&format!(
            "\n[client.transport]\ntype = \"noise\"\n\n[client.transport.noise]\nremote_public_key = \"{}\"\n",
            toml_escape(key)
        ));
    }
    let mut sorted: Vec<&UplinkService> = services.iter().collect();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));
    if sorted.is_empty() {
        toml.push_str("\n[client.services]\n");
    }
    for s in sorted {
        toml.push_str(&format!(
            "\n[client.services.{name}]\ntoken = \"{tok}\"\nlocal_addr = \"127.0.0.1:{port}\"\n",
            name = toml_key(&s.name),
            tok = toml_escape(&target.token),
            port = s.local_port,
        ));
    }
    toml
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

/// Escape a string for a TOML **basic string** value (the `"..."` form).
///
/// TOML 1.0 forbids a raw control character inside a basic string: besides
/// `\` and `"`, the range U+0000-U+0008, U+000A-U+001F and U+007F must be
/// escaped — `\b`, `\t`, `\n`, `\f`, `\r` where a short form exists,
/// `\uXXXX` otherwise.
///
/// This is not hypothetical tidiness. The registrar is the *sole* writer of
/// `server.toml`; a single tenant id or token file that ends up carrying a
/// newline would emit a file rathole cannot parse, and every tunnel — not
/// just that tenant's — drops at the next reconcile. The reconciler also
/// refuses tokens containing control characters (see `server::token_fault`),
/// so this is the second line of defence, not the first.
fn toml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\u{c}' => out.push_str("\\f"),
            '\r' => out.push_str("\\r"),
            c if c.is_control() => {
                out.push_str("\\u");
                let n = c as u32;
                // Every `is_control()` char is below U+0100, so four hex
                // digits always suffice for the \uXXXX form.
                for shift in [12u32, 8, 4, 0] {
                    out.push(char::from_digit((n >> shift) & 0xF, 16).unwrap_or('0'));
                }
            }
            c => out.push(c),
        }
    }
    out
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
