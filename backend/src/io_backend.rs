//! The real [`Losos`] implementation: files, environment, `systemd-run`.
//!
//! Every path is environment-overridable so the CLI can be driven against a
//! throwaway directory in tests without touching `/etc` or `/var`.
//!
//! Every file this module owns is written through [`atomic_write`] — a unique
//! temp file, fsynced, then renamed over the target. `state.json` and
//! `overrides.nix` both take that route: the appliance has no shell, so a
//! config truncated by a crash or by two concurrent writers would fail every
//! later rebuild with nobody able to log in and repair it.

use crate::losos::Losos;
use crate::model::State;
use crate::overrides::DEFAULT_OVERRIDES_NIX;
use crate::supervisor;
use anyhow::Context;
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

/// Where everything lives. Resolved once from the environment.
#[derive(Debug, Clone)]
pub struct Paths {
    /// `$LOSOS_STATE_DIR`, holding `state.json` and `rebuild.log`.
    pub state_dir: PathBuf,
    /// `$LOSOS_OVERRIDES` — replaced by `apply` / `factory-reset`, line-patched
    /// by `change --mode`. The only Nix file the control plane writes.
    pub overrides_file: PathBuf,
    /// `$LOSOS_FLAKE`, the flake reference rebuilds are made from.
    pub flake_ref: String,
    /// `$LOSOS_CONFIG_DIR`, the git repository the flake lives in
    /// (`crate::config_repo`). `overrides_file` is normally inside it.
    pub config_dir: PathBuf,
    /// `$LOSOS_OPTIONS_FILE`, the option document (`crate::options`).
    pub options_file: PathBuf,
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

impl Paths {
    /// Resolve from the environment, falling back to the appliance defaults.
    pub fn from_env() -> Self {
        Paths {
            state_dir: PathBuf::from(env_or("LOSOS_STATE_DIR", "/var/lib/losos")),
            overrides_file: PathBuf::from(env_or(
                "LOSOS_OVERRIDES",
                "/etc/nixos/modules/overrides.nix",
            )),
            flake_ref: env_or("LOSOS_FLAKE", "/etc/nixos#install"),
            config_dir: PathBuf::from(env_or(
                "LOSOS_CONFIG_DIR",
                crate::config_repo::DEFAULT_CONFIG_DIR,
            )),
            options_file: PathBuf::from(env_or(
                "LOSOS_OPTIONS_FILE",
                crate::options::DEFAULT_OPTIONS_FILE,
            )),
        }
    }

    pub fn state_file(&self) -> PathBuf {
        self.state_dir.join("state.json")
    }

    pub fn rebuild_log(&self) -> PathBuf {
        self.state_dir.join("rebuild.log")
    }

    /// The owner's look (`crate::look`), beside the state.
    pub fn look_file(&self) -> PathBuf {
        self.state_dir.join("look.json")
    }

    /// The uploaded background picture, raw; its type is in the look.
    pub fn background_file(&self) -> PathBuf {
        self.state_dir.join("background.img")
    }

    /// How the last configuration sync went (`crate::config_repo`).
    pub fn sync_report_file(&self) -> PathBuf {
        self.state_dir.join("config-sync.json")
    }

    /// The git credential store for LosOS Git, written from the bot token
    /// before each fetch or push. 0600, in the daemon's 0700 state directory.
    pub fn git_credentials_file(&self) -> PathBuf {
        self.state_dir.join("git-credentials")
    }

    /// The `Authorization` header curl reads for LosOS Git's API, written
    /// from the bot token before each request so it is in no argv.
    pub fn forgejo_header_file(&self) -> PathBuf {
        self.state_dir.join("forgejo-auth")
    }
}

/// Distinguishes the temp files of concurrent writers within one process.
static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);
/// Distinguishes rebuild jobs queued within the same second. Process-lifetime,
/// so it survives every clone of [`IoLosos`].
static JOB_SEQ: AtomicU64 = AtomicU64::new(0);

/// A temp path nobody else will pick: the pid separates processes, the counter
/// separates threads within one.
///
/// The old fixed `<name>.tmp` was the bug — two writers truncated and filled
/// the *same* temp file, then renamed it in turn, so the survivor could be a
/// blend of both payloads.
fn temp_path(path: &Path) -> PathBuf {
    let seq = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "losos".to_string());
    path.with_file_name(format!(".{name}.{}.{seq}.tmp", std::process::id()))
}

/// Fill a fresh temp file and flush it to the disk itself.
///
/// `create_new` is what keeps `mode` honest: the file carries its permissions
/// from the moment it exists, instead of being created wide and narrowed after.
///
/// `owner`, when set, is applied through the open descriptor (`fchown`) before
/// the file is renamed into place. A path-based `chown` after the rename
/// follows symlinks, and in a directory someone else can write to that is a
/// way to make root hand them any file on the box.
fn fill_temp(tmp: &Path, content: &[u8], mode: u32, owner: Option<u32>) -> std::io::Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(tmp)?;
    if let Some(uid) = owner {
        std::os::unix::fs::fchown(&file, Some(uid), Some(uid))?;
    }
    file.write_all(content)?;
    // Without this the rename can land before the bytes do, and a power cut
    // between the two leaves a correctly named, empty config.
    file.sync_all()
}

/// Write `content` to `path` atomically, with `mode` on the resulting file and
/// `dir_mode` on any parent directory this call has to create.
fn write_atomically(
    path: &Path,
    content: &[u8],
    mode: u32,
    dir_mode: u32,
    owner: Option<u32>,
) -> anyhow::Result<()> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(dir_mode)
            .create(dir)
            .with_context(|| format!("creating {}", dir.display()))?;
    }

    let tmp = temp_path(path);
    if let Err(e) = fill_temp(&tmp, content, mode, owner) {
        let _ = std::fs::remove_file(&tmp);
        return Err(anyhow::Error::new(e).context(format!("writing {}", tmp.display())));
    }
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(anyhow::Error::new(e).context(format!(
            "renaming {} to {}",
            tmp.display(),
            path.display()
        )));
    }

    // The rename is only durable once the directory entry is on disk too. A
    // failure here means the new content is live but might not survive a power
    // cut — worth a line in the journal, not worth failing a completed write.
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        match std::fs::File::open(dir).and_then(|d| d.sync_all()) {
            Ok(()) => {}
            Err(e) => {
                tracing::debug!(dir = %dir.display(), error = %e, "could not fsync directory")
            }
        }
    }
    Ok(())
}

/// Write `content` to `path` atomically: a unique temp file, fsynced, then
/// renamed over the target.
///
/// The rename is atomic on POSIX, so a reader either sees the whole old file or
/// the whole new one — never a truncated config, and never a blend of two
/// concurrent writers.
pub fn atomic_write(path: &Path, content: &[u8]) -> anyhow::Result<()> {
    write_atomically(path, content, 0o644, 0o755, None)
}

/// [`atomic_write`] for a secret: mode 0600 from creation, in a 0700 directory.
///
/// Used for the admin token, which is equivalent to root on this appliance.
/// Creating it world-readable and chmodding afterwards leaves a window in which
/// any local process can read it, and a token that was ever readable is a token
/// to rotate.
pub fn atomic_write_secret(path: &Path, content: &[u8]) -> anyhow::Result<()> {
    write_atomically(path, content, 0o600, 0o700, None)
}

/// [`atomic_write_secret`] for a file that must belong to `uid` (owner and
/// group), owned from before it appears under its real name.
///
/// For the staged Nextcloud password, which lands in a directory the pod's
/// uid can write to. The old sequence, write as root then `chown(path)`, let
/// that uid swap the freshly renamed file for a symlink to the admin token in
/// the gap and have root chown the token over to it.
pub fn atomic_write_secret_owned(path: &Path, content: &[u8], uid: u32) -> anyhow::Result<()> {
    write_atomically(path, content, 0o600, 0o700, Some(uid))
}

/// The real [`crate::recovery::CodeStore`]: one 0600 file plus `/dev/urandom`.
///
/// Separate from [`IoLosos`] rather than folded into it because the recovery
/// code is the one secret here that must outlive a factory reset, so its path
/// comes from the unit's environment (`LOSOS_RECOVERY_FILE`) and not from
/// [`Paths`], whose entries all live under directories a reset clears.
#[derive(Debug, Clone)]
pub struct FileCodeStore {
    path: PathBuf,
}

impl FileCodeStore {
    /// The path `modules/recovery.nix` puts in the unit's environment, falling
    /// back to [`crate::recovery::DEFAULT_RECOVERY_FILE`].
    pub fn from_env() -> Self {
        Self {
            path: crate::recovery::code_file(),
        }
    }
}

impl Default for FileCodeStore {
    fn default() -> Self {
        Self::from_env()
    }
}

/// The real [`crate::catalogue::Fetch`]: one `curl` per search.
///
/// A subprocess rather than a client crate, for the reasons in the header of
/// `catalogue.rs` — chiefly that `flake/packages.nix` pins `cargoHash`, so a
/// new dependency needs a hash only a `nix build` can produce.
///
/// The flags are the interesting part, and each one closes something:
///
///   * `--proto =https` and `--proto-redir =https` — this may speak nothing
///     but HTTPS, and a redirect may not downgrade it. `search_url` only ever
///     builds an `https://` URL, so today these are belt and braces; they are
///     here so that stays true if the endpoint is ever made configurable.
///   * `--fail` — a non-2xx exits non-zero instead of handing back an error
///     page for `parse_results` to reject less legibly.
///   * `--max-time` — bounds how long "no network" takes to say so. Without it
///     a black-holed route holds the request until the browser gives up.
///   * `--max-filesize` — a catalogue that answers with something enormous is
///     not going to be rendered; refuse it rather than buffer it.
///   * no `--insecure`, ever. The appliance's own certificate is self-signed
///     (`modules/tls.nix`), which tempts that flag; this is an outbound call to
///     a public host and the system trust store is correct for it.
///
/// Nothing is passed through a shell: the argument vector goes to `execve`, so
/// the query needs no quoting to be safe. [`crate::catalogue::validate_query`]
/// still refuses control characters, so the property does not rest on that.
pub struct CurlFetch;

