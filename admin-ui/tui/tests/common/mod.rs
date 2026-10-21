//! A box or an edge on loopback that answers the envelope the way lososd and
//! losos-registrar do, with losos-access's own server half, and an `Io` that
//! captures what a command prints. A box also plays LosOS Login's device
//! sign-in for the companion ([`Login`]) and takes its Bearer tokens on the
//! companion's routes, as lososd does.

#![allow(dead_code)]

pub mod fixtures;

use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use losos::commands::{run, Io};
use losos::keychain::{Keychain, MemoryKeychain};
use losos_access::{open, seal, HostKey, Keyring, Replay};
use serde_json::{json, Value};

pub const TOKEN: &str = "losos_pat_0123abcd_00112233445566778899aabbccddeeff00112233";

/// What a host does with a call that opened: status and body.
pub type Handler = dyn Fn(&str, &str, &Value) -> (u16, Value) + Send + Sync;

pub struct Mock {
    pub url: String,
    /// The key it serves and seals with; a test may swap it.
    pub host: Arc<Mutex<HostKey>>,
    /// The seeded keys it accepts; a test may add one.
    pub ring: Arc<Mutex<String>>,
    /// Every call that opened: method, path, body.
    pub calls: Arc<Mutex<Vec<(String, String, Value)>>>,
    /// LosOS Login's side of the companion's sign-in.
    pub login: Arc<Mutex<Login>>,
}

/// What the box's LosOS Login knows of the companion.
#[derive(Default)]
pub struct Login {
    /// The browser says no instead of approving.
    pub deny: bool,
    /// How often the token endpoint was asked for the device code.
    pub polls: u32,
    /// Live access tokens.
    pub access: Vec<String>,
    /// The one refresh token that still counts.
    pub refresh: Option<String>,
    pub refreshes: u32,
    /// Tokens handed back at the revocation endpoint.
    pub revoked: Vec<String>,
    /// Every Bearer token the box was shown, live or not.
    pub shown: Vec<String>,
    issued: u32,
}

impl Login {
    fn issue(&mut self) -> Value {
        self.issued += 1;
        let access = format!("authelia_at_{}.sig", self.issued);
        let refresh = format!("authelia_rt_{}.sig", self.issued);
        self.access.push(access.clone());
        self.refresh = Some(refresh.clone());
        json!({ "access_token": access, "refresh_token": refresh,
            "expires_in": 3599, "token_type": "bearer", "scope": "openid profile offline_access" })
    }

    /// The box stops taking every access token it handed out, as an hour
    /// going by does.
    pub fn expire(&mut self) {
        self.access.clear();
    }

    /// The token endpoint, the device authorization and the revocation.
    fn answer(&mut self, url: &str, path: &str, form: &HashMap<String, String>) -> (u16, Value) {
        let oauth = |error: &str| (400, json!({ "error": error }));
        if form.get("client_id").map(String::as_str) != Some(losos_access::COMPANION_CLIENT_ID) {
            return (401, json!({ "error": "invalid_client" }));
        }
        match path {
            "/auth/api/oidc/device-authorization" => {
                let page = format!("{url}/auth/consent/openid/device-authorization");
                (
                    200,
                    json!({ "device_code": "authelia_dc_1", "user_code": "WDJBMJHT",
                    "verification_uri": page,
                    "verification_uri_complete": format!("{page}?user_code=WDJBMJHT"),
                    "expires_in": 600, "interval": 0 }),
                )
            }
            "/auth/api/oidc/token" => match form.get("grant_type").map(String::as_str) {
                Some("urn:ietf:params:oauth:grant-type:device_code") => {
                    self.polls += 1;
                    if form.get("device_code").map(String::as_str) != Some("authelia_dc_1") {
                        oauth("invalid_grant")
                    } else if self.polls == 1 {
                        oauth("authorization_pending")
                    } else if self.deny {
                        oauth("access_denied")
                    } else {
                        (200, self.issue())
                    }
                }
                Some("refresh_token") => {
                    // A refresh token counts once; showing an old one ends
                    // the whole sign-in, as LosOS Login does.
                    if form.get("refresh_token") == self.refresh.as_ref() {
                        self.refreshes += 1;
                        // The access token it replaces stops counting too.
                        self.access.clear();
                        (200, self.issue())
                    } else {
                        self.access.clear();
                        self.refresh = None;
                        oauth("invalid_grant")
                    }
                }
                _ => oauth("unsupported_grant_type"),
            },
            "/auth/api/oidc/revocation" => {
                let token = form.get("token").cloned().unwrap_or_default();
                self.access.retain(|t| *t != token);
                if self.refresh.as_ref() == Some(&token) {
                    self.refresh = None;
                }
                self.revoked.push(token);
                (200, json!({}))
            }
            _ => (404, json!({ "error": "not here" })),
        }
    }
}

