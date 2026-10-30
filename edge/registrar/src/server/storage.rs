//! The backup door: a buyer's restic repository on the storage it bought.
//!
//! A paid storage order is a PersistentVolumeClaim in the mesh, and Longhorn
//! keeps its replicas on other people's boxes. Whatever the buyer keeps there
//! must be unreadable to those boxes, to this edge, and to anyone who takes
//! one of them away, so the edge offers the volume in one shape only: a
//! restic REST repository. restic on the buyer's box encrypts every chunk
//! with the box's recovery code before it leaves (`backend/src/backup.rs` on
//! the box), and names each file by the hash of its ciphertext. The replicas,
//! the edge and the network between them only ever carry that ciphertext,
//! and the key is never anywhere but on the box and in the owner's memory.
//!
//! On the first request for an order the edge starts restic's `rest-server`
//! in the buyer's `market-<id>` namespace with the claim mounted, pinned to
//! the edge's own node (the one node every edge has, and the one that holds
//! the image), and a Service in front. Every request after that is
//! forwarded to the Service, signed in as the edge with a password made for
//! that order alone (`Door::upstream`), because every mesh node can reach a
//! Service's cluster address and none of them should reach the repository.
//!
//! # The door's key
//!
//! A request carries HTTP Basic credentials: the order id and a secret the
//! box derives from its recovery code and the order id
//! (`backend/src/backup.rs`, `door_secret`). The edge keeps only the
//! secret's SHA-256 (`Door::key`), set by the buyer through
//! `POST /market/storage-door` with its appliance token. Not the token
//! itself, on purpose: the day the backup is needed the box that bought the
//! storage may be gone, and a new box knows the recovery code but holds no
//! token. The recovery code then opens the door and decrypts what is behind
//! it, the same one secret for both, as with a bucket.
//!
//! Like the machine image transfers, this runs outside the guard: a pack is
//! megabytes, and a backup sends many. Bodies are buffered whole, up to
//! [`MAX_BODY`] (restic writes 16 MiB packs unless told otherwise), and at
//! most [`MAX_REQUESTS`] are in flight. restic retries a `503` with backoff,
//! so a busy edge or a server still starting costs the box time, not a run.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::extract::{Path as UrlPath, RawQuery, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::any;
use axum::Router;
use base64::Engine;
use tokio::sync::{Mutex, Semaphore};

use axum::Json;
use serde::Deserialize;

use super::{kube_access, kube_create, market_of, market_tenant, AppState, KubeAccess, MarketAuth};
use crate::action::Action;
use crate::error::ApiError;
use crate::market::{MarketError, OpenDoor, StorageVolume};

/// Requests forwarded at once, across every box.
pub(super) const MAX_REQUESTS: usize = 8;

/// The largest body taken or handed back. restic's packs are 16 MiB by
/// default and at most 128 MiB; a box never asks for more than the default.
pub(super) const MAX_BODY: usize = 64 << 20;

/// The port `rest-server` listens on inside the pod and the Service.
const PORT: u16 = 8000;

/// How long one forwarded request may take, a whole pack included.
const UPSTREAM_TIMEOUT: Duration = Duration::from_secs(120);

/// How long the server's address may take to accept. It is on this node,
/// so anything slower is a pod that is not there.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);

/// What a box is told while its server starts. restic waits and asks again.
const RETRY_AFTER_SECS: &str = "5";

/// The door's own state, kept in [`AppState`].
pub(super) struct Door {
    requests: Semaphore,
    /// Each order's Service address, so a backup's thousands of requests do
    /// not each ask the apiserver. Dropped when the address stops answering.
    upstream: Mutex<HashMap<String, String>>,
    client: reqwest::Client,
}

impl Door {
    pub(super) fn new() -> reqwest::Result<Self> {
        Ok(Self {
            requests: Semaphore::new(MAX_REQUESTS),
            upstream: Mutex::new(HashMap::new()),
            client: reqwest::Client::builder()
                .timeout(UPSTREAM_TIMEOUT)
                .connect_timeout(CONNECT_TIMEOUT)
                .no_proxy()
                .build()?,
        })
    }
}