impl crate::catalogue::Fetch for CurlFetch {
    fn get(&mut self, url: &str) -> anyhow::Result<String> {
        /// Bytes of response body accepted. The screen renders twenty rows.
        const MAX_BYTES: usize = 2 * 1024 * 1024;

        let out = std::process::Command::new("curl")
            .args(["--silent", "--show-error", "--fail", "--location"])
            .args(["--proto", "=https", "--proto-redir", "=https"])
            .args(["--max-time", &crate::catalogue::TIMEOUT_SECS.to_string()])
            .args(["--max-filesize", &MAX_BYTES.to_string()])
            .args(["--header", "Accept: application/json"])
            .arg(url)
            .output()
            // `curl` comes from the unit's `path` in modules/daemon.nix, which
            // *replaces* PATH. Name that here: the bare ENOENT names only
            // "curl" and sends the reader looking in the wrong file.
            .context("running curl (is it on lososd's unit path? see modules/daemon.nix)")?;

        if !out.status.success() {
            // curl's own diagnosis is the useful half and it goes to the
            // journal. The caller turns this into "the search did not come
            // back", because an HTTP client's stderr is not something to put
            // in front of the owner.
            let why = String::from_utf8_lossy(&out.stderr);
            anyhow::bail!("the catalogue search failed: {}", why.trim());
        }
        if out.stdout.len() > MAX_BYTES {
            // --max-filesize only acts on a declared Content-Length, so a
            // chunked response can still overrun it.
            anyhow::bail!("the catalogue returned more than {MAX_BYTES} bytes");
        }
        String::from_utf8(out.stdout).context("the catalogue returned a body that is not UTF-8")
    }
}

/// Browse the LAN for `_losos-edge._tcp` once.
///
/// `None` when `avahi-browse` could not run at all (not on the unit path,
/// Avahi down, the browse hung past its budget): the caller reports the LAN
/// as unsearched rather than empty. `--terminate` makes the browse return
/// once the cache has settled; `timeout` is the ceiling for an Avahi that
/// never settles. Both binaries come from the unit's `path` in
/// modules/daemon.nix.
fn browse_lan() -> Option<String> {
    let out = std::process::Command::new("timeout")
        .arg(crate::edge::BROWSE_TIMEOUT_SECS.to_string())
        .args([
            "avahi-browse",
            "--parsable",
            "--resolve",
            "--terminate",
            crate::edge::SERVICE_TYPE,
        ])
        .stdin(std::process::Stdio::null())
        .output();
    match out {
        Ok(out) if out.status.success() => Some(String::from_utf8_lossy(&out.stdout).into_owned()),
        Ok(out) => {
            tracing::info!(
                status = ?out.status.code(),
                stderr = %String::from_utf8_lossy(&out.stderr).trim(),
                "avahi-browse did not complete; the LAN was not searched"
            );
            None
        }
        Err(e) => {
            tracing::info!(error = %e, "avahi-browse could not run (is it on lososd's unit path? see modules/daemon.nix)");
            None
        }
    }
}

/// Whether `url` is a live edge registrar: its `/health` answers 2xx inside
/// the probe budget. Plain curl, no `--proto` pin: a LAN edge is reached
/// over http by design (there is no CA for `.local`), and the probe carries
/// nothing but the request line.
fn edge_answers(url: &str) -> bool {
    let out = std::process::Command::new("curl")
        .args([
            "--silent",
            "--show-error",
            "--fail",
            "--output",
            "/dev/null",
        ])
        .args(["--max-time", &crate::edge::PROBE_TIMEOUT_SECS.to_string()])
        .args(["--max-redirs", "0"])
        .arg(format!("{url}/health"))
        .stdin(std::process::Stdio::null())
        .output();
    match out {
        Ok(out) => out.status.success(),
        Err(e) => {
            tracing::info!(error = %e, "curl could not run (is it on lososd's unit path? see modules/daemon.nix)");
            false
        }
    }
}

