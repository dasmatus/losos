//! An open gateway's half of the check boxes run before they trust it with
//! the market (`losos_access::gateway`, the box's half).
//!
//! Anyone may run a gateway. What a box asks of it is the passport its owner
//! signed, a TPM that shows the published image is running untouched, and
//! fee settings that pay LosOS and the owner 2% each. The edge's part is to
//! show those three things honestly:
//!
//! * the **settings** (the passport plus the fee addresses) are a file the
//!   owner sends with `POST /passport/settings`, signed with their OpenPGP
//!   key. The edge writes it and the machine reboots
//!   (`losos-gateway-reboot.path`); at boot, before this process starts,
//!   `losos-gateway-measure` copies it to `/run` and extends PCR 15 with
//!   its SHA-256. What `GET /passport` serves is that measured copy, so a box
//!   that hashes it can compare it with the quoted PCR 15.
//! * the **TPM** identity (endorsement key, its maker's certificate, an
//!   attestation key) and the firmware's event log are made once per boot by
//!   `losos-gateway-tpm` into a directory this process reads.
//! * the **quote** is made per request: `POST /passport/attest` opens the
//!   box's credential with the TPM (which only works for the attestation
//!   key in this TPM) and quotes PCRs 4, 11 and 15 with the box's nonce and
//!   this edge's key as qualifying data.
//!
//! The TPM is used through `tpm2-tools` on the raw device, one request at a
//! time: a policy session has to outlive the command that started it, which
//! the kernel's resource manager does not allow across processes.

use std::path::{Path, PathBuf};
use std::time::Duration;

use base64::Engine;
use losos_access::gateway::check::{
    self, AttestAnswer, AttestRequest, Hello, Settings, TpmIdentity,
};
use losos_access::gateway::passport::{self, Owner, PassportError};
use losos_access::gateway::tpm;
use miette::{miette, Context, IntoDiagnostic, Result};
use ring::signature::{Ed25519KeyPair, KeyPair};
use serde::Deserialize;

use crate::action::Action;
use crate::identity::{from_hex, keygen, load_key, random_hex, to_hex, write_private};

/// The longest settings file the edge takes: a passport with an owner key
/// and sixteen networks is a few kilobytes.
pub const MAX_SETTINGS: usize = 64 * 1024;
/// How long an attestation waits for the TPM before answering 503.
pub const TPM_WAIT: Duration = Duration::from_secs(20);
/// Quote attempts before giving up: a PCR extended between the quote and
/// the read makes the two disagree, and the edge never sends that pair.
const QUOTE_TRIES: usize = 3;

/// `serve --gateway-key-file … --gateway-settings-file … --gateway-measured-file
/// … --gateway-tpm-dir … [--gateway-tcti …]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayOpts {
    /// The edge's Ed25519 key (hex PKCS#8, 0600), made on the first start.
    pub key_file: String,
    /// Where an accepted `POST /passport/settings` is written; a path unit
    /// reboots the machine when it changes.
    pub settings_file: String,
    /// The copy `losos-gateway-measure` measured at boot.
    pub measured_file: String,
    /// What `losos-gateway-tpm` made at boot: `ek.pub`, `ak.pub`, `ek.ctx`,
    /// `ak.ctx`, `ek.der`, `chain/*.der`, `eventlog.bin`.
    pub tpm_dir: String,
    /// The `TPM2TOOLS_TCTI` for the per-request commands.
    pub tcti: String,
}

