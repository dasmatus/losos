//! lososd's plugins: WebAssembly components that do the daemon's work.
//!
//! lososd is the host. It keeps what a plugin must not decide for itself:
//! the listener, who a caller is and which section of `sections.json` a
//! route is in (`crate::http`), the D-Bus name, and the rules below. Each
//! area of the admin API (the look, people, storage, apps, the market, …)
//! is a plugin: a component written against `plugins/wit/plugin.wit` and a
//! manifest that lists its routes and everything its imports may touch.
//!
//! * **Where from.** Only a read-only directory in the Nix store
//!   ([`trust`]); `LOSOS_PLUGIN_DIR` names it and the NixOS module sets it
//!   to `losos.backend.plugins`. Anything writable, anything outside the
//!   store, and lososd refuses to start.
//! * **What with.** No WASI: a plugin has no files, clock or sockets of its
//!   own. Every import goes through [`host`], which checks the manifest
//!   ([`manifest`]) first.
//! * **How long, how much.** One fresh instance per call, so nothing leaks
//!   between two callers; linear memory capped by `limits.memoryMiB`, and an
//!   epoch deadline that traps a call past `limits.timeoutSecs`.
//!
//! Components are compiled once, when lososd starts, with Cranelift.

#![allow(unreachable_pub)]

pub mod bindings;
pub mod exec;
pub mod files;
pub mod host;
pub mod manifest;
pub mod net;
pub mod trust;
pub mod units;

use anyhow::{bail, Context};
use bindings::losos::plugin::types::{Caller, Error, Request, Response};
use host::{Cache, HostState, Locks};
use manifest::{Manifest, Route};
use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use wasmtime::component::{Component, HasSelf, Linker, ResourceTable};
use wasmtime::{Config, Engine, Store, StoreLimitsBuilder};

/// One epoch, the resolution of every deadline.
const EPOCH: Duration = Duration::from_millis(50);

/// A plugin, loaded.
pub struct Loaded {
    pub manifest: Manifest,
    pre: bindings::PluginPre<HostState>,
    cache: Mutex<Cache>,
    jobs: AtomicU32,
    ticking: Mutex<HashSet<String>>,
}

impl Loaded {
    fn cache(&self) -> MutexGuard<'_, Cache> {
        self.cache.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Whether `caller` may call `method` on `route`: the host's section check,
/// handed in by `crate::http`, so a dispatched call is held to the same
/// rule as one that arrived.
pub type Authorize = dyn Fn(&Caller, &str, &str) -> Result<(), String> + Send + Sync;

/// What every call shares.
pub struct Inner {
    engine: Engine,
    plugins: Vec<Arc<Loaded>>,
    locks: Locks,
    started: Instant,
    authorize: Mutex<Option<Arc<Authorize>>>,
}

/// The loaded plugins and their routes. Cheap to clone.
#[derive(Clone)]
pub struct Runtime(Arc<Inner>);

/// A route that matched a request.
pub struct Matched {
    pub plugin: Arc<Loaded>,
    pub route: Route,
    pub params: Vec<(String, String)>,
}

/// Percent-decode one path segment; `None` for one that is not UTF-8.
fn decode_segment(seg: &str) -> Option<String> {
    let bytes = seg.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Some(b) = std::str::from_utf8(&bytes[i + 1..i + 3])
                .ok()
                .and_then(|h| u8::from_str_radix(h, 16).ok())
            {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).ok()
}

/// Match `path` against a route pattern; the params on a match.
pub fn match_route(pattern: &str, path: &str) -> Option<Vec<(String, String)>> {
    let mut pat = pattern.split('/');
    let mut got = path.split('/');
    let mut params = Vec::new();
    loop {
        match (pat.next(), got.next()) {
            (None, None) => return Some(params),
            (Some(p), Some(g)) => {
                if let Some(name) = p.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
                    if g.is_empty() {
                        return None;
                    }
                    params.push((name.to_string(), decode_segment(g)?));
                } else if p != g {
                    return None;
                }
            }
            _ => return None,
        }
    }
}

fn literal_segments(pattern: &str) -> usize {
    pattern.split('/').filter(|s| !s.starts_with('{')).count()
}

