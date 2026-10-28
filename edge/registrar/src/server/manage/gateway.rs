//! The gateway's own settings on the manage page.
//!
//! An edge gateway has no shell: no console login and no SSH. Everything
//! its owner does to it happens on this page, signed in with the manage key
//! the gateway shows on its screen:
//!
//! ```text
//! GET    /manage/api/gateway              name, passport, uplink, enrolled boxes
//! PUT    /manage/api/gateway/uplink       relay the boxes here through a hub
//! DELETE /manage/api/gateway/uplink       stop relaying
//! DELETE /manage/api/gateway/boxes/{id}   forget a box that left; it enrols
//!                                         again on its next contact
//! PUT    /manage/api/gateway/cluster      start a secondary cluster, or join
//!                                         the one another gateway started
//! DELETE /manage/api/gateway/cluster      stand alone again
//! ```
//!
//! The owner's passport is claimed through `POST /passport/settings`, which
//! needs no key because the owner's signature is the proof.
//!
//! The uplink is the file `--uplink-file` names, with the hub's tokens in
//! 0600 files beside it. The registrar re-reads it on every uplink pass; the
//! NixOS module restarts the tunnel when the directory changes
//! (`modules/edge-gateway.nix`).

use std::path::{Path, PathBuf};

use axum::extract::{Path as UrlPath, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, put};
use axum::{Json, Router};
use serde::Deserialize;

use super::{off, plain, unauthorized, AppState};
use crate::action::Action;
use crate::fsutil;
use crate::relay::{dns_label, UplinkFile};
use crate::secondary::{plain_url, ClusterFile};

const TOKEN_FILE: &str = "uplink.token";
const PIN_FILE: &str = "hub-tunnel.pin";
const MAX_TOKEN: usize = 512;
const MAX_URL: usize = 255;

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/manage/api/gateway", get(show))
        .route(
            "/manage/api/gateway/uplink",
            put(set_uplink).delete(clear_uplink),
        )
        .route("/manage/api/gateway/boxes/{id}", delete(forget))
        .route(
            "/manage/api/gateway/cluster",
            put(set_cluster).delete(leave_cluster),
        )
}

/// The body of `PUT /manage/api/gateway/uplink`: the hub's tenant row its
/// operator handed over, with the tokens themselves rather than files.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UplinkForm {
    pub id: String,
    pub token: String,
    pub registrar_url: String,
    pub tunnel_endpoint: String,
}

impl UplinkForm {
    /// What is wrong with the form, in words the page shows.
    pub(crate) fn problem(&self) -> Option<&'static str> {
        if !dns_label(&self.id) {
            return Some("the id must be a DNS label, as the hub's operator gave it");
        }
        let url = self.registrar_url.trim();
        if url.len() > MAX_URL
            || !(url.starts_with("https://") || url.starts_with("http://"))
            || url.chars().any(|c| c.is_whitespace() || c.is_control())
        {
            return Some("the registrar must be an http(s) URL");
        }
        if !host_port(self.tunnel_endpoint.trim()) {
            return Some("the tunnel endpoint must be host:port");
        }
        if !token_shaped(&self.token) {
            return Some("the token must be 1 to 512 characters with no spaces");
        }
        None
    }
}

fn token_shaped(t: &str) -> bool {
    let t = t.trim();
    !t.is_empty() && t.len() <= MAX_TOKEN && t.chars().all(|c| c.is_ascii_graphic())
}

fn host_port(s: &str) -> bool {
    let Some((host, port)) = s.rsplit_once(':') else {
        return false;
    };
    let host = host.trim_start_matches('[').trim_end_matches(']');
    !host.is_empty()
        && host.len() <= MAX_URL
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':'))
        && port.parse::<u16>().is_ok_and(|p| p != 0)
}

/// The directory the uplink file and its tokens live in.
fn uplink_dir(file: &str) -> PathBuf {
    Path::new(file)
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}

/// The refusal for a request without the manage key, or `None` to go on.
fn refused(st: &AppState, headers: &HeaderMap) -> Option<Response> {
    match &st.manage {
        None => Some(off()),
        Some(m) if !m.signed_in(headers) => Some(unauthorized()),
        Some(_) => None,
    }
}