/// The body of `POST /passport/settings`.
#[derive(Debug, Clone, Deserialize)]
pub struct Claim {
    /// The new settings file, byte for byte.
    pub settings: String,
    /// An armored detached OpenPGP signature over `settings` by the current
    /// owner, or by the new passport's key when the gateway has none yet.
    pub signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ClaimError {
    #[error("not a settings file: {0}")]
    Malformed(String),
    #[error("the passport does not hold: {0}")]
    Passport(PassportError),
    #[error("the passport names another edge key; this edge's is {0}")]
    OtherEdgeKey(String),
    #[error(
        "the fee split must be {} and {} basis points",
        check::LOSOS_BPS,
        check::OWNER_BPS
    )]
    Fees,
    #[error("the settings are not signed by the gateway's current owner")]
    NotTheOwner,
    #[error("could not write the settings: {0}")]
    Write(String),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AttestError {
    #[error("{0}")]
    BadRequest(&'static str),
    #[error("the TPM is busy")]
    Busy,
    #[error("the TPM failed: {0}")]
    Tpm(String),
}

pub struct Gateway {
    key: Ed25519KeyPair,
    public_hex: String,
    opts: GatewayOpts,
    tpm: tokio::sync::Mutex<()>,
}

/// A nonce the gateway signs and quotes over: 32 to 128 hex characters.
#[must_use]
pub fn nonce_ok(nonce: &str) -> bool {
    (32..=128).contains(&nonce.len()) && nonce.bytes().all(|b| b.is_ascii_hexdigit())
}

impl Gateway {
    /// Open the edge key (made 0600 on the first start). The settings and
    /// the TPM files are read per request, so a gateway that has not been
    /// claimed yet still starts and answers `GET /passport` with 404.
    pub fn open(opts: GatewayOpts) -> Result<Self> {
        let path = Path::new(&opts.key_file);
        let key = match std::fs::read_to_string(path) {
            Ok(hex) => {
                load_key(&hex).wrap_err_with(|| format!("the edge key {}", path.display()))?
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let (pkcs8, public) = keygen()?;
                if let Some(dir) = path.parent() {
                    std::fs::create_dir_all(dir)
                        .into_diagnostic()
                        .wrap_err_with(|| format!("creating {}", dir.display()))?;
                }
                write_private(path, &pkcs8)?;
                tracing::info!(
                    target: Action::Passport.target(),
                    "made this gateway's edge key at {} (public key {public})",
                    path.display()
                );
                load_key(&pkcs8)?
            }
            Err(e) => {
                return Err(e)
                    .into_diagnostic()
                    .wrap_err_with(|| format!("reading the edge key {}", path.display()))
            }
        };
        let public_hex = to_hex(key.public_key().as_ref());
        Ok(Gateway {
            key,
            public_hex,
            opts,
            tpm: tokio::sync::Mutex::new(()),
        })
    }

    /// The edge key the passport must name, 64 hex characters.
    #[must_use]
    pub fn public_key(&self) -> &str {
        &self.public_hex
    }

    /// The settings measured at this boot, or `None` before the gateway was
    /// claimed.
    #[must_use]
    pub fn measured(&self) -> Option<String> {
        std::fs::read_to_string(&self.opts.measured_file)
            .ok()
            .filter(|s| !s.is_empty())
    }

    /// Who the measured passport names, for the operator's view and the
    /// journal. Not a trust decision: boxes make that.
    #[must_use]
    pub fn owner(&self) -> Option<Owner> {
        let settings: Settings = serde_json::from_str(&self.measured()?).ok()?;
        passport::check_owner_key(&settings.passport.key, now_secs())
            .ok()
            .map(|(_, o)| o)
    }

    fn tpm_file(&self, name: &str) -> PathBuf {
        Path::new(&self.opts.tpm_dir).join(name)
    }

    fn read_tpm(&self, name: &str) -> std::result::Result<Vec<u8>, String> {
        let path = self.tpm_file(name);
        std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// `GET /passport?nonce=`: `Ok(None)` before the gateway was claimed.
    pub fn hello(&self, nonce: &str) -> std::result::Result<Option<Hello>, String> {
        let Some(settings) = self.measured() else {
            return Ok(None);
        };
        let b64 = |b: &[u8]| base64::engine::general_purpose::STANDARD.encode(b);
        let mut chain = Vec::new();
        if let Ok(dir) = std::fs::read_dir(self.tpm_file("chain")) {
            let mut paths: Vec<PathBuf> = dir.flatten().map(|e| e.path()).collect();
            paths.sort();
            for p in paths {
                chain.push(b64(
                    &std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?
                ));
            }
        }
        Ok(Some(Hello {
            settings,
            nonce_signature: to_hex(
                self.key
                    .sign(check::nonce_message(nonce).as_bytes())
                    .as_ref(),
            ),
            tpm: TpmIdentity {
                ek_public: to_hex(&self.read_tpm("ek.pub")?),
                ek_cert: b64(&self.read_tpm("ek.der")?),
                ek_chain: chain,
                ak_public: to_hex(&self.read_tpm("ak.pub")?),
            },
        }))
    }

    /// `POST /passport/attest`: open the credential, quote, read the PCRs
    /// and the event log. One at a time; a request that waits longer than
    /// [`TPM_WAIT`] is told the TPM is busy.
    pub async fn attest(
        &self,
        req: AttestRequest,
    ) -> std::result::Result<AttestAnswer, AttestError> {
        if !nonce_ok(&req.nonce) {
            return Err(AttestError::BadRequest(
                "nonce must be 32 to 128 hex characters",
            ));
        }
        let credential = from_hex(&req.credential)
            .filter(|c| c.len() <= 4096)
            .ok_or(AttestError::BadRequest("credential must be hex"))?;
        tpm::parse_credential_file(&credential)
            .map_err(|_| AttestError::BadRequest("not a credential file"))?;
        let _one = tokio::time::timeout(TPM_WAIT, self.tpm.lock())
            .await
            .map_err(|_| AttestError::Busy)?;
        let qualifying = check::qualifying_data(&req.nonce, &self.public_hex);
        let opts = self.opts.clone();
        tokio::task::spawn_blocking(move || attest_blocking(&opts, &credential, &qualifying))
            .await
            .map_err(|e| AttestError::Tpm(e.to_string()))?
    }

    /// `POST /passport/settings`: take new settings signed by the owner and
    /// write them; the machine reboots to measure them.
    pub fn claim(&self, claim: &Claim, now: i64) -> std::result::Result<Owner, ClaimError> {
        if claim.settings.len() > MAX_SETTINGS {
            return Err(ClaimError::Malformed("too long".into()));
        }
        let settings: Settings = serde_json::from_str(&claim.settings)
            .map_err(|e| ClaimError::Malformed(e.to_string()))?;
        if settings.version != 1 {
            return Err(ClaimError::Malformed(format!(
                "version {}",
                settings.version
            )));
        }
        let p = &settings.passport;
        let (statement, owner) = passport::verify(&p.statement, &p.signature, &p.key, now)
            .map_err(ClaimError::Passport)?;
        if statement.edge_key != self.public_hex {
            return Err(ClaimError::OtherEdgeKey(self.public_hex.clone()));
        }
        if settings.fees.losos_bps != check::LOSOS_BPS
            || settings.fees.owner_bps != check::OWNER_BPS
        {
            return Err(ClaimError::Fees);
        }
        // The current owner signs a change; on a gateway with no owner yet,
        // the new passport's own key does.
        let signer = match std::fs::read_to_string(&self.opts.settings_file) {
            Ok(current) => serde_json::from_str::<Settings>(&current)
                .map(|s| s.passport.key)
                .unwrap_or_else(|_| p.key.clone()),
            Err(_) => p.key.clone(),
        };
        let (signer_key, _) =
            passport::check_owner_key(&signer, now).map_err(ClaimError::Passport)?;
        if !passport::signed_by(&signer_key, &claim.signature, claim.settings.as_bytes()) {
            return Err(ClaimError::NotTheOwner);
        }
        write_settings(Path::new(&self.opts.settings_file), &claim.settings)
            .map_err(ClaimError::Write)?;
        tracing::info!(
            target: Action::Passport.target(),
            "settings for {} accepted from {} ({}); rebooting to measure them",
            statement.url, owner.user_id, owner.fingerprint
        );
        Ok(owner)
    }
}

fn write_settings(path: &Path, text: &str) -> std::result::Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text)
        .and_then(|()| std::fs::rename(&tmp, path))
        .map_err(|e| format!("{}: {e}", path.display()))
}

