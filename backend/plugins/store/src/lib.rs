//! The app marketplace: Docker Hub images and community agent addons.
//!
//! Two sources, one place to look:
//!
//! * **Docker Hub** (`/api/store/hub/*`): search, tags, what an image says
//!   about itself, and install, which becomes an isolated Compose app
//!   (`x-losos.isolated`, `backend/src/compose.rs`) through lososd's own
//!   `POST /api/apps/compose`, so Hub images run on the same rootless Podman
//!   as every app and the same shaping has the last word.
//! * **Agent addons** (`/api/store/addons`, `/api/addons*`): MCP servers the
//!   community lists in an index, each pinned to an image digest, each run
//!   by the `losos-addon@` unit (`modules/store.nix`) in a user namespace of
//!   its own and published on loopback. They run only on a primary or
//!   tertiary connection ([`addon::gate`]); behind a local gateway the box
//!   browses the index and stops them.
//!
//! The tools of the installed addons are what an agent harness reads
//! (`GET /api/addons`, names `addon_<slug>__<tool>`) and calls
//! (`POST /api/addons/{slug}/call`).

pub mod addon;
pub mod hub;

use losos_plugin::{
    cache, clock, config, dispatch, exec, fs, http, log, Plugin, Request, Response,
};
use serde_json::{json, Value};

const UNIT_PREFIX: &str = "losos-addon@";
/// An addon that has not answered as an MCP server in this long has failed.
const START_SECS: u64 = 900;

struct Cfg {
    dir: String,
    hub: String,
    registry: String,
    auth: String,
    index: String,
    ports: (u16, u16),
}

fn cfg() -> Cfg {
    let port = |key: &str, default: u16| {
        config::get(key)
            .and_then(|v| v.parse().ok())
            .unwrap_or(default)
    };
    Cfg {
        dir: config::get_or("dir", "/var/lib/losos-addons"),
        hub: config::get_or("hubUrl", "https://hub.docker.com"),
        registry: config::get_or("registryUrl", "https://registry-1.docker.io"),
        auth: config::get_or("authUrl", "https://auth.docker.io"),
        index: config::get_or("indexUrl", ""),
        ports: (port("portLow", 41000), port("portHigh", 41099)),
    }
}

fn json_error(status: u16, message: &str, reason: Option<&str>) -> Response {
    let mut body = json!({ "error": message });
    if let Some(r) = reason {
        body["reason"] = json!(r);
    }
    Response::json(status, &body)
}

// ── The gate ──────────────────────────────────────────────────────────────

fn read_gate() -> losos_gate::Gate {
    let file = config::get_or("pathEnv", "/run/losos/edge-path.env");
    let marker = config::get_or("noEdgeMarker", "/run/losos/edge-none");
    let env = fs::read_string_optional(&file).unwrap_or(None);
    let none = fs::exists(&marker).unwrap_or(false);
    losos_gate::gate(env.as_deref(), none)
}

// ── Records on disk ───────────────────────────────────────────────────────

struct Record {
    manifest: addon::Manifest,
    raw: Value,
    host_port: u16,
    state: String,
    message: String,
    tools: Vec<Value>,
    installed_at: u64,
    started_at: u64,
}

impl Record {
    fn to_json(&self) -> Value {
        json!({
            "manifest": self.raw,
            "hostPort": self.host_port,
            "state": self.state,
            "message": self.message,
            "tools": self.tools,
            "installedAt": self.installed_at,
            "startedAt": self.started_at,
        })
    }

    fn from_json(v: &Value) -> Option<Record> {
        Some(Record {
            manifest: addon::parse_manifest(&v["manifest"]).ok()?,
            raw: v["manifest"].clone(),
            host_port: u16::try_from(v["hostPort"].as_u64()?).ok()?,
            state: v["state"].as_str()?.to_string(),
            message: v["message"].as_str().unwrap_or_default().to_string(),
            tools: v["tools"].as_array().cloned().unwrap_or_default(),
            installed_at: v["installedAt"].as_u64().unwrap_or(0),
            started_at: v["startedAt"].as_u64().unwrap_or(0),
        })
    }

    fn unit(&self) -> String {
        format!("{UNIT_PREFIX}{}.service", self.manifest.slug)
    }

    fn url(&self) -> String {
        format!("http://127.0.0.1:{}{}", self.host_port, self.manifest.path)
    }

