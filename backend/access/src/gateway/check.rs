//! What a box asks a gateway, and how it judges the answers.
//!
//! Two round trips. `GET /passport?nonce=N` brings the gateway's settings
//! (the passport and the fee addresses, as the exact text the gateway
//! measured into PCR 15 when it booted), a signature over the nonce by the
//! gateway's Ed25519 key, and its TPM's endorsement and attestation keys
//! ([`Hello`]). [`introduce`] checks everything that needs no TPM: the
//! passport, the URL, the nonce signature, the fees, the maker's
//! certificate. Then [`challenge`] makes a credential only that TPM can
//! open, and `POST /passport/attest` brings back the opened secret, a quote
//! over PCRs 4, 11 and 15 bound to the nonce and the edge key, and the
//! firmware's event log ([`AttestAnswer`]). [`judge`] decides.
//!
//! A [`Refusal`] says why a gateway is not trusted, and
//! [`Refusal::tampered`] whether that is proof of tampering, which disables
//! the gateway on the box for good, or only something the box does not know
//! (an image it has no measurements for, a TPM maker it has no root for, a
//! fee address that differs from the ones it ships), which it may learn later.

use std::collections::BTreeMap;

use base64::Engine;
use rand::{CryptoRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use x509_cert::Certificate;

use super::passport::{self, normalize_address, Owner, PassportError, Statement};
use super::tpm::{self, Attest, Public, Signature, ALG_SHA256};
use super::{ekcert, eventlog};

/// The share of a sale each side gets, in basis points: 2% to LosOS, 2% to
/// the gateway's owner. A gateway whose settings say anything else is not
/// trusted.
pub const LOSOS_BPS: u32 = 200;
pub const OWNER_BPS: u32 = 200;
/// The PCRs quoted, in index order.
pub const PCRS: [u8; 3] = [4, 11, 15];
const SETTINGS_VERSION: u32 = 1;
const NONCE_PREFIX: &str = "losos-gateway-nonce-v1";
const QUOTE_PREFIX: &str = "losos-gateway-quote-v1";

/// The gateway's settings file, measured into PCR 15 before anything else
/// on the gateway runs. The box hashes the exact text it is sent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub version: u32,
    pub passport: PassportFields,
    pub fees: Fees,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PassportFields {
    /// The statement's exact text ([`Statement::render`]).
    pub statement: String,
    /// The owner's armored detached signature over it.
    pub signature: String,
    /// The owner's armored public key.
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fees {
    pub losos_bps: u32,
    pub owner_bps: u32,
    /// Network to LosOS's payout address there, as the box ships them.
    pub losos: BTreeMap<String, String>,
}

/// `GET /passport?nonce=`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hello {
    /// The settings file's text, byte for byte.
    pub settings: String,
    /// The Ed25519 signature over [`nonce_message`], hex.
    pub nonce_signature: String,
    pub tpm: TpmIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TpmIdentity {
    /// `TPM2B_PUBLIC` of the endorsement key, hex.
    pub ek_public: String,
    /// The maker's certificate for it, DER in base64.
    pub ek_cert: String,
    /// Intermediate certificates, DER in base64, any order.
    #[serde(default)]
    pub ek_chain: Vec<String>,
    /// `TPM2B_PUBLIC` of the attestation key, hex.
    pub ak_public: String,
}

/// `POST /passport/attest`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttestRequest {
    pub nonce: String,
    /// The credential, in the file format `tpm2_activatecredential -i`
    /// reads, hex.
    pub credential: String,
}

/// The answer to [`AttestRequest`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttestAnswer {
    /// The credential's secret as the TPM opened it, hex.
    pub secret: String,
    /// `TPMS_ATTEST` of the quote, hex.
    pub attest: String,
    /// `TPMT_SIGNATURE` over it, hex.
    pub signature: String,
    /// The quoted PCRs' SHA-256 values, by index, hex.
    pub pcrs: BTreeMap<String, String>,
    /// The firmware's event log, base64.
    pub event_log: String,
}