pub(super) fn router(state: AppState) -> Router {
    Router::new()
        .route("/market/storage/{order}/", any(root))
        .route("/market/storage/{order}/{*rest}", any(nested))
        .with_state(state)
}

async fn root(
    State(st): State<AppState>,
    UrlPath(order): UrlPath<String>,
    method: Method,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
    body: Body,
) -> Response {
    serve(&st, &order, "", method, query, &headers, body).await
}

async fn nested(
    State(st): State<AppState>,
    UrlPath((order, rest)): UrlPath<(String, String)>,
    method: Method,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
    body: Body,
) -> Response {
    serve(&st, &order, &rest, method, query, &headers, body).await
}

fn refuse(status: StatusCode, why: &'static str) -> Response {
    (status, why).into_response()
}

fn unauthorised() -> Response {
    let mut r = refuse(
        StatusCode::UNAUTHORIZED,
        "sign in with the order and its door key",
    );
    r.headers_mut().insert(
        header::WWW_AUTHENTICATE,
        HeaderValue::from_static("Basic realm=\"losos\""),
    );
    r
}

fn busy(why: &'static str) -> Response {
    let mut r = refuse(StatusCode::SERVICE_UNAVAILABLE, why);
    r.headers_mut().insert(
        header::RETRY_AFTER,
        HeaderValue::from_static(RETRY_AFTER_SECS),
    );
    r
}