    fn save(&self, c: &Cfg) {
        let path = format!("{}/{}/addon.json", c.dir, self.manifest.slug);
        let body = serde_json::to_vec_pretty(&self.to_json()).unwrap_or_default();
        if let Err(e) = fs::write(&path, &body, fs::WriteOptions::public()) {
            log::error(&format!("cannot write {path}: {e}"));
        }
    }

    /// The addon as a page or a harness sees it.
    fn view(&self, gate: &losos_gate::Gate) -> Value {
        let live = gate.open && self.state == "ready";
        let tools: Vec<Value> = if live {
            self.tools
                .iter()
                .map(|t| {
                    json!({
                        "name": addon::model_name(&self.manifest.slug, t["name"].as_str().unwrap_or("")),
                        "tool": t["name"],
                        "description": t["description"],
                        "inputSchema": t["inputSchema"],
                    })
                })
                .collect()
        } else {
            Vec::new()
        };
        let state = if gate.open {
            self.state.as_str()
        } else {
            "paused"
        };
        json!({
            "addon": self.manifest.view(),
            "slug": self.manifest.slug,
            "title": self.manifest.title,
            "state": state,
            "message": if gate.open { self.message.as_str() } else { gate.sentence() },
            "installedAt": self.installed_at,
            "tools": tools,
        })
    }
}

fn records(c: &Cfg) -> Vec<Record> {
    let mut out = Vec::new();
    let Ok(entries) = fs::list(&c.dir) else {
        return out;
    };
    for e in entries {
        let path = format!("{}/{}/addon.json", c.dir, e.name);
        let Ok(Some(bytes)) = fs::read_optional(&path) else {
            continue;
        };
        match serde_json::from_slice::<Value>(&bytes)
            .ok()
            .as_ref()
            .and_then(Record::from_json)
        {
            Some(r) => out.push(r),
            None => log::warn(&format!("{path} is not an addon record")),
        }
    }
    out.sort_by(|a, b| a.manifest.slug.cmp(&b.manifest.slug));
    out
}

fn find(c: &Cfg, slug: &str) -> Option<Record> {
    if !losos_names::release(slug) {
        return None;
    }
    records(c).into_iter().find(|r| r.manifest.slug == slug)
}

// ── systemd ───────────────────────────────────────────────────────────────

fn systemctl(args: &[&str]) -> Option<exec::Output> {
    match exec::Command::new("systemctl")
        .args(args.iter().copied())
        .timeout_ms(20_000)
        .output()
    {
        Ok(o) => Some(o),
        Err(e) => {
            log::warn(&format!("systemctl {}: {e}", args.join(" ")));
            None
        }
    }
}

fn unit_active(unit: &str) -> bool {
    systemctl(&["is-active", "--quiet", unit]).is_some_and(|o| o.success())
}

fn start_unit(unit: &str) {
    let _ = systemctl(&["start", "--no-block", unit]);
}

fn stop_unit(unit: &str) {
    let _ = systemctl(&["stop", "--no-block", unit]);
}

// ── Talking MCP ───────────────────────────────────────────────────────────

fn post(
    url: &str,
    session: Option<&str>,
    body: &Value,
    timeout_ms: u32,
) -> Result<http::Response, String> {
    let mut req = http::Request::post(url)
        .header("accept", "application/json, text/event-stream")
        .json(body)
        .timeout_ms(timeout_ms);
    if let Some(s) = session {
        req = req.header("mcp-session-id", s);
    }
    req.fetch(4 * 1024 * 1024).map_err(|e| e.to_string())
}

/// `initialize`, `initialized`, then `request`, and the answer's result.
fn mcp(r: &Record, request: &Value, id: u64, timeout_ms: u32) -> Result<Value, String> {
    let url = r.url();
    let init = post(&url, None, &addon::initialize_body(), 8000)?;
    if !init.is_success() {
        return Err(format!("the addon answered {} to initialize", init.status));
    }
    addon::rpc_result(&init.body, 1)?;
    let session = init.header("mcp-session-id").map(str::to_string);
    let _ = post(&url, session.as_deref(), &addon::initialized_body(), 8000);
    let answer = post(&url, session.as_deref(), request, timeout_ms)?;
    if !answer.is_success() {
        return Err(format!("the addon answered {}", answer.status));
    }
    addon::rpc_result(&answer.body, id)
}

fn probe(r: &Record) -> Result<Vec<Value>, String> {
    mcp(r, &addon::tools_list_body(), 2, 10_000).map(|v| addon::parse_tools(&v))
}