/// `GET {url}/identity?nonce={nonce}`: the body when the edge has an
/// identity to show, `None` on 404 (a company edge) or any failure. Same
/// curl discipline as the health probe; the body is bounded by curl's
/// `--max-filesize` so a hostile edge cannot feed the parser a gigabyte.
fn edge_identity(url: &str, nonce: &str) -> Option<String> {
    let out = std::process::Command::new("curl")
        .args(["--silent", "--show-error", "--fail"])
        .args(["--max-time", &crate::edge::PROBE_TIMEOUT_SECS.to_string()])
        .args(["--max-redirs", "0"])
        .args(["--max-filesize", "65536"])
        .arg(format!("{url}/identity?nonce={nonce}"))
        .stdin(std::process::Stdio::null())
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The LosOS root public key this box trusts, from the file
/// `$LOSOS_EDGE_ROOT_KEY_FILE` names (`losos.proxy.officialRootKeyFile`).
/// An absent or empty file means no edge can be official: fail closed.
fn root_public_key() -> Option<String> {
    let path = std::env::var("LOSOS_EDGE_ROOT_KEY_FILE").ok()?;
    let text = std::fs::read_to_string(path).ok()?;
    crate::edge::parse_root_key_file(&text)
}

/// A fresh 32-byte nonce, hex, for one scan's challenges.
fn fresh_nonce() -> String {
    use ring::rand::SecureRandom;
    let mut bytes = [0u8; 32];
    if ring::rand::SystemRandom::new().fill(&mut bytes).is_err() {
        // No randomness means no challenge worth trusting: an all-zero nonce
        // is still a valid request, and a replayed answer to it would only
        // ever mark an edge official that the root did sign.
        tracing::warn!("the system random source failed; this scan's nonce is not fresh");
    }
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// One complete scan: the LAN browse, the configured URL from
/// `$LOSOS_EDGE_URL`, a probe of every candidate, and the identity
/// challenge for the ones that answered.
fn scan_edge_now() -> crate::edge::EdgeStatus {
    let configured = std::env::var("LOSOS_EDGE_URL")
        .ok()
        .filter(|v| !v.trim().is_empty());
    let configured_rathole = std::env::var("LOSOS_EDGE_RATHOLE")
        .ok()
        .filter(|v| !v.trim().is_empty());
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let root = root_public_key();
    let nonce = fresh_nonce();
    let trust = crate::edge::Trust {
        root_public: root.as_deref(),
        nonce: &nonce,
    };
    crate::edge::assemble_with(
        browse_lan().as_deref(),
        configured.as_deref(),
        configured_rathole.as_deref(),
        edge_answers,
        &trust,
        edge_identity,
        now,
    )
}

/// Tell the tunnel units which edge the scan chose (modules/proxy.nix reads
/// `$LOSOS_EDGE_PATH_FILE`), and start, restart or stop them on a change.
///
/// Only when `LOSOS_EDGE_PATH_FILE` is set, which modules/daemon.nix does
/// with the master proxy on: without it there are no units to drive. The
/// decision is [`crate::edge::drive`], from what the previous scan left on
/// disk; both files are on /run, so a boot starts clean and the units dial
/// the configured edge until the first scan. `systemctl` is asked not to
/// block: this runs on the scanner thread, and a unit that takes a while
/// to stop must not delay the next scan.
fn drive_edge_path(status: &crate::edge::EdgeStatus) {
    let Some(path_file) = std::env::var("LOSOS_EDGE_PATH_FILE")
        .ok()
        .filter(|v| !v.is_empty())
    else {
        return;
    };
    let path_file = PathBuf::from(path_file);
    let none_file = std::env::var("LOSOS_EDGE_NONE_FILE")
        .ok()
        .filter(|v| !v.is_empty())
        .map_or_else(|| path_file.with_file_name("edge-none"), PathBuf::from);
    let pin_dir = std::env::var("LOSOS_EDGE_PIN_DIR")
        .ok()
        .filter(|v| !v.is_empty());
    let configured_pin = std::env::var("LOSOS_EDGE_NOISE_PUB_FILE")
        .ok()
        .filter(|v| !v.is_empty());

    let next = status.path.as_ref().map(|p| {
        // The pin lives with the edge it belongs to: the configured one at
        // losos.proxy.noisePublicKeyFile, a LAN edge under the pin dir by
        // its host, so a second gateway never inherits the first's key.
        // Noise is on for every edge or none (the configured file is unset
        // only when the module runs plain TCP).
        let pin = configured_pin.as_ref().map(|configured| match p.source {
            crate::edge::Source::Configured => configured.clone(),
            crate::edge::Source::Lan => match &pin_dir {
                Some(dir) => format!("{dir}/{}", crate::edge::pin_file_name(&p.url)),
                None => configured.clone(),
            },
        });
        crate::edge::path_env(p, pin.as_deref())
    });
    let prev = std::fs::read_to_string(&path_file).ok();
    let had_none = none_file.exists();

    let action = crate::edge::drive(prev.as_deref(), had_none, next.as_deref());
    let written = match &next {
        Some(env) => {
            let mut ok = true;
            if had_none {
                if let Err(e) = std::fs::remove_file(&none_file) {
                    tracing::error!(error = %e, path = %none_file.display(), "could not clear the no-edge marker");
                    ok = false;
                }
            }
            if prev.as_deref() != Some(env.as_str()) {
                if let Err(e) = atomic_write(&path_file, env.as_bytes()) {
                    tracing::error!(error = %e, path = %path_file.display(), "could not write the edge path");
                    ok = false;
                }
            }
            ok
        }
        None => {
            let mut ok = true;
            if !had_none {
                if let Err(e) = atomic_write(&none_file, b"") {
                    tracing::error!(error = %e, path = %none_file.display(), "could not write the no-edge marker");
                    ok = false;
                }
            }
            if prev.is_some() {
                let _ = std::fs::remove_file(&path_file);
            }
            ok
        }
    };
    if !written {
        return;
    }
    let verb = match action {
        crate::edge::Drive::Restart => "restart",
        crate::edge::Drive::Stop => "stop",
        crate::edge::Drive::Nothing => return,
    };
    match &status.path {
        Some(p) => {
            tracing::info!(edge = %p.url, rathole = %p.rathole, source = ?p.source, "edge path: {verb} the tunnel units")
        }
        None => tracing::info!("no edge in reach: {verb} the tunnel units"),
    }
    let out = std::process::Command::new("systemctl")
        .args([verb, "--no-block"])
        .args(crate::edge::PATH_UNITS)
        .output();
    match out {
        Ok(o) if o.status.success() => {}
        Ok(o) => tracing::error!(
            stderr = %String::from_utf8_lossy(&o.stderr).trim(),
            "systemctl {verb} of the tunnel units failed"
        ),
        Err(e) => tracing::error!(error = %e, "could not run systemctl for the tunnel units"),
    }
}

/// Mint the box's proxy token and rathole bootstrap token when the files
/// are absent (`LOSOS_PROXY_TOKEN_FILE`, `LOSOS_PROXY_BOOTSTRAP_FILE`; 64
/// hex characters, 0600, atomically), so a stock box can enrol with a LAN
/// gateway that takes any box on first contact. A file that exists is never
/// touched, whatever it holds: an official edge was given that token out of
/// band, and replacing it would lock the box out of its own tenant row.
fn ensure_proxy_secrets() {
    for var in ["LOSOS_PROXY_TOKEN_FILE", "LOSOS_PROXY_BOOTSTRAP_FILE"] {
        let Some(path) = std::env::var(var).ok().filter(|v| !v.is_empty()) else {
            continue;
        };
        let path = Path::new(&path);
        if path.exists() {
            continue;
        }
        let mut buf = [0u8; 32];
        let read = std::fs::File::open("/dev/urandom").and_then(|mut f| {
            use std::io::Read;
            f.read_exact(&mut buf)
        });
        if let Err(e) = read {
            tracing::error!(error = %e, "could not read /dev/urandom to mint {var}");
            continue;
        }
        let token: String = buf.iter().map(|b| format!("{b:02x}")).collect();
        match atomic_write_secret(path, token.as_bytes()) {
            Ok(()) => tracing::info!(path = %path.display(), "minted the {var} secret"),
            Err(e) => tracing::error!(error = %e, path = %path.display(), "could not mint {var}"),
        }
    }
}

/// Keep the edge scan fresh: one pass now, then one every
/// [`crate::edge::SCAN_INTERVAL`] for the life of the daemon.
///
/// A plain thread rather than a tokio task, like the rebuild watcher: the
/// scan is two subprocesses and a sleep, and the command core that reads the
/// result is synchronous.
pub fn start_edge_scanner(backend: &IoLosos) {
    let backend = backend.clone();
    let spawned = std::thread::Builder::new()
        .name("lososd-edge-scan".into())
        .spawn(move || {
            ensure_proxy_secrets();
            loop {
                backend.scan_edge();
                std::thread::sleep(crate::edge::SCAN_INTERVAL);
            }
        });
    if let Err(e) = spawned {
        tracing::error!(error = %e, "could not start the edge scanner; sharing stays gated on inline scans");
    }
}

/// Where the live custom domains go for LosOS cloud
/// (`$LOSOS_PUBLIC_NAMES_FILE`). Its directory is world-readable on purpose
/// and outside the daemon's 0700 state directory: the Nextcloud pod mounts it
/// read-only (modules/workloads.nix) and its PHP config reads the file on
/// every request. The names are public anyway; they are in public DNS.
pub const DEFAULT_PUBLIC_NAMES_FILE: &str = "/var/lib/losos-public-names/domains.json";

/// Where the relay pass goes for `losos-registrar announce --relay-pass-file`
/// (modules/proxy.nix). On `/run`: a pass is good for hours, so there is
/// nothing to keep across a reboot, and the next sync writes a fresh one.
pub const DEFAULT_RELAY_PASS_FILE: &str = "/run/losos/relay-pass";

/// How often the daemon asks the edge which custom domains are live, besides
/// every time the owner opens or changes them. A domain the edge turns live
/// on its own (the owner's DNS records appeared) reaches LosOS cloud within
/// this, so it is short enough that an owner who just added the records is
/// not left at "Untrusted domain" for long, and long enough to be one small
/// request in a quiet hour.
pub const DOMAIN_SYNC_INTERVAL: std::time::Duration = std::time::Duration::from_secs(120);

/// Keep the live custom domains fresh for LosOS cloud: ask the edge once now
/// and then every [`DOMAIN_SYNC_INTERVAL`]. Only an official edge is asked
/// (the same gate as the Network pane), and an answer that does not come
/// leaves the last one in place.
pub fn start_domain_sync(backend: &IoLosos) {
    let backend = backend.clone();
    let spawned = std::thread::Builder::new()
        .name("lososd-domain-sync".into())
        .spawn(move || {
            // Give the edge scanner its first pass, so the gate has a reading.
            std::thread::sleep(crate::edge::SCAN_INTERVAL);
            loop {
                let mut b = backend.clone();
                if let Err(e) = crate::losos::cmd_domains(&mut b) {
                    tracing::debug!(error = %e, "custom domains not refreshed");
                }
                std::thread::sleep(DOMAIN_SYNC_INTERVAL);
            }
        });
    if let Err(e) = spawned {
        tracing::error!(error = %e, "could not start the custom-domain sync; domains refresh only when the Network pane asks");
    }
}

/// Drive backups, restores and an erase (`crate::erase::tick`) on a short
/// cadence, under the state lock like every other writer of `state.json`.
pub fn start_erase_driver(backend: &IoLosos) {
    let backend = backend.clone();
    let spawned = std::thread::Builder::new()
        .name("lososd-erase".into())
        .spawn(move || loop {
            if let Err(e) = backend.serialized(crate::erase::tick) {
                tracing::warn!(error = ?e, "backup and erase step failed");
            }
            std::thread::sleep(crate::erase::TICK_EVERY);
        });
    if let Err(e) = spawned {
        tracing::error!(error = %e, "could not start the backup and erase driver");
    }
}

/// One request to the registrar's market, by `curl`.
///
/// The body — which carries the appliance's proxy token — goes to curl on
/// stdin (`--data-binary @-`), so the secret is never in an argument vector.
/// `--fail` is deliberately absent: the registrar's 4xx bodies are the
/// refusals the owner needs to read, and the status comes back via
/// `--write-out`. `--proto =https` and no `--insecure` for the reasons in
/// [`CurlFetch`], with more at stake here since this call authenticates.
fn curl_market(
    config: &crate::market::Config,
    op: &crate::market::Op,
) -> anyhow::Result<(u16, String)> {
    /// A listing page is small; a registrar that sends more is wrong.
    const MAX_BYTES: usize = 1024 * 1024;
    /// Above the registrar's own budget for the routes that wait on Stripe
    /// (`STRIPE_ROUTE_TIMEOUT` in backend-registrar/src/server.rs, 22 s:
    /// four gate calls of 5 s plus persistence), so a slow but successful
    /// onboarding is not dropped here after the account was already made.
    const TIMEOUT_SECS: &str = "25";

    let (method, path) = op.route();
    let token = std::fs::read_to_string(&config.token_file)
        .with_context(|| format!("reading the proxy token {}", config.token_file))?;
    let body = op.body(&config.appliance_id, token.trim());

    let mut cmd = std::process::Command::new("curl");
    cmd.args(["--silent", "--show-error"])
        .args([
            "--proto",
            "=https",
            "--proto-redir",
            "=https",
            "--max-redirs",
            "0",
        ])
        .args(["--max-time", TIMEOUT_SECS])
        .args(["--max-filesize", &MAX_BYTES.to_string()])
        .args(["--header", "Accept: application/json"])
        .args(["--write-out", "\n%{http_code}"])
        .args(["--request", method]);
    if body.is_some() {
        cmd.args(["--header", "Content-Type: application/json"])
            .args(["--data-binary", "@-"]);
    }
    cmd.arg(format!("{}{path}", config.registrar_url))
        .stdin(if body.is_some() {
            std::process::Stdio::piped()
        } else {
            std::process::Stdio::null()
        })
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = cmd
        .spawn()
        .context("running curl (is it on lososd's unit path? see modules/daemon.nix)")?;
    if let (Some(body), Some(mut stdin)) = (body, child.stdin.take()) {
        stdin
            .write_all(body.as_bytes())
            .context("sending the market request to curl")?;
    }
    let out = child.wait_with_output().context("waiting for curl")?;
    if !out.status.success() {
        anyhow::bail!(
            "the market request failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    if out.stdout.len() > MAX_BYTES + 8 {
        anyhow::bail!("the registrar returned more than {MAX_BYTES} bytes");
    }
    let text = String::from_utf8(out.stdout).context("the registrar returned non-UTF-8")?;
    let (body, code) = text
        .rsplit_once('\n')
        .context("curl returned no status line")?;
    let status = code
        .trim()
        .parse::<u16>()
        .context("curl returned a status that is not a number")?;
    Ok((status, body.to_string()))
}

/// The first non-empty line of a subprocess's stderr, for a sentence.
fn first_line(stderr: &[u8]) -> String {
    String::from_utf8_lossy(stderr)
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("")
        .to_string()
}

/// One request to LosOS Git's REST API as the bot administrator, by `curl`.
///
/// The token goes into a 0600 file as a ready-made `Authorization` header
/// and curl reads it with `--header @file`; the body — which carries a
/// password for two of the operations — goes to curl on stdin. Neither is
/// ever in an argument vector. Loopback and plain HTTP: the request never
/// leaves the box (`modules/workloads.nix` binds Forgejo to 127.0.0.1).
///
/// A curl that could not connect is [`crate::config_repo::NotUp`], the
/// normal state of a box whose Forgejo pod is still pulling. A reply, any
/// reply, is handed back with its status for [`crate::config_repo::classify`].
fn curl_forgejo(
    paths: &Paths,
    repo: &crate::config_repo::RepoConfig,
    op: &crate::config_repo::ForgejoOp,
    secret: Option<&crate::setup::Secret>,
) -> anyhow::Result<(u16, String)> {
    use crate::config_repo::NotUp;
    const MAX_BYTES: usize = 1024 * 1024;

    let token = read_bot_token(repo)?;
    let header = format!("Authorization: token {token}\n");
    atomic_write_secret(&paths.forgejo_header_file(), header.as_bytes())
        .context("writing the LosOS Git header file")?;
    let (method, path) = op.route();
    let body = op.body(secret)?;

    let mut cmd = std::process::Command::new("curl");
    cmd.args(["--silent", "--show-error"])
        .args(["--max-time", "20"])
        .args(["--max-filesize", &MAX_BYTES.to_string()])
        .args(["--header", "Accept: application/json"])
        .arg("--header")
        .arg(format!("@{}", paths.forgejo_header_file().display()))
        .args(["--write-out", "\n%{http_code}"])
        .args(["--request", method]);
    if body.is_some() {
        cmd.args(["--header", "Content-Type: application/json"])
            .args(["--data-binary", "@-"]);
    }
    cmd.arg(format!("{}{path}", repo.forgejo_url))
        .stdin(if body.is_some() {
            std::process::Stdio::piped()
        } else {
            std::process::Stdio::null()
        })
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = cmd
        .spawn()
        .context("running curl (is it on lososd's unit path? see modules/daemon.nix)")?;
    if let (Some(body), Some(mut stdin)) = (body, child.stdin.take()) {
        stdin
            .write_all(body.as_bytes())
            .context("sending the LosOS Git request to curl")?;
    }
    let out = child.wait_with_output().context("waiting for curl")?;
    if !out.status.success() {
        return Err(NotUp(format!(
            "LosOS Git is not answering yet ({})",
            first_line(&out.stderr)
        ))
        .into());
    }
    let text = String::from_utf8(out.stdout).context("LosOS Git returned non-UTF-8")?;
    let (body, code) = text
        .rsplit_once('\n')
        .context("curl returned no status line")?;
    let status = code
        .trim()
        .parse::<u16>()
        .context("curl returned a status that is not a number")?;
    Ok((status, body.to_string()))
}

/// The bot administrator's access token, or [`crate::config_repo::NotUp`]
/// when the Forgejo bootstrap has not written it yet.
fn read_bot_token(repo: &crate::config_repo::RepoConfig) -> anyhow::Result<String> {
    match std::fs::read_to_string(&repo.token_file) {
        Ok(t) if !t.trim().is_empty() => Ok(t.trim().to_string()),
        Ok(_) => Err(crate::config_repo::NotUp(
            "LosOS Git has not finished its first start yet (the access token is empty)"
                .to_string(),
        )
        .into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(crate::config_repo::NotUp(
            "LosOS Git has not finished its first start yet (no access token at its path)"
                .to_string(),
        )
        .into()),
        Err(e) => Err(anyhow::Error::new(e).context(format!("reading {}", repo.token_file))),
    }
}

/// `git`, in the configuration repository, with the identity and the
/// settings every invocation here wants.
///
/// `HOME` is pointed at the state directory: the unit runs with
/// `ProtectHome=true`, so `/root` is unreadable and git would warn on every
/// call about a `.gitconfig` it cannot read. `GIT_TERMINAL_PROMPT=0` is what
/// keeps a push with no credentials from hanging on a prompt nobody will
/// answer. `with_auth` adds a credential store written from the bot token,
/// for the two commands that talk to LosOS Git.
fn git(paths: &Paths, with_auth: bool, args: &[&str]) -> anyhow::Result<std::process::Output> {
    let mut cmd = std::process::Command::new("git");
    cmd.arg("-C")
        .arg(&paths.config_dir)
        .args([
            "-c",
            &format!("user.name={}", crate::config_repo::COMMITTER_NAME),
        ])
        .args([
            "-c",
            &format!("user.email={}", crate::config_repo::COMMITTER_EMAIL),
        ])
        .env("HOME", &paths.state_dir)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env_remove("GIT_ASKPASS")
        .env_remove("SSH_ASKPASS");
    if with_auth {
        if let Some(repo) = crate::config_repo::RepoConfig::from_env().filter(|r| r.via_forgejo()) {
            let token = read_bot_token(&repo)?;
            let host = repo
                .forgejo_url
                .split_once("://")
                .map_or(repo.forgejo_url.as_str(), |(_, rest)| rest)
                .trim_end_matches('/');
            let scheme = repo
                .forgejo_url
                .split_once("://")
                .map_or("http", |(scheme, _)| scheme);
            let line = format!(
                "{scheme}://{}:{token}@{host}\n",
                crate::config_repo::BOT_USER
            );
            let store = paths.git_credentials_file();
            atomic_write_secret(&store, line.as_bytes())
                .context("writing the LosOS Git credential store")?;
            cmd.args([
                "-c",
                &format!("credential.helper=store --file={}", store.display()),
            ]);
        }
    }
    cmd.args(args)
        .stdin(std::process::Stdio::null())
        .output()
        .context("running git (is it on lososd's unit path? see modules/daemon.nix)")
}

/// `git` that must succeed; the first stderr line is the error.
fn git_ok(paths: &Paths, with_auth: bool, args: &[&str]) -> anyhow::Result<String> {
    let out = git(paths, with_auth, args)?;
    if !out.status.success() {
        anyhow::bail!("git {}: {}", args.join(" "), first_line(&out.stderr));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

impl crate::recovery::CodeStore for FileCodeStore {
    fn read_code(&mut self) -> anyhow::Result<Option<String>> {
        match std::fs::read_to_string(&self.path) {
            Ok(s) => Ok(Some(s)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            // Must be an error rather than `None`: `ensure_code` mints over a
            // `None`, so reporting an EIO or an EACCES that way would destroy a
            // code the owner is still holding on paper. `tests/recovery.rs`
            // asserts the propagation.
            Err(e) => Err(anyhow::Error::new(e).context(format!(
                "reading the recovery code at {}",
                self.path.display()
            ))),
        }
    }

    fn write_code(&mut self, code: &str) -> anyhow::Result<()> {
        // Trailing newline so `cat` of the file in a rescue shell prints
        // cleanly; `is_well_formed` trims, so the round trip is exact.
        atomic_write_secret(&self.path, format!("{code}\n").as_bytes())
            .with_context(|| format!("writing the recovery code to {}", self.path.display()))
    }

    fn fresh_bytes(&mut self) -> anyhow::Result<[u8; 16]> {
        use std::io::Read;
        let mut buf = [0u8; 16];
        std::fs::File::open("/dev/urandom")
            .and_then(|mut f| f.read_exact(&mut buf))
            .context("reading 16 bytes from /dev/urandom")?;
        Ok(buf)
    }
}

/// Read the persisted state.
///
/// A missing file means a fresh appliance. A *corrupt* file is read as the
/// defaults rather than raising: the admin UI going blank is a worse failure
/// than silently resetting to defaults, and the next write repairs the file.
///
/// With one exception: a corrupt or otherwise unreadable file reads as **claimed**. Unclaimed is the
/// state in which `POST /api/setup/claim` needs no token, so failing open
/// here would hand an owned box to the next LAN caller whenever its state
/// file was damaged. A genuinely fresh box has no file, not a corrupt one.
pub fn read_state(path: &Path) -> State {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        // `read` reports NotFound for a dangling symlink too, so absence is
        // confirmed on the directory entry itself: only an entry that is really
        // missing is a fresh box, and an entry that exists (or cannot be
        // inspected) is not.
        Err(e)
            if e.kind() == std::io::ErrorKind::NotFound
                && matches!(
                    std::fs::symlink_metadata(path),
                    Err(ref m) if m.kind() == std::io::ErrorKind::NotFound
                ) =>
        {
            return State::default()
        }
        Err(e) => {
            // A path that became a directory, lost traversal permission or
            // hit an I/O error is not a fresh box: fail closed like corruption.
            tracing::warn!(path = %path.display(), error = %e, "state file unreadable; using defaults (claimed)");
            return State {
                claimed: true,
                ..State::default()
            };
        }
    };
    match serde_json::from_slice::<State>(&bytes) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "unreadable state file; using defaults (claimed)");
            State {
                claimed: true,
                ..State::default()
            }
        }
    }
}

/// The look document, leniently: absent or unreadable is the default, so a
/// damaged file costs the owner their wallpaper and not the admin page.
fn read_look(path: &Path) -> crate::look::Look {
    match std::fs::read(path) {
        Ok(bytes) => match serde_json::from_slice::<crate::look::Look>(&bytes) {
            Ok(look) => look,
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "unreadable look file; using the plain look");
                crate::look::Look::default()
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => crate::look::Look::default(),
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "look file unreadable; using the plain look");
            crate::look::Look::default()
        }
    }
}

/// Persist the state atomically.
pub fn write_state(path: &Path, s: &State) -> anyhow::Result<()> {
    let bytes = serde_json::to_vec(s).context("encoding state")?;
    atomic_write(path, &bytes)
}

// ── Backups and erasing the box ─────────────────────────────────────────

/// Where the backup machinery keeps its files, from the environment
/// `modules/backup.nix` gives lososd, with the appliance defaults.
#[derive(Debug, Clone)]
pub struct BackupFiles {
    /// The bucket, as JSON, 0600.
    pub target: PathBuf,
    /// The same bucket as the units' `EnvironmentFile=`, 0600.
    pub env: PathBuf,
    /// The scripts' working directory: the log, the report, the staged
    /// settings of a restore.
    pub dir: PathBuf,
    /// The repository password a backup uses: the recovery code.
    pub recovery_code: PathBuf,
    /// The code a restore was given, on `/run`.
    pub restore_code: PathBuf,
    pub backup_script: Option<String>,
    pub restore_script: Option<String>,
    pub grace_secs: u64,
    /// What `losos-factory-wipe.service` looks for at boot.
    pub wipe_marker: PathBuf,
    /// What an erase gave up outside the box; the wipe keeps it.
    pub erase_report: PathBuf,
}

impl BackupFiles {
    pub fn from_env() -> Self {
        let var = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        BackupFiles {
            target: PathBuf::from(env_or(
                "LOSOS_BACKUP_TARGET_FILE",
                "/var/secrets/losos-backup.json",
            )),
            env: PathBuf::from(env_or(
                "LOSOS_BACKUP_ENV_FILE",
                "/var/secrets/losos-backup.env",
            )),
            dir: PathBuf::from(env_or("LOSOS_BACKUP_DIR", "/var/lib/losos-backup")),
            recovery_code: crate::recovery::code_file(),
            restore_code: PathBuf::from(env_or(
                "LOSOS_RESTORE_CODE_FILE",
                "/run/losos/restore-code",
            )),
            backup_script: var("LOSOS_BACKUP_SCRIPT"),
            restore_script: var("LOSOS_RESTORE_SCRIPT"),
            grace_secs: var("LOSOS_ERASE_GRACE_SECS")
                .and_then(|v| v.parse().ok())
                .unwrap_or(900),
            wipe_marker: PathBuf::from(env_or("LOSOS_WIPE_MARKER", "/persist/.losos-factory-wipe")),
            erase_report: PathBuf::from(env_or(
                "LOSOS_ERASE_REPORT",
                "/var/lib/losos-erase/report.json",
            )),
        }
    }

    pub fn log(&self) -> PathBuf {
        self.dir.join("job.log")
    }

    /// Written by the backup script after a successful run.
    pub fn report(&self) -> PathBuf {
        self.dir.join("last.json")
    }

    /// Where the restore script leaves the `overrides.nix` it brought back.
    pub fn restored_overrides(&self) -> PathBuf {
        self.dir.join("restored-overrides.nix")
    }
}

/// The `systemd-run` arguments for one backup or restore job.
///
/// The bucket's keys arrive through `EnvironmentFile=`, the password through
/// `RESTIC_PASSWORD_FILE`: neither is ever an argument here, so `ps` and the
/// journal never show them. The script carries its own PATH (it is a
/// `writeShellApplication`), so unlike a rebuild nothing of lososd's is
/// passed on.
pub fn backup_launch_args(
    files: &BackupFiles,
    kind: crate::backup::Kind,
    job: &str,
    script: &str,
) -> Vec<String> {
    let log = files.log().to_string_lossy().into_owned();
    let password = match kind {
        crate::backup::Kind::Backup => &files.recovery_code,
        crate::backup::Kind::Restore => &files.restore_code,
    };
    vec![
        format!("--unit={}", kind.unit(job)),
        format!("--description=losos {} {job}", kind.as_str()),
        format!("--property=EnvironmentFile={}", files.env.display()),
        format!("--property=StandardOutput=append:{log}"),
        format!("--property=StandardError=append:{log}"),
        format!("--setenv=LOSOS_BACKUP_DIR={}", files.dir.display()),
        format!("--setenv=RESTIC_PASSWORD_FILE={}", password.display()),
        script.to_string(),
    ]
}

/// Remove a file; one that is not there is already removed.
fn remove_if_present(path: &Path) -> anyhow::Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(anyhow::Error::new(e).context(format!("removing {}", path.display()))),
    }
}

