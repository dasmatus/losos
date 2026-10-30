//! Hand-rolled argument parsing. Kept dependency-free (no clap) because the
//! surface is small and fixed; every flag has a NixOS-module default so the
//! binary is only ever invoked with the values the module generates.

use std::time::Duration;

use miette::{miette, IntoDiagnostic, Result};

use crate::domains::{DomainsOpts, DEFAULT_DOH_URL};
use crate::market::{
    MarketOpts, DEFAULT_FEE_BPS, MAX_COIN_FEE_LIMIT, MAX_FEE_BPS, SUPPORTED_CURRENCIES,
};
use crate::provision::{ProvisionGithub, GITHUB_API_URL, GITHUB_OAUTH_URL};
use crate::stripe_gate::GateOpts;

fn arg<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1).map(std::string::String::as_str))
}

/// Every value of a flag that may be given more than once.
fn args_all<'a>(args: &'a [String], flag: &str) -> Vec<&'a str> {
    args.windows(2)
        .filter(|w| w[0] == flag)
        .map(|w| w[1].as_str())
        .collect()
}

/// `lab`: LosOS Lab's guests under libvirt (`crate::lab`).
fn parse_lab(rest: &[String]) -> Result<crate::lab::LabOpts> {
    use crate::lab::{LabOpts, VirtType};
    let d = LabOpts::defaults();
    let mut origins = d.origins.clone();
    for o in args_all(rest, "--origin") {
        let o = o.trim_end_matches('/');
        let ok = crate::lab::authority(o).is_some() && o.bytes().all(|b| b.is_ascii_graphic());
        if !ok {
            return Err(miette!(
                "bad --origin {o:?}; expected scheme://host[:port], like https://lab.example.org"
            ));
        }
        origins.push(o.to_string());
    }
    let connect = arg(rest, "--connect").unwrap_or(&d.connect);
    if !connect.starts_with("qemu") {
        return Err(miette!("bad --connect {connect:?}; expected a qemu URI such as qemu:///session or qemu:///system"));
    }
    Ok(LabOpts {
        listen: arg(rest, "--listen").unwrap_or(&d.listen).to_string(),
        connect: connect.to_string(),
        images: arg(rest, "--images").map_or(d.images.clone(), std::path::PathBuf::from),
        origins,
        token_file: arg(rest, "--token-file").map(str::to_string),
        virsh: arg(rest, "--virsh").unwrap_or(&d.virsh).to_string(),
        max_guests: match arg(rest, "--max-guests") {
            None => d.max_guests,
            Some(raw) => raw
                .parse()
                .ok()
                .filter(|n| (1..=64).contains(n))
                .ok_or_else(|| miette!("bad --max-guests {raw:?}; expected 1..=64"))?,
        },
        memory_mib: match arg(rest, "--memory") {
            None => d.memory_mib,
            Some(raw) => raw
                .parse()
                .ok()
                .filter(|n| (32..=4096).contains(n))
                .ok_or_else(|| miette!("bad --memory {raw:?}; expected 32..=4096 (MiB)"))?,
        },
        virt_type: match arg(rest, "--virt-type").unwrap_or("auto") {
            "auto" => VirtType::Auto,
            "kvm" => VirtType::Kvm,
            "qemu" => VirtType::Qemu,
            other => {
                return Err(miette!(
                    "bad --virt-type {other:?}; expected auto, kvm or qemu"
                ))
            }
        },
        idle: match arg(rest, "--idle") {
            None => d.idle,
            Some(raw) => parse_dur(raw)?,
        },
    })
}

fn req<'a>(args: &'a [String], flag: &str) -> Result<&'a str> {
    arg(args, flag).ok_or_else(|| miette!("missing required flag {flag}"))
}

/// Parse a `<n><unit>` duration (`ms`/`s`/`m`/`h`, bare number = seconds).
///
/// Zero is rejected: every duration this crate parses is a loop period or a
/// TTL, and `0` turns the reconciler into a busy-loop, the heartbeat into a
/// flood, or the TTL into "prune every tenant on the next tick". The
/// multiplication is checked — `parse_dur("5124095576030432h")` used to
/// overflow `u64` seconds (a debug-build panic, a silently tiny interval in
/// release) instead of being rejected as the nonsense it is.
fn parse_dur(s: &str) -> Result<Duration> {
    let s = s.trim();
    let (n, secs_per) = if let Some(stripped) = s.strip_suffix("ms") {
        (stripped, None)
    } else if let Some(stripped) = s.strip_suffix('s') {
        (stripped, Some(1))
    } else if let Some(stripped) = s.strip_suffix('m') {
        (stripped, Some(60))
    } else if let Some(stripped) = s.strip_suffix('h') {
        (stripped, Some(3600))
    } else {
        (s, Some(1))
    };
    let n: u64 = n
        .trim()
        .parse()
        .map_err(|_| miette!("bad duration {s:?}; expected <n>[ms|s|m|h]"))?;
    if n == 0 {
        return Err(miette!("duration {s:?} must be greater than zero"));
    }
    match secs_per {
        None => Ok(Duration::from_millis(n)),
        Some(mul) => n
            .checked_mul(mul)
            .map(Duration::from_secs)
            .ok_or_else(|| miette!("duration {s:?} overflows")),
    }
}

/// `serve --relay-routes`: where the official edge keeps the route table
/// of boxes behind local edges, and the key it signs relay passes with.
/// Both default to files beside the registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutesOpts {
    /// `relay-routes.json` (`crate::routes::TableFile`).
    pub table_file: String,
    /// 64 hex characters, made on first start when absent.
    pub pass_key_file: String,
}

/// `serve` options. Traefik discovers the boxes by polling
/// `traefik_provider_listen`; the registrar is the sole writer of
/// `tunnel_config`, which the tunnel re-reads every second (no signal
/// needed), so there is no tunnel handle here.
///
/// The `mesh_*` / `kube_*` fields are the `/cluster/join` half and are all
/// optional. `modules/edge.nix` only appends their flags under
/// `lib.optionals cfg.cluster.enable`, so an edge with
/// `losos.edge.cluster.enable = false` parses exactly the arguments it always
/// did and the join route answers 503 — the master-proxy half is untouched by
/// the mesh existing.
#[derive(Debug, Clone)]
pub struct ServeOpts {
    pub listen: String,
    pub registry_path: String,
    /// Where Traefik's HTTP provider reads the boxes' routes, and the page an
    /// unreachable box answers with (`server::provider_router`). Loopback:
    /// nothing but Traefik on the same machine is meant to reach it.
    pub traefik_provider_listen: String,
    /// The tunnel's service file (`crate::tunnel::ServerFile`).
    pub tunnel_config: String,
    pub port_range: (u16, u16),
    /// The tunnel's `pin` file (`crate::tunnel::load_or_make_identity`),
    /// served verbatim at `GET /tunnel-pin` so appliances can pin the
    /// tunnel's certificate on first start. Public by nature; `None` = 404.
    pub tunnel_pin_file: Option<String>,
    pub tenants_file: String,
    pub reconcile_interval: Duration,
    pub heartbeat_ttl: Duration,
    /// The rke2 *agent* token an accepted join is handed back, read at request
    /// time rather than at boot so a rotated file needs no restart. `None`
    /// disables the join route.
    pub mesh_agent_token_file: Option<String>,
    /// `https://<advertiseAddr>:<supervisorPort>` — the rke2 **supervisor**
    /// port (9345), not the apiserver port (6443). An agent registers on the
    /// supervisor; pointing it at 6443 fails at join time with a TLS error
    /// that names neither port.
    pub mesh_server_addr: Option<String>,
    /// The mesh apiserver the stale-node cleanup talks to. Loopback by
    /// default: the registrar runs on the same host as `rke2-server`.
    pub kube_api: String,
    /// Bearer token for `kube_api`, extracted from the `losos-registrar`
    /// ServiceAccount by `losos-mesh-rbac.service`.
    pub kube_token_file: Option<String>,
    /// The mesh cluster CA. The cleanup client pins its root store to this
    /// file and disables the built-in webpki roots, so a public CA cannot
    /// impersonate the cluster's apiserver.
    pub kube_ca_file: Option<String>,
    /// Where the reconciler publishes the per-node compute windows for the
    /// edge's `losos-mesh-taint.service` to read.
    pub compute_windows_file: String,
    /// The market. `None` (neither `--market-gate-socket` nor
    /// `--market-no-cards`) and every `/market/*` route answers 503; the rest
    /// of the API is unchanged.
    pub market: Option<Box<MarketOpts>>,
    /// Buying USDC with euros at Binance (`crate::onramp`). `None` unless
    /// `--onramp-binance-api` was given, and every `/market/onramp*` route
    /// answers 503.
    pub onramp: Option<crate::onramp::OnrampOpts>,
    /// AI models run across lending boxes (`crate::inference`), from
    /// `--inference-models`. Needs the market: requests are paid through
    /// its taps.
    pub inference: Option<crate::inference::InferenceOpts>,
    /// An open gateway's passport and TPM (`crate::gateway`). `None` unless
    /// `--gateway-key-file` was given, and every `/passport*` route answers
    /// 404: an edge boxes share through but never trust with the market.
    pub gateway: Option<crate::gateway::GatewayOpts>,
    /// Where the operator routes (`/operator/tokens*`) ask who a GitHub
    /// token belongs to.
    /// `--github-api-url`; the default is api.github.com, the override is
    /// for the tests' fake GitHub.
    pub github_api_url: String,
    /// `None` unless `--dns-zone` was given: no zone, no custom domains, and
    /// every `/domains/*` route answers 503.
    pub domains: Option<Box<DomainsOpts>>,
    /// Open enrolment (`crate::relay::Enrolment`): the directory this edge
    /// keeps boxes it accepted on first contact in. `None` keeps enrolment
    /// closed, which every internet-facing edge must.
    pub enrol_dir: Option<String>,
    /// The uplink to a hub (`crate::relay`). `None` and this edge relays
    /// nothing anywhere.
    pub uplink: Option<crate::relay::UplinkOpts>,
    /// The secondary cluster this gateway may belong to
    /// (`crate::secondary`), from `--secondary-file`. `None` and the edge
    /// never syncs with a peer.
    pub secondary: Option<crate::secondary::SecondaryOpts>,
    /// The official cluster this edge belongs to (`crate::official`), from
    /// `--official-cluster`: the load balancer it sits behind.
    pub official_cluster: Option<String>,
    /// The official edges it lists for boxes and gateways to spread over and
    /// move to, from `--official-peer CLUSTER,URL,TUNNEL` (repeatable).
    pub official_peers: Vec<crate::official::OfficialPeer>,
    /// The ban list (`crate::official::Bans`), from `--bans-file`. A
    /// gateway's uplink leaves a banned hub for an official edge outside
    /// every ban.
    pub bans_file: Option<String>,
    /// The route table for boxes behind a local edge (`crate::routes`).
    /// `None` unless `--relay-routes` was given, which needs `--dns-zone`:
    /// only an edge that routes domains keeps the table.
    pub routes: Option<RoutesOpts>,
    /// The terminal (`crate::operator`). `None` unless `--access-dir` was
    /// given, and every `/operator/*` route answers 404.
    pub operator: Option<crate::operator::OperatorOpts>,
    /// The page a company's admins look after their boxes from
    /// (`server::manage`). `None` unless `--manage-key-file` was given, and
    /// every `/manage*` route answers 404.
    pub manage: Option<ManageOpts>,
    /// Call media for this edge's boxes (`crate::calls_relay`). `None`
    /// unless `--calls-relay-listen` was given, and `GET /calls/relay`
    /// answers 404.
    pub calls_relay: Option<crate::calls_relay::CallsRelayOpts>,
    /// Mesh CI's queue (`crate::ci`): the directory it keeps the archives
    /// of waiting jobs in. `None` unless `--ci-dir` was given, and every
    /// `/ci/*` route answers 503.
    pub ci_dir: Option<String>,
    /// Tor and I2P addresses for the boxes (`crate::hidden`). `None` unless
    /// `--hidden-dir` was given.
    pub hidden: Option<crate::hidden::HiddenOpts>,
    /// The edge as a way out (`crate::exit`). `None` unless `--exit-dir`
    /// was given.
    pub exit: Option<crate::exit::ExitOpts>,
    /// The boxes' Tang keys (`crate::tang`). `None` unless `--tang-dir` was
    /// given.
    pub tang: Option<crate::tang::TangOpts>,
    /// The ECH key (`crate::ech`): `--ech-key-file FILE --ech-public-name
    /// NAME`. `None` publishes no HTTPS records with an `ech` value.
    pub ech: Option<EchOpts>,
}

