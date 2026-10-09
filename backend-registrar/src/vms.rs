//! Virtual machines on the mesh, sold by the replica.
//!
//! A box that shares its storage with the mesh (`losos.sharingMyStorage`)
//! can also host other people's virtual machines, and any market tenant can
//! rent them. The hypervisor is KubeVirt on the edge's mesh cluster, with CDI
//! importing the disk images (`modules/edge.nix`, `losos.edge.vms`); this
//! module is what the market needs to sell and fulfil them.
//!
//! How it fits the market (`crate::market`):
//!
//!   * A hosting box lists [`crate::market::Kind::Vm`], priced per
//!     replica-month, with as many replicas as it is willing to run. It may
//!     only do so while it is enrolled in the mesh and says it hosts machines
//!     (`/cluster/join`'s `host_vms`, which the box sends only while its
//!     shared storage is on).
//!   * A buyer orders N replicas of one image. The split is fixed at half and
//!     half ([`VM_FEE_BPS`]): the Stripe destination charge forwards half to
//!     the hosting box's owner and the platform keeps the other half. The
//!     operator's `--market-fee-bps` does not apply to machines, and the Stripe
//!     gate refuses a machine checkout with any other split.
//!   * Once paid, the reconciler creates N `VirtualMachine`s in the buyer's
//!     `market-<buyer>` namespace, pinned to the hosting box by its
//!     `losos.dev/appliance` node label, each importing its own disk from the
//!     chosen image, and one `Service` in front of them. With `--vm-domain`
//!     set the edge's Traefik serves that Service at
//!     `https://<order>.<vm-domain>`, load-balanced over the replicas.
//!   * When the month runs out every replica is halted (`runStrategy:
//!     Halted`), which hands the replicas back to the listing. The disks stay:
//!     they hold the buyer's data, and removing them is the operator's call,
//!     as it is for a storage order's volume.
//!
//! The image catalogue ([`Catalogue`]) is the LosOS demo image plus every
//! system Quickemu supports that publishes a ready-to-boot cloud disk. The
//! rest of Quickemu's list ships installer ISOs only; those need an install
//! console in the browser, which does not exist yet, so the catalogue names
//! them and offers none of them. A buyer may also upload their own QCOW2:
//! the file is kept on the edge ([`Upload`]) and each replica imports it from
//! a capability URL only the cluster is told.
//!
//! Everything in this file is pure: manifests, validation, the catalogue and
//! the QCOW2 header check. The routes and the apiserver calls are in
//! `server.rs`, next to the storage fulfilment they mirror.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::market::MarketError;

/// The platform's half of a machine sale, in basis points. Not an operator
/// setting: LosOS and the hosting box's owner split it evenly, and the gate
/// holds every machine checkout to exactly this.
pub const VM_FEE_BPS: u32 = 5_000;
/// Most replicas one order may run.
pub const MAX_REPLICAS: u64 = 10;
/// Most replicas one box may offer to host.
pub const MAX_HOSTED_REPLICAS: u64 = 50;
/// Largest cloud-init document a buyer may hand a machine.
pub const MAX_USER_DATA: usize = 16 * 1024;
/// Longest display name of a machine or an upload.
pub const MAX_NAME_CHARS: usize = 40;
/// Uploaded images one buyer may keep on the edge at once.
pub const MAX_UPLOADS_PER_BUYER: usize = 3;
/// How long an upload ticket may wait for its file.
pub const UPLOAD_TICKET_SECS: u64 = 3600;
/// The largest virtual disk a replica may ask for, uploaded or not.
pub const MAX_DISK_GIB: u64 = 512;
/// The most memory a replica may ask for.
pub const MAX_MEMORY_MIB: u64 = 64 * 1024;
/// The node label every LosOS box carries in the mesh (`modules/cluster.nix`).
pub const APPLIANCE_LABEL: &str = "losos.dev/appliance";
/// The compute-window taint (`modules/edge.nix`). A machine tolerates it: a
/// box that hosts machines has agreed to run them around the clock, and a
/// paid month cannot stop every morning.
pub const WINDOW_TAINT: &str = "losos.dev/compute-window";
/// Label on everything the market creates, and the order it belongs to.
pub const MANAGED_LABEL: &str = "losos.market/managed";
pub const ORDER_LABEL: &str = "losos.market/order";

