//! Running an app as `notshared` on Podman: a Compose project per app,
//! rootless, as that user.
//!
//! # Which runtime an app gets
//!
//! The user an app runs as picks it. A `notshared` app is a Podman Compose
//! project started by `notshared` itself (`modules/podman-apps.nix`), so a
//! box that lends nothing to the mesh needs no cluster for its apps at all.
//! A `shared` app stays a Helm release in the box's own cluster
//! ([`crate::apps`]), because that side of the box is the one the mesh
//! touches.
//!
//! # Two ways in, one project out
//!
//!   * [`shape_compose`] takes a Compose file, the LosOS app format
//!     (`handbook/docs/in-depth/app-format.md`): what the AI builder writes,
//!     what the box's own apps ship as, what an owner pastes.
//!   * [`from_kube`] takes a Helm chart after [`crate::apps::shape`] has
//!     shaped it, and turns each pod's containers into services, its config
//!     maps and secrets into files, and its claims into the same folders.
//!
//! Either way the project follows the rules the cluster shaping keeps: every
//! service on the host network as the app's user (uid 1000 inside the
//! container too, through `userns_mode: keep-id`, so the files it writes
//! belong to `notshared`), no capabilities and no way to gain any, its data
//! in `/home/notshared/data/apps/<name>`, a port from 1024 up that the box
//! does not keep for itself. Anything that would reach past them (a folder
//! of the box, a device, privileges, a build) refuses with a sentence.
//!
//! Services reach each other at `127.0.0.1`, and by name: every service name
//! (and, for a chart, every Service name in the chart) is an `extra_hosts`
//! entry for `127.0.0.1`. The ports have to match, because nothing sits in
//! between: a chart Service on port 80 in front of a container on 8080 is
//! reached at 8080.
//!
//! # Shape
//!
//! Pure, like the cluster shaping: JSON in, a [`Project`] out, tested here.
//! `losos-app-shape` writes it to disk and the install job starts it.

use crate::apps::{check_port, get_path_mut, kept_kind, refuse, Refusal, RunAs, ShapeCtx, Shaped};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

/// Where every user's Podman state lives: its image store and one directory
/// per app. Under `/var`, so it is persisted, and outside the home, which is
/// tmpfs above `data/`.
pub const PODMAN_ROOT: &str = "/var/lib/losos-podman";

/// What the box writes into an app's project directory.
pub const COMPOSE_FILE: &str = "compose.json";

/// Image archives to load before the project starts, one path per line.
pub const IMAGES_FILE: &str = "images.txt";

/// The files an app ships, and the ones made from a chart's config maps and
/// secrets, under the project directory.
pub const FILES_DIR: &str = "files";

/// The most services one app may have.
pub const MAX_SERVICES: usize = 32;

/// The app's directory under [`PODMAN_ROOT`].
#[must_use]
pub fn project_dir(run_as: RunAs, release: &str) -> String {
    format!("{PODMAN_ROOT}/{}/projects/{release}", run_as.as_str())
}

/// The Compose project name, which prefixes every container's name.
#[must_use]
pub fn project_name(release: &str) -> String {
    format!("losos-{release}")
}

/// A file the box writes into `files/` before the app starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct File {
    /// Relative to the project's `files/`.
    pub path: String,
    pub bytes: Vec<u8>,
}

/// An app as Podman Compose will run it.
#[derive(Debug, Clone, PartialEq)]
pub struct Project {
    /// What goes into `compose.json`.
    pub compose: Value,
    /// Files made for it (a chart's config maps and secrets).
    pub files: Vec<File>,
    /// Folders in the user's data domain to make, owned by the user.
    pub dirs: Vec<String>,
    /// Folders inside the project to make, owned by the user (a pod's
    /// scratch space).
    pub scratch: Vec<String>,
    /// Every TCP port a service listens on.
    pub ports: Vec<u16>,
    /// The one the page links to.
    pub primary: Option<u16>,
    /// Image archives in the Nix store, loaded before the project starts.
    pub images: Vec<String>,
}

// ── Small parts ───────────────────────────────────────────────────────────

fn is_service_name(n: &str) -> bool {
    !n.is_empty()
        && n.len() <= 63
        && n.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
        && n.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
}

/// A named volume becomes a folder, so its name must be one.
fn is_volume_name(n: &str) -> bool {
    is_service_name(n) && !n.contains("..")
}

/// A path the app shipped, relative to its project, without `..`. Returns
/// it without the leading `./`.
fn relative(path: &str) -> Option<String> {
    let p = path.trim();
    let p = p.strip_prefix("./").unwrap_or(p);
    let p = p.trim_end_matches('/');
    if p.is_empty() || p == "." {
        return Some(String::new());
    }
    if p.starts_with('/') || p.starts_with('~') || p.contains('\0') {
        return None;
    }
    let ok = p
        .split('/')
        .all(|part| !part.is_empty() && part != "." && part != "..");
    ok.then(|| p.to_string())
}

/// `path` is something the app shipped: a file, or a folder above one.
fn shipped(path: &str, files: &[String]) -> bool {
    path.is_empty()
        || files
            .iter()
            .any(|f| f == path || f.starts_with(&format!("{path}/")))
}

