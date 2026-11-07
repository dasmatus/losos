//! Native TLS trust verification, adapted from `admin-ui/tui/src/http.rs`.
//!
//! Replaces blanket `danger_accept_invalid_certs` with explicit trust modes:
//! public CA, pinned cert hash (TOFU), or first-use recording. This satisfies
//! security checks (CodeQL) while supporting LosOS boxes with self-signed certs.

#![cfg(not(target_arch = "wasm32"))]

use losos_client::Trust;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::client::WebPkiServerVerifier;
use rustls::crypto::{ring, verify_tls12_signature, verify_tls13_signature, CryptoProvider};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, RootCertStore, SignatureScheme};
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex, OnceLock};

fn crypto_provider() -> Arc<CryptoProvider> {
    static P: OnceLock<Arc<CryptoProvider>> = OnceLock::new();
    P.get_or_init(|| Arc::new(ring::default_provider())).clone()
}

/// Observed certificate state from the TLS verifier (native only).
#[derive(Clone, Default, Debug)]
pub struct TlsObservation {
    seen: Arc<Mutex<Option<String>>>,
    changed: Arc<Mutex<bool>>,
}

impl TlsObservation {
    pub fn seen_hash(&self) -> Option<String> {
        self.seen.lock().ok().and_then(|g| g.clone())
    }

    pub fn pin_changed(&self) -> bool {
        self.changed.lock().ok().map(|g| *g).unwrap_or(false)
    }
}

#[derive(Debug)]
struct Verifier {
    web: Arc<WebPkiServerVerifier>,
    provider: Arc<CryptoProvider>,
    trust: Trust,
    seen: Arc<Mutex<Option<String>>>,
    changed: Arc<Mutex<bool>>,
}

fn sha256_hex(der: &[u8]) -> String {
    hex::encode(Sha256::digest(der))
}

impl ServerCertVerifier for Verifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let public = self.web.verify_server_cert(
            end_entity,
            intermediates,
            server_name,
            ocsp_response,
            now,
        );
        let err = match public {
            Ok(v) => return Ok(v),
            Err(e) => e,
        };
        let hash = sha256_hex(end_entity.as_ref());
        match &self.trust {
            Trust::Pinned(pin) if pin.eq_ignore_ascii_case(&hash) => {
                if let Ok(mut g) = self.seen.lock() {
                    *g = Some(hash);
                }
                Ok(ServerCertVerified::assertion())
            }
            Trust::Pinned(_) => {
                if let Ok(mut g) = self.changed.lock() {
                    *g = true;
                }
                Err(err)
            }
            // TOFU / explicit first-use: accept and record the certificate hash so
            // the caller can persist it as a pin. `Permissive` is treated as first-use
            // here (no silent accept-all without recording); prefer `FirstUse`/pins.
            Trust::FirstUse | Trust::Permissive => {
                if let Ok(mut g) = self.seen.lock() {
                    *g = Some(hash);
                }
                Ok(ServerCertVerified::assertion())
            }
            Trust::Public => Err(err),
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

pub fn client_config(trust: &Trust) -> Result<(rustls::ClientConfig, TlsObservation), String> {
    let provider = crypto_provider();
    // Ensure a process-wide default for libraries that rely on it (best-effort).
    let _ = ring::default_provider().install_default();
    let mut roots = RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let web = WebPkiServerVerifier::builder_with_provider(Arc::new(roots), provider.clone())
        .build()
        .map_err(|e| e.to_string())?;
    let obs = TlsObservation::default();
    let verifier = Arc::new(Verifier {
        web,
        provider: provider.clone(),
        trust: trust.clone(),
        seen: Arc::clone(&obs.seen),
        changed: Arc::clone(&obs.changed),
    });
    let config = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| e.to_string())?
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth();
    Ok((config, obs))
}