/// `serve --ech-key-file FILE --ech-public-name NAME`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EchOpts {
    /// RFC 9934 PEM, made on the first start, read by the TLS front.
    pub key_file: String,
    /// The name the outer handshake shows: the edge's own domain.
    pub public_name: String,
}

/// `serve --manage-key-file FILE [--manage-ui DIR] [--manage-traefik-metrics URL]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManageOpts {
    /// The key the page signs in with: 64 hex characters, 0600, made on the
    /// first start if missing and shown on the gateway's console.
    pub key_file: String,
    /// The built admin UI, whose `manage/index.html` is the page. `None`
    /// serves only the JSON routes.
    pub ui_dir: Option<String>,
    /// Traefik's Prometheus endpoint (`losos.edge.traefikMetricsPort`), read
    /// every minute for the Statistics page (`crate::stats`). `None` leaves
    /// the page empty.
    pub traefik_metrics: Option<String>,
}

/// `passport` options: what a gateway's owner runs on their own machine
/// (`crate::gateway::cli`), and the box's check run by hand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PassportOpts {
    /// `passport statement --url U --edge-key HEX --payout NET=ADDR ...
    /// [--days D]`: the text the owner signs with `gpg --detach-sign
    /// --armor`. The edge key is `GET <U>/passport/edge-key`.
    Statement {
        url: String,
        edge_key: String,
        payouts: Vec<(String, String)>,
        days: i64,
    },
    /// `passport settings --statement F --signature F --key F
    /// --losos-payouts F`: the settings file, checked as a box checks it.
    Settings {
        statement: String,
        signature: String,
        key: String,
        losos_payouts: String,
    },
    /// `passport measure --uki FILE --name N [--app FILE ...]`: the image
    /// entry boxes ship: PCR 11 of the unified kernel image and the
    /// Authenticode hashes of it and of each `--app` (the kernel inside it).
    Measure {
        uki: String,
        name: String,
        apps: Vec<String>,
    },
    /// `passport image --name N [--event-log F]`: the image entry for the
    /// machine it runs on, as it booted: PCR 11 read from its TPM and the
    /// programs its firmware's event log says it started. For a box that
    /// pins a gateway it has looked at, and for the VM tests.
    Image { name: String, event_log: String },
    /// `passport check --url U --tpm-roots F --images F --payouts F`: the
    /// box's whole check against a running gateway. Exit 0 when it would be
    /// trusted, 1 with the reason when not, 2 when the reason is tampering.
    Check {
        url: String,
        tpm_roots: String,
        images: String,
        payouts: String,
    },
}

/// `announce` options. The appliance reads its token from `token_file`
/// (0600, provisioned out of band, persisted via /var).
#[derive(Debug, Clone)]
pub struct AnnounceOpts {
    pub registrar_url: String,
    pub appliance_id: String,
    pub hostname: String,
    pub token_file: String,
    pub heartbeat_interval: Duration,
    /// One-minute load average per core, below which this box calls itself
    /// idle and lets the mesh schedule onto it inside its compute window.
    ///
    /// See crate::idle for why the default is 0.25 rather than something near
    /// zero: an idle losos box runs two kubelets, containerd, Postgres, Redis
    /// and a PHP-FPM pool, so a threshold near zero would mean "never idle"
    /// and the feature would quietly never fire.
    pub idle_load_threshold: f64,
    /// `--relay-pass-file`: where lososd keeps the relay pass its official
    /// edge issued (`crate::routes`), re-read on every register and
    /// heartbeat and sent along so a local edge can forward it. Missing or
    /// empty sends none.
    pub relay_pass_file: Option<String>,
    /// `--managed-file`: lososd's marker that the owner lets this edge
    /// manage the box (present means yes), re-read on every heartbeat.
    pub managed_file: Option<String>,
    /// `--llm-lend-file`: the MiB this box lends to AI models, as decimal
    /// text, present only while its llama.cpp RPC server runs
    /// (`losos.inference.lend`). Re-read on every heartbeat; missing,
    /// unreadable or not a number lends nothing.
    pub llm_lend_file: Option<String>,
    /// `--hidden-file`: where to keep the Tor and I2P addresses the edge
    /// made for this box (`crate::hidden`), from each heartbeat's answer.
    /// Removed when the edge makes none.
    pub hidden_file: Option<String>,
    /// `--tls-ready-file`: the public names this box holds its own
    /// certificate for, one per line (`crate::passthrough`), re-read on every
    /// heartbeat. Present, even empty, asks the edge to pass TLS through;
    /// missing leaves TLS to the edge.
    pub tls_ready_file: Option<String>,
    /// `--tls-names-file`: where to keep the public names the edge routes to
    /// this box, from each heartbeat's answer, for the certificate unit.
    pub tls_names_file: Option<String>,
}

/// `join` options. Runs once per boot on the appliance, from
/// `losos-mesh-join.service`, before `rke2-agent.service`.
///
/// It authenticates with the *same* `/var/secrets/losos-proxy-token` the
/// announce client uses — mesh enrolment mints no second credential — and
/// writes the rke2 node token the edge hands back to `out_token_file`, which
/// is `losos.cluster.tokenFile`.
#[derive(Debug, Clone)]
pub struct JoinOpts {
    pub registrar_url: String,
    pub appliance_id: String,
    pub node_name: String,
    pub token_file: String,
    pub out_token_file: String,
    pub share_compute: bool,
    pub window_start: String,
    pub window_end: String,
    /// The zone the two bounds are wall-clock times in, taken from the
    /// appliance's own `time.timeZone` by modules/cluster.nix. The edge writes
    /// the taint and so compares on its own clock; without this the hours are
    /// reinterpreted in the edge's zone.
    pub window_tz: String,
    /// `--host-vms`: the box hosts other people's virtual machines
    /// (`crate::vms`). modules/cluster.nix passes true only while the box's
    /// shared storage is on.
    pub host_vms: bool,
    /// `losos.cluster.serverAddr` as this box has it configured. Purely a
    /// consistency check: a mismatch against the edge's answer is logged, not
    /// enforced, because the edge is the authority on its own address.
    pub expect_server_addr: Option<String>,
}

/// `provision` options: the key ceremony as the operator runs it, from
/// their own machine, behind a GitHub sign-in (`crate::provision`). Every
/// verb but `verify` takes the sign-in flags: `--client-id` (else
/// `LOSOS_GITHUB_CLIENT_ID`, else operators.json) and, for the tests' fake
/// GitHub, `--github-oauth-url` / `--github-api-url`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProvisionOpts {
    /// `provision whoami`: sign in, say who and whether they are listed.
    Whoami { github: ProvisionGithub },
    /// `provision token create|list|revoke --edge U ...`: the edge's
    /// terminal tokens (`docs/edge-terminal.md`), made and revoked with the
    /// GitHub token of the sign-in.
    Token {
        github: ProvisionGithub,
        edge: String,
        verb: TokenVerb,
    },
}

