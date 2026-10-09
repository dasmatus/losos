//! The virtual machine routes and their fulfilment (`crate::vms`).
//!
//! Two kinds of route live here. The small ones (the catalogue, a buyer's
//! machines, an upload ticket, removing an upload) are ordinary market routes
//! inside the router's body cap, timeout and concurrency guard. The two
//! transfers (a box streaming a buyer's QCOW2 up, CDI's importer streaming it
//! back down) cannot fit a 16 KiB, five-second budget, so [`transfer_router`]
//! is merged outside those layers with its own: at most [`MAX_TRANSFERS`] at
//! once, [`TRANSFER_IDLE`] without a byte and the transfer is dropped, and an
//! upload is held to `--vm-upload-max-gib`. Both are authorised by a
//! capability in the path rather than an appliance token, because neither
//! caller holds one: lososd hands the box's token to the ticket route only,
//! and CDI is only ever told a URL.

use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use axum::body::{Body, Bytes, HttpBody};
use axum::extract::{Path as UrlPath, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, put};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

use super::{
    kube_access, kube_create, market_of, market_tenant, remove_if_exists, write_if_changed,
    AppState, KubeAccess, MarketAuth, TRAEFIK_FILE_MODE,
};
use crate::action::Action;
use crate::error::ApiError;
use crate::market::{namespace_for, Market, MarketError};
use crate::vms::{self, ReplicaStatus, Shape, VmProvision};

/// Transfers in flight at once, uploads and fetches together. A transfer
/// holds a connection for minutes, so this is kept well under the guard's
/// `MAX_INFLIGHT`, which it does not count against.
pub(super) const MAX_TRANSFERS: usize = 4;

/// How long a transfer may go without moving a byte before it is dropped:
/// the slow-loris bound the five-second request budget is everywhere else.
const TRANSFER_IDLE: Duration = Duration::from_secs(60);

/// The size of one read when serving an upload back.
const FETCH_CHUNK: usize = 256 * 1024;

// ── the small routes ─────────────────────────────────────────────────────

/// One image as the page shows it. No source URL: the buyer picks a system,
/// not a mirror.
#[derive(Debug, Serialize)]
pub(super) struct ImageView {
    id: String,
    name: String,
    family: String,
    cloud_init: bool,
    efi: bool,
    disk_gib: u64,
    memory_mib: u64,
}

/// `GET /market/vm-images`.
#[derive(Debug, Serialize)]
pub(super) struct CatalogueView {
    images: Vec<ImageView>,
    installer_only: Vec<String>,
    not_offered: Vec<vms::NotOffered>,
    /// The size of every replica on this edge.
    shape: Shape,
    /// The platform's share of a machine sale, in basis points: half.
    fee_bps: u32,
    max_replicas: u64,
    max_upload_bytes: u64,
    /// Whether a machine gets an `https://<order>.<domain>` address.
    published: bool,
}

/// `GET /market/vm-images`: what a buyer can start, anonymous like the shelf.
/// 503 unless the edge runs machines.
pub(super) async fn catalogue(State(st): State<AppState>) -> Result<Json<CatalogueView>, ApiError> {
    let market = market_of(&st).await?;
    let opts = market.vm_opts().ok_or(MarketError::Unconfigured)?;
    let catalogue = market.vm_catalogue()?;
    Ok(Json(CatalogueView {
        images: catalogue
            .images
            .iter()
            .map(|i| ImageView {
                id: i.id.clone(),
                name: i.name.clone(),
                family: i.family.clone(),
                cloud_init: i.cloud_init,
                efi: i.efi,
                disk_gib: opts.shape.disk_gib.max(i.min_disk_gib),
                memory_mib: opts.shape.memory_mib.max(i.min_memory_mib),
            })
            .collect(),
        installer_only: catalogue.installer_only,
        not_offered: catalogue.not_offered,
        shape: opts.shape,
        fee_bps: vms::VM_FEE_BPS,
        max_replicas: vms::MAX_REPLICAS,
        max_upload_bytes: opts.max_upload_bytes,
        published: opts.domain.is_some(),
    }))
}

/// One of the buyer's machine orders, with what each replica is doing.
#[derive(Debug, Serialize)]
pub(super) struct MachineView {
    order_id: String,
    replicas: Vec<ReplicaStatus>,
}