impl Mock {
    pub fn fingerprint(&self) -> String {
        self.host.lock().unwrap().fingerprint().to_string()
    }

    pub fn seed(&self, armored_public: &str) {
        let mut ring = self.ring.lock().unwrap();
        ring.push('\n');
        ring.push_str(armored_public);
    }
}

/// A box (`edge` false) or an edge, accepting `seeded` keys and `TOKEN`.
pub fn mock(edge: bool, seeded: &str, handler: Box<Handler>) -> Mock {
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    serve(server, "http", edge, seeded, handler)
}

/// The same over HTTPS with `cert`, as a box serves its own self-signed
/// certificate.
pub fn mock_tls(edge: bool, seeded: &str, handler: Box<Handler>, cert: &Cert) -> Mock {
    let server = tiny_http::Server::https("127.0.0.1:0", cert.config()).unwrap();
    serve(server, "https", edge, seeded, handler)
}

/// A self-signed certificate for 127.0.0.1.
pub struct Cert {
    pub pem: String,
    pub key: String,
    pub der: Vec<u8>,
    /// The key pair's subjectPublicKeyInfo, DER.
    pub spki: Vec<u8>,
}

impl Cert {
    pub fn new() -> Cert {
        let made = rcgen::generate_simple_self_signed(vec!["127.0.0.1".into(), "localhost".into()])
            .unwrap();
        Cert {
            pem: made.cert.pem(),
            key: made.key_pair.serialize_pem(),
            der: made.cert.der().to_vec(),
            spki: made.key_pair.public_key_der(),
        }
    }

    pub fn config(&self) -> tiny_http::SslConfig {
        tiny_http::SslConfig {
            certificate: self.pem.clone().into_bytes(),
            private_key: self.key.clone().into_bytes(),
        }
    }

    /// What the client pins: the SHA-256 of the DER, hex.
    pub fn pin(&self) -> String {
        use sha2::Digest;
        hex::encode(sha2::Sha256::digest(&self.der))
    }
}

fn serve(
    server: tiny_http::Server,
    scheme: &str,
    edge: bool,
    seeded: &str,
    handler: Box<Handler>,
) -> Mock {
    let url = format!("{scheme}://{}", server.server_addr().to_ip().unwrap());
    let host = Arc::new(Mutex::new(
        HostKey::generate(if edge {
            "LosOS edge <edge>"
        } else {
            "LosOS box <box>"
        })
        .unwrap(),
    ));
    let ring = Arc::new(Mutex::new(seeded.to_string()));
    let calls = Arc::new(Mutex::new(Vec::new()));
    let login = Arc::new(Mutex::new(Login::default()));
    let (key_path, call_path) = if edge {
        ("/operator/host-key", "/operator/call")
    } else {
        ("/api/access/host-key", "/api/access/call")
    };
    {
        let (host, ring, calls, login) = (host.clone(), ring.clone(), calls.clone(), login.clone());
        let url = url.clone();
        std::thread::spawn(move || {
            let replay = Replay::default();
            for mut request in server.incoming_requests() {
                let path = request.url().to_string();
                let mut body = Vec::new();
                let _ = request.as_reader().read_to_end(&mut body);
                // What a box's front door answers outside the API: LosOS
                // cloud's WebDAV asks for credentials, LosOS Git a page.
                if !edge && path.starts_with("/nextcloud/") {
                    let challenge = tiny_http::Header::from_bytes(
                        "WWW-Authenticate",
                        r#"Basic realm="LosOS cloud", charset="UTF-8""#,
                    )
                    .unwrap();
                    let _ = request.respond(
                        tiny_http::Response::from_string("")
                            .with_status_code(401)
                            .with_header(challenge),
                    );
                    continue;
                }
                let bearer = request
                    .headers()
                    .iter()
                    .find(|h| h.field.equiv("Authorization"))
                    .and_then(|h| h.value.as_str().strip_prefix("Bearer ").map(str::to_string));
                let method = request.method().as_str().to_string();
                let (status, content_type, text) = if !edge && path.starts_with("/auth/api/oidc/") {
                    let form: HashMap<String, String> =
                        url::form_urlencoded::parse(&body).into_owned().collect();
                    let (status, answer) = login.lock().unwrap().answer(&url, &path, &form);
                    (status, "application/json", answer.to_string())
                } else if let Some(token) = bearer.filter(|_| !edge) {
                    // lososd: a live companion token, on the companion's
                    // routes only.
                    let mut login = login.lock().unwrap();
                    login.shown.push(token.clone());
                    if !login.access.contains(&token) {
                        (
                            401,
                            "application/json",
                            json!({ "error": "sign in" }).to_string(),
                        )
                    } else if !losos_access::companion_route(&method, &path) {
                        (403, "application/json",
                            json!({ "error": "the LosOS companion reads files, repositories, mail and the box's health, nothing else" }).to_string())
                    } else {
                        calls
                            .lock()
                            .unwrap()
                            .push((method.clone(), path.clone(), Value::Null));
                        let (status, answer) = handler(&method, &path, &Value::Null);
                        (status, "application/json", answer.to_string())
                    }
                } else if path == key_path {
                    let host = host.lock().unwrap();
                    let doc = json!({ "fingerprint": host.fingerprint(), "publicKey": host.armored_public() });
                    (200, "application/json", doc.to_string())
                } else if path == call_path {
                    let host = host.lock().unwrap();
                    let ring = Keyring::parse(&ring.lock().unwrap()).unwrap();
                    match open(&host, &ring, &body, &replay, losos_access::now_secs()) {
                        Err(refusal) => (
                            refusal.status(),
                            "application/json",
                            json!({ "error": refusal.to_string() }).to_string(),
                        ),
                        Ok(opened) if opened.request.token != TOKEN => (
                            403,
                            "application/json",
                            json!({ "error": "the token is not known" }).to_string(),
                        ),
                        Ok(opened) => {
                            let r = &opened.request;
                            calls.lock().unwrap().push((
                                r.method.clone(),
                                r.path.clone(),
                                r.body.clone(),
                            ));
                            let (status, answer) = handler(&r.method, &r.path, &r.body);
                            let sealed =
                                seal(&host, &opened.signer, &r.nonce, status, answer).unwrap();
                            (200, "application/pgp-encrypted", sealed)
                        }
                    }
                } else {
                    (
                        404,
                        "application/json",
                        json!({ "error": "not here" }).to_string(),
                    )
                };
                let header = tiny_http::Header::from_bytes("Content-Type", content_type).unwrap();
                let _ = request.respond(
                    tiny_http::Response::from_string(text)
                        .with_status_code(status)
                        .with_header(header),
                );
            }
        });
    }
    Mock {
        url,
        host,
        ring,
        calls,
        login,
    }
}