async fn show(State(st): State<AppState>, headers: HeaderMap) -> Response {
    if let Some(r) = refused(&st, &headers) {
        return r;
    }
    let name = tokio::fs::read_to_string("/proc/sys/kernel/hostname")
        .await
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    let passport = st.gateway.as_ref().map(|gw| {
        serde_json::json!({
            "edgeKey": gw.public_key(),
            "claimed": gw.measured().is_some(),
            "owner": gw.owner().map(|o| serde_json::json!({
                "fingerprint": o.fingerprint,
                "userId": o.user_id,
            })),
        })
    });
    let uplink = match &st.opts.uplink {
        None => serde_json::Value::Null,
        Some(u) => {
            let file = tokio::fs::read(&u.file)
                .await
                .ok()
                .and_then(|b| serde_json::from_slice::<UplinkFile>(&b).ok());
            let relaying = tokio::fs::read(&u.tunnel_config)
                .await
                .ok()
                .and_then(|b| serde_json::from_slice::<crate::tunnel::ClientFile>(&b).ok())
                .map_or(0, |f| f.services.len());
            serde_json::json!({
                "set": file.as_ref().map(|f| serde_json::json!({
                    "id": f.id,
                    "registrarUrl": f.registrar_url,
                    "tunnelEndpoint": f.tunnel_endpoint,
                })),
                "relaying": relaying,
            })
        }
    };
    let enrolled = match &st.enrolment {
        None => Vec::new(),
        Some(e) => e
            .list()
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|(id, _)| id)
            .collect(),
    };
    // The secondary cluster, secret included: this page is the only place
    // the owner reads it to join another gateway.
    let cluster = match &st.secondary {
        None => serde_json::Value::Null,
        Some(sec) => {
            let file = crate::server::read_cluster_file(&sec.opts.file).await;
            serde_json::json!({
                "joined": file.as_ref().map(|f| {
                    let mut gateways = vec![serde_json::json!({
                        "member": f.member,
                        "url": sec.opts.self_url,
                    })];
                    gateways.extend(
                        sec.peers
                            .alive(sec.opts.interval * 3, std::time::Instant::now())
                            .into_iter()
                            .map(|g| serde_json::json!({ "member": g.member, "url": g.url })),
                    );
                    serde_json::json!({
                        "name": f.name,
                        "member": f.member,
                        "secret": f.secret,
                        "url": sec.opts.self_url,
                        "gateways": gateways,
                    })
                }),
            })
        }
    };
    // The main cluster as the hub last answered: the gateways on primary
    // links, this one among them once the uplink is set.
    let main =
        tokio::fs::read(Path::new(&st.opts.registry_path).with_file_name("main-cluster.json"))
            .await
            .ok()
            .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
            .unwrap_or_else(|| serde_json::json!([]));
    Json(serde_json::json!({
        "name": name,
        "passport": passport,
        "uplink": uplink,
        "enrolled": enrolled,
        "cluster": cluster,
        "main": main,
    }))
    .into_response()
}

/// The body of `PUT /manage/api/gateway/cluster`: a name alone starts a
/// cluster with this gateway in it; a name, the founder's secret and the URL
/// of a gateway already in it join that cluster.
#[derive(Debug, Deserialize)]
pub(crate) struct ClusterForm {
    pub name: String,
    #[serde(default)]
    pub secret: Option<String>,
    #[serde(default)]
    pub peer: Option<String>,
}

impl ClusterForm {
    /// The file to write for `member`, or what is wrong with the form.
    pub(crate) fn file(
        &self,
        member: &str,
        fresh_secret: impl FnOnce() -> String,
    ) -> Result<ClusterFile, &'static str> {
        let name = self.name.trim();
        if !dns_label(name) {
            return Err(
                "the cluster name must be a DNS label: lowercase letters, digits and inner hyphens",
            );
        }
        let secret = self
            .secret
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let peer = self
            .peer
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let file = match (secret, peer) {
            (None, None) => ClusterFile {
                name: name.to_string(),
                member: member.to_string(),
                secret: fresh_secret(),
                peers: Vec::new(),
            },
            (Some(secret), Some(peer)) => {
                if !plain_url(peer) {
                    return Err("the other gateway's address must be an http(s) URL");
                }
                ClusterFile {
                    name: name.to_string(),
                    member: member.to_string(),
                    secret: secret.to_string(),
                    peers: vec![peer.trim_end_matches('/').to_string()],
                }
            }
            _ => return Err("joining needs both the secret and the other gateway's address"),
        };
        match file.fault() {
            Some("the secret is not at least 32 hex characters") => {
                Err("the secret is the hex line the first gateway's page shows")
            }
            Some(fault) => Err(fault),
            None => Ok(file),
        }
    }
}

