//! People from other boxes, known by their chat address.
//!
//! A person on another box (`@eva:kitchenbox.local`) has no account here
//! and no password to give. Their own box vouches for them the way Matrix
//! vouches for a person to any service: it hands them an OpenID token
//! (lososd's `POST /api/chat/openid`), they bring it here, and this box asks
//! their homeserver whose token it is, at
//! `https://<server>/_matrix/federation/v1/openid/userinfo`. The answer
//! names the person, and only a person of that same server: a server
//! cannot vouch for someone on another.
//!
//! The person then gets an account of their own here ([`Store::ensure_remote_user`]),
//! whose login is their address without the `@` and which no person of
//! this box can have, and a token for this browser. What they may do is
//! [`super::auth`]'s: read and take part in public repositories and in the
//! ones whose owner made them a collaborator, never the box's internal ones.
//!
//! Boxes on a LAN answer with certificates they made themselves. With
//! `self_signed` the certificate is not checked against an authority;
//! instead the key in it is kept the first time a server answers
//! (`matrix_servers`), and a different key later is refused, the way the
//! cloud's mesh links keep a box's key.
//!
//! [`Store::ensure_remote_user`]: super::db::Store::ensure_remote_user

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use actix_web::{web, HttpRequest, HttpResponse};
use rusqlite::{params, OptionalExtension};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, DigitallySignedStruct, SignatureScheme};
use serde::Deserialize;
use serde_json::{json, Value};

use super::db::Store;
use super::server::{self, Fail, State};

type Answer = Result<HttpResponse, Fail>;

/// How long one question to another server may take.
const TIMEOUT: Duration = Duration::from_secs(10);
/// The most of an answer that is read.
const MAX_ANSWER: usize = 64 * 1024;
/// What a token for a person from another box may do: read, take part in
/// issues and pull requests, and push to their own branches
/// ([`super::transport`] keeps them there).
pub const SCOPES: [&str; 3] = ["write:issue", "write:repository", "read:user"];

/// How this box reaches other homeservers.
#[derive(Debug, Clone, Default)]
pub struct MatrixConfig {
    /// Take a certificate nothing vouches for, and keep its key.
    pub self_signed: bool,
    /// Where to reach a server instead of finding it (`name` to a base URL
    /// such as `http://127.0.0.1:8448`), for tests.
    pub urls: HashMap<String, String>,
    /// This box's own chat server name: its people sign in with LosOS
    /// Login, never through it.
    pub own: Option<String>,
}

/// `@eva:kitchenbox.local` → (`eva`, `kitchenbox.local`), or `None` when
/// it is not a user ID of the shape Matrix allows.
pub fn split_id(id: &str) -> Option<(&str, &str)> {
    let rest = id.strip_prefix('@')?;
    let (local, server) = rest.split_once(':')?;
    let local_ok = !local.is_empty()
        && local.len() <= 64
        && local
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "._=-/".contains(c));
    (local_ok && server_name_ok(server) && id.len() <= 255).then_some((local, server))
}

/// A server name: a host name, an IPv4 address, or either with a port.
pub fn server_name_ok(name: &str) -> bool {
    let (host, port) = match name.rsplit_once(':') {
        Some((h, p)) => (h, Some(p)),
        None => (name, None),
    };
    let port_ok = port.is_none_or(|p| p.parse::<u16>().is_ok_and(|n| n > 0));
    let host_ok = !host.is_empty()
        && host.len() <= 253
        && host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        });
    host_ok && port_ok
}

/// The login of the account a person from another box gets here:
/// `@eva:kitchenbox.local` → `eva@kitchenbox.local`.
pub fn login_of(id: &str) -> Option<String> {
    let (local, server) = split_id(id)?;
    Some(format!("{local}@{server}"))
}

/// Why another server's word could not be had.
#[derive(Debug, thiserror::Error)]
pub enum AskError {
    #[error("{0} did not answer")]
    Unreachable(String),
    #[error("{0} answered with a key it did not have before")]
    KeyChanged(String),
    #[error("{0} gave an answer that is not Matrix's")]
    Garbled(String),
}

/// Ask `server` whose OpenID token `token` is, and return the user ID,
/// which is always on `server`. `Ok(None)`: the server does not know the
/// token (expired, made up, or for another server).
pub fn whose_token(
    store: &Store,
    config: &MatrixConfig,
    server: &str,
    token: &str,
) -> Result<Option<String>, AskError> {
    let base = base_url(store, config, server)?;
    let url = format!(
        "{base}/_matrix/federation/v1/openid/userinfo?access_token={}",
        encode(token)
    );
    let (status, body) = get(store, config, server, &url)?;
    if status != 200 {
        return Ok(None);
    }
    let sub = body
        .get("sub")
        .and_then(Value::as_str)
        .ok_or_else(|| AskError::Garbled(server.into()))?;
    match split_id(sub) {
        Some((_, theirs)) if theirs.eq_ignore_ascii_case(server) => Ok(Some(sub.to_string())),
        _ => Ok(None),
    }
}