/// `ord_` and lowercase hex, the shape the market mints.
pub(super) fn order_ok(id: &str) -> bool {
    id.strip_prefix("ord_").is_some_and(|hex| {
        !hex.is_empty()
            && hex.len() <= 48
            && hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

/// A path inside a restic repository: `config`, or a folder (`data`,
/// `keys`, `locks`, `snapshots`, `index`) and a file in it, the data folder
/// with one level of two-hex subfolders. Anything else is not restic, and
/// never reaches the server.
pub(super) fn repo_path_ok(path: &str) -> bool {
    let parts: Vec<&str> = path.trim_end_matches('/').split('/').collect();
    let hex = |s: &str| !s.is_empty() && s.len() <= 64 && s.bytes().all(|b| b.is_ascii_hexdigit());
    match parts.as_slice() {
        [""] | ["config"] => true,
        ["data" | "keys" | "locks" | "snapshots" | "index"] => true,
        ["keys" | "locks" | "snapshots" | "index", file] => hex(file),
        ["data", file] => hex(file),
        ["data", sub, file] => sub.len() == 2 && hex(sub) && hex(file),
        _ => false,
    }
}

/// The user name and password of an `Authorization: Basic` header.
pub(super) fn basic_auth(headers: &HeaderMap) -> Option<(String, String)> {
    let raw = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let encoded = raw.strip_prefix("Basic ")?;
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded.trim())
        .ok()?;
    let text = String::from_utf8(decoded).ok()?;
    let (id, token) = text.split_once(':')?;
    Some((id.to_string(), token.to_string()))
}

/// The name of an order's server and Service: `rest-ord-<hex>`, a DNS label
/// that starts with a letter, 33 characters for the ids the market makes and
/// within 63 for any [`order_ok`] passes.
pub(super) fn server_name(pvc: &str) -> String {
    format!("rest-{pvc}")
}

#[allow(clippy::too_many_arguments)]
async fn serve(
    st: &AppState,
    order: &str,
    rest: &str,
    method: Method,
    query: Option<String>,
    headers: &HeaderMap,
    body: Body,
) -> Response {
    let market = match market_of(st).await {
        Ok(m) => Arc::clone(m),
        Err(e) => return e.into_response(),
    };
    let Some(image) = market.storage_image().map(str::to_string) else {
        return refuse(
            StatusCode::NOT_FOUND,
            "this edge keeps no backups on bought storage",
        );
    };
    // One answer for a malformed order, an unknown one, a wrong key and no
    // key, so the door says nothing about which orders exist.
    let Some((user, secret)) = basic_auth(headers) else {
        return unauthorised();
    };
    if user != order || !order_ok(order) || secret.is_empty() {
        return unauthorised();
    }
    let Some(OpenDoor {
        volume,
        upstream: password,
    }) = market.door_volume(order, &key_of(&secret)).await
    else {
        tracing::warn!(target: Action::Market.target(), "storage {order}: wrong door key");
        return unauthorised();
    };
    if !repo_path_ok(rest) {
        return refuse(StatusCode::NOT_FOUND, "not a restic repository path");
    }
    let Ok(_permit) = st.storage.requests.try_acquire() else {
        return busy("busy with other backups; retry later");
    };

    let address = match upstream(st, order, &volume, &image, &password).await {
        Ok(Some(a)) => a,
        Ok(None) => return busy("the storage is starting; retry in a few seconds"),
        Err(e) => return e.into_response(),
    };

    let bytes = match axum::body::to_bytes(body, MAX_BODY).await {
        Ok(b) => b,
        Err(_) => return refuse(StatusCode::PAYLOAD_TOO_LARGE, "a pack larger than 64 MiB"),
    };
    let mut url = format!("http://{address}/{rest}");
    if let Some(q) = query {
        url.push('?');
        url.push_str(&q);
    }
    let head = method == Method::HEAD;
    let mut request = st
        .storage
        .client
        .request(method, &url)
        .basic_auth(UPSTREAM_USER, Some(&password))
        .body(bytes);
    for name in [header::CONTENT_TYPE, header::ACCEPT, header::RANGE] {
        if let Some(v) = headers.get(&name) {
            request = request.header(name, v);
        }
    }
    let answer = match request.send().await {
        Ok(a) => a,
        Err(e) => {
            // The pod moved, restarted or is not ready yet: ask the
            // apiserver again next time.
            st.storage.upstream.lock().await.remove(order);
            tracing::warn!(target: Action::Market.target(), "storage {order}: {e}");
            return busy("the storage did not answer; retry in a few seconds");
        }
    };
    let status = answer.status();
    let out = answer_headers(head, answer.headers());
    if answer.content_length().is_some_and(|n| n > MAX_BODY as u64) {
        return refuse(StatusCode::BAD_GATEWAY, "the storage answered too much");
    }
    match answer.bytes().await {
        Ok(b) => (status, out, b).into_response(),
        Err(e) => {
            tracing::warn!(target: Action::Market.target(), "storage {order}: {e}");
            busy("the storage stopped answering; retry in a few seconds")
        }
    }
}

/// The headers of the server's answer the door passes on. A HEAD answer
/// keeps the server's Content-Length, which is how restic learns a file's
/// size: without it the empty body reads as a zero-byte `config` and restic
/// calls the repository invalid.
fn answer_headers(head: bool, from: &HeaderMap) -> HeaderMap {
    let mut out = HeaderMap::new();
    let length = head.then_some(header::CONTENT_LENGTH);
    for name in [
        header::CONTENT_TYPE,
        header::CONTENT_RANGE,
        header::ACCEPT_RANGES,
    ]
    .into_iter()
    .chain(length)
    {
        if let Some(v) = from.get(&name) {
            out.insert(name, v.clone());
        }
    }
    out
}

/// The door key of a secret: its SHA-256 in lowercase hex.
pub(super) fn key_of(secret: &str) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA256, secret.as_bytes());
    digest.as_ref().iter().map(|b| format!("{b:02x}")).collect()
}

#[derive(Debug, Deserialize)]
pub(super) struct DoorReq {
    #[serde(flatten)]
    auth: MarketAuth,
    order_id: String,
    /// [`key_of`] the box's secret; the secret never leaves the box.
    door: String,
}

/// `POST /market/storage-door`: the buyer sets the key of its bought
/// volume's backup door. 204 set; 404 not the caller's bound storage; 400 a
/// key of the wrong shape.
pub(super) async fn set_door(
    State(st): State<AppState>,
    Json(req): Json<DoorReq>,
) -> Result<StatusCode, ApiError> {
    let market = market_tenant(&st, &req.auth).await?;
    if market.storage_image().is_none() {
        return Err(MarketError::NotFound.into());
    }
    market
        .set_door(&req.order_id, &req.auth.appliance_id, &req.door)
        .await?;
    tracing::info!(
        target: Action::Market.target(),
        "{} set the backup door of {}",
        req.auth.appliance_id,
        req.order_id,
    );
    Ok(StatusCode::NO_CONTENT)
}

