//! A plugin's manifest: `<name>.json` beside `<name>.wasm`.
//!
//! The manifest is the plugin's whole grant. It names the routes the plugin
//! answers and every resource its imports may touch: file roots, programs,
//! unit names, HTTP endpoints, locks. The host refuses anything else
//! whatever the plugin asks (`super::host`). It sits in the same read-only
//! directory as the component, so it is as trusted as the code it grants:
//! both come out of the same Nix build.
//!
//! Paths and endpoints may name lososd's environment as `${VAR}` or
//! `${VAR:-default}`, which is how a test, or a box with the feature off,
//! moves or drops them. An entry that expands to nothing is dropped; a
//! `${VAR}` with no default whose variable is unset refuses the plugin.

use anyhow::{bail, Context};
use serde::Deserialize;
use std::collections::BTreeMap;

/// Body size a route accepts when its manifest entry does not say.
pub const DEFAULT_MAX_BODY: usize = 256 * 1024;
/// What the global actix limit lets through; no route may ask for more.
pub const MAX_BODY_CEILING: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Raw {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    routes: Vec<RawRoute>,
    #[serde(default)]
    files: Vec<RawFileRoot>,
    #[serde(default)]
    exec: Vec<RawExec>,
    #[serde(default)]
    units: Option<RawUnits>,
    #[serde(default)]
    http: Vec<String>,
    #[serde(default)]
    locks: Vec<String>,
    #[serde(default)]
    timers: Vec<Timer>,
    #[serde(default)]
    dispatch: bool,
    #[serde(default)]
    limits: Limits,
    #[serde(default)]
    config: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawRoute {
    method: String,
    path: String,
    #[serde(default)]
    max_body: Option<usize>,
    #[serde(default)]
    unguarded: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawFileRoot {
    path: String,
    access: Access,
    #[serde(default)]
    chown: Vec<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum RawExec {
    Name(String),
    Rule {
        program: String,
        #[serde(default)]
        uids: Vec<u32>,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawUnits {
    prefix: String,
    programs: Vec<String>,
}

/// Read only, or read and write.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Access {
    Ro,
    Rw,
}

/// One route the plugin answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    /// Upper case.
    pub method: String,
    /// `/api/look/widgets/{id}`: literal segments and `{name}` ones.
    pub path: String,
    pub max_body: usize,
    /// lososd does not authenticate the call; nginx in front still does.
    /// For the one thing a browser cannot send a token with, a CSS
    /// background picture.
    pub unguarded: bool,
}

/// A file or directory the plugin may reach, and how.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRoot {
    pub path: String,
    pub access: Access,
    /// Uids a written file may be handed to.
    pub chown: Vec<u32>,
}

/// A program the plugin may run, and as whom besides root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecRule {
    /// A bare name looked up on lososd's PATH, or an absolute path.
    pub program: String,
    pub uids: Vec<u32>,
}

/// The transient units the plugin may start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Units {
    /// Every unit name starts with this. Always `losos-…-`.
    pub prefix: String,
    pub programs: Vec<String>,
}

/// A timer: `background.tick(name)` every `every` seconds, or once at the
/// start when `every` is absent.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Timer {
    pub name: String,
    #[serde(default)]
    pub every: Option<u64>,
}

/// What one call of the plugin may spend.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Limits {
    /// Linear memory, MiB.
    #[serde(default = "default_memory")]
    #[serde(rename = "memoryMiB")]
    pub memory_mib: u32,
    /// Wall clock for a request or a tick, seconds.
    #[serde(default = "default_timeout")]
    pub timeout_secs: u32,
    /// Wall clock for a job, seconds.
    #[serde(default = "default_job_secs")]
    pub job_secs: u32,
    /// Jobs of this plugin running at once.
    #[serde(default = "default_jobs")]
    pub jobs: u32,
}

fn default_memory() -> u32 {
    64
}
fn default_timeout() -> u32 {
    30
}
fn default_job_secs() -> u32 {
    600
}
fn default_jobs() -> u32 {
    2
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            memory_mib: default_memory(),
            timeout_secs: default_timeout(),
            job_secs: default_job_secs(),
            jobs: default_jobs(),
        }
    }
}