/// What `provision token` does on the edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenVerb {
    /// `create --name N --key FINGERPRINT [--scope read|full]
    /// [--expires-in-days D | --never-expires]`. Ninety days unless told.
    Create {
        name: String,
        fingerprint: String,
        scope: String,
        expires_in_days: Option<u32>,
    },
    List,
    /// `revoke ID`.
    Revoke {
        id: String,
    },
}

// Parsed once at startup and matched once; the size spread between the
// serve options and the one-shot subcommands costs nothing worth a Box.
#[allow(clippy::large_enum_variant)]
pub enum Mode {
    Serve(ServeOpts),
    Announce(AnnounceOpts),
    TunnelServer(crate::tunnel::ServerOpts),
    TunnelClient {
        config: String,
    },
    Join(JoinOpts),
    StripeGate(GateOpts),
    Passport(PassportOpts),
    Provision(ProvisionOpts),
    Enrol(EnrolOpts),
    Lab(crate::lab::LabOpts),
    ExitSocks(crate::exit::SocksOpts),
    /// `ech-key --key-file FILE --public-name NAME`: make the ECH key if
    /// missing (`crate::ech`) and print its ECHConfigList, before the TLS
    /// front starts.
    EchKey(EchOpts),
}

/// `enrol` options: the LAN owner's view of the boxes a `--enrol-dir`
/// edge took in on first contact (`crate::relay::Enrolment`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnrolOpts {
    /// `enrol list --dir DIR`: one `id hostname` line per enrolled box.
    List { dir: String },
    /// `enrol forget --dir DIR ID`: drop the box and its token file. The
    /// running registrar sees the file change on its next lookup; a box
    /// still heartbeating re-enrols with the token it holds, so this is
    /// for a box that is gone, not a way to evict one that is present.
    Forget { dir: String, id: String },
}

pub fn parse(args: Vec<String>) -> Result<Mode> {
    if args.is_empty() {
        return Err(miette!(
            "usage: losos-registrar serve|announce|tunnel-server|tunnel-client|join|stripe-gate|passport|provision|enrol|lab|exit-socks ..."
        ));
    }
    let mode = &args[0];
    let rest = &args[1..];
    // Convert slice to Vec<String> for the helper closures.
    let rest: Vec<String> = rest.to_vec();
    match mode.as_str() {
        "serve" => {
            let range =
                arg(&rest, "--port-range").ok_or_else(|| miette!("missing --port-range"))?;
            let (lo, hi) = parse_range(range)?;
            Ok(Mode::Serve(ServeOpts {
                listen: arg(&rest, "--listen")
                    .unwrap_or("127.0.0.1:8443")
                    .to_string(),
                registry_path: req(&rest, "--registry")?.to_string(),
                traefik_provider_listen: arg(&rest, "--traefik-provider-listen")
                    .unwrap_or("127.0.0.1:8090")
                    .to_string(),
                tunnel_config: req(&rest, "--tunnel-config")?.to_string(),
                port_range: (lo, hi),
                tunnel_pin_file: arg(&rest, "--tunnel-pin-file").map(str::to_string),
                tenants_file: req(&rest, "--tenants-file")?.to_string(),
                reconcile_interval: parse_dur(arg(&rest, "--reconcile-interval").unwrap_or("15s"))?,
                heartbeat_ttl: parse_dur(arg(&rest, "--heartbeat-ttl").unwrap_or("120s"))?,
                mesh_agent_token_file: arg(&rest, "--mesh-agent-token-file").map(str::to_string),
                mesh_server_addr: arg(&rest, "--mesh-server-addr").map(str::to_string),
                kube_api: arg(&rest, "--kube-api")
                    .unwrap_or("https://127.0.0.1:6443")
                    .trim_end_matches('/')
                    .to_string(),
                kube_token_file: arg(&rest, "--kube-token-file").map(str::to_string),
                kube_ca_file: arg(&rest, "--kube-ca-file").map(str::to_string),
                compute_windows_file: arg(&rest, "--compute-windows-file")
                    .unwrap_or("/var/lib/losos-registrar/compute-windows.json")
                    .to_string(),
                market: parse_market(&rest)?,
                onramp: parse_onramp(&rest)?,
                inference: match arg(&rest, "--inference-models") {
                    None => None,
                    Some(file) => Some(crate::inference::InferenceOpts {
                        models_file: file.to_string(),
                        port_range: parse_range(arg(&rest, "--llm-port-range").ok_or_else(
                            || miette!("--inference-models needs --llm-port-range"),
                        )?)?,
                        env_dir: arg(&rest, "--llm-env-dir")
                            .unwrap_or("/run/losos-llm-env")
                            .to_string(),
                        socket_dir: arg(&rest, "--llm-socket-dir")
                            .unwrap_or("/run/losos-llm-heads")
                            .to_string(),
                    }),
                },
                gateway: arg(&rest, "--gateway-key-file").map(|key| crate::gateway::GatewayOpts {
                    key_file: key.to_string(),
                    settings_file: arg(&rest, "--gateway-settings-file")
                        .unwrap_or("/var/lib/losos-gateway/settings.json")
                        .to_string(),
                    measured_file: arg(&rest, "--gateway-measured-file")
                        .unwrap_or("/run/losos-gateway/settings.json")
                        .to_string(),
                    tpm_dir: arg(&rest, "--gateway-tpm-dir")
                        .unwrap_or("/run/losos-gateway/tpm")
                        .to_string(),
                    tcti: arg(&rest, "--gateway-tcti")
                        .unwrap_or("device:/dev/tpm0")
                        .to_string(),
                }),
                github_api_url: arg(&rest, "--github-api-url")
                    .unwrap_or(GITHUB_API_URL)
                    .trim_end_matches('/')
                    .to_string(),
                domains: parse_domains(&rest)?,
                routes: parse_routes(&rest)?,
                enrol_dir: arg(&rest, "--enrol-dir").map(str::to_string),
                ci_dir: arg(&rest, "--ci-dir").map(str::to_string),
                hidden: match arg(&rest, "--hidden-dir") {
                    None => None,
                    Some(dir) => Some(crate::hidden::HiddenOpts {
                        dir: dir.to_string(),
                        addresses_file: arg(&rest, "--hidden-addresses")
                            .unwrap_or("/var/lib/losos-hidden/addresses.json")
                            .to_string(),
                        http_port: arg(&rest, "--hidden-http-port")
                            .unwrap_or("8480")
                            .parse()
                            .map_err(|_| miette!("bad --hidden-http-port"))?,
                        https_port: arg(&rest, "--hidden-https-port")
                            .unwrap_or("8444")
                            .parse()
                            .map_err(|_| miette!("bad --hidden-https-port"))?,
                    }),
                },
                exit: match arg(&rest, "--exit-dir") {
                    None => None,
                    Some(dir) => Some(crate::exit::ExitOpts {
                        dir: dir.to_string(),
                        subnet: crate::exit::parse_subnet(
                            arg(&rest, "--exit-subnet").unwrap_or("10.153.0.0/16"),
                        )
                        .ok_or_else(|| miette!("bad --exit-subnet"))?,
                        endpoint: req(&rest, "--exit-endpoint")?.to_string(),
                        public_key_file: req(&rest, "--exit-public-key-file")?.to_string(),
                        onion_file: arg(&rest, "--exit-onion-file").map(str::to_string),
                        onion_port: arg(&rest, "--exit-onion-port")
                            .unwrap_or("1080")
                            .parse()
                            .map_err(|_| miette!("bad --exit-onion-port"))?,
                    }),
                },
                tang: arg(&rest, "--tang-dir").map(|dir| crate::tang::TangOpts {
                    dir: dir.into(),
                    tangd: arg(&rest, "--tangd").unwrap_or("tangd").into(),
                    keygen: arg(&rest, "--tangd-keygen")
                        .unwrap_or("tangd-keygen")
                        .into(),
                }),
                operator: arg(&rest, "--access-dir").map(|dir| crate::operator::OperatorOpts {
                    access_dir: dir.to_string(),
                    keys_file: arg(&rest, "--access-keys-file")
                        .unwrap_or("/etc/losos/edge-access-keys.asc")
                        .to_string(),
                    seal_script: arg(&rest, "--seal-credential").map(str::to_string),
                    systemd_run: arg(&rest, "--systemd-run").map(str::to_string),
                }),
                manage: arg(&rest, "--manage-key-file").map(|file| ManageOpts {
                    key_file: file.to_string(),
                    ui_dir: arg(&rest, "--manage-ui").map(str::to_string),
                    traefik_metrics: arg(&rest, "--manage-traefik-metrics").map(str::to_string),
                }),
                calls_relay: parse_calls_relay(&rest)?,
                ech: match (
                    arg(&rest, "--ech-key-file"),
                    arg(&rest, "--ech-public-name"),
                ) {
                    (None, None) => None,
                    (Some(key_file), Some(public_name)) => Some(EchOpts {
                        key_file: key_file.to_string(),
                        public_name: public_name.to_string(),
                    }),
                    _ => return Err(miette!("--ech-key-file and --ech-public-name go together")),
                },
                secondary: match arg(&rest, "--secondary-file") {
                    None => None,
                    Some(file) => Some(crate::secondary::SecondaryOpts {
                        file: file.to_string(),
                        self_url: req(&rest, "--secondary-self-url")?
                            .trim_end_matches('/')
                            .to_string(),
                        self_tunnel: req(&rest, "--secondary-self-tunnel")?.to_string(),
                        interval: parse_dur(arg(&rest, "--secondary-interval").unwrap_or("10s"))?,
                    }),
                },
                official_cluster: match arg(&rest, "--official-cluster") {
                    None => None,
                    Some(c) if crate::relay::dns_label(c) => Some(c.to_string()),
                    Some(c) => return Err(miette!("--official-cluster {c:?} is not a DNS label")),
                },
                official_peers: {
                    let peers = args_all(&rest, "--official-peer")
                        .into_iter()
                        .map(|p| {
                            crate::official::OfficialPeer::parse(p)
                                .map_err(|e| miette!("--official-peer {e}"))
                        })
                        .collect::<Result<Vec<_>>>()?;
                    if peers.len() > crate::official::MAX_OFFICIALS {
                        return Err(miette!(
                            "more than {} --official-peer flags",
                            crate::official::MAX_OFFICIALS
                        ));
                    }
                    peers
                },
                bans_file: arg(&rest, "--bans-file").map(str::to_string),
                uplink: match arg(&rest, "--uplink-file") {
                    None => None,
                    Some(file) => Some(crate::relay::UplinkOpts {
                        file: file.to_string(),
                        tunnel_config: arg(&rest, "--uplink-tunnel-config")
                            .unwrap_or("/etc/losos-tunnel/uplink.json")
                            .to_string(),
                        interval: parse_dur(arg(&rest, "--uplink-interval").unwrap_or("30s"))?,
                    }),
                },
            }))
        }
        "passport" => parse_passport(&rest).map(Mode::Passport),
        "provision" => parse_provision(&rest),
        "lab" => parse_lab(&rest).map(Mode::Lab),
        "exit-socks" => Ok(Mode::ExitSocks(crate::exit::SocksOpts {
            listen: arg(&rest, "--listen")
                .unwrap_or("127.0.0.1:1081")
                .to_string(),
            peers_file: req(&rest, "--peers")?.to_string(),
        })),
        "enrol" => {
            let verb = rest.first().map(String::as_str).unwrap_or("");
            let rest: Vec<String> = rest.iter().skip(1).cloned().collect();
            match verb {
                "list" => Ok(Mode::Enrol(EnrolOpts::List {
                    dir: req(&rest, "--dir")?.to_string(),
                })),
                "forget" => {
                    let dir = req(&rest, "--dir")?.to_string();
                    // The one positional: whatever is neither `--dir` nor
                    // its value.
                    let id = rest
                        .iter()
                        .enumerate()
                        .find(|(i, a)| !a.starts_with("--") && (*i == 0 || rest[i - 1] != "--dir"))
                        .map(|(_, a)| a.clone())
                        .ok_or_else(|| {
                            miette!("usage: losos-registrar enrol forget --dir DIR ID")
                        })?;
                    Ok(Mode::Enrol(EnrolOpts::Forget { dir, id }))
                }
                _ => Err(miette!(
                    "usage: losos-registrar enrol list|forget --dir DIR ..."
                )),
            }
        }
        "announce" => Ok(Mode::Announce(AnnounceOpts {
            registrar_url: req(&rest, "--registrar-url")?.to_string(),
            appliance_id: req(&rest, "--appliance-id")?.to_string(),
            hostname: req(&rest, "--hostname")?.to_string(),
            token_file: req(&rest, "--token-file")?.to_string(),
            heartbeat_interval: parse_dur(arg(&rest, "--heartbeat-interval").unwrap_or("30s"))?,
            idle_load_threshold: parse_threshold(
                arg(&rest, "--idle-load-threshold").unwrap_or("0.25"),
            )?,
            relay_pass_file: arg(&rest, "--relay-pass-file").map(str::to_string),
            managed_file: arg(&rest, "--managed-file").map(str::to_string),
            llm_lend_file: arg(&rest, "--llm-lend-file").map(str::to_string),
            hidden_file: arg(&rest, "--hidden-file").map(str::to_string),
            tls_ready_file: arg(&rest, "--tls-ready-file").map(str::to_string),
            tls_names_file: arg(&rest, "--tls-names-file").map(str::to_string),
        })),
        "ech-key" => Ok(Mode::EchKey(EchOpts {
            key_file: req(&rest, "--key-file")?.to_string(),
            public_name: req(&rest, "--public-name")?.to_string(),
        })),
        "tunnel-server" => Ok(Mode::TunnelServer(crate::tunnel::ServerOpts {
            listen: arg(&rest, "--listen")
                .unwrap_or("[::]:2333")
                .parse()
                .map_err(|_| miette!("bad --listen; expected ip:port"))?,
            identity_dir: req(&rest, "--identity-dir")?.into(),
            config: req(&rest, "--config")?.into(),
        })),
        "tunnel-client" => Ok(Mode::TunnelClient {
            config: req(&rest, "--config")?.to_string(),
        }),
        "join" => Ok(Mode::Join(JoinOpts {
            registrar_url: req(&rest, "--registrar-url")?.to_string(),
            appliance_id: req(&rest, "--appliance-id")?.to_string(),
            node_name: req(&rest, "--node-name")?.to_string(),
            token_file: req(&rest, "--token-file")?.to_string(),
            out_token_file: req(&rest, "--out-token-file")?.to_string(),
            share_compute: parse_bool(
                "--share-compute",
                arg(&rest, "--share-compute").unwrap_or("false"),
            )?,
            host_vms: parse_bool("--host-vms", arg(&rest, "--host-vms").unwrap_or("false"))?,
            window_start: parse_hhmm(arg(&rest, "--window-start").unwrap_or("23:00"))?.to_string(),
            window_end: parse_hhmm(arg(&rest, "--window-end").unwrap_or("07:00"))?.to_string(),
            window_tz: parse_tz(arg(&rest, "--window-tz").unwrap_or("UTC"))?.to_string(),
            expect_server_addr: arg(&rest, "--expect-server-addr").map(str::to_string),
        })),
        "stripe-gate" => {
            let currency = arg(&rest, "--currency").unwrap_or("eur");
            if !valid_currency(currency) {
                return Err(miette!(
                    "bad --currency {currency:?}; expected a supported two-decimal ISO 4217 code"
                ));
            }
            Ok(Mode::StripeGate(GateOpts {
                socket: req(&rest, "--socket")?.to_string(),
                stripe_key_file: req(&rest, "--stripe-key-file")?.to_string(),
                webhook_secret_file: req(&rest, "--webhook-secret-file")?.to_string(),
                stripe_api: arg(&rest, "--stripe-api")
                    .unwrap_or("https://api.stripe.com")
                    .to_string(),
                currency: currency.to_string(),
                fee_bps: match arg(&rest, "--fee-bps") {
                    None => None,
                    Some(raw) => match raw.parse::<u32>() {
                        Ok(bps) if bps <= MAX_FEE_BPS => Some(bps),
                        _ => {
                            return Err(miette!(
                                "bad --fee-bps {raw:?}; expected at most {MAX_FEE_BPS}"
                            ))
                        }
                    },
                },
                return_url: match arg(&rest, "--return-url") {
                    None => None,
                    Some(u) if crate::stripe_gate::url_ok(u) => Some(u.to_string()),
                    Some(u) => return Err(miette!("bad --return-url {u:?}")),
                },
                hardware_catalogue: hardware_catalogue(&rest, "--hardware-catalogue", currency)?,
            }))
        }
        other => Err(miette!(
            "unknown subcommand {other:?}; expected serve|announce|seed|join|stripe-gate"
        )),
    }
}

