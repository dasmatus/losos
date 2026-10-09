//! Installing an app the catalogue found: a Helm chart, run in this box's own
//! cluster as one of its two data users.
//!
//! # What "installing" means on this box
//!
//! The local k3s server (`modules/cluster.nix`) runs no CNI, no kube-proxy
//! and no DNS, and its one node is NotReady for life. That is on purpose and
//! it is why Nextcloud and Forgejo are static pods. A chart from the
//! catalogue knows none of this: its Deployment would sit Pending behind the
//! not-ready taint, and its pods would never get a sandbox without a pod
//! network. So every chart is rendered by Helm and then *shaped* by
//! [`shape`] before anything reaches the cluster:
//!
//!   * every pod runs on the host network, tolerates every taint and
//!     resolves through the host, like the box's own workloads;
//!   * every container runs as the user the owner chose (`notshared`, uid
//!     1000, or `shared`, uid 1001), never root, with no capabilities and no
//!     way to gain any;
//!   * every volume claim becomes a directory in that user's data domain,
//!     `/home/<user>/data/apps/<release>/<claim>`, which is persisted;
//!   * the app gets no access to the cluster: RBAC objects are dropped and no
//!     service account token is mounted;
//!   * anything that would reach past those rules (a host path, a cluster-wide
//!     object, a port the box keeps for itself) refuses the install with a
//!     sentence naming it.
//!
//! The non-root uid is what keeps a pod on the host network away from lososd:
//! `modules/daemon.nix` rejects loopback connections to the API port from
//! every uid but root and nginx.
//!
//! The shaping runs as a Helm 4 post-renderer plugin (`modules/apps.nix`),
//! the `losos-app-shape` binary. Helm passes hooks through it as well, and
//! `--skip-crds` keeps the one thing it would not see out of the install.
//!
//! # What the owner reaches
//!
//! A pod on the host network listens on the box itself, behind the firewall.
//! nginx forwards one port of a fixed range (`losos.apps.ports`) to the
//! app's port, LAN only, so the page can link `http://<box>:<port>/`. The
//! forward is a file under `/var/lib/losos-apps/nginx`, written by the job
//! that installed the app.
//!
//! # Shape
//!
//! Pure validation, port allocation and shaping, tested here; the effects
//! (fetching a chart, starting the job, the records on disk) behind
//! [`crate::losos::Losos`] like every other command.

use crate::losos::Losos;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

/// Every app gets a namespace of its own, so removing it is one uninstall and
/// nothing an app makes can land beside another's.
pub const NAMESPACE_PREFIX: &str = "losos-app-";

/// The label every shaped object carries.
pub const RELEASE_LABEL: &str = "losos.app/release";

/// Longest app name. A DNS label is 63 characters and the namespace adds the
/// prefix; Helm wants a release name of 53 at most.
pub const MAX_RELEASE_CHARS: usize = 40;

/// Largest values document from the property form, encoded.
pub const MAX_VALUES_BYTES: usize = 64 * 1024;

/// Largest values text from the text editor.
pub const MAX_VALUES_YAML_BYTES: usize = 256 * 1024;

/// The lowest port an unprivileged process can bind. Every app runs without
/// root and without capabilities, so a chart that listens below this cannot
/// start, and it is better told so than left crash-looping.
pub const FIRST_UNPRIVILEGED_PORT: u16 = 1024;

/// A request the owner can act on, in a sentence. The HTTP layer answers it
/// as a 400.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invalid(pub String);

impl std::fmt::Display for Invalid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Invalid {}

fn invalid<T>(message: impl Into<String>) -> anyhow::Result<T> {
    Err(anyhow::Error::new(Invalid(message.into())))
}

/// An install that cannot happen on this box right now: no cluster to run it
/// in, the shared side locked, a job already running. The HTTP layer answers
/// it as a 409.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict(pub String);

impl std::fmt::Display for Conflict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Conflict {}

fn conflict<T>(message: impl Into<String>) -> anyhow::Result<T> {
    Err(anyhow::Error::new(Conflict(message.into())))
}

// ── Who the app runs as ───────────────────────────────────────────────────

/// The two data users of `modules/configuration.nix`. Their uids are pinned
/// there and asserted by `tests/impermanence.nix`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunAs {
    /// uid 1000, the private side. Always available.
    Notshared,
    /// uid 1001, the side lent to the mesh. Its data domain is fscrypt-locked
    /// while the box does not share its disk (`modules/fscrypt.nix`), so an
    /// app can run as it only then.
    Shared,
}

impl RunAs {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "notshared" => Some(RunAs::Notshared),
            "shared" => Some(RunAs::Shared),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            RunAs::Notshared => "notshared",
            RunAs::Shared => "shared",
        }
    }

    /// The uid, which is also the gid.
    pub fn uid(self) -> u32 {
        match self {
            RunAs::Notshared => 1000,
            RunAs::Shared => 1001,
        }
    }

    /// Where this user's apps keep their files. Inside `/home/<user>/data`
    /// because that is the persisted directory (`modules/impermanence.nix`);
    /// anywhere else in the home is tmpfs and gone at the next boot.
    pub fn apps_root(self) -> String {
        format!("/home/{}/data/apps", self.as_str())
    }

    pub fn data_root(self, release: &str) -> String {
        format!("{}/{release}", self.apps_root())
    }
}

// ── The chart ─────────────────────────────────────────────────────────────

/// Where a chart comes from: a repository, a name in it and a version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChartRef {
    /// `https://` for a classic chart repository, `oci://` for a registry.
    pub repo: String,
    pub name: String,
    pub version: String,
}

/// `https://` or `oci://` followed by a host, no userinfo, no whitespace.
fn is_chart_repo(u: &str) -> bool {
    let rest = ["https://", "oci://"].iter().find_map(|scheme| {
        u.get(..scheme.len())
            .filter(|p| p.eq_ignore_ascii_case(scheme))
            .map(|_| &u[scheme.len()..])
    });
    let Some(rest) = rest else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    !authority.is_empty()
        && !authority.contains('@')
        && !u.chars().any(|c| c.is_whitespace() || c.is_control())
        && u.len() <= 512
}

/// A chart name as Helm and Artifact Hub spell them.
fn is_chart_name(n: &str) -> bool {
    let mut chars = n.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_lowercase() || c.is_ascii_digit())
        && n.len() <= 100
        && n.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '_' | '.'))
}

/// A version Helm can be asked for: semver's alphabet, nothing that could be
/// read as a range or a flag.
fn is_chart_version(v: &str) -> bool {
    !v.is_empty()
        && v.len() <= 64
        && !v.starts_with('-')
        && v.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+' | '_'))
}