fn now_secs() -> i64 {
    i64::try_from(crate::market::now_secs()).unwrap_or(i64::MAX)
}

/// Run one `tpm2_*` tool against the gateway's TPM.
fn tool(opts: &GatewayOpts, dir: &Path, args: &[&str]) -> std::result::Result<(), AttestError> {
    let out = std::process::Command::new(args[0])
        .args(&args[1..])
        .current_dir(dir)
        .env("TPM2TOOLS_TCTI", &opts.tcti)
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| AttestError::Tpm(format!("{}: {e}", args[0])))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(AttestError::Tpm(format!(
            "{} failed: {}",
            args[0],
            String::from_utf8_lossy(&out.stderr).trim()
        )))
    }
}

fn attest_blocking(
    opts: &GatewayOpts,
    credential: &[u8],
    qualifying: &[u8; 32],
) -> std::result::Result<AttestAnswer, AttestError> {
    let dir = std::env::temp_dir().join(format!(
        "losos-attest-{}",
        random_hex(8).ok_or_else(|| AttestError::Tpm("no randomness".into()))?
    ));
    std::fs::create_dir(&dir).map_err(|e| AttestError::Tpm(e.to_string()))?;
    let result = attest_in(opts, &dir, credential, qualifying);
    let _ = std::fs::remove_dir_all(&dir);
    // Whatever happened, leave no object or session loaded on the raw TPM.
    let _ = tool(opts, Path::new("/"), &["tpm2_flushcontext", "-t"]);
    let _ = tool(opts, Path::new("/"), &["tpm2_flushcontext", "-s"]);
    result
}