impl Runtime {
    /// The plugins `LOSOS_PLUGIN_DIR` names, under [`trust::Trust::from_env`].
    /// No directory at all is a daemon with only its own routes, which is
    /// what the binary's tests start; the NixOS module always sets one.
    pub fn from_env() -> anyhow::Result<Runtime> {
        let env = |var: &str| std::env::var(var).ok();
        match std::env::var_os("LOSOS_PLUGIN_DIR").filter(|d| !d.is_empty()) {
            Some(dir) => Runtime::load(Path::new(&dir), &trust::Trust::from_env(), &env)
                .with_context(|| format!("loading the plugins in {}", dir.to_string_lossy())),
            None => {
                tracing::warn!("LOSOS_PLUGIN_DIR is not set; no plugin routes are served");
                Runtime::with(Vec::new(), None)
            }
        }
    }

    /// Load every plugin in `dir` under `trust`, with `${VAR}`s from `env`.
    pub fn load(
        dir: &Path,
        trust: &trust::Trust,
        env: manifest::Env<'_>,
    ) -> anyhow::Result<Runtime> {
        let found = trust::discover(dir, trust)?;
        Runtime::with(found, Some(env))
    }

    fn with(found: Vec<trust::Found>, env: Option<manifest::Env<'_>>) -> anyhow::Result<Runtime> {
        let none = |_: &str| None;
        let env = env.unwrap_or(&none);
        let mut config = Config::new();
        config.epoch_interruption(true);
        let engine =
            Engine::new(&config).map_err(|e| anyhow::anyhow!("starting Wasmtime: {e:?}"))?;
        let mut linker: Linker<HostState> = Linker::new(&engine);
        bindings::Plugin::add_to_linker::<HostState, HasSelf<HostState>>(&mut linker, |s| s)
            .map_err(|e| anyhow::anyhow!("linking the host: {e:?}"))?;

        let mut plugins: Vec<Arc<Loaded>> = Vec::new();
        for f in found {
            let started = Instant::now();
            let manifest = Manifest::parse(
                &std::fs::read(&f.manifest)
                    .with_context(|| format!("reading {}", f.manifest.display()))?,
                &f.name,
                env,
            )
            .with_context(|| format!("plugin {}", f.name))?;
            for route in &manifest.routes {
                if let Some(other) = plugins.iter().find(|p| {
                    p.manifest
                        .routes
                        .iter()
                        .any(|r| r.method == route.method && r.path == route.path)
                }) {
                    bail!(
                        "{} {} is claimed by both {} and {}",
                        route.method,
                        route.path,
                        other.manifest.name,
                        manifest.name
                    );
                }
            }
            let bytes =
                std::fs::read(&f.wasm).with_context(|| format!("reading {}", f.wasm.display()))?;
            let component = Component::new(&engine, &bytes)
                .map_err(|e| anyhow::anyhow!("compiling plugin {}: {e:?}", f.name))?;
            let pre = linker
                .instantiate_pre(&component)
                .and_then(bindings::PluginPre::new)
                .map_err(|e| {
                    anyhow::anyhow!("plugin {} does not fit lososd's imports: {e:?}", f.name)
                })?;
            tracing::info!(
                plugin = %f.name,
                routes = manifest.routes.len(),
                ms = started.elapsed().as_millis() as u64,
                "plugin loaded"
            );
            plugins.push(Arc::new(Loaded {
                manifest,
                pre,
                cache: Mutex::default(),
                jobs: AtomicU32::new(0),
                ticking: Mutex::default(),
            }));
        }

        // The deadlines' clock.
        let ticker = engine.weak();
        std::thread::Builder::new()
            .name("plugin-epoch".to_string())
            .spawn(move || loop {
                std::thread::sleep(EPOCH);
                match ticker.upgrade() {
                    Some(engine) => engine.increment_epoch(),
                    None => return,
                }
            })?;

        Ok(Runtime(Arc::new(Inner {
            engine,
            plugins,
            locks: Locks::default(),
            started: Instant::now(),
            authorize: Mutex::new(None),
        })))
    }

    /// The section check a dispatched call goes through.
    pub fn set_authorize(&self, f: Arc<Authorize>) {
        *self.0.authorize.lock().unwrap_or_else(|e| e.into_inner()) = Some(f);
    }