/// The built-in catalogue. An operator can replace it with
/// `--vm-catalogue`; the file has the same shape.
pub const DEFAULT_CATALOGUE: &str = include_str!("../vm-images.json");

/// Where a replica's disk comes from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    /// A disk image over https (QCOW2, raw, optionally xz or gz): CDI's
    /// `http` source.
    Http(String),
    /// A KubeVirt container disk, `docker://…`: CDI's `registry` source.
    Registry(String),
}

impl Source {
    fn valid(&self) -> bool {
        match self {
            Source::Http(url) => url.starts_with("https://") && plain_url(url),
            Source::Registry(url) => url.starts_with("docker://") && plain_url(url),
        }
    }

    /// The `spec.source` of a CDI DataVolume.
    #[must_use]
    pub fn cdi(&self) -> Value {
        match self {
            Source::Http(url) => json!({ "http": { "url": url } }),
            Source::Registry(url) => json!({ "registry": { "url": url } }),
        }
    }
}

fn plain_url(url: &str) -> bool {
    url.len() <= 512
        && url.bytes().all(|b| {
            b.is_ascii_graphic() && !matches!(b, b'"' | b'\'' | b'\\' | b'`' | b'<' | b'>')
        })
}

/// One image a buyer can start replicas of.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Image {
    /// `ubuntu-24.04`, `losos`. Lowercase, digits, `.` and `-`.
    pub id: String,
    pub name: String,
    /// What it is a release of: Quickemu's own name for the system
    /// (`quickget <family>`), or `losos`.
    pub family: String,
    pub source: Source,
    /// The smallest disk the image fits on; the replica gets the larger of
    /// this and the edge's `--vm-disk-gib`.
    #[serde(default)]
    pub min_disk_gib: u64,
    /// The least memory it runs in; the replica gets the larger of this and
    /// the edge's `--vm-memory-mib`. LosOS runs two clusters and LosOS cloud,
    /// so it asks for more than a cloud image does.
    #[serde(default)]
    pub min_memory_mib: u64,
    /// Whether the image reads cloud-init, i.e. whether a buyer's user data
    /// does anything.
    #[serde(default)]
    pub cloud_init: bool,
    /// Boots with UEFI firmware rather than BIOS. The LosOS image is GPT with
    /// an ESP and systemd-boot, and starts no other way.
    #[serde(default)]
    pub efi: bool,
}

/// A system Quickemu supports that the catalogue deliberately does not offer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotOffered {
    pub family: String,
    pub why: String,
}

/// What the edge offers to run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Catalogue {
    pub images: Vec<Image>,
    /// Quickemu systems that ship installer ISOs only. They need an install
    /// console, which the market does not have yet.
    #[serde(default)]
    pub installer_only: Vec<String>,
    /// Quickemu systems left out for another reason, with the reason.
    #[serde(default)]
    pub not_offered: Vec<NotOffered>,
}

impl Catalogue {
    /// Parse and check a catalogue.
    ///
    /// # Errors
    /// A sentence naming the first entry that is wrong.
    pub fn parse(text: &str) -> Result<Self, String> {
        let catalogue: Catalogue =
            serde_json::from_str(text).map_err(|e| format!("not a VM catalogue: {e}"))?;
        let mut seen = BTreeSet::new();
        for image in &catalogue.images {
            if !image_id_ok(&image.id) || image.id.starts_with("upl") {
                return Err(format!("image id {:?} is not valid", image.id));
            }
            if !seen.insert(image.id.as_str()) {
                return Err(format!("image id {:?} appears twice", image.id));
            }
            if !display_name_ok(&image.name) {
                return Err(format!("image {:?} has no usable name", image.id));
            }
            if !image.source.valid() {
                return Err(format!(
                    "image {:?}: the source must be an https:// or docker:// URL",
                    image.id
                ));
            }
            if image.min_disk_gib > MAX_DISK_GIB {
                return Err(format!("image {:?} needs too large a disk", image.id));
            }
            if image.min_memory_mib > MAX_MEMORY_MIB {
                return Err(format!("image {:?} needs too much memory", image.id));
            }
        }
        if catalogue.images.is_empty() {
            return Err("the catalogue offers no image".to_string());
        }
        Ok(catalogue)
    }