fn attest_in(
    opts: &GatewayOpts,
    dir: &Path,
    credential: &[u8],
    qualifying: &[u8; 32],
) -> std::result::Result<AttestAnswer, AttestError> {
    let tpm_dir = Path::new(&opts.tpm_dir);
    let ek = tpm_dir.join("ek.ctx");
    let ak = tpm_dir.join("ak.ctx");
    let (ek, ak) = (ek.to_string_lossy(), ak.to_string_lossy());
    std::fs::write(dir.join("cred.bin"), credential)
        .map_err(|e| AttestError::Tpm(e.to_string()))?;

    tool(
        opts,
        dir,
        &["tpm2_startauthsession", "--policy-session", "-S", "s.ctx"],
    )?;
    tool(opts, dir, &["tpm2_policysecret", "-S", "s.ctx", "-c", "e"])?;
    tool(
        opts,
        dir,
        &[
            "tpm2_activatecredential",
            "-c",
            &ak,
            "-C",
            &ek,
            "-i",
            "cred.bin",
            "-o",
            "secret.bin",
            "-P",
            "session:s.ctx",
        ],
    )?;
    tool(opts, dir, &["tpm2_flushcontext", "s.ctx"])?;
    let secret =
        std::fs::read(dir.join("secret.bin")).map_err(|e| AttestError::Tpm(e.to_string()))?;

    let selection = "sha256:4,11,15";
    let q = to_hex(qualifying);
    for _ in 0..QUOTE_TRIES {
        tool(
            opts,
            dir,
            &[
                "tpm2_quote",
                "-c",
                &ak,
                "-l",
                selection,
                "-q",
                &q,
                "-m",
                "attest.bin",
                "-s",
                "sig.bin",
                "-g",
                "sha256",
            ],
        )?;
        tool(opts, dir, &["tpm2_pcrread", selection, "-o", "pcrs.bin"])?;
        let read =
            |n: &str| std::fs::read(dir.join(n)).map_err(|e| AttestError::Tpm(e.to_string()));
        let (attest, sig, values) = (read("attest.bin")?, read("sig.bin")?, read("pcrs.bin")?);
        if values.len() != 96 {
            return Err(AttestError::Tpm(format!(
                "read {} bytes of PCRs, not 96",
                values.len()
            )));
        }
        let parsed = tpm::Attest::parse(&attest).map_err(|e| AttestError::Tpm(e.to_string()))?;
        let quoted: Vec<[u8; 32]> = values
            .chunks(32)
            .map(|c| <[u8; 32]>::try_from(c).unwrap_or([0; 32]))
            .collect();
        if parsed.pcr_digest != tpm::pcr_digest(&quoted) {
            // A PCR moved between the quote and the read: quote again.
            continue;
        }
        let event_log = std::fs::read(tpm_dir.join("eventlog.bin"))
            .map_err(|e| AttestError::Tpm(format!("the event log: {e}")))?;
        let pcrs = check::PCRS
            .iter()
            .zip(&quoted)
            .map(|(i, v)| (i.to_string(), to_hex(v)))
            .collect();
        return Ok(AttestAnswer {
            secret: to_hex(&secret),
            attest: to_hex(&attest),
            signature: to_hex(&sig),
            pcrs,
            event_log: base64::engine::general_purpose::STANDARD.encode(event_log),
        });
    }
    Err(AttestError::Tpm(
        "the PCRs kept changing while being quoted".into(),
    ))
}

