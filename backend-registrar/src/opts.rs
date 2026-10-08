//! Hand-rolled argument parsing. Kept dependency-free (no clap) because the
//! surface is small and fixed; every flag has a NixOS-module default so the
//! binary is only ever invoked with the values the module generates.

use std::time::Duration;

use miette::{miette, IntoDiagnostic, Result};

use crate::market::{MarketOpts, DEFAULT_FEE_BPS, MAX_FEE_BPS, SUPPORTED_CURRENCIES};
use crate::provision::{EdgeSpec, ProvisionGithub, GITHUB_API_URL, GITHUB_OAUTH_URL};
use crate::stripe_gate::GateOpts;

fn arg<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1).map(std::string::String::as_str))
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

/// `serve` options. The registrar is the sole writer of `traefik_dir`'s
/// `losos.yml` and `rathole_config`; rathole hot-reloads the latter via its
/// `notify` file-watcher (no signal needed), so there is no rathole-service
/// handle here.
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
    pub traefik_dir: String,
    pub rathole_config: String,
    pub rathole_bind_addr: String,
    pub rathole_bind_port: u16,
    pub port_range: (u16, u16),
    pub bootstrap_token_file: String,
    /// 0600 file holding the rathole Noise private key; `None` = plain TCP.
    pub noise_private_key_file: Option<String>,
    /// The matching public key, served verbatim at `GET /noise-public-key` so
    /// appliances can pin it on first start. Public by nature; `None` = 404.
    pub noise_public_key_file: Option<String>,
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
    /// The Stripe Connect market. `None` — no `--market-gate-socket` — and
    /// every `/market/*` route answers 503; the rest of the API is unchanged.
    pub market: Option<Box<MarketOpts>>,
    /// This edge's identity (`crate::identity`): the 0600 key file (made on
    /// the first start if missing) and the certificate the LosOS root signed
    /// for it (installed by `POST /identity/cert`). Both or neither; with
    /// neither, every `/identity*` route answers 404 and boxes treat the
    /// edge as a company edge (sharing, no trading).
    pub identity_key_file: Option<String>,
    pub identity_cert_file: Option<String>,
    /// Where `POST /identity/cert` asks who a GitHub token belongs to.
    /// `--github-api-url`; the default is api.github.com, the override is
    /// for the tests' fake GitHub.
    pub github_api_url: String,
}

/// `identity` options: the offline key ceremony (`crate::identity`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentityOpts {
    /// `identity keygen --out FILE`: a new Ed25519 key, written 0600 to FILE;
    /// the public key is printed. Used for the root key and for each edge.
    Keygen { out: String },
    /// `identity sign --root-key FILE --public-key HEX --name N --url U
    /// --days D`: the root issues an edge certificate, printed as JSON.
    Sign {
        root_key: String,
        public_key: String,
        name: String,
        url: String,
        days: u64,
    },
    /// `identity show --key FILE`: print the public key of a key file.
    Show { key: String },
    /// `identity verify --root-public HEX --url U --nonce HEX --answer FILE`:
    /// run the box's four checks over a `GET /identity?nonce=` answer saved
    /// to FILE (`-` for stdin). Exit 0 and name the edge, or exit 1 with the
    /// reason. What provisioning runs after it deployed an identity.
    Verify {
        root_public: String,
        url: String,
        nonce: String,
        answer: String,
    },
}

/// `seed` options. Writes the declarative rathole `[server]` base (for zero
/// tenants) so rathole can start before `serve`'s reconciler has run once.
/// Reuses [`crate::desired_config`] — the same function `serve`'s reconciler
/// calls — so the seed and the steady-state output can never drift.
#[derive(Debug, Clone)]
pub struct SeedOpts {
    pub rathole_config: String,
    pub rathole_bind_addr: String,
    pub rathole_bind_port: u16,
    pub bootstrap_token_file: String,
    /// 0600 file holding the rathole Noise private key; `None` = plain TCP.
    pub noise_private_key_file: Option<String>,
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
    /// `provision root-keygen --out FILE [--publish] [--repo R] [--base B]`:
    /// the root key, 0600, never overwritten; with `--publish`, the pull
    /// request that ships the public half (asks for `public_repo`).
    RootKeygen {
        github: ProvisionGithub,
        out: String,
        publish: bool,
        repo: String,
        base: String,
    },
    /// `provision publish (--root-key FILE | --root-public HEX) [--repo R]
    /// [--base B]`: the pull request alone.
    Publish {
        github: ProvisionGithub,
        root_key: Option<String>,
        root_public: Option<String>,
        repo: String,
        base: String,
    },
    /// `provision edge --name N --url U --root-key FILE [--days D]`: read
    /// the edge's public key from `GET <U>/identity/public-key`, sign its
    /// certificate, push it to `POST <U>/identity/cert` with the GitHub
    /// token of the sign-in, then the four checks.
    Edge {
        github: ProvisionGithub,
        root_key: String,
        spec: EdgeSpec,
    },
    /// `provision verify --url U (--root-key FILE | --root-public HEX)`: the
    /// four checks alone. No sign-in: anyone may ask an edge who it is.
    Verify {
        url: String,
        root_key: Option<String>,
        root_public: Option<String>,
    },
}