fn as_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn port_of(v: &Value) -> Option<u16> {
    match v {
        Value::Number(n) => n.as_u64().and_then(|n| u16::try_from(n).ok()),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

/// The container port of a Compose `ports` entry: `8080`, `"80:8080"`,
/// `"127.0.0.1:80:8080/tcp"`, `{target: 8080}`. `None` for UDP and ranges.
fn compose_port(entry: &Value) -> Option<u16> {
    match entry {
        Value::Object(m) => {
            if m.get("protocol")
                .and_then(Value::as_str)
                .is_some_and(|p| p != "tcp")
            {
                return None;
            }
            m.get("target").and_then(port_of)
        }
        Value::Number(_) => port_of(entry),
        Value::String(s) => {
            let (spec, proto) = s.split_once('/').unwrap_or((s, "tcp"));
            if proto != "tcp" {
                return None;
            }
            spec.rsplit(':').next()?.trim().parse().ok()
        }
        _ => None,
    }
}

/// The settings every service gets, whatever it asked for.
fn harden(service: &mut Map<String, Value>, uid: u32, hosts: &[String]) {
    service.insert("network_mode".into(), json!("host"));
    service.insert("user".into(), json!(format!("{uid}:{uid}")));
    service.insert("userns_mode".into(), json!("keep-id"));
    service.insert("cap_drop".into(), json!(["ALL"]));
    service.insert("security_opt".into(), json!(["no-new-privileges"]));
    if !hosts.is_empty() {
        let entries: Vec<String> = hosts.iter().map(|h| format!("{h}:127.0.0.1")).collect();
        service.insert("extra_hosts".into(), json!(entries));
    }
}

fn finish(
    services: Map<String, Value>,
    ports: Vec<(u16, Option<String>)>,
    wanted: Option<u16>,
    ctx: &ShapeCtx,
) -> Result<(Value, Vec<u16>, Option<u16>), Refusal> {
    let mut all: Vec<u16> = ports.iter().map(|(p, _)| *p).collect();
    if let Some(w) = wanted {
        all.push(w);
    }
    all.sort_unstable();
    all.dedup();
    for port in &all {
        check_port(*port, ctx)?;
    }
    let named_web = |n: &Option<String>| {
        n.as_deref()
            .is_some_and(|n| ["http", "web", "ui", "https"].iter().any(|w| n.contains(w)))
    };
    let primary = wanted.or_else(|| {
        ports
            .iter()
            .find(|(_, n)| named_web(n))
            .or_else(|| ports.first())
            .map(|(p, _)| *p)
    });
    let compose = json!({
        "name": project_name(&ctx.release),
        "services": services,
        // One pod per project is podman-compose's default, and a pod cannot
        // take `--userns=keep-id` or join the host's network.
        "x-podman": { "in_pod": false },
    });
    Ok((compose, all, primary))
}

// ── A Compose file ────────────────────────────────────────────────────────

/// Service keys passed through as they are.
const KEPT: &[&str] = &[
    "image",
    "command",
    "entrypoint",
    "working_dir",
    "read_only",
    "depends_on",
    "healthcheck",
    "stop_signal",
    "stop_grace_period",
    "init",
    "labels",
    "pull_policy",
    "platform",
    "shm_size",
    "stdin_open",
    "tty",
];

/// Service keys dropped without a word: they mean nothing on the host
/// network of one box, or the box sets them itself.
const DROPPED: &[&str] = &[
    "container_name",
    "hostname",
    "domainname",
    "networks",
    "deploy",
    "logging",
    "mem_limit",
    "memswap_limit",
    "mem_reservation",
    "cpus",
    "cpu_shares",
    "cpu_quota",
    "cpuset",
    "ulimits",
    "user",
    "scale",
    "profiles",
    "dns",
    "dns_search",
    "mac_address",
];

/// Shape a Compose file to run on this box as `ctx.run_as`. `shipped` is
/// every file the app came with, relative to its folder.
pub fn shape_compose(
    doc: Value,
    ctx: &ShapeCtx,
    shipped_files: &[String],
) -> Result<Project, Refusal> {
    let Value::Object(mut top) = doc else {
        return refuse("The app's compose file is not a map of settings.");
    };
    let uid = ctx.run_as.uid();
    let root = ctx.run_as.data_root(&ctx.release);
    let files_root = format!("{}/{FILES_DIR}", project_dir(ctx.run_as, &ctx.release));

    let meta = top.remove("x-losos").unwrap_or(Value::Null);
    let wanted = meta
        .get("port")
        .map(|p| port_of(p).ok_or_else(|| Refusal("x-losos.port must be a port number.".into())));
    let wanted = wanted.transpose()?;
    let mut images = Vec::new();
    if let Some(list) = meta.get("images") {
        let Some(list) = list.as_array() else {
            return refuse("x-losos.images must be a list of image archives.");
        };
        for archive in list {
            let Some(a) = archive.as_str() else {
                return refuse("x-losos.images must be a list of image archives.");
            };
            if !a.starts_with("/nix/store/") || a.contains("..") || a.contains('\n') {
                return refuse(format!(
                    "The image archive {a} is not in the Nix store, which is the only place \
                     this box loads one from."
                ));
            }
            images.push(a.to_string());
        }
    }

    for key in top.keys() {
        match key.as_str() {
            "services" | "volumes" | "networks" | "name" | "version" | "secrets" | "configs" => {}
            k if k.starts_with("x-") => {}
            k => {
                return refuse(format!(
                    "The compose file asks for {k}, which apps on this box cannot have."
                ))
            }
        }
    }

    // Named volumes: a folder each. Anything with a driver or outside the
    // file cannot be made a folder.
    let mut named: BTreeSet<String> = BTreeSet::new();
    if let Some(Value::Object(volumes)) = top.get("volumes") {
        for (name, def) in volumes {
            if !is_volume_name(name) {
                return refuse(format!("The compose file names a strange volume ({name})."));
            }
            if let Value::Object(def) = def {
                if def.get("external").and_then(Value::as_bool) == Some(true)
                    || def.get("driver").is_some()
                    || def.get("driver_opts").is_some()
                {
                    return refuse(format!(
                        "The volume {name} asks for a driver or for something outside the app, \
                         which apps on this box cannot have."
                    ));
                }
            }
            named.insert(name.clone());
        }
    }

    // Secrets and configs: files the app shipped, mounted read-only.
    let mut shared_files: BTreeMap<(String, String), String> = BTreeMap::new();
    for kind in ["secrets", "configs"] {
        let Some(Value::Object(defs)) = top.get(kind) else {
            continue;
        };
        for (name, def) in defs {
            let file = def.get("file").and_then(Value::as_str).and_then(relative);
            match file {
                Some(f) if !f.is_empty() && shipped(&f, shipped_files) => {
                    shared_files.insert(
                        (kind.to_string(), name.clone()),
                        format!("{files_root}/{f}"),
                    );
                }
                _ => {
                    return refuse(format!(
                        "The {} {name} must be a file the app ships beside its compose file.",
                        &kind[..kind.len() - 1]
                    ))
                }
            }
        }
    }

    let Some(Value::Object(services)) = top.remove("services") else {
        return refuse("The compose file has no services.");
    };
    if services.is_empty() {
        return refuse("The compose file has no services.");
    }
    if services.len() > MAX_SERVICES {
        return refuse(format!(
            "An app here can have at most {MAX_SERVICES} services."
        ));
    }
    let names: Vec<String> = services.keys().cloned().collect();
    for n in &names {
        if !is_service_name(n) {
            return refuse(format!("The compose file names a strange service ({n})."));
        }
    }

    let mut dirs = Vec::new();
    let mut ports: Vec<(u16, Option<String>)> = Vec::new();
    let mut out = Map::new();
    for (name, svc) in services {
        let Value::Object(svc) = svc else {
            return refuse(format!("The service {name} is not a map of settings."));
        };
        let mut shaped = Map::new();
        let mut file_mounts = Vec::new();
        for (key, value) in svc {
            let k = key.as_str();
            if KEPT.contains(&k) {
                shaped.insert(key, value);
                continue;
            }
            if DROPPED.contains(&k) || k.starts_with("x-") {
                continue;
            }
            match k {
                "network_mode" if value.as_str() == Some("host") => {}
                "restart" => {
                    let ok = value.as_str().is_some_and(|r| {
                        matches!(r, "no" | "always" | "on-failure" | "unless-stopped")
                    });
                    if !ok {
                        return refuse(format!("The service {name} has a strange restart policy."));
                    }
                    shaped.insert(key, value);
                }
                "ports" | "expose" => {
                    let Value::Array(list) = value else {
                        return refuse(format!("The service {name} lists its {k} strangely."));
                    };
                    for entry in &list {
                        if let Some(p) = compose_port(entry) {
                            ports.push((p, Some(name.clone())));
                        }
                    }
                }
                "environment" => {
                    shaped.insert(key, environment(&name, value)?);
                }
                "env_file" => {
                    let list = match value {
                        Value::Array(l) => l,
                        one => vec![one],
                    };
                    let mut paths = Vec::new();
                    for entry in list {
                        let raw = match &entry {
                            Value::String(s) => Some(s.as_str()),
                            Value::Object(m) => m.get("path").and_then(Value::as_str),
                            _ => None,
                        };
                        match raw.and_then(relative) {
                            Some(f) if !f.is_empty() && shipped(&f, shipped_files) => {
                                paths.push(json!(format!("{files_root}/{f}")));
                            }
                            _ => {
                                return refuse(format!(
                                    "The service {name} reads settings from a file the app does not ship."
                                ))
                            }
                        }
                    }
                    shaped.insert(key, Value::Array(paths));
                }
                "tmpfs" => {
                    shaped.insert(key, value);
                }
                "volumes" => {
                    let Value::Array(list) = value else {
                        return refuse(format!("The service {name} lists its volumes strangely."));
                    };
                    let mut mounts = Vec::new();
                    for entry in list {
                        mounts.push(compose_mount(
                            &name,
                            &entry,
                            &named,
                            &root,
                            &files_root,
                            shipped_files,
                            &mut dirs,
                        )?);
                    }
                    shaped.insert(key, Value::Array(mounts));
                }
                "secrets" | "configs" => {
                    let Value::Array(list) = value else {
                        return refuse(format!("The service {name} lists its {k} strangely."));
                    };
                    for entry in list {
                        let (source, target) = match &entry {
                            Value::String(s) => (s.clone(), None),
                            Value::Object(m) => (
                                m.get("source")
                                    .and_then(Value::as_str)
                                    .unwrap_or("")
                                    .to_string(),
                                m.get("target").and_then(Value::as_str).map(str::to_string),
                            ),
                            _ => (String::new(), None),
                        };
                        let Some(path) = shared_files.get(&(k.to_string(), source.clone())) else {
                            return refuse(format!(
                                "The service {name} asks for the {} {source}, which the compose file does not define.",
                                &k[..k.len() - 1]
                            ));
                        };
                        let target = target.unwrap_or_else(|| {
                            if k == "secrets" {
                                format!("/run/secrets/{source}")
                            } else {
                                format!("/{source}")
                            }
                        });
                        let target = if target.starts_with('/') {
                            target
                        } else {
                            format!("/run/secrets/{target}")
                        };
                        file_mounts.push(json!({
                            "type": "bind", "source": path, "target": target, "read_only": true,
                        }));
                    }
                }
                _ => {
                    return refuse(format!(
                        "The service {name} asks for {k}, which apps on this box cannot have."
                    ))
                }
            }
        }
        if !file_mounts.is_empty() {
            let mounts = shaped.entry("volumes").or_insert_with(|| json!([]));
            if let Value::Array(m) = mounts {
                m.extend(file_mounts);
            }
        }
        if !shaped.get("image").is_some_and(Value::is_string) {
            return refuse(format!(
                "The service {name} names no image. Apps here are not built on the box."
            ));
        }
        shaped
            .entry("restart")
            .or_insert_with(|| json!("unless-stopped"));
        harden(&mut shaped, uid, &names);
        out.insert(name, Value::Object(shaped));
    }

    let (compose, all, primary) = finish(out, ports, wanted, ctx)?;
    dirs.sort();
    dirs.dedup();
    Ok(Project {
        compose,
        files: Vec::new(),
        dirs,
        scratch: Vec::new(),
        ports: all,
        primary,
        images,
    })
}

fn environment(service: &str, value: Value) -> Result<Value, Refusal> {
    let mut env = Map::new();
    match value {
        Value::Object(m) => {
            for (k, v) in m {
                if v.is_null() {
                    continue;
                }
                let Some(v) = as_string(&v) else {
                    return refuse(format!(
                        "The service {service} sets {k} to something that is not text."
                    ));
                };
                env.insert(k, json!(v));
            }
        }
        Value::Array(list) => {
            for entry in list {
                let Some(line) = entry.as_str() else {
                    return refuse(format!(
                        "The service {service} lists its environment strangely."
                    ));
                };
                // `KEY` alone takes the value from the shell that starts
                // the app, and that shell has nothing for it.
                if let Some((k, v)) = line.split_once('=') {
                    env.insert(k.to_string(), json!(v));
                }
            }
        }
        _ => {
            return refuse(format!(
                "The service {service} lists its environment strangely."
            ))
        }
    }
    Ok(Value::Object(env))
}

/// One `volumes` entry of a service, as a long-form mount the box allows.
fn compose_mount(
    service: &str,
    entry: &Value,
    named: &BTreeSet<String>,
    root: &str,
    files_root: &str,
    shipped_files: &[String],
    dirs: &mut Vec<String>,
) -> Result<Value, Refusal> {
    let (kind, source, target, read_only) = match entry {
        Value::String(s) => {
            let parts: Vec<&str> = s.split(':').collect();
            match parts.as_slice() {
                [target] => (
                    "volume".to_string(),
                    String::new(),
                    (*target).to_string(),
                    false,
                ),
                [source, target] => (
                    String::new(),
                    (*source).to_string(),
                    (*target).to_string(),
                    false,
                ),
                [source, target, mode] => (
                    String::new(),
                    (*source).to_string(),
                    (*target).to_string(),
                    mode.split(',').any(|m| m == "ro"),
                ),
                _ => return refuse(format!("The service {service} has a strange volume ({s}).")),
            }
        }
        Value::Object(m) => (
            m.get("type")
                .and_then(Value::as_str)
                .unwrap_or("volume")
                .to_string(),
            m.get("source")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            m.get("target")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            m.get("read_only").and_then(Value::as_bool).unwrap_or(false),
        ),
        _ => {
            return refuse(format!(
                "The service {service} lists its volumes strangely."
            ))
        }
    };
    if !target.starts_with('/') {
        return refuse(format!(
            "The service {service} mounts a volume at a strange place ({target})."
        ));
    }
    if kind == "tmpfs" {
        return Ok(json!({ "type": "tmpfs", "target": target }));
    }
    // An anonymous volume lives in the user's image store and goes with
    // the container.
    if source.is_empty() {
        if kind == "bind" {
            return refuse(format!("The service {service} binds nothing at {target}."));
        }
        return Ok(json!({ "type": "volume", "target": target }));
    }
    let is_path = source.starts_with('.') || source.starts_with('/') || source.starts_with('~');
    if kind == "bind" || (kind.is_empty() && is_path) {
        let Some(rel) = relative(&source) else {
            return refuse(format!(
                "The service {service} asks for the box's own folder {source}, which apps on \
                 this box cannot have. Use a named volume for its data."
            ));
        };
        if !shipped(&rel, shipped_files) {
            return refuse(format!(
                "The service {service} mounts {source}, which the app does not ship."
            ));
        }
        let path = if rel.is_empty() {
            files_root.to_string()
        } else {
            format!("{files_root}/{rel}")
        };
        return Ok(
            json!({ "type": "bind", "source": path, "target": target, "read_only": read_only }),
        );
    }
    if kind == "volume" || kind.is_empty() {
        if !named.contains(&source) {
            return refuse(format!(
                "The service {service} mounts the volume {source}, which the compose file does not define."
            ));
        }
        let path = format!("{root}/{source}");
        dirs.push(path.clone());
        return Ok(
            json!({ "type": "bind", "source": path, "target": target, "read_only": read_only }),
        );
    }
    refuse(format!(
        "The service {service} asks for a {kind} volume, which apps on this box cannot have."
    ))
}

// ── A shaped chart ────────────────────────────────────────────────────────

/// Standard base64 with or without padding, which is how a chart's secrets
/// carry their values. No crate: this is the one place that reads it.
fn base64_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut acc: u32 = 0;
    let mut bits = 0;
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' | b'\n' | b'\r' | b' ' => continue,
            _ => return None,
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(u8::try_from((acc >> bits) & 0xff).expect("masked"));
        }
    }
    Some(out)
}