/// `losos-registrar passport …`: what a gateway's owner runs on their own
/// machine, and the box's check run by hand.
pub mod cli {
    use super::*;
    use std::collections::BTreeMap;
    use std::process::ExitCode;

    use crate::opts::PassportOpts;
    use losos_access::gateway::{check::Anchors, eventlog, pe, uki};

    /// The statement an owner signs, for `url`, this edge key, these payouts
    /// and `days` from now.
    pub fn statement(
        url: &str,
        edge_key: &str,
        payouts: &[(String, String)],
        days: i64,
    ) -> Result<String> {
        let statement = passport::Statement {
            url: url.trim().trim_end_matches('/').to_string(),
            edge_key: edge_key.trim().to_string(),
            owner_payouts: payouts.iter().cloned().collect::<BTreeMap<_, _>>(),
            not_after: now_secs() + days * 86_400,
        };
        let text = statement.render();
        passport::Statement::parse(&text).map_err(|e| miette!("{e}"))?;
        Ok(text)
    }

    /// The settings file from a signed statement, the owner's public key and
    /// LosOS's payout addresses, checked as a box would check them.
    pub fn settings(
        statement: &str,
        signature: &str,
        key: &str,
        losos: BTreeMap<String, String>,
    ) -> Result<String> {
        let (parsed, _) =
            passport::verify(statement, signature, key, now_secs()).map_err(|e| miette!("{e}"))?;
        let settings = Settings {
            version: 1,
            passport: check::PassportFields {
                statement: statement.to_string(),
                signature: signature.to_string(),
                key: key.to_string(),
            },
            fees: check::Fees {
                losos_bps: check::LOSOS_BPS,
                owner_bps: check::OWNER_BPS,
                losos: losos.clone(),
            },
        };
        check::check_fees(&settings.fees, &parsed.owner_payouts, &losos)
            .map_err(|e| miette!("{e}"))?;
        serde_json::to_string_pretty(&settings).into_diagnostic()
    }

    /// The image entry boxes ship for a unified kernel image: its PCR 11,
    /// and the Authenticode hashes of the image, the kernel inside it and
    /// any other program the firmware may start on the way.
    pub fn measure(uki_bytes: &[u8], name: &str, apps: &[Vec<u8>]) -> Result<check::Image> {
        let err = |e: pe::PeError| miette!("{e}");
        let mut hashes = vec![to_hex(&pe::authenticode_sha256(uki_bytes).map_err(err)?)];
        if let Some(linux) = pe::section(uki_bytes, ".linux").map_err(err)? {
            hashes.push(to_hex(&pe::authenticode_sha256(linux).map_err(err)?));
        }
        for app in apps {
            hashes.push(to_hex(&pe::authenticode_sha256(app).map_err(err)?));
        }
        hashes.dedup();
        Ok(check::Image {
            name: name.to_string(),
            pcr11: to_hex(&uki::pcr11(uki_bytes).map_err(err)?),
            apps: hashes,
        })
    }