/// One published gateway image: the PCR 11 its unified kernel image leaves
/// behind, and the Authenticode hashes of every program the firmware may
/// start on the way to it (the image itself, the kernel inside it).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Image {
    pub name: String,
    pub pcr11: String,
    pub apps: Vec<String>,
}

/// What the box trusts, from files shipped with it.
#[derive(Debug, Clone, Default)]
pub struct Anchors {
    pub tpm_roots: Vec<Certificate>,
    /// The makers' intermediate CAs, for a gateway whose TPM does not carry
    /// its own chain.
    pub tpm_intermediates: Vec<Certificate>,
    pub images: Vec<Image>,
    /// Network to LosOS's payout address there. Empty: no gateway passes.
    pub payouts: BTreeMap<String, String>,
}

impl Anchors {
    /// The three files a box ships: TPM makers' certificates (PEM: the
    /// self-signed ones are roots, the rest intermediates), the published
    /// gateway images (a JSON list of [`Image`]) and LosOS's payout
    /// addresses (a JSON object, network to address).
    pub fn from_texts(
        roots_pem: &str,
        images_json: &str,
        payouts_json: &str,
    ) -> Result<Anchors, String> {
        let (tpm_roots, tpm_intermediates) = roots_from_pem(roots_pem)
            .into_iter()
            .partition(|c| c.tbs_certificate.subject == c.tbs_certificate.issuer);
        Ok(Anchors {
            tpm_roots,
            tpm_intermediates,
            images: serde_json::from_str(images_json)
                .map_err(|e| format!("the gateway images: {e}"))?,
            payouts: serde_json::from_str(payouts_json)
                .map_err(|e| format!("the payout addresses: {e}"))?,
        })
    }
}

/// Certificates from a PEM bundle; anything not a certificate block is
/// skipped, and so is a block this box cannot parse: the makers' bundle
/// carries a few in encodings no EK chain here needs.
pub fn roots_from_pem(text: &str) -> Vec<Certificate> {
    let mut out = Vec::new();
    let mut body: Option<String> = None;
    for line in text.lines().map(str::trim) {
        if line == "-----BEGIN CERTIFICATE-----" {
            body = Some(String::new());
        } else if line == "-----END CERTIFICATE-----" {
            let b64 = body.take().unwrap_or_default();
            let Ok(der) = base64::engine::general_purpose::STANDARD.decode(b64) else {
                continue;
            };
            if let Ok(cert) = ekcert::parse(&der) {
                out.push(cert);
            }
        } else if let Some(b) = body.as_mut() {
            b.push_str(line);
        }
    }
    out
}

/// Why a gateway is not trusted.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Refusal {
    #[error("the gateway's answer is malformed: {0}")]
    Malformed(String),
    #[error("the passport does not hold: {0}")]
    Passport(PassportError),
    #[error("the passport is for {named}, not {asked}")]
    WrongUrl { named: String, asked: String },
    #[error("the nonce is not signed by the gateway's key")]
    NonceSignature,
    #[error("the fee settings differ from the ones this box ships: {0}")]
    Payout(String),
    #[error("this box ships no LosOS payout addresses, so no gateway can be checked")]
    NoPayouts,
    #[error("the TPM is not one this box knows: {0}")]
    UnknownTpm(String),
    #[error("the gateway runs an image this box has no measurements for")]
    UnknownImage,
    #[error("the firmware log has an entry in PCR 4 this box does not judge (kind {0:#x})")]
    UnknownBootEvent(u32),
    #[error("forged attestation: {0}")]
    Forged(&'static str),
    #[error("the gateway's settings or state changed after it booted (PCR 15)")]
    Tainted,
    #[error("the firmware started a program that is not part of the published image")]
    BootPath,
}

impl Refusal {
    /// Is this proof that someone changed the gateway? Then the box
    /// disables it for good. Everything else is a reason to wait.
    #[must_use]
    pub fn tampered(&self) -> bool {
        matches!(
            self,
            Refusal::Forged(_) | Refusal::Tainted | Refusal::BootPath
        )
    }