    pub fn plugins(&self) -> &[Arc<Loaded>] {
        &self.0.plugins
    }

    /// The largest body any plugin route accepts.
    pub fn max_body(&self) -> usize {
        self.0
            .plugins
            .iter()
            .flat_map(|p| p.manifest.routes.iter().map(|r| r.max_body))
            .max()
            .unwrap_or(0)
    }

    /// The route `method path` reaches, if a plugin answers it. The most
    /// literal pattern wins, so `/api/x/new` beats `/api/x/{id}`.
    pub fn route(&self, method: &str, path: &str) -> Option<Matched> {
        route_in(&self.0, method, path)
    }

    /// Answer one request. A trap or a deadline is a 500 here and a line in
    /// the journal.
    pub fn handle(&self, plugin: &Arc<Loaded>, request: &Request) -> Response {
        handle_in(&self.0, plugin, request)
    }

    /// Fire every timer's first tick and keep the periodic ones going.
    pub fn start_timers(&self) {
        for plugin in &self.0.plugins {
            for timer in &plugin.manifest.timers {
                let rt = self.0.clone();
                let plugin = plugin.clone();
                let timer = timer.clone();
                let spawned = std::thread::Builder::new()
                    .name(format!("plugin-{}-{}", plugin.manifest.name, timer.name))
                    .spawn(move || match timer.every {
                        None => tick(&rt, &plugin, &timer.name),
                        Some(every) => loop {
                            std::thread::sleep(Duration::from_secs(every));
                            tick(&rt, &plugin, &timer.name);
                        },
                    });
                if let Err(e) = spawned {
                    tracing::error!(error = %e, "could not start a plugin timer");
                }
            }
        }
    }
}

fn route_in(rt: &Inner, method: &str, path: &str) -> Option<Matched> {
    let mut best: Option<Matched> = None;
    for plugin in &rt.plugins {
        for route in &plugin.manifest.routes {
            if route.method != method {
                continue;
            }
            if let Some(params) = match_route(&route.path, path) {
                let better = best.as_ref().is_none_or(|b| {
                    literal_segments(&route.path) > literal_segments(&b.route.path)
                });
                if better {
                    best = Some(Matched {
                        plugin: plugin.clone(),
                        route: route.clone(),
                        params,
                    });
                }
            }
        }
    }
    best
}

/// A fresh instance of `plugin` with `secs` to run.
fn instantiate(
    rt: &Arc<Inner>,
    plugin: &Arc<Loaded>,
    secs: u32,
) -> wasmtime::Result<(Store<HostState>, bindings::Plugin)> {
    let limits = StoreLimitsBuilder::new()
        .memory_size(plugin.manifest.limits.memory_mib as usize * 1024 * 1024)
        .instances(4)
        .tables(8)
        .memories(2)
        .build();
    let state = HostState {
        rt: rt.clone(),
        plugin: plugin.clone(),
        table: ResourceTable::new(),
        limits,
        deadline: Instant::now() + Duration::from_secs(u64::from(secs)),
    };
    let mut store = Store::new(&rt.engine, state);
    store.limiter(|s| &mut s.limits);
    let ticks = (u64::from(secs) * 1000) / EPOCH.as_millis() as u64;
    store.set_epoch_deadline(ticks.max(1));
    let instance = plugin.pre.instantiate(&mut store)?;
    Ok((store, instance))
}

fn internal_error() -> Response {
    Response {
        status: 500,
        headers: vec![("content-type".to_string(), "application/json".to_string())],
        body: br#"{"error":"command failed; see the lososd journal"}"#.to_vec(),
    }
}

fn handle_in(rt: &Arc<Inner>, plugin: &Arc<Loaded>, request: &Request) -> Response {
    let secs = plugin.manifest.limits.timeout_secs;
    let result = instantiate(rt, plugin, secs).and_then(|(mut store, instance)| {
        instance
            .losos_plugin_handler()
            .call_handle(&mut store, request)
    });
    match result {
        Ok(resp) => resp,
        Err(e) => {
            tracing::error!(
                plugin = %plugin.manifest.name,
                route = %request.route,
                error = ?e,
                "plugin call failed"
            );
            internal_error()
        }
    }
}

