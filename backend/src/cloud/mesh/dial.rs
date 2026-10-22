//! Reaching another box: a WebSocket at `wss://<name>/mesh/v1/link`.
//!
//! Who is at the other end is settled by the handshake's signatures
//! (`link`), not by the certificate. Boxes on a LAN answer with
//! certificates they made themselves, so with `self_signed` the certificate
//! is not checked: the connection is still encrypted, and a box in the
//! middle can neither pose as either end nor change what they send, but it
//! could read it. Through an edge with a real certificate, turn it off.

use std::collections::HashMap;
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, DigitallySignedStruct, SignatureScheme};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::Connector;

use super::link::{Frame, Wire};
use super::{DialFn, MeshError};

/// The largest frame a link carries: a 16 MiB file read, in base64.
pub const MAX_FRAME: usize = 24 * 1024 * 1024;
/// Frames queued each way before the sender waits.
const QUEUE: usize = 512;

/// How this box dials others.
#[derive(Clone, Default)]
pub struct Dialer {
    /// Accept a certificate nothing vouches for.
    pub self_signed: bool,
    /// Where to dial a name instead of `wss://<name>/mesh/v1/link`, for
    /// tests.
    pub urls: HashMap<String, String>,
}

impl Dialer {
    pub fn into_fn(self) -> DialFn {
        let me = Arc::new(self);
        Arc::new(move |name: String| {
            let me = me.clone();
            Box::pin(async move { me.dial(&name).await })
        })
    }

    async fn dial(&self, name: &str) -> Result<Wire, MeshError> {
        let url = self
            .urls
            .get(name)
            .cloned()
            .unwrap_or_else(|| format!("wss://{name}/mesh/v1/link"));
        let connector = if url.starts_with("wss://") {
            Some(Connector::Rustls(Arc::new(tls_config(self.self_signed)?)))
        } else {
            None
        };
        let config = WebSocketConfig::default()
            .max_message_size(Some(MAX_FRAME))
            .max_frame_size(Some(MAX_FRAME));
        let connect = tokio_tungstenite::connect_async_tls_with_config(
            url.as_str(),
            Some(config),
            false,
            connector,
        );
        let (ws, _) = tokio::time::timeout(super::link::STEP, connect)
            .await
            .map_err(|_| MeshError::Unreachable(name.into()))?
            .map_err(|e| {
                tracing::info!(peer = name, error = %e, "mesh: dial failed");
                MeshError::Unreachable(name.into())
            })?;
        Ok(pump(ws))
    }
}

/// Bridge a WebSocket to a pair of frame channels.
pub fn pump<S>(ws: tokio_tungstenite::WebSocketStream<S>) -> Wire
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let (mut sink, mut stream) = ws.split();
    let (out_tx, mut out_rx) = mpsc::channel::<Frame>(QUEUE);
    let (in_tx, in_rx) = mpsc::channel::<Frame>(QUEUE);
    tokio::spawn(async move {
        while let Some(f) = out_rx.recv().await {
            let m = match f {
                Frame::Text(t) => Message::Text(t.into()),
                Frame::Binary(b) => Message::Binary(b.into()),
            };
            if sink.send(m).await.is_err() {
                break;
            }
        }
        let _ = sink.close().await;
    });
    tokio::spawn(async move {
        while let Some(Ok(m)) = stream.next().await {
            let f = match m {
                Message::Text(t) => Frame::Text(t.to_string()),
                Message::Binary(b) => Frame::Binary(b.to_vec()),
                Message::Close(_) => break,
                _ => continue,
            };
            if in_tx.send(f).await.is_err() {
                break;
            }
        }
    });
    (out_tx, in_rx)
}

fn tls_config(self_signed: bool) -> Result<ClientConfig, MeshError> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let builder = ClientConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions()
        .map_err(|_| MeshError::Protocol)?;
    if self_signed {
        return Ok(builder
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(AnyCertificate(provider)))
            .with_no_client_auth());
    }
    let mut roots = rustls::RootCertStore::empty();
    for cert in rustls_native_certs::load_native_certs().certs {
        let _ = roots.add(cert);
    }
    Ok(builder.with_root_certificates(roots).with_no_client_auth())
}

/// Takes any certificate, but still checks that the server holds its key,
/// so the connection is encrypted to whoever answered.
#[derive(Debug)]
struct AnyCertificate(Arc<rustls::crypto::CryptoProvider>);

impl ServerCertVerifier for AnyCertificate {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}