/// `POST /market/vms/status`: what each replica of the caller's machines is
/// doing. One list call to the mesh for the buyer's namespace, whatever the
/// number of orders. 503 when the mesh does not answer.
pub(super) async fn status(
    State(st): State<AppState>,
    Json(req): Json<MarketAuth>,
) -> Result<Json<Vec<MachineView>>, ApiError> {
    let market = market_tenant(&st, &req).await?;
    if market.vm_opts().is_none() {
        return Err(MarketError::Unconfigured.into());
    }
    let orders = market.vm_orders_of(&req.appliance_id).await;
    if orders.is_empty() {
        return Ok(Json(Vec::new()));
    }
    let Some(namespace) = namespace_for(&req.appliance_id) else {
        return Ok(Json(Vec::new()));
    };
    let kube = kube_access(&st).await?;
    let url = format!(
        "{}/apis/kubevirt.io/v1/namespaces/{namespace}/virtualmachines?labelSelector={}%3Dtrue",
        kube.api,
        vms::MANAGED_LABEL,
    );
    let list = kube_get(&kube, &url).await?;
    Ok(Json(
        orders
            .into_iter()
            .map(|(order_id, _, replicas)| MachineView {
                replicas: vms::replica_statuses(&order_id, replicas, &list),
                order_id,
            })
            .collect(),
    ))
}

#[derive(Debug, Deserialize)]
pub(super) struct TicketReq {
    #[serde(flatten)]
    auth: MarketAuth,
    name: String,
    #[serde(default)]
    efi: bool,
}

#[derive(Debug, Serialize)]
pub(super) struct TicketView {
    upload_id: String,
    /// Where to `PUT` the file, relative to this edge. Single use, for an
    /// hour.
    upload_path: String,
    max_bytes: u64,
}

/// `POST /market/vm-images/ticket`: reserve an upload. Any market tenant may
/// upload; whether a box may *host* is a separate question.
pub(super) async fn ticket(
    State(st): State<AppState>,
    Json(req): Json<TicketReq>,
) -> Result<(StatusCode, Json<TicketView>), ApiError> {
    let market = market_tenant(&st, &req.auth).await?;
    let max_bytes = market
        .vm_opts()
        .ok_or(MarketError::Unconfigured)?
        .max_upload_bytes;
    let (upload_id, ticket) = market
        .upload_ticket(&req.auth.appliance_id, req.name.trim(), req.efi)
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(TicketView {
            upload_id,
            upload_path: format!("/market/vm-images/upload/{ticket}"),
            max_bytes,
        }),
    ))
}

#[derive(Debug, Deserialize)]
pub(super) struct RemoveReq {
    #[serde(flatten)]
    auth: MarketAuth,
    upload_id: String,
}

/// `POST /market/vm-images/remove`: forget one of the caller's images and
/// delete its file. Replicas that already imported it keep their disks.
pub(super) async fn remove(
    State(st): State<AppState>,
    Json(req): Json<RemoveReq>,
) -> Result<StatusCode, ApiError> {
    let market = market_tenant(&st, &req.auth).await?;
    let opts = market.vm_opts().ok_or(MarketError::Unconfigured)?.clone();
    if !upload_id_ok(&req.upload_id) {
        return Err(MarketError::NotFound.into());
    }
    market
        .remove_upload(&req.auth.appliance_id, &req.upload_id)
        .await?;
    delete_upload_files(&opts, &req.upload_id).await;
    Ok(StatusCode::NO_CONTENT)
}

/// `upl_` and hex, the shape [`Market::upload_ticket`] mints.
fn upload_id_ok(id: &str) -> bool {
    id.strip_prefix("upl_").is_some_and(|hex| {
        !hex.is_empty() && hex.len() <= 64 && hex.bytes().all(|b| b.is_ascii_hexdigit())
    })
}

// ── the transfers ────────────────────────────────────────────────────────

/// The two transfer routes, for `build` to merge outside the guard.
pub(super) fn transfer_router(state: AppState) -> Router {
    Router::new()
        .route("/market/vm-images/upload/{ticket}", put(upload))
        .route("/market/vm-images/fetch/{token}", get(fetch))
        .with_state(state)
}