/// `provision token <verb>`: the flags after `token`, verb first.
fn parse_token_verb(rest: &[String]) -> Result<TokenVerb> {
    let usage = || {
        miette!(
            "usage: losos-registrar provision token create|list|revoke --edge URL ...\n  \
             create --name N --key FINGERPRINT [--scope read|full] [--expires-in-days D | --never-expires]\n  \
             revoke ID"
        )
    };
    let verb = rest.first().map(String::as_str).ok_or_else(usage)?;
    let rest = &rest[1..];
    match verb {
        "list" => Ok(TokenVerb::List),
        "create" => {
            let scope = arg(rest, "--scope").unwrap_or("full");
            if !matches!(scope, "read" | "full") {
                return Err(miette!("bad --scope {scope:?}; expected read or full"));
            }
            let never = rest.iter().any(|a| a == "--never-expires");
            let days = arg(rest, "--expires-in-days");
            let expires_in_days = match (never, days) {
                (true, Some(_)) => {
                    return Err(miette!(
                        "--never-expires and --expires-in-days exclude each other"
                    ))
                }
                (true, None) => None,
                (false, None) => Some(90),
                (false, Some(raw)) => Some(
                    raw.parse()
                        .ok()
                        .filter(|d| (1..=3660).contains(d))
                        .ok_or_else(|| {
                            miette!("bad --expires-in-days {raw:?}; expected 1..=3660")
                        })?,
                ),
            };
            Ok(TokenVerb::Create {
                name: req(rest, "--name")?.to_string(),
                fingerprint: req(rest, "--key")?.to_string(),
                scope: scope.to_string(),
                expires_in_days,
            })
        }
        "revoke" => {
            // The one positional: whatever is not a flag or a flag's value.
            let id = rest
                .iter()
                .enumerate()
                .find(|(i, a)| !a.starts_with("--") && (*i == 0 || !rest[i - 1].starts_with("--")))
                .map(|(_, a)| a.clone())
                .ok_or_else(usage)?;
            Ok(TokenVerb::Revoke { id })
        }
        _ => Err(usage()),
    }
}

fn valid_currency(c: &str) -> bool {
    SUPPORTED_CURRENCIES.contains(&c)
}