impl ChartRef {
    /// The chart as given, or a sentence saying which part is wrong. Every
    /// part ends up in an argument vector of `helm`, so a value that starts
    /// with `-` or carries a space is refused here rather than trusted to the
    /// argument parser.
    pub fn validate(&self) -> anyhow::Result<()> {
        if !is_chart_repo(&self.repo) {
            return invalid("The chart's repository must be an https:// or oci:// address.");
        }
        if !is_chart_name(&self.name) {
            return invalid("That is not a chart name.");
        }
        if !is_chart_version(&self.version) {
            return invalid("That is not a chart version.");
        }
        Ok(())
    }

    /// The reference `helm` takes, and the `--repo` it needs with it: an OCI
    /// chart is addressed by URL, a classic one by name within a repository.
    pub fn helm_ref(&self) -> (String, Option<String>) {
        if self.repo.to_ascii_lowercase().starts_with("oci://") {
            (
                format!("{}/{}", self.repo.trim_end_matches('/'), self.name),
                None,
            )
        } else {
            (self.name.clone(), Some(self.repo.clone()))
        }
    }
}

/// What the install dialog draws its properties from.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChartFiles {
    /// The chart's `values.yaml`, as written, for the text editor.
    #[serde(rename = "valuesYaml")]
    pub values_yaml: String,
    /// The same document as JSON, for the property form.
    pub values: Value,
    /// The chart's `values.schema.json`, when it ships one.
    pub schema: Option<Value>,
}

// ── The app's name ────────────────────────────────────────────────────────

/// A release name: a DNS label, short enough for the namespace prefix.
pub fn validate_release(name: &str) -> anyhow::Result<()> {
    let ok = !name.is_empty()
        && name.len() <= MAX_RELEASE_CHARS
        && name.starts_with(|c: char| c.is_ascii_lowercase())
        && !name.ends_with('-')
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if ok {
        Ok(())
    } else {
        invalid(format!(
            "Name the app with small letters, digits and dashes, starting with a letter, \
             at most {MAX_RELEASE_CHARS} characters."
        ))
    }
}

/// The name an app gets unless the owner types another: the chart's, folded
/// into a release name.
#[must_use]
pub fn default_release(chart: &str) -> String {
    let mut out = String::new();
    for c in chart.chars() {
        let c = c.to_ascii_lowercase();
        if c.is_ascii_lowercase() || (c.is_ascii_digit() && !out.is_empty()) {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
        if out.len() == MAX_RELEASE_CHARS {
            break;
        }
    }
    let out = out.trim_end_matches('-').to_string();
    if out.is_empty() {
        "app".to_string()
    } else {
        out
    }
}

// ── The request ───────────────────────────────────────────────────────────

/// `POST /api/apps/install`.
#[derive(Debug, Clone, PartialEq)]
pub struct InstallRequest {
    pub release: String,
    pub chart: ChartRef,
    pub run_as: RunAs,
    /// What the property form changed, as one object Helm reads last.
    pub values: Value,
    /// The values text, when the owner edited it.
    pub values_yaml: Option<String>,
}

impl InstallRequest {
    /// Read and check the body. Every refusal is a sentence for the dialog.
    pub fn parse(body: &Value) -> anyhow::Result<Self> {
        let text = |key: &str| body.get(key).and_then(Value::as_str).map(str::trim);
        let chart = body.get("chart").unwrap_or(&Value::Null);
        let chart_text = |key: &str| {
            chart
                .get(key)
                .and_then(Value::as_str)
                .map(|s| s.trim().to_string())
                .unwrap_or_default()
        };
        let chart = ChartRef {
            repo: chart_text("repo"),
            name: chart_text("name"),
            version: chart_text("version"),
        };
        chart.validate()?;

        let release = text("release").unwrap_or_default().to_string();
        validate_release(&release)?;

        let Some(run_as) = text("runAs").and_then(RunAs::parse) else {
            return invalid("Choose who the app runs as: notshared or shared.");
        };

        let values = match body.get("values") {
            None | Some(Value::Null) => Value::Object(Map::new()),
            Some(v @ Value::Object(_)) => v.clone(),
            Some(_) => return invalid("The properties must be one object."),
        };
        if serde_json::to_vec(&values)?.len() > MAX_VALUES_BYTES {
            return invalid("The properties are too large.");
        }

        let values_yaml = match body.get("valuesYaml") {
            None | Some(Value::Null) => None,
            Some(Value::String(s)) if s.len() > MAX_VALUES_YAML_BYTES => {
                return invalid("The values text is too large.");
            }
            Some(Value::String(s)) if s.contains('\0') => {
                return invalid("The values text has a NUL byte in it.");
            }
            Some(Value::String(s)) => Some(s.clone()),
            Some(_) => return invalid("The values text must be text."),
        };

        Ok(InstallRequest {
            release,
            chart,
            run_as,
            values,
            values_yaml,
        })
    }
}

// ── The record of an installed app ────────────────────────────────────────

/// Where an app is in its life. The job writes `running` or `failed` when it
/// ends; lososd writes the other two when it starts one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Installing,
    Running,
    Failed,
    Removing,
}

impl Phase {
    /// A job is meant to be running in this phase.
    pub fn busy(self) -> bool {
        matches!(self, Phase::Installing | Phase::Removing)
    }
}

/// One installed app, as `<apps dir>/<release>/app.json`. The install job
/// reads the chart and the values from it and writes the outcome back.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    pub release: String,
    pub chart: ChartRef,
    pub run_as: RunAs,
    #[serde(default)]
    pub values: Value,
    #[serde(default)]
    pub values_yaml: Option<String>,
    /// The port nginx forwards to the app, from `losos.apps.ports`.
    pub front_port: u16,
    /// The port the app listens on, once the job has rendered the chart.
    #[serde(default)]
    pub app_port: Option<u16>,
    pub phase: Phase,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub updated_at: u64,
}

/// What lososd needs to know about this box to offer installs at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// The range nginx forwards from, inclusive.
    pub ports: (u16, u16),
}

/// Read `$LOSOS_APPS_PORTS` (`30000-30099`).
pub fn parse_port_range(raw: &str) -> Option<(u16, u16)> {
    let (a, b) = raw.trim().split_once('-')?;
    let (a, b) = (a.parse().ok()?, b.parse().ok()?);
    (a <= b && a >= FIRST_UNPRIVILEGED_PORT).then_some((a, b))
}

/// The front port for `release`: the one it already has, else the lowest the
/// other apps leave free. `None` when the range is used up.
pub fn allocate_port(range: (u16, u16), records: &[Record], release: &str) -> Option<u16> {
    if let Some(own) = records.iter().find(|r| r.release == release) {
        return Some(own.front_port);
    }
    (range.0..=range.1).find(|p| records.iter().all(|r| r.front_port != *p))
}