/// Where a server's federation API is: a test's address, the server name
/// itself when it carries a port, what `/.well-known/matrix/server` says,
/// or port 8448.
fn base_url(store: &Store, config: &MatrixConfig, server: &str) -> Result<String, AskError> {
    if let Some(url) = config.urls.get(server) {
        return Ok(url.trim_end_matches('/').to_string());
    }
    if server.contains(':') {
        return Ok(format!("https://{server}"));
    }
    let known = get(
        store,
        config,
        server,
        &format!("https://{server}/.well-known/matrix/server"),
    );
    if let Ok((200, body)) = known {
        if let Some(to) = body.get("m.server").and_then(Value::as_str) {
            if server_name_ok(to) {
                let to = if to.contains(':') {
                    to.to_string()
                } else {
                    format!("{to}:8448")
                };
                return Ok(format!("https://{to}"));
            }
        }
    }
    Ok(format!("https://{server}:8448"))
}

/// Whether an address may be another box: not this one (loopback, the
/// unspecified address) and not link-local or multicast.
fn reachable(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => {
            !(v4.is_loopback()
                || v4.is_unspecified()
                || v4.is_link_local()
                || v4.is_multicast()
                || v4.is_broadcast())
        }
        std::net::IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return reachable(std::net::IpAddr::V4(v4));
            }
            !(v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || (v6.segments()[0] & 0xffc0) == 0xfe80)
        }
    }
}

/// One `GET`, answered as JSON. `server` names whose key a self-signed
/// certificate is kept under.
fn get(
    store: &Store,
    config: &MatrixConfig,
    server: &str,
    url: &str,
) -> Result<(u16, Value), AskError> {
    let unreachable = || AskError::Unreachable(server.to_string());
    let (tls, rest) = if let Some(r) = url.strip_prefix("https://") {
        (true, r)
    } else if let Some(r) = url.strip_prefix("http://") {
        (false, r)
    } else {
        return Err(unreachable());
    };
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => (h, p.parse::<u16>().map_err(|_| unreachable())?),
        None => (authority, if tls { 443 } else { 80 }),
    };
    let addr = (host, port)
        .to_socket_addrs()
        .map_err(|_| unreachable())?
        .next()
        .ok_or_else(unreachable)?;
    // Another server's name never leads back into this box.
    if !config.urls.contains_key(server) && !reachable(addr.ip()) {
        return Err(unreachable());
    }
    let tcp = TcpStream::connect_timeout(&addr, TIMEOUT).map_err(|_| unreachable())?;
    tcp.set_read_timeout(Some(TIMEOUT))
        .map_err(|_| unreachable())?;
    tcp.set_write_timeout(Some(TIMEOUT))
        .map_err(|_| unreachable())?;
    let request = format!(
        "GET {path} HTTP/1.0\r\nHost: {authority}\r\nAccept: application/json\r\nUser-Agent: LosOS Git\r\n\r\n"
    );
    let mut raw = Vec::new();
    if tls {
        let seen = Arc::new(Mutex::new(None));
        let tls_config =
            client_config(config.self_signed, seen.clone()).map_err(|_| unreachable())?;
        let name = ServerName::try_from(host.to_string()).map_err(|_| unreachable())?;
        let conn =
            rustls::ClientConnection::new(Arc::new(tls_config), name).map_err(|_| unreachable())?;
        let mut stream = rustls::StreamOwned::new(conn, tcp);
        stream
            .write_all(request.as_bytes())
            .map_err(|_| unreachable())?;
        if config.self_signed {
            let key = seen
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone()
                .ok_or_else(unreachable)?;
            keep_key(store, server, &key)?;
        }
        read_all(&mut stream, &mut raw);
    } else {
        let mut tcp = tcp;
        tcp.write_all(request.as_bytes())
            .map_err(|_| unreachable())?;
        read_all(&mut tcp, &mut raw);
    }
    let text = String::from_utf8_lossy(&raw);
    let (head, body) = text
        .split_once("\r\n\r\n")
        .ok_or_else(|| AskError::Garbled(server.into()))?;
    let status = head
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<u16>().ok())
        .ok_or_else(|| AskError::Garbled(server.into()))?;
    let body = serde_json::from_str(body.trim()).unwrap_or(Value::Null);
    Ok((status, body))
}

/// Read until the other end closes, or the answer is too long, or a read
/// fails (a TLS close without notify included): what came is the answer.
fn read_all(r: &mut impl Read, out: &mut Vec<u8>) {
    let mut buf = [0u8; 8192];
    while out.len() < MAX_ANSWER {
        match r.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => out.extend_from_slice(&buf[..n]),
        }
    }
}