// ── The reconcile pass ────────────────────────────────────────────────────

fn sync() {
    let c = cfg();
    let gate = read_gate();
    for mut r in records(&c) {
        let unit = r.unit();
        let active = unit_active(&unit);
        let now = clock::now();
        if !gate.open {
            if active {
                stop_unit(&unit);
            }
            continue;
        }
        if !active {
            if r.state != "failed" {
                start_unit(&unit);
                if r.state == "ready" || r.state == "paused" {
                    r.state = "starting".into();
                    r.started_at = now;
                    r.save(&c);
                }
            }
            continue;
        }
        match probe(&r) {
            Ok(tools) => {
                if r.state != "ready" || r.tools != tools {
                    r.state = "ready".into();
                    r.message = String::new();
                    r.tools = tools;
                    r.save(&c);
                }
            }
            Err(why) => {
                if r.state == "ready" {
                    r.state = "starting".into();
                    r.started_at = now;
                    r.save(&c);
                } else if r.state != "failed" && now.saturating_sub(r.started_at) > START_SECS {
                    r.state = "failed".into();
                    r.message = format!(
                        "{} did not answer as an MCP server within {} minutes ({why}).",
                        r.manifest.title,
                        START_SECS / 60
                    );
                    r.save(&c);
                    stop_unit(&unit);
                }
            }
        }
    }
}

// ── Docker Hub ────────────────────────────────────────────────────────────

fn fetch_json(url: &str, headers: &[(&str, &str)]) -> Result<http::Response, String> {
    let mut req = http::Request::get(url).timeout_ms(15_000);
    for (k, v) in headers {
        req = req.header(k, v);
    }
    req.fetch(8 * 1024 * 1024).map_err(|e| e.to_string())
}

fn unreachable(what: &str, why: &str) -> Response {
    log::warn(&format!("{what}: {why}"));
    json_error(
        502,
        &format!("{what} could not be reached. Try again in a moment."),
        Some("unreachable"),
    )
}

fn hub_search(req: &Request) -> Response {
    let q = req.query_param("q").unwrap_or_default();
    let q = match hub::query(&q) {
        Ok(q) => q.to_string(),
        Err(e) => return json_error(400, e, None),
    };
    let c = cfg();
    let key = format!("hub/search/{q}");
    if let Some(rows) = cache::get_json::<Value>(&key) {
        return Response::ok(&json!({ "source": "Docker Hub", "results": rows }));
    }
    match fetch_json(&hub::search_url(&c.hub, &q), &[]) {
        Ok(r) if r.status == 200 => {
            let rows = Value::Array(hub::parse_search(&r.body));
            cache::set_json(&key, &rows, 300);
            Response::ok(&json!({ "source": "Docker Hub", "results": rows }))
        }
        Ok(r) => unreachable("Docker Hub", &format!("answered {}", r.status)),
        Err(e) => unreachable("Docker Hub", &e),
    }
}

fn image_param(req: &Request) -> Result<(String, String), Response> {
    hub::repository(&req.query_param("image").unwrap_or_default())
        .map_err(|e| json_error(400, e, None))
}

fn hub_tags(req: &Request) -> Response {
    let (owner, name) = match image_param(req) {
        Ok(v) => v,
        Err(r) => return r,
    };
    match fetch_json(&hub::tags_url(&cfg().hub, &owner, &name), &[]) {
        Ok(r) if r.status == 200 => Response::ok(&json!({ "tags": hub::parse_tags(&r.body) })),
        Ok(r) if r.status == 404 => json_error(
            404,
            "Docker Hub has no image of that name.",
            Some("noImage"),
        ),
        Ok(r) => unreachable("Docker Hub", &format!("answered {}", r.status)),
        Err(e) => unreachable("Docker Hub", &e),
    }
}

const MANIFEST_TYPES: &str = "application/vnd.oci.image.index.v1+json, \
application/vnd.docker.distribution.manifest.list.v2+json, \
application/vnd.oci.image.manifest.v1+json, \
application/vnd.docker.distribution.manifest.v2+json";