/// The address of the order's server, starting it the first time. `None`
/// while it is being started.
async fn upstream(
    st: &AppState,
    order: &str,
    volume: &StorageVolume,
    image: &str,
    password: &str,
) -> Result<Option<String>, ApiError> {
    if let Some(a) = st.storage.upstream.lock().await.get(order) {
        return Ok(Some(a.clone()));
    }
    let kube = kube_access(st).await?;
    let name = server_name(&volume.pvc);
    if let Some(ip) = service_ip(&kube, &volume.namespace, &name).await? {
        let address = format!("{ip}:{PORT}");
        st.storage
            .upstream
            .lock()
            .await
            .insert(order.to_string(), address.clone());
        return Ok(Some(address));
    }
    start_server(&kube, image, volume, password).await?;
    tracing::info!(
        target: Action::Market.target(),
        "started the backup server for {}/{}",
        volume.namespace,
        volume.pvc,
    );
    Ok(None)
}

/// The Service's cluster address, or `None` when there is no Service yet.
async fn service_ip(
    kube: &KubeAccess,
    namespace: &str,
    name: &str,
) -> Result<Option<String>, ApiError> {
    let url = format!("{}/api/v1/namespaces/{namespace}/services/{name}", kube.api);
    let response = kube
        .client
        .get(&url)
        .bearer_auth(&kube.token)
        .send()
        .await
        .map_err(|e| ApiError::KubeApi(format!("GET {url}: {e}")))?;
    match response.status() {
        reqwest::StatusCode::NOT_FOUND => Ok(None),
        s if s.is_success() => {
            let doc: serde_json::Value = response
                .json()
                .await
                .map_err(|e| ApiError::KubeApi(format!("GET {url}: {e}")))?;
            Ok(doc["spec"]["clusterIP"]
                .as_str()
                .filter(|ip| ip.parse::<std::net::IpAddr>().is_ok())
                .map(str::to_string))
        }
        s => Err(ApiError::KubeApi(format!("GET {url} -> {s}"))),
    }
}

/// Create the order's password Secret, its Deployment, then its Service.
/// Each is idempotent: an object that exists is a `409`, which counts as
/// done. The password never changes for an order, so an old Secret is the
/// right one.
async fn start_server(
    kube: &KubeAccess,
    image: &str,
    volume: &StorageVolume,
    password: &str,
) -> Result<(), ApiError> {
    let ns = &volume.namespace;
    kube_create(
        kube,
        &format!("{}/api/v1/namespaces/{ns}/secrets", kube.api),
        &secret(volume, password),
    )
    .await?;
    kube_create(
        kube,
        &format!("{}/apis/apps/v1/namespaces/{ns}/deployments", kube.api),
        &deployment(image, volume),
    )
    .await?;
    kube_create(
        kube,
        &format!("{}/api/v1/namespaces/{ns}/services", kube.api),
        &service(volume),
    )
    .await
}

/// The user the edge signs in to every order's server as.
const UPSTREAM_USER: &str = "edge";

/// The server's htpasswd line for `password`: rest-server's `{SHA}` form,
/// SHA-1 in base64. SHA-1 is enough here: the password is 32 random bytes,
/// so there is nothing to guess.
pub(super) fn htpasswd(password: &str) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY, password.as_bytes());
    format!(
        "{UPSTREAM_USER}:{{SHA}}{}\n",
        base64::engine::general_purpose::STANDARD.encode(digest.as_ref())
    )
}

/// The Secret that holds the server's one account, the edge's.
pub(super) fn secret(volume: &StorageVolume, password: &str) -> serde_json::Value {
    serde_json::json!({
        "apiVersion": "v1",
        "kind": "Secret",
        "metadata": {
            "name": server_name(&volume.pvc),
            "namespace": volume.namespace,
            "labels": {
                "losos.market/managed": "true",
                "losos.market/order": volume.pvc,
            },
        },
        "type": "Opaque",
        "stringData": { "htpasswd": htpasswd(password) },
    })
}

