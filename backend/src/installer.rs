//! `losos-ctl install` — the unattended appliance installer.
//!
//! This is the most destructive code in the project: it repartitions every
//! fixed disk it finds. The structure exists to keep that honest.
//!
//!   * Drive detection is [`detect_candidates`], a pure function over parsed
//!     `lsblk --json` output — testable with fixtures, no disks required.
//!   * The flow is a pure planner, [`plan_install`], producing an ordered
//!     [`InstallAction`] list, and a thin executor, [`execute`], that walks it
//!     through the [`Install`] trait. Tests assert on the *plan*, so ordering
//!     bugs surface without formatting anything.
//!
//! Several steps look redundant and are not. Each is load-bearing:
//!
//!   * The clone's `.git` is deleted, because nix's git fetcher exposes only
//!     *tracked* files to flake evaluation. `install-target.nix` is written
//!     into the clone afterwards, so with `.git` present it would be invisible
//!     and the build would silently fall back to the default target drive —
//!     formatting the wrong disk.
//!   * `disko` is passed `--yes-wipe-all-disks`, because its combined destroy
//!     mode otherwise prompts on stdin and an unattended run reads EOF and
//!     aborts.
//!   * `/mnt/persist/nix` is bind-mounted over `/mnt/nix` before installing,
//!     because the installed system's `/nix` is an impermanence bind from
//!     `/persist/nix`. Without it the store lands on the `/mnt` tmpfs and the
//!     first boot finds an empty store.
//!   * The keyfile is staged into `/mnt/etc/keys` *before* `nixos-install`,
//!     because the chrooted bootloader install resolves `boot.initrd.secrets`
//!     inside `/mnt` and hard-fails when the source is missing. Only on the
//!     keyfile path: on the TPM path that copy is what would put the key on
//!     the unencrypted ESP, so it is deliberately absent.
//!   * The TPM2 token is enrolled right after `disko`, before `nixos-install`,
//!     and always from the same random keyfile the volume was formatted with.
//!     Both modes format identically and unattended; the TPM path then adds a
//!     sealed token and keeps the keyfile only inside `/persist` (as the
//!     recovery slot `cryptsetup resize` and a future re-enrolment use), the
//!     keyfile path bakes it into the initrd. Enrolling before the long
//!     install means a chip that cannot seal fails the run in seconds, not
//!     after twenty minutes of copying.
//!   * With `--tang-edge`, the box's edge is a Tang server too, one only
//!     this box can name the owner of (edge/registrar/src/tang.rs). The CLI
//!     asks the edge for the key before anything is planned, and the plan
//!     carries the claim secret into `/persist` beside the keyfile.
//!   * With `--tang`, the TPM2 is one of two Clevis pins instead
//!     (modules/network-unlock.nix): the keyfile is sealed to the chip AND a
//!     Tang server into `persist.jwe`, no systemd TPM2 token is enrolled, and
//!     the JWE is staged into `/mnt/etc/keys` like the keyfile path's key,
//!     because the initrd carries it. It is ciphertext there, which is the
//!     difference.

use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Smallest disk worth installing onto (1 GB).
pub const MIN_BYTES: u64 = 1_000_000_000;

/// The block device carrying the LUKS header: the `persist` logical volume of
/// the `persist-vg` volume group modules/disko.nix declares. The TPM2 token is
/// enrolled here, not on the opened mapping.
pub const LUKS_HEADER_DEVICE: &str = "/dev/persist-vg/persist";

// ── Block devices ────────────────────────────────────────────────────────────

/// One node of the `lsblk --json` tree.
///
/// Children are kept so a disk can be rejected when *any* partition beneath it
/// is mounted — that is what stops the installer eating the disk it is running
/// from.
#[derive(Debug, Clone, Deserialize)]
pub struct BlockDev {
    pub name: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default, deserialize_with = "de_removable")]
    pub rm: bool,
    #[serde(rename = "type", default)]
    pub dev_type: String,
    #[serde(default, deserialize_with = "de_mountpoints", alias = "mountpoint")]
    pub mountpoints: Vec<String>,
    #[serde(default)]
    pub children: Vec<BlockDev>,
}

/// `rm` is a JSON boolean in util-linux >= 2.39 and an integer in older
/// releases. Accept either; anything else is treated as "not removable".
fn de_removable<'de, D>(d: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(match serde_json::Value::deserialize(d)? {
        serde_json::Value::Bool(b) => b,
        serde_json::Value::Number(n) => n.as_i64().unwrap_or(0) != 0,
        _ => false,
    })
}

/// `mountpoints` is an array that may contain nulls (a partition with no
/// filesystem); older lsblk emits a single `mountpoint` string instead. Both
/// shapes collapse to "the mountpoints that actually exist".
fn de_mountpoints<'de, D>(d: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(match serde_json::Value::deserialize(d)? {
        serde_json::Value::Array(xs) => xs
            .into_iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect(),
        serde_json::Value::String(s) => vec![s],
        _ => Vec::new(),
    })
}

/// The `blockdevices` wrapper `lsblk --json` emits.
#[derive(Debug, Deserialize)]
pub struct LsblkOutput {
    pub blockdevices: Vec<BlockDev>,
}

/// Whether this device or anything beneath it is mounted.
fn has_mountpoint(b: &BlockDev) -> bool {
    !b.mountpoints.is_empty() || b.children.iter().any(has_mountpoint)
}

/// The drive filter: whole disks, not removable, over [`MIN_BYTES`], with
/// nothing mounted anywhere in their subtree.
///
/// Names come back `/dev/`-prefixed, which is the form written into
/// `install-target.nix`.
pub fn detect_candidates(devs: &[BlockDev]) -> Vec<String> {
    devs.iter()
        .filter(|b| b.dev_type == "disk" && !b.rm && b.size > MIN_BYTES && !has_mountpoint(b))
        .map(|b| format!("/dev/{}", b.name))
        .collect()
}

// ── Rendering ────────────────────────────────────────────────────────────────