/// A shared buffer a command writes into.
#[derive(Clone, Default)]
pub struct Captured(pub Arc<Mutex<Vec<u8>>>);

impl Write for Captured {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Captured {
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap()).into_owned()
    }
}

/// One run of the client, as a script would make it: no terminal.
pub struct Run {
    pub code: u8,
    pub out: String,
    pub err: String,
}

pub struct Client {
    pub keychain: Arc<dyn Keychain + Send + Sync>,
    pub env: HashMap<String, String>,
}

pub fn fake_bin(name: &str) -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fake-bin")
        .join(name)
        .display()
        .to_string()
}

impl Client {
    pub fn new() -> Client {
        Client::with(Arc::new(MemoryKeychain::new()))
    }

    pub fn with(keychain: Arc<dyn Keychain + Send + Sync>) -> Client {
        let mut env = HashMap::new();
        env.insert("PATH".into(), std::env::var("PATH").unwrap_or_default());
        env.insert("LANG".into(), "en_US.UTF-8".into());
        Client { keychain, env }
    }

    pub fn set(&mut self, name: &str, value: &str) -> &mut Client {
        self.env.insert(name.into(), value.into());
        self
    }

    pub fn run(&self, args: &[&str], stdin: &str) -> Run {
        let out = Captured::default();
        let err = Captured::default();
        let mut io = Io {
            out: Box::new(out.clone()),
            err: Box::new(err.clone()),
            stdin: Box::new(std::io::Cursor::new(stdin.as_bytes().to_vec())),
            env: self.env.clone(),
            keychain: self.keychain.clone(),
            stdin_tty: false,
            stdout_tty: false,
            stderr_tty: false,
        };
        let argv: Vec<String> = std::iter::once("losos")
            .chain(args.iter().copied())
            .map(str::to_string)
            .collect();
        let code = run(&argv, &mut io);
        Run {
            code,
            out: out.text(),
            err: err.text(),
        }
    }
}

/// The owner's key, as `gpg --export-secret-keys --armor` would give it.
pub struct Owner {
    pub key: HostKey,
    pub file: tempfile::NamedTempFile,
}

pub fn owner() -> Owner {
    let key = HostKey::generate("Owner <owner@example.org>").unwrap();
    let mut file = tempfile::NamedTempFile::new().unwrap();
    file.write_all(key.armored_secret().unwrap().as_bytes())
        .unwrap();
    Owner { key, file }
}