/// A status and reason, kept small so the upload path's `Result`s stay cheap.
type Refusal = (StatusCode, &'static str);

fn refuse(status: StatusCode, why: &'static str) -> Response {
    (status, why).into_response()
}

/// Removes a half-written upload unless [`PartFile::keep`] is called, so a
/// client that hangs up mid-file (the handler's future is then dropped, not
/// finished) leaves nothing behind.
struct PartFile {
    path: PathBuf,
    armed: bool,
}

impl PartFile {
    fn keep(mut self) {
        self.armed = false;
    }
}

impl Drop for PartFile {
    fn drop(&mut self) {
        if self.armed {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// `PUT /market/vm-images/upload/{ticket}`: the file itself. The ticket is
/// the credential: 64 hex characters from [`crate::market::Market::upload_ticket`],
/// spent by a stored file. The body must be a QCOW2 (version 2 or 3) no
/// larger than the edge allows; anything else is refused before it is kept.
///
/// Answers: 201 stored, with `{upload_id, size, min_disk_gib}`; 404 no such
/// ticket, or it ran out; 409 the same ticket is already uploading; 413 too
/// large; 415 not a QCOW2; 408 the client stopped sending; 503 machines are
/// off or [`MAX_TRANSFERS`] are already running.
async fn upload(
    State(st): State<AppState>,
    UrlPath(ticket): UrlPath<String>,
    headers: HeaderMap,
    body: Body,
) -> Response {
    if !vms::capability_ok(&ticket) {
        return refuse(StatusCode::NOT_FOUND, "no such upload");
    }
    let market = match market_of(&st).await {
        Ok(m) => Arc::clone(m),
        Err(e) => return e.into_response(),
    };
    let Some(opts) = market.vm_opts().cloned() else {
        return ApiError::from(MarketError::Unconfigured).into_response();
    };
    let Some(pending) = market.upload_for_ticket(&ticket).await else {
        return refuse(StatusCode::NOT_FOUND, "no such upload, or its hour ran out");
    };
    let declared = headers
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());
    if declared.is_some_and(|n| n > opts.max_upload_bytes) {
        return refuse(
            StatusCode::PAYLOAD_TOO_LARGE,
            "the image is larger than this edge takes",
        );
    }
    let Ok(_permit) = Arc::clone(&st.transfers).try_acquire_owned() else {
        return refuse(
            StatusCode::SERVICE_UNAVAILABLE,
            "busy with other images; retry later",
        );
    };

    let dir = Path::new(&opts.upload_dir);
    if let Err(e) = ensure_private_dir(dir).await {
        tracing::error!(target: Action::Market.target(), "upload dir {}: {e}", dir.display());
        return refuse(
            StatusCode::INSUFFICIENT_STORAGE,
            "the edge cannot store images",
        );
    }
    let part_path = dir.join(format!("{}.part", pending.id));
    let file = match open_part(&part_path).await {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            return refuse(StatusCode::CONFLICT, "this image is already uploading");
        }
        Err(e) => {
            tracing::error!(target: Action::Market.target(), "open {}: {e}", part_path.display());
            return refuse(
                StatusCode::INSUFFICIENT_STORAGE,
                "the edge cannot store images",
            );
        }
    };
    let part = PartFile {
        path: part_path.clone(),
        armed: true,
    };
    let (size, virtual_size) = match receive(body, file, opts.max_upload_bytes).await {
        Ok(got) => got,
        Err((status, why)) => return refuse(status, why),
    };

    let final_path = opts.upload_path(&pending.id);
    if let Err(e) = tokio::fs::rename(&part_path, &final_path).await {
        tracing::error!(
            target: Action::Market.target(),
            "rename {} to {}: {e}",
            part_path.display(),
            final_path.display(),
        );
        return refuse(
            StatusCode::INSUFFICIENT_STORAGE,
            "the edge cannot store images",
        );
    }
    part.keep();
    if let Err(e) = market.upload_stored(&pending.id, size, virtual_size).await {
        let _ = tokio::fs::remove_file(&final_path).await;
        return ApiError::from(e).into_response();
    }
    tracing::info!(
        target: Action::Market.target(),
        "stored image {} for {} ({size} bytes, {virtual_size} virtual)",
        pending.id,
        pending.owner,
    );
    let min_disk_gib = virtual_size.div_ceil(1024 * 1024 * 1024);
    (
        StatusCode::CREATED,
        Json(serde_json::json!({
            "upload_id": pending.id,
            "size": size,
            "min_disk_gib": min_disk_gib,
        })),
    )
        .into_response()
}

/// Stream `body` into `file`, checking the QCOW2 header as soon as it has
/// arrived. Returns `(bytes, virtual size)`, or the status and reason to
/// refuse with.
async fn receive(
    mut body: Body,
    mut file: tokio::fs::File,
    max: u64,
) -> Result<(u64, u64), Refusal> {
    let mut head: Vec<u8> = Vec::with_capacity(32);
    let mut virtual_size = None;
    let mut size: u64 = 0;
    loop {
        let frame = tokio::time::timeout(
            TRANSFER_IDLE,
            std::future::poll_fn(|cx| Pin::new(&mut body).poll_frame(cx)),
        )
        .await
        .map_err(|_| (StatusCode::REQUEST_TIMEOUT, "the upload stalled"))?;
        let Some(frame) = frame else { break };
        let frame = frame.map_err(|_| (StatusCode::BAD_REQUEST, "the upload broke off"))?;
        let Ok(data) = frame.into_data() else {
            continue;
        };
        size = size.saturating_add(data.len() as u64);
        if size > max {
            return Err((
                StatusCode::PAYLOAD_TOO_LARGE,
                "the image is larger than this edge takes",
            ));
        }
        if virtual_size.is_none() {
            let want = 32 - head.len();
            head.extend_from_slice(&data[..want.min(data.len())]);
            if head.len() == 32 {
                virtual_size = Some(vms::qcow2_virtual_size(&head).ok_or((
                    StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    "not a QCOW2 image (version 2 or 3)",
                ))?);
            }
        }
        write_all(&mut file, &data).await?;
    }
    let Some(virtual_size) = virtual_size else {
        return Err((
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "not a QCOW2 image (version 2 or 3)",
        ));
    };
    if virtual_size > vms::MAX_DISK_GIB * 1024 * 1024 * 1024 {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            "the image's disk is larger than a machine may have",
        ));
    }
    file.sync_all().await.map_err(|e| {
        tracing::error!(target: Action::Market.target(), "sync upload: {e}");
        (
            StatusCode::INSUFFICIENT_STORAGE,
            "the edge cannot store images",
        )
    })?;
    Ok((size, virtual_size))
}