/// The image's configuration blob: token, manifest (through an index to the
/// Linux x86-64 one), then the blob, following the one redirect a registry
/// gives to its storage.
fn image_config(owner: &str, name: &str, tag: &str) -> Result<Vec<u8>, String> {
    let c = cfg();
    let scope = format!("repository:{owner}/{name}:pull");
    let url = format!(
        "{}/token?service=registry.docker.io&scope={}",
        c.auth,
        hub::encode(&scope)
    );
    let r = fetch_json(&url, &[])?;
    if r.status != 200 {
        return Err(format!("the registry's token answered {}", r.status));
    }
    let token = serde_json::from_slice::<Value>(&r.body)
        .ok()
        .and_then(|v| v["token"].as_str().map(str::to_string))
        .ok_or("the registry gave no token")?;
    let bearer = format!("Bearer {token}");
    let get_manifest = |reference: &str| {
        fetch_json(
            &format!("{}/v2/{owner}/{name}/manifests/{reference}", c.registry),
            &[("authorization", &bearer), ("accept", MANIFEST_TYPES)],
        )
    };
    let r = get_manifest(tag)?;
    if r.status != 200 {
        return Err(format!("the registry answered {} for the tag", r.status));
    }
    let mut doc: Value = serde_json::from_slice(&r.body).map_err(|e| e.to_string())?;
    if let Some(list) = doc["manifests"].as_array() {
        let digest = list
            .iter()
            .find(|m| m["platform"]["architecture"] == "amd64" && m["platform"]["os"] == "linux")
            .and_then(|m| m["digest"].as_str())
            .ok_or("the image has no Linux x86-64 build")?
            .to_string();
        if !digest.starts_with("sha256:") || digest.len() != 71 {
            return Err("the registry named a strange digest".into());
        }
        let r = get_manifest(&digest)?;
        if r.status != 200 {
            return Err(format!("the registry answered {} for the build", r.status));
        }
        doc = serde_json::from_slice(&r.body).map_err(|e| e.to_string())?;
    }
    let digest = doc["config"]["digest"]
        .as_str()
        .ok_or("the image names no configuration")?;
    if !digest.starts_with("sha256:") || digest.len() != 71 {
        return Err("the registry named a strange digest".into());
    }
    let blob = format!("{}/v2/{owner}/{name}/blobs/{digest}", c.registry);
    let mut r = fetch_json(&blob, &[("authorization", &bearer)])?;
    if matches!(r.status, 301 | 302 | 307 | 308) {
        let to = r
            .header("location")
            .map(str::to_string)
            .filter(|l| l.starts_with("https://") || l.starts_with(&c.registry))
            .ok_or("the registry sent the configuration somewhere odd")?;
        r = fetch_json(&to, &[])?;
    }
    if r.status != 200 {
        return Err(format!(
            "the registry answered {} for the configuration",
            r.status
        ));
    }
    Ok(r.body)
}

fn hub_inspect(req: &Request) -> Response {
    let (owner, name) = match image_param(req) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let tag = req.query_param("tag").unwrap_or_else(|| "latest".into());
    if !hub::tag_ok(&tag) {
        return json_error(400, "That is not an image tag.", None);
    }
    match image_config(&owner, &name, &tag) {
        Ok(config) => Response::ok(&hub::parse_inspect(&config)),
        Err(e) => unreachable("The image's registry", &e),
    }
}

fn hub_install(req: &Request) -> Response {
    let install = match hub::parse_install(&req.json_value()) {
        Ok(i) => i,
        Err(e) => return json_error(400, &e, None),
    };
    let ask = Request {
        method: "POST".into(),
        path: "/api/apps/compose".into(),
        route: String::new(),
        params: Vec::new(),
        query: String::new(),
        headers: vec![("content-type".into(), "application/json".into())],
        body: serde_json::to_vec(&hub::compose_request(&install)).unwrap_or_default(),
        caller: req.caller.clone(),
    };
    match dispatch::call(ask) {
        Ok(r) => {
            let mut out = Response::new(r.status).header("content-type", "application/json");
            out.body = r.body;
            out
        }
        Err(e) => {
            log::warn(&format!("POST /api/apps/compose: {e}"));
            json_error(
                500,
                "The app could not be started. See the box's journal.",
                None,
            )
        }
    }
}

// ── Addons ────────────────────────────────────────────────────────────────