/// The production backend.
///
/// Every clone shares one lock, and that lock is the whole serialisation story
/// for `state.json`. There used to be two: `dbus` and `http` each built their
/// own, so a D-Bus `Change` and a `POST /api/change` raced, and the rebuild
/// watcher recorded outcomes under no lock at all — which could drop a
/// finished rebuild, or clobber the record of the one that replaced it.
///
/// Cloning is cheap and carries no state: the paths are immutable and the lock
/// is shared, so a clone is a second handle to the same appliance.
#[derive(Debug, Clone)]
pub struct IoLosos {
    pub paths: Paths,
    state_lock: Arc<Mutex<()>>,
    /// The latest edge scan (`crate::edge`), shared by every clone. `None`
    /// until the scanner thread has run once; `edge_status` then scans
    /// inline rather than answer "unknown" to the first caller.
    edge: Arc<Mutex<Option<crate::edge::EdgeStatus>>>,
}

impl IoLosos {
    /// A backend and a fresh lock. Call this **once** per process and clone the
    /// result; two separately constructed backends do not serialise each other.
    pub fn new(paths: Paths) -> Self {
        IoLosos {
            paths,
            state_lock: Arc::new(Mutex::new(())),
            edge: Arc::new(Mutex::new(None)),
        }
    }

    /// Look for an edge proxy now and remember the answer. What the scanner
    /// thread calls on its cadence, and what `edge_status` falls back to
    /// before the thread's first pass.
    pub fn scan_edge(&self) -> crate::edge::EdgeStatus {
        let status = scan_edge_now();
        tracing::debug!(
            reachable = status.reachable,
            edges = status.edges.len(),
            lan_searched = status.lan_searched,
            "edge scan"
        );
        *self.edge.lock().unwrap_or_else(|p| p.into_inner()) = Some(status.clone());
        drive_edge_path(&status);
        status
    }