async fn write_all(file: &mut tokio::fs::File, data: &Bytes) -> Result<(), Refusal> {
    file.write_all(data).await.map_err(|e| {
        tracing::error!(target: Action::Market.target(), "write upload: {e}");
        (
            StatusCode::INSUFFICIENT_STORAGE,
            "the edge cannot store images",
        )
    })
}

async fn ensure_private_dir(dir: &Path) -> std::io::Result<()> {
    let dir = dir.to_path_buf();
    tokio::task::spawn_blocking(move || {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&dir)
    })
    .await
    .map_err(std::io::Error::other)?
}

/// Open a new partial file, refusing one that exists: that is the same
/// ticket uploading twice at once.
async fn open_part(path: &Path) -> std::io::Result<tokio::fs::File> {
    let path = path.to_path_buf();
    let file = tokio::task::spawn_blocking(move || {
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
    })
    .await
    .map_err(std::io::Error::other)??;
    Ok(tokio::fs::File::from_std(file))
}

/// `GET /market/vm-images/fetch/{token}`: a stored upload, for CDI's
/// importer. The token is the credential; it is written only into the
/// owner's own DataVolumes. Serves one byte range when asked
/// (`Range: bytes=a-b`), which lets the importer read the QCOW2 in place
/// rather than download it to scratch space first. `HEAD` answers the size.
async fn fetch(
    State(st): State<AppState>,
    UrlPath(token): UrlPath<String>,
    headers: HeaderMap,
) -> Response {
    if !vms::capability_ok(&token) {
        return refuse(StatusCode::NOT_FOUND, "no such image");
    }
    let market = match market_of(&st).await {
        Ok(m) => Arc::clone(m),
        Err(e) => return e.into_response(),
    };
    let Some(opts) = market.vm_opts().cloned() else {
        return ApiError::from(MarketError::Unconfigured).into_response();
    };
    let Some(upload) = market.upload_for_token(&token).await else {
        return refuse(StatusCode::NOT_FOUND, "no such image");
    };
    let Ok(permit) = Arc::clone(&st.transfers).try_acquire_owned() else {
        return refuse(
            StatusCode::SERVICE_UNAVAILABLE,
            "busy with other images; retry later",
        );
    };
    let path = opts.upload_path(&upload.id);
    let mut file = match tokio::fs::File::open(&path).await {
        Ok(f) => f,
        Err(e) => {
            tracing::error!(target: Action::Market.target(), "open {}: {e}", path.display());
            return refuse(StatusCode::NOT_FOUND, "no such image");
        }
    };
    let len = match file.metadata().await {
        Ok(m) => m.len(),
        Err(_) => return refuse(StatusCode::NOT_FOUND, "no such image"),
    };
    let range = headers
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok())
        .map(|v| parse_range(v, len));
    let (status, start, end) = match range {
        None | Some(Range::Ignored) => (StatusCode::OK, 0, len),
        Some(Range::Bytes(start, end)) => (StatusCode::PARTIAL_CONTENT, start, end),
        Some(Range::Unsatisfiable) => {
            let mut r = refuse(StatusCode::RANGE_NOT_SATISFIABLE, "range outside the image");
            if let Ok(v) = HeaderValue::from_str(&format!("bytes */{len}")) {
                r.headers_mut().insert(header::CONTENT_RANGE, v);
            }
            return r;
        }
    };
    if start > 0 && file.seek(std::io::SeekFrom::Start(start)).await.is_err() {
        return refuse(StatusCode::INTERNAL_SERVER_ERROR, "cannot read the image");
    }
    let remaining = end - start;
    let stream = futures_util::stream::unfold(
        (file, remaining, permit),
        |(mut file, remaining, permit)| async move {
            if remaining == 0 {
                return None;
            }
            let want = usize::try_from(remaining).map_or(FETCH_CHUNK, |r| r.min(FETCH_CHUNK));
            let mut buf = vec![0u8; want];
            match tokio::time::timeout(TRANSFER_IDLE, file.read(&mut buf)).await {
                Ok(Ok(0)) => None,
                Ok(Ok(n)) => {
                    buf.truncate(n);
                    Some((
                        Ok::<Bytes, std::io::Error>(Bytes::from(buf)),
                        (file, remaining - n as u64, permit),
                    ))
                }
                Ok(Err(e)) => Some((Err(e), (file, 0, permit))),
                Err(_) => Some((
                    Err(std::io::Error::other("image read stalled")),
                    (file, 0, permit),
                )),
            }
        },
    );
    let mut response = Response::new(Body::from_stream(stream));
    *response.status_mut() = status;
    let h = response.headers_mut();
    h.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    if let Ok(v) = HeaderValue::from_str(&remaining.to_string()) {
        h.insert(header::CONTENT_LENGTH, v);
    }
    if status == StatusCode::PARTIAL_CONTENT {
        if let Ok(v) = HeaderValue::from_str(&format!("bytes {start}-{}/{len}", end - 1)) {
            h.insert(header::CONTENT_RANGE, v);
        }
    }
    response
}