/// The community index: fetched from the box's index address, kept five
/// minutes.
fn catalogue(c: &Cfg, fresh: bool) -> Result<(Vec<addon::Manifest>, usize), String> {
    if c.index.is_empty() {
        return Err("This box has no addon index set.".into());
    }
    if !fresh {
        if let Some(body) = cache::get("addons/index") {
            return Ok(addon::parse_index(&body));
        }
    }
    let r = fetch_json(&c.index, &[])?;
    if r.status != 200 {
        return Err(format!("the index answered {}", r.status));
    }
    if r.body.len() > 2 * 1024 * 1024 {
        return Err("the index is larger than 2 MiB".into());
    }
    cache::set("addons/index", &r.body, 300);
    Ok(addon::parse_index(&r.body))
}

fn addons_catalogue(req: &Request) -> Response {
    let c = cfg();
    let gate = read_gate();
    let installed = records(&c);
    match catalogue(&c, req.query_param("refresh").is_some()) {
        Ok((list, refused)) => {
            let rows: Vec<Value> = list
                .iter()
                .map(|m| {
                    let mut v = m.view();
                    v["installed"] = json!(installed.iter().any(|r| r.manifest.slug == m.slug));
                    v
                })
                .collect();
            Response::ok(&json!({
                "source": "Community index",
                "gate": gate.view(),
                "addons": rows,
                "refused": refused,
            }))
        }
        Err(e) => {
            log::warn(&format!("addon index: {e}"));
            Response::ok(&json!({
                "source": "Community index",
                "gate": gate.view(),
                "addons": [],
                "refused": 0,
                "unavailable": "The community index could not be reached.",
            }))
        }
    }
}

fn addon_install(req: &Request) -> Response {
    let c = cfg();
    let gate = read_gate();
    if !gate.open {
        return json_error(409, gate.sentence(), gate.reason);
    }
    let body = req.json_value();
    let slug = body["slug"].as_str().unwrap_or_default();
    let manifest = match catalogue(&c, false) {
        Ok((list, _)) => list.into_iter().find(|m| m.slug == slug),
        Err(e) => return unreachable("The community index", &e),
    };
    let Some(manifest) = manifest else {
        return json_error(
            404,
            "The index lists no addon of that name.",
            Some("noAddon"),
        );
    };
    let existing = records(&c);
    if existing.iter().any(|r| r.manifest.slug == manifest.slug) {
        return json_error(409, "That addon is already installed.", Some("installed"));
    }
    if manifest.network && body["allowNetwork"].as_bool() != Some(true) {
        return json_error(
            400,
            "This addon reaches beyond the box, to the internet and your network. Confirm that first.",
            Some("networkNotAccepted"),
        );
    }
    let given = body["env"].as_object();
    for name in given.into_iter().flatten().map(|(k, _)| k) {
        if !manifest.env.iter().any(|e| &e.name == name) {
            return json_error(
                400,
                &format!("{name} is not a setting of this addon."),
                None,
            );
        }
    }
    let mut env_file = String::new();
    for spec in &manifest.env {
        let value = given
            .and_then(|m| m.get(&spec.name))
            .and_then(Value::as_str)
            .unwrap_or("");
        if value.is_empty() {
            if spec.required {
                return json_error(400, &format!("{} is required.", spec.label), None);
            }
            continue;
        }
        if value.len() > 4096 || value.contains(['\n', '\r', '\0']) {
            return json_error(
                400,
                &format!(
                    "The value of {} is too long or has a line break.",
                    spec.label
                ),
                None,
            );
        }
        env_file.push_str(&format!("{}={}\n", spec.name, value));
    }
    let used: Vec<u16> = existing.iter().map(|r| r.host_port).collect();
    let Some(port) = addon::pick_port(c.ports.0, c.ports.1, &used) else {
        return json_error(
            409,
            "This box has no room for another addon. Remove one first.",
            Some("noRoom"),
        );
    };
    let dir = format!("{}/{}", c.dir, manifest.slug);
    if let Err(e) = fs::create_dir(&dir, Some(0o755)) {
        log::error(&format!("cannot make {dir}: {e}"));
        return json_error(
            500,
            "The addon could not be set up. See the box's journal.",
            None,
        );
    }
    let env_path = format!("{dir}/env");
    let opts = fs::WriteOptions {
        mode: Some(0o600),
        owner: Some(1000),
        create_dirs: false,
    };
    if let Err(e) = fs::write(&env_path, env_file.as_bytes(), opts) {
        log::error(&format!("cannot write {env_path}: {e}"));
        return json_error(
            500,
            "The addon could not be set up. See the box's journal.",
            None,
        );
    }
    let now = clock::now();
    let raw = manifest_json(&manifest);
    let record = Record {
        manifest,
        raw,
        host_port: port,
        state: "installing".into(),
        message: String::new(),
        tools: Vec::new(),
        installed_at: now,
        started_at: now,
    };
    record.save(&c);
    start_unit(&record.unit());
    Response::json(202, &record.view(&gate))
}