/// The first key a server answered with is kept; another one later is
/// refused.
fn keep_key(store: &Store, server: &str, key: &str) -> Result<(), AskError> {
    let name = server.to_ascii_lowercase();
    let conn = store.conn();
    let known: Option<String> = conn
        .query_row(
            "SELECT key FROM matrix_servers WHERE name = ?1",
            [&name],
            |r| r.get(0),
        )
        .optional()
        .map_err(|_| AskError::Unreachable(server.into()))?;
    match known {
        Some(k) if k == key => Ok(()),
        Some(_) => Err(AskError::KeyChanged(server.into())),
        None => {
            conn.execute(
                "INSERT INTO matrix_servers (name, key, seen) VALUES (?1, ?2, ?3)",
                params![name, key, super::now()],
            )
            .map_err(|_| AskError::Unreachable(server.into()))?;
            Ok(())
        }
    }
}

fn client_config(
    self_signed: bool,
    seen: Arc<Mutex<Option<String>>>,
) -> Result<ClientConfig, rustls::Error> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let builder = ClientConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions()?;
    if self_signed {
        return Ok(builder
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(KeepKey { provider, seen }))
            .with_no_client_auth());
    }
    let mut roots = rustls::RootCertStore::empty();
    for cert in rustls_native_certs::load_native_certs().certs {
        let _ = roots.add(cert);
    }
    Ok(builder.with_root_certificates(roots).with_no_client_auth())
}

/// Takes any certificate, notes the SHA-256 of the public key in it, and
/// still checks that the server holds that key.
#[derive(Debug)]
struct KeepKey {
    provider: Arc<rustls::crypto::CryptoProvider>,
    seen: Arc<Mutex<Option<String>>>,
}