// ── Shaping a rendered chart ──────────────────────────────────────────────

/// What [`shape`] needs to know about the install it is shaping.
#[derive(Debug, Clone)]
pub struct ShapeCtx {
    pub release: String,
    pub run_as: RunAs,
    /// Ports the box keeps for itself (`$LOSOS_APPS_RESERVED_PORTS`).
    pub reserved: Vec<u16>,
    /// Ports other apps already listen on, with the app's name.
    pub taken: Vec<(u16, String)>,
}

impl ShapeCtx {
    pub fn namespace(&self) -> String {
        format!("{NAMESPACE_PREFIX}{}", self.release)
    }
}

/// The chart as it will be applied, and what the job needs to know about it.
#[derive(Debug, Clone, PartialEq)]
pub struct Shaped {
    pub docs: Vec<Value>,
    /// Directories to create, owned by the app's user, before the pods start.
    pub dirs: Vec<String>,
    /// Every TCP port a container declares.
    pub ports: Vec<u16>,
    /// The one the page links to.
    pub primary: Option<u16>,
}

/// A chart this box will not run, and why, in a sentence for the owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal(pub String);

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Refusal {}

fn refuse<T>(message: impl Into<String>) -> Result<T, Refusal> {
    Err(Refusal(message.into()))
}

/// Kinds dropped without a word: they do nothing on a single node with no
/// ingress, no autoscaler and no network policy, or (RBAC) they would hand
/// the app power over the cluster. Dropping can only take power away.
const DROPPED_KINDS: &[&str] = &[
    "PersistentVolumeClaim",
    "Ingress",
    "NetworkPolicy",
    "PodDisruptionBudget",
    "HorizontalPodAutoscaler",
    "VerticalPodAutoscaler",
    "Role",
    "RoleBinding",
    "ClusterRole",
    "ClusterRoleBinding",
    "ServiceMonitor",
    "PodMonitor",
    "PrometheusRule",
    "HTTPRoute",
    "GRPCRoute",
];

/// Where a kept kind's pod template sits, by kind, with the API group it must
/// belong to. A kind outside this list and [`DROPPED_KINDS`] refuses the
/// install: a webhook, a cluster-wide object or a volume of its own could
/// all reach past the rules this module keeps.
fn kept_kind(api_version: &str, kind: &str) -> Option<&'static [&'static str]> {
    let group = api_version.split_once('/').map_or("", |(g, _)| g);
    match (group, kind) {
        ("", "Pod") => Some(&["spec"]),
        ("apps", "Deployment" | "StatefulSet" | "DaemonSet" | "ReplicaSet") | ("batch", "Job") => {
            Some(&["spec", "template", "spec"])
        }
        ("batch", "CronJob") => Some(&["spec", "jobTemplate", "spec", "template", "spec"]),
        ("", "Service" | "ConfigMap" | "Secret" | "ServiceAccount") => Some(&[]),
        _ => None,
    }
}

fn get_path_mut<'a>(doc: &'a mut Value, path: &[&str]) -> Option<&'a mut Map<String, Value>> {
    let mut at = doc;
    for key in path {
        at = at.get_mut(*key)?;
    }
    at.as_object_mut()
}

/// A Helm test pod: only `helm test` runs it, and nothing here does.
fn is_test_hook(doc: &Value) -> bool {
    doc.pointer("/metadata/annotations/helm.sh~1hook")
        .and_then(Value::as_str)
        .is_some_and(|h| h.split(',').any(|h| h.trim().starts_with("test")))
}

/// Shape a rendered chart for this box. See the module docs for the rules;
/// each refusal names the object that broke one.
pub fn shape(docs: Vec<Value>, ctx: &ShapeCtx) -> Result<Shaped, Refusal> {
    let mut out = Vec::new();
    let mut dirs: Vec<String> = Vec::new();
    let mut ports: Vec<(u16, Option<String>)> = Vec::new();
    let mut service_ports: Vec<u16> = Vec::new();
    let root = ctx.run_as.data_root(&ctx.release);
    let namespace = ctx.namespace();

    for mut doc in docs {
        if !doc.is_object() {
            continue;
        }
        let kind = doc
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let api_version = doc
            .get("apiVersion")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let name = doc
            .pointer("/metadata/name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if kind.is_empty() {
            continue;
        }
        if DROPPED_KINDS.contains(&kind.as_str()) || is_test_hook(&doc) {
            continue;
        }
        let Some(pod_path) = kept_kind(&api_version, &kind) else {
            return refuse(format!(
                "This app asks for a {kind} ({name}), which apps on this box cannot have."
            ));
        };

        let meta = doc
            .as_object_mut()
            .expect("checked above")
            .entry("metadata")
            .or_insert_with(|| json!({}));
        if let Some(meta) = meta.as_object_mut() {
            meta.insert("namespace".into(), json!(namespace));
            let labels = meta.entry("labels").or_insert_with(|| json!({}));
            if let Some(labels) = labels.as_object_mut() {
                labels.insert(RELEASE_LABEL.into(), json!(ctx.release));
            }
        }

        match kind.as_str() {
            "Service" => shape_service(&mut doc, &mut service_ports),
            "Deployment" | "StatefulSet" | "ReplicaSet" => {
                if let Some(spec) = get_path_mut(&mut doc, &["spec"]) {
                    // One node, and every pod on the host network: a second
                    // replica could only fail to bind the first one's port.
                    if spec.get("replicas").and_then(Value::as_u64).unwrap_or(1) > 1 {
                        spec.insert("replicas".into(), json!(1));
                    }
                }
            }
            _ => {}
        }

        // A StatefulSet's claim templates become directories too: the
        // volumes are added to the pod under the template's name, which is
        // the name its containers mount.
        let mut claim_templates = Vec::new();
        if kind == "StatefulSet" {
            if let Some(spec) = get_path_mut(&mut doc, &["spec"]) {
                if let Some(Value::Array(templates)) = spec.remove("volumeClaimTemplates") {
                    for t in templates {
                        if let Some(n) = t.pointer("/metadata/name").and_then(Value::as_str) {
                            claim_templates.push(n.to_string());
                        }
                    }
                }
                spec.remove("persistentVolumeClaimRetentionPolicy");
            }
        }

        if !pod_path.is_empty() {
            let Some(pod) = get_path_mut(&mut doc, pod_path) else {
                return refuse(format!("The {kind} {name} has no pod in it."));
            };
            for claim in claim_templates {
                let volumes = pod.entry("volumes").or_insert_with(|| json!([]));
                if let Some(volumes) = volumes.as_array_mut() {
                    if !volumes
                        .iter()
                        .any(|v| v.get("name").and_then(Value::as_str) == Some(claim.as_str()))
                    {
                        volumes.push(json!({ "name": claim, "persistentVolumeClaim": { "claimName": claim } }));
                    }
                }
            }
            shape_pod(pod, ctx, &root, &kind, &name, &mut dirs, &mut ports)?;
        }
        out.push(doc);
    }

    let mut all: Vec<u16> = ports.iter().map(|(p, _)| *p).collect();
    all.sort_unstable();
    all.dedup();
    // A port declared only on a Service still tells us where the app listens.
    let candidates: Vec<u16> = if all.is_empty() {
        let mut s = service_ports.clone();
        s.sort_unstable();
        s.dedup();
        s
    } else {
        all.clone()
    };
    for port in &candidates {
        check_port(*port, ctx)?;
    }

    let named_web = |n: &Option<String>| {
        n.as_deref()
            .is_some_and(|n| ["http", "web", "ui", "https"].iter().any(|w| n.contains(w)))
    };
    let primary = ports
        .iter()
        .find(|(_, n)| named_web(n))
        .or_else(|| ports.first())
        .map(|(p, _)| *p)
        .or_else(|| service_ports.first().copied());

    dirs.sort();
    dirs.dedup();
    Ok(Shaped {
        docs: out,
        dirs,
        ports: candidates,
        primary,
    })
}