/// The default repository the public key is published to.
pub const PUBLISH_REPO: &str = "dasmatus/losos";

// Parsed once at startup and matched once; the size spread between the
// serve options and the one-shot subcommands costs nothing worth a Box.
#[allow(clippy::large_enum_variant)]
pub enum Mode {
    Serve(ServeOpts),
    Announce(AnnounceOpts),
    Seed(SeedOpts),
    Join(JoinOpts),
    StripeGate(GateOpts),
    Identity(IdentityOpts),
    Provision(ProvisionOpts),
}

pub fn parse(args: Vec<String>) -> Result<Mode> {
    if args.is_empty() {
        return Err(miette!(
            "usage: losos-registrar serve|announce|seed|join|stripe-gate|identity|provision ..."
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
                traefik_dir: req(&rest, "--traefik-dir")?.to_string(),
                rathole_config: req(&rest, "--rathole-config")?.to_string(),
                rathole_bind_addr: arg(&rest, "--rathole-bind-addr")
                    .unwrap_or("0.0.0.0")
                    .to_string(),
                rathole_bind_port: arg(&rest, "--rathole-bind-port")
                    .unwrap_or("2333")
                    .parse()
                    .map_err(|_| miette!("bad --rathole-bind-port"))?,
                port_range: (lo, hi),
                bootstrap_token_file: req(&rest, "--bootstrap-token-file")?.to_string(),
                noise_private_key_file: arg(&rest, "--noise-private-key-file").map(str::to_string),
                noise_public_key_file: arg(&rest, "--noise-public-key-file").map(str::to_string),
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
                identity_key_file: arg(&rest, "--identity-key-file").map(str::to_string),
                identity_cert_file: arg(&rest, "--identity-cert-file").map(str::to_string),
                github_api_url: arg(&rest, "--github-api-url")
                    .unwrap_or(GITHUB_API_URL)
                    .trim_end_matches('/')
                    .to_string(),
            }))
        }
        "identity" => {
            let verb = rest.first().map(String::as_str).unwrap_or("");
            let rest: Vec<String> = rest.iter().skip(1).cloned().collect();
            match verb {
                "keygen" => Ok(Mode::Identity(IdentityOpts::Keygen {
                    out: req(&rest, "--out")?.to_string(),
                })),
                "sign" => Ok(Mode::Identity(IdentityOpts::Sign {
                    root_key: req(&rest, "--root-key")?.to_string(),
                    public_key: req(&rest, "--public-key")?.to_string(),
                    name: req(&rest, "--name")?.to_string(),
                    url: req(&rest, "--url")?.to_string(),
                    days: arg(&rest, "--days")
                        .unwrap_or("365")
                        .parse()
                        .ok()
                        .filter(|d| (1..=3650).contains(d))
                        .ok_or_else(|| miette!("bad --days; expected 1..=3650"))?,
                })),
                "show" => Ok(Mode::Identity(IdentityOpts::Show {
                    key: req(&rest, "--key")?.to_string(),
                })),
                "verify" => Ok(Mode::Identity(IdentityOpts::Verify {
                    root_public: req(&rest, "--root-public")?.to_string(),
                    url: req(&rest, "--url")?.to_string(),
                    nonce: req(&rest, "--nonce")?.to_string(),
                    answer: req(&rest, "--answer")?.to_string(),
                })),
                _ => Err(miette!(
                    "usage: losos-registrar identity keygen|sign|show|verify ..."
                )),
            }
        }
        "provision" => parse_provision(&rest),
        "announce" => Ok(Mode::Announce(AnnounceOpts {
            registrar_url: req(&rest, "--registrar-url")?.to_string(),
            appliance_id: req(&rest, "--appliance-id")?.to_string(),
            hostname: req(&rest, "--hostname")?.to_string(),
            token_file: req(&rest, "--token-file")?.to_string(),
            heartbeat_interval: parse_dur(arg(&rest, "--heartbeat-interval").unwrap_or("30s"))?,
            idle_load_threshold: parse_threshold(
                arg(&rest, "--idle-load-threshold").unwrap_or("0.25"),
            )?,
        })),
        "seed" => Ok(Mode::Seed(SeedOpts {
            rathole_config: req(&rest, "--rathole-config")?.to_string(),
            rathole_bind_addr: arg(&rest, "--rathole-bind-addr")
                .unwrap_or("0.0.0.0")
                .to_string(),
            rathole_bind_port: arg(&rest, "--rathole-bind-port")
                .unwrap_or("2333")
                .parse()
                .map_err(|_| miette!("bad --rathole-bind-port"))?,
            bootstrap_token_file: req(&rest, "--bootstrap-token-file")?.to_string(),
            noise_private_key_file: arg(&rest, "--noise-private-key-file").map(str::to_string),
        })),
        "join" => Ok(Mode::Join(JoinOpts {
            registrar_url: req(&rest, "--registrar-url")?.to_string(),
            appliance_id: req(&rest, "--appliance-id")?.to_string(),
            node_name: req(&rest, "--node-name")?.to_string(),
            token_file: req(&rest, "--token-file")?.to_string(),
            out_token_file: req(&rest, "--out-token-file")?.to_string(),
            share_compute: parse_bool(arg(&rest, "--share-compute").unwrap_or("false"))?,
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
            }))
        }
        other => Err(miette!(
            "unknown subcommand {other:?}; expected serve|announce|seed|join|stripe-gate"
        )),
    }
}