/// `rest-server` with the claim mounted, on the edge's node, as nobody, with
/// no service account, no capabilities and a read-only root. Its one account
/// is the edge's, from the order's Secret: every mesh node can reach a
/// ClusterIP Service, and none of them knows the password.
pub(super) fn deployment(image: &str, volume: &StorageVolume) -> serde_json::Value {
    let name = server_name(&volume.pvc);
    serde_json::json!({
        "apiVersion": "apps/v1",
        "kind": "Deployment",
        "metadata": {
            "name": name,
            "namespace": volume.namespace,
            "labels": {
                "losos.market/managed": "true",
                "losos.market/order": volume.pvc,
            },
        },
        "spec": {
            "replicas": 1,
            // A volume is attached to one node at a time.
            "strategy": { "type": "Recreate" },
            "selector": { "matchLabels": { "losos.market/backup": volume.pvc } },
            "template": {
                "metadata": {
                    "labels": {
                        "losos.market/managed": "true",
                        "losos.market/backup": volume.pvc,
                    },
                },
                "spec": {
                    "automountServiceAccountToken": false,
                    "enableServiceLinks": false,
                    "nodeSelector": { "node-role.kubernetes.io/control-plane": "true" },
                    "tolerations": [
                        {
                            "key": "node-role.kubernetes.io/control-plane",
                            "operator": "Exists",
                            "effect": "NoSchedule",
                        },
                        { "key": "CriticalAddonsOnly", "operator": "Exists" },
                    ],
                    "securityContext": {
                        "runAsNonRoot": true,
                        "runAsUser": 65534,
                        "runAsGroup": 65534,
                        "fsGroup": 65534,
                        "seccompProfile": { "type": "RuntimeDefault" },
                    },
                    "containers": [{
                        "name": "rest-server",
                        "image": image,
                        "imagePullPolicy": "Never",
                        "command": [
                            "/bin/rest-server",
                            "--path", "/data",
                            "--listen", format!(":{PORT}"),
                            "--htpasswd-file", "/etc/rest-server/htpasswd",
                        ],
                        "ports": [{ "containerPort": PORT }],
                        "readinessProbe": { "tcpSocket": { "port": PORT } },
                        "securityContext": {
                            "allowPrivilegeEscalation": false,
                            "readOnlyRootFilesystem": true,
                            "capabilities": { "drop": ["ALL"] },
                        },
                        "volumeMounts": [
                            { "name": "data", "mountPath": "/data" },
                            { "name": "auth", "mountPath": "/etc/rest-server", "readOnly": true },
                        ],
                    }],
                    "volumes": [
                        {
                            "name": "data",
                            "persistentVolumeClaim": { "claimName": volume.pvc },
                        },
                        {
                            "name": "auth",
                            "secret": { "secretName": name, "defaultMode": 0o440 },
                        },
                    ],
                },
            },
        },
    })
}