/// A manifest with its `${VAR}`s filled in and its rules checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub name: String,
    pub description: String,
    pub routes: Vec<Route>,
    pub files: Vec<FileRoot>,
    pub exec: Vec<ExecRule>,
    pub units: Option<Units>,
    pub http: Vec<String>,
    pub locks: Vec<String>,
    pub timers: Vec<Timer>,
    pub dispatch: bool,
    pub limits: Limits,
    pub config: BTreeMap<String, String>,
}

/// A plugin name, a lock name: lower-case letters, digits and `-`.
pub fn is_name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 48
        && s.bytes().next().is_some_and(|b| b.is_ascii_lowercase())
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// An absolute path with no `.`, `..`, empty segment or trailing `/`.
pub fn is_clean_path(p: &str) -> bool {
    p.starts_with('/')
        && p.len() > 1
        && !p.ends_with('/')
        && !p.contains('\0')
        && p[1..]
            .split('/')
            .all(|seg| !seg.is_empty() && seg != "." && seg != "..")
}

/// Where `${VAR}` in a manifest is looked up: the daemon's environment,
/// or a map in tests.
pub type Env<'a> = &'a dyn Fn(&str) -> Option<String>;

/// Fill in `${VAR}` and `${VAR:-default}` from `env`.
pub fn expand(s: &str, env: Env<'_>) -> anyhow::Result<String> {
    let mut out = String::new();
    let mut rest = s;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let end = after
            .find('}')
            .with_context(|| format!("unclosed ${{ in {s:?}"))?;
        let inner = &after[..end];
        let (var, default) = match inner.split_once(":-") {
            Some((v, d)) => (v, Some(d)),
            None => (inner, None),
        };
        if var.is_empty()
            || !var
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
        {
            bail!("{var:?} in {s:?} is not an environment variable name");
        }
        match (env(var).filter(|v| !v.is_empty()), default) {
            (Some(v), _) => out.push_str(&v),
            (None, Some(d)) => out.push_str(d),
            (None, None) => bail!("{s:?} needs ${var}, which lososd's environment does not set"),
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

impl Manifest {
    /// Parse `json`, expand it against `env` and check every rule. `stem`
    /// is the file name the manifest was found under, which must be its
    /// `name`.
    pub fn parse(json: &[u8], stem: &str, env: Env<'_>) -> anyhow::Result<Manifest> {
        let raw: Raw = serde_json::from_slice(json).context("the manifest is not valid")?;
        if !is_name(&raw.name) {
            bail!("{:?} is not a plugin name", raw.name);
        }
        if raw.name != stem {
            bail!(
                "the manifest names {:?} but sits beside {stem}.wasm",
                raw.name
            );
        }

        let mut routes = Vec::new();
        for r in raw.routes {
            let method = r.method.to_ascii_uppercase();
            if !matches!(method.as_str(), "GET" | "POST" | "PUT" | "DELETE" | "PATCH") {
                bail!("route {} {}: unknown method", r.method, r.path);
            }
            if !(r.path.starts_with("/api/") || r.path.starts_with("/internal/"))
                || !is_route_pattern(&r.path)
            {
                bail!("route {}: not an /api/ or /internal/ path pattern", r.path);
            }
            let max_body = r.max_body.unwrap_or(DEFAULT_MAX_BODY);
            if max_body > MAX_BODY_CEILING {
                bail!("route {}: maxBody is over {MAX_BODY_CEILING}", r.path);
            }
            if r.unguarded && method != "GET" {
                bail!("route {method} {}: only a GET may be unguarded", r.path);
            }
            let route = Route {
                method,
                path: r.path,
                max_body,
                unguarded: r.unguarded,
            };
            if routes
                .iter()
                .any(|x: &Route| x.method == route.method && x.path == route.path)
            {
                bail!("route {} {} is listed twice", route.method, route.path);
            }
            routes.push(route);
        }

        let mut files = Vec::new();
        for f in raw.files {
            let path = expand(&f.path, env)?;
            if path.is_empty() {
                continue;
            }
            if !is_clean_path(&path) {
                bail!("file root {path:?} is not a clean absolute path");
            }
            if path.split('/').filter(|s| !s.is_empty()).count() < 2 {
                bail!("file root {path:?} is too close to /");
            }
            files.push(FileRoot {
                path,
                access: f.access,
                chown: f.chown,
            });
        }

        let mut exec = Vec::new();
        for e in raw.exec {
            let (program, uids) = match e {
                RawExec::Name(p) => (p, Vec::new()),
                RawExec::Rule { program, uids } => (program, uids),
            };
            let program = expand(&program, env)?;
            if program.is_empty() {
                continue;
            }
            check_program(&program)?;
            exec.push(ExecRule { program, uids });
        }

        let units = match raw.units {
            None => None,
            Some(u) => {
                if !(u.prefix.starts_with("losos-") && u.prefix.ends_with('-'))
                    || !u
                        .prefix
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
                {
                    bail!("units prefix {:?} must be losos-…-", u.prefix);
                }
                let mut programs = Vec::new();
                for p in u.programs {
                    let p = expand(&p, env)?;
                    if p.is_empty() {
                        continue;
                    }
                    check_program(&p)?;
                    programs.push(p);
                }
                Some(Units {
                    prefix: u.prefix,
                    programs,
                })
            }
        };

        let mut http = Vec::new();
        for h in raw.http {
            let h = expand(&h, env)?;
            if h.is_empty() {
                continue;
            }
            check_endpoint(&h)?;
            http.push(h);
        }

        for l in &raw.locks {
            if !is_name(l) {
                bail!("{l:?} is not a lock name");
            }
        }
        for t in &raw.timers {
            if !is_name(&t.name) {
                bail!("{:?} is not a timer name", t.name);
            }
            if t.every == Some(0) {
                bail!("timer {}: every must be at least 1 second", t.name);
            }
        }
        if raw.limits.memory_mib == 0 || raw.limits.memory_mib > 1024 {
            bail!("limits.memoryMiB must be 1 to 1024");
        }
        if raw.limits.timeout_secs == 0 || raw.limits.job_secs == 0 || raw.limits.jobs == 0 {
            bail!("limits must be at least 1");
        }

        let mut config = BTreeMap::new();
        for (k, v) in raw.config {
            config.insert(k, expand(&v, env)?);
        }

        Ok(Manifest {
            name: raw.name,
            description: raw.description,
            routes,
            files,
            exec,
            units,
            http,
            locks: raw.locks,
            timers: raw.timers,
            dispatch: raw.dispatch,
            limits: raw.limits,
            config,
        })
    }
}

/// `/api/a/{b}/c`: literal segments of URL-safe characters and whole-segment
/// `{name}`s.
/// A pattern may end in one `/`, the index of a prefix (`/api/basic/`); no
/// other segment may be empty.
fn is_route_pattern(p: &str) -> bool {
    let p = p.strip_suffix('/').unwrap_or(p);
    p[1..].split('/').all(|seg| {
        if let Some(name) = seg.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
            !name.is_empty() && name.bytes().all(|b| b.is_ascii_lowercase() || b == b'_')
        } else {
            !seg.is_empty()
                && seg != "."
                && seg != ".."
                && seg
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        }
    })
}

/// A program is a bare name, or a clean absolute path. Never a shell or a
/// way back to arbitrary commands: those would turn one grant into root.
fn check_program(p: &str) -> anyhow::Result<()> {
    const NEVER: &[&str] = &[
        "sh",
        "bash",
        "dash",
        "zsh",
        "env",
        "sudo",
        "su",
        "runuser",
        "setpriv",
        "nsenter",
        "systemd-run",
        "machinectl",
        "busybox",
        "xargs",
        "find",
        "perl",
        "python3",
        "python",
    ];
    let base = p.rsplit('/').next().unwrap_or(p);
    if NEVER.contains(&base) {
        bail!("{p:?} would run anything; it cannot be granted");
    }
    let ok = if p.contains('/') {
        is_clean_path(p)
    } else {
        !p.is_empty()
            && p.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.+".contains(&b))
    };
    if !ok {
        bail!("{p:?} is not a program name or a clean absolute path");
    }
    Ok(())
}

/// `http://host[:port]`, `https://host[:port]`, `https://*` or
/// `unix:/path.sock`.
fn check_endpoint(e: &str) -> anyhow::Result<()> {
    if let Some(sock) = e.strip_prefix("unix:") {
        if !is_clean_path(sock) {
            bail!("{e:?}: the socket must be a clean absolute path");
        }
        return Ok(());
    }
    if e == "https://*" {
        return Ok(());
    }
    let rest = e
        .strip_prefix("http://")
        .or_else(|| e.strip_prefix("https://"))
        .with_context(|| format!("{e:?} is not an http(s) origin or unix: socket"))?;
    if rest.is_empty()
        || rest.contains('/')
        || rest.contains('@')
        || !rest
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-.:[]".contains(&b))
    {
        bail!("{e:?} must be an origin: scheme, host and port, nothing more");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(var: &str) -> Option<String> {
        match var {
            "LOSOS_STATE_DIR" => Some("/tmp/state".to_string()),
            "EMPTY" => Some(String::new()),
            _ => None,
        }
    }

    fn parse(json: &str) -> anyhow::Result<Manifest> {
        Manifest::parse(json.as_bytes(), "look", &env)
    }

    #[test]
    fn a_manifest_is_expanded_and_checked() {
        let m = parse(
            r#"{
                "name": "look",
                "routes": [
                    {"method": "get", "path": "/api/look"},
                    {"method": "PUT", "path": "/api/look/background", "maxBody": 9000000},
                    {"method": "GET", "path": "/api/look/background", "unguarded": true},
                    {"method": "DELETE", "path": "/api/look/widgets/{id}"}
                ],
                "files": [
                    {"path": "${LOSOS_STATE_DIR:-/var/lib/losos}/look.json", "access": "rw"},
                    {"path": "${UNSET:-}", "access": "ro"}
                ],
                "exec": ["git", {"program": "/run/current-system/sw/bin/occ", "uids": [1000]}],
                "units": {"prefix": "losos-backup-", "programs": ["${EMPTY:-}", "/nix/store/x-backup/bin/backup"]},
                "http": ["http://127.0.0.1:11434", "unix:/run/losos-search/cloud.sock", "${UNSET:-}"],
                "locks": ["look"],
                "timers": [{"name": "start"}, {"name": "sync", "every": 30}],
                "config": {"lookFile": "${LOSOS_STATE_DIR}/look.json"}
            }"#,
        )
        .unwrap();
        assert_eq!(m.routes[0].method, "GET");
        assert_eq!(m.routes[0].max_body, DEFAULT_MAX_BODY);
        assert_eq!(m.routes[1].max_body, 9_000_000);
        assert!(m.routes[2].unguarded);
        assert_eq!(
            m.files.len(),
            1,
            "an entry that expands to nothing is dropped"
        );
        assert_eq!(m.files[0].path, "/tmp/state/look.json");
        assert_eq!(m.exec[1].uids, [1000]);
        assert_eq!(
            m.units.as_ref().unwrap().programs,
            ["/nix/store/x-backup/bin/backup"]
        );
        assert_eq!(m.http.len(), 2);
        assert_eq!(m.config["lookFile"], "/tmp/state/look.json");
        assert_eq!(m.limits, Limits::default());
    }

    #[test]
    fn statistics_loads_on_a_box_without_traefik() {
        // Traefik's URL is set only where apps.nix runs; a box without it
        // must still start lososd, and the plugin then says there is
        // nothing to count.
        let json = include_bytes!("../../plugins/statistics/manifest.json");
        let none = |_: &str| None;
        let m = Manifest::parse(json, "statistics", &none).unwrap();
        assert!(m.http.is_empty());
        assert_eq!(m.config["traefik"], "");
        let set = |v: &str| (v == "LOSOS_TRAEFIK_URL").then(|| "http://127.0.0.1:8089".to_string());
        let m = Manifest::parse(json, "statistics", &set).unwrap();
        assert_eq!(m.http, ["http://127.0.0.1:8089"]);
        assert_eq!(m.config["traefik"], "http://127.0.0.1:8089");
    }

    #[test]
    fn what_cannot_be_granted_refuses_the_plugin() {
        for bad in [
            // A name that is not the file's.
            r#"{"name": "other"}"#,
            // Unknown keys are typos, not features.
            r#"{"name": "look", "filez": []}"#,
            r#"{"name": "look", "routes": [{"method": "GET", "path": "/etc/passwd"}]}"#,
            r#"{"name": "look", "routes": [{"method": "GET", "path": "/api/../x"}]}"#,
            r#"{"name": "look", "routes": [{"method": "GET", "path": "/api//x"}]}"#,
            r#"{"name": "look", "routes": [{"method": "GET", "path": "/api/x//"}]}"#,
            r#"{"name": "look", "routes": [{"method": "POST", "path": "/api/x", "unguarded": true}]}"#,
            r#"{"name": "look", "routes": [{"method": "GET", "path": "/api/x"}, {"method": "GET", "path": "/api/x"}]}"#,
            r#"{"name": "look", "routes": [{"method": "GET", "path": "/api/x", "maxBody": 999999999999}]}"#,
            r#"{"name": "look", "files": [{"path": "/", "access": "rw"}]}"#,
            r#"{"name": "look", "files": [{"path": "/var", "access": "rw"}]}"#,
            r#"{"name": "look", "files": [{"path": "/var/lib/../etc", "access": "ro"}]}"#,
            r#"{"name": "look", "files": [{"path": "relative/x", "access": "ro"}]}"#,
            r#"{"name": "look", "files": [{"path": "${NOT_SET}/x", "access": "ro"}]}"#,
            r#"{"name": "look", "exec": ["sh"]}"#,
            r#"{"name": "look", "exec": ["/bin/bash"]}"#,
            r#"{"name": "look", "exec": ["systemd-run"]}"#,
            r#"{"name": "look", "exec": ["git;rm"]}"#,
            r#"{"name": "look", "units": {"prefix": "nixos-", "programs": []}}"#,
            r#"{"name": "look", "units": {"prefix": "losos-x", "programs": []}}"#,
            r#"{"name": "look", "http": ["http://127.0.0.1:8082/api"]}"#,
            r#"{"name": "look", "http": ["ftp://x"]}"#,
            r#"{"name": "look", "http": ["http://user@x"]}"#,
            r#"{"name": "look", "locks": ["State"]}"#,
            r#"{"name": "look", "timers": [{"name": "x", "every": 0}]}"#,
            r#"{"name": "look", "limits": {"memoryMiB": 0}}"#,
        ] {
            assert!(parse(bad).is_err(), "{bad} should be refused");
        }
    }

    #[test]
    fn a_route_may_end_in_one_slash() {
        assert!(is_route_pattern("/api/basic/"));
        assert!(is_route_pattern("/api/basic/{page}"));
        for bad in ["/api//", "/api/x//", "/api//x", "/api/./"] {
            assert!(!is_route_pattern(bad), "{bad:?}");
        }
    }

    #[test]
    fn expansion() {
        assert_eq!(
            expand("a${LOSOS_STATE_DIR}b", &env).unwrap(),
            "a/tmp/stateb"
        );
        assert_eq!(expand("${UNSET:-x/y}", &env).unwrap(), "x/y");
        assert_eq!(expand("${EMPTY:-d}", &env).unwrap(), "d");
        assert!(expand("${UNSET}", &env).is_err());
        assert!(expand("${lower}", &env).is_err());
        assert!(expand("${OPEN", &env).is_err());
        assert_eq!(expand("plain", &env).unwrap(), "plain");
    }

    #[test]
    fn clean_paths() {
        assert!(is_clean_path("/var/lib/losos"));
        for bad in ["", "/", "var", "/a/", "/a//b", "/a/./b", "/a/../b", "/a\0"] {
            assert!(!is_clean_path(bad), "{bad:?}");
        }
    }
}