/// Render `modules/install-target.nix`.
///
/// The format is asserted on by the installer VM test, so it is exact.
pub fn render_target(
    drives: &[String],
    tpm: bool,
    tang: &[Tang],
    bios: bool,
    gpg_keys: &[String],
) -> String {
    let list = drives
        .iter()
        .map(|d| format!("\"{d}\""))
        .collect::<Vec<_>>()
        .join(" ");
    let mut s = String::new();
    for line in [
        "# Generated by losos-install -- do not hand-edit; re-run losos-install",
        "# to change the target drives, TPM mode or boot firmware. This file is imported by",
        "# flake.nix only when it exists (builtins.pathExists), so it stays out",
        "# of the admin app's modules/overrides.nix (which losos-ctl apply",
        "# rewrites wholesale) and out of any published flake. The installer",
        "# commits it into the local /persist/etc/nixos git repo so the default",
        "# git+file:///etc/nixos auto-upgrade keeps seeing it.",
        "{ ... }:",
        "",
        "{",
    ] {
        s.push_str(line);
        s.push('\n');
    }
    s.push_str(&format!("  losos.targetDrives = [ {list} ];\n"));
    s.push_str(&format!(
        "  losos.tpm.enable = {};\n",
        if tpm { "true" } else { "false" }
    ));
    // Only when there are any, like the keys below. parse_tang has already
    // refused anything that could end the string.
    if !tang.is_empty() {
        s.push_str("  losos.tpm.tang = [\n");
        for t in tang {
            s.push_str(&format!(
                "    {{ url = \"{}\"; thumbprint = \"{}\";{} }}\n",
                t.url,
                t.thumbprint,
                if t.edge { " edge = true;" } else { "" }
            ));
        }
        s.push_str("  ];\n");
    }
    s.push_str(&format!(
        "  losos.bios = {};\n",
        if bios { "true" } else { "false" }
    ));
    // Only when there are any, so the file a stock medium writes is the one
    // the installer VM test asserts on, byte for byte.
    if !gpg_keys.is_empty() {
        s.push_str("  # Seeded for the losos terminal client; see docs/terminal-access.md.\n");
        s.push_str("  losos.access.gpgKeys = [\n");
        for key in gpg_keys {
            s.push_str("    ''\n");
            for line in key.trim().lines() {
                s.push_str("      ");
                s.push_str(line.trim_end());
                s.push('\n');
            }
            s.push_str("    ''\n");
        }
        s.push_str("  ];\n");
    }
    s.push_str("}\n");
    s
}

/// Why an armoured key cannot go into a Nix indented string, if it cannot.
/// Armour is base64 and header lines, so neither ever appears in a real one.
pub fn gpg_key_fault(armored: &str) -> Option<&'static str> {
    if armored.contains("''") || armored.contains("${") {
        Some("it holds '' or ${, which no armoured key does")
    } else if !armored.contains("-----BEGIN PGP PUBLIC KEY BLOCK-----") {
        Some("it is not an armoured public key (gpg --armor --export <id>)")
    } else {
        None
    }
}

/// A Tang server the disk key is bound to, from `--tang URL#THUMBPRINT`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tang {
    pub url: String,
    /// The thumbprint of the server's signing key (`tang-show-keys`). Clevis
    /// checks the server's advertisement against it, so nothing is trusted on
    /// first use.
    pub thumbprint: String,
    /// The box's edge, which made this key for this box alone and stops
    /// answering when the owner reports the box stolen. Required on its own
    /// in [`clevis_policy`], not one of the "any one will do" servers.
    pub edge: bool,
}

/// Parse `URL#THUMBPRINT`. The same shapes `losos.tpm.tang` accepts
/// (modules/options.nix), checked here so a typo stops the install before any
/// disk is touched rather than at the rebuild.
pub fn parse_tang(arg: &str) -> Result<Tang, String> {
    let (url, thumbprint) = arg
        .rsplit_once('#')
        .ok_or_else(|| format!("--tang {arg}: expected URL#THUMBPRINT (tang-show-keys on the server prints the thumbprint)"))?;
    let rest = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
        .ok_or_else(|| format!("--tang {arg}: the address must start with http:// or https://"))?;
    if rest.is_empty()
        || url
            .chars()
            .any(|c| c.is_whitespace() || matches!(c, '"' | '\'' | '$' | '`' | '\\'))
    {
        return Err(format!("--tang {arg}: not a usable address"));
    }
    if thumbprint.len() < 20
        || !thumbprint
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(format!(
            "--tang {arg}: the thumbprint is the base64url string tang-show-keys prints"
        ));
    }
    Ok(Tang {
        url: url.to_string(),
        thumbprint: thumbprint.to_string(),
        edge: false,
    })
}

/// An edge address for `--tang-edge`: http(s), no characters that could end
/// a Nix string or a shell word, no trailing slash.
pub fn edge_base(url: &str) -> Result<String, String> {
    let url = url.trim().trim_end_matches('/');
    let rest = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
        .ok_or_else(|| {
            format!("--tang-edge {url}: the edge's address must start with http:// or https://")
        })?;
    if rest.is_empty()
        || url
            .chars()
            .any(|c| c.is_whitespace() || matches!(c, '"' | '\'' | '$' | '`' | '\\' | '#' | '?'))
    {
        return Err(format!("--tang-edge {url}: not a usable address"));
    }
    Ok(url.to_string())
}

/// The edge's answer to `POST /tang/keys`: this box's id there and the
/// thumbprint of its new signing key. Both are checked for shape, since they
/// go into install-target.nix.
pub fn edge_tang(base: &str, answer: &str) -> Result<Tang, String> {
    #[derive(Deserialize)]
    struct Answer {
        id: String,
        thumbprint: String,
    }
    let a: Answer = serde_json::from_str(answer)
        .map_err(|e| format!("the edge's Tang answer is not what it should be: {e}"))?;
    if a.id.len() != 32 || !a.id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
        return Err("the edge's Tang answer carries no box id".to_string());
    }
    let t = parse_tang(&format!("{base}/tang/{}#{}", a.id, a.thumbprint))?;
    Ok(Tang { edge: true, ..t })
}

/// The Clevis policy: the TPM2 (no PCRs, for the reason [`LUKS_HEADER_DEVICE`]'s
/// enrolment gives) AND any one of the home network's Tang servers AND the
/// edge, each of the last two only when there is one. The edge is a pin of
/// its own: as one of "any one will do" it would answer anywhere and make
/// the home servers pointless.
pub fn clevis_policy(tang: &[Tang]) -> String {
    let pin = |t: &Tang| serde_json::json!({ "url": t.url, "thp": t.thumbprint });
    let home: Vec<serde_json::Value> = tang.iter().filter(|t| !t.edge).map(pin).collect();
    let edge: Vec<serde_json::Value> = tang.iter().filter(|t| t.edge).map(pin).collect();
    let mut pins = serde_json::Map::new();
    pins.insert("tpm2".into(), serde_json::json!({}));
    if !home.is_empty() {
        pins.insert(
            "sss".into(),
            serde_json::json!({ "t": 1, "pins": { "tang": home } }),
        );
    }
    if !edge.is_empty() {
        pins.insert("tang".into(), serde_json::Value::Array(edge.clone()));
    }
    let t = 1 + usize::from(!home.is_empty()) + edge.len();
    serde_json::json!({ "t": t, "pins": pins }).to_string()
}

// ── Options and the plan ─────────────────────────────────────────────────────

/// Everything the planner needs. The path and URL fields are environment
/// overridable and filled in by the CLI layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    pub tpm: bool,
    /// Tang servers the TPM2 shares the key with. Empty: the TPM2 alone.
    pub tang: Vec<Tang>,
    /// Install for legacy BIOS (GRUB) rather than UEFI (systemd-boot). Resolved
    /// by the CLI layer from the running firmware unless forced.
    pub bios: bool,
    pub drives: Option<Vec<String>>,
    pub no_install: bool,
    pub disko_script: Option<PathBuf>,
    pub emit_target: Option<PathBuf>,
    pub flake_url: String,
    /// The branch to clone: the release channel the box will follow
    /// (`losos.configRepo.upstream.branch`), so a new box starts on the
    /// version its first nightly update would take. `None` is the
    /// repository's default branch.
    pub flake_branch: Option<String>,
    pub flake_work: PathBuf,
    pub keyfile: PathBuf,
    pub target_rel: PathBuf,
    pub emit_file: PathBuf,
    /// The owner's GPG public keys, armoured, for terminal access
    /// (`losos.access.gpgKeys`). Checked by the CLI layer before they get
    /// here; empty on a stock medium.
    pub gpg_keys: Vec<String>,
    /// The GitHub App problem reports sign in with, from the medium's own
    /// credential (`report-client-id`), handed to the installed box. None
    /// on a medium built without one.
    pub report_client_id: Option<String>,
}