/// The `--market-*` flags. Enabled by `--market-gate-socket`, the Unix socket
/// of the Stripe gate (`stripe-gate`), which alone holds the key and the
/// webhook secrets; the return URL is then required, because a market that can
/// take payment but has nowhere to send the buyer back to would be half-made.
/// Or enabled by `--market-no-cards`: coins only, no gate and no Stripe
/// account, which needs `--market-usdc` and takes the mode of its networks.
fn parse_passport(args: &[String]) -> Result<PassportOpts> {
    let verb = args.first().map(String::as_str).unwrap_or("");
    let rest: Vec<String> = args.iter().skip(1).cloned().collect();
    Ok(match verb {
        "statement" => PassportOpts::Statement {
            url: req(&rest, "--url")?.to_string(),
            edge_key: req(&rest, "--edge-key")?.to_string(),
            payouts: args_all(&rest, "--payout")
                .into_iter()
                .map(|p| {
                    p.split_once('=')
                        .map(|(n, a)| (n.to_string(), a.to_string()))
                        .ok_or_else(|| miette!("--payout takes NETWORK=ADDRESS, not {p}"))
                })
                .collect::<Result<_>>()?,
            days: arg(&rest, "--days")
                .unwrap_or("365")
                .parse()
                .ok()
                .filter(|d| (1..=3650).contains(d))
                .ok_or_else(|| miette!("bad --days; expected 1..=3650"))?,
        },
        "settings" => PassportOpts::Settings {
            statement: req(&rest, "--statement")?.to_string(),
            signature: req(&rest, "--signature")?.to_string(),
            key: req(&rest, "--key")?.to_string(),
            losos_payouts: req(&rest, "--losos-payouts")?.to_string(),
        },
        "measure" => PassportOpts::Measure {
            uki: req(&rest, "--uki")?.to_string(),
            name: req(&rest, "--name")?.to_string(),
            apps: args_all(&rest, "--app")
                .into_iter()
                .map(str::to_string)
                .collect(),
        },
        "image" => PassportOpts::Image {
            name: req(&rest, "--name")?.to_string(),
            event_log: arg(&rest, "--event-log")
                .unwrap_or("/sys/kernel/security/tpm0/binary_bios_measurements")
                .to_string(),
        },
        "check" => PassportOpts::Check {
            url: req(&rest, "--url")?.trim_end_matches('/').to_string(),
            tpm_roots: req(&rest, "--tpm-roots")?.to_string(),
            images: req(&rest, "--images")?.to_string(),
            payouts: req(&rest, "--payouts")?.to_string(),
        },
        _ => {
            return Err(miette!(
                "usage: losos-registrar passport statement|settings|measure|image|check ..."
            ))
        }
    })
}

fn parse_provision(args: &[String]) -> Result<Mode> {
    let verb = args.first().map(String::as_str).unwrap_or("");
    let rest: Vec<String> = args.iter().skip(1).cloned().collect();
    let github = ProvisionGithub {
        oauth_url: arg(&rest, "--github-oauth-url")
            .unwrap_or(GITHUB_OAUTH_URL)
            .to_string(),
        api_url: arg(&rest, "--github-api-url")
            .unwrap_or(GITHUB_API_URL)
            .to_string(),
        client_id: arg(&rest, "--client-id").map(str::to_string),
    };
    let mode = match verb {
        "whoami" => ProvisionOpts::Whoami { github },
        "token" => ProvisionOpts::Token {
            github,
            edge: req(&rest, "--edge")?.trim_end_matches('/').to_string(),
            verb: parse_token_verb(&rest)?,
        },
        _ => return Err(miette!("usage: losos-registrar provision whoami|token ...")),
    };
    Ok(Mode::Provision(mode))
}

/// Binance's on-ramp (`crate::onramp`):
///
/// ```text
/// --onramp-binance-api https://<the origin Binance gave the partner>
/// --onramp-binance-client-id <X-Tesla-ClientId>
/// --onramp-binance-key-file /run/credentials/losos-registrar.service/binance-connect-key
/// --onramp-binance-token-file /run/credentials/losos-registrar.service/binance-connect-token
/// --onramp-binance-webhook-key <Binance's public key, base64>
/// ```
///
/// It sells USDC on Base, so the market must settle on Base mainnet: a test
/// edge's USDC is not what Binance delivers.
fn parse_onramp(args: &[String]) -> Result<Option<crate::onramp::OnrampOpts>> {
    let Some(api_url) = arg(args, "--onramp-binance-api") else {
        if let Some(flag) = [
            "--onramp-binance-client-id",
            "--onramp-binance-key-file",
            "--onramp-binance-token-file",
            "--onramp-binance-webhook-key-file",
        ]
        .into_iter()
        .find(|f| arg(args, f).is_some())
        {
            return Err(miette!("{flag} needs --onramp-binance-api"));
        }
        return Ok(None);
    };
    let api_url = api_url.trim_end_matches('/');
    if !crate::onramp::api_url_ok(api_url) {
        return Err(miette!(
            "bad --onramp-binance-api {api_url:?}; expected an https origin with no path (http only to loopback)"
        ));
    }
    let client_id = req(args, "--onramp-binance-client-id")?;
    if client_id.is_empty() || !client_id.bytes().all(|b| b.is_ascii_graphic()) {
        return Err(miette!("bad --onramp-binance-client-id"));
    }
    let file = |flag: &str| -> Result<String> {
        let path = req(args, flag)?;
        if !path.starts_with('/') {
            return Err(miette!("{flag} must be a credential file path"));
        }
        Ok(path.to_string())
    };
    // Binance's key for its notifications: public, PEM or one base64 line.
    let key_path = req(args, "--onramp-binance-webhook-key-file")?;
    let key_text = std::fs::read_to_string(key_path)
        .map_err(|e| miette!("read --onramp-binance-webhook-key-file {key_path}: {e}"))?;
    let webhook_key = crate::onramp::parse_public_key(&key_text).ok_or_else(|| {
        miette!("--onramp-binance-webhook-key-file {key_path} holds no RSA public key")
    })?;
    let live_base = parse_usdc(args, arg(args, "--market-currency").unwrap_or("eur"))
        .ok()
        .flatten()
        .is_some_and(|u| !u.test() && u.chains.iter().any(|c| c.network.key == "base"));
    if !live_base {
        return Err(miette!(
            "--onramp-binance-api needs --market-usdc network=base: Binance delivers USDC on Base mainnet"
        ));
    }
    let state_file = arg(args, "--market-state-file")
        .map(|m| {
            std::path::Path::new(m)
                .with_file_name("onramp.json")
                .to_string_lossy()
                .into_owned()
        })
        .unwrap_or_else(|| "/var/lib/losos-registrar/onramp.json".to_string());
    Ok(Some(crate::onramp::OnrampOpts {
        api_url: api_url.to_string(),
        client_id: client_id.to_string(),
        key_file: file("--onramp-binance-key-file")?,
        token_file: file("--onramp-binance-token-file")?,
        webhook_key,
        state_file,
    }))
}

fn parse_market(args: &[String]) -> Result<Option<Box<MarketOpts>>> {
    // `--market-no-cards`: coins only, no Stripe gate. Exactly one of the two
    // turns the market on.
    let cards = !args.iter().any(|a| a == "--market-no-cards");
    let gate_socket = match (arg(args, "--market-gate-socket"), cards) {
        (None, true) => return Ok(None),
        (Some(socket), true) => socket,
        (None, false) => "",
        (Some(_), false) => {
            return Err(miette!(
                "--market-no-cards takes no --market-gate-socket: an edge without cards runs no Stripe gate"
            ))
        }
    };
    let currency = arg(args, "--market-currency").unwrap_or("eur");
    if !valid_currency(currency) {
        return Err(miette!(
            "bad --market-currency {currency:?}; expected a supported two-decimal ISO 4217 code"
        ));
    }
    let fee_bps: u32 = match arg(args, "--market-fee-bps") {
        None => DEFAULT_FEE_BPS,
        Some(raw) => raw
            .parse()
            .map_err(|_| miette!("bad --market-fee-bps {raw:?}"))?,
    };
    if fee_bps > MAX_FEE_BPS {
        return Err(miette!(
            "--market-fee-bps {fee_bps} exceeds the {MAX_FEE_BPS} basis point ceiling"
        ));
    }
    let storage_class =
        arg(args, "--market-storage-class").unwrap_or(crate::market::DEFAULT_STORAGE_CLASS);
    if !crate::relay::dns_name(storage_class) {
        return Err(miette!(
            "bad --market-storage-class {storage_class:?}; expected a Kubernetes object name (DNS-1123 subdomain)"
        ));
    }
    // Where Stripe sends a buyer back to: needed only with cards.
    let return_url = if cards {
        req(args, "--market-return-url")?
    } else {
        arg(args, "--market-return-url").unwrap_or("")
    };
    if (cards || !return_url.is_empty()) && !crate::stripe_gate::url_ok(return_url) {
        return Err(miette!(
            "--market-return-url must be an absolute http(s) URL with a host, no credentials and no #fragment"
        ));
    }
    let hardware = hardware_catalogue(args, "--market-hardware-catalogue", currency)?;
    let usdc = parse_usdc(args, currency)?;
    if !cards && usdc.is_none() {
        return Err(miette!(
            "--market-no-cards needs --market-usdc: without cards the market takes coins, and USDC is the checkout they ride on"
        ));
    }
    if !cards && hardware.is_some() {
        return Err(miette!(
            "--market-hardware-catalogue needs cards: hardware is paid by card through the Stripe gate"
        ));
    }
    // Cents a seller may owe on coin sales before buyers can no longer pay
    // them in coins. Capped so a stray digit cannot switch the rule off.
    let coin_fee_limit: u64 = match arg(args, "--market-coin-fee-limit") {
        None => crate::market::DEFAULT_COIN_FEE_LIMIT,
        Some(raw) => raw
            .parse()
            .map_err(|_| miette!("bad --market-coin-fee-limit {raw:?}; expected cents"))?,
    };
    if coin_fee_limit > MAX_COIN_FEE_LIMIT {
        return Err(miette!(
            "--market-coin-fee-limit {coin_fee_limit} exceeds {MAX_COIN_FEE_LIMIT} cents"
        ));
    }
    // How long a seller's box may be offline in a row before its buyers are
    // refunded (`crate::slots`). Under a minute would refund a box for a
    // reboot; over a week would leave a buyer paying for nothing that long.
    let offline_refund_secs: u64 = match arg(args, "--market-offline-refund-after") {
        None => crate::slots::DEFAULT_OFFLINE_SECS,
        Some(raw) => raw
            .parse()
            .map_err(|_| miette!("bad --market-offline-refund-after {raw:?}; expected seconds"))?,
    };
    if !(MIN_OFFLINE_REFUND_SECS..=MAX_OFFLINE_REFUND_SECS).contains(&offline_refund_secs) {
        return Err(miette!(
            "--market-offline-refund-after {offline_refund_secs} is outside {MIN_OFFLINE_REFUND_SECS}..={MAX_OFFLINE_REFUND_SECS} seconds"
        ));
    }
    // The image of the backup door on bought storage (`server/storage.rs`).
    // Only ever run with imagePullPolicy Never, so it is a name the edge's own
    // node already holds, and the shape is checked here rather than at the
    // first box's backup.
    let storage_image = arg(args, "--market-storage-image").map(str::to_string);
    if let Some(image) = &storage_image {
        if !image_ref_ok(image) {
            return Err(miette!(
                "bad --market-storage-image {image:?}; expected an image reference such as losos.local/rest-server:0.13.0"
            ));
        }
    }
    // Coins and Solana USDC reach the seller's wallet at once, so nothing
    // can be held for the slot or refunded from it. Off unless the operator
    // takes that on.
    let direct_pay = args.iter().any(|a| a == "--market-direct-pay");
    Ok(Some(Box::new(MarketOpts {
        state_file: arg(args, "--market-state-file")
            .unwrap_or("/var/lib/losos-registrar/market.json")
            .to_string(),
        gate_socket: gate_socket.to_string(),
        return_url: return_url.to_string(),
        currency: currency.to_string(),
        fee_bps,
        storage_class: storage_class.to_string(),
        storage_image,
        hardware_catalogue: hardware,
        vms: parse_vms(args)?,
        usdc,
        btc: parse_btc(args, currency)?,
        xmr: parse_xmr(args, currency)?,
        cards,
        zec: parse_zec(args, currency)?,
        coin_fee_limit,
        offline_refund_secs,
        direct_pay,
    })))
}

