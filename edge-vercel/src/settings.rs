//! Configuration from the environment, materialised into the files the
//! registrar reads.
//!
//! The registrar authenticates against a `tenants.json` whose entries name a
//! *token file*, and `modules/edge.nix` generates that file from
//! `losos.edge.tenants`. A Vercel Function has neither NixOS nor `/var/secrets`,
//! but it does have environment variables and a writable scratch directory. So
//! [`Settings::materialise`] writes the whitelist and one 0600 token file per
//! tenant under the state directory and hands the registrar the paths — the
//! authentication code that runs is the production code, down to the
//! `token_fault` checks on each secret, and this module never compares a token
//! itself.
//!
//! `ServeOpts` is built by feeding an argument vector to
//! [`losos_registrar::opts::parse`], not by filling the struct in: the
//! duration, port-range and currency validation then has one implementation.

use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use losos_registrar::opts::{parse, Mode, ServeOpts};
use miette::{miette, IntoDiagnostic, Result, WrapErr};
use serde::{Deserialize, Serialize};

/// `LOSOS_TENANTS`: the operator whitelist, a JSON object keyed by appliance
/// id. Required; an edge with no tenants answers 401 to everyone.
pub const TENANTS_VAR: &str = "LOSOS_TENANTS";
/// `LOSOS_BOOTSTRAP_TOKEN`: rathole's `default_token`. It only reaches the
/// rendered `rathole.toml` on the scratch disk, which nothing on this host
/// serves or reads, so it may be left unset: a random value is used.
pub const BOOTSTRAP_VAR: &str = "LOSOS_BOOTSTRAP_TOKEN";
/// `LOSOS_HEARTBEAT_TTL`: how long after its last heartbeat a box is pruned.
/// The registrar's duration syntax (`120s`, `5m`); default `120s`.
pub const TTL_VAR: &str = "LOSOS_HEARTBEAT_TTL";
/// `LOSOS_PORT_RANGE`: the rathole port range (`lo-hi`) ports are allocated
/// from; default `50000-50100`. Cosmetic here — no rathole binds them — but
/// the registry carries a port per tenant and the range bounds it.
pub const PORT_RANGE_VAR: &str = "LOSOS_PORT_RANGE";
/// `LOSOS_STATE_DIR`: the scratch directory; default `/tmp/losos-edge`.
/// `/tmp` is the one writable path in a Vercel Function.
pub const STATE_DIR_VAR: &str = "LOSOS_STATE_DIR";
/// `LOSOS_STATE_KEY`: the Redis key the registry snapshot is kept under;
/// default `losos:edge:registry`.
pub const STATE_KEY_VAR: &str = "LOSOS_STATE_KEY";

/// The `(url, token)` variable pairs that name the Redis store, in the order
/// they are tried. The first pair is this crate's own, for pointing at any
/// Upstash-compatible endpoint; the other two are what Vercel's marketplace
/// writes into a project when an Upstash Redis store is connected to it, under
/// its current and its older naming.
pub const STORE_VARS: [(&str, &str); 3] = [
    ("LOSOS_REDIS_REST_URL", "LOSOS_REDIS_REST_TOKEN"),
    ("UPSTASH_REDIS_REST_URL", "UPSTASH_REDIS_REST_TOKEN"),
    ("KV_REST_API_URL", "KV_REST_API_TOKEN"),
];

const DEFAULT_TTL: &str = "120s";
const DEFAULT_PORT_RANGE: &str = "50000-50100";
const DEFAULT_STATE_DIR: &str = "/tmp/losos-edge";
const DEFAULT_STATE_KEY: &str = "losos:edge:registry";

/// One entry of `LOSOS_TENANTS`. `cluster` and `market` mirror
/// `losos.edge.tenants.<id>.{cluster,market}` so a whitelist can be copied
/// across verbatim; on this host both routes answer 503 whatever they say.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TenantSpec {
    pub hostname: String,
    pub token: String,
    #[serde(default)]
    pub cluster: bool,
    #[serde(default)]
    pub market: bool,
}

/// Where the registry lives between requests.
#[derive(Debug, Clone)]
pub struct StoreSettings {
    /// The Upstash REST endpoint, `https://<name>.upstash.io`.
    pub url: String,
    pub token: String,
    pub key: String,
}

/// Everything the host needs, resolved and validated, before any file is
/// written.
#[derive(Debug, Clone)]
pub struct Settings {
    pub tenants: BTreeMap<String, TenantSpec>,
    pub bootstrap_token: String,
    pub heartbeat_ttl: String,
    pub port_range: String,
    pub state_dir: PathBuf,
    /// `None` runs with in-memory state only.
    pub store: Option<StoreSettings>,
}