    /// This machine's image entry as it booted: PCR 11 from its TPM (the
    /// tools' TCTI, the raw device by default) and the programs its event
    /// log names on PCR 4. It vouches for nothing: it is what a box would
    /// see, written down.
    pub fn image(name: &str, event_log: &[u8]) -> Result<check::Image> {
        let events = eventlog::parse(event_log).map_err(|e| miette!("{e}"))?;
        let apps = eventlog::boot_applications(&events)
            .map_err(|kind| miette!("PCR 4 holds an event of type {kind:#x} no box accepts"))?;
        let dir = std::env::temp_dir().join(format!(
            "losos-image-{}",
            random_hex(8).ok_or_else(|| miette!("the system random source failed"))?
        ));
        std::fs::create_dir(&dir).into_diagnostic()?;
        let out = dir.join("pcr11.bin");
        let status = std::process::Command::new("tpm2_pcrread")
            .args(["sha256:11", "-o"])
            .arg(&out)
            .env(
                "TPM2TOOLS_TCTI",
                std::env::var("TPM2TOOLS_TCTI").unwrap_or_else(|_| "device:/dev/tpm0".into()),
            )
            .stdout(std::process::Stdio::null())
            .status()
            .into_diagnostic()
            .wrap_err("running tpm2_pcrread")?;
        let pcr11 = std::fs::read(&out);
        let _ = std::fs::remove_dir_all(&dir);
        if !status.success() {
            return Err(miette!("tpm2_pcrread failed"));
        }
        let pcr11 = pcr11.into_diagnostic()?;
        if pcr11.len() != 32 {
            return Err(miette!("read {} bytes of PCR 11, not 32", pcr11.len()));
        }
        let mut hashes: Vec<String> = apps.iter().map(|a| to_hex(a)).collect();
        hashes.dedup();
        Ok(check::Image {
            name: name.to_string(),
            pcr11: to_hex(&pcr11),
            apps: hashes,
        })
    }

    fn read(path: &str) -> Result<String> {
        std::fs::read_to_string(path)
            .into_diagnostic()
            .wrap_err_with(|| format!("reading {path}"))
    }