/// Where the installer leaves the report client ID for the installed box:
/// a `legacy` path of its credential (modules/problems.nix), so the box's
/// first boot seals it into /etc/credstore.encrypted and deletes this copy.
pub const REPORT_CLIENT_ID_HANDOVER: &str = "/mnt/persist/var/lib/losos/report-client-id";

/// One side effect. Keeping these as data rather than calling straight into the
/// world is what makes the ordering testable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallAction {
    Log(String),
    WriteTarget(PathBuf, String),
    EnsureKeyfile(PathBuf),
    RunDiskoScript(PathBuf),
    /// URL, branch (`None`: the default one), directory.
    CloneFlake(String, Option<String>, PathBuf),
    RunDisko(PathBuf),
    BindMount(PathBuf, PathBuf),
    RunNixosInstall(PathBuf),
    LayFlake(PathBuf, PathBuf),
    CopyKeyfile(PathBuf, PathBuf),
    /// Seal a LUKS2 token to the TPM2 on `device`, authenticating with the
    /// keyfile the volume was formatted with. Never prompts.
    EnrollTpm(PathBuf, PathBuf),
    /// Seal the keyfile (first) with Clevis under the policy (third) into a
    /// JWE (second): the TPM2 and a Tang server.
    BindTang(PathBuf, PathBuf, String),
}

/// What a keyfile install costs, in one line the owner sees on the console. The
/// handbook page says the rest; the admin pages and the box's own screen say
/// the same thing after the first boot.
pub const NO_TPM_WARNING: &str = "losos-install: warning: no TPM, so the disk key is on the unencrypted boot partition and anyone with this disk can read it. See https://losos.dasmat.us/reference/tpm";

/// `--tpm` / `--no-tpm` against what the installer medium can see.
///
/// Neither flag means "use the chip if there is one": TPM2 is the default
/// wherever it exists, the keyfile is what a box without one gets. `--tpm` on
/// a machine with no chip is an error rather than a silent keyfile install,
/// because the owner asked for a property the result would not have.
pub fn resolve_tpm(explicit: Option<bool>, present: bool) -> Result<bool, String> {
    match explicit {
        Some(true) if !present => Err(
            "--tpm given but no TPM2 device (/dev/tpmrm0 or /dev/tpm0) is visible; enable the TPM in the firmware or install with --no-tpm"
                .to_string(),
        ),
        Some(choice) => Ok(choice),
        None => Ok(present),
    }
}

/// Turn options plus detected devices into an ordered action list.
///
/// Three flows, in the order the flags are checked: `--emit-target` (write the
/// file and stop), `--disko-script` (format and mount with a prebuilt script,
/// used by the VM test), and the real install.
pub fn plan_install(opts: &Options, devs: &[BlockDev]) -> Result<Vec<InstallAction>, String> {
    if opts.tang.iter().filter(|t| t.edge).count() > 1 {
        return Err("--tang-edge names one edge".to_string());
    }
    if !opts.tang.is_empty() && !opts.tpm {
        return Err(
            "--tang needs the TPM2: the disk key is sealed to the chip AND a Tang server"
                .to_string(),
        );
    }
    let drives = resolve_drives(opts, devs)?;
    let target = render_target(&drives, opts.tpm, &opts.tang, opts.bios, &opts.gpg_keys);
    let mut acts = vec![InstallAction::Log(format!(
        "losos-install: target drives: {}",
        drives.join(" ")
    ))];

    if let Some(f) = &opts.emit_target {
        acts.push(InstallAction::WriteTarget(f.clone(), target));
        acts.push(InstallAction::Log(format!(
            "losos-install: wrote {}",
            f.display()
        )));
        return Ok(acts);
    }

    acts.push(InstallAction::Log(if !opts.tang.is_empty() {
        format!(
            "losos-install: unlock: TPM2 and a Tang server on this network ({}); recovery key kept inside /persist",
            opts.tang.iter().map(|t| t.url.as_str()).collect::<Vec<_>>().join(", ")
        )
    } else if opts.tpm {
        "losos-install: unlock: TPM2 (disk key sealed to the chip; recovery key kept inside /persist)"
            .to_string()
    } else {
        "losos-install: unlock: keyfile in the initrd (no TPM2 chip, or --no-tpm)".to_string()
    }));
    if !opts.tpm {
        acts.push(InstallAction::Log(NO_TPM_WARNING.to_string()));
    }

    // The keyfile must exist before disko runs: it is the LUKS passwordFile in
    // both modes. The TPM path formats with it too and enrols the chip from
    // it, which is what makes that path unattended.
    acts.push(InstallAction::EnsureKeyfile(opts.keyfile.clone()));

    if let Some(script) = &opts.disko_script {
        acts.push(InstallAction::WriteTarget(opts.emit_file.clone(), target));
        acts.push(InstallAction::RunDiskoScript(script.clone()));
        acts.extend(enroll(opts));
        acts.push(InstallAction::Log(
            "losos-install: format+mount complete (test mode, no install).".to_string(),
        ));
        return Ok(acts);
    }

    let work = &opts.flake_work;
    acts.push(InstallAction::CloneFlake(
        opts.flake_url.clone(),
        opts.flake_branch.clone(),
        work.clone(),
    ));
    acts.push(InstallAction::WriteTarget(
        work.join(&opts.target_rel),
        target,
    ));
    acts.push(InstallAction::RunDisko(work.clone()));
    acts.extend(enroll(opts));
    acts.extend(install_tail(opts, work));
    Ok(acts)
}

/// Where the Clevis JWE is staged on the medium: beside the keyfile.
pub fn staged_jwe(opts: &Options) -> PathBuf {
    opts.keyfile.with_file_name("persist.jwe")
}

/// Where the box's key on its edge is staged, `{"url", "claim"}`: the
/// address and the secret that names this box's owner to it. Beside the
/// keyfile, and kept beside it inside `/persist`, where
/// `losos-tang-edge-owner` (modules/network-unlock.nix) reads it.
pub fn staged_claim(keyfile: &Path) -> PathBuf {
    keyfile.with_file_name("tang-edge.json")
}