fn check_port(port: u16, ctx: &ShapeCtx) -> Result<(), Refusal> {
    if port < FIRST_UNPRIVILEGED_PORT {
        return refuse(format!(
            "This app listens on port {port}. Apps here run without root and can only use \
             ports from {FIRST_UNPRIVILEGED_PORT} up; look for a port setting in its properties."
        ));
    }
    if ctx.reserved.contains(&port) {
        return refuse(format!(
            "This app listens on port {port}, which the box keeps for itself. \
             Look for a port setting in its properties."
        ));
    }
    if let Some((_, other)) = ctx
        .taken
        .iter()
        .find(|(p, other)| *p == port && *other != ctx.release)
    {
        return refuse(format!(
            "This app listens on port {port}, which the app {other} already uses. \
             Look for a port setting in its properties."
        ));
    }
    Ok(())
}

/// No node ports, no load balancer: nothing would serve either, and Helm's
/// wait would sit on a load balancer address that never comes.
fn shape_service(doc: &mut Value, ports: &mut Vec<u16>) {
    let Some(spec) = get_path_mut(doc, &["spec"]) else {
        return;
    };
    if matches!(
        spec.get("type").and_then(Value::as_str),
        Some("LoadBalancer" | "NodePort")
    ) {
        spec.insert("type".into(), json!("ClusterIP"));
    }
    for key in [
        "externalIPs",
        "loadBalancerIP",
        "loadBalancerSourceRanges",
        "externalTrafficPolicy",
        "healthCheckNodePort",
        "allocateLoadBalancerNodePorts",
        "loadBalancerClass",
    ] {
        spec.remove(key);
    }
    if let Some(Value::Array(list)) = spec.get_mut("ports") {
        for p in list.iter_mut().filter_map(Value::as_object_mut) {
            p.remove("nodePort");
            let tcp = p
                .get("protocol")
                .and_then(Value::as_str)
                .is_none_or(|proto| proto == "TCP");
            let target = p
                .get("targetPort")
                .and_then(Value::as_u64)
                .or_else(|| p.get("port").and_then(Value::as_u64));
            if let (true, Some(t)) = (tcp, target.and_then(|t| u16::try_from(t).ok())) {
                ports.push(t);
            }
        }
    }
}

fn shape_pod(
    pod: &mut Map<String, Value>,
    ctx: &ShapeCtx,
    root: &str,
    kind: &str,
    name: &str,
    dirs: &mut Vec<String>,
    ports: &mut Vec<(u16, Option<String>)>,
) -> Result<(), Refusal> {
    let uid = ctx.run_as.uid();

    // The box's own workloads run this way (modules/workloads.nix): no pod
    // network exists to put them on, and no DNS service to ask.
    pod.insert("hostNetwork".into(), json!(true));
    pod.insert("dnsPolicy".into(), json!("Default"));
    pod.insert("automountServiceAccountToken".into(), json!(false));
    // The one node is NotReady for life; nothing else would ever schedule.
    pod.insert("tolerations".into(), json!([{ "operator": "Exists" }]));
    for key in [
        "hostPID",
        "hostIPC",
        "hostUsers",
        "nodeName",
        "nodeSelector",
        "affinity",
        "topologySpreadConstraints",
        "priorityClassName",
        "priority",
        "preemptionPolicy",
        "runtimeClassName",
        "schedulerName",
        "ephemeralContainers",
        "resourceClaims",
    ] {
        pod.remove(key);
    }
    pod.insert(
        "securityContext".into(),
        json!({
            "runAsUser": uid,
            "runAsGroup": uid,
            "fsGroup": uid,
            "runAsNonRoot": true,
            "seccompProfile": { "type": "RuntimeDefault" },
        }),
    );

    if let Some(Value::Array(volumes)) = pod.get_mut("volumes") {
        for volume in volumes.iter_mut().filter_map(Value::as_object_mut) {
            shape_volume(volume, root, kind, name, dirs)?;
        }
    }

    for field in ["initContainers", "containers"] {
        let Some(Value::Array(containers)) = pod.get_mut(field) else {
            continue;
        };
        for c in containers.iter_mut().filter_map(Value::as_object_mut) {
            if c.contains_key("volumeDevices") {
                return refuse(format!(
                    "The {kind} {name} asks for a raw device, which apps on this box cannot have."
                ));
            }
            let read_only_root = c
                .get("securityContext")
                .and_then(|s| s.get("readOnlyRootFilesystem"))
                .and_then(Value::as_bool);
            let mut security = json!({
                "runAsUser": uid,
                "runAsGroup": uid,
                "runAsNonRoot": true,
                "privileged": false,
                "allowPrivilegeEscalation": false,
                "capabilities": { "drop": ["ALL"] },
                "seccompProfile": { "type": "RuntimeDefault" },
            });
            if let Some(ro) = read_only_root {
                security["readOnlyRootFilesystem"] = json!(ro);
            }
            c.insert("securityContext".into(), security);

            if let Some(Value::Array(mounts)) = c.get_mut("volumeMounts") {
                for m in mounts.iter_mut().filter_map(Value::as_object_mut) {
                    // Bidirectional propagation needs a privileged container.
                    m.remove("mountPropagation");
                }
            }
            if let Some(Value::Array(list)) = c.get_mut("ports") {
                for p in list.iter_mut().filter_map(Value::as_object_mut) {
                    // On the host network the container port *is* the host
                    // port; a different one would not validate.
                    p.remove("hostPort");
                    p.remove("hostIP");
                    let tcp = p
                        .get("protocol")
                        .and_then(Value::as_str)
                        .is_none_or(|proto| proto == "TCP");
                    let port = p
                        .get("containerPort")
                        .and_then(Value::as_u64)
                        .and_then(|n| u16::try_from(n).ok());
                    if let (true, Some(port), "containers") = (tcp, port, field) {
                        let pname = p.get("name").and_then(Value::as_str).map(str::to_string);
                        ports.push((port, pname));
                    }
                }
            }
        }
    }
    Ok(())
}