/// A `Range` header, as far as this route honours one.
#[derive(Debug, PartialEq, Eq)]
enum Range {
    /// `[start, end)`.
    Bytes(u64, u64),
    /// Not a single byte range this route serves; send the whole file, which
    /// RFC 9110 allows.
    Ignored,
    Unsatisfiable,
}

fn parse_range(value: &str, len: u64) -> Range {
    let Some(spec) = value.trim().strip_prefix("bytes=") else {
        return Range::Ignored;
    };
    if spec.contains(',') {
        return Range::Ignored;
    }
    let Some((a, b)) = spec.split_once('-') else {
        return Range::Ignored;
    };
    let (a, b) = (a.trim(), b.trim());
    if a.is_empty() {
        // `bytes=-N`: the last N bytes.
        let Ok(n) = b.parse::<u64>() else {
            return Range::Ignored;
        };
        if n == 0 || len == 0 {
            return Range::Unsatisfiable;
        }
        return Range::Bytes(len.saturating_sub(n), len);
    }
    let Ok(start) = a.parse::<u64>() else {
        return Range::Ignored;
    };
    if start >= len {
        return Range::Unsatisfiable;
    }
    let end = if b.is_empty() {
        len
    } else {
        match b.parse::<u64>() {
            Ok(last) if last >= start => last.saturating_add(1).min(len),
            _ => return Range::Ignored,
        }
    };
    Range::Bytes(start, end)
}

