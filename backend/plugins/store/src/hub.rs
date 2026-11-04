//! Docker Hub: what the search finds, which tags an image has, what the
//! image says about itself, and the Compose file that runs it.
//!
//! Everything here is pure: URLs are built, answers parsed and the project
//! made without a request. The plugin fetches, and lososd's own shaping
//! (`backend/src/compose.rs`, `x-losos.isolated`) has the last word on
//! what the file may ask for.

use serde_json::{json, Map, Value};

/// Rows a search returns.
pub const SEARCH_ROWS: usize = 20;
/// Tags the picker offers.
pub const TAG_ROWS: usize = 25;
/// The registry every image here comes from, written out in full so the
/// box never guesses a registry from a short name.
pub const REGISTRY: &str = "docker.io";

/// Percent-encode everything but RFC 3986's unreserved set.
#[must_use]
pub fn encode(raw: &str) -> String {
    let mut out = String::new();
    for b in raw.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// A search term worth sending: two to 128 characters, no control characters.
pub fn query(raw: &str) -> Result<&str, &'static str> {
    let q = raw.trim();
    let n = q.chars().count();
    if n < 2 {
        return Err("Search for at least 2 characters.");
    }
    if n > 128 {
        return Err("That search is longer than 128 characters.");
    }
    if q.chars().any(char::is_control) {
        return Err("Remove the line breaks and control characters.");
    }
    Ok(q)
}

fn name_part(p: &str) -> bool {
    let b = p.as_bytes();
    !b.is_empty()
        && b.len() <= 64
        && b[0].is_ascii_alphanumeric()
        && b[b.len() - 1].is_ascii_alphanumeric()
        && b.iter().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'.' | b'_' | b'-')
        })
        && !p.contains("..")
}

/// A Docker Hub repository, `name` (an official image) or `owner/name`,
/// as `(owner, name)` with `library` for an official image.
pub fn repository(image: &str) -> Result<(String, String), &'static str> {
    let image = image.trim();
    let bad = "That is not a Docker Hub image name.";
    let (owner, name) = match image.split_once('/') {
        None => ("library", image),
        Some((owner, name)) => (owner, name),
    };
    if !name_part(owner) || !name_part(name) || name.contains('/') {
        return Err(bad);
    }
    Ok((owner.to_string(), name.to_string()))
}

/// An image tag: letters, digits, `_`, `.` and `-`, at most 128.
#[must_use]
pub fn tag_ok(tag: &str) -> bool {
    let b = tag.as_bytes();
    !b.is_empty()
        && b.len() <= 128
        && (b[0].is_ascii_alphanumeric() || b[0] == b'_')
        && b.iter()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'.' | b'-'))
}

/// The reference the box pulls: always the registry in full.
pub fn reference(image: &str, tag: &str) -> Result<String, &'static str> {
    let (owner, name) = repository(image)?;
    if !tag_ok(tag) {
        return Err("That is not an image tag.");
    }
    Ok(format!("{REGISTRY}/{owner}/{name}:{tag}"))
}

#[must_use]
pub fn search_url(base: &str, q: &str) -> String {
    format!(
        "{base}/v2/search/repositories/?query={}&page_size={SEARCH_ROWS}",
        encode(q)
    )
}

#[must_use]
pub fn tags_url(base: &str, owner: &str, name: &str) -> String {
    format!(
        "{base}/v2/repositories/{owner}/{name}/tags/?page_size={TAG_ROWS}&ordering=last_updated"
    )
}

fn text(v: &Value, key: &str, max: usize) -> Option<String> {
    let s = v.get(key)?.as_str()?.trim();
    (!s.is_empty()).then(|| s.chars().filter(|c| !c.is_control()).take(max).collect())
}

fn count(v: &Value, key: &str) -> u64 {
    v.get(key).and_then(Value::as_u64).unwrap_or(0)
}

/// Hub's search answer as rows. A row without a usable name is dropped, not
/// guessed at.
#[must_use]
pub fn parse_search(body: &[u8]) -> Vec<Value> {
    let Ok(doc) = serde_json::from_slice::<Value>(body) else {
        return Vec::new();
    };
    let mut rows = Vec::new();
    for r in doc["results"].as_array().into_iter().flatten() {
        let Some(repo) = text(r, "repo_name", 140).or_else(|| text(r, "name", 140)) else {
            continue;
        };
        let Ok((owner, name)) = repository(&repo) else {
            continue;
        };
        let official = r["is_official"].as_bool() == Some(true) || owner == "library";
        rows.push(json!({
            "image": if owner == "library" { name.clone() } else { format!("{owner}/{name}") },
            "name": name,
            "publisher": if official { "Docker official image".to_string() } else { owner },
            "official": official,
            "summary": text(r, "short_description", 240),
            "stars": count(r, "star_count"),
            "pulls": count(r, "pull_count"),
            "homepage": format!("https://hub.docker.com/{}", if official { format!("_/{name}") } else { format!("r/{repo}") }),
        }));
    }
    rows
}