    /// The built-in catalogue, or the operator's file.
    ///
    /// # Errors
    /// The file cannot be read or is not a valid catalogue.
    pub fn load(path: Option<&str>) -> Result<Self, String> {
        match path {
            None => Self::parse(DEFAULT_CATALOGUE),
            Some(p) => {
                let text = std::fs::read_to_string(p).map_err(|e| format!("read {p}: {e}"))?;
                Self::parse(&text)
            }
        }
    }

    #[must_use]
    pub fn image(&self, id: &str) -> Option<&Image> {
        self.images.iter().find(|i| i.id == id)
    }
}

/// `[a-z0-9][a-z0-9.-]{0,39}`.
#[must_use]
pub fn image_id_ok(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 40
        && id.as_bytes()[0].is_ascii_alphanumeric()
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-')
}

/// A short printable name: what the owner calls a machine or an upload.
#[must_use]
pub fn display_name_ok(name: &str) -> bool {
    let trimmed = name.trim();
    !trimmed.is_empty()
        && trimmed.len() == name.len()
        && name.chars().count() <= MAX_NAME_CHARS
        && !name.chars().any(char::is_control)
}

/// The cloud-init a buyer may pass: a `#cloud-config` document or a script,
/// UTF-8, at most [`MAX_USER_DATA`] bytes.
///
/// # Errors
/// The sentence the buyer sees.
pub fn check_user_data(text: &str) -> Result<(), MarketError> {
    if text.len() > MAX_USER_DATA {
        return Err(MarketError::Invalid("cloud-init is longer than 16 KiB"));
    }
    if !(text.starts_with("#cloud-config") || text.starts_with("#!")) {
        return Err(MarketError::Invalid(
            "cloud-init must start with #cloud-config or #!",
        ));
    }
    if text.contains('\0') {
        return Err(MarketError::Invalid("cloud-init has a NUL byte"));
    }
    Ok(())
}

/// What the buyer asked for on a machine order, and what the edge has done
/// about it. Rides on [`crate::market::Order`] as `vm`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VmOrder {
    /// A catalogue id or one of the buyer's upload ids (`upl_…`).
    pub image: String,
    /// What the buyer calls it.
    pub name: String,
    #[serde(default)]
    pub user_data: Option<String>,
    /// Every replica's `VirtualMachine` and the `Service` exist.
    #[serde(default)]
    pub created: bool,
    /// Every replica was halted after the order lapsed.
    #[serde(default)]
    pub halted: bool,
    /// The Service's cluster IP, once known; the edge's Traefik routes to it.
    #[serde(default)]
    pub service_ip: Option<String>,
}

/// The hostname-safe stem of an order: `vm-` and the first twelve hex digits
/// of its id. Short enough for every name built from it to stay a DNS label.
#[must_use]
pub fn stem(order_id: &str) -> String {
    let hex: String = order_id
        .strip_prefix("ord_")
        .unwrap_or(order_id)
        .chars()
        .filter(char::is_ascii_hexdigit)
        .take(12)
        .collect::<String>()
        .to_ascii_lowercase();
    format!("vm-{hex}")
}

/// The `VirtualMachine` names of an order's replicas, `<stem>-<n>`.
#[must_use]
pub fn replica_names(order_id: &str, replicas: u64) -> Vec<String> {
    let stem = stem(order_id);
    (0..replicas).map(|n| format!("{stem}-{n}")).collect()
}