// ── fulfilment ───────────────────────────────────────────────────────────

/// The machine half of the market's reconcile pass: drop uploads whose
/// ticket ran out, create the replicas and Service of every paid order that
/// has none, halt every replica of a lapsed order, and rewrite the Traefik
/// file that publishes the live ones.
///
/// Idempotent like the storage half: a `409` on create is an earlier pass
/// that died before the order was marked, and a `404` on halt is a replica
/// the operator already deleted. Disks are never deleted here.
pub(super) async fn fulfil(st: &AppState, market: &Market) {
    let Some(opts) = market.vm_opts().cloned() else {
        return;
    };
    match market.sweep_uploads().await {
        Ok(dropped) => {
            for id in dropped {
                delete_upload_files(&opts, &id).await;
            }
        }
        Err(e) => tracing::error!(
            target: Action::Market.target(),
            "could not drop expired image uploads: {e}",
        ),
    }
    let pending = market.pending_vms().await;
    let lapsed = market.lapsed_vms().await;
    if !pending.is_empty() || !lapsed.is_empty() {
        match kube_access(st).await {
            Ok(kube) => {
                for p in pending {
                    create_machines(&kube, market, &opts, &p).await;
                }
                for (order_id, namespace, replicas) in lapsed {
                    halt_machines(&kube, market, &order_id, &namespace, replicas).await;
                }
            }
            Err(e) => tracing::error!(
                target: Action::Market.target(),
                "{} machine order(s) cannot be started and {} cannot be halted: {e}",
                pending.len(),
                lapsed.len(),
            ),
        }
    }
    publish_routes(market, &opts).await;
}

async fn create_machines(kube: &KubeAccess, market: &Market, opts: &vms::VmOpts, p: &VmProvision) {
    match create_machines_inner(kube, opts, p).await {
        Ok(service_ip) => match market.mark_vm_created(&p.order_id, service_ip).await {
            Ok(()) => tracing::info!(
                target: Action::Market.target(),
                "started {} replica(s) of order {} on {}",
                p.replicas,
                p.order_id,
                p.host,
            ),
            Err(e) => tracing::error!(
                target: Action::Market.target(),
                "machines for order {} exist but could not be recorded: {e}",
                p.order_id,
            ),
        },
        Err(e) => tracing::error!(
            target: Action::Market.target(),
            "starting machines for order {}: {e}",
            p.order_id,
        ),
    }
}

/// Namespace, then each replica, then the Service; the Service's cluster IP
/// is what Traefik is pointed at.
async fn create_machines_inner(
    kube: &KubeAccess,
    opts: &vms::VmOpts,
    p: &VmProvision,
) -> Result<Option<String>, ApiError> {
    super::ensure_namespace(kube, &p.namespace).await?;
    let machines = format!(
        "{}/apis/kubevirt.io/v1/namespaces/{}/virtualmachines",
        kube.api, p.namespace,
    );
    for name in vms::replica_names(&p.order_id, p.replicas) {
        let manifest = vms::vm_manifest(p, &name, opts.shape, opts.storage_class.as_deref());
        kube_create(kube, &machines, &manifest).await?;
    }
    let services = format!("{}/api/v1/namespaces/{}/services", kube.api, p.namespace);
    kube_create(
        kube,
        &services,
        &vms::service_manifest(&p.order_id, &p.namespace),
    )
    .await?;
    let service = kube_get(kube, &format!("{services}/{}", vms::stem(&p.order_id))).await?;
    Ok(service["spec"]["clusterIP"]
        .as_str()
        .filter(|ip| ip.parse::<std::net::IpAddr>().is_ok())
        .map(str::to_string))
}