    /// The cached scan, or a fresh one when there is none yet.
    fn edge_cached(&self) -> crate::edge::EdgeStatus {
        let cached = self.edge.lock().unwrap_or_else(|p| p.into_inner()).clone();
        match cached {
            Some(status) => status,
            None => self.scan_edge(),
        }
    }

    pub fn from_env() -> Self {
        Self::new(Paths::from_env())
    }

    /// Take the state lock.
    ///
    /// Poisoning is recovered from on purpose. The mutex guards no in-memory
    /// invariant — the state is a file, and a corrupt one already reads as the
    /// default — so honouring the poison would turn one panicked command into a
    /// permanently dead admin surface on a box with no shell to repair it.
    pub(crate) fn lock_state(&self) -> MutexGuard<'_, ()> {
        self.state_lock.lock().unwrap_or_else(|poisoned| {
            tracing::warn!("state lock was poisoned by a panicking command; recovering");
            poisoned.into_inner()
        })
    }

    /// Run one command with the state lock held from the first effect to the
    /// last.
    ///
    /// Per-effect locking would not do: a command is a read-modify-write spread
    /// over several trait calls (`load_state`, mutate, `save_state`,
    /// `spawn_rebuild`), and two of them interleaving is exactly the race this
    /// prevents.
    ///
    /// Not reentrant — never call it from inside `f`.
    pub fn serialized<T>(
        &self,
        f: impl FnOnce(&mut IoLosos) -> anyhow::Result<T>,
    ) -> anyhow::Result<T> {
        let _guard = self.lock_state();
        let mut backend = self.clone();
        f(&mut backend)
    }
}

impl Default for IoLosos {
    fn default() -> Self {
        Self::from_env()
    }
}

impl Losos for IoLosos {
    fn load_state(&mut self) -> anyhow::Result<State> {
        Ok(read_state(&self.paths.state_file()))
    }

    fn edge_status(&mut self) -> anyhow::Result<crate::edge::EdgeStatus> {
        Ok(self.edge_cached())
    }

    fn save_state(&mut self, s: &State) -> anyhow::Result<()> {
        write_state(&self.paths.state_file(), s)
    }

    fn write_overrides(&mut self, body: &str) -> anyhow::Result<()> {
        atomic_write(&self.paths.overrides_file, body.as_bytes())
    }