/// The size of one replica, the same for every machine on this edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Shape {
    pub cpu: u32,
    pub memory_mib: u64,
    pub disk_gib: u64,
}

impl Default for Shape {
    fn default() -> Self {
        Self {
            cpu: 1,
            memory_mib: 2048,
            disk_gib: 20,
        }
    }
}

/// The `--vm-*` settings: present when the edge's mesh runs KubeVirt.
#[derive(Debug, Clone)]
pub struct VmOpts {
    /// `--vm-catalogue`; `None` is the built-in [`DEFAULT_CATALOGUE`].
    pub catalogue: Option<String>,
    pub shape: Shape,
    /// `--vm-storage-class`: where replicas' disks are claimed from. `None`
    /// leaves it to the cluster's default class.
    pub storage_class: Option<String>,
    /// `--vm-upload-dir`: where buyers' images are kept. 0700.
    pub upload_dir: String,
    /// `--vm-upload-max-gib`, in bytes: the largest file one upload may be.
    pub max_upload_bytes: u64,
    /// `--vm-fetch-base`: this registrar's public https origin, which CDI's
    /// importer on the hosting box fetches an upload from.
    pub fetch_base: String,
    /// `--vm-domain`: machines are published as `https://<order>.<domain>`.
    /// `None` publishes nothing.
    pub domain: Option<String>,
    /// The Traefik dynamic file the machine routes are written to (beside
    /// `losos.yml`). `None` with no domain.
    pub routes_file: Option<String>,
}

/// An order's image, resolved to what a replica is made from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    pub source: Source,
    pub disk_gib: u64,
    pub memory_mib: u64,
    pub efi: bool,
}

impl VmOpts {
    /// Where one upload is kept.
    #[must_use]
    pub fn upload_path(&self, id: &str) -> std::path::PathBuf {
        std::path::Path::new(&self.upload_dir).join(format!("{id}.qcow2"))
    }

    /// The URL CDI fetches an upload from.
    #[must_use]
    pub fn fetch_url(&self, token: &str) -> String {
        format!(
            "{}/market/vm-images/fetch/{token}",
            self.fetch_base.trim_end_matches('/')
        )
    }

    /// Resolve `image` for `buyer`: a catalogue entry, or one of the buyer's
    /// stored uploads. `None` for anything else.
    #[must_use]
    pub fn resolve(
        &self,
        catalogue: &Catalogue,
        uploads: &std::collections::BTreeMap<String, Upload>,
        buyer: &str,
        image: &str,
    ) -> Option<Resolved> {
        if let Some(entry) = catalogue.image(image) {
            return Some(Resolved {
                source: entry.source.clone(),
                disk_gib: self.shape.disk_gib.max(entry.min_disk_gib),
                memory_mib: self.shape.memory_mib.max(entry.min_memory_mib),
                efi: entry.efi,
            });
        }
        let upload = uploads
            .get(image)
            .filter(|u| u.owner == buyer && u.stored())?;
        Some(Resolved {
            source: Source::Http(self.fetch_url(&upload.token)),
            disk_gib: self.shape.disk_gib.max(upload.min_disk_gib()),
            memory_mib: self.shape.memory_mib,
            efi: upload.efi,
        })
    }
}

/// One paid machine order the reconciler still has to create.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmProvision {
    pub order_id: String,
    pub namespace: String,
    /// The hosting box's appliance id, which is its node's
    /// `losos.dev/appliance` label.
    pub host: String,
    pub replicas: u64,
    pub source: Source,
    pub disk_gib: u64,
    pub memory_mib: u64,
    pub efi: bool,
    pub user_data: Option<String>,
}