/// A config map's or a secret's keys, as bytes.
fn object_data(doc: &Value) -> BTreeMap<String, Vec<u8>> {
    let mut data = BTreeMap::new();
    let secret = doc.get("kind").and_then(Value::as_str) == Some("Secret");
    if let Some(Value::Object(m)) = doc.get("data") {
        for (k, v) in m {
            let Some(s) = v.as_str() else { continue };
            let bytes = if secret {
                base64_decode(s).unwrap_or_default()
            } else {
                s.as_bytes().to_vec()
            };
            data.insert(k.clone(), bytes);
        }
    }
    if let Some(Value::Object(m)) = doc.get("stringData") {
        for (k, v) in m {
            if let Some(s) = as_string(v) {
                data.insert(k.clone(), s.into_bytes());
            }
        }
    }
    if let Some(Value::Object(m)) = doc.get("binaryData") {
        for (k, v) in m {
            if let Some(b) = v.as_str().and_then(base64_decode) {
                data.insert(k.clone(), b);
            }
        }
    }
    data
}

/// What a pod volume becomes.
#[derive(Debug, Clone)]
enum Vol {
    /// A folder the shaping made of a claim, or the pod's scratch space.
    Dir(String),
    /// Files written under the project's `files/`, at this relative folder.
    Files(String),
    /// Memory-backed scratch space.
    Tmpfs,
}