    /// A short name for the API and the UI.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Refusal::Malformed(_) => "malformed",
            Refusal::Passport(_) => "passport",
            Refusal::WrongUrl { .. } => "wrongUrl",
            Refusal::NonceSignature => "nonceSignature",
            Refusal::Payout(_) => "payoutChanged",
            Refusal::NoPayouts => "noPayouts",
            Refusal::UnknownTpm(_) => "unknownTpm",
            Refusal::UnknownImage => "unknownImage",
            Refusal::UnknownBootEvent(_) => "unknownBootEvent",
            Refusal::Forged(_) => "forged",
            Refusal::Tainted => "tainted",
            Refusal::BootPath => "bootPath",
        }
    }
}

fn malformed(what: impl std::fmt::Display) -> Refusal {
    Refusal::Malformed(what.to_string())
}

/// What the gateway signs with its Ed25519 key to show it holds it.
#[must_use]
pub fn nonce_message(nonce: &str) -> String {
    format!("{NONCE_PREFIX}\n{nonce}\n")
}

/// The qualifying data of the quote: binds it to this box's nonce and to
/// the edge key the passport names.
#[must_use]
pub fn qualifying_data(nonce: &str, edge_key: &str) -> [u8; 32] {
    Sha256::digest(format!("{QUOTE_PREFIX}\n{nonce}\n{edge_key}\n")).into()
}

/// The value PCR 15 holds on an untouched gateway: one extend, with the
/// settings file's SHA-256, from zero.
#[must_use]
pub fn expected_pcr15(settings: &str) -> [u8; 32] {
    tpm::extend(&[0; 32], &Sha256::digest(settings.as_bytes()))
}

/// A gateway that passed everything short of the TPM round trip.
#[derive(Debug, Clone)]
pub struct Introduced {
    pub url: String,
    pub statement: Statement,
    pub owner: Owner,
    pub settings: String,
    /// The endorsement key's SHA-256 name in hex: the TPM's identity, which
    /// a disabled gateway is remembered by.
    pub ek_id: String,
    ek: Public,
    ak_name: Vec<u8>,
}

/// The fee settings against what the box ships.
pub fn check_fees(
    fees: &Fees,
    owner: &BTreeMap<String, String>,
    ours: &BTreeMap<String, String>,
) -> Result<(), Refusal> {
    if ours.is_empty() {
        return Err(Refusal::NoPayouts);
    }
    if fees.losos_bps != LOSOS_BPS || fees.owner_bps != OWNER_BPS {
        return Err(Refusal::Payout(format!(
            "the split is {}/{} basis points, not {LOSOS_BPS}/{OWNER_BPS}",
            fees.losos_bps, fees.owner_bps
        )));
    }
    for (network, address) in ours {
        match fees.losos.get(network) {
            Some(a) if normalize_address(network, a) == normalize_address(network, address) => {}
            Some(a) => {
                return Err(Refusal::Payout(format!(
                    "LosOS's address on {network} is {a}, not {address}"
                )))
            }
            None => {
                return Err(Refusal::Payout(format!(
                    "LosOS's address on {network} is missing"
                )))
            }
        }
    }
    if let Some(extra) = fees.losos.keys().find(|n| !ours.contains_key(*n)) {
        return Err(Refusal::Payout(format!(
            "{extra} is not a network LosOS is paid on"
        )));
    }
    if let Some(extra) = owner.keys().find(|n| !ours.contains_key(*n)) {
        return Err(Refusal::Payout(format!(
            "the owner is paid on {extra}, where LosOS is not"
        )));
    }
    Ok(())
}

fn unhex(what: &str, s: &str) -> Result<Vec<u8>, Refusal> {
    hex::decode(s).map_err(|_| malformed(format!("{what} is not hex")))
}

fn unb64(what: &str, s: &str) -> Result<Vec<u8>, Refusal> {
    base64::engine::general_purpose::STANDARD
        .decode(s)
        .map_err(|_| malformed(format!("{what} is not base64")))
}