fn valid_currency(c: &str) -> bool {
    SUPPORTED_CURRENCIES.contains(&c)
}

/// The `--market-*` flags. Enabled by `--market-gate-socket`, the Unix socket
/// of the Stripe gate (`stripe-gate`), which alone holds the key and the
/// webhook secrets; the return URL is then required, because a market that can
/// take payment but has nowhere to send the buyer back to would be half-made.
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
    let repo = arg(&rest, "--repo").unwrap_or(PUBLISH_REPO).to_string();
    let base = arg(&rest, "--base").unwrap_or("main").to_string();
    let root_pair = |rest: &[String]| -> Result<(Option<String>, Option<String>)> {
        let key = arg(rest, "--root-key").map(str::to_string);
        let public = arg(rest, "--root-public").map(str::to_string);
        if key.is_none() && public.is_none() {
            return Err(miette!("need --root-key FILE or --root-public HEX"));
        }
        Ok((key, public))
    };
    let mode = match verb {
        "whoami" => ProvisionOpts::Whoami { github },
        "root-keygen" => ProvisionOpts::RootKeygen {
            github,
            out: req(&rest, "--out")?.to_string(),
            publish: rest.iter().any(|a| a == "--publish"),
            repo,
            base,
        },
        "publish" => {
            let (root_key, root_public) = root_pair(&rest)?;
            ProvisionOpts::Publish {
                github,
                root_key,
                root_public,
                repo,
                base,
            }
        }
        "edge" => ProvisionOpts::Edge {
            github,
            root_key: req(&rest, "--root-key")?.to_string(),
            spec: EdgeSpec {
                name: req(&rest, "--name")?.to_string(),
                url: req(&rest, "--url")?.to_string(),
                days: arg(&rest, "--days")
                    .unwrap_or("365")
                    .parse()
                    .ok()
                    .filter(|d| (1..=3650).contains(d))
                    .ok_or_else(|| miette!("bad --days; expected 1..=3650"))?,
            },
        },
        "verify" => {
            let (root_key, root_public) = root_pair(&rest)?;
            ProvisionOpts::Verify {
                url: req(&rest, "--url")?.to_string(),
                root_key,
                root_public,
            }
        }
        _ => {
            return Err(miette!(
                "usage: losos-registrar provision whoami|root-keygen|publish|edge|verify ..."
            ))
        }
    };
    Ok(Mode::Provision(mode))
}

fn parse_market(args: &[String]) -> Result<Option<Box<MarketOpts>>> {
    let Some(gate_socket) = arg(args, "--market-gate-socket") else {
        return Ok(None);
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
    if !dns_subdomain(storage_class) {
        return Err(miette!(
            "bad --market-storage-class {storage_class:?}; expected a Kubernetes object name (DNS-1123 subdomain)"
        ));
    }
    let return_url = req(args, "--market-return-url")?;
    if !crate::stripe_gate::url_ok(return_url) {
        return Err(miette!(
            "--market-return-url must be an absolute http(s) URL with a host, no credentials and no #fragment"
        ));
    }
    Ok(Some(Box::new(MarketOpts {
        state_file: arg(args, "--market-state-file")
            .unwrap_or("/var/lib/losos-registrar/market.json")
            .to_string(),
        gate_socket: gate_socket.to_string(),
        return_url: return_url.to_string(),
        currency: currency.to_string(),
        fee_bps,
        storage_class: storage_class.to_string(),
    })))
}

/// A Kubernetes object name in the DNS-1123 subdomain form, which is what a
/// `StorageClass` name must be: at most 253 characters of dot-separated
/// labels, each 1 to 63 characters of lowercase alphanumerics with inner
/// hyphens. Anything else would
/// only fail later, as a claim the apiserver refuses on every reconcile pass.
fn dns_subdomain(name: &str) -> bool {
    name.len() <= 253
        && name.split('.').all(|label| {
            (1..=63).contains(&label.len())
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
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
fn parse_bool(s: &str) -> Result<bool> {
    match s.trim() {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(miette!(
            "bad --share-compute {other:?}; expected true|false"
        )),
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

/// Parse a `lo-hi` rathole port range, inclusive on both ends.
///
/// `lo` of 0 and an inverted range are rejected here rather than surfacing
/// later: port 0 is not bindable, and `hi < lo` makes `alloc_port`'s
/// `(lo..=hi)` sweep empty, so the first appliance to register would be told
/// the range is exhausted with no hint that the *range itself* is malformed.
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