/// The TPM2 enrolment, on the TPM path only. Runs against the freshly
/// formatted header, before anything slow. With Tang servers it is the
/// Clevis binding instead, which asks a server now: one that cannot be
/// reached, or answers with another key, fails the run here.
fn enroll(opts: &Options) -> Vec<InstallAction> {
    if !opts.tpm {
        return Vec::new();
    }
    if !opts.tang.is_empty() {
        return vec![
            InstallAction::Log(
                "losos-install: sealing the disk key to the TPM2 and a Tang server (clevis)"
                    .to_string(),
            ),
            InstallAction::BindTang(
                opts.keyfile.clone(),
                staged_jwe(opts),
                clevis_policy(&opts.tang),
            ),
        ];
    }
    vec![
        InstallAction::Log(
            "losos-install: sealing the disk key to the TPM2 (systemd-cryptenroll)".to_string(),
        ),
        InstallAction::EnrollTpm(PathBuf::from(LUKS_HEADER_DEVICE), opts.keyfile.clone()),
    ]
}

/// The steps after the disks are formatted and mounted.
fn install_tail(opts: &Options, work: &Path) -> Vec<InstallAction> {
    // The installed system's /nix is an impermanence bind from /persist/nix,
    // and disko mounts only its own devices — so without this the store would
    // be written to the throwaway /mnt tmpfs.
    let mut acts = vec![InstallAction::BindMount(
        PathBuf::from("/mnt/persist/nix"),
        PathBuf::from("/mnt/nix"),
    )];

    if opts.no_install {
        acts.push(InstallAction::Log(
            "losos-install: --no-install set; stopping after disko. /mnt is ready.".to_string(),
        ));
        return acts;
    }

    if !opts.tpm {
        // The chroot copy, before nixos-install: the bootloader install reads
        // it when it bakes the secret into the initrd. Keyfile path only — on
        // the TPM path this copy is exactly what would put the key on the
        // unencrypted ESP, and boot.nix declares no initrd secret there.
        acts.push(InstallAction::CopyKeyfile(
            opts.keyfile.clone(),
            PathBuf::from("/mnt/etc/keys/persist-keyfile"),
        ));
    }
    // The /persist copy, in both modes, inside the encrypted volume. On the
    // keyfile path later rebuilds on the running box read it when they
    // regenerate the initrd; on the TPM path it is the recovery slot:
    // `losos-ctl grow` authenticates `cryptsetup resize` with it, and it is
    // what a re-enrolment after a cleared TPM would start from. It is only
    // reachable once the volume is open, which is the point.
    acts.push(InstallAction::CopyKeyfile(
        opts.keyfile.clone(),
        PathBuf::from("/mnt/persist/etc/keys/persist-keyfile"),
    ));
    if !opts.tang.is_empty() {
        // The JWE, twice for the same two reasons: the chroot copy is what
        // the bootloader install bakes into the initrd, the /persist copy is
        // what later rebuilds bake (/etc/keys is persisted).
        for dst in [
            "/mnt/etc/keys/persist.jwe",
            "/mnt/persist/etc/keys/persist.jwe",
        ] {
            acts.push(InstallAction::CopyKeyfile(
                staged_jwe(opts),
                PathBuf::from(dst),
            ));
        }
    }
    if opts.tang.iter().any(|t| t.edge) {
        // Read only by the running box, so only the /persist copy.
        acts.push(InstallAction::CopyKeyfile(
            staged_claim(&opts.keyfile),
            PathBuf::from("/mnt/persist/etc/keys/tang-edge.json"),
        ));
    }

    acts.push(InstallAction::RunNixosInstall(work.to_path_buf()));
    if let Some(id) = &opts.report_client_id {
        acts.push(InstallAction::WriteTarget(
            PathBuf::from(REPORT_CLIENT_ID_HANDOVER),
            id.clone(),
        ));
    }
    acts.push(InstallAction::LayFlake(
        work.to_path_buf(),
        PathBuf::from("/mnt/persist/etc/nixos"),
    ));
    // Said again as the last thing before `done`, because that is what stays
    // on the screen: the line at the top has scrolled away by then.
    if !opts.tpm {
        acts.push(InstallAction::Log(NO_TPM_WARNING.to_string()));
    }
    acts.push(InstallAction::Log(
        "losos-install: done. Remove the install medium and reboot into the installed system."
            .to_string(),
    ));
    acts
}

/// The explicit drive list, or auto-detection.
pub fn resolve_drives(opts: &Options, devs: &[BlockDev]) -> Result<Vec<String>, String> {
    match &opts.drives {
        Some(ds) if !ds.is_empty() => Ok(ds.clone()),
        Some(_) => Err("--drives given but empty".to_string()),
        None => {
            let cs = detect_candidates(devs);
            if cs.is_empty() {
                Err("no candidate fixed disks found (whole disks, non-removable, >1GB, not mounted); pass --drives to override".to_string())
            } else {
                Ok(cs)
            }
        }
    }
}

// ── The effect trait ─────────────────────────────────────────────────────────

/// The effects the executor needs.
///
/// There is deliberately no `read_block_devices` method: enumeration is an
/// input to the pure planner, not something the fake should have to simulate.
pub trait Install {
    fn write_file_atomic(&mut self, path: &Path, text: &str) -> anyhow::Result<()>;
    fn ensure_keyfile(&mut self, path: &Path) -> anyhow::Result<()>;
    fn run_disko_script(&mut self, path: &Path) -> anyhow::Result<()>;
    fn clone_flake(&mut self, url: &str, branch: Option<&str>, work: &Path) -> anyhow::Result<()>;
    fn run_disko(&mut self, work: &Path) -> anyhow::Result<()>;
    fn bind_mount(&mut self, src: &Path, dst: &Path) -> anyhow::Result<()>;
    fn run_nixos_install(&mut self, work: &Path) -> anyhow::Result<()>;
    fn lay_flake(&mut self, work: &Path, dest: &Path) -> anyhow::Result<()>;
    fn copy_keyfile(&mut self, src: &Path, dst: &Path) -> anyhow::Result<()>;
    fn enroll_tpm(&mut self, device: &Path, keyfile: &Path) -> anyhow::Result<()>;
    fn bind_tang(&mut self, keyfile: &Path, jwe: &Path, policy: &str) -> anyhow::Result<()>;
    fn log_info(&mut self, msg: &str);
}

