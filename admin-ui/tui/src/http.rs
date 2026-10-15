//! HTTP and HTTPS for the two kinds of request this client makes: the host
//! key at login, and the envelope after it.
//!
//! The certificate check is ours. A certificate a public authority vouches
//! for passes as anywhere. A box serves its own self-signed one
//! (`modules/tls.nix`), which no authority knows, so the client pins it the
//! way it pins the host key: at login the certificate is taken as shown and
//! its SHA-256 kept with the host, and every later call must see that same
//! certificate. Login can take any certificate because trust there comes
//! from the host key's fingerprint, which the owner compares by eye, and
//! every call after it is sealed to that key whatever carries it.

use std::io::Read;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::client::WebPkiServerVerifier;
use rustls::crypto::{ring, verify_tls12_signature, verify_tls13_signature, CryptoProvider};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, RootCertStore, SignatureScheme};
use sha2::{Digest, Sha256};

/// Which certificates a connection accepts beyond the ones the web trusts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trust {
    /// Only a certificate a public authority vouches for.
    Public,
    /// That, or exactly this certificate (SHA-256, hex).
    Pinned(String),
    /// Any certificate; the one shown is recorded. Login only.
    FirstUse,
}

#[derive(Debug)]
struct Verifier {
    web: Arc<WebPkiServerVerifier>,
    provider: Arc<CryptoProvider>,
    trust: Trust,
    /// The hash of a certificate that passed only by `trust`.
    seen: Mutex<Option<String>>,
    /// A pinned certificate was replaced by another.
    changed: Mutex<bool>,
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
        let public =
            self.web
                .verify_server_cert(end_entity, intermediates, server_name, ocsp_response, now);
        let error = match public {
            Ok(verified) => return Ok(verified),
            Err(e) => e,
        };
        let hash = sha256_hex(end_entity.as_ref());
        match &self.trust {
            Trust::Pinned(pin) if pin.eq_ignore_ascii_case(&hash) => {
                *self.seen.lock().unwrap() = Some(hash);
                Ok(ServerCertVerified::assertion())
            }
            Trust::Pinned(_) => {
                tracing::warn!(certificate = %hash, "not the pinned certificate");
                *self.changed.lock().unwrap() = true;
                Err(error)
            }
            Trust::FirstUse => {
                tracing::info!(certificate = %hash, %error, "certificate taken at login");
                *self.seen.lock().unwrap() = Some(hash);
                Ok(ServerCertVerified::assertion())
            }
            Trust::Public => Err(error),
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

#[derive(Debug, Clone)]
pub struct Response {
    pub status: u16,
    pub content_type: String,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpError {
    /// No answer: DNS, refused, a timeout, a certificate nobody vouches for.
    Unreachable(String),
    /// The pinned certificate was replaced.
    CertificateChanged,
}

/// One client per host, with that host's trust.
#[derive(Clone)]
pub struct Http {
    agent: ureq::Agent,
    verifier: Arc<Verifier>,
}

/// The largest answer read: the envelope's own limit is 1 MiB of plaintext,
/// and armour adds a third.
const MAX_BODY: u64 = 4 << 20;

impl Http {
    pub fn new(trust: Trust) -> Http {
        let provider = Arc::new(ring::default_provider());
        let mut roots = RootCertStore::empty();
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let web = WebPkiServerVerifier::builder_with_provider(Arc::new(roots), provider.clone())
            .build()
            .expect("the bundled roots are valid");
        let verifier = Arc::new(Verifier {
            web,
            provider: provider.clone(),
            trust,
            seen: Mutex::new(None),
            changed: Mutex::new(false),
        });
        let config = rustls::ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .expect("ring supports the default versions")
            .dangerous()
            .with_custom_certificate_verifier(verifier.clone())
            .with_no_client_auth();
        let agent = ureq::AgentBuilder::new()
            .tls_config(Arc::new(config))
            .timeout_connect(Duration::from_secs(15))
            .timeout(Duration::from_secs(120))
            .redirects(0)
            .build();
        Http { agent, verifier }
    }

    /// The SHA-256 of a certificate that was taken only because of the
    /// trust this client was made with: what login pins.
    pub fn taken_certificate(&self) -> Option<String> {
        self.verifier.seen.lock().unwrap().clone()
    }

    pub fn get(&self, url: &str, accept: &str) -> Result<Response, HttpError> {
        self.send(self.agent.get(url).set("Accept", accept), None)
    }

    pub fn post(
        &self,
        url: &str,
        content_type: &str,
        accept: &str,
        body: &str,
    ) -> Result<Response, HttpError> {
        let request = self
            .agent
            .post(url)
            .set("Content-Type", content_type)
            .set("Accept", accept);
        self.send(request, Some(body))
    }

    fn send(&self, request: ureq::Request, body: Option<&str>) -> Result<Response, HttpError> {
        let url = request.url().to_string();
        let result = match body {
            Some(b) => request.send_string(b),
            None => request.call(),
        };
        let response = match result {
            Ok(r) => r,
            Err(ureq::Error::Status(_, r)) => r,
            Err(ureq::Error::Transport(t)) => {
                if *self.verifier.changed.lock().unwrap() {
                    return Err(HttpError::CertificateChanged);
                }
                let mut detail = t.to_string();
                let mut source = std::error::Error::source(&t);
                while let Some(cause) = source {
                    let more = cause.to_string();
                    if !detail.contains(&more) {
                        detail = format!("{detail}: {more}");
                    }
                    source = cause.source();
                }
                tracing::debug!(%url, %detail, "no answer");
                return Err(HttpError::Unreachable(detail));
            }
        };
        let status = response.status();
        let content_type = response.header("content-type").unwrap_or("").to_string();
        let mut text = String::new();
        response
            .into_reader()
            .take(MAX_BODY)
            .read_to_string(&mut text)
            .map_err(|e| HttpError::Unreachable(e.to_string()))?;
        tracing::debug!(%url, status, %content_type, bytes = text.len(), "answer");
        Ok(Response {
            status,
            content_type,
            body: text,
        })
    }
}