/// `registry/name:tag`: lowercase letters, digits and `.-_/:`, one tag.
fn image_ref_ok(image: &str) -> bool {
    let Some((name, tag)) = image.rsplit_once(':') else {
        return false;
    };
    !name.is_empty()
        && !tag.is_empty()
        && image.len() <= 200
        && !name.starts_with(['/', '.', '-'])
        && name.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'-' | b'_' | b'/')
        })
        && tag
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
}

const MIN_OFFLINE_REFUND_SECS: u64 = 60;
const MAX_OFFLINE_REFUND_SECS: u64 = 7 * 24 * 60 * 60;

/// Bitcoin orders, through the edge's own node:
///
/// ```text
/// --market-btc-rpc http://127.0.0.1:8332 --market-btc-cookie-file /var/lib/bitcoind-losos/.cookie
/// ```
///
/// The cookie file is read on every call, since the node writes a new one
/// each time it starts. `--market-btc-wallet` names the watch-only wallet
/// the order addresses go into. Coin orders are quoted in US dollars and
/// paid from the same checkout as USDC, so both are required.
fn parse_btc(args: &[String], currency: &str) -> Result<Option<crate::bitcoin::BtcOpts>> {
    let Some(rpc) = arg(args, "--market-btc-rpc") else {
        return Ok(None);
    };
    if !crate::stablecoin::rpc_url_ok(rpc) {
        return Err(miette!(
            "bad --market-btc-rpc; expected https, or http to 127.0.0.1, with no credentials in it"
        ));
    }
    let cookie_file = req(args, "--market-btc-cookie-file")?;
    let wallet = arg(args, "--market-btc-wallet").unwrap_or(crate::bitcoin::DEFAULT_WALLET);
    if wallet.is_empty()
        || wallet.len() > 64
        || !wallet
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(miette!("bad --market-btc-wallet {wallet:?}"));
    }
    if currency != "usd" {
        return Err(miette!("--market-btc-rpc needs --market-currency usd"));
    }
    if args_all(args, "--market-usdc").is_empty() {
        return Err(miette!("--market-btc-rpc needs --market-usdc"));
    }
    Ok(Some(crate::bitcoin::BtcOpts {
        rpc: rpc.to_string(),
        cookie_file: cookie_file.to_string(),
        wallet: wallet.to_string(),
    }))
}

/// Monero orders, through the edge's own wallet service and node:
///
/// ```text
/// --market-xmr-wallet-rpc http://127.0.0.1:18083 --market-xmr-login-file /var/lib/losos-monero-wallet/rpc-login
/// --market-xmr-daemon-rpc http://127.0.0.1:18081
/// ```
///
/// The login file is the wallet service's `user:password`, read on every
/// call. Like Bitcoin, Monero is quoted in US dollars beside USDC.
fn parse_xmr(args: &[String], currency: &str) -> Result<Option<crate::monero::XmrOpts>> {
    let Some(wallet_rpc) = arg(args, "--market-xmr-wallet-rpc") else {
        return Ok(None);
    };
    let daemon_rpc = req(args, "--market-xmr-daemon-rpc")?;
    for (flag, url) in [
        ("--market-xmr-wallet-rpc", wallet_rpc),
        ("--market-xmr-daemon-rpc", daemon_rpc),
    ] {
        if !crate::stablecoin::rpc_url_ok(url) {
            return Err(miette!(
                "bad {flag}; expected https, or http to 127.0.0.1, with no credentials in it"
            ));
        }
    }
    let login_file = req(args, "--market-xmr-login-file")?;
    if currency != "usd" {
        return Err(miette!(
            "--market-xmr-wallet-rpc needs --market-currency usd"
        ));
    }
    if args_all(args, "--market-usdc").is_empty() {
        return Err(miette!("--market-xmr-wallet-rpc needs --market-usdc"));
    }
    Ok(Some(crate::monero::XmrOpts {
        wallet_rpc: wallet_rpc.to_string(),
        login_file: login_file.to_string(),
        daemon_rpc: daemon_rpc.to_string(),
    }))
}

/// Shielded Zcash orders, watched through public lightwalletd servers, one
/// `--market-zec-server` each:
///
/// ```text
/// --market-zec-server https://zec-node.cakewallet.com:443 --market-zec-server https://na.zec.rocks:443
/// ```
///
/// A payment counts once servers of two operators agree on it, so the list
/// names at least two. `--market-zec-network` is `main` unless a test edge
/// names `test` or `regtest`. Quoted in US dollars beside USDC, like Bitcoin.
fn parse_zec(args: &[String], currency: &str) -> Result<Option<crate::zcash::ZecOpts>> {
    let servers = args_all(args, "--market-zec-server");
    if servers.is_empty() {
        return Ok(None);
    }
    for url in &servers {
        if !crate::stablecoin::rpc_url_ok(url) {
            return Err(miette!(
                "bad --market-zec-server {url:?}; expected https, or http to 127.0.0.1, with no credentials in it"
            ));
        }
    }
    let servers: Vec<String> = servers.iter().map(|s| (*s).to_string()).collect();
    if !crate::zcash::operators_enough(&servers) {
        return Err(miette!(
            "--market-zec-server names servers of one operator; a payment counts only when two operators agree"
        ));
    }
    let network = match arg(args, "--market-zec-network") {
        None => crate::zcash::Network::Main,
        Some(text) => crate::zcash::Network::parse(text).ok_or_else(|| {
            miette!("bad --market-zec-network {text:?}; expected main, test or regtest")
        })?,
    };
    if currency != "usd" {
        return Err(miette!("--market-zec-server needs --market-currency usd"));
    }
    if args_all(args, "--market-usdc").is_empty() {
        return Err(miette!("--market-zec-server needs --market-usdc"));
    }
    Ok(Some(crate::zcash::ZecOpts { servers, network }))
}