async fn halt_machines(
    kube: &KubeAccess,
    market: &Market,
    order_id: &str,
    namespace: &str,
    replicas: u64,
) {
    for name in vms::replica_names(order_id, replicas) {
        let url = format!(
            "{}/apis/kubevirt.io/v1/namespaces/{namespace}/virtualmachines/{name}",
            kube.api,
        );
        if let Err(e) = kube_merge_patch(kube, &url, &vms::halt_patch()).await {
            tracing::error!(
                target: Action::Market.target(),
                "halting {namespace}/{name} of lapsed order {order_id}: {e}",
            );
            return;
        }
    }
    match market.mark_vm_halted(order_id).await {
        Ok(()) => tracing::info!(
            target: Action::Market.target(),
            "halted the {replicas} replica(s) of lapsed order {order_id}; their disks are kept",
        ),
        Err(e) => tracing::error!(
            target: Action::Market.target(),
            "replicas of order {order_id} are halted but could not be recorded: {e}",
        ),
    }
}

/// Write `losos-vms.yml` beside the tenants' Traefik file, or remove it when
/// no machine is live, so Traefik drops the routers of halted ones.
async fn publish_routes(market: &Market, opts: &vms::VmOpts) {
    let Some(file) = &opts.routes_file else {
        return;
    };
    let path = Path::new(file);
    let result = match crate::config::vm_routes_yaml(&market.vm_routes().await) {
        Some(yaml) => write_if_changed(path, &yaml, TRAEFIK_FILE_MODE).await,
        None => remove_if_exists(path).await,
    };
    if let Err(e) = result {
        tracing::error!(target: Action::Market.target(), "machine routes {file}: {e:?}");
    }
}

async fn delete_upload_files(opts: &vms::VmOpts, id: &str) {
    for path in [
        opts.upload_path(id),
        Path::new(&opts.upload_dir).join(format!("{id}.part")),
    ] {
        match tokio::fs::remove_file(&path).await {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => tracing::warn!(
                target: Action::Market.target(),
                "remove {}: {e}",
                path.display(),
            ),
        }
    }
}

async fn kube_get(kube: &KubeAccess, url: &str) -> Result<serde_json::Value, ApiError> {
    let response = kube
        .client
        .get(url)
        .bearer_auth(&kube.token)
        .send()
        .await
        .map_err(|e| ApiError::KubeApi(format!("GET {url}: {e}")))?;
    let status = response.status();
    if !status.is_success() {
        return Err(ApiError::KubeApi(format!("GET {url} -> {status}")));
    }
    response
        .json()
        .await
        .map_err(|e| ApiError::KubeApi(format!("GET {url}: {e}")))
}

/// A JSON merge patch; `404` counts as done (the object is gone).
async fn kube_merge_patch(
    kube: &KubeAccess,
    url: &str,
    patch: &serde_json::Value,
) -> Result<(), ApiError> {
    let response = kube
        .client
        .patch(url)
        .bearer_auth(&kube.token)
        .header(header::CONTENT_TYPE, "application/merge-patch+json")
        .body(patch.to_string())
        .send()
        .await
        .map_err(|e| ApiError::KubeApi(format!("PATCH {url}: {e}")))?;
    let status = response.status();
    if status.is_success() || status == StatusCode::NOT_FOUND {
        return Ok(());
    }
    Err(ApiError::KubeApi(format!("PATCH {url} -> {status}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_single_byte_range_is_served_and_anything_else_is_the_whole_file() {
        assert_eq!(parse_range("bytes=0-99", 1000), Range::Bytes(0, 100));
        assert_eq!(parse_range("bytes=900-", 1000), Range::Bytes(900, 1000));
        assert_eq!(parse_range("bytes=-100", 1000), Range::Bytes(900, 1000));
        assert_eq!(parse_range("bytes=990-5000", 1000), Range::Bytes(990, 1000));
        assert_eq!(parse_range("bytes=1000-", 1000), Range::Unsatisfiable);
        assert_eq!(parse_range("bytes=0-1,5-9", 1000), Range::Ignored);
        assert_eq!(parse_range("items=0-1", 1000), Range::Ignored);
        assert_eq!(parse_range("bytes=9-1", 1000), Range::Ignored);
    }
}