pub(super) fn service(volume: &StorageVolume) -> serde_json::Value {
    serde_json::json!({
        "apiVersion": "v1",
        "kind": "Service",
        "metadata": {
            "name": server_name(&volume.pvc),
            "namespace": volume.namespace,
            "labels": {
                "losos.market/managed": "true",
                "losos.market/order": volume.pvc,
            },
        },
        "spec": {
            "selector": { "losos.market/backup": volume.pvc },
            "ports": [{ "port": PORT, "targetPort": PORT }],
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_restic_paths_pass() {
        for ok in [
            "",
            "config",
            "keys/",
            "keys/0123abcd",
            "data/",
            "data/ab/ab12cd",
            "data/ab12cd",
            "locks/ff00",
            "snapshots/",
            "index/00",
        ] {
            assert!(repo_path_ok(ok), "{ok}");
        }
        for bad in [
            "../etc/passwd",
            "data/../config",
            "data/a/ab12",
            "data/zz/ab12",
            "keys/not-hex",
            "admin",
            "config/x",
            "data/ab/cd/ef",
        ] {
            assert!(!repo_path_ok(bad), "{bad}");
        }
    }

    #[test]
    fn a_door_key_is_the_secrets_sha256() {
        assert_eq!(
            key_of("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert!(crate::market::door_hash_ok(&key_of("x")));
    }

    #[test]
    fn order_ids_are_the_markets_shape() {
        assert!(order_ok("ord_ab12"));
        assert!(!order_ok("ord_"));
        assert!(!order_ok("ord_AB12"));
        assert!(!order_ok("lst_ab12"));
        assert!(!order_ok("ord_ab/../x"));
    }

    #[test]
    fn basic_auth_reads_id_and_token() {
        let mut h = HeaderMap::new();
        let v = base64::engine::general_purpose::STANDARD.encode("mattbox:tok:en");
        h.insert(
            header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Basic {v}")).unwrap(),
        );
        assert_eq!(
            basic_auth(&h),
            Some(("mattbox".to_string(), "tok:en".to_string()))
        );
        h.insert(header::AUTHORIZATION, HeaderValue::from_static("Bearer x"));
        assert_eq!(basic_auth(&h), None);
    }

    #[test]
    fn a_head_answer_keeps_the_size_of_the_file() {
        let mut from = HeaderMap::new();
        from.insert(header::CONTENT_LENGTH, "155".parse().unwrap());
        from.insert(
            header::CONTENT_TYPE,
            "application/octet-stream".parse().unwrap(),
        );
        from.insert(header::SERVER, "rest-server".parse().unwrap());
        let head = answer_headers(true, &from);
        assert_eq!(head.get(header::CONTENT_LENGTH).unwrap(), "155");
        assert!(head.get(header::SERVER).is_none());
        // A body answer gets its length from the body the door sends.
        let get = answer_headers(false, &from);
        assert!(get.get(header::CONTENT_LENGTH).is_none());
        assert_eq!(
            get.get(header::CONTENT_TYPE).unwrap(),
            "application/octet-stream"
        );
    }

    #[test]
    fn the_password_is_in_rest_servers_sha_form() {
        // One `edge:{SHA}<base64 of the SHA-1>` line. That rest-server takes
        // it is shown by nix/tests/mesh-backup.nix, which runs the server on
        // the line the registrar wrote.
        let password = crate::identity::random_hex(32).unwrap();
        let line = htpasswd(&password);
        let hash = line
            .strip_prefix("edge:{SHA}")
            .and_then(|l| l.strip_suffix('\n'))
            .unwrap();
        let digest = base64::engine::general_purpose::STANDARD
            .decode(hash)
            .unwrap();
        let expected =
            ring::digest::digest(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY, password.as_bytes());
        assert_eq!(digest, expected.as_ref());
    }

    #[test]
    fn the_server_runs_unprivileged_on_the_edge_with_the_claim() {
        let v = StorageVolume {
            namespace: "market-b".to_string(),
            pvc: "ord-ab12".to_string(),
        };
        let d = deployment("losos.local/rest-server:0.13.0", &v);
        let pod = &d["spec"]["template"]["spec"];
        assert_eq!(d["metadata"]["name"], "rest-ord-ab12");
        assert_eq!(pod["automountServiceAccountToken"], false);
        assert_eq!(
            pod["nodeSelector"]["node-role.kubernetes.io/control-plane"],
            "true"
        );
        assert_eq!(pod["securityContext"]["runAsNonRoot"], true);
        assert_eq!(
            pod["volumes"][0]["persistentVolumeClaim"]["claimName"],
            "ord-ab12"
        );
        assert_eq!(pod["volumes"][1]["secret"]["secretName"], "rest-ord-ab12");
        let c = &pod["containers"][0];
        let command: Vec<&str> = c["command"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a.as_str().unwrap())
            .collect();
        assert!(!command.contains(&"--no-auth"), "{command:?}");
        assert!(
            command.contains(&"/etc/rest-server/htpasswd"),
            "{command:?}"
        );
        assert_eq!(c["imagePullPolicy"], "Never");
        assert_eq!(c["securityContext"]["readOnlyRootFilesystem"], true);
        assert_eq!(c["securityContext"]["capabilities"]["drop"][0], "ALL");
        let password = crate::identity::random_hex(32).unwrap();
        let k = secret(&v, &password);
        assert_eq!(k["metadata"]["name"], "rest-ord-ab12");
        assert_eq!(k["stringData"]["htpasswd"], htpasswd(&password));
        let s = service(&v);
        assert_eq!(s["spec"]["selector"]["losos.market/backup"], "ord-ab12");
        assert_eq!(
            d["spec"]["selector"]["matchLabels"]["losos.market/backup"],
            "ord-ab12"
        );
    }
}