/// USDC settlement: one `--market-usdc` per network, each a comma list of
/// `key=value`:
///
/// ```text
/// --market-usdc network=base-sepolia,rpc=/run/credentials/losos-registrar.service/usdc-rpc-base-sepolia,pay=0x...,treasury=0x...
/// --market-usdc network=solana-devnet,rpc=/run/credentials/losos-registrar.service/usdc-rpc-solana-devnet,pay=<program>,treasury=<base58>
/// ```
///
/// The token address is Circle's for the network, from
/// [`crate::stablecoin::NETWORKS`], not a flag, so a config cannot point the
/// market at a lookalike token. The RPC URL is a file the unit reads, never
/// the URL itself: the URL is a systemd credential and must not be on the
/// command line, where `ps` would show it. Every network must be of one
/// mode, test or live. `--market-ens-rpc-file` adds name lookups on Ethereum
/// (live) or Sepolia (test). `--market-usdc-releaser-file` is the escrow's
/// releaser key, also a credential file, and every network with an escrow
/// needs it: every EVM network, and Solana when `pay` names the
/// `losos-escrow` program. A Solana network without `pay` is paid straight
/// to the seller, which only a direct-pay edge offers.
fn parse_usdc(args: &[String], currency: &str) -> Result<Option<crate::stablecoin::UsdcOpts>> {
    use crate::stablecoin::{Family, UsdcChain, UsdcOpts};
    let specs = args_all(args, "--market-usdc");
    let ens = arg(args, "--market-ens-rpc-file");
    let releaser = arg(args, "--market-usdc-releaser-file");
    if specs.is_empty() {
        if ens.is_some() {
            return Err(miette!(
                "--market-ens-rpc-file needs at least one --market-usdc"
            ));
        }
        if releaser.is_some() {
            return Err(miette!(
                "--market-usdc-releaser-file needs at least one --market-usdc"
            ));
        }
        return Ok(None);
    }
    if currency != "usd" {
        return Err(miette!(
            "USDC settlement needs --market-currency usd; prices are USDC cents"
        ));
    }
    let credential = |flag: &str, path: &str| -> Result<String> {
        if path.is_empty()
            || path.starts_with("http://")
            || path.starts_with("https://")
            || path.contains('\n')
        {
            return Err(miette!(
                "{flag} must be a credential file path, not the URL"
            ));
        }
        Ok(path.to_string())
    };
    let mut chains: Vec<UsdcChain> = Vec::new();
    for spec in specs {
        let mut fields = std::collections::BTreeMap::new();
        for part in spec.split(',') {
            let Some((k, v)) = part.split_once('=') else {
                return Err(miette!(
                    "bad --market-usdc {spec:?}; expected key=value pairs"
                ));
            };
            if fields.insert(k, v).is_some() {
                return Err(miette!("--market-usdc {spec:?} names {k} twice"));
            }
        }
        let field = |k: &str| fields.get(k).copied();
        if let Some(unknown) = fields
            .keys()
            .find(|k| !matches!(**k, "network" | "rpc" | "pay" | "treasury" | "genesis"))
        {
            return Err(miette!("--market-usdc does not take {unknown}"));
        }
        let key =
            field("network").ok_or_else(|| miette!("--market-usdc {spec:?} has no network"))?;
        let names: Vec<&str> = crate::stablecoin::NETWORKS.iter().map(|n| n.key).collect();
        let network = crate::stablecoin::network(key).ok_or_else(|| {
            miette!(
                "--market-usdc network {key:?} is not one of {}",
                names.join(", ")
            )
        })?;
        if chains.iter().any(|c| c.network.key == key) {
            return Err(miette!("--market-usdc names {key} twice"));
        }
        if chains
            .first()
            .is_some_and(|c| c.network.test != network.test)
        {
            return Err(miette!(
                "--market-usdc mixes test and live networks; one edge settles in one mode"
            ));
        }
        let rpc = credential(
            "--market-usdc rpc",
            field("rpc").ok_or_else(|| miette!("--market-usdc {key} has no rpc"))?,
        )?;
        let treasury =
            field("treasury").ok_or_else(|| miette!("--market-usdc {key} has no treasury"))?;
        let chain = match network.family {
            Family::Evm => {
                let pay = field("pay").ok_or_else(|| miette!("--market-usdc {key} has no pay"))?;
                let Some(pay) = crate::stablecoin::normalize_address(pay) else {
                    return Err(miette!("--market-usdc {key} pay is not an 0x address"));
                };
                let Some(treasury) = crate::stablecoin::normalize_address(treasury) else {
                    return Err(miette!("--market-usdc {key} treasury is not an 0x address"));
                };
                if pay == treasury {
                    return Err(miette!("--market-usdc {key} pay and treasury must differ"));
                }
                if field("genesis").is_some() {
                    return Err(miette!("--market-usdc genesis is for Solana networks"));
                }
                UsdcChain {
                    network,
                    rpc_file: rpc,
                    pay: Some(pay),
                    treasury,
                    genesis: None,
                }
            }
            Family::Solana => {
                let pay = field("pay")
                    .map(|p| {
                        crate::solana::normalize_address(p).ok_or_else(|| {
                            miette!("--market-usdc {key} pay is not the escrow program's address")
                        })
                    })
                    .transpose()?;
                let Some(treasury) = crate::solana::normalize_address(treasury) else {
                    return Err(miette!(
                        "--market-usdc {key} treasury is not a Solana address"
                    ));
                };
                // A local validator has its own genesis. Only a test network
                // may name one, so a live edge always checks mainnet's.
                let genesis = match field("genesis") {
                    None => None,
                    Some(_) if !network.test => {
                        return Err(miette!(
                            "--market-usdc {key} genesis is only for test networks"
                        ));
                    }
                    Some(g) => Some(
                        crate::solana::normalize_address(g)
                            .ok_or_else(|| miette!("--market-usdc {key} genesis is not a hash"))?,
                    ),
                };
                if pay.as_deref() == Some(treasury.as_str()) {
                    return Err(miette!("--market-usdc {key} pay and treasury must differ"));
                }
                UsdcChain {
                    network,
                    rpc_file: rpc,
                    pay,
                    treasury,
                    genesis,
                }
            }
        };
        chains.push(chain);
    }
    // The table's order, not the flags': Nix passes an attribute set in
    // name order, and the first network is the one a page offers first.
    chains.sort_by_key(|c| {
        crate::stablecoin::NETWORKS
            .iter()
            .position(|n| n.key == c.network.key)
    });
    let ens_rpc_file = ens
        .map(|path| credential("--market-ens-rpc-file", path))
        .transpose()?;
    let price_sources = match arg(args, "--market-price-sources") {
        None => [
            crate::coins::PriceSource::kraken(),
            crate::coins::PriceSource::coingecko(),
        ],
        Some(spec) => {
            let (kraken, gecko) = spec.split_once(',').ok_or_else(|| {
                miette!("--market-price-sources is <kraken base URL>,<CoinGecko base URL>")
            })?;
            for url in [kraken, gecko] {
                if !(url.starts_with("https://") || url.starts_with("http://127.0.0.1")) {
                    return Err(miette!(
                        "--market-price-sources takes https URLs, or http on 127.0.0.1"
                    ));
                }
            }
            [
                crate::coins::PriceSource::Kraken {
                    base: kraken.to_string(),
                },
                crate::coins::PriceSource::CoinGecko {
                    base: gecko.to_string(),
                },
            ]
        }
    };
    let releaser_file = releaser
        .map(|path| credential("--market-usdc-releaser-file", path))
        .transpose()?;
    // An escrow holds each payment until the edge releases it, so an edge
    // without the key would take payments it can never pay out.
    if releaser_file.is_none() && chains.iter().any(|c| c.pay.is_some()) {
        return Err(miette!(
            "a --market-usdc with an escrow needs --market-usdc-releaser-file: the escrow's releaser key"
        ));
    }
    Ok(Some(UsdcOpts {
        chains,
        ens_rpc_file,
        price_sources,
        releaser_file,
    }))
}

/// The `--vm-*` flags: virtual machines on the mesh (`crate::vms`), turned on
/// by `--vm-enable` on an edge that runs the market. The catalogue is loaded
/// here so a bad file stops the unit at start rather than at the first order.
fn parse_vms(args: &[String]) -> Result<Option<crate::vms::VmOpts>> {
    if !args.iter().any(|a| a == "--vm-enable") {
        return Ok(None);
    }
    let catalogue = arg(args, "--vm-catalogue").map(str::to_string);
    crate::vms::Catalogue::load(catalogue.as_deref())
        .map_err(|e| miette!("bad --vm-catalogue: {e}"))?;
    let number = |flag: &str, default: u64, range: std::ops::RangeInclusive<u64>| -> Result<u64> {
        match arg(args, flag) {
            None => Ok(default),
            Some(raw) => raw
                .parse::<u64>()
                .ok()
                .filter(|n| range.contains(n))
                .ok_or_else(|| {
                    miette!(
                        "bad {flag} {raw:?}; expected {}..={}",
                        range.start(),
                        range.end()
                    )
                }),
        }
    };
    let defaults = crate::vms::Shape::default();
    let shape = crate::vms::Shape {
        cpu: u32::try_from(number("--vm-cpu", u64::from(defaults.cpu), 1..=16)?)
            .map_err(|_| miette!("bad --vm-cpu"))?,
        memory_mib: number(
            "--vm-memory-mib",
            defaults.memory_mib,
            512..=crate::vms::MAX_MEMORY_MIB,
        )?,
        disk_gib: number(
            "--vm-disk-gib",
            defaults.disk_gib,
            1..=crate::vms::MAX_DISK_GIB,
        )?,
    };
    let storage_class = arg(args, "--vm-storage-class").map(str::to_string);
    if let Some(class) = &storage_class {
        if !crate::relay::dns_name(class) {
            return Err(miette!(
                "bad --vm-storage-class {class:?}; expected a Kubernetes object name"
            ));
        }
    }
    let fetch_base = req(args, "--vm-fetch-base")?
        .trim_end_matches('/')
        .to_string();
    if !fetch_base.starts_with("https://") || !crate::stripe_gate::url_ok(&fetch_base) {
        return Err(miette!(
            "--vm-fetch-base must be this registrar's public https:// origin"
        ));
    }
    let domain = arg(args, "--vm-domain").map(str::to_string);
    if let Some(d) = &domain {
        if !crate::relay::dns_name(d) || !d.contains('.') {
            return Err(miette!("bad --vm-domain {d:?}; expected a domain name"));
        }
    }
    Ok(Some(crate::vms::VmOpts {
        catalogue,
        shape,
        storage_class,
        upload_dir: arg(args, "--vm-upload-dir")
            .unwrap_or("/var/lib/losos-registrar/vm-images")
            .to_string(),
        max_upload_bytes: number("--vm-upload-max-gib", 32, 1..=crate::vms::MAX_DISK_GIB)?
            * 1024
            * 1024
            * 1024,
        fetch_base,
        domain,
    }))
}