/// Tags a Linux x86-64 box can run, newest first, as Hub orders them.
#[must_use]
pub fn parse_tags(body: &[u8]) -> Vec<Value> {
    let Ok(doc) = serde_json::from_slice::<Value>(body) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for t in doc["results"].as_array().into_iter().flatten() {
        let Some(name) = t["name"].as_str().filter(|n| tag_ok(n)) else {
            continue;
        };
        let images = t["images"].as_array();
        // A tag with no platform list is shown; one with a list must have ours.
        let runs_here = images.is_none_or(|list| {
            list.is_empty()
                || list.iter().any(|i| {
                    i["architecture"] == "amd64" && i["os"].as_str().is_none_or(|o| o == "linux")
                })
        });
        if runs_here {
            out.push(json!({ "name": name, "updated": text(t, "last_updated", 40) }));
        }
    }
    out
}

/// What an image's configuration blob says about how to run it.
#[must_use]
pub fn parse_inspect(config: &[u8]) -> Value {
    let doc: Value = serde_json::from_slice(config).unwrap_or(Value::Null);
    let c = &doc["config"];
    let mut ports: Vec<u16> = c["ExposedPorts"]
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(k, _)| k.strip_suffix("/tcp"))
        .filter_map(|p| p.parse().ok())
        .collect();
    ports.sort_unstable();
    let mut volumes: Vec<&str> = c["Volumes"]
        .as_object()
        .into_iter()
        .flatten()
        .map(|(k, _)| k.as_str())
        .filter(|v| volume_target(v))
        .collect();
    volumes.sort_unstable();
    // Names only: an image's default values can be secrets of its maker's.
    let env: Vec<&str> = c["Env"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter_map(|e| e.split_once('=').map(|(k, _)| k))
        .filter(|k| env_name(k) && !matches!(*k, "PATH" | "HOME" | "HOSTNAME" | "TERM"))
        .collect();
    json!({
        "ports": ports,
        "volumes": volumes,
        "env": env,
        "user": c["User"].as_str().filter(|u| !u.is_empty()),
        "architecture": doc["architecture"],
    })
}

/// A variable name an environment accepts here.
#[must_use]
pub fn env_name(name: &str) -> bool {
    let b = name.as_bytes();
    !b.is_empty()
        && b.len() <= 128
        && (b[0].is_ascii_alphabetic() || b[0] == b'_')
        && b.iter().all(|c| c.is_ascii_alphanumeric() || *c == b'_')
}

/// A path inside the container an app keeps its data at.
#[must_use]
pub fn volume_target(path: &str) -> bool {
    path.starts_with('/')
        && path.len() <= 200
        && path != "/"
        && !path.contains("..")
        && !path.contains("//")
        && path.bytes().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, b'/' | b'.' | b'_' | b'-' | b'+' | b'@')
        })
}

/// What the owner picked, parsed from the install request.
#[derive(Debug, Clone, PartialEq)]
pub struct Install {
    pub release: String,
    pub title: String,
    pub reference: String,
    pub ports: Vec<u16>,
    pub primary: Option<u16>,
    pub env: Vec<(String, String)>,
    pub volumes: Vec<String>,
}