/// A claim becomes a directory in the user's domain; the kinds of volume
/// that carry nothing of the host's pass; everything else refuses.
fn shape_volume(
    volume: &mut Map<String, Value>,
    root: &str,
    kind: &str,
    name: &str,
    dirs: &mut Vec<String>,
) -> Result<(), Refusal> {
    let vname = volume
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let source = volume.keys().find(|k| *k != "name").cloned();
    match source.as_deref() {
        None | Some("emptyDir" | "configMap" | "secret" | "downwardAPI") => Ok(()),
        Some("projected") => {
            if let Some(Value::Array(sources)) = volume
                .get_mut("projected")
                .and_then(|p| p.get_mut("sources"))
            {
                sources.retain(|s| {
                    s.get("configMap").is_some()
                        || s.get("secret").is_some()
                        || s.get("downwardAPI").is_some()
                });
            }
            Ok(())
        }
        // A generic ephemeral volume needs a provisioner this box does not
        // run; scratch space is what it was for.
        Some("ephemeral") => {
            volume.remove("ephemeral");
            volume.insert("emptyDir".into(), json!({}));
            Ok(())
        }
        Some("persistentVolumeClaim") => {
            let claim = volume
                .get("persistentVolumeClaim")
                .and_then(|c| c.get("claimName"))
                .and_then(Value::as_str)
                .unwrap_or(&vname)
                .to_string();
            if !is_dir_name(&claim) {
                return refuse(format!(
                    "The {kind} {name} names a strange volume ({claim})."
                ));
            }
            let path = format!("{root}/{claim}");
            volume.remove("persistentVolumeClaim");
            // `Directory`, not `DirectoryOrCreate`: the kubelet would make a
            // missing one as root, which the app's user could not write. The
            // job makes it first, owned by that user.
            volume.insert(
                "hostPath".into(),
                json!({ "path": path, "type": "Directory" }),
            );
            dirs.push(path);
            Ok(())
        }
        Some(other) => refuse(format!(
            "The {kind} {name} asks for a {other} volume, which apps on this box cannot have."
        )),
    }
}

/// A claim name is a DNS name; anything else must not become a path.
fn is_dir_name(n: &str) -> bool {
    !n.is_empty()
        && n.len() <= 253
        && !n.starts_with('.')
        && n.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '.'))
}

/// The shaped documents as Helm reads them back: JSON is YAML, one document
/// after another.
#[must_use]
pub fn render(docs: &[Value]) -> String {
    let mut out = String::new();
    for doc in docs {
        out.push_str("---\n");
        out.push_str(&serde_json::to_string(doc).expect("a Value always encodes"));
        out.push('\n');
    }
    out
}

// ── Commands ──────────────────────────────────────────────────────────────

/// The phase the page should show: a busy record whose job is gone stopped
/// before it could say how it ended.
fn effective<L: Losos>(l: &mut L, mut r: Record) -> Record {
    if r.phase.busy() && !l.app_job_active(&r.release) {
        r.phase = Phase::Failed;
        if r.message.is_none() {
            r.message = Some("The job stopped before it finished.".to_string());
        }
    }
    r
}

fn record_json(r: &Record) -> Value {
    json!({
        "release": r.release,
        "chart": r.chart,
        "runAs": r.run_as,
        "values": r.values,
        "valuesYaml": r.values_yaml,
        "frontPort": r.front_port,
        "appPort": r.app_port,
        "phase": r.phase,
        "message": r.message,
        "updatedAt": r.updated_at,
    })
}

/// `GET /api/apps`: what is installed, and whether anything can be.
pub fn cmd_apps<L: Losos>(l: &mut L) -> anyhow::Result<Value> {
    if l.apps_config().is_none() {
        return Ok(json!({ "available": false, "reason": "noCluster", "apps": [] }));
    }
    let shared = l.load_state()?.sharing;
    let mut apps = Vec::new();
    for r in l.load_app_records()? {
        let r = effective(l, r);
        apps.push(record_json(&r));
    }
    Ok(json!({ "available": true, "sharedAvailable": shared, "apps": apps }))
}

/// `GET /api/apps/chart`: the chart's values and schema, for the dialog.
pub fn cmd_app_chart<L: Losos>(l: &mut L, chart: &ChartRef) -> anyhow::Result<Value> {
    chart.validate()?;
    if l.apps_config().is_none() {
        return conflict("This box runs no apps of its own, so it cannot install one.");
    }
    let files = l.chart_files(chart)?;
    Ok(json!({
        "chart": chart,
        "release": default_release(&chart.name),
        "valuesYaml": files.values_yaml,
        "values": files.values,
        "schema": files.schema,
    }))
}

/// `POST /api/apps/install`: install an app, or change one already installed
/// under the same name.
pub fn cmd_app_install<L: Losos>(l: &mut L, req: &InstallRequest) -> anyhow::Result<Value> {
    let Some(config) = l.apps_config() else {
        return conflict("This box runs no apps of its own, so it cannot install one.");
    };
    if req.run_as == RunAs::Shared && !l.load_state()?.sharing {
        return conflict(
            "An app can run as shared only while this box shares its disk. \
             Choose notshared, or turn sharing on first.",
        );
    }
    let records = l.load_app_records()?;
    if let Some(old) = records.iter().find(|r| r.release == req.release) {
        if old.chart.name != req.chart.name || old.chart.repo != req.chart.repo {
            return conflict(format!(
                "Another app is already called {}. Choose a different name.",
                req.release
            ));
        }
        if old.run_as != req.run_as {
            return conflict(format!(
                "{} already runs as {}. Remove it first to run it as {}.",
                req.release,
                old.run_as.as_str(),
                req.run_as.as_str()
            ));
        }
        if l.app_job_active(&req.release) {
            return conflict(format!(
                "{} is still being changed. Wait for it to finish.",
                req.release
            ));
        }
    }
    let Some(front_port) = allocate_port(config.ports, &records, &req.release) else {
        return conflict("This box has no room for another app. Remove one first.");
    };
    let record = Record {
        release: req.release.clone(),
        chart: req.chart.clone(),
        run_as: req.run_as,
        values: req.values.clone(),
        values_yaml: req.values_yaml.clone(),
        front_port,
        app_port: records
            .iter()
            .find(|r| r.release == req.release)
            .and_then(|r| r.app_port),
        phase: Phase::Installing,
        message: None,
        updated_at: l.now(),
    };
    l.save_app_record(&record)?;
    l.start_app_job(&record.release, Action::Install)?;
    Ok(record_json(&record))
}