impl ServerCertVerifier for KeepKey {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let spki = public_key_info(end_entity.as_ref()).ok_or(
            rustls::Error::InvalidCertificate(rustls::CertificateError::BadEncoding),
        )?;
        let digest = ring::digest::digest(&ring::digest::SHA256, spki);
        *self
            .seen
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(hex(digest.as_ref()));
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
            &self.provider.signature_verification_algorithms,
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
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

/// One DER element at the start of `der`: (tag, contents, the whole
/// element's length).
fn der_element(der: &[u8]) -> Option<(u8, &[u8], usize)> {
    let tag = *der.first()?;
    let first = *der.get(1)? as usize;
    let (len, head) = if first < 0x80 {
        (first, 2)
    } else {
        let n = first & 0x7f;
        if n == 0 || n > 4 {
            return None;
        }
        let mut len = 0usize;
        for i in 0..n {
            len = (len << 8) | *der.get(2 + i)? as usize;
        }
        (len, 2 + n)
    };
    let end = head.checked_add(len)?;
    Some((tag, der.get(head..end)?, end))
}

/// The SubjectPublicKeyInfo of an X.509 certificate, as DER: the seventh
/// element of the to-be-signed part, or the sixth when it has no version.
fn public_key_info(cert: &[u8]) -> Option<&[u8]> {
    let (_, certificate, _) = der_element(cert)?;
    let (_, tbs, _) = der_element(certificate)?;
    let mut rest = tbs;
    let mut index = 0;
    loop {
        let (tag, _, len) = der_element(rest)?;
        // [0] version is optional: skip it without counting.
        let counted = tag != 0xa0;
        if counted {
            if index == 5 {
                return Some(&rest[..len]);
            }
            index += 1;
        }
        rest = &rest[len..];
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Percent-encoding for a query value.
fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

// ── The routes ──────────────────────────────────────────────────────────────

pub fn routes(scope: actix_web::Scope) -> actix_web::Scope {
    scope.route("/matrix/session", web::post().to(session))
}

#[derive(Deserialize)]
struct NewSession {
    /// The OpenID token the person's own box gave them.
    access_token: String,
    /// Their homeserver, which made it.
    matrix_server_name: String,
    /// What they are called there, to show beside their address.
    #[serde(default)]
    display_name: String,
}

/// `POST /api/v1/matrix/session`: a person from another box signs in with
/// the OpenID token their box gave them, and gets a token for this
/// browser.
async fn session(req: HttpRequest, body: web::Json<NewSession>, state: web::Data<State>) -> Answer {
    // A person of this box is signed in already; this is for the others.
    let c = server::caller(&req, &state)?;
    if c.user().is_some_and(|u| !u.is_remote()) {
        return Err(Fail::bad("You are signed in to this box already."));
    }
    let server = body.matrix_server_name.trim().to_ascii_lowercase();
    if !server_name_ok(&server) {
        return Err(Fail::bad("That is not a chat server's name."));
    }
    if state.config.matrix.own.as_deref() == Some(server.as_str()) {
        return Err(Fail::bad(
            "That is this box's own chat server: sign in here instead.",
        ));
    }
    if body.access_token.is_empty() || body.access_token.len() > 512 {
        return Err(Fail::bad("The token is missing."));
    }
    let display = body
        .display_name
        .trim()
        .chars()
        .take(64)
        .collect::<String>();
    let token = body.access_token.clone();
    let state2 = state.clone();
    let server2 = server.clone();
    let asked =
        web::block(move || whose_token(&state2.store, &state2.config.matrix, &server2, &token))
            .await
            .map_err(|_| {
                Fail::new(
                    actix_web::http::StatusCode::BAD_GATEWAY,
                    "The check stopped.",
                )
            })?;
    let id = match asked {
        Ok(Some(id)) => id,
        Ok(None) => {
            return Err(Fail::unauthorized(format!(
                "{server} does not vouch for this sign-in. Start again from your own box."
            )))
        }
        Err(e) => {
            return Err(Fail::new(
                actix_web::http::StatusCode::BAD_GATEWAY,
                e.to_string(),
            ))
        }
    };
    let user = state.store.ensure_remote_user(&id, &display)?;
    let scopes: Vec<String> = SCOPES.iter().map(|s| s.to_string()).collect();
    let name = format!("Signed in from {server} {}", super::now());
    let (_, value) = state.store.create_token(user.id, &name, &scopes)?;
    tracing::info!(person = %id, "a person from another box signed in");
    Ok(HttpResponse::Created().json(json!({
        "token": value,
        "login": user.login,
        "matrix_id": id,
        "full_name": user.full_name,
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_server_name_never_leads_back_into_this_box() {
        for ip in [
            "127.0.0.1",
            "0.0.0.0",
            "169.254.1.1",
            "::1",
            "fe80::1",
            "::ffff:127.0.0.1",
            "224.0.0.251",
        ] {
            assert!(!reachable(ip.parse().unwrap()), "{ip}");
        }
        for ip in ["192.168.1.20", "10.0.0.5", "203.0.113.7", "fd00::5"] {
            assert!(reachable(ip.parse().unwrap()), "{ip}");
        }
    }

    #[test]
    fn addresses_split_and_become_logins_no_box_account_can_have() {
        assert_eq!(
            split_id("@eva:kitchenbox.local"),
            Some(("eva", "kitchenbox.local"))
        );
        assert_eq!(
            login_of("@eva:kitchenbox.local").as_deref(),
            Some("eva@kitchenbox.local")
        );
        assert!(super::super::names::owner("eva@kitchenbox.local").is_err());
        assert_eq!(
            split_id("@eva:box.example:8448"),
            Some(("eva", "box.example:8448"))
        );
        for bad in [
            "eva:kitchenbox.local",
            "@:kitchenbox.local",
            "@Eva:kitchenbox.local",
            "@eva",
            "@eva:",
            "@eva:-bad.local",
            "@eva:box.local:99999",
            "@eva:box..local",
            "@e va:box.local",
        ] {
            assert_eq!(split_id(bad), None, "{bad}");
        }
    }

    #[test]
    fn the_public_key_is_found_in_a_certificate() {
        // A minimal certificate shape: SEQ { SEQ tbs { [0] v, serial,
        // alg, issuer, validity, subject, spki }, alg, sig }.
        fn tlv(tag: u8, body: &[u8]) -> Vec<u8> {
            let mut v = vec![tag, body.len() as u8];
            v.extend_from_slice(body);
            v
        }
        let spki = tlv(0x30, b"key");
        let mut tbs = Vec::new();
        tbs.extend(tlv(0xa0, &tlv(0x02, &[2])));
        tbs.extend(tlv(0x02, &[7]));
        for _ in 0..4 {
            tbs.extend(tlv(0x30, b"x"));
        }
        tbs.extend(spki.clone());
        let mut cert = tlv(0x30, &tbs);
        cert.extend(tlv(0x30, b"alg"));
        cert.extend(tlv(0x03, b"sig"));
        let cert = tlv(0x30, &cert);
        assert_eq!(public_key_info(&cert), Some(spki.as_slice()));
        assert_eq!(public_key_info(&[0x30, 0x05, 0x30]), None);
    }

    #[test]
    fn a_server_keeps_the_key_it_first_answered_with() {
        let store = Store::in_memory().unwrap();
        keep_key(&store, "kitchenbox.local", "k1").unwrap();
        keep_key(&store, "KITCHENBOX.local", "k1").unwrap();
        assert!(matches!(
            keep_key(&store, "kitchenbox.local", "k2"),
            Err(AskError::KeyChanged(_))
        ));
    }

    #[test]
    fn query_values_are_encoded() {
        assert_eq!(encode("a b/c+d"), "a%20b%2Fc%2Bd");
        assert_eq!(encode("abc-_.~"), "abc-_.~");
    }
}