/// A hardware catalogue flag: the file must load, in the edge's currency, so
/// a typo stops the unit at start rather than at the first order.
fn hardware_catalogue(args: &[String], flag: &str, currency: &str) -> Result<Option<String>> {
    let Some(path) = arg(args, flag) else {
        return Ok(None);
    };
    let catalogue =
        crate::hardware::Catalogue::load(path).map_err(|e| miette!("bad {flag}: {e}"))?;
    if catalogue.currency != currency {
        return Err(miette!(
            "bad {flag}: the catalogue is in {}, the market in {currency}",
            catalogue.currency
        ));
    }
    Ok(Some(path.to_string()))
}

/// The route table for boxes behind local edges: `--relay-routes` turns it
/// on, on an edge that already routes custom domains.
fn parse_routes(args: &[String]) -> Result<Option<RoutesOpts>> {
    if !args.iter().any(|a| a == "--relay-routes") {
        return Ok(None);
    }
    if arg(args, "--dns-zone").is_none() {
        return Err(miette!(
            "--relay-routes needs --dns-zone: only an edge that routes custom domains keeps the route table"
        ));
    }
    let beside_registry = |name: &str| -> Result<String> {
        Ok(std::path::Path::new(req(args, "--registry")?)
            .with_file_name(name)
            .to_string_lossy()
            .into_owned())
    };
    let table_file = match arg(args, "--relay-routes-file") {
        Some(f) => f.to_string(),
        None => beside_registry("relay-routes.json")?,
    };
    let pass_key_file = match arg(args, "--relay-pass-key-file") {
        Some(f) => f.to_string(),
        None => beside_registry("relay-pass.key")?,
    };
    Ok(Some(RoutesOpts {
        table_file,
        pass_key_file,
    }))
}

/// The DNS half of `serve`: `--dns-zone` turns it on, and then the edge must
/// say which addresses its names resolve to.
fn parse_domains(args: &[String]) -> Result<Option<Box<DomainsOpts>>> {
    let Some(zone) = arg(args, "--dns-zone") else {
        return Ok(None);
    };
    let zone = zone.trim().trim_end_matches('.').to_ascii_lowercase();
    if zone.split('.').count() < 2 || !crate::relay::dns_name(&zone) {
        return Err(miette!(
            "bad --dns-zone {zone:?}; expected a domain such as boxes.losos.cfd"
        ));
    }
    let list = |flag: &str| -> Vec<String> {
        arg(args, flag)
            .unwrap_or("")
            .split(',')
            .map(|s| s.trim().trim_end_matches('.').to_ascii_lowercase())
            .filter(|s| !s.is_empty())
            .collect()
    };
    let ipv4 = list("--dns-ipv4")
        .iter()
        .map(|a| {
            a.parse()
                .map_err(|_| miette!("bad --dns-ipv4 address {a:?}"))
        })
        .collect::<Result<Vec<std::net::Ipv4Addr>>>()?;
    let ipv6 = list("--dns-ipv6")
        .iter()
        .map(|a| {
            a.parse()
                .map_err(|_| miette!("bad --dns-ipv6 address {a:?}"))
        })
        .collect::<Result<Vec<std::net::Ipv6Addr>>>()?;
    if ipv4.is_empty() && ipv6.is_empty() {
        return Err(miette!(
            "--dns-zone needs --dns-ipv4 and/or --dns-ipv6: the edge's public addresses, which every box name resolves to"
        ));
    }
    let mut nameservers = list("--dns-nameservers");
    if nameservers.is_empty() {
        nameservers.push(format!("ns1.{zone}"));
    }
    if let Some(bad) = nameservers.iter().find(|n| !crate::relay::dns_name(n)) {
        return Err(miette!("bad --dns-nameservers entry {bad:?}"));
    }
    let hostmaster = arg(args, "--dns-hostmaster").map_or_else(
        || format!("hostmaster.{zone}"),
        |h| h.trim().trim_end_matches('.').to_string(),
    );
    if !crate::relay::dns_name(&hostmaster) {
        return Err(miette!("bad --dns-hostmaster {hostmaster:?}; write the mailbox as a domain (hostmaster.example.org)"));
    }
    let doh_url = arg(args, "--dns-check-url").unwrap_or(DEFAULT_DOH_URL);
    if !(doh_url.starts_with("https://") || doh_url.starts_with("http://")) {
        return Err(miette!(
            "bad --dns-check-url {doh_url:?}; expected a DNS-over-HTTPS JSON endpoint"
        ));
    }
    Ok(Some(Box::new(DomainsOpts {
        state_file: arg(args, "--domains-state-file")
            .unwrap_or("/var/lib/losos-registrar/domains.json")
            .to_string(),
        zone_file: arg(args, "--dns-zone-file")
            .map_or_else(|| format!("/var/lib/losos-dns/{zone}.zone"), str::to_string),
        public_domain: arg(args, "--public-domain")
            .unwrap_or("")
            .trim()
            .trim_end_matches('.')
            .to_ascii_lowercase(),
        zone,
        nameservers,
        hostmaster,
        ipv4,
        ipv6,
        doh_url: doh_url.to_string(),
    })))
}

/// Accept an IANA zone name, rejecting anything the edge's `date` would
/// silently resolve to UTC.
///
/// Defaulting to `UTC` when the flag is absent matches what the edge did before
/// the zone travelled at all, so an un-upgraded caller keeps its old behaviour
/// instead of acquiring a new one.
fn parse_tz(v: &str) -> miette::Result<&str> {
    if crate::window::valid_tz(v) {
        Ok(v)
    } else {
        Err(miette!(
            "invalid --window-tz {v:?}; expected an IANA zone name such as Europe/Berlin or UTC"
        ))
    }
}

/// A non-negative load threshold.
///
/// Rejects negatives and non-finite values rather than clamping: both mean the
/// module generated something it did not intend, and a box that silently
/// decides it is never idle (or always idle) is worse than one that refuses to
/// start and says why.
fn parse_threshold(v: &str) -> miette::Result<f64> {
    match v.parse::<f64>() {
        Ok(n) if n.is_finite() && n >= 0.0 => Ok(n),
        _ => Err(miette!(
            "invalid --idle-load-threshold {v:?}; expected a non-negative number such as 0.25"
        )),
    }
}

/// Parse the Nix spelling of a boolean. `modules/cluster.nix` interpolates
/// `losos.cluster.shareCompute` straight into the unit's `ExecStart`, so the
/// only two values that can arrive are `true` and `false` — and anything else
/// means the module was edited into producing something it did not intend, so
/// it is a boot-time failure that names the flag rather than a silent `false`
/// that quietly stops the box contributing compute.
fn parse_bool(flag: &str, s: &str) -> Result<bool> {
    match s.trim() {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(miette!("bad {flag} {other:?}; expected true|false")),
    }
}

/// Validate an `HH:MM` compute-window bound, returning it unchanged.
///
/// The same rule lososd enforces on `losos.cluster.computeWindow.{start,end}`
/// and the same one the edge re-checks on the wire. Rejecting here means a
/// malformed window fails `losos-mesh-join.service` at boot with the flag
/// named, rather than travelling to the edge to come back as an opaque 400 on
/// a box with no shell to read it from.
fn parse_hhmm(s: &str) -> Result<&str> {
    let s = s.trim();
    if crate::window::valid_hhmm(s) {
        Ok(s)
    } else {
        Err(miette!("bad window {s:?}; expected HH:MM in 00:00-23:59"))
    }
}

/// Parse a `lo-hi` tunnel port range, inclusive on both ends.
///
/// `lo` of 0 and an inverted range are rejected here rather than surfacing
/// later: port 0 is not bindable, and `hi < lo` makes `alloc_port`'s
/// `(lo..=hi)` sweep empty, so the first appliance to register would be told
/// the range is exhausted with no hint that the *range itself* is malformed.
/// `serve --calls-relay-listen ADDR [--calls-relay-media-ports LO-HI]
/// [--calls-relay-address IP[=OFFERED]]...`.
fn parse_calls_relay(args: &[String]) -> Result<Option<crate::calls_relay::CallsRelayOpts>> {
    let Some(listen) = arg(args, "--calls-relay-listen") else {
        return Ok(None);
    };
    let listen = listen
        .parse()
        .map_err(|_| miette!("bad --calls-relay-listen {listen:?}; expected ip:port"))?;
    let media_ports = parse_range(arg(args, "--calls-relay-media-ports").unwrap_or("40000-40999"))?;
    let ip = |v: &str| {
        v.parse::<std::net::IpAddr>()
            .map_err(|_| miette!("bad --calls-relay-address {v:?}; expected IP or IP=OFFERED"))
    };
    let addresses = args_all(args, "--calls-relay-address")
        .into_iter()
        .map(|a| {
            let (bind, offered) = a.split_once('=').unwrap_or((a, a));
            Ok((ip(bind)?, ip(offered)?))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Some(crate::calls_relay::CallsRelayOpts {
        listen,
        media_ports,
        addresses,
    }))
}

fn parse_range(s: &str) -> Result<(u16, u16)> {
    let (lo, hi) = s
        .trim()
        .split_once('-')
        .ok_or_else(|| miette!("bad port range {s:?}; expected lo-hi"))?;
    let lo: u16 = lo.trim().parse().into_diagnostic()?;
    let hi: u16 = hi.trim().parse().into_diagnostic()?;
    if lo == 0 {
        return Err(miette!("bad port range {s:?}; port 0 is not bindable"));
    }
    if hi < lo {
        return Err(miette!(
            "bad port range {s:?}; high end is below the low end"
        ));
    }
    Ok((lo, hi))
}