/// The install request: `{image, tag?, release?, title?, ports?, primaryPort?,
/// env?, volumes?}`.
pub fn parse_install(body: &Value) -> Result<Install, String> {
    let image = body["image"].as_str().unwrap_or_default();
    let tag = body["tag"].as_str().map_or("latest", str::trim);
    let reference = reference(image, tag).map_err(str::to_string)?;
    let (_, name) = repository(image).map_err(str::to_string)?;
    let release = match body["release"]
        .as_str()
        .map(str::trim)
        .filter(|r| !r.is_empty())
    {
        Some(r) => r.to_string(),
        None => release_of(&name),
    };
    if !losos_names::release(&release) {
        return Err(
            "Name the app with small letters, digits and dashes, starting with a letter.".into(),
        );
    }
    let title = match body["title"]
        .as_str()
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        Some(t) if t.chars().count() <= 80 && !t.contains(['\n', '\0']) => t.to_string(),
        Some(_) => return Err("The title must be one line of at most 80 characters.".into()),
        None => name.clone(),
    };
    let mut ports = Vec::new();
    for p in body["ports"].as_array().into_iter().flatten() {
        let port = p
            .as_u64()
            .and_then(|n| u16::try_from(n).ok())
            .filter(|n| *n >= 1)
            .ok_or("A port is a number from 1 to 65535.")?;
        if !ports.contains(&port) {
            ports.push(port);
        }
    }
    if ports.len() > 8 {
        return Err("An app can publish at most 8 ports.".into());
    }
    let primary = match body.get("primaryPort") {
        None | Some(Value::Null) => None,
        Some(p) => {
            let port = p
                .as_u64()
                .and_then(|n| u16::try_from(n).ok())
                .ok_or("The main port is a number from 1 to 65535.")?;
            if !ports.contains(&port) {
                return Err("The main port must be one of the app's ports.".into());
            }
            Some(port)
        }
    };
    let mut env = Vec::new();
    if let Some(map) = body["env"].as_object() {
        if map.len() > 64 {
            return Err("An app can have at most 64 settings.".into());
        }
        for (k, v) in map {
            let v = v.as_str().ok_or_else(|| format!("{k} must be text."))?;
            if !env_name(k) {
                return Err(format!("{k} is not a setting name."));
            }
            if v.len() > 4096 || v.contains('\0') {
                return Err(format!("The value of {k} is too long or holds a NUL."));
            }
            env.push((k.clone(), v.to_string()));
        }
    }
    let mut volumes = Vec::new();
    for v in body["volumes"].as_array().into_iter().flatten() {
        let v = v.as_str().unwrap_or_default();
        if !volume_target(v) {
            return Err(format!("{v:?} is not a folder inside the app."));
        }
        if !volumes.iter().any(|x| x == v) {
            volumes.push(v.to_string());
        }
    }
    if volumes.len() > 8 {
        return Err("An app can keep data in at most 8 folders.".into());
    }
    Ok(Install {
        release,
        title,
        reference,
        ports,
        primary,
        env,
        volumes,
    })
}

/// A repository name as an app name: lower case, runs of other characters
/// one dash, starting with a letter.
#[must_use]
pub fn release_of(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let mut out = out.trim_end_matches('-').to_string();
    if !out.starts_with(|c: char| c.is_ascii_lowercase()) {
        out.insert_str(0, "app-");
    }
    out.truncate(40);
    out.trim_end_matches('-').to_string()
}