    fn read_overrides(&mut self) -> anyhow::Result<String> {
        match std::fs::read_to_string(&self.paths.overrides_file) {
            Ok(body) => Ok(body),
            // Absent means a fresh appliance, and the committed defaults are
            // the honest answer. A read *error* is not the same thing: reporting
            // defaults would have the settings page paint values the box is not
            // running, and the next Apply would write them over the real config.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Ok(DEFAULT_OVERRIDES_NIX.to_string())
            }
            Err(e) => Err(anyhow::Error::new(e)
                .context(format!("reading {}", self.paths.overrides_file.display()))),
        }
    }

    fn spawn_rebuild(&mut self, job: &str) -> anyhow::Result<()> {
        supervisor::spawn_rebuild(self, job)
    }

    fn rebuild_log_tail(&mut self) -> anyhow::Result<String> {
        Ok(supervisor::log_tail(&self.paths.rebuild_log()))
    }

    // ── The owner's look ────────────────────────────────────────────────
    fn load_look(&mut self) -> anyhow::Result<crate::look::Look> {
        Ok(read_look(&self.paths.look_file()))
    }

    fn save_look(&mut self, look: &crate::look::Look) -> anyhow::Result<()> {
        let bytes = serde_json::to_vec(look).context("encoding the look")?;
        atomic_write(&self.paths.look_file(), &bytes)
    }

    fn write_background(&mut self, bytes: &[u8]) -> anyhow::Result<()> {
        atomic_write(&self.paths.background_file(), bytes)
    }

    fn read_background(&mut self) -> anyhow::Result<Option<Vec<u8>>> {
        let path = self.paths.background_file();
        match std::fs::read(&path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(anyhow::Error::new(e).context(format!("reading {}", path.display()))),
        }
    }

    fn remove_background(&mut self) -> anyhow::Result<()> {
        let path = self.paths.background_file();
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(anyhow::Error::new(e).context(format!("removing {}", path.display()))),
        }
    }

    // ── Online growth of /persist ───────────────────────────────────────
    fn vg_free(&mut self) -> anyhow::Result<crate::grow::VgFree> {
        // `vgs --units b --nosuffix --noheadings -o vg_free_count,vg_extent_size`
        // gives the two numbers with no locale formatting to reparse. The
        // extent count is already a count, so --units only affects the size.
        let out = std::process::Command::new("vgs")
            .args([
                "--noheadings",
                "--nosuffix",
                "--units",
                "b",
                "-o",
                "vg_free_count,vg_extent_size",
                crate::grow::VG,
            ])
            .output()
            .with_context(|| format!("running vgs against {}", crate::grow::VG))?;
        if !out.status.success() {
            anyhow::bail!(
                "vgs failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        let text = String::from_utf8_lossy(&out.stdout);
        let mut fields = text.split_whitespace();
        let free_extents: u64 = fields
            .next()
            .and_then(|f| f.parse().ok())
            .with_context(|| format!("no free-extent count in vgs output: {text:?}"))?;
        let extent_bytes: u64 = fields
            .next()
            .and_then(|f| f.parse().ok())
            .with_context(|| format!("no extent size in vgs output: {text:?}"))?;
        Ok(crate::grow::VgFree {
            free_extents,
            extent_bytes,
        })
    }

    fn luks_key_file(&mut self) -> anyhow::Result<Option<String>> {
        // `$LOSOS_LUKS_KEYFILE`, set by modules/daemon.nix in both unlock
        // modes (the TPM path keeps the keyfile inside /persist as its
        // recovery slot).
        //
        // Unset means "rely on the kernel keyring", where the volume key sits
        // after a TPM2 unlock. Where it is neither set nor in the keyring,
        // cryptsetup falls back to prompting on stdin
        // — and a daemon has none, so the resize dies with "Nothing to read on
        // input." *after* lvextend has already grown the logical volume. That
        // is not theoretical: it is how tests/resize.nix failed first.
        Ok(std::env::var("LOSOS_LUKS_KEYFILE")
            .ok()
            .filter(|p| !p.is_empty()))
    }

    fn run_grow(&mut self, action: &crate::grow::GrowAction) -> anyhow::Result<()> {
        let argv = crate::grow::action_argv(action);
        let (cmd, args) = argv.split_first().expect("action_argv is never empty");
        let out = std::process::Command::new(cmd)
            .args(args)
            .output()
            .with_context(|| format!("running {}", argv.join(" ")))?;
        if !out.status.success() {
            anyhow::bail!(
                "{} failed: {}",
                argv.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        Ok(())
    }

    fn persist_bytes(&mut self) -> anyhow::Result<u64> {
        // `df -B1`, not `df -h`: this number is compared before and after to
        // decide whether the grow did anything, so a value rounded to "1.6T"
        // would report no change for the first 50 GB of growth.
        //
        // A subprocess rather than statvfs(2) because the alternative is a new
        // crate dependency, and backend/Cargo.toml declares its whole set up
        // front specifically so Cargo.lock — and the cargoHash pinned in
        // flake/packages.nix — settles once. The other three steps here are
        // subprocesses anyway.
        let out = std::process::Command::new("df")
            .args(["-B1", "--output=size", "/persist"])
            .output()
            .context("running df against /persist")?;
        if !out.status.success() {
            anyhow::bail!("df failed: {}", String::from_utf8_lossy(&out.stderr).trim());
        }
        let text = String::from_utf8_lossy(&out.stdout);
        // Line 1 is the "1B-blocks" header; line 2 is the number.
        text.lines()
            .nth(1)
            .and_then(|l| l.trim().parse().ok())
            .with_context(|| format!("no size in df output: {text:?}"))
    }

    // ── Setting the Nextcloud admin password ────────────────────────────
    fn nextcloud_mode(&mut self) -> anyhow::Result<crate::setup::NcMode> {
        // No default. Guessing "container" would run `crictl` against a socket
        // that does not exist on a native box, and guessing "native" would run
        // `nextcloud-occ`, which is not even in the closure of a container-mode
        // box — and both failures read as "the command is broken" rather than
        // "the daemon was not told". modules/daemon.nix sets this from
        // config.losos.nextcloud.mode.
        let raw = match std::env::var("LOSOS_NEXTCLOUD_MODE") {
            Ok(raw) => raw,
            Err(std::env::VarError::NotPresent) => anyhow::bail!(
                "LOSOS_NEXTCLOUD_MODE is not set, so lososd cannot tell whether \
                 Nextcloud is running as a k3s workload or natively. \
                 modules/daemon.nix must set it from config.losos.nextcloud.mode."
            ),
            Err(e) => return Err(anyhow::Error::new(e).context("LOSOS_NEXTCLOUD_MODE")),
        };
        crate::setup::NcMode::parse(raw.trim()).with_context(|| {
            format!("LOSOS_NEXTCLOUD_MODE must be 'container' or 'native': {raw:?}")
        })
    }

    fn nextcloud_target(
        &mut self,
        mode: crate::setup::NcMode,
    ) -> anyhow::Result<crate::setup::Target> {
        if mode == crate::setup::NcMode::Native {
            return Ok(crate::setup::Target::Native);
        }
        let socket = std::env::var("LOSOS_CRI_SOCKET")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| crate::setup::DEFAULT_CRI_SOCKET.to_string());
        let argv = crate::setup::resolve_argv(&socket);
        let (cmd, args) = argv.split_first().context("resolve_argv is never empty")?;
        let out = std::process::Command::new(cmd)
            .args(args)
            .output()
            .with_context(|| {
                format!(
                    "running {} (is pkgs.cri-tools on lososd's unit path?)",
                    argv.join(" ")
                )
            })?;
        if !out.status.success() {
            anyhow::bail!(
                "{} failed: {}",
                argv.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        let id = crate::setup::parse_container_ids(&String::from_utf8_lossy(&out.stdout))
            .map_err(|e| anyhow::anyhow!(e))?;
        Ok(crate::setup::Target::Container { socket, id })
    }

    fn run_occ(
        &mut self,
        action: &crate::setup::OccAction,
        secret: &crate::setup::Secret,
    ) -> anyhow::Result<Option<crate::setup::OccOutcome>> {
        use crate::setup::{OccAction, OccOutcome, SecretChannel};
        match action {
            OccAction::StageSecret { path, uid } => {
                let path = Path::new(path);
                // No trailing newline: the in-image wrapper reads this with
                // `$(cat …)`, which strips trailing newlines, so writing one
                // would make the two sides agree only by accident.
                // Owned by the pod's uid from creation, via fchown on the temp
                // file: never a path-based chown after the rename, which would
                // follow a symlink the pod planted in its own directory.
                atomic_write_secret_owned(path, secret.expose().as_bytes(), *uid)
                    .with_context(|| format!("staging the new password at {}", path.display()))?;
                Ok(None)
            }
            OccAction::ClearSecret { path } => {
                match std::fs::remove_file(path) {
                    Ok(()) => {}
                    // Already gone is the goal, not a failure.
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => {
                        return Err(anyhow::Error::new(e)
                            .context(format!("removing the staged password at {path}")))
                    }
                }
                Ok(None)
            }
            OccAction::RunOcc { argv, secret: chan } => {
                let (cmd, args) = argv.split_first().context("RunOcc argv is never empty")?;
                let mut child = std::process::Command::new(cmd);
                child.args(args);
                if let SecretChannel::Env = chan {
                    // /proc/<pid>/environ is mode 0400 owner-only and this
                    // child is root, unlike /proc/<pid>/cmdline.
                    child.env("OC_PASS", secret.expose());
                    // ResetPassword.php reads `getenv('NC_PASS') ?: getenv('OC_PASS')`,
                    // so an NC_PASS inherited from anywhere would silently win
                    // over the password the owner just typed.
                    child.env_remove("NC_PASS");
                    // The nixpkgs nextcloud-occ wrapper tests `$USER` under
                    // `set -u`, and systemd does not export USER to a root
                    // service with no User=. Unset, the wrapper aborts with
                    // "USER: unbound variable" before occ ever starts.
                    child.env("USER", "root");
                }
                let out = child.output().with_context(|| {
                    format!(
                        "running {} (is the occ wrapper on lososd's unit path?)",
                        // The argv is safe to quote: the password is never in it.
                        argv.join(" ")
                    )
                })?;
                Ok(Some(OccOutcome {
                    // A signalled child has no code; -1 is not a status occ can
                    // return, so it cannot be mistaken for one.
                    code: out.status.code().unwrap_or(-1),
                    stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
                    stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
                }))
            }
        }
    }

    fn nextcloud_status(
        &mut self,
        target: &crate::setup::Target,
    ) -> anyhow::Result<crate::setup::OccOutcome> {
        let argv = crate::setup::plan_status(target);
        let (cmd, args) = argv.split_first().context("plan_status is never empty")?;
        let mut child = std::process::Command::new(cmd);
        child.args(args);
        // Same reason as the native RunOcc arm above: nixpkgs' nextcloud-occ
        // wrapper reads `$USER` under `set -u`.
        child.env("USER", "root");
        let out = child
            .output()
            .with_context(|| format!("running {}", argv.join(" ")))?;
        Ok(crate::setup::OccOutcome {
            code: out.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        })
    }

    fn nextcloud_last_log(&mut self, mode: crate::setup::NcMode) -> anyhow::Result<Option<String>> {
        if mode == crate::setup::NcMode::Native {
            return Ok(None);
        }
        let socket = std::env::var("LOSOS_CRI_SOCKET")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| crate::setup::DEFAULT_CRI_SOCKET.to_string());
        let run = |argv: Vec<String>| -> anyhow::Result<std::process::Output> {
            let (cmd, args) = argv.split_first().context("argv is never empty")?;
            std::process::Command::new(cmd)
                .args(args)
                .output()
                .with_context(|| format!("running {}", argv.join(" ")))
        };
        let listed = run(crate::setup::last_container_argv(&socket))?;
        if !listed.status.success() {
            anyhow::bail!(
                "crictl ps --all failed: {}",
                String::from_utf8_lossy(&listed.stderr).trim()
            );
        }
        let stdout = String::from_utf8_lossy(&listed.stdout);
        let Some(id) = stdout.lines().map(str::trim).find(|l| !l.is_empty()) else {
            return Ok(None);
        };
        // `crictl logs` prints the container's stdout and stderr on its own
        // matching streams; the entrypoint's `fail` writes to stderr.
        let logged = run(crate::setup::container_log_argv(&socket, id))?;
        let text = format!(
            "{}\n{}",
            String::from_utf8_lossy(&logged.stdout),
            String::from_utf8_lossy(&logged.stderr)
        );
        Ok(if text.trim().is_empty() {
            None
        } else {
            Some(text)
        })
    }

    fn nextcloud_login(
        &mut self,
        user: &str,
        secret: &crate::setup::Secret,
        client: &str,
    ) -> anyhow::Result<crate::signin::LoginOutcome> {
        use std::io::Read;
        use std::net::{TcpStream, ToSocketAddrs};
        use std::time::Duration;

        let url = std::env::var("LOSOS_NEXTCLOUD_LOGIN_URL")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| crate::signin::DEFAULT_LOGIN_URL.to_string());
        let host = std::env::var("LOSOS_NEXTCLOUD_LOGIN_HOST")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| crate::signin::DEFAULT_LOGIN_HOST.to_string());
        let endpoint = crate::signin::parse_login_url(&url).map_err(|e| anyhow::anyhow!(e))?;
        let request = crate::signin::login_request(&endpoint, &host, user, secret, client);

        // A raw socket rather than curl: curl takes credentials on the
        // command line or in a file, and neither is a channel this crate
        // lets a password use (see the header of crate::setup). The hop is
        // loopback and the answer is one status line, so std is enough.
        let addr = endpoint
            .addr
            .to_socket_addrs()
            .with_context(|| format!("resolving {}", endpoint.addr))?
            .next()
            .with_context(|| format!("{} names no address", endpoint.addr))?;
        let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(5))
            .with_context(|| format!("connecting to LosOS cloud at {addr}"))?;
        // Nextcloud's brute-force protection answers a wrong password late on
        // purpose — up to 25 seconds — so the read waits longer than that.
        stream.set_read_timeout(Some(Duration::from_secs(40)))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;
        stream
            .write_all(&request)
            .context("sending the sign-in probe to LosOS cloud")?;
        // The status line is all that is read; the rest is drained only as
        // far as one buffer goes, and `Connection: close` ends the exchange.
        let mut buf = vec![0u8; 4096];
        let mut filled = 0;
        while filled < buf.len() {
            match stream.read(&mut buf[filled..]) {
                Ok(0) => break,
                Ok(n) => {
                    filled += n;
                    if buf[..filled].windows(4).any(|w| w == b"\r\n\r\n") {
                        break;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e).context("reading LosOS cloud's answer"),
            }
        }
        Ok(crate::signin::interpret_login(crate::signin::status_code(
            &buf[..filled],
        )))
    }

    fn recovery_code(&mut self) -> anyhow::Result<crate::recovery::Recovery> {
        crate::recovery::ensure_code(&mut FileCodeStore::from_env())
    }

    fn search_apps(&mut self, query: &str) -> anyhow::Result<Vec<crate::catalogue::App>> {
        let mut fetch = CurlFetch;
        let url = crate::catalogue::search_url(query);
        let body = crate::catalogue::Fetch::get(&mut fetch, &url)?;
        crate::catalogue::parse_results(&body)
    }

    fn write_public_names(&mut self, names: &[String]) -> anyhow::Result<()> {
        let path =
            std::path::PathBuf::from(env_or("LOSOS_PUBLIC_NAMES_FILE", DEFAULT_PUBLIC_NAMES_FILE));
        let body = format!("{}\n", serde_json::to_string(names)?);
        // Unchanged is the common case (every sync, most page loads); leave
        // the file and its mtime alone then.
        if std::fs::read_to_string(&path).is_ok_and(|old| old == body) {
            return Ok(());
        }
        atomic_write(&path, body.as_bytes()).with_context(|| format!("writing {}", path.display()))
    }

    fn write_relay_pass(&mut self, pass: Option<&str>) -> anyhow::Result<()> {
        let path =
            std::path::PathBuf::from(env_or("LOSOS_RELAY_PASS_FILE", DEFAULT_RELAY_PASS_FILE));
        match pass {
            None => match std::fs::remove_file(&path) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(e).with_context(|| format!("removing {}", path.display())),
            },
            Some(pass) => {
                let body = format!("{pass}\n");
                if std::fs::read_to_string(&path).is_ok_and(|old| old == body) {
                    return Ok(());
                }
                atomic_write_secret(&path, body.as_bytes())
                    .with_context(|| format!("writing {}", path.display()))
            }
        }
    }

    fn market_request(&mut self, op: &crate::market::Op) -> anyhow::Result<crate::market::Outcome> {
        let Some(config) = crate::market::Config::from_env() else {
            return Ok(crate::market::Outcome::Unavailable);
        };
        let (status, body) = curl_market(&config, op)?;
        crate::market::classify(status, &body)
    }

    fn read_options_doc(&mut self) -> anyhow::Result<Option<crate::options::OptionsDoc>> {
        match std::fs::read_to_string(&self.paths.options_file) {
            Ok(text) => crate::options::parse(&text)
                .map(Some)
                .with_context(|| format!("reading {}", self.paths.options_file.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(anyhow::Error::new(e)
                .context(format!("reading {}", self.paths.options_file.display()))),
        }
    }

    fn config_repo(&mut self) -> Option<crate::config_repo::RepoConfig> {
        crate::config_repo::RepoConfig::from_env()
    }

    fn config_head(&mut self) -> anyhow::Result<Option<crate::config_repo::Head>> {
        let out = git(&self.paths, false, &["rev-parse", "--verify", "-q", "HEAD"])?;
        if !out.status.success() {
            // Not a repository, or one with no commit yet: both are "none".
            return Ok(None);
        }
        let sha = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let branch = git_ok(
            &self.paths,
            false,
            &["symbolic-ref", "--short", "-q", "HEAD"],
        )
        .unwrap_or_else(|_| "HEAD".to_string());
        Ok(Some(crate::config_repo::Head { sha, branch }))
    }

    fn config_commit(&mut self, subject: &str, body: &str) -> anyhow::Result<Option<String>> {
        git_ok(&self.paths, false, &["add", "-A"])?;
        let staged = git(&self.paths, false, &["diff", "--cached", "--quiet"])?;
        // Exit 0 is "nothing staged" — unless there is no HEAD yet, in which
        // case `diff --cached` compares against the empty tree and a fresh
        // repository with files reads as dirty, which is right.
        if staged.status.success() {
            return Ok(None);
        }
        let mut args = vec!["commit", "-q", "-m", subject];
        if !body.is_empty() {
            args.extend(["-m", body]);
        }
        git_ok(&self.paths, false, &args)?;
        let sha = git_ok(&self.paths, false, &["rev-parse", "HEAD"])?;
        tracing::info!(sha = %sha, subject, "committed to the configuration repository");
        Ok(Some(sha))
    }

    fn config_fetch(&mut self, url: &str, branch: &str) -> anyhow::Result<Option<String>> {
        let refspec = format!("refs/heads/{branch}");
        let out = git(&self.paths, true, &["ls-remote", "--heads", url, &refspec])?;
        if !out.status.success() {
            return Err(crate::config_repo::NotUp(format!(
                "LosOS Git could not be reached ({})",
                first_line(&out.stderr)
            ))
            .into());
        }
        if String::from_utf8_lossy(&out.stdout).trim().is_empty() {
            return Ok(None);
        }
        git_ok(&self.paths, true, &["fetch", "-q", url, &refspec])?;
        Ok(Some(git_ok(
            &self.paths,
            false,
            &["rev-parse", "FETCH_HEAD"],
        )?))
    }

    fn config_is_ancestor(&mut self, ancestor: &str, of: &str) -> anyhow::Result<bool> {
        let out = git(
            &self.paths,
            false,
            &["merge-base", "--is-ancestor", ancestor, of],
        )?;
        match out.status.code() {
            Some(0) => Ok(true),
            Some(1) => Ok(false),
            _ => anyhow::bail!(
                "git merge-base --is-ancestor {ancestor} {of}: {}",
                first_line(&out.stderr)
            ),
        }
    }

    fn config_fast_forward(&mut self, to: &str) -> anyhow::Result<()> {
        git_ok(&self.paths, false, &["merge", "-q", "--ff-only", to])?;
        tracing::info!(sha = to, "fast-forwarded the configuration repository");
        Ok(())
    }

    fn config_push(&mut self, url: &str, branch: &str) -> anyhow::Result<()> {
        let refspec = format!("HEAD:refs/heads/{branch}");
        git_ok(&self.paths, true, &["push", "-q", url, &refspec])?;
        Ok(())
    }

    fn config_show(&mut self, rev: &str, path: &str) -> anyhow::Result<Option<String>> {
        let spec = format!("{rev}:{path}");
        let out = git(&self.paths, false, &["show", &spec])?;
        if out.status.success() {
            return Ok(Some(String::from_utf8_lossy(&out.stdout).into_owned()));
        }
        let why = first_line(&out.stderr);
        if why.contains("does not exist") || why.contains("exists on disk, but not in") {
            return Ok(None);
        }
        anyhow::bail!("git show {spec}: {why}");
    }

    fn config_log(&mut self, n: usize) -> anyhow::Result<Vec<crate::config_repo::LogEntry>> {
        let count = format!("-n{n}");
        let out = git(
            &self.paths,
            false,
            &["log", &count, "--format=%H%x1f%cI%x1f%s"],
        )?;
        if !out.status.success() {
            // No commits yet, or not a repository: an empty history.
            return Ok(Vec::new());
        }
        Ok(String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|l| {
                let mut parts = l.splitn(3, '\x1f');
                Some(crate::config_repo::LogEntry {
                    sha: parts.next()?.to_string(),
                    when: parts.next()?.to_string(),
                    subject: parts.next().unwrap_or("").to_string(),
                })
            })
            .collect())
    }

    fn forgejo_request(
        &mut self,
        op: &crate::config_repo::ForgejoOp,
        secret: Option<&crate::setup::Secret>,
    ) -> anyhow::Result<(u16, String)> {
        let repo = crate::config_repo::RepoConfig::from_env()
            .context("LOSOS_CONFIG_REPO is not set; see modules/config-repo.nix")?;
        curl_forgejo(&self.paths, &repo, op, secret)
    }

    fn mint_secret(&mut self) -> anyhow::Result<crate::setup::Secret> {
        let mut buf = [0u8; 24];
        {
            use std::io::Read;
            std::fs::File::open("/dev/urandom")
                .and_then(|mut urandom| urandom.read_exact(&mut buf))
                .context("reading /dev/urandom")?;
        }
        let hex: String = buf.iter().map(|b| format!("{b:02x}")).collect();
        Ok(crate::setup::Secret::new(&hex))
    }

    fn load_sync_report(&mut self) -> anyhow::Result<Option<crate::config_repo::SyncReport>> {
        match std::fs::read_to_string(self.paths.sync_report_file()) {
            Ok(text) => Ok(serde_json::from_str(&text).ok()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(anyhow::Error::new(e).context("reading the sync report")),
        }
    }

    fn save_sync_report(&mut self, report: &crate::config_repo::SyncReport) -> anyhow::Result<()> {
        let text = serde_json::to_vec_pretty(report)?;
        atomic_write_secret(&self.paths.sync_report_file(), &text)
    }

    // ── Backups and erasing the box ─────────────────────────────────────
    fn backup_target(&mut self) -> anyhow::Result<Option<crate::backup::Target>> {
        let path = BackupFiles::from_env().target;
        match std::fs::read_to_string(&path) {
            // A file that no longer parses is no target: the owner sets it
            // again, rather than the pane failing to load.
            Ok(text) => Ok(serde_json::from_str(&text).ok()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(anyhow::Error::new(e).context("reading the backup target")),
        }
    }

    fn write_backup_target(
        &mut self,
        target: Option<&crate::backup::Target>,
    ) -> anyhow::Result<()> {
        let files = BackupFiles::from_env();
        match target {
            Some(t) => {
                atomic_write_secret(&files.env, t.env_file().as_bytes())?;
                atomic_write_secret(&files.target, &serde_json::to_vec_pretty(t)?)
            }
            None => {
                remove_if_present(&files.env)?;
                remove_if_present(&files.target)
            }
        }
    }

    fn start_backup_job(&mut self, kind: crate::backup::Kind, job: &str) -> anyhow::Result<()> {
        let files = BackupFiles::from_env();
        let script = match kind {
            crate::backup::Kind::Backup => &files.backup_script,
            crate::backup::Kind::Restore => &files.restore_script,
        };
        let Some(script) = script else {
            anyhow::bail!("this build has no {} script", kind.as_str());
        };
        // A fresh log per job, so the tail the owner is shown is this run's.
        std::fs::create_dir_all(&files.dir).context("creating the backup directory")?;
        std::fs::write(files.log(), b"").context("starting the backup log")?;
        let status = std::process::Command::new("systemd-run")
            .args(backup_launch_args(&files, kind, job, script))
            .status()
            .context("running systemd-run")?;
        anyhow::ensure!(status.success(), "systemd-run exited {status}");
        Ok(())
    }

    fn poll_backup_job(&mut self, kind: crate::backup::Kind, job: &str) -> supervisor::Poll {
        supervisor::poll_named(&kind.unit(job))
    }

    fn stop_backup_job(&mut self, kind: crate::backup::Kind, job: &str) -> anyhow::Result<()> {
        let status = std::process::Command::new("systemctl")
            .args(["stop", "--no-block", &kind.unit(job)])
            .status()
            .context("running systemctl stop")?;
        anyhow::ensure!(status.success(), "systemctl stop exited {status}");
        Ok(())
    }

    fn backup_log_tail(&mut self) -> String {
        supervisor::log_tail(&BackupFiles::from_env().log())
    }

    fn backup_report(&mut self) -> anyhow::Result<Option<crate::backup::Report>> {
        match std::fs::read_to_string(BackupFiles::from_env().report()) {
            Ok(text) => Ok(crate::backup::parse_report(&text)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(anyhow::Error::new(e).context("reading the backup report")),
        }
    }

    fn write_restore_code(&mut self, code: Option<&str>) -> anyhow::Result<()> {
        let path = BackupFiles::from_env().restore_code;
        match code {
            Some(c) => atomic_write_secret(&path, c.as_bytes()),
            None => remove_if_present(&path),
        }
    }

    fn take_restored_overrides(&mut self) -> anyhow::Result<Option<String>> {
        let path = BackupFiles::from_env().restored_overrides();
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                remove_if_present(&path)?;
                Ok(Some(text))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(anyhow::Error::new(e).context("reading the restored settings")),
        }
    }

    fn now(&mut self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs())
    }

    fn erase_grace_secs(&mut self) -> u64 {
        BackupFiles::from_env().grace_secs
    }

    fn wipe_and_reboot(&mut self) -> anyhow::Result<()> {
        let files = BackupFiles::from_env();
        // The marker is the whole instruction: whatever happens to this
        // process from here, the next boot wipes.
        atomic_write_secret(&files.wipe_marker, b"erase\n")?;
        let status = std::process::Command::new("systemctl")
            .args(["reboot", "--no-block"])
            .status()
            .context("running systemctl reboot")?;
        anyhow::ensure!(status.success(), "systemctl reboot exited {status}");
        Ok(())
    }

    fn write_erase_report(&mut self, report: &crate::erase::Outside) -> anyhow::Result<()> {
        atomic_write_secret(
            &BackupFiles::from_env().erase_report,
            &serde_json::to_vec_pretty(report)?,
        )
    }

    fn read_erase_report(&mut self) -> anyhow::Result<Option<crate::erase::Outside>> {
        match std::fs::read_to_string(BackupFiles::from_env().erase_report) {
            Ok(text) => Ok(serde_json::from_str(&text).ok()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(anyhow::Error::new(e).context("reading the erase report")),
        }
    }

    fn next_job_id(&mut self) -> anyhow::Result<String> {
        // The timestamp and the pid are both constant within one second of one
        // long-lived daemon, so they alone let two jobs collide — and a
        // collision means `systemd-run --unit=` fails with "Unit already
        // exists" *after* the command has already rewritten overrides.nix,
        // while `supervisor::finish` can no longer tell the two rebuilds apart.
        let seq = JOB_SEQ.fetch_add(1, Ordering::Relaxed);
        Ok(format!(
            "{}-{}-{}",
            chrono::Utc::now().format("%Y%m%d%H%M%S"),
            std::process::id(),
            seq
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Mode, Rebuild, RebuildState};

    fn tmpdir() -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "losos-io-test-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn atomic_write_creates_parents_and_leaves_no_temp_file() {
        let dir = tmpdir().join("nested/deeper");
        let target = dir.join("state.json");
        atomic_write(&target, b"hello").unwrap();
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "hello");
        let strays: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().contains("tmp"))
            .collect();
        assert!(strays.is_empty(), "temp file left behind: {strays:?}");
        std::fs::remove_dir_all(tmpdir()).ok();
    }

    #[test]
    fn state_file_from_before_the_claim_flag_reads_as_claimed() {
        let p = tmpdir().join("pre-claim.json");
        std::fs::write(&p, br#"{"mode":"local","sharing":false}"#).unwrap();
        assert!(read_state(&p).claimed);
        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn staged_secret_is_owned_through_the_fd_and_never_follows_a_symlink() {
        use std::os::unix::fs::MetadataExt;
        let dir = tmpdir().join("stage");
        std::fs::create_dir_all(&dir).unwrap();
        let victim = dir.join("victim");
        std::fs::write(&victim, b"untouched").unwrap();
        let staged = dir.join(".losos-setpass");
        // What a hostile pod would plant: the staged name already pointing at
        // a file it must not get.
        std::os::unix::fs::symlink(&victim, &staged).unwrap();
        let uid = std::fs::metadata(&dir).unwrap().uid();
        atomic_write_secret_owned(&staged, b"pw", uid).unwrap();
        let meta = std::fs::symlink_metadata(&staged).unwrap();
        assert!(
            meta.file_type().is_file(),
            "the symlink was replaced, not followed"
        );
        assert_eq!(meta.uid(), uid);
        assert_eq!(meta.mode() & 0o777, 0o600);
        assert_eq!(std::fs::read(&staged).unwrap(), b"pw");
        assert_eq!(std::fs::read(&victim).unwrap(), b"untouched");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn unreadable_state_path_reads_as_claimed() {
        // A directory where the file should be: read fails, but not NotFound.
        let p = tmpdir().join("state-is-a-dir.json");
        std::fs::create_dir_all(&p).unwrap();
        assert!(read_state(&p).claimed);
        std::fs::remove_dir_all(&p).ok();
    }

    #[test]
    fn dangling_state_symlink_reads_as_claimed() {
        let dir = tmpdir().join("dangling");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("state.json");
        std::os::unix::fs::symlink(dir.join("nowhere.json"), &p).unwrap();
        assert!(read_state(&p).claimed);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn explicit_unclaimed_on_disk_reads_as_claimed() {
        // Written by a build that serialised the old default on every save.
        let p = tmpdir().join("explicit-false.json");
        std::fs::write(&p, br#"{"mode":"local","sharing":false,"claimed":false}"#).unwrap();
        assert!(read_state(&p).claimed);
        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn missing_state_file_reads_as_default() {
        let p = tmpdir().join("does-not-exist.json");
        assert_eq!(read_state(&p), State::default());
    }

    #[test]
    fn corrupt_state_file_reads_as_default_but_claimed() {
        let p = tmpdir().join("corrupt.json");
        std::fs::write(&p, b"{ this is not json").unwrap();
        assert_eq!(
            read_state(&p),
            State {
                claimed: true,
                ..State::default()
            }
        );
        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn state_round_trips_through_the_file() {
        let p = tmpdir().join("round-trip.json");
        let s = State {
            mode: Mode::Mesh,
            sharing: true,
            rebuild: Some(Rebuild {
                job: "job-7".into(),
                state: RebuildState::Building,
                progress: 0,
                message: "rebuild started".into(),
            }),
            claimed: true,
            backup_job: None,
            erase: None,
        };
        write_state(&p, &s).unwrap();
        assert_eq!(read_state(&p), s);
        std::fs::remove_file(&p).ok();
    }
}