impl Settings {
    /// Read the environment. Fails on a missing or malformed whitelist and on a
    /// store URL without its token (or the reverse), so a half-configured
    /// project refuses to start rather than quietly running without a store.
    pub fn from_env() -> Result<Self> {
        let raw = std::env::var(TENANTS_VAR)
            .map_err(|_| miette!("{TENANTS_VAR} is not set; see edge-vercel/README.md"))?;
        let tenants: BTreeMap<String, TenantSpec> = serde_json::from_str(&raw)
            .into_diagnostic()
            .wrap_err_with(|| format!("{TENANTS_VAR} is not a JSON object of tenants"))?;
        let store = store_from_env()?;
        Ok(Self {
            tenants,
            bootstrap_token: std::env::var(BOOTSTRAP_VAR).unwrap_or_else(|_| random_hex(64)),
            heartbeat_ttl: std::env::var(TTL_VAR).unwrap_or_else(|_| DEFAULT_TTL.to_string()),
            port_range: std::env::var(PORT_RANGE_VAR)
                .unwrap_or_else(|_| DEFAULT_PORT_RANGE.to_string()),
            state_dir: std::env::var(STATE_DIR_VAR)
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from(DEFAULT_STATE_DIR)),
            store,
        })
    }

    /// Write the whitelist, the token files and the bootstrap token under the
    /// state directory, and build the registrar's `serve` options against them.
    ///
    /// Paths are laid out so nothing collides with what the registrar itself
    /// writes there (`registry.json`, `rathole.toml`, `traefik/losos.yml`,
    /// `compute-windows.json`). The listen address is a formality: the host
    /// never binds it, Vercel's bridge delivers requests.
    pub fn materialise(&self) -> Result<ServeOpts> {
        let dir = &self.state_dir;
        create_private_dir(dir)?;
        let tokens = dir.join("tokens");
        create_private_dir(&tokens)?;

        let mut whitelist = serde_json::Map::new();
        for (id, spec) in &self.tenants {
            // Ids become file names. The registrar's own id rules (a DNS label
            // for the mesh, a TOML key for rathole) are stricter than this, but
            // a path separator here would write outside the token directory.
            if id.is_empty() || id.contains('/') || id.contains('\0') || id == "." || id == ".." {
                return Err(miette!("tenant id {id:?} cannot be a file name"));
            }
            let token_file = tokens.join(id);
            write_private_file(&token_file, token_bytes(&spec.token))?;
            whitelist.insert(
                id.clone(),
                serde_json::json!({
                    "hostname": spec.hostname,
                    "token_file": token_file,
                    "cluster": spec.cluster,
                    "market": spec.market,
                }),
            );
        }
        let tenants_file = dir.join("tenants.json");
        write_private_file(
            &tenants_file,
            serde_json::to_string_pretty(&serde_json::Value::Object(whitelist))
                .into_diagnostic()?
                .as_bytes(),
        )?;
        let bootstrap_file = dir.join("bootstrap.token");
        write_private_file(&bootstrap_file, self.bootstrap_token.as_bytes())?;

        let path = |name: &str| dir.join(name).to_string_lossy().into_owned();
        let argv: Vec<String> = [
            "serve",
            "--listen",
            "127.0.0.1:0",
            "--registry",
            &path("registry.json"),
            "--traefik-dir",
            &path("traefik"),
            "--rathole-config",
            &path("rathole.toml"),
            "--port-range",
            &self.port_range,
            "--bootstrap-token-file",
            &bootstrap_file.to_string_lossy(),
            "--tenants-file",
            &tenants_file.to_string_lossy(),
            "--heartbeat-ttl",
            &self.heartbeat_ttl,
            "--compute-windows-file",
            &path("compute-windows.json"),
        ]
        .into_iter()
        .map(str::to_string)
        .collect();
        match parse(argv)? {
            Mode::Serve(opts) => Ok(opts),
            _ => Err(miette!("parse(\"serve\", ..) did not yield serve options")),
        }
    }
}

/// The token bytes to write. Trimmed, because the registrar trims what it
/// reads and a trailing newline pasted into a dashboard field would otherwise
/// be part of nothing.
fn token_bytes(token: &str) -> &[u8] {
    token.trim().as_bytes()
}

/// The first configured `(url, token)` pair of [`STORE_VARS`], with the key.
fn store_from_env() -> Result<Option<StoreSettings>> {
    for (url_var, token_var) in STORE_VARS {
        let url = std::env::var(url_var).ok().filter(|v| !v.trim().is_empty());
        let token = std::env::var(token_var)
            .ok()
            .filter(|v| !v.trim().is_empty());
        match (url, token) {
            (None, None) => continue,
            (Some(url), Some(token)) => {
                if !url.starts_with("https://") {
                    return Err(miette!(
                        "{url_var} must be an https:// URL; the store token would otherwise cross in cleartext"
                    ));
                }
                return Ok(Some(StoreSettings {
                    url: url.trim_end_matches('/').to_string(),
                    token: token.trim().to_string(),
                    key: std::env::var(STATE_KEY_VAR)
                        .unwrap_or_else(|_| DEFAULT_STATE_KEY.to_string()),
                }));
            }
            _ => return Err(miette!("{url_var} and {token_var} must be set together")),
        }
    }
    Ok(None)
}

fn create_private_dir(path: &Path) -> Result<()> {
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)
        .into_diagnostic()
        .wrap_err_with(|| format!("create {}", path.display()))
}

/// Create-or-truncate `path` at 0600 and write `bytes`. The directory is
/// private to this process already; the mode is belt and braces, and it is
/// what the registrar documents for a token file.
fn write_private_file(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut f = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .into_diagnostic()
        .wrap_err_with(|| format!("open {}", path.display()))?;
    f.write_all(bytes)
        .into_diagnostic()
        .wrap_err_with(|| format!("write {}", path.display()))
}

/// `n` lowercase hex characters from the system CSPRNG.
fn random_hex(n: usize) -> String {
    use ring::rand::{SecureRandom, SystemRandom};
    let mut bytes = vec![0u8; n.div_ceil(2)];
    // `fill` fails only if the OS entropy source does; there is no sensible
    // fallback for a token, and this one is never checked by anyone, so a
    // constant on failure is the honest, loud-in-the-log choice.
    if SystemRandom::new().fill(&mut bytes).is_err() {
        tracing::error!("system CSPRNG unavailable; using a fixed placeholder bootstrap token");
        return "0".repeat(n);
    }
    let mut out = String::with_capacity(n);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out.truncate(n);
    out
}