/// One replica's `VirtualMachine`.
///
/// It boots from its own DataVolume, imported by CDI from `p.source` on the
/// hosting box (the storage class decides where the disk lives; the edge's
/// default keeps it in the box's shared directory), with one virtio NIC
/// behind the pod network's masquerade. It is pinned to the hosting box and
/// tolerates the compute-window taint.
#[must_use]
pub fn vm_manifest(
    p: &VmProvision,
    name: &str,
    shape: Shape,
    storage_class: Option<&str>,
) -> Value {
    let order = stem(&p.order_id);
    let disk = format!("{name}-disk");
    // Access and volume mode spelled out: CDI reads them from the class's
    // StorageProfile otherwise, and a local-path class has none.
    let mut storage = json!({
        "accessModes": ["ReadWriteOnce"],
        "volumeMode": "Filesystem",
        "resources": { "requests": { "storage": format!("{}Gi", p.disk_gib) } },
    });
    if let Some(class) = storage_class {
        storage["storageClassName"] = json!(class);
    }
    let mut disks = vec![json!({ "name": "root", "disk": { "bus": "virtio" } })];
    let mut volumes = vec![json!({ "name": "root", "dataVolume": { "name": disk } })];
    if let Some(user_data) = &p.user_data {
        disks.push(json!({ "name": "cloudinit", "disk": { "bus": "virtio" } }));
        volumes.push(json!({
            "name": "cloudinit",
            "cloudInitNoCloud": { "userData": user_data },
        }));
    }
    let mut domain = json!({
        "cpu": { "cores": shape.cpu },
        "memory": { "guest": format!("{}Mi", p.memory_mib.max(shape.memory_mib)) },
        "devices": {
            "disks": disks,
            "interfaces": [{ "name": "default", "masquerade": {} }],
        },
    });
    if p.efi {
        // No Secure Boot: it needs SMM, which not every host offers, and the
        // LosOS image's loader is not signed for the guest's firmware anyway.
        domain["firmware"] = json!({ "bootloader": { "efi": { "secureBoot": false } } });
    }
    json!({
        "apiVersion": "kubevirt.io/v1",
        "kind": "VirtualMachine",
        "metadata": {
            "name": name,
            "namespace": p.namespace,
            "labels": { MANAGED_LABEL: "true", ORDER_LABEL: order },
        },
        "spec": {
            "runStrategy": "Always",
            "dataVolumeTemplates": [{
                "metadata": { "name": disk },
                "spec": { "source": p.source.cdi(), "storage": storage },
            }],
            "template": {
                "metadata": {
                    "labels": { MANAGED_LABEL: "true", ORDER_LABEL: order },
                },
                "spec": {
                    "nodeSelector": { APPLIANCE_LABEL: p.host },
                    "tolerations": [{
                        "key": WINDOW_TAINT,
                        "operator": "Exists",
                        "effect": "NoSchedule",
                    }],
                    "domain": domain,
                    "networks": [{ "name": "default", "pod": {} }],
                    "volumes": volumes,
                },
            },
        },
    })
}

/// The `Service` in front of an order's replicas: port 80 on each.
#[must_use]
pub fn service_manifest(order_id: &str, namespace: &str) -> Value {
    let order = stem(order_id);
    json!({
        "apiVersion": "v1",
        "kind": "Service",
        "metadata": {
            "name": order,
            "namespace": namespace,
            "labels": { MANAGED_LABEL: "true", ORDER_LABEL: order },
        },
        "spec": {
            "selector": { ORDER_LABEL: order },
            "ports": [{ "name": "http", "port": 80, "targetPort": 80, "protocol": "TCP" }],
        },
    })
}

/// The merge patch that stops a replica and keeps its disk.
#[must_use]
pub fn halt_patch() -> Value {
    json!({ "spec": { "runStrategy": "Halted" } })
}

/// What a replica is doing, from its `VirtualMachine`'s
/// `status.printableStatus` (`Provisioning`, `Starting`, `Running`,
/// `Stopped`, `ErrorUnschedulable`, …). The buyer sees one word of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReplicaStatus {
    pub name: String,
    pub status: String,
}