/// The body of `POST /api/apps/compose` for an install.
#[must_use]
pub fn compose_request(i: &Install) -> Value {
    let mut service = Map::new();
    service.insert("image".into(), json!(i.reference));
    if !i.ports.is_empty() {
        service.insert(
            "ports".into(),
            Value::Array(i.ports.iter().map(|p| json!(p.to_string())).collect()),
        );
    }
    if !i.env.is_empty() {
        let env: Map<String, Value> = i.env.iter().map(|(k, v)| (k.clone(), json!(v))).collect();
        service.insert("environment".into(), Value::Object(env));
    }
    let mut volumes = Map::new();
    let mut mounts = Vec::new();
    for (n, target) in i.volumes.iter().enumerate() {
        let name = format!("data{}", if n == 0 { String::new() } else { n.to_string() });
        mounts.push(json!(format!("{name}:{target}")));
        volumes.insert(name, json!({}));
    }
    if !mounts.is_empty() {
        service.insert("volumes".into(), Value::Array(mounts));
    }
    let mut meta = json!({ "isolated": true, "title": i.title });
    if let Some(p) = i.primary.or_else(|| i.ports.first().copied()) {
        meta["port"] = json!(p);
    }
    let mut doc = json!({
        "x-losos": meta,
        "services": { i.release.clone(): Value::Object(service) },
    });
    if !volumes.is_empty() {
        doc["volumes"] = Value::Object(volumes);
    }
    json!({ "release": i.release, "title": i.title, "compose": doc })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_tags() {
        assert_eq!(
            repository("nginx").unwrap(),
            ("library".into(), "nginx".into())
        );
        assert_eq!(
            repository("bitnami/redis").unwrap(),
            ("bitnami".into(), "redis".into())
        );
        for bad in [
            "",
            "A/b",
            "a/b/c",
            "../x",
            "a b",
            "-x",
            "x-",
            "docker.io/library/nginx",
        ] {
            assert!(repository(bad).is_err(), "{bad:?}");
        }
        assert_eq!(
            reference("nginx", "1.27-alpine").unwrap(),
            "docker.io/library/nginx:1.27-alpine"
        );
        for bad in ["", "-x", "a b", "a@sha256:00", "x".repeat(129).as_str()] {
            assert!(reference("nginx", bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn queries_are_encoded_and_bounded() {
        assert!(query("a").is_err());
        assert!(query("a\nb").is_err());
        assert!(query(&"a".repeat(129)).is_err());
        assert_eq!(query(" redis ").unwrap(), "redis");
        assert_eq!(
            search_url("https://hub.docker.com", "a b&c"),
            "https://hub.docker.com/v2/search/repositories/?query=a%20b%26c&page_size=20"
        );
    }

    #[test]
    fn search_rows() {
        let rows = parse_search(
            br#"{"count":3,"results":[
            {"repo_name":"nginx","short_description":"Official build","star_count":21000,"pull_count":10000000000,"is_official":true},
            {"repo_name":"bitnami/redis","short_description":"","star_count":3,"pull_count":5,"is_official":false},
            {"repo_name":"BAD NAME"},
            {"short_description":"nameless"}]}"#,
        );
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["image"], "nginx");
        assert_eq!(rows[0]["publisher"], "Docker official image");
        assert_eq!(rows[0]["pulls"], 10_000_000_000_u64);
        assert_eq!(rows[1]["image"], "bitnami/redis");
        assert_eq!(rows[1]["publisher"], "bitnami");
        assert_eq!(rows[1]["summary"], Value::Null);
        assert_eq!(
            rows[1]["homepage"],
            "https://hub.docker.com/r/bitnami/redis"
        );
        assert!(parse_search(b"{not json").is_empty());
    }

    #[test]
    fn tags_that_run_here() {
        let tags = parse_tags(
            br#"{"results":[
            {"name":"1.27","last_updated":"2026-10-01T00:00:00Z","images":[{"architecture":"arm64","os":"linux"},{"architecture":"amd64","os":"linux"}]},
            {"name":"arm-only","images":[{"architecture":"arm64","os":"linux"}]},
            {"name":"win","images":[{"architecture":"amd64","os":"windows"}]},
            {"name":"latest"},
            {"name":"bad tag"}]}"#,
        );
        let names: Vec<&str> = tags.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(names, ["1.27", "latest"]);
    }

    #[test]
    fn what_an_image_says_about_itself() {
        let v = parse_inspect(
            br#"{"architecture":"amd64","config":{"User":"1001",
            "ExposedPorts":{"8080/tcp":{},"53/udp":{},"80/tcp":{}},
            "Volumes":{"/data":{},"/":{},"/a/../b":{}},
            "Env":["PATH=/usr/bin","POSTGRES_PASSWORD=","APP_MODE=prod","9BAD=x"]}}"#,
        );
        assert_eq!(v["ports"], json!([80, 8080]));
        assert_eq!(v["volumes"], json!(["/data"]));
        assert_eq!(v["env"], json!(["POSTGRES_PASSWORD", "APP_MODE"]));
        assert_eq!(v["user"], "1001");
        assert_eq!(parse_inspect(b"nope")["ports"], json!([]));
    }

    #[test]
    fn an_install_becomes_an_isolated_compose_file() {
        let i = parse_install(&json!({
            "image": "bitnami/redis", "tag": "7.4", "title": "Redis",
            "ports": [6379], "primaryPort": 6379,
            "env": { "ALLOW_EMPTY_PASSWORD": "yes" },
            "volumes": ["/bitnami/redis/data", "/extra"],
        }))
        .unwrap();
        assert_eq!(i.release, "redis");
        let req = compose_request(&i);
        assert_eq!(req["release"], "redis");
        let doc = &req["compose"];
        assert_eq!(
            doc["x-losos"],
            json!({ "isolated": true, "title": "Redis", "port": 6379 })
        );
        let svc = &doc["services"]["redis"];
        assert_eq!(svc["image"], "docker.io/bitnami/redis:7.4");
        assert_eq!(svc["ports"], json!(["6379"]));
        assert_eq!(
            svc["volumes"],
            json!(["data:/bitnami/redis/data", "data1:/extra"])
        );
        assert_eq!(doc["volumes"], json!({ "data": {}, "data1": {} }));
        assert_eq!(svc["environment"]["ALLOW_EMPTY_PASSWORD"], "yes");
    }

    #[test]
    fn a_bad_install_says_what_is_wrong() {
        for (body, word) in [
            (json!({}), "image"),
            (json!({ "image": "nginx", "tag": "a b" }), "tag"),
            (
                json!({ "image": "nginx", "release": "Bad_Name" }),
                "small letters",
            ),
            (json!({ "image": "nginx", "ports": [0] }), "port"),
            (
                json!({ "image": "nginx", "ports": [80], "primaryPort": 81 }),
                "main port",
            ),
            (
                json!({ "image": "nginx", "env": { "1X": "a" } }),
                "setting name",
            ),
            (json!({ "image": "nginx", "env": { "A": 1 } }), "text"),
            (json!({ "image": "nginx", "volumes": ["../etc"] }), "folder"),
            (json!({ "image": "nginx", "title": "a\nb" }), "title"),
        ] {
            let e = parse_install(&body).unwrap_err();
            assert!(e.contains(word), "{body} -> {e}");
        }
    }

    #[test]
    fn app_names_from_image_names() {
        assert_eq!(release_of("nginx"), "nginx");
        assert_eq!(release_of("my.App_server"), "my-app-server");
        assert_eq!(release_of("7zip"), "app-7zip");
        assert!(losos_names::release(&release_of("0__x")));
    }
}