/// This gateway's name in a cluster: its host name, as a DNS label.
fn member_name(host: &str) -> String {
    let m = host.trim().to_ascii_lowercase();
    if dns_label(&m) {
        m
    } else {
        "gateway".to_string()
    }
}

fn fresh_secret() -> String {
    use ring::rand::SecureRandom;
    let mut raw = [0u8; 32];
    ring::rand::SystemRandom::new()
        .fill(&mut raw)
        .expect("the system's random source");
    crate::identity::to_hex(&raw)
}

async fn set_cluster(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(form): Json<ClusterForm>,
) -> Response {
    if let Some(r) = refused(&st, &headers) {
        return r;
    }
    let Some(sec) = &st.secondary else {
        return plain(
            StatusCode::NOT_FOUND,
            "this gateway runs no secondary cluster",
        );
    };
    let creating = form.secret.as_deref().is_none_or(|s| s.trim().is_empty());
    if creating {
        if let Some(current) = crate::server::read_cluster_file(&sec.opts.file).await {
            return plain(
                StatusCode::CONFLICT,
                &format!(
                    "this gateway is in the cluster {} already: leave it first",
                    current.name
                ),
            );
        }
    }
    let host = tokio::fs::read_to_string("/proc/sys/kernel/hostname")
        .await
        .unwrap_or_default();
    let file = match form.file(&member_name(&host), fresh_secret) {
        Ok(f) => f,
        Err(problem) => return plain(StatusCode::BAD_REQUEST, problem),
    };
    let bytes = match serde_json::to_vec_pretty(&file) {
        Ok(b) => b,
        Err(e) => return plain(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    };
    if let Some(dir) = Path::new(&sec.opts.file).parent() {
        if let Err(e) = tokio::fs::create_dir_all(dir).await {
            tracing::error!(target: Action::Serve.target(), "the cluster directory: {e}");
        }
    }
    match fsutil::atomic_write(Path::new(&sec.opts.file), &bytes, 0o600).await {
        Ok(()) => {
            tracing::info!(
                target: Action::Serve.target(),
                "{} cluster {} as {} from the manage page",
                if creating { "started" } else { "joined" },
                file.name,
                file.member,
            );
            Json(serde_json::json!({
                "name": file.name,
                "member": file.member,
                "secret": file.secret,
                "url": sec.opts.self_url,
            }))
            .into_response()
        }
        Err(e) => {
            tracing::error!(target: Action::Serve.target(), "writing the cluster file: {e}");
            plain(
                StatusCode::INSUFFICIENT_STORAGE,
                "the cluster could not be saved",
            )
        }
    }
}

async fn leave_cluster(State(st): State<AppState>, headers: HeaderMap) -> Response {
    if let Some(r) = refused(&st, &headers) {
        return r;
    }
    let Some(sec) = &st.secondary else {
        return plain(
            StatusCode::NOT_FOUND,
            "this gateway runs no secondary cluster",
        );
    };
    match remove(Path::new(&sec.opts.file)).await {
        Ok(()) => {
            sec.peers.clear();
            tracing::info!(target: Action::Serve.target(), "left the cluster from the manage page");
            Json(serde_json::json!({ "joined": null })).into_response()
        }
        Err(e) => {
            tracing::error!(target: Action::Serve.target(), "removing the cluster file: {e}");
            plain(
                StatusCode::INSUFFICIENT_STORAGE,
                "the cluster could not be left",
            )
        }
    }
}

async fn set_uplink(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(form): Json<UplinkForm>,
) -> Response {
    if let Some(r) = refused(&st, &headers) {
        return r;
    }
    let Some(u) = &st.opts.uplink else {
        return plain(StatusCode::NOT_FOUND, "this edge relays to no hub");
    };
    if let Some(problem) = form.problem() {
        return plain(StatusCode::BAD_REQUEST, problem);
    }
    match write_uplink(&u.file, &form).await {
        Ok(()) => {
            tracing::info!(
                target: Action::Serve.target(),
                "uplink set from the manage page: {} via {}",
                form.id,
                form.registrar_url.trim()
            );
            Json(serde_json::json!({ "set": true })).into_response()
        }
        Err(e) => {
            tracing::error!(target: Action::Serve.target(), "writing the uplink: {e}");
            plain(
                StatusCode::INSUFFICIENT_STORAGE,
                "the uplink could not be saved",
            )
        }
    }
}

/// Write the tokens first and the uplink file last, so the registrar never
/// reads a file that names a token not yet there. A pinned key for the old
/// hub must not be kept for a new one.
pub(crate) async fn write_uplink(file: &str, form: &UplinkForm) -> std::io::Result<()> {
    let dir = uplink_dir(file);
    tokio::fs::create_dir_all(&dir).await?;
    let token = dir.join(TOKEN_FILE);
    fsutil::atomic_write(&token, form.token.trim().as_bytes(), 0o600).await?;
    // A new hub is pinned afresh on the next uplink pass.
    remove(&dir.join(PIN_FILE)).await?;
    let uplink = UplinkFile {
        registrar_url: form.registrar_url.trim().trim_end_matches('/').to_string(),
        tunnel_endpoint: form.tunnel_endpoint.trim().to_string(),
        id: form.id.clone(),
        token_file: token.to_string_lossy().into_owned(),
        pin_file: dir.join(PIN_FILE).to_string_lossy().into_owned(),
    };
    let bytes = serde_json::to_vec_pretty(&uplink).map_err(std::io::Error::other)?;
    fsutil::atomic_write(Path::new(file), &bytes, 0o600).await
}

async fn remove(path: &Path) -> std::io::Result<()> {
    match tokio::fs::remove_file(path).await {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

async fn clear_uplink(State(st): State<AppState>, headers: HeaderMap) -> Response {
    if let Some(r) = refused(&st, &headers) {
        return r;
    }
    let Some(u) = &st.opts.uplink else {
        return plain(StatusCode::NOT_FOUND, "this edge relays to no hub");
    };
    let dir = uplink_dir(&u.file);
    // The rendered tunnel config goes with it, so the tunnel stops instead
    // of dialling the old hub.
    let result = async {
        remove(Path::new(&u.file)).await?;
        for name in [TOKEN_FILE, PIN_FILE] {
            remove(&dir.join(name)).await?;
        }
        remove(Path::new(&u.tunnel_config)).await
    }
    .await;
    match result {
        Ok(()) => {
            tracing::info!(target: Action::Serve.target(), "uplink cleared from the manage page");
            Json(serde_json::json!({ "set": false })).into_response()
        }
        Err(e) => {
            tracing::error!(target: Action::Serve.target(), "clearing the uplink: {e}");
            plain(
                StatusCode::INSUFFICIENT_STORAGE,
                "the uplink could not be cleared",
            )
        }
    }
}

async fn forget(
    State(st): State<AppState>,
    headers: HeaderMap,
    UrlPath(id): UrlPath<String>,
) -> Response {
    if let Some(r) = refused(&st, &headers) {
        return r;
    }
    let Some(enrolment) = &st.enrolment else {
        return plain(StatusCode::NOT_FOUND, "this edge enrols no boxes");
    };
    if !dns_label(&id) {
        return plain(StatusCode::BAD_REQUEST, "not a box id");
    }
    match enrolment.forget(&id).await {
        Ok(true) => {
            tracing::info!(target: Action::Enrol.target(), "forgot {id} from the manage page");
            Json(serde_json::json!({ "forgotten": id })).into_response()
        }
        Ok(false) => plain(StatusCode::NOT_FOUND, "that box is not enrolled here"),
        Err(e) => {
            tracing::error!(target: Action::Enrol.target(), "forgetting {id}: {e}");
            plain(
                StatusCode::INSUFFICIENT_STORAGE,
                "the box could not be forgotten",
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cluster_is_started_with_a_fresh_secret_or_joined_with_the_founders() {
        let start = ClusterForm {
            name: "office".into(),
            secret: None,
            peer: None,
        };
        let f = start.file("gw-a", || "ab".repeat(32)).unwrap();
        assert_eq!((f.name.as_str(), f.member.as_str()), ("office", "gw-a"));
        assert_eq!(f.secret.len(), 64);
        assert!(f.peers.is_empty());

        let join = ClusterForm {
            name: "office".into(),
            secret: Some("ab".repeat(32)),
            peer: Some("http://gw-a.local:8443/".into()),
        };
        let f = join.file("gw-b", || unreachable!()).unwrap();
        assert_eq!(f.peers, ["http://gw-a.local:8443"]);

        for (form, why) in [
            (
                ClusterForm {
                    name: "Office!".into(),
                    secret: None,
                    peer: None,
                },
                "DNS label",
            ),
            (
                ClusterForm {
                    name: "office".into(),
                    secret: Some("ab".repeat(32)),
                    peer: None,
                },
                "both",
            ),
            (
                ClusterForm {
                    name: "office".into(),
                    secret: Some("short".into()),
                    peer: Some("http://gw-a.local:8443".into()),
                },
                "hex line",
            ),
            (
                ClusterForm {
                    name: "office".into(),
                    secret: Some("ab".repeat(32)),
                    peer: Some("javascript:alert(1)".into()),
                },
                "http(s)",
            ),
        ] {
            let err = form.file("gw-b", || "ab".repeat(32)).unwrap_err();
            assert!(err.contains(why), "{err}");
        }
        assert_eq!(member_name("GW-A\n"), "gw-a");
        assert_eq!(member_name("bad_name"), "gateway");
    }

    fn form() -> UplinkForm {
        UplinkForm {
            id: "acme".into(),
            token: "t0ken".into(),
            registrar_url: "https://register.example.test/".into(),
            tunnel_endpoint: "relay.example.test:2333".into(),
        }
    }

    #[test]
    fn the_form_is_checked_before_anything_is_written() {
        assert_eq!(form().problem(), None);
        let bad = |f: fn(&mut UplinkForm)| {
            let mut x = form();
            f(&mut x);
            x.problem().is_some()
        };
        assert!(bad(|f| f.id = "Not A Label".into()));
        assert!(bad(|f| f.registrar_url = "ftp://x".into()));
        assert!(bad(|f| f.registrar_url = "https://x y".into()));
        assert!(bad(|f| f.tunnel_endpoint = "relay".into()));
        assert!(bad(|f| f.tunnel_endpoint = "relay:0".into()));
        assert!(bad(|f| f.tunnel_endpoint = "re lay:1".into()));
        assert!(bad(|f| f.token = "two words".into()));
        assert!(bad(|f| f.token = String::new()));
        assert!(!bad(|f| f.tunnel_endpoint = "[2001:db8::1]:2333".into()));
    }

    #[tokio::test]
    async fn an_uplink_is_written_with_its_tokens_beside_it_and_a_new_hub_drops_the_old_pin() {
        let dir = std::env::temp_dir().join(format!("losos-uplink-form-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let file = dir.join("uplink.json");
        let file_str = file.to_string_lossy().into_owned();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(PIN_FILE), "old hub pin").unwrap();
        write_uplink(&file_str, &form()).await.unwrap();
        let u: UplinkFile = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
        assert_eq!(u.registrar_url, "https://register.example.test");
        assert_eq!(u.id, "acme");
        assert_eq!(std::fs::read_to_string(&u.token_file).unwrap(), "t0ken");
        assert_eq!(u.tunnel_endpoint, "relay.example.test:2333");
        assert_eq!(Path::new(&u.pin_file), dir.join(PIN_FILE));
        assert!(!dir.join(PIN_FILE).exists(), "the old hub's pin was kept");
        use std::os::unix::fs::PermissionsExt;
        for name in ["uplink.json", TOKEN_FILE] {
            let mode = std::fs::metadata(dir.join(name))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600, "{name}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