/// The manifest as the index states it, rebuilt for the record.
fn manifest_json(m: &addon::Manifest) -> Value {
    json!({
        "slug": m.slug, "title": m.title, "summary": m.summary, "author": m.author,
        "homepage": m.homepage, "version": m.version, "image": m.image,
        "port": m.port, "path": m.path,
        "network": if m.network { "outbound" } else { "none" },
        "env": m.env.iter().map(|e| json!({
            "name": e.name, "label": e.label, "secret": e.secret, "required": e.required,
        })).collect::<Vec<_>>(),
        "data": m.data, "memoryMiB": m.memory_mib,
    })
}

fn addons_list() -> Response {
    let c = cfg();
    let gate = read_gate();
    let addons: Vec<Value> = records(&c).iter().map(|r| r.view(&gate)).collect();
    Response::ok(&json!({ "gate": gate.view(), "addons": addons }))
}

fn addon_remove(slug: &str) -> Response {
    let c = cfg();
    let Some(r) = find(&c, slug) else {
        return json_error(404, "No addon of that name is installed.", Some("noAddon"));
    };
    stop_unit(&r.unit());
    let dir = format!("{}/{}", c.dir, r.manifest.slug);
    if let Err(e) = fs::remove_tree_if_present(&dir) {
        log::error(&format!("cannot remove {dir}: {e}"));
        return json_error(
            500,
            "The addon could not be removed. See the box's journal.",
            None,
        );
    }
    Response::ok(&json!({ "removed": slug }))
}

fn addon_call(req: &Request, slug: &str) -> Response {
    let c = cfg();
    let gate = read_gate();
    if !gate.open {
        return json_error(409, gate.sentence(), gate.reason);
    }
    let Some(r) = find(&c, slug) else {
        return json_error(404, "No addon of that name is installed.", Some("noAddon"));
    };
    if r.state != "ready" {
        return json_error(409, "That addon is not running yet.", Some("notReady"));
    }
    let body = req.json_value();
    let Some(tool) = body["tool"]
        .as_str()
        .filter(|t| r.tools.iter().any(|x| x["name"] == *t))
    else {
        return json_error(400, "That addon has no such tool.", Some("noTool"));
    };
    let arguments = if body["arguments"].is_object() {
        body["arguments"].clone()
    } else {
        json!({})
    };
    match mcp(&r, &addon::tools_call_body(tool, &arguments), 2, 120_000) {
        Ok(result) => Response::ok(&result),
        Err(why) => {
            log::warn(&format!("addon {slug} {tool}: {why}"));
            json_error(
                502,
                &format!("The addon failed: {why}."),
                Some("addonFailed"),
            )
        }
    }
}

fn overview() -> Response {
    let c = cfg();
    let gate = read_gate();
    let addons = records(&c);
    Response::ok(&json!({
        "hub": { "available": true, "source": "Docker Hub" },
        "addons": {
            "gate": gate.view(),
            "indexSet": !c.index.is_empty(),
            "installed": addons.len(),
        },
    }))
}

pub struct StorePlugin;

impl Plugin for StorePlugin {
    fn handle(req: Request) -> Response {
        match (req.method.as_str(), req.route.as_str()) {
            ("GET", "/api/store") => overview(),
            ("GET", "/api/store/hub/search") => hub_search(&req),
            ("GET", "/api/store/hub/tags") => hub_tags(&req),
            ("GET", "/api/store/hub/inspect") => hub_inspect(&req),
            ("POST", "/api/store/hub/install") => hub_install(&req),
            ("GET", "/api/store/addons") => addons_catalogue(&req),
            ("POST", "/api/store/addons") => addon_install(&req),
            ("GET", "/api/addons") => addons_list(),
            ("POST", "/api/addons/{slug}/call") => {
                addon_call(&req, req.param("slug").unwrap_or(""))
            }
            ("DELETE", "/api/addons/{slug}") => addon_remove(req.param("slug").unwrap_or("")),
            _ => Response::error(404, "not found"),
        }
    }

    fn tick(timer: &str) {
        if timer == "sync" || timer == "start" {
            sync();
        }
    }
}

losos_plugin::export!(StorePlugin);