/// Replace Kubernetes' `$(NAME)` references with the container's own
/// variables, as the kubelet does; `$$(NAME)` stays literal.
fn expand(s: &str, env: &Map<String, Value>) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(at) = rest.find("$(") {
        if rest[..at].ends_with('$') {
            out.push_str(&rest[..at - 1]);
            out.push_str("$(");
            rest = &rest[at + 2..];
            continue;
        }
        out.push_str(&rest[..at]);
        let after = &rest[at + 2..];
        match after.find(')') {
            Some(end) => {
                let name = &after[..end];
                match env.get(name).and_then(Value::as_str) {
                    Some(v) => out.push_str(v),
                    None => {
                        out.push_str("$(");
                        out.push_str(name);
                        out.push(')');
                    }
                }
                rest = &after[end + 1..];
            }
            None => {
                out.push_str(&rest[at..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

fn service_name(workload: &str, container: &str) -> String {
    let raw = if workload == container {
        workload.to_string()
    } else {
        format!("{workload}-{container}")
    };
    let mut s: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    s.truncate(63);
    s
}

/// Turn a chart [`crate::apps::shape`] already shaped into a Compose
/// project. The shaping made the rules hold for a pod; this keeps them for
/// a container on its own.
pub fn from_kube(shaped: Shaped, ctx: &ShapeCtx) -> Result<Project, Refusal> {
    let uid = ctx.run_as.uid();
    let project = project_dir(ctx.run_as, &ctx.release);
    let namespace = ctx.namespace();

    let mut maps: BTreeMap<(String, String), BTreeMap<String, Vec<u8>>> = BTreeMap::new();
    let mut hosts: BTreeSet<String> = BTreeSet::new();
    for doc in &shaped.docs {
        let kind = doc.get("kind").and_then(Value::as_str).unwrap_or("");
        let name = doc
            .pointer("/metadata/name")
            .and_then(Value::as_str)
            .unwrap_or("");
        match kind {
            "ConfigMap" | "Secret" => {
                maps.insert((kind.to_string(), name.to_string()), object_data(doc));
            }
            "Service" => {
                for h in [
                    name.to_string(),
                    format!("{name}.{namespace}"),
                    format!("{name}.{namespace}.svc"),
                    format!("{name}.{namespace}.svc.cluster.local"),
                ] {
                    hosts.insert(h);
                }
            }
            _ => {}
        }
    }

    let mut files: Vec<File> = Vec::new();
    let mut scratch: Vec<String> = Vec::new();
    let mut services = Map::new();

    for doc in &shaped.docs {
        let kind = doc
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let api = doc
            .get("apiVersion")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let workload = doc
            .pointer("/metadata/name")
            .and_then(Value::as_str)
            .unwrap_or("app")
            .to_string();
        if kind == "CronJob" {
            return refuse(format!(
                "This app asks for a CronJob ({workload}), which apps run as notshared cannot have. \
                 Run it as shared, or look for a property that turns it off."
            ));
        }
        let Some(path) = kept_kind(&api, &kind) else {
            continue;
        };
        if path.is_empty() {
            continue;
        }
        let mut doc = doc.clone();
        let Some(pod) = get_path_mut(&mut doc, path) else {
            continue;
        };
        let pod = pod.clone();
        let job = kind == "Job";
        if kind == "StatefulSet" {
            if let Some(svc) = doc.pointer("/spec/serviceName").and_then(Value::as_str) {
                hosts.insert(format!("{workload}-0.{svc}"));
                hosts.insert(format!("{workload}-0.{svc}.{namespace}.svc.cluster.local"));
            }
        }

        // The pod's volumes.
        let mut vols: BTreeMap<String, Vol> = BTreeMap::new();
        for v in pod
            .get("volumes")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let vname = v
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            if let Some(p) = v.pointer("/hostPath/path").and_then(Value::as_str) {
                vols.insert(vname, Vol::Dir(p.to_string()));
            } else if let Some(e) = v.get("emptyDir") {
                if e.get("medium").and_then(Value::as_str) == Some("Memory") {
                    vols.insert(vname, Vol::Tmpfs);
                } else {
                    let dir = format!(
                        "{project}/scratch/{}/{vname}",
                        service_name(&workload, "pod")
                    );
                    scratch.push(dir.clone());
                    vols.insert(vname, Vol::Dir(dir));
                }
            } else {
                let rel = format!("kube/{}/{vname}", service_name(&workload, "pod"));
                let mut sources = Vec::new();
                if let Some(c) = v.get("configMap") {
                    sources.push(("ConfigMap", c.clone()));
                }
                if let Some(s) = v.get("secret") {
                    sources.push(("Secret", s.clone()));
                }
                for s in v
                    .pointer("/projected/sources")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    if let Some(c) = s.get("configMap") {
                        sources.push(("ConfigMap", c.clone()));
                    }
                    if let Some(c) = s.get("secret") {
                        sources.push(("Secret", c.clone()));
                    }
                }
                for (k, src) in sources {
                    let name_key = if k == "Secret" && src.get("secretName").is_some() {
                        "secretName"
                    } else {
                        "name"
                    };
                    let name = src.get(name_key).and_then(Value::as_str).unwrap_or("");
                    let optional = src
                        .get("optional")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    let Some(data) = maps.get(&(k.to_string(), name.to_string())) else {
                        if optional {
                            continue;
                        }
                        return refuse(format!(
                            "The {kind} {workload} mounts the {k} {name}, which the chart does not make."
                        ));
                    };
                    match src.get("items").and_then(Value::as_array) {
                        Some(items) => {
                            for item in items {
                                let key = item.get("key").and_then(Value::as_str).unwrap_or("");
                                let to = item.get("path").and_then(Value::as_str).unwrap_or(key);
                                let Some(to) = relative(to).filter(|t| !t.is_empty()) else {
                                    return refuse(format!(
                                        "The {kind} {workload} writes a file at a strange place."
                                    ));
                                };
                                if let Some(bytes) = data.get(key) {
                                    files.push(File {
                                        path: format!("{rel}/{to}"),
                                        bytes: bytes.clone(),
                                    });
                                }
                            }
                        }
                        None => {
                            for (key, bytes) in data {
                                if relative(key).is_some_and(|k| !k.is_empty() && !k.contains('/'))
                                {
                                    files.push(File {
                                        path: format!("{rel}/{key}"),
                                        bytes: bytes.clone(),
                                    });
                                }
                            }
                        }
                    }
                }
                vols.insert(vname, Vol::Files(rel));
            }
        }

        let inits: Vec<Value> = pod
            .get("initContainers")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mains: Vec<Value> = pod
            .get("containers")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut init_names: Vec<String> = Vec::new();
        let mut list: Vec<(Value, bool)> = Vec::new();
        for c in inits {
            // A native sidecar keeps running beside the app.
            let sidecar = c.get("restartPolicy").and_then(Value::as_str) == Some("Always");
            list.push((c, !sidecar));
        }
        for c in mains {
            list.push((c, false));
        }

        for (c, is_init) in list {
            let cname = c.get("name").and_then(Value::as_str).unwrap_or("main");
            let sname = service_name(&workload, cname);
            let Some(image) = c.get("image").and_then(Value::as_str) else {
                return refuse(format!(
                    "The container {cname} of {workload} names no image."
                ));
            };
            let env = kube_env(&c, &maps, &workload, &namespace)?;
            let mut svc = Map::new();
            svc.insert("image".into(), json!(image));
            if let Some(Value::Array(cmd)) = c.get("command") {
                let cmd: Vec<String> = cmd
                    .iter()
                    .filter_map(as_string)
                    .map(|s| expand(&s, &env))
                    .collect();
                svc.insert("entrypoint".into(), json!(cmd));
            }
            if let Some(Value::Array(args)) = c.get("args") {
                let args: Vec<String> = args
                    .iter()
                    .filter_map(as_string)
                    .map(|s| expand(&s, &env))
                    .collect();
                svc.insert("command".into(), json!(args));
            }
            if !env.is_empty() {
                svc.insert("environment".into(), Value::Object(env));
            }
            if let Some(w) = c.get("workingDir").and_then(Value::as_str) {
                svc.insert("working_dir".into(), json!(w));
            }
            if c.pointer("/securityContext/readOnlyRootFilesystem")
                .and_then(Value::as_bool)
                == Some(true)
            {
                svc.insert("read_only".into(), json!(true));
            }
            for (from, to) in [("tty", "tty"), ("stdin", "stdin_open")] {
                if let Some(b) = c.get(from).and_then(Value::as_bool) {
                    svc.insert(to.into(), json!(b));
                }
            }
            let mut mounts = Vec::new();
            for m in c
                .get("volumeMounts")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let vname = m.get("name").and_then(Value::as_str).unwrap_or("");
                let target = m.get("mountPath").and_then(Value::as_str).unwrap_or("");
                let read_only = m.get("readOnly").and_then(Value::as_bool).unwrap_or(false);
                let sub = m.get("subPath").and_then(Value::as_str).unwrap_or("");
                let Some(sub) = relative(sub) else {
                    return refuse(format!(
                        "The container {cname} of {workload} mounts a strange path."
                    ));
                };
                let Some(vol) = vols.get(vname) else {
                    return refuse(format!(
                        "The container {cname} of {workload} mounts {vname}, which its pod does not have."
                    ));
                };
                let join = |base: &str| {
                    if sub.is_empty() {
                        base.to_string()
                    } else {
                        format!("{base}/{sub}")
                    }
                };
                let mount = match vol {
                    Vol::Tmpfs => json!({ "type": "tmpfs", "target": target }),
                    Vol::Dir(d) => {
                        let source = join(d);
                        if !sub.is_empty() {
                            if source.starts_with(&project) {
                                scratch.push(source.clone());
                            } else {
                                // A claim's folder is in the data domain;
                                // a sub-folder of it is made the same way.
                                scratch.push(source.clone());
                            }
                        }
                        json!({ "type": "bind", "source": source, "target": target, "read_only": read_only })
                    }
                    Vol::Files(rel) => {
                        let source = format!("{project}/{FILES_DIR}/{}", join(rel));
                        json!({ "type": "bind", "source": source, "target": target, "read_only": true })
                    }
                };
                mounts.push(mount);
            }
            if !mounts.is_empty() {
                svc.insert("volumes".into(), Value::Array(mounts));
            }
            if is_init {
                svc.insert("restart".into(), json!("no"));
                if let Some(prev) = init_names.last() {
                    svc.insert(
                        "depends_on".into(),
                        json!({ prev: { "condition": "service_completed_successfully" } }),
                    );
                }
                init_names.push(sname.clone());
            } else {
                svc.insert(
                    "restart".into(),
                    json!(if job { "on-failure" } else { "unless-stopped" }),
                );
                if !init_names.is_empty() {
                    let deps: Map<String, Value> = init_names
                        .iter()
                        .map(|n| {
                            (
                                n.clone(),
                                json!({ "condition": "service_completed_successfully" }),
                            )
                        })
                        .collect();
                    svc.insert("depends_on".into(), Value::Object(deps));
                }
            }
            if services.contains_key(&sname) {
                return refuse(format!(
                    "Two containers of this app would both be called {sname}."
                ));
            }
            services.insert(sname, Value::Object(svc));
        }
    }
    if services.is_empty() {
        return refuse("This app has nothing to run.");
    }
    if services.len() > MAX_SERVICES {
        return refuse(format!(
            "An app here can have at most {MAX_SERVICES} services."
        ));
    }
    let hosts: Vec<String> = hosts.into_iter().chain(services.keys().cloned()).collect();
    let hosts: Vec<String> = hosts
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    for svc in services.values_mut() {
        if let Value::Object(s) = svc {
            harden(s, uid, &hosts);
        }
    }

    // The chart's own ports: the cluster shaping already checked them.
    let ports = shaped.ports.clone();
    let primary = shaped.primary;
    let compose = json!({
        "name": project_name(&ctx.release),
        "services": services,
        "x-podman": { "in_pod": false },
    });
    scratch.sort();
    scratch.dedup();
    let dirs: Vec<String> = shaped.dirs.clone();
    let (scratch_in, scratch_data): (Vec<String>, Vec<String>) =
        scratch.into_iter().partition(|d| d.starts_with(&project));
    let mut dirs = dirs;
    dirs.extend(scratch_data);
    dirs.sort();
    dirs.dedup();
    Ok(Project {
        compose,
        files,
        dirs,
        scratch: scratch_in,
        ports,
        primary,
        images: Vec::new(),
    })
}

/// A container's environment, with every reference resolved against the
/// chart's own objects.
fn kube_env(
    c: &Value,
    maps: &BTreeMap<(String, String), BTreeMap<String, Vec<u8>>>,
    workload: &str,
    namespace: &str,
) -> Result<Map<String, Value>, Refusal> {
    let mut env = Map::new();
    let text = |b: &Vec<u8>| String::from_utf8_lossy(b).into_owned();
    for from in c
        .get("envFrom")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let prefix = from.get("prefix").and_then(Value::as_str).unwrap_or("");
        for (kind, key) in [("ConfigMap", "configMapRef"), ("Secret", "secretRef")] {
            let Some(r) = from.get(key) else { continue };
            let name = r.get("name").and_then(Value::as_str).unwrap_or("");
            let optional = r.get("optional").and_then(Value::as_bool).unwrap_or(false);
            match maps.get(&(kind.to_string(), name.to_string())) {
                Some(data) => {
                    for (k, v) in data {
                        env.insert(format!("{prefix}{k}"), json!(text(v)));
                    }
                }
                None if optional => {}
                None => {
                    return refuse(format!(
                        "{workload} reads its settings from the {kind} {name}, which the chart does not make."
                    ))
                }
            }
        }
    }
    for e in c.get("env").and_then(Value::as_array).into_iter().flatten() {
        let Some(name) = e.get("name").and_then(Value::as_str) else {
            continue;
        };
        if let Some(v) = e.get("value") {
            let v = as_string(v).unwrap_or_default();
            let v = expand(&v, &env);
            env.insert(name.to_string(), json!(v));
            continue;
        }
        let Some(from) = e.get("valueFrom") else {
            env.insert(name.to_string(), json!(""));
            continue;
        };
        let reference = [("ConfigMap", "configMapKeyRef"), ("Secret", "secretKeyRef")]
            .into_iter()
            .find_map(|(kind, key)| from.get(key).map(|r| (kind, r)));
        if let Some((kind, r)) = reference {
            let map = r.get("name").and_then(Value::as_str).unwrap_or("");
            let key = r.get("key").and_then(Value::as_str).unwrap_or("");
            let optional = r.get("optional").and_then(Value::as_bool).unwrap_or(false);
            match maps
                .get(&(kind.to_string(), map.to_string()))
                .and_then(|d| d.get(key))
            {
                Some(v) => {
                    env.insert(name.to_string(), json!(text(v)));
                }
                None if optional => {}
                None => return refuse(format!(
                    "{workload} reads {name} from the {kind} {map}, which the chart does not make."
                )),
            }
            continue;
        }
        if let Some(field) = from.pointer("/fieldRef/fieldPath").and_then(Value::as_str) {
            let v = match field {
                "metadata.name" => workload.to_string(),
                "metadata.namespace" => namespace.to_string(),
                "status.podIP" | "status.hostIP" => "127.0.0.1".to_string(),
                "status.podIPs" | "status.hostIPs" => "127.0.0.1".to_string(),
                "spec.nodeName" => "localhost".to_string(),
                "spec.serviceAccountName" => "default".to_string(),
                _ => String::new(),
            };
            env.insert(name.to_string(), json!(v));
            continue;
        }
        // A resource field: the box sets no limits, so there is none to read.
        env.insert(name.to_string(), json!(""));
    }
    Ok(env)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apps::shape;

    fn ctx() -> ShapeCtx {
        ShapeCtx {
            release: "board".into(),
            run_as: RunAs::Notshared,
            reserved: vec![8082, 3000],
            taken: vec![(9000, "other".into())],
        }
    }

    fn compose(doc: Value) -> Result<Project, Refusal> {
        shape_compose(
            doc,
            &ctx(),
            &[
                "site/index.html".into(),
                "app.env".into(),
                "db-password".into(),
            ],
        )
    }

    #[test]
    fn a_compose_service_runs_on_the_host_network_as_notshared_and_nothing_more() {
        let p = compose(json!({
            "x-losos": { "title": "Board", "port": 8080 },
            "services": {
                "web": {
                    "image": "docker.io/library/nginx:1.27-alpine",
                    "user": "0",
                    "container_name": "mine",
                    "ports": ["80:8080"],
                    "environment": { "MODE": "prod", "WORKERS": 4, "UNSET": null },
                    "volumes": ["data:/var/lib/board", "./site:/usr/share/nginx/html:ro", "/tmp/cache"],
                    "depends_on": ["db"],
                },
                "db": {
                    "image": "docker.io/library/postgres:17",
                    "expose": [5432],
                    "environment": ["POSTGRES_PASSWORD=secret", "FROM_SHELL"],
                    "restart": "always",
                },
            },
            "volumes": { "data": {} },
        }))
        .unwrap();
        let web = &p.compose["services"]["web"];
        assert_eq!(web["network_mode"], "host");
        assert_eq!(web["user"], "1000:1000");
        assert_eq!(web["userns_mode"], "keep-id");
        assert_eq!(web["cap_drop"], json!(["ALL"]));
        assert_eq!(web["security_opt"], json!(["no-new-privileges"]));
        assert_eq!(web["restart"], "unless-stopped");
        assert_eq!(web["extra_hosts"], json!(["web:127.0.0.1", "db:127.0.0.1"]));
        assert!(web.get("container_name").is_none());
        assert!(web.get("ports").is_none());
        assert_eq!(
            web["environment"],
            json!({ "MODE": "prod", "WORKERS": "4" })
        );
        assert_eq!(
            web["volumes"],
            json!([
                { "type": "bind", "source": "/home/notshared/data/apps/board/data", "target": "/var/lib/board", "read_only": false },
                { "type": "bind", "source": "/var/lib/losos-podman/notshared/projects/board/files/site", "target": "/usr/share/nginx/html", "read_only": true },
                { "type": "volume", "target": "/tmp/cache" },
            ])
        );
        let db = &p.compose["services"]["db"];
        assert_eq!(db["restart"], "always");
        assert_eq!(db["environment"], json!({ "POSTGRES_PASSWORD": "secret" }));
        assert_eq!(p.compose["name"], "losos-board");
        assert_eq!(p.compose["x-podman"], json!({ "in_pod": false }));
        assert_eq!(p.dirs, vec!["/home/notshared/data/apps/board/data"]);
        assert_eq!(p.ports, vec![5432, 8080]);
        assert_eq!(p.primary, Some(8080));
    }

    #[test]
    fn the_box_picks_the_web_port_when_the_app_names_none() {
        let p = compose(json!({ "services": {
            "api": { "image": "a", "expose": ["7000"] },
            "ui": { "image": "b", "ports": [{ "target": 8443, "published": 443 }, "5353:5353/udp"] },
        }}))
        .unwrap();
        assert_eq!(p.ports, vec![7000, 8443]);
        // A service called ui or web is the one with the page.
        assert_eq!(p.primary, Some(8443));
    }

    #[test]
    fn what_reaches_past_the_rules_refuses_with_the_service_named() {
        for (svc, why) in [
            (json!({ "image": "a", "privileged": true }), "privileged"),
            (json!({ "image": "a", "cap_add": ["NET_ADMIN"] }), "cap_add"),
            (json!({ "image": "a", "devices": ["/dev/sda"] }), "devices"),
            (json!({ "image": "a", "pid": "host" }), "pid"),
            (
                json!({ "image": "a", "network_mode": "bridge" }),
                "network_mode",
            ),
            (json!({ "build": "." }), "build"),
            (
                json!({ "image": "a", "volumes": ["/etc:/host-etc"] }),
                "/etc",
            ),
            (json!({ "image": "a", "volumes": ["../up:/x"] }), "../up"),
            (
                json!({ "image": "a", "volumes": ["./missing:/x"] }),
                "does not ship",
            ),
            (
                json!({ "image": "a", "volumes": ["nodef:/x"] }),
                "does not define",
            ),
            (json!({ "image": "a", "ports": ["80"] }), "port 80"),
            (
                json!({ "image": "a", "ports": ["8082"] }),
                "keeps for itself",
            ),
            (
                json!({ "image": "a", "ports": ["9000"] }),
                "other already uses",
            ),
            (
                json!({ "image": "a", "env_file": "/etc/shadow" }),
                "does not ship",
            ),
        ] {
            let e = compose(json!({ "services": { "web": svc } })).unwrap_err();
            assert!(e.0.contains(why), "{why}: {e}");
        }
        let e = compose(json!({ "services": {}, "include": ["x.yaml"] })).unwrap_err();
        assert!(e.0.contains("include"), "{e}");
        let e = compose(json!({ "services": { "w": { "image": "a" } }, "volumes": { "v": { "external": true } } })).unwrap_err();
        assert!(e.0.contains("outside the app"), "{e}");
        let e = compose(json!({ "x-losos": { "images": ["/home/x.tar"] }, "services": { "w": { "image": "a" } } })).unwrap_err();
        assert!(e.0.contains("Nix store"), "{e}");
    }

    #[test]
    fn secrets_and_env_files_are_files_the_app_ships_mounted_read_only() {
        let p = compose(json!({
            "services": { "db": {
                "image": "postgres",
                "env_file": ["./app.env"],
                "secrets": ["pw", { "source": "pw", "target": "/etc/pw" }],
            }},
            "secrets": { "pw": { "file": "./db-password" } },
        }))
        .unwrap();
        let db = &p.compose["services"]["db"];
        let files = "/var/lib/losos-podman/notshared/projects/board/files";
        assert_eq!(db["env_file"], json!([format!("{files}/app.env")]));
        assert_eq!(
            db["volumes"],
            json!([
                { "type": "bind", "source": format!("{files}/db-password"), "target": "/run/secrets/pw", "read_only": true },
                { "type": "bind", "source": format!("{files}/db-password"), "target": "/etc/pw", "read_only": true },
            ])
        );
        let e = compose(json!({ "services": { "db": { "image": "p", "secrets": ["nope"] } } }))
            .unwrap_err();
        assert!(e.0.contains("does not define"), "{e}");
        let e = compose(json!({ "services": { "db": { "image": "p" } }, "secrets": { "pw": { "environment": "X" } } })).unwrap_err();
        assert!(e.0.contains("must be a file"), "{e}");
    }

    #[test]
    fn nix_built_images_are_listed_for_loading() {
        let p = compose(json!({
            "x-losos": { "images": ["/nix/store/abc-board.tar.gz"] },
            "services": { "w": { "image": "losos.local/board:1" } },
        }))
        .unwrap();
        assert_eq!(p.images, vec!["/nix/store/abc-board.tar.gz"]);
    }

    fn chart() -> Vec<Value> {
        vec![
            json!({ "apiVersion": "v1", "kind": "ConfigMap", "metadata": { "name": "board-config" },
                "data": { "board.conf": "listen 8080\n", "MODE": "prod" } }),
            json!({ "apiVersion": "v1", "kind": "Secret", "metadata": { "name": "board-secret" },
                "data": { "password": "aHVudGVyMg==" } }),
            json!({ "apiVersion": "v1", "kind": "Service", "metadata": { "name": "board-db" },
                "spec": { "ports": [{ "port": 5432 }] } }),
            json!({ "apiVersion": "apps/v1", "kind": "Deployment", "metadata": { "name": "board" },
                "spec": { "replicas": 2, "template": { "spec": {
                    "initContainers": [{ "name": "migrate", "image": "board:1", "args": ["migrate", "--db=$(DB)"],
                        "env": [{ "name": "DB", "value": "board-db:5432" }] }],
                    "containers": [{
                        "name": "board", "image": "board:1",
                        "command": ["/bin/board"], "args": ["--name=$(POD)", "--literal=$$(POD)"],
                        "ports": [{ "name": "http", "containerPort": 8080 }],
                        "envFrom": [{ "configMapRef": { "name": "board-config" } }],
                        "env": [
                            { "name": "POD", "valueFrom": { "fieldRef": { "fieldPath": "metadata.name" } } },
                            { "name": "IP", "valueFrom": { "fieldRef": { "fieldPath": "status.podIP" } } },
                            { "name": "PASSWORD", "valueFrom": { "secretKeyRef": { "name": "board-secret", "key": "password" } } },
                            { "name": "MISSING", "valueFrom": { "secretKeyRef": { "name": "nope", "key": "x", "optional": true } } },
                        ],
                        "volumeMounts": [
                            { "name": "data", "mountPath": "/data" },
                            { "name": "config", "mountPath": "/etc/board/board.conf", "subPath": "board.conf" },
                            { "name": "tmp", "mountPath": "/tmp" },
                            { "name": "cache", "mountPath": "/cache" },
                        ],
                        "securityContext": { "readOnlyRootFilesystem": true },
                    }],
                    "volumes": [
                        { "name": "data", "persistentVolumeClaim": { "claimName": "board-data" } },
                        { "name": "config", "configMap": { "name": "board-config" } },
                        { "name": "tmp", "emptyDir": {} },
                        { "name": "cache", "emptyDir": { "medium": "Memory" } },
                    ],
                }}},
            }),
        ]
    }

    #[test]
    fn a_shaped_chart_becomes_a_project_with_its_containers_files_and_folders() {
        let shaped = shape(chart(), &ctx()).unwrap();
        let p = from_kube(shaped, &ctx()).unwrap();
        let services = p.compose["services"].as_object().unwrap();
        assert_eq!(
            services.keys().collect::<Vec<_>>(),
            vec!["board-migrate", "board"]
        );

        let init = &services["board-migrate"];
        assert_eq!(init["restart"], "no");
        assert_eq!(init["command"], json!(["migrate", "--db=board-db:5432"]));

        let main = &services["board"];
        assert_eq!(main["network_mode"], "host");
        assert_eq!(main["user"], "1000:1000");
        assert_eq!(main["userns_mode"], "keep-id");
        assert_eq!(main["read_only"], true);
        assert_eq!(main["restart"], "unless-stopped");
        assert_eq!(main["entrypoint"], json!(["/bin/board"]));
        assert_eq!(main["command"], json!(["--name=board", "--literal=$(POD)"]));
        assert_eq!(
            main["depends_on"],
            json!({ "board-migrate": { "condition": "service_completed_successfully" } })
        );
        let env = &main["environment"];
        assert_eq!(env["MODE"], "prod");
        assert_eq!(env["IP"], "127.0.0.1");
        assert_eq!(env["PASSWORD"], "hunter2");
        assert!(env.get("MISSING").is_none());
        let hosts = main["extra_hosts"].as_array().unwrap();
        assert!(hosts.contains(&json!("board-db:127.0.0.1")));
        assert!(hosts.contains(&json!(
            "board-db.losos-app-board.svc.cluster.local:127.0.0.1"
        )));

        let project = "/var/lib/losos-podman/notshared/projects/board";
        assert_eq!(
            main["volumes"],
            json!([
                { "type": "bind", "source": "/home/notshared/data/apps/board/board-data", "target": "/data", "read_only": false },
                { "type": "bind", "source": format!("{project}/files/kube/board-pod/config/board.conf"), "target": "/etc/board/board.conf", "read_only": true },
                { "type": "bind", "source": format!("{project}/scratch/board-pod/tmp"), "target": "/tmp", "read_only": false },
                { "type": "tmpfs", "target": "/cache" },
            ])
        );
        let mut paths: Vec<&str> = p.files.iter().map(|f| f.path.as_str()).collect();
        paths.sort_unstable();
        assert_eq!(
            paths,
            vec![
                "kube/board-pod/config/MODE",
                "kube/board-pod/config/board.conf"
            ]
        );
        assert_eq!(p.dirs, vec!["/home/notshared/data/apps/board/board-data"]);
        assert_eq!(p.scratch, vec![format!("{project}/scratch/board-pod/tmp")]);
        assert_eq!(p.primary, Some(8080));
    }

    #[test]
    fn a_chart_that_needs_what_it_does_not_make_refuses() {
        let mut docs = chart();
        docs.remove(1);
        let e = from_kube(shape(docs, &ctx()).unwrap(), &ctx()).unwrap_err();
        assert!(e.0.contains("Secret board-secret"), "{e}");

        let cron = json!({ "apiVersion": "batch/v1", "kind": "CronJob", "metadata": { "name": "tick" },
            "spec": { "jobTemplate": { "spec": { "template": { "spec": { "containers": [{ "name": "t", "image": "t" }] } } } } } });
        let e = from_kube(shape(vec![cron], &ctx()).unwrap(), &ctx()).unwrap_err();
        assert!(e.0.contains("CronJob"), "{e}");
    }

    #[test]
    fn base64_reads_padded_and_unpadded_values() {
        assert_eq!(base64_decode("aHVudGVyMg==").unwrap(), b"hunter2");
        assert_eq!(base64_decode("aHVudGVyMg").unwrap(), b"hunter2");
        assert_eq!(base64_decode("").unwrap(), b"");
        assert!(base64_decode("not base64!").is_none());
    }

    #[test]
    fn relative_paths_stay_inside_the_app() {
        assert_eq!(relative("./site/").as_deref(), Some("site"));
        assert_eq!(relative(".").as_deref(), Some(""));
        assert_eq!(relative("a/b").as_deref(), Some("a/b"));
        assert!(relative("../x").is_none());
        assert!(relative("a/../../x").is_none());
        assert!(relative("/etc").is_none());
        assert!(relative("~/x").is_none());
    }
}