/// Walk an action list through the trait.
pub fn execute<I: Install>(installer: &mut I, acts: &[InstallAction]) -> anyhow::Result<()> {
    for a in acts {
        match a {
            InstallAction::Log(t) => installer.log_info(t),
            InstallAction::WriteTarget(p, t) => installer.write_file_atomic(p, t)?,
            InstallAction::EnsureKeyfile(p) => installer.ensure_keyfile(p)?,
            InstallAction::RunDiskoScript(p) => installer.run_disko_script(p)?,
            InstallAction::CloneFlake(u, b, d) => installer.clone_flake(u, b.as_deref(), d)?,
            InstallAction::RunDisko(d) => installer.run_disko(d)?,
            InstallAction::BindMount(s, d) => installer.bind_mount(s, d)?,
            InstallAction::RunNixosInstall(d) => installer.run_nixos_install(d)?,
            InstallAction::LayFlake(s, d) => installer.lay_flake(s, d)?,
            InstallAction::CopyKeyfile(s, d) => installer.copy_keyfile(s, d)?,
            InstallAction::EnrollTpm(dev, key) => installer.enroll_tpm(dev, key)?,
            InstallAction::BindTang(key, jwe, policy) => installer.bind_tang(key, jwe, policy)?,
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Options {
        Options {
            tpm: false,
            tang: Vec::new(),
            bios: false,
            drives: None,
            no_install: false,
            disko_script: None,
            emit_target: None,
            flake_url: "https://github.com/dasmatus/losos.git".into(),
            flake_branch: Some("channel/daily".into()),
            flake_work: PathBuf::from("/tmp/losos-flake"),
            keyfile: PathBuf::from("/etc/keys/persist-keyfile"),
            target_rel: PathBuf::from("modules/install-target.nix"),
            emit_file: PathBuf::from("/tmp/losos-install-target.nix"),
            gpg_keys: Vec::new(),
            report_client_id: None,
        }
    }

    #[test]
    fn the_report_client_id_goes_to_the_box_only_when_the_medium_has_one() {
        let handover = |a: &InstallAction| matches!(a, InstallAction::WriteTarget(p, _) if p == Path::new(REPORT_CLIENT_ID_HANDOVER));
        let acts = plan_install(&opts(), &targets()).unwrap();
        assert!(!acts.iter().any(handover));

        let o = Options {
            report_client_id: Some("Iv23test".into()),
            ..opts()
        };
        let acts = plan_install(&o, &targets()).unwrap();
        let at = |f: &dyn Fn(&InstallAction) -> bool| acts.iter().position(f).unwrap();
        let written = at(&handover);
        // Into /persist once nixos-install has made the tree, never onto
        // the throwaway /mnt tmpfs.
        assert!(at(&|a| matches!(a, InstallAction::RunNixosInstall(_))) < written);
        assert!(matches!(&acts[written], InstallAction::WriteTarget(_, id) if id == "Iv23test"));
        assert!(REPORT_CLIENT_ID_HANDOVER.starts_with("/mnt/persist/"));
    }

    fn disk(name: &str, size: u64, children: Vec<BlockDev>) -> BlockDev {
        BlockDev {
            name: name.into(),
            size,
            rm: false,
            dev_type: "disk".into(),
            mountpoints: vec![],
            children,
        }
    }

    fn part(name: &str, mps: &[&str]) -> BlockDev {
        BlockDev {
            name: name.into(),
            size: GI,
            rm: false,
            dev_type: "part".into(),
            mountpoints: mps.iter().map(|s| s.to_string()).collect(),
            children: vec![],
        }
    }

    const GI: u64 = 1024 * 1024 * 1024;

    fn targets() -> Vec<BlockDev> {
        vec![
            disk("vda", 8 * GI, vec![part("vda1", &["/"])]),
            disk("vdb", 2 * GI, vec![]),
            disk("vdc", 2 * GI, vec![]),
            disk("vdd", 2 * GI, vec![]),
        ]
    }

    fn index_of(acts: &[InstallAction], pred: impl Fn(&InstallAction) -> bool) -> usize {
        acts.iter().position(pred).unwrap_or(acts.len())
    }

    #[test]
    fn finds_the_empty_disks_and_skips_the_mounted_root() {
        assert_eq!(
            detect_candidates(&targets()),
            vec!["/dev/vdb", "/dev/vdc", "/dev/vdd"]
        );
    }

    #[test]
    fn excludes_removable_tiny_and_non_disk_devices() {
        let mut removable = disk("sda", 2 * GI, vec![]);
        removable.rm = true;
        let tiny = disk("sdb", 500 * 1024 * 1024, vec![]);
        let mut loop0 = disk("loop0", 4 * GI, vec![]);
        loop0.dev_type = "loop".into();

        let mut devs = targets();
        devs.extend([removable, tiny, loop0]);
        assert_eq!(
            detect_candidates(&devs),
            vec!["/dev/vdb", "/dev/vdc", "/dev/vdd"]
        );
    }

    #[test]
    fn accepts_boolean_rm_from_modern_lsblk() {
        let json = r#"{"blockdevices":[
            {"name":"loop0","size":1876725760,"rm":false,"type":"loop","mountpoints":["/nix/.ro-store"]},
            {"name":"nvme0n1","size":512110190592,"rm":false,"type":"disk","mountpoints":[],"children":[
              {"name":"nvme0n1p1","size":512110190592,"rm":false,"type":"part","mountpoints":["/"]}]},
            {"name":"sdb","size":2000398934016,"rm":false,"type":"disk","mountpoints":[]},
            {"name":"sdc","size":2000398934016,"rm":true,"type":"disk","mountpoints":[]}]}"#;
        let out: LsblkOutput = serde_json::from_str(json).unwrap();
        assert_eq!(detect_candidates(&out.blockdevices), vec!["/dev/sdb"]);
    }

    #[test]
    fn accepts_legacy_integer_rm() {
        let json = r#"{"blockdevices":[
            {"name":"sdb","size":2000398934016,"rm":0,"type":"disk","mountpoints":[]},
            {"name":"sdc","size":2000398934016,"rm":1,"type":"disk","mountpoints":[]}]}"#;
        let out: LsblkOutput = serde_json::from_str(json).unwrap();
        assert_eq!(detect_candidates(&out.blockdevices), vec!["/dev/sdb"]);
    }

    #[test]
    fn tolerates_null_mountpoints_and_the_legacy_singular_key() {
        let json = r#"{"blockdevices":[
            {"name":"sdb","size":2000398934016,"rm":0,"type":"disk","mountpoints":[null]},
            {"name":"sdc","size":2000398934016,"rm":0,"type":"disk","mountpoint":"/boot"}]}"#;
        let out: LsblkOutput = serde_json::from_str(json).unwrap();
        // sdb's null mountpoint means "unmounted"; sdc really is mounted.
        assert_eq!(detect_candidates(&out.blockdevices), vec!["/dev/sdb"]);
    }

    #[test]
    fn seeded_keys_go_in_as_indented_strings() {
        let key = "-----BEGIN PGP PUBLIC KEY BLOCK-----\n\nmDMEZ\n=abcd\n-----END PGP PUBLIC KEY BLOCK-----\n";
        let t = render_target(&["/dev/vdb".into()], true, &[], false, &[key.into()]);
        assert!(t.contains(
            "  losos.access.gpgKeys = [\n    ''\n      -----BEGIN PGP PUBLIC KEY BLOCK-----\n"
        ));
        assert!(t.contains("      -----END PGP PUBLIC KEY BLOCK-----\n    ''\n  ];\n}\n"));
        // No keys, no line: a stock medium's file is unchanged.
        assert!(!render_target(&["/dev/vdb".into()], true, &[], false, &[]).contains("gpgKeys"));
        assert!(gpg_key_fault(key).is_none());
        assert!(gpg_key_fault("${builtins.exec}").is_some());
        assert!(gpg_key_fault("-----BEGIN PGP PUBLIC KEY BLOCK----- ''").is_some());
        assert!(gpg_key_fault("-----BEGIN PGP PRIVATE KEY BLOCK-----").is_some());
    }

    #[test]
    fn render_target_is_exact() {
        let t = render_target(
            &["/dev/vdb".into(), "/dev/vdc".into()],
            false,
            &[],
            false,
            &[],
        );
        assert!(t.contains(r#"  losos.targetDrives = [ "/dev/vdb" "/dev/vdc" ];"#));
        assert!(t.contains("  losos.tpm.enable = false;"));
        assert!(t.contains("{ ... }:"));
        assert!(t.ends_with("}\n"));
        assert!(t.contains("  losos.bios = false;"));
        assert!(
            render_target(&["/dev/nvme0n1".into()], true, &[], false, &[])
                .contains("losos.tpm.enable = true;")
        );
        assert!(render_target(&["/dev/vdb".into()], false, &[], true, &[])
            .contains("  losos.bios = true;"));
    }

    #[test]
    fn emit_target_writes_and_stops_before_touching_disks() {
        let o = Options {
            emit_target: Some(PathBuf::from("/tmp/detected.nix")),
            ..opts()
        };
        let acts = plan_install(&o, &targets()).unwrap();
        assert!(acts
            .iter()
            .any(|a| matches!(a, InstallAction::WriteTarget(..))));
        for bad in acts.iter() {
            assert!(!matches!(
                bad,
                InstallAction::RunDisko(_)
                    | InstallAction::RunDiskoScript(_)
                    | InstallAction::CloneFlake(..)
                    | InstallAction::RunNixosInstall(_)
            ));
        }
    }

    #[test]
    fn disko_script_ensures_the_keyfile_and_skips_the_install() {
        let o = Options {
            disko_script: Some(PathBuf::from("/etc/losos/disko-script")),
            ..opts()
        };
        let acts = plan_install(&o, &targets()).unwrap();
        assert!(acts
            .iter()
            .any(|a| matches!(a, InstallAction::EnsureKeyfile(_))));
        assert!(acts
            .iter()
            .any(|a| matches!(a, InstallAction::RunDiskoScript(_))));
        assert!(!acts
            .iter()
            .any(|a| matches!(a, InstallAction::RunNixosInstall(_))));
    }

    #[test]
    fn tpm_mode_keeps_the_keyfile_off_the_esp_and_enrols_before_installing() {
        let o = Options {
            tpm: true,
            ..opts()
        };
        let acts = plan_install(&o, &targets()).unwrap();
        // Both modes format from the same random keyfile, unattended.
        assert!(acts
            .iter()
            .any(|a| matches!(a, InstallAction::EnsureKeyfile(_))));
        // The chroot copy is the one the bootloader install bakes into the
        // initrd on the ESP; it must not exist on the TPM path.
        let copies: Vec<&PathBuf> = acts
            .iter()
            .filter_map(|a| match a {
                InstallAction::CopyKeyfile(_, dst) => Some(dst),
                _ => None,
            })
            .collect();
        assert_eq!(
            copies,
            vec![&PathBuf::from("/mnt/persist/etc/keys/persist-keyfile")]
        );
        // The token is sealed from that keyfile, on the header device, after
        // the format and before the slow install.
        let pos = |f: &dyn Fn(&InstallAction) -> bool| acts.iter().position(f).unwrap();
        let disko = pos(&|a| matches!(a, InstallAction::RunDisko(_)));
        let enrol = pos(&|a| matches!(a, InstallAction::EnrollTpm(..)));
        let install = pos(&|a| matches!(a, InstallAction::RunNixosInstall(_)));
        assert!(disko < enrol && enrol < install, "{acts:?}");
        assert_eq!(
            acts[enrol],
            InstallAction::EnrollTpm(
                PathBuf::from(LUKS_HEADER_DEVICE),
                PathBuf::from("/etc/keys/persist-keyfile")
            )
        );
        // ...and the store still has to be bound through /persist.
        assert!(acts
            .iter()
            .any(|a| matches!(a, InstallAction::BindMount(..))));
    }

    fn home_tang() -> Tang {
        parse_tang("http://tang.lan:7500#x3ZDsT7uAzTcJiJ9qZ7f0wJ8Kq0pEJ2O5b5b7l7XyZk").unwrap()
    }

    #[test]
    fn tang_mode_binds_with_clevis_instead_of_a_token_and_stages_the_jwe() {
        let o = Options {
            tpm: true,
            tang: vec![home_tang()],
            ..opts()
        };
        let acts = plan_install(&o, &targets()).unwrap();
        // No systemd TPM2 token: disko.nix leaves tpm2-device= off crypttab
        // in this mode, and a token nobody reads is one more way in.
        assert!(!acts
            .iter()
            .any(|a| matches!(a, InstallAction::EnrollTpm(..))));
        let pos = |f: &dyn Fn(&InstallAction) -> bool| acts.iter().position(f).unwrap();
        let disko = pos(&|a| matches!(a, InstallAction::RunDisko(_)));
        let bind = pos(&|a| matches!(a, InstallAction::BindTang(..)));
        let install = pos(&|a| matches!(a, InstallAction::RunNixosInstall(_)));
        assert!(disko < bind && bind < install, "{acts:?}");
        let InstallAction::BindTang(key, jwe, policy) = &acts[bind] else {
            unreachable!()
        };
        assert_eq!(key, &PathBuf::from("/etc/keys/persist-keyfile"));
        assert_eq!(jwe, &PathBuf::from("/etc/keys/persist.jwe"));
        let policy: serde_json::Value = serde_json::from_str(policy).unwrap();
        assert_eq!(policy["t"], 2);
        assert_eq!(policy["pins"]["tpm2"], serde_json::json!({}));
        assert_eq!(policy["pins"]["sss"]["t"], 1);
        assert_eq!(
            policy["pins"]["sss"]["pins"]["tang"][0]["url"],
            "http://tang.lan:7500"
        );
        // The keyfile stays only inside /persist; the JWE goes to both places,
        // the chroot copy before nixos-install bakes it into the initrd.
        let copies: Vec<(usize, &PathBuf)> = acts
            .iter()
            .enumerate()
            .filter_map(|(i, a)| match a {
                InstallAction::CopyKeyfile(_, dst) => Some((i, dst)),
                _ => None,
            })
            .collect();
        let dsts: Vec<&PathBuf> = copies.iter().map(|(_, d)| *d).collect();
        assert_eq!(
            dsts,
            vec![
                &PathBuf::from("/mnt/persist/etc/keys/persist-keyfile"),
                &PathBuf::from("/mnt/etc/keys/persist.jwe"),
                &PathBuf::from("/mnt/persist/etc/keys/persist.jwe"),
            ]
        );
        assert!(copies.iter().all(|(i, _)| *i < install));
        // The target file names the server, so rebuilds keep the mode.
        let target = acts
            .iter()
            .find_map(|a| match a {
                InstallAction::WriteTarget(_, t) => Some(t.clone()),
                _ => None,
            })
            .unwrap();
        assert!(target.contains(
            "  losos.tpm.tang = [\n    { url = \"http://tang.lan:7500\"; thumbprint = \"x3ZDsT7uAzTcJiJ9qZ7f0wJ8Kq0pEJ2O5b5b7l7XyZk\"; }\n  ];\n"
        ));
    }

    #[test]
    fn tang_without_the_tpm_is_refused_before_any_disk_is_touched() {
        let o = Options {
            tpm: false,
            tang: vec![home_tang()],
            ..opts()
        };
        assert!(plan_install(&o, &targets()).unwrap_err().contains("TPM2"));
    }

    const EDGE_ANSWER: &str = r#"{"id":"0123456789abcdef0123456789abcdef","thumbprint":"0_JX_0smhrUKXWw8gE19kBbQBY-Nk7-c6Qssa5xtbGs"}"#;

    #[test]
    fn the_edge_is_a_pin_of_its_own_and_its_claim_goes_into_persist() {
        let edge = edge_tang(
            &edge_base("https://register.example/").unwrap(),
            EDGE_ANSWER,
        )
        .unwrap();
        assert!(edge.edge);
        assert_eq!(
            edge.url,
            "https://register.example/tang/0123456789abcdef0123456789abcdef"
        );
        // The edge alone: the TPM2 AND the edge.
        let policy: serde_json::Value =
            serde_json::from_str(&clevis_policy(std::slice::from_ref(&edge))).unwrap();
        assert_eq!(policy["t"], 2);
        assert_eq!(policy["pins"]["tang"][0]["url"], edge.url.as_str());
        assert!(policy["pins"].get("sss").is_none());
        // With a home server too, all three: a box carried away fails the
        // home server, and a box reported stolen fails the edge.
        let both = vec![home_tang(), edge.clone()];
        let policy: serde_json::Value = serde_json::from_str(&clevis_policy(&both)).unwrap();
        assert_eq!(policy["t"], 3);
        assert_eq!(policy["pins"]["sss"]["t"], 1);
        assert_eq!(
            policy["pins"]["sss"]["pins"]["tang"][0]["url"],
            "http://tang.lan:7500"
        );
        assert_eq!(policy["pins"]["tang"].as_array().unwrap().len(), 1);

        let o = Options {
            tpm: true,
            tang: both,
            ..opts()
        };
        let acts = plan_install(&o, &targets()).unwrap();
        let install = acts
            .iter()
            .position(|a| matches!(a, InstallAction::RunNixosInstall(_)))
            .unwrap();
        let claim = acts
            .iter()
            .position(|a| {
                matches!(a, InstallAction::CopyKeyfile(src, dst)
                    if src == &PathBuf::from("/etc/keys/tang-edge.json")
                        && dst == &PathBuf::from("/mnt/persist/etc/keys/tang-edge.json"))
            })
            .unwrap();
        assert!(claim < install);
        let target = acts
            .iter()
            .find_map(|a| match a {
                InstallAction::WriteTarget(_, t) => Some(t.clone()),
                _ => None,
            })
            .unwrap();
        assert!(target.contains(
            "    { url = \"https://register.example/tang/0123456789abcdef0123456789abcdef\"; thumbprint = \"0_JX_0smhrUKXWw8gE19kBbQBY-Nk7-c6Qssa5xtbGs\"; edge = true; }\n"
        ));
        // A box without the edge carries no claim.
        let o = Options {
            tpm: true,
            tang: vec![home_tang()],
            ..opts()
        };
        assert!(!plan_install(&o, &targets()).unwrap().iter().any(
            |a| matches!(a, InstallAction::CopyKeyfile(src, _) if src.ends_with("tang-edge.json"))
        ));
        // One edge.
        let o = Options {
            tpm: true,
            tang: vec![edge.clone(), edge],
            ..opts()
        };
        assert!(plan_install(&o, &targets()).is_err());
    }

    #[test]
    fn an_edge_answer_or_address_that_could_end_a_string_is_refused() {
        for bad in [
            "register.example",
            "https://",
            "https://r.example/\"x",
            "https://r.example/${x}",
            "https://r.example/#x",
        ] {
            assert!(edge_base(bad).is_err(), "{bad}");
        }
        for bad in [
            r#"{"id":"../../x","thumbprint":"0_JX_0smhrUKXWw8gE19kBbQBY-Nk7-c6Qssa5xtbGs"}"#,
            r#"{"id":"0123456789abcdef0123456789abcdef","thumbprint":"short"}"#,
            r#"{"id":"0123456789abcdef0123456789abcdef","thumbprint":"0_JX_0smhrUKXWw8gE19kBbQBY\"; x"}"#,
            "not json",
        ] {
            assert!(edge_tang("https://r.example", bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_stock_target_file_says_nothing_about_tang() {
        assert!(!render_target(&["/dev/vdb".into()], true, &[], false, &[]).contains("tang"));
    }

    #[test]
    fn tang_arguments_are_checked_like_the_option() {
        assert_eq!(home_tang().url, "http://tang.lan:7500");
        for bad in [
            "http://tang.lan:7500",
            "tang.lan#x3ZDsT7uAzTcJiJ9qZ7f0wJ8Kq0pEJ2O5b5b7l7XyZk",
            "http://#x3ZDsT7uAzTcJiJ9qZ7f0wJ8Kq0pEJ2O5b5b7l7XyZk",
            "http://tang.lan\"; x = 1;#x3ZDsT7uAzTcJiJ9qZ7f0wJ8Kq0pEJ2O5b5b7l7XyZk",
            "http://tang.lan/${x}#x3ZDsT7uAzTcJiJ9qZ7f0wJ8Kq0pEJ2O5b5b7l7XyZk",
            "http://tang.lan#short",
            "http://tang.lan#x3ZDsT7uAzTcJiJ9qZ7f0wJ8Kq0pEJ2O5b5b7l7Xy\"",
        ] {
            assert!(parse_tang(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn keyfile_mode_bakes_the_key_into_the_initrd_and_never_touches_the_tpm() {
        let acts = plan_install(&opts(), &targets()).unwrap();
        assert!(!acts
            .iter()
            .any(|a| matches!(a, InstallAction::EnrollTpm(..))));
        let copies: Vec<&PathBuf> = acts
            .iter()
            .filter_map(|a| match a {
                InstallAction::CopyKeyfile(_, dst) => Some(dst),
                _ => None,
            })
            .collect();
        assert_eq!(
            copies,
            vec![
                &PathBuf::from("/mnt/etc/keys/persist-keyfile"),
                &PathBuf::from("/mnt/persist/etc/keys/persist-keyfile")
            ]
        );
        // Both copies precede nixos-install: the chroot one is read by the
        // bootloader install.
        let install = acts
            .iter()
            .position(|a| matches!(a, InstallAction::RunNixosInstall(_)))
            .unwrap();
        assert!(acts[..install]
            .iter()
            .any(|a| matches!(a, InstallAction::CopyKeyfile(..))));
    }

    #[test]
    fn disko_script_mode_enrols_the_tpm_too() {
        // nix/tests/tpm.nix drives this seam with swtpm, so the test-mode flow
        // has to seal the token just like the real one, right after the
        // prebuilt script formats.
        let o = Options {
            tpm: true,
            disko_script: Some(PathBuf::from("/etc/losos/disko-script")),
            ..opts()
        };
        let acts = plan_install(&o, &targets()).unwrap();
        let script = acts
            .iter()
            .position(|a| matches!(a, InstallAction::RunDiskoScript(_)))
            .unwrap();
        assert!(matches!(acts[script + 1], InstallAction::Log(_)));
        assert!(matches!(acts[script + 2], InstallAction::EnrollTpm(..)));
        assert!(!acts
            .iter()
            .any(|a| matches!(a, InstallAction::RunNixosInstall(_))));
    }

    #[test]
    fn tpm_is_the_default_where_a_chip_exists_and_an_error_where_it_is_forced_without_one() {
        assert_eq!(resolve_tpm(None, true), Ok(true));
        assert_eq!(resolve_tpm(None, false), Ok(false));
        assert_eq!(resolve_tpm(Some(false), true), Ok(false));
        assert_eq!(resolve_tpm(Some(true), true), Ok(true));
        assert!(resolve_tpm(Some(true), false)
            .unwrap_err()
            .contains("--no-tpm"));
    }

    #[test]
    fn the_clone_is_the_channel_the_box_will_follow() {
        let acts = plan_install(&opts(), &targets()).unwrap();
        let clone = acts
            .iter()
            .find_map(|a| match a {
                InstallAction::CloneFlake(url, branch, _) => {
                    Some((url.as_str(), branch.as_deref()))
                }
                _ => None,
            })
            .expect("the plan clones the flake");
        assert_eq!(
            clone,
            (
                "https://github.com/dasmatus/losos.git",
                Some("channel/daily")
            )
        );
    }

    #[test]
    fn full_path_runs_every_stage() {
        let acts = plan_install(&opts(), &targets()).unwrap();
        for want in [
            matches!(acts[0], InstallAction::Log(_)),
            acts.iter()
                .any(|a| matches!(a, InstallAction::EnsureKeyfile(_))),
            acts.iter()
                .any(|a| matches!(a, InstallAction::CloneFlake(..))),
            acts.iter().any(|a| matches!(a, InstallAction::RunDisko(_))),
            acts.iter()
                .any(|a| matches!(a, InstallAction::RunNixosInstall(_))),
            acts.iter()
                .any(|a| matches!(a, InstallAction::LayFlake(..))),
            acts.iter()
                .any(|a| matches!(a, InstallAction::CopyKeyfile(..))),
        ] {
            assert!(want);
        }
    }

    /// The ordering guarantees. These are the assertions that would have caught
    /// the "empty store on first boot" and "bootloader install fails" bugs.
    #[test]
    fn staging_precedes_nixos_install() {
        let acts = plan_install(&opts(), &targets()).unwrap();
        let install = index_of(&acts, |a| matches!(a, InstallAction::RunNixosInstall(_)));
        let bind = index_of(&acts, |a| matches!(a, InstallAction::BindMount(..)));
        let disko = index_of(&acts, |a| matches!(a, InstallAction::RunDisko(_)));
        let lay = index_of(&acts, |a| matches!(a, InstallAction::LayFlake(..)));

        assert!(acts.contains(&InstallAction::BindMount(
            PathBuf::from("/mnt/persist/nix"),
            PathBuf::from("/mnt/nix")
        )));
        assert!(disko < bind, "the bind needs /mnt/persist mounted first");
        assert!(
            bind < install,
            "the store must be bound before it is filled"
        );
        assert!(install < lay);

        let chroot_key = index_of(
            &acts,
            |a| matches!(a, InstallAction::CopyKeyfile(_, d) if d == Path::new("/mnt/etc/keys/persist-keyfile")),
        );
        let persist_key = index_of(
            &acts,
            |a| matches!(a, InstallAction::CopyKeyfile(_, d) if d == Path::new("/mnt/persist/etc/keys/persist-keyfile")),
        );
        assert!(
            chroot_key < install,
            "the chrooted bootloader install reads this path"
        );
        assert!(persist_key < install);
    }

    #[test]
    fn no_install_still_binds_so_mnt_is_really_ready() {
        let o = Options {
            no_install: true,
            ..opts()
        };
        let acts = plan_install(&o, &targets()).unwrap();
        assert!(acts.iter().any(|a| matches!(a, InstallAction::RunDisko(_))));
        assert!(acts
            .iter()
            .any(|a| matches!(a, InstallAction::BindMount(..))));
        assert!(!acts
            .iter()
            .any(|a| matches!(a, InstallAction::RunNixosInstall(_))));
        assert!(!acts
            .iter()
            .any(|a| matches!(a, InstallAction::LayFlake(..))));
    }

    #[test]
    fn no_run_ends_with_a_manual_enrolment_hint() {
        // The old TPM path told the owner to run systemd-cryptenroll by hand
        // after the first boot, on a box with no shell. Nothing may say that
        // any more: enrolment is an action in the plan, not a chore.
        for tpm in [false, true] {
            let acts = plan_install(&Options { tpm, ..opts() }, &targets()).unwrap();
            let logs: Vec<&String> = acts
                .iter()
                .filter_map(|a| match a {
                    InstallAction::Log(t) => Some(t),
                    _ => None,
                })
                .collect();
            assert!(logs.iter().any(|l| l.contains("done")));
            assert!(!logs.iter().any(|l| l.contains("passphrase")), "{logs:?}");
            assert_eq!(
                logs.iter().any(|l| l.contains("unlock: TPM2")),
                tpm,
                "{logs:?}"
            );
        }
    }

    #[test]
    fn a_keyfile_install_warns_and_the_warning_is_the_last_line_before_done() {
        for tpm in [false, true] {
            let acts = plan_install(&Options { tpm, ..opts() }, &targets()).unwrap();
            let logs: Vec<&String> = acts
                .iter()
                .filter_map(|a| match a {
                    InstallAction::Log(t) => Some(t),
                    _ => None,
                })
                .collect();
            let warned = logs.iter().filter(|l| l.as_str() == NO_TPM_WARNING).count();
            assert_eq!(warned, if tpm { 0 } else { 2 }, "{logs:?}");
            if !tpm {
                let n = logs.len();
                assert_eq!(logs[n - 2].as_str(), NO_TPM_WARNING, "{logs:?}");
                assert!(logs[n - 1].contains("done"), "{logs:?}");
            }
        }
    }

    #[test]
    fn no_candidates_is_an_error_not_an_empty_wipe() {
        let mounted = vec![disk("vda", 8 * GI, vec![part("vda1", &["/"])])];
        assert!(plan_install(&opts(), &mounted).is_err());
    }

    #[test]
    fn explicit_drives_bypass_detection() {
        let o = Options {
            drives: Some(vec!["/dev/nvme0n1".into()]),
            ..opts()
        };
        let acts = plan_install(&o, &targets()).unwrap();
        let rendered = acts.iter().find_map(|a| match a {
            InstallAction::WriteTarget(_, t) => Some(t.clone()),
            _ => None,
        });
        assert!(rendered.unwrap().contains("/dev/nvme0n1"));
    }

    #[test]
    fn empty_drives_list_is_rejected() {
        let o = Options {
            drives: Some(vec![]),
            ..opts()
        };
        assert!(plan_install(&o, &targets()).is_err());
    }
}