/// Read the replicas out of a `VirtualMachineList`, in replica order. One
/// that is not in the list yet reads `Preparing`, like one whose disk is
/// still importing.
#[must_use]
pub fn replica_statuses(order_id: &str, replicas: u64, list: &Value) -> Vec<ReplicaStatus> {
    let found: std::collections::BTreeMap<&str, &str> = list["items"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|vm| {
                    Some((
                        vm["metadata"]["name"].as_str()?,
                        vm["status"]["printableStatus"]
                            .as_str()
                            .unwrap_or("Starting"),
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    replica_names(order_id, replicas)
        .into_iter()
        .map(|name| {
            let status = found.get(name.as_str()).map_or("Pending", |s| *s);
            ReplicaStatus {
                status: printable(status).to_string(),
                name,
            }
        })
        .collect()
}

/// Squash KubeVirt's status words into the handful the page draws.
fn printable(status: &str) -> &'static str {
    match status {
        "Running" => "Running",
        "Stopped" | "Stopping" | "Halted" => "Stopped",
        "Provisioning" | "WaitingForVolumeBinding" | "Pending" => "Preparing",
        "Starting" | "Migrating" => "Starting",
        "Paused" => "Paused",
        s if s.starts_with("Error") || s.contains("Error") || s == "CrashLoopBackOff" => "Failed",
        "Terminating" => "Stopped",
        _ => "Starting",
    }
}

// ── uploads ──────────────────────────────────────────────────────────────

/// A buyer's own image, kept on the edge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Upload {
    /// `upl_<hex>`.
    pub id: String,
    pub owner: String,
    pub name: String,
    /// The single-use capability the box streams the file to. Cleared once
    /// the file is stored.
    #[serde(default)]
    pub ticket: Option<String>,
    /// The capability CDI fetches the file with. Only ever written into the
    /// buyer's own DataVolumes.
    pub token: String,
    pub created_at: u64,
    /// Bytes on the edge, once stored.
    #[serde(default)]
    pub size: Option<u64>,
    /// The QCOW2's virtual disk size, once stored.
    #[serde(default)]
    pub virtual_size: Option<u64>,
    /// The owner says it boots with UEFI.
    #[serde(default)]
    pub efi: bool,
}

impl Upload {
    #[must_use]
    pub fn stored(&self) -> bool {
        self.size.is_some()
    }

    /// GiB a replica of this image needs: its virtual size, rounded up.
    #[must_use]
    pub fn min_disk_gib(&self) -> u64 {
        self.virtual_size
            .map_or(0, |bytes| bytes.div_ceil(1024 * 1024 * 1024))
    }
}

/// An upload as its owner sees it: no capabilities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UploadView {
    pub id: String,
    pub name: String,
    pub stored: bool,
    pub size: Option<u64>,
    pub min_disk_gib: u64,
    pub created_at: u64,
}

impl UploadView {
    #[must_use]
    pub fn of(u: &Upload) -> Self {
        Self {
            id: u.id.clone(),
            name: u.name.clone(),
            stored: u.stored(),
            size: u.size,
            min_disk_gib: u.min_disk_gib(),
            created_at: u.created_at,
        }
    }
}

/// A 64-hex-character capability.
#[must_use]
pub fn capability_ok(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// The QCOW2 header's virtual size, or `None` when `header` is not the start
/// of a version 2 or 3 QCOW2 file. Only QCOW2 is taken: it is what the page
/// asks for, and a raw image has no header to read a size from.
#[must_use]
pub fn qcow2_virtual_size(header: &[u8]) -> Option<u64> {
    if header.len() < 32 || &header[..4] != b"QFI\xfb" {
        return None;
    }
    let version = u32::from_be_bytes(header[4..8].try_into().ok()?);
    if !(2..=3).contains(&version) {
        return None;
    }
    let size = u64::from_be_bytes(header[24..32].try_into().ok()?);
    (size > 0).then_some(size)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provision() -> VmProvision {
        VmProvision {
            order_id: "ord_0123456789abcdef01234567".to_string(),
            namespace: "market-buyer-box".to_string(),
            host: "seller-box".to_string(),
            replicas: 2,
            source: Source::Http("https://example.org/disk.qcow2".to_string()),
            disk_gib: 20,
            memory_mib: 0,
            efi: false,
            user_data: None,
        }
    }

    #[test]
    fn the_built_in_catalogue_loads_and_offers_losos() {
        let c = Catalogue::load(None).expect("built-in catalogue");
        let losos = c.image("losos").expect("LosOS is offered");
        assert!(matches!(losos.source, Source::Registry(_)));
        assert!(c.images.len() > 5);
        assert!(!c.installer_only.is_empty());
        assert!(c.not_offered.iter().any(|n| n.family == "macos"));
        // Every family is one Quickemu knows, or LosOS.
        for image in &c.images {
            assert!(!image.family.is_empty(), "{}", image.id);
        }
    }

    #[test]
    fn a_catalogue_with_a_bad_entry_is_refused() {
        let one = |id: &str, url: &str| {
            format!(
                r#"{{"images":[{{"id":"{id}","name":"X","family":"x","source":{{"http":"{url}"}}}}]}}"#
            )
        };
        assert!(Catalogue::parse(&one("x-1", "https://a.example/x.qcow2")).is_ok());
        assert!(Catalogue::parse(&one("X", "https://a.example/x.qcow2")).is_err());
        assert!(Catalogue::parse(&one("upl_1", "https://a.example/x.qcow2")).is_err());
        assert!(Catalogue::parse(&one("x", "http://a.example/x.qcow2")).is_err());
        assert!(Catalogue::parse(&one("x", "https://a.example/x\\\".qcow2")).is_err());
        assert!(Catalogue::parse(r#"{"images":[]}"#).is_err());
    }

    #[test]
    fn names_and_cloud_init_are_bounded() {
        assert!(display_name_ok("web server"));
        assert!(!display_name_ok(""));
        assert!(!display_name_ok(" padded"));
        assert!(!display_name_ok("tab\there"));
        assert!(!display_name_ok(&"x".repeat(41)));
        assert!(check_user_data("#cloud-config\npackages: [nginx]\n").is_ok());
        assert!(check_user_data("#!/bin/sh\necho hi\n").is_ok());
        assert!(check_user_data("packages: [nginx]").is_err());
        assert!(check_user_data(&format!("#!/bin/sh\n{}", "x".repeat(MAX_USER_DATA))).is_err());
    }

    #[test]
    fn replicas_are_named_from_the_order_and_stay_dns_labels() {
        let names = replica_names("ord_0123456789abcdef01234567", 3);
        assert_eq!(
            names,
            [
                "vm-0123456789ab-0",
                "vm-0123456789ab-1",
                "vm-0123456789ab-2"
            ]
        );
        assert!(names.iter().all(|n| n.len() < 63));
    }

    #[test]
    fn a_replica_is_pinned_to_its_host_and_tolerates_the_window() {
        let mut p = provision();
        p.user_data = Some("#cloud-config\n".to_string());
        let vm = vm_manifest(
            &p,
            "vm-0123456789ab-0",
            Shape::default(),
            Some("losos-vm-local"),
        );
        let spec = &vm["spec"]["template"]["spec"];
        assert_eq!(spec["nodeSelector"][APPLIANCE_LABEL], "seller-box");
        assert_eq!(spec["tolerations"][0]["key"], WINDOW_TAINT);
        assert_eq!(spec["domain"]["cpu"]["cores"], 1);
        assert_eq!(spec["domain"]["memory"]["guest"], "2048Mi");
        assert_eq!(vm["metadata"]["namespace"], "market-buyer-box");
        assert_eq!(vm["spec"]["runStrategy"], "Always");
        let dv = &vm["spec"]["dataVolumeTemplates"][0];
        assert_eq!(dv["metadata"]["name"], "vm-0123456789ab-0-disk");
        assert_eq!(
            dv["spec"]["source"]["http"]["url"],
            "https://example.org/disk.qcow2"
        );
        assert_eq!(
            dv["spec"]["storage"]["resources"]["requests"]["storage"],
            "20Gi"
        );
        assert_eq!(dv["spec"]["storage"]["storageClassName"], "losos-vm-local");
        assert_eq!(
            spec["volumes"][1]["cloudInitNoCloud"]["userData"],
            "#cloud-config\n"
        );
        // The Service selects every replica of the order and nothing else.
        let svc = service_manifest(&p.order_id, &p.namespace);
        assert_eq!(svc["spec"]["selector"][ORDER_LABEL], "vm-0123456789ab");
        assert_eq!(
            vm["spec"]["template"]["metadata"]["labels"][ORDER_LABEL],
            "vm-0123456789ab"
        );
    }

    #[test]
    fn without_user_data_no_cloud_init_disk_is_attached() {
        let vm = vm_manifest(&provision(), "vm-x-0", Shape::default(), None);
        let spec = &vm["spec"]["template"]["spec"];
        assert_eq!(spec["volumes"].as_array().map(Vec::len), Some(1));
        assert!(vm["spec"]["dataVolumeTemplates"][0]["spec"]["storage"]
            .get("storageClassName")
            .is_none());
        assert!(spec["domain"].get("firmware").is_none());
    }

    #[test]
    fn an_efi_image_with_more_memory_gets_both() {
        let mut p = provision();
        p.efi = true;
        p.memory_mib = 4096;
        let vm = vm_manifest(&p, "vm-x-0", Shape::default(), None);
        let domain = &vm["spec"]["template"]["spec"]["domain"];
        assert_eq!(domain["firmware"]["bootloader"]["efi"]["secureBoot"], false);
        assert_eq!(domain["memory"]["guest"], "4096Mi");
    }

    #[test]
    fn statuses_come_back_in_replica_order_and_missing_ones_are_preparing() {
        let list = json!({ "items": [
            { "metadata": { "name": "vm-0123456789ab-1" }, "status": { "printableStatus": "Running" } },
            { "metadata": { "name": "vm-0123456789ab-0" }, "status": { "printableStatus": "ErrorUnschedulable" } },
        ]});
        let s = replica_statuses("ord_0123456789abcdef01234567", 3, &list);
        let words: Vec<&str> = s.iter().map(|r| r.status.as_str()).collect();
        assert_eq!(words, ["Failed", "Running", "Preparing"]);
    }

    #[test]
    fn only_a_qcow2_header_gives_a_size() {
        let mut h = vec![0u8; 64];
        h[..4].copy_from_slice(b"QFI\xfb");
        h[4..8].copy_from_slice(&3u32.to_be_bytes());
        h[24..32].copy_from_slice(&(10u64 * 1024 * 1024 * 1024 + 1).to_be_bytes());
        assert_eq!(qcow2_virtual_size(&h), Some(10 * 1024 * 1024 * 1024 + 1));
        let u = Upload {
            id: "upl_1".to_string(),
            owner: "b".to_string(),
            name: "n".to_string(),
            ticket: None,
            token: "t".to_string(),
            created_at: 0,
            size: Some(1),
            virtual_size: qcow2_virtual_size(&h),
            efi: false,
        };
        assert_eq!(u.min_disk_gib(), 11);
        h[4..8].copy_from_slice(&1u32.to_be_bytes());
        assert_eq!(qcow2_virtual_size(&h), None);
        assert_eq!(qcow2_virtual_size(b"\x7fELF"), None);
        assert_eq!(qcow2_virtual_size(&[0u8; 64]), None);
    }

    #[test]
    fn capabilities_are_64_lowercase_hex() {
        assert!(capability_ok(&"a1".repeat(32)));
        assert!(!capability_ok(&"A1".repeat(32)));
        assert!(!capability_ok("../../etc/passwd"));
    }
}