    /// The box's whole check against a running gateway: `Ok` with what it
    /// would show, or the refusal.
    pub async fn check_gateway(
        client: &reqwest::Client,
        url: &str,
        anchors: &Anchors,
    ) -> Result<std::result::Result<(Owner, check::Verdict), check::Refusal>> {
        let nonce = random_hex(32).ok_or_else(|| miette!("the system random source failed"))?;
        let resp = client
            .get(format!("{url}/passport?nonce={nonce}"))
            .send()
            .await
            .into_diagnostic()
            .wrap_err_with(|| format!("GET {url}/passport"))?;
        if !resp.status().is_success() {
            return Err(miette!("{url}/passport answered {}", resp.status()));
        }
        let hello: Hello = resp.json().await.into_diagnostic()?;
        let intro = match check::introduce(url, &nonce, &hello, anchors, now_secs()) {
            Ok(i) => i,
            Err(r) => return Ok(Err(r)),
        };
        let (challenge, req) = match check::challenge(&intro, &nonce, &mut rand::thread_rng()) {
            Ok(c) => c,
            Err(r) => return Ok(Err(r)),
        };
        let resp = client
            .post(format!("{url}/passport/attest"))
            .json(&req)
            .send()
            .await
            .into_diagnostic()
            .wrap_err_with(|| format!("POST {url}/passport/attest"))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(miette!(
                "{url}/passport/attest answered {status}: {}",
                text.trim()
            ));
        }
        let answer: AttestAnswer = resp.json().await.into_diagnostic()?;
        Ok(check::judge(
            &intro,
            &nonce,
            &challenge,
            &answer,
            &hello.tpm.ak_public,
            anchors,
        )
        .map(|v| (intro.owner.clone(), v)))
    }

    pub async fn run(opts: PassportOpts) -> Result<ExitCode> {
        match opts {
            PassportOpts::Statement {
                url,
                edge_key,
                payouts,
                days,
            } => {
                print!("{}", statement(&url, &edge_key, &payouts, days)?);
            }
            PassportOpts::Settings {
                statement,
                signature,
                key,
                losos_payouts,
            } => {
                let losos: BTreeMap<String, String> = serde_json::from_str(&read(&losos_payouts)?)
                    .into_diagnostic()
                    .wrap_err("the LosOS payout file is a JSON object, network to address")?;
                println!(
                    "{}",
                    settings(&read(&statement)?, &read(&signature)?, &read(&key)?, losos)?
                );
            }
            PassportOpts::Measure { uki, name, apps } => {
                let bytes = std::fs::read(&uki)
                    .into_diagnostic()
                    .wrap_err_with(|| format!("reading {uki}"))?;
                let apps = apps
                    .iter()
                    .map(|a| {
                        std::fs::read(a)
                            .into_diagnostic()
                            .wrap_err_with(|| format!("reading {a}"))
                    })
                    .collect::<Result<Vec<_>>>()?;
                println!(
                    "{}",
                    serde_json::to_string_pretty(&measure(&bytes, &name, &apps)?)
                        .into_diagnostic()?
                );
            }
            PassportOpts::Image { name, event_log } => {
                let bytes = std::fs::read(&event_log)
                    .into_diagnostic()
                    .wrap_err_with(|| format!("reading {event_log}"))?;
                println!(
                    "{}",
                    serde_json::to_string_pretty(&image(&name, &bytes)?).into_diagnostic()?
                );
            }
            PassportOpts::Check {
                url,
                tpm_roots,
                images,
                payouts,
            } => {
                let anchors =
                    Anchors::from_texts(&read(&tpm_roots)?, &read(&images)?, &read(&payouts)?)
                        .map_err(|e| miette!("{e}"))?;
                let client = reqwest::Client::builder()
                    .timeout(Duration::from_secs(60))
                    .build()
                    .into_diagnostic()?;
                match check_gateway(&client, &url, &anchors).await? {
                    Ok((owner, verdict)) => {
                        println!(
                            "trusted: {} ({}), image {}",
                            owner.user_id, owner.fingerprint, verdict.image
                        );
                    }
                    Err(r) => {
                        eprintln!("not trusted ({}): {r}", r.code());
                        return Ok(ExitCode::from(if r.tampered() { 2 } else { 1 }));
                    }
                }
            }
        }
        Ok(ExitCode::SUCCESS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(dir: &Path) -> GatewayOpts {
        GatewayOpts {
            key_file: dir.join("edge.key").to_string_lossy().into(),
            settings_file: dir.join("settings.json").to_string_lossy().into(),
            measured_file: dir.join("measured.json").to_string_lossy().into(),
            tpm_dir: dir.join("tpm").to_string_lossy().into(),
            tcti: "device:/dev/null".into(),
        }
    }

    fn scratch() -> PathBuf {
        let d = std::env::temp_dir().join(format!("losos-gw-{}", random_hex(8).unwrap()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn the_key_is_made_once_and_an_unclaimed_gateway_says_nothing() {
        let dir = scratch();
        let a = Gateway::open(opts(&dir)).unwrap();
        let b = Gateway::open(opts(&dir)).unwrap();
        assert_eq!(a.public_key(), b.public_key());
        assert_eq!(a.public_key().len(), 64);
        assert_eq!(a.hello(&"00".repeat(16)), Ok(None));
        assert!(a.owner().is_none());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn nonces_are_bounded_hex() {
        assert!(nonce_ok(&"ab".repeat(16)));
        assert!(!nonce_ok("ab"));
        assert!(!nonce_ok(&"zz".repeat(16)));
        assert!(!nonce_ok(&"ab".repeat(65)));
    }
}