/// Check what the gateway said about itself in [`Hello`], for the URL the
/// box asked and the nonce it sent.
pub fn introduce(
    url: &str,
    nonce: &str,
    hello: &Hello,
    anchors: &Anchors,
    now: i64,
) -> Result<Introduced, Refusal> {
    let settings: Settings = serde_json::from_str(&hello.settings).map_err(malformed)?;
    if settings.version != SETTINGS_VERSION {
        return Err(malformed(format!("settings version {}", settings.version)));
    }
    let p = &settings.passport;
    let (statement, owner) =
        passport::verify(&p.statement, &p.signature, &p.key, now).map_err(Refusal::Passport)?;
    if statement.url != url.trim_end_matches('/') {
        return Err(Refusal::WrongUrl {
            named: statement.url,
            asked: url.to_string(),
        });
    }

    let key_bytes: [u8; 32] = unhex("the edge key", &statement.edge_key)?
        .try_into()
        .map_err(|_| malformed("the edge key"))?;
    let key = ed25519_dalek::VerifyingKey::from_bytes(&key_bytes)
        .map_err(|_| malformed("the edge key is not an Ed25519 key"))?;
    let sig_bytes: [u8; 64] = unhex("the nonce signature", &hello.nonce_signature)?
        .try_into()
        .map_err(|_| Refusal::NonceSignature)?;
    key.verify_strict(
        nonce_message(nonce).as_bytes(),
        &ed25519_dalek::Signature::from_bytes(&sig_bytes),
    )
    .map_err(|_| Refusal::NonceSignature)?;

    check_fees(&settings.fees, &statement.owner_payouts, &anchors.payouts)?;

    let t = &hello.tpm;
    let ek = Public::from_tpm2b(&unhex("the EK", &t.ek_public)?).map_err(malformed)?;
    let ak = Public::from_tpm2b(&unhex("the AK", &t.ak_public)?).map_err(malformed)?;
    tpm::check_ak(&ak).map_err(|e| Refusal::UnknownTpm(e.to_string()))?;
    let cert = ekcert::parse(&unb64("the EK certificate", &t.ek_cert)?)
        .map_err(|e| Refusal::UnknownTpm(e.to_string()))?;
    let chain = t
        .ek_chain
        .iter()
        .map(|c| {
            unb64("an EK chain certificate", c)
                .and_then(|der| ekcert::parse(&der).map_err(|e| Refusal::UnknownTpm(e.to_string())))
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .chain(anchors.tpm_intermediates.iter().cloned())
        .collect::<Vec<_>>();
    ekcert::certifies(&cert, &ek).map_err(|e| Refusal::UnknownTpm(e.to_string()))?;
    ekcert::chains_to(&cert, &chain, &anchors.tpm_roots)
        .map_err(|e| Refusal::UnknownTpm(e.to_string()))?;

    Ok(Introduced {
        url: statement.url.clone(),
        ek_id: hex::encode(ek.name().map_err(malformed)?),
        ak_name: ak.name().map_err(malformed)?,
        statement,
        owner,
        settings: hello.settings.clone(),
        ek,
    })
}

/// The secret a box keeps between the two round trips.
pub struct Challenge {
    secret: [u8; 32],
}

/// Make the credential for [`AttestRequest`]: only the TPM holding the
/// endorsement key, with the attestation key loaded, can open it.
pub fn challenge(
    intro: &Introduced,
    nonce: &str,
    rng: &mut (impl CryptoRng + RngCore),
) -> Result<(Challenge, AttestRequest), Refusal> {
    let mut secret = [0u8; 32];
    rng.fill_bytes(&mut secret);
    let (id, enc) = tpm::make_credential(&intro.ek, &intro.ak_name, &secret, rng)
        .map_err(|e| Refusal::UnknownTpm(e.to_string()))?;
    Ok((
        Challenge { secret },
        AttestRequest {
            nonce: nonce.to_string(),
            credential: hex::encode(tpm::credential_file(&id, &enc)),
        },
    ))
}

/// A gateway the box trusts, as of this check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Verdict {
    pub image: String,
    /// The TPM's boot counter: a change means the gateway rebooted.
    pub reset_count: u32,
}

/// Judge the answer to the challenge. The attestation key is the one
/// [`introduce`] saw (its name went into the credential), so only its
/// TPM could have opened the secret.
pub fn judge(
    intro: &Introduced,
    nonce: &str,
    challenge: &Challenge,
    answer: &AttestAnswer,
    ak_public: &str,
    anchors: &Anchors,
) -> Result<Verdict, Refusal> {
    let ak = Public::from_tpm2b(&unhex("the AK", ak_public)?).map_err(malformed)?;
    if ak.name().map_err(malformed)? != intro.ak_name {
        return Err(Refusal::Forged(
            "the attestation key changed between the two requests",
        ));
    }
    let secret = unhex("the secret", &answer.secret)?;
    if !bool::from(secret.ct_eq(&challenge.secret)) {
        return Err(Refusal::Forged(
            "the attestation key is not in the certified TPM",
        ));
    }

    let attest_bytes = unhex("the quote", &answer.attest)?;
    let sig =
        Signature::parse(&unhex("the quote's signature", &answer.signature)?).map_err(malformed)?;
    tpm::verify_quote(&ak, &attest_bytes, &sig)
        .map_err(|_| Refusal::Forged("the quote is not signed by the attestation key"))?;
    let attest = Attest::parse(&attest_bytes).map_err(malformed)?;
    if attest.extra_data != qualifying_data(nonce, &intro.statement.edge_key) {
        return Err(Refusal::Forged(
            "the quote is not for this nonce and edge key",
        ));
    }
    if attest.selection != vec![(ALG_SHA256, PCRS.to_vec())] {
        return Err(malformed("the quote is not over SHA-256 PCRs 4, 11 and 15"));
    }
    let mut values = [[0u8; 32]; 3];
    for (slot, index) in values.iter_mut().zip(PCRS) {
        let v = answer
            .pcrs
            .get(&index.to_string())
            .ok_or_else(|| malformed(format!("PCR {index} is missing")))?;
        *slot = unhex("a PCR", v)?
            .try_into()
            .map_err(|_| malformed("a PCR is not 32 bytes"))?;
    }
    if attest.pcr_digest != tpm::pcr_digest(&values) {
        return Err(Refusal::Forged("the PCR values are not the ones quoted"));
    }
    let [pcr4, pcr11, pcr15] = values;

    if pcr15 != expected_pcr15(&intro.settings) {
        return Err(Refusal::Tainted);
    }

    let events = eventlog::parse(&unb64("the event log", &answer.event_log)?).map_err(malformed)?;
    if eventlog::replay(&events, 4) != pcr4 {
        return Err(Refusal::Forged("the event log is not the history of PCR 4"));
    }
    let apps = eventlog::boot_applications(&events).map_err(Refusal::UnknownBootEvent)?;

    let pcr11_hex = hex::encode(pcr11);
    let image = anchors
        .images
        .iter()
        .find(|i| i.pcr11.eq_ignore_ascii_case(&pcr11_hex))
        .ok_or(Refusal::UnknownImage)?;
    if apps.is_empty()
        || apps.iter().any(|a| {
            let a = hex::encode(a);
            !image.apps.iter().any(|ok| ok.eq_ignore_ascii_case(&a))
        })
    {
        return Err(Refusal::BootPath);
    }

    Ok(Verdict {
        image: image.name.clone(),
        reset_count: attest.reset_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gateway::passport::tests::{armored_public, owner_key, sign, statement};
    use crate::gateway::tpm::tests::vectors;
    use ed25519_dalek::Signer;

    const URL: &str = "http://gw.local:8080";
    const NONCE: &str = "00112233445566778899aabbccddeeff";
    const BASE: &str = "0x52908400098527886E0F7030069857D2E4169EE7";

    fn ours() -> BTreeMap<String, String> {
        BTreeMap::from([(
            "base".to_string(),
            "0x1111111111111111111111111111111111111111".to_string(),
        )])
    }

    fn anchors() -> Anchors {
        Anchors {
            tpm_roots: vec![ekcert::parse(&hex::decode(vectors::CA_CERT).unwrap()).unwrap()],
            tpm_intermediates: vec![],
            images: vec![],
            payouts: ours(),
        }
    }

    fn hello(edge: &ed25519_dalek::SigningKey, losos: BTreeMap<String, String>) -> Hello {
        let key = owner_key("Ada Gateway <ada@example.net>");
        let text = statement(URL, &hex::encode(edge.verifying_key().as_bytes())).render();
        let settings = Settings {
            version: 1,
            passport: PassportFields {
                signature: sign(&key, &text),
                key: armored_public(&key),
                statement: text,
            },
            fees: Fees {
                losos_bps: 200,
                owner_bps: 200,
                losos,
            },
        };
        Hello {
            settings: serde_json::to_string(&settings).unwrap(),
            nonce_signature: hex::encode(edge.sign(nonce_message(NONCE).as_bytes()).to_bytes()),
            tpm: TpmIdentity {
                ek_public: vectors::EK_PUB.into(),
                ek_cert: base64::engine::general_purpose::STANDARD
                    .encode(hex::decode(vectors::EK_CERT).unwrap()),
                ek_chain: vec![],
                ak_public: vectors::AK_PUB.into(),
            },
        }
    }

    fn edge_key() -> ed25519_dalek::SigningKey {
        ed25519_dalek::SigningKey::from_bytes(&[9; 32])
    }

    #[test]
    fn an_honest_hello_is_introduced() {
        let intro = introduce(
            URL,
            NONCE,
            &hello(&edge_key(), ours()),
            &anchors(),
            1_800_000_000,
        )
        .unwrap();
        assert_eq!(intro.owner.user_id, "Ada Gateway <ada@example.net>");
        assert_eq!(intro.ek_id.len(), 68);
        let (_, req) = challenge(&intro, NONCE, &mut rand::thread_rng()).unwrap();
        assert!(tpm::parse_credential_file(&hex::decode(req.credential).unwrap()).is_ok());
    }

    #[test]
    fn changed_payouts_another_url_a_foreign_nonce_and_an_unknown_maker_are_refused() {
        let edge = edge_key();
        let mut changed = ours();
        changed.insert("base".into(), BASE.into());
        let r = introduce(
            URL,
            NONCE,
            &hello(&edge, changed),
            &anchors(),
            1_800_000_000,
        )
        .unwrap_err();
        assert_eq!(r.code(), "payoutChanged");
        assert!(!r.tampered());

        let r = introduce(
            "http://other:8080",
            NONCE,
            &hello(&edge, ours()),
            &anchors(),
            1_800_000_000,
        )
        .unwrap_err();
        assert_eq!(r.code(), "wrongUrl");

        let r = introduce(
            URL,
            "ffff",
            &hello(&edge, ours()),
            &anchors(),
            1_800_000_000,
        )
        .unwrap_err();
        assert_eq!(r, Refusal::NonceSignature);

        let mut no_roots = anchors();
        no_roots.tpm_roots.clear();
        let r = introduce(URL, NONCE, &hello(&edge, ours()), &no_roots, 1_800_000_000).unwrap_err();
        assert_eq!(r.code(), "unknownTpm");

        let mut none = anchors();
        none.payouts.clear();
        let r = introduce(
            URL,
            NONCE,
            &hello(&edge, BTreeMap::new()),
            &none,
            1_800_000_000,
        )
        .unwrap_err();
        assert_eq!(r, Refusal::NoPayouts);
    }

    #[test]
    fn fees_must_be_two_and_two_on_our_networks_only() {
        let fees = |l: u32, o: u32| Fees {
            losos_bps: l,
            owner_bps: o,
            losos: ours(),
        };
        let owner = BTreeMap::from([("base".to_string(), BASE.to_string())]);
        check_fees(&fees(200, 200), &owner, &ours()).unwrap();
        assert!(check_fees(&fees(100, 300), &owner, &ours()).is_err());
        let solana = BTreeMap::from([("solana".to_string(), "x".to_string())]);
        assert!(check_fees(&fees(200, 200), &solana, &ours()).is_err());
        // Case does not make an EVM address another address.
        let mut upper = fees(200, 200);
        upper.losos.insert(
            "base".into(),
            "0x1111111111111111111111111111111111111111"
                .to_uppercase()
                .replace("0X", "0x"),
        );
        check_fees(&upper, &owner, &ours()).unwrap();
    }

    /// A quote from swtpm: its qualifying data is a fixed value, not one
    /// derived from a nonce, so judging it stops at the binding check. The
    /// rest of [`judge`] runs in `nix/tests/edge-gateway-tpm.nix`.
    #[test]
    fn a_quote_for_another_nonce_is_forged_and_a_wrong_secret_too() {
        let intro = introduce(
            URL,
            NONCE,
            &hello(&edge_key(), ours()),
            &anchors(),
            1_800_000_000,
        )
        .unwrap();
        let (ch, _) = challenge(&intro, NONCE, &mut rand::thread_rng()).unwrap();
        let values = hex::decode(vectors::PCR_VALUES).unwrap();
        let pcrs = BTreeMap::from([
            ("4".to_string(), hex::encode(&values[..32])),
            ("11".to_string(), hex::encode(&values[32..64])),
            ("15".to_string(), hex::encode(&values[64..])),
        ]);
        let mut answer = AttestAnswer {
            secret: hex::encode(ch.secret),
            attest: vectors::ATTEST.into(),
            signature: vectors::SIG.into(),
            pcrs,
            event_log: String::new(),
        };
        let r = judge(&intro, NONCE, &ch, &answer, vectors::AK_PUB, &anchors()).unwrap_err();
        assert_eq!(
            r,
            Refusal::Forged("the quote is not for this nonce and edge key")
        );
        assert!(r.tampered());

        answer.secret = hex::encode([0u8; 32]);
        let r = judge(&intro, NONCE, &ch, &answer, vectors::AK_PUB, &anchors()).unwrap_err();
        assert_eq!(
            r,
            Refusal::Forged("the attestation key is not in the certified TPM")
        );

        answer.secret = hex::encode(ch.secret);
        let r = judge(&intro, NONCE, &ch, &answer, vectors::EK_PUB, &anchors()).unwrap_err();
        assert!(r.tampered());
    }

    #[test]
    fn pcr15_is_one_extend_of_the_settings_hash() {
        assert_eq!(
            expected_pcr15("hello"),
            tpm::extend(&[0; 32], &Sha256::digest(b"hello"))
        );
        let pem = format!(
            "junk\n-----BEGIN CERTIFICATE-----\n{}\n-----END CERTIFICATE-----\n",
            base64::engine::general_purpose::STANDARD
                .encode(hex::decode(vectors::CA_CERT).unwrap())
        );
        assert_eq!(roots_from_pem(&pem).len(), 1);
    }

    /// The real bundle (Microsoft's TrustedTpm.cab as PEM, `.#losos-tpm-roots`):
    /// `LOSOS_TPM_ROOTS=… cargo test -- --ignored`.
    #[test]
    #[ignore = "needs the TPM makers' bundle"]
    fn the_makers_bundle_parses_into_roots_and_intermediates() {
        let path = std::env::var("LOSOS_TPM_ROOTS").expect("LOSOS_TPM_ROOTS");
        let pem = std::fs::read_to_string(path).unwrap();
        let a = Anchors::from_texts(&pem, "[]", "{}").unwrap();
        assert!(a.tpm_roots.len() > 30, "{} roots", a.tpm_roots.len());
        assert!(
            a.tpm_intermediates.len() > 2000,
            "{} intermediates",
            a.tpm_intermediates.len()
        );
        assert!(!a.tpm_intermediates.is_empty());
    }
}