/// One tick of `timer`, unless one is still running.
fn tick(rt: &Arc<Inner>, plugin: &Arc<Loaded>, timer: &str) {
    {
        let mut ticking = plugin.ticking.lock().unwrap_or_else(|e| e.into_inner());
        if !ticking.insert(timer.to_string()) {
            tracing::debug!(plugin = %plugin.manifest.name, timer, "previous tick still running");
            return;
        }
    }
    let secs = plugin.manifest.limits.timeout_secs;
    let result = instantiate(rt, plugin, secs).and_then(|(mut store, instance)| {
        instance
            .losos_plugin_background()
            .call_tick(&mut store, timer)
    });
    if let Err(e) = result {
        tracing::error!(plugin = %plugin.manifest.name, timer, error = ?e, "plugin tick failed");
    }
    plugin
        .ticking
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(timer);
}

/// Start `background.run(kind, input)` on a thread of its own.
fn spawn_job(
    rt: &Arc<Inner>,
    plugin: &Arc<Loaded>,
    kind: String,
    input: Vec<u8>,
) -> Result<(), Error> {
    let limit = plugin.manifest.limits.jobs;
    if plugin
        .jobs
        .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
            (n < limit).then_some(n + 1)
        })
        .is_err()
    {
        return Err(Error::Failed(format!(
            "{limit} jobs of this plugin are already running"
        )));
    }
    let rt = rt.clone();
    let plugin = plugin.clone();
    let name = format!("plugin-{}-job", plugin.manifest.name);
    let spawned = std::thread::Builder::new().name(name).spawn(move || {
        let secs = plugin.manifest.limits.job_secs;
        let result = instantiate(&rt, &plugin, secs).and_then(|(mut store, instance)| {
            instance
                .losos_plugin_background()
                .call_run(&mut store, &kind, &input)
        });
        if let Err(e) = result {
            tracing::error!(plugin = %plugin.manifest.name, kind, error = ?e, "plugin job failed");
        }
        plugin.jobs.fetch_sub(1, Ordering::SeqCst);
    });
    if let Err(e) = spawned {
        // The thread never ran, so it never gave its slot back.
        return Err(Error::Failed(format!("starting the job: {e}")));
    }
    Ok(())
}

/// Route a request a plugin made with `dispatch.call`.
fn dispatch(rt: &Arc<Inner>, request: Request) -> Result<Response, Error> {
    let path = request.path.clone();
    let Some(matched) = route_in(rt, &request.method, &path) else {
        return Ok(Response {
            status: 404,
            headers: vec![("content-type".to_string(), "application/json".to_string())],
            body: br#"{"error":"not found"}"#.to_vec(),
        });
    };
    let authorize = rt
        .authorize
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .ok_or_else(|| Error::Failed("dispatch is not ready".to_string()))?;
    if let Err(why) = authorize(&request.caller, &request.method, &matched.route.path) {
        return Ok(Response {
            status: 403,
            headers: vec![("content-type".to_string(), "application/json".to_string())],
            body: serde_json::to_vec(&serde_json::json!({ "error": why })).unwrap_or_default(),
        });
    }
    let request = Request {
        route: matched.route.path.clone(),
        params: matched.params,
        ..request
    };
    Ok(handle_in(rt, &matched.plugin, &request))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_match_by_segment() {
        assert_eq!(match_route("/api/look", "/api/look"), Some(vec![]));
        assert_eq!(match_route("/api/look", "/api/look/"), None);
        assert_eq!(match_route("/api/look", "/api/looks"), None);
        assert_eq!(
            match_route("/api/look/widgets/{id}", "/api/look/widgets/w12"),
            Some(vec![("id".to_string(), "w12".to_string())])
        );
        assert_eq!(
            match_route("/api/look/widgets/{id}", "/api/look/widgets/a%20b"),
            Some(vec![("id".to_string(), "a b".to_string())])
        );
        assert_eq!(
            match_route("/api/look/widgets/{id}", "/api/look/widgets/"),
            None
        );
        assert_eq!(
            match_route("/api/look/widgets/{id}", "/api/look/widgets/a/b"),
            None
        );
        assert_eq!(match_route("/api/x/{id}", "/api/x/%ff"), None, "not UTF-8");
    }
}