/// `POST /api/apps/remove`: take an app off the box. Its files stay in the
/// user's data domain, so installing it again under the same name finds them.
pub fn cmd_app_remove<L: Losos>(l: &mut L, release: &str) -> anyhow::Result<Value> {
    validate_release(release)?;
    if l.apps_config().is_none() {
        return conflict("This box runs no apps of its own.");
    }
    let Some(mut record) = l
        .load_app_records()?
        .into_iter()
        .find(|r| r.release == release)
    else {
        return invalid(format!("No app here is called {release}."));
    };
    if l.app_job_active(release) {
        return conflict(format!(
            "{release} is still being changed. Wait for it to finish."
        ));
    }
    record.phase = Phase::Removing;
    record.message = None;
    record.updated_at = l.now();
    l.save_app_record(&record)?;
    l.start_app_job(release, Action::Remove)?;
    Ok(record_json(&record))
}

/// What a job does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Install,
    Remove,
}

impl Action {
    pub fn as_str(self) -> &'static str {
        match self {
            Action::Install => "install",
            Action::Remove => "remove",
        }
    }
}

/// The transient unit a job runs in. One per app, so a second job for the
/// same app cannot start while the first runs.
#[must_use]
pub fn job_unit(release: &str) -> String {
    format!("losos-app-{release}.service")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> ShapeCtx {
        ShapeCtx {
            release: "jellyfin".into(),
            run_as: RunAs::Notshared,
            reserved: vec![8080, 8082, 3000],
            taken: vec![],
        }
    }

    fn deployment() -> Value {
        json!({
            "apiVersion": "apps/v1",
            "kind": "Deployment",
            "metadata": { "name": "jellyfin", "namespace": "default" },
            "spec": {
                "replicas": 3,
                "template": { "spec": {
                    "nodeSelector": { "disk": "ssd" },
                    "hostPID": true,
                    "securityContext": { "runAsUser": 0, "sysctls": [{ "name": "a", "value": "b" }] },
                    "containers": [{
                        "name": "jellyfin",
                        "image": "jellyfin/jellyfin:10.10",
                        "securityContext": { "privileged": true, "capabilities": { "add": ["SYS_ADMIN"] } },
                        "ports": [{ "name": "http", "containerPort": 8096, "hostPort": 80 }],
                        "volumeMounts": [{ "name": "config", "mountPath": "/config", "mountPropagation": "Bidirectional" }],
                    }],
                    "volumes": [
                        { "name": "config", "persistentVolumeClaim": { "claimName": "jellyfin-config" } },
                        { "name": "cache", "emptyDir": {} },
                    ],
                }},
            },
        })
    }

    #[test]
    fn a_deployment_runs_on_the_host_network_as_the_chosen_user_and_nothing_more() {
        let shaped = shape(vec![deployment()], &ctx()).unwrap();
        let d = &shaped.docs[0];
        assert_eq!(d["metadata"]["namespace"], "losos-app-jellyfin");
        assert_eq!(d["metadata"]["labels"][RELEASE_LABEL], "jellyfin");
        assert_eq!(d["spec"]["replicas"], 1);
        let pod = &d["spec"]["template"]["spec"];
        assert_eq!(pod["hostNetwork"], true);
        assert_eq!(pod["dnsPolicy"], "Default");
        assert_eq!(pod["automountServiceAccountToken"], false);
        assert_eq!(pod["tolerations"], json!([{ "operator": "Exists" }]));
        assert!(pod.get("hostPID").is_none());
        assert!(pod.get("nodeSelector").is_none());
        assert_eq!(pod["securityContext"]["runAsUser"], 1000);
        assert!(pod["securityContext"].get("sysctls").is_none());
        let c = &pod["containers"][0];
        assert_eq!(c["securityContext"]["privileged"], false);
        assert_eq!(c["securityContext"]["runAsUser"], 1000);
        assert_eq!(
            c["securityContext"]["capabilities"],
            json!({ "drop": ["ALL"] })
        );
        assert!(c["ports"][0].get("hostPort").is_none());
        assert!(c["volumeMounts"][0].get("mountPropagation").is_none());
        assert_eq!(
            pod["volumes"][0],
            json!({ "name": "config", "hostPath": {
                "path": "/home/notshared/data/apps/jellyfin/jellyfin-config", "type": "Directory" } })
        );
        assert_eq!(
            pod["volumes"][1],
            json!({ "name": "cache", "emptyDir": {} })
        );
        assert_eq!(
            shaped.dirs,
            vec!["/home/notshared/data/apps/jellyfin/jellyfin-config"]
        );
        assert_eq!(shaped.primary, Some(8096));
    }

    #[test]
    fn shared_puts_the_files_in_the_shared_domain_as_uid_1001() {
        let c = ShapeCtx {
            run_as: RunAs::Shared,
            ..ctx()
        };
        let shaped = shape(vec![deployment()], &c).unwrap();
        let pod = &shaped.docs[0]["spec"]["template"]["spec"];
        assert_eq!(pod["securityContext"]["runAsUser"], 1001);
        assert_eq!(pod["containers"][0]["securityContext"]["runAsGroup"], 1001);
        assert_eq!(
            shaped.dirs,
            vec!["/home/shared/data/apps/jellyfin/jellyfin-config"]
        );
    }

    #[test]
    fn a_host_path_refuses_the_install_and_says_which_object_asked() {
        let mut d = deployment();
        d["spec"]["template"]["spec"]["volumes"][1] =
            json!({ "name": "media", "hostPath": { "path": "/" } });
        let e = shape(vec![d], &ctx()).unwrap_err();
        assert!(e.0.contains("hostPath"), "{e}");
        assert!(e.0.contains("Deployment jellyfin"), "{e}");
    }

    #[test]
    fn cluster_wide_objects_refuse_and_rbac_is_dropped() {
        let role = json!({ "apiVersion": "rbac.authorization.k8s.io/v1", "kind": "ClusterRoleBinding",
            "metadata": { "name": "admin" } });
        let shaped = shape(vec![role, deployment()], &ctx()).unwrap();
        assert_eq!(shaped.docs.len(), 1);

        for (api, kind) in [
            ("v1", "PersistentVolume"),
            ("apiextensions.k8s.io/v1", "CustomResourceDefinition"),
            (
                "admissionregistration.k8s.io/v1",
                "MutatingWebhookConfiguration",
            ),
            ("v1", "Namespace"),
            ("example.com/v1", "Deployment"),
        ] {
            let doc = json!({ "apiVersion": api, "kind": kind, "metadata": { "name": "x" } });
            let e = shape(vec![doc], &ctx()).unwrap_err();
            assert!(e.0.contains(kind), "{kind}: {e}");
        }
    }

    #[test]
    fn claims_are_dropped_and_test_hooks_never_reach_the_cluster() {
        let pvc = json!({ "apiVersion": "v1", "kind": "PersistentVolumeClaim", "metadata": { "name": "c" } });
        let test = json!({ "apiVersion": "v1", "kind": "Pod",
            "metadata": { "name": "t", "annotations": { "helm.sh/hook": "test" } },
            "spec": { "containers": [{ "name": "t", "image": "busybox" }] } });
        let shaped = shape(vec![pvc, test, deployment()], &ctx()).unwrap();
        assert_eq!(shaped.docs.len(), 1);
        assert_eq!(shaped.docs[0]["kind"], "Deployment");
    }

    #[test]
    fn a_stateful_sets_claim_templates_become_directories_it_mounts() {
        let sts = json!({ "apiVersion": "apps/v1", "kind": "StatefulSet", "metadata": { "name": "db" },
            "spec": {
                "volumeClaimTemplates": [{ "metadata": { "name": "data" } }],
                "template": { "spec": { "containers": [{ "name": "db", "image": "x",
                    "ports": [{ "containerPort": 5433 }],
                    "volumeMounts": [{ "name": "data", "mountPath": "/data" }] }] } },
            } });
        let shaped = shape(vec![sts], &ctx()).unwrap();
        let spec = &shaped.docs[0]["spec"];
        assert!(spec.get("volumeClaimTemplates").is_none());
        assert_eq!(
            spec["template"]["spec"]["volumes"][0]["hostPath"]["path"],
            "/home/notshared/data/apps/jellyfin/data"
        );
        assert_eq!(shaped.primary, Some(5433));
    }

    #[test]
    fn a_load_balancer_becomes_a_cluster_ip_and_its_port_still_counts() {
        let svc = json!({ "apiVersion": "v1", "kind": "Service", "metadata": { "name": "s" },
            "spec": { "type": "LoadBalancer", "externalIPs": ["1.2.3.4"],
                      "ports": [{ "port": 80, "targetPort": 8096, "nodePort": 30001 }] } });
        let shaped = shape(vec![svc], &ctx()).unwrap();
        let spec = &shaped.docs[0]["spec"];
        assert_eq!(spec["type"], "ClusterIP");
        assert!(spec.get("externalIPs").is_none());
        assert!(spec["ports"][0].get("nodePort").is_none());
        assert_eq!(shaped.primary, Some(8096));
    }

    #[test]
    fn ports_the_box_or_another_app_holds_refuse_with_the_port_named() {
        let mut d = deployment();
        d["spec"]["template"]["spec"]["containers"][0]["ports"][0]["containerPort"] = json!(8082);
        assert!(shape(vec![d.clone()], &ctx())
            .unwrap_err()
            .0
            .contains("8082"));

        d["spec"]["template"]["spec"]["containers"][0]["ports"][0]["containerPort"] = json!(80);
        assert!(shape(vec![d.clone()], &ctx())
            .unwrap_err()
            .0
            .contains("without root"));

        let c = ShapeCtx {
            taken: vec![(8096, "emby".into())],
            ..ctx()
        };
        assert!(shape(vec![deployment()], &c)
            .unwrap_err()
            .0
            .contains("emby"));
        // Its own port, from an earlier install of the same app, is no conflict.
        let c = ShapeCtx {
            taken: vec![(8096, "jellyfin".into())],
            ..ctx()
        };
        assert!(shape(vec![deployment()], &c).is_ok());
    }

    #[test]
    fn a_claim_name_cannot_climb_out_of_the_apps_directory() {
        let mut d = deployment();
        d["spec"]["template"]["spec"]["volumes"][0]["persistentVolumeClaim"]["claimName"] =
            json!("../../../etc");
        assert!(shape(vec![d], &ctx()).is_err());
    }

    #[test]
    fn projected_tokens_are_taken_out_of_the_pod() {
        let mut d = deployment();
        d["spec"]["template"]["spec"]["volumes"][1] = json!({ "name": "p", "projected": { "sources": [
            { "serviceAccountToken": { "path": "token" } }, { "configMap": { "name": "c" } } ] } });
        let shaped = shape(vec![d], &ctx()).unwrap();
        assert_eq!(
            shaped.docs[0]["spec"]["template"]["spec"]["volumes"][1]["projected"]["sources"],
            json!([{ "configMap": { "name": "c" } }])
        );
    }

    #[test]
    fn the_rendered_output_is_one_json_document_per_object() {
        let out = render(&[json!({ "a": 1 }), json!({ "b": 2 })]);
        assert_eq!(out, "---\n{\"a\":1}\n---\n{\"b\":2}\n");
    }

    #[test]
    fn chart_references_that_could_become_flags_or_other_hosts_are_refused() {
        let ok = ChartRef {
            repo: "https://utkuozdemir.org/helm-charts".into(),
            name: "jellyfin".into(),
            version: "2.0.0".into(),
        };
        assert!(ok.validate().is_ok());
        assert_eq!(
            ok.helm_ref(),
            (
                "jellyfin".into(),
                Some("https://utkuozdemir.org/helm-charts".into())
            )
        );
        let oci = ChartRef {
            repo: "oci://ghcr.io/x/charts/".into(),
            ..ok.clone()
        };
        assert_eq!(
            oci.helm_ref(),
            ("oci://ghcr.io/x/charts/jellyfin".into(), None)
        );
        for bad in [
            ChartRef {
                repo: "http://example.org".into(),
                ..ok.clone()
            },
            ChartRef {
                repo: "file:///etc".into(),
                ..ok.clone()
            },
            ChartRef {
                repo: "https://a@b.example".into(),
                ..ok.clone()
            },
            ChartRef {
                name: "--kubeconfig".into(),
                ..ok.clone()
            },
            ChartRef {
                name: "Jelly Fin".into(),
                ..ok.clone()
            },
            ChartRef {
                version: "-1".into(),
                ..ok.clone()
            },
            ChartRef {
                version: ">=1.0".into(),
                ..ok.clone()
            },
        ] {
            assert!(bad.validate().is_err(), "{bad:?}");
        }
    }

    #[test]
    fn release_names_are_dns_labels_and_the_default_comes_from_the_chart() {
        assert!(validate_release("jellyfin").is_ok());
        assert!(validate_release("my-app-2").is_ok());
        for bad in [
            "",
            "2app",
            "App",
            "app-",
            "a_b",
            &"a".repeat(MAX_RELEASE_CHARS + 1),
        ] {
            assert!(validate_release(bad).is_err(), "{bad}");
        }
        assert_eq!(default_release("jellyfin"), "jellyfin");
        assert_eq!(default_release("Home_Assistant.2"), "home-assistant-2");
        assert_eq!(default_release("123"), "app");
    }

    #[test]
    fn front_ports_are_kept_on_update_and_allocated_lowest_first() {
        let rec = |release: &str, front_port| Record {
            release: release.into(),
            chart: ChartRef {
                repo: "https://x.example".into(),
                name: "x".into(),
                version: "1".into(),
            },
            run_as: RunAs::Notshared,
            values: json!({}),
            values_yaml: None,
            front_port,
            app_port: None,
            phase: Phase::Running,
            message: None,
            updated_at: 0,
        };
        let records = vec![rec("a", 30000), rec("b", 30002)];
        assert_eq!(allocate_port((30000, 30002), &records, "b"), Some(30002));
        assert_eq!(allocate_port((30000, 30002), &records, "c"), Some(30001));
        let full = vec![rec("a", 30000), rec("b", 30001)];
        assert_eq!(allocate_port((30000, 30001), &full, "c"), None);
        assert_eq!(parse_port_range("30000-30099"), Some((30000, 30099)));
        assert_eq!(parse_port_range("80-90"), None);
    }

    #[test]
    fn an_install_request_is_read_with_every_refusal_a_sentence() {
        let body = json!({
            "release": "jellyfin",
            "runAs": "notshared",
            "chart": { "repo": "https://utkuozdemir.org/helm-charts", "name": "jellyfin", "version": "2.0.0" },
            "values": { "service": { "port": 8097 } },
        });
        let req = InstallRequest::parse(&body).unwrap();
        assert_eq!(req.run_as, RunAs::Notshared);
        assert_eq!(req.values["service"]["port"], 8097);
        assert_eq!(req.values_yaml, None);

        let mut no_user = body.clone();
        no_user["runAs"] = json!("root");
        assert!(InstallRequest::parse(&no_user).is_err());
        let mut list = body.clone();
        list["values"] = json!([1, 2]);
        assert!(InstallRequest::parse(&list).is_err());
    }
    // ── The commands, against the fake ──────────────────────────────────

    fn install_body(run_as: &str) -> Value {
        json!({
            "release": "jellyfin",
            "runAs": run_as,
            "chart": { "repo": "https://utkuozdemir.org/helm-charts", "name": "jellyfin", "version": "2.0.0" },
            "values": {},
        })
    }

    #[test]
    fn an_install_writes_the_record_then_starts_the_job() {
        let mut f = crate::fake::FakeLosos::default();
        let req = InstallRequest::parse(&install_body("notshared")).unwrap();
        let out = cmd_app_install(&mut f, &req).unwrap();
        assert_eq!(out["phase"], "installing");
        assert_eq!(out["frontPort"], 30000);
        assert_eq!(f.app_records.len(), 1);
        assert_eq!(f.app_jobs, vec![("jellyfin".to_string(), Action::Install)]);

        // A second install of the same app while its job runs is refused.
        let e = cmd_app_install(&mut f, &req).unwrap_err();
        assert!(e.downcast_ref::<Conflict>().is_some(), "{e}");

        // Once the job is done, the same request changes it in place and
        // keeps its port.
        f.app_jobs_active.clear();
        let out = cmd_app_install(&mut f, &req).unwrap();
        assert_eq!(out["frontPort"], 30000);
        assert_eq!(f.app_records.len(), 1);
    }

    #[test]
    fn shared_is_refused_while_the_box_keeps_its_disk_to_itself() {
        let mut f = crate::fake::FakeLosos::default();
        f.state.sharing = false;
        let req = InstallRequest::parse(&install_body("shared")).unwrap();
        let e = cmd_app_install(&mut f, &req).unwrap_err();
        assert!(e.downcast_ref::<Conflict>().is_some(), "{e}");
        assert!(f.app_jobs.is_empty());

        f.state.sharing = true;
        assert!(cmd_app_install(&mut f, &req).is_ok());
    }

    #[test]
    fn a_box_with_no_cluster_says_so_and_installs_nothing() {
        let mut f = crate::fake::FakeLosos {
            apps: None,
            ..Default::default()
        };
        assert_eq!(
            cmd_apps(&mut f).unwrap(),
            json!({ "available": false, "reason": "noCluster", "apps": [] })
        );
        let req = InstallRequest::parse(&install_body("notshared")).unwrap();
        assert!(cmd_app_install(&mut f, &req).is_err());
        assert!(f.app_jobs.is_empty());
    }

    #[test]
    fn a_busy_record_whose_job_is_gone_reads_as_failed() {
        let mut f = crate::fake::FakeLosos::default();
        let req = InstallRequest::parse(&install_body("notshared")).unwrap();
        cmd_app_install(&mut f, &req).unwrap();
        assert_eq!(cmd_apps(&mut f).unwrap()["apps"][0]["phase"], "installing");
        f.app_jobs_active.clear();
        let apps = cmd_apps(&mut f).unwrap();
        assert_eq!(apps["apps"][0]["phase"], "failed");
        assert!(apps["apps"][0]["message"].is_string());
    }

    #[test]
    fn another_chart_under_the_same_name_is_refused() {
        let mut f = crate::fake::FakeLosos::default();
        let req = InstallRequest::parse(&install_body("notshared")).unwrap();
        cmd_app_install(&mut f, &req).unwrap();
        f.app_jobs_active.clear();
        let mut other = req.clone();
        other.chart.name = "emby".into();
        let e = cmd_app_install(&mut f, &other).unwrap_err();
        assert!(e.to_string().contains("Another app"), "{e}");
    }

    #[test]
    fn removing_marks_the_record_and_starts_the_job() {
        let mut f = crate::fake::FakeLosos::default();
        let req = InstallRequest::parse(&install_body("notshared")).unwrap();
        cmd_app_install(&mut f, &req).unwrap();
        assert!(
            cmd_app_remove(&mut f, "jellyfin").is_err(),
            "still installing"
        );
        f.app_jobs_active.clear();
        let out = cmd_app_remove(&mut f, "jellyfin").unwrap();
        assert_eq!(out["phase"], "removing");
        assert_eq!(
            f.app_jobs.last(),
            Some(&("jellyfin".to_string(), Action::Remove))
        );
        assert!(cmd_app_remove(&mut f, "nothing").is_err());
    }
}
