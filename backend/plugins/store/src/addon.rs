//! Agent addons: community-built MCP servers, each in a rootless container.
//!
//! An addon is a container image that speaks MCP over HTTP (the streamable
//! transport, JSON-RPC in POST bodies) on one port. The box runs it as
//! `notshared` in a user namespace of its own, published on the box's
//! loopback only, and talks to it from here. This module holds what can be
//! decided without touching the box: the manifest a community entry must
//! pass, the gate, the MCP messages and their answers.

use serde_json::{json, Value};

/// Longest addon name: it is the front of every tool name a model sees.
pub const MAX_SLUG: usize = 24;
/// Settings one addon can ask the owner for.
pub const MAX_ENV: usize = 16;
/// The tools an addon may offer; more is cut.
pub const MAX_TOOLS: usize = 64;

/// What an addon's entry in the community index says about it.
#[derive(Debug, Clone, PartialEq)]
pub struct Manifest {
    pub slug: String,
    pub title: String,
    pub summary: String,
    pub author: String,
    pub homepage: Option<String>,
    pub version: String,
    /// `registry/name@sha256:<digest>`: pinned, so what runs is what was
    /// listed, whatever the registry serves under a tag later.
    pub image: String,
    /// The port the server listens on inside the container.
    pub port: u16,
    /// The MCP endpoint's path.
    pub path: String,
    /// Whether it needs to reach beyond the box. Shown before install.
    pub network: bool,
    pub env: Vec<EnvSpec>,
    /// Whether it keeps files, in a folder of its own at `/data`.
    pub data: bool,
    pub memory_mib: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnvSpec {
    pub name: String,
    pub label: String,
    pub secret: bool,
    pub required: bool,
}

fn one_line(v: &Value, key: &str, max: usize) -> Option<String> {
    let s = v.get(key)?.as_str()?.trim();
    (!s.is_empty() && s.chars().count() <= max && !s.chars().any(char::is_control))
        .then(|| s.to_string())
}

fn dns_name(p: &str) -> bool {
    let b = p.as_bytes();
    !b.is_empty()
        && b.len() <= 253
        && b[0].is_ascii_alphanumeric()
        && b.iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'.' | b'-'))
}

fn path_part(p: &str) -> bool {
    let b = p.as_bytes();
    !b.is_empty()
        && b.len() <= 128
        && b[0].is_ascii_alphanumeric()
        && b.iter().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'.' | b'_' | b'-')
        })
}

/// `registry[:port]/path@sha256:<64 hex>` and nothing else.
#[must_use]
pub fn pinned_image(image: &str) -> bool {
    let Some((name, digest)) = image.split_once("@sha256:") else {
        return false;
    };
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f'))
    {
        return false;
    }
    let mut parts = name.split('/');
    let Some(registry) = parts.next() else {
        return false;
    };
    let (host, port) = registry.split_once(':').unwrap_or((registry, ""));
    let rest: Vec<&str> = parts.collect();
    dns_name(host)
        && (host.contains('.') || host == "localhost")
        && (port.is_empty() || port.parse::<u16>().is_ok())
        && !rest.is_empty()
        && rest.len() <= 6
        && rest.iter().all(|p| path_part(p))
}

/// The entry of the index, or the sentence that says why it is not one.
pub fn parse_manifest(v: &Value) -> Result<Manifest, String> {
    let slug = one_line(v, "slug", MAX_SLUG).ok_or("An addon needs a short name (slug).")?;
    if !losos_names::release(&slug) {
        return Err(format!(
            "{slug:?} is not an addon name: small letters, digits and dashes, a letter first."
        ));
    }
    let title =
        one_line(v, "title", 60).ok_or("An addon needs a title of at most 60 characters.")?;
    let summary = one_line(v, "summary", 280).ok_or("An addon needs a one-line summary.")?;
    let author = one_line(v, "author", 80).ok_or("An addon names its author.")?;
    let version = one_line(v, "version", 32).ok_or("An addon names its version.")?;
    let homepage = match v.get("homepage") {
        None | Some(Value::Null) => None,
        Some(h) => Some(
            h.as_str()
                .filter(|h| {
                    h.starts_with("https://") && h.len() <= 300 && !h.contains(char::is_whitespace)
                })
                .ok_or("The homepage must be an https:// link.")?
                .to_string(),
        ),
    };
    let image = one_line(v, "image", 400).ok_or("An addon names its image.")?;
    if !pinned_image(&image) {
        return Err(
            "The image must be pinned to a digest: registry.example/name@sha256:<64 hex digits>."
                .into(),
        );
    }
    let port = match v.get("port") {
        None => 8080,
        Some(p) => p
            .as_u64()
            .and_then(|n| u16::try_from(n).ok())
            .filter(|n| *n >= 1)
            .ok_or("The port must be a number from 1 to 65535.")?,
    };
    let path = match v.get("path") {
        None => "/mcp".to_string(),
        Some(p) => p
            .as_str()
            .filter(|p| {
                p.starts_with('/')
                    && p.len() <= 100
                    && !p.contains("..")
                    && !p.contains("//")
                    && p.bytes()
                        .all(|c| c.is_ascii_alphanumeric() || b"/-_.".contains(&c))
            })
            .ok_or("The path must be a clean path such as /mcp.")?
            .to_string(),
    };
    let network = match v.get("network") {
        None => false,
        Some(Value::String(s)) if s == "none" => false,
        Some(Value::String(s)) if s == "outbound" => true,
        Some(_) => return Err("network is \"none\" or \"outbound\".".into()),
    };
    let mut env = Vec::new();
    for e in v["env"].as_array().into_iter().flatten() {
        let name = one_line(e, "name", 128).ok_or("A setting needs a name.")?;
        if !super::hub::env_name(&name) {
            return Err(format!("{name} is not a setting name."));
        }
        if env.iter().any(|x: &EnvSpec| x.name == name) {
            return Err(format!("{name} is listed twice."));
        }
        env.push(EnvSpec {
            label: one_line(e, "label", 80).unwrap_or_else(|| name.clone()),
            secret: e["secret"].as_bool() == Some(true),
            required: e["required"].as_bool() == Some(true),
            name,
        });
    }
    if env.len() > MAX_ENV {
        return Err(format!("An addon can ask for at most {MAX_ENV} settings."));
    }
    let memory_mib = match v.get("memoryMiB") {
        None => 256,
        Some(m) => m
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .filter(|n| (32..=2048).contains(n))
            .ok_or("memoryMiB is from 32 to 2048.")?,
    };
    Ok(Manifest {
        slug,
        title,
        summary,
        author,
        homepage,
        version,
        image,
        port,
        path,
        network,
        env,
        data: v["data"].as_bool() == Some(true),
        memory_mib,
    })
}

impl Manifest {
    /// The entry as the pages show it.
    #[must_use]
    pub fn view(&self) -> Value {
        json!({
            "slug": self.slug,
            "title": self.title,
            "summary": self.summary,
            "author": self.author,
            "homepage": self.homepage,
            "version": self.version,
            "image": self.image,
            "network": if self.network { "outbound" } else { "none" },
            "data": self.data,
            "memoryMiB": self.memory_mib,
            "env": self.env.iter().map(|e| json!({
                "name": e.name, "label": e.label, "secret": e.secret, "required": e.required,
            })).collect::<Vec<_>>(),
        })
    }
}

/// The community index: every entry that passes, and how many did not.
#[must_use]
pub fn parse_index(body: &[u8]) -> (Vec<Manifest>, usize) {
    let Ok(doc) = serde_json::from_slice::<Value>(body) else {
        return (Vec::new(), 0);
    };
    let mut out: Vec<Manifest> = Vec::new();
    let mut refused = 0;
    for entry in doc["addons"].as_array().into_iter().flatten() {
        match parse_manifest(entry) {
            Ok(m) if !out.iter().any(|o| o.slug == m.slug) => out.push(m),
            _ => refused += 1,
        }
    }
    (out, refused)
}

/// The host port for a new addon: the first of `low..=high` no addon uses.
#[must_use]
pub fn pick_port(low: u16, high: u16, used: &[u16]) -> Option<u16> {
    (low..=high).find(|p| !used.contains(p))
}

// ── MCP ───────────────────────────────────────────────────────────────────

pub const PROTOCOL: &str = "2025-03-26";

#[must_use]
pub fn initialize_body() -> Value {
    json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {
            "protocolVersion": PROTOCOL,
            "capabilities": {},
            "clientInfo": { "name": "losos", "version": "1" },
        },
    })
}

#[must_use]
pub fn initialized_body() -> Value {
    json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })
}

#[must_use]
pub fn tools_list_body() -> Value {
    json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {} })
}

#[must_use]
pub fn tools_call_body(tool: &str, arguments: &Value) -> Value {
    json!({
        "jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": { "name": tool, "arguments": arguments },
    })
}

/// The JSON-RPC answer with `id` out of an HTTP body, which a server sends
/// as plain JSON or as an event stream. `Ok` holds the `result`; an error
/// object, or no answer, is the `Err` sentence.
pub fn rpc_result(body: &[u8], id: u64) -> Result<Value, String> {
    let text = String::from_utf8_lossy(body);
    let mut candidates: Vec<Value> = Vec::new();
    let trimmed = text.trim();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        if let Ok(v) = serde_json::from_str::<Value>(trimmed) {
            candidates.push(v);
        }
    } else {
        for line in text.lines() {
            if let Some(data) = line.strip_prefix("data:") {
                if let Ok(v) = serde_json::from_str::<Value>(data.trim()) {
                    candidates.push(v);
                }
            }
        }
    }
    let messages = candidates.into_iter().flat_map(|v| match v {
        Value::Array(list) => list,
        one => vec![one],
    });
    for m in messages {
        if m["id"] == json!(id) {
            if let Some(e) = m.get("error") {
                let msg = e["message"].as_str().unwrap_or("the addon refused");
                return Err(msg.chars().filter(|c| !c.is_control()).take(300).collect());
            }
            return m
                .get("result")
                .cloned()
                .ok_or_else(|| "the addon answered with nothing".to_string());
        }
    }
    Err("the addon did not answer as an MCP server".into())
}

fn tool_name(n: &str) -> bool {
    !n.is_empty()
        && n.len() <= 64
        && n.bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.'))
}

/// The tools of a `tools/list` result, those with a name a model can use.
#[must_use]
pub fn parse_tools(result: &Value) -> Vec<Value> {
    result["tools"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|t| {
            let name = t["name"].as_str().filter(|n| tool_name(n))?;
            let description: String = t["description"]
                .as_str()
                .unwrap_or_default()
                .chars()
                .filter(|c| !c.is_control() || *c == '\n')
                .take(1000)
                .collect();
            let schema = if t["inputSchema"].is_object() {
                t["inputSchema"].clone()
            } else {
                json!({ "type": "object", "properties": {} })
            };
            Some(json!({ "name": name, "description": description, "inputSchema": schema }))
        })
        .take(MAX_TOOLS)
        .collect()
}

/// The name a model sees: `addon_<slug>__<tool>`.
#[must_use]
pub fn model_name(slug: &str, tool: &str) -> String {
    format!("addon_{slug}__{tool}")
}

/// Back from a model's name to the addon and its tool.
#[must_use]
pub fn split_model_name(name: &str) -> Option<(&str, &str)> {
    let rest = name.strip_prefix("addon_")?;
    let (slug, tool) = rest.split_once("__")?;
    (losos_names::release(slug) && tool_name(tool)).then_some((slug, tool))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn entry() -> Value {
        json!({
            "slug": "web-fetch", "title": "Web fetch", "summary": "Reads a web page.",
            "author": "Ada", "version": "1.2.0",
            "homepage": "https://example.org/fetch",
            "image": format!("ghcr.io/ada/mcp-fetch@sha256:{DIGEST}"),
            "network": "outbound",
            "env": [{ "name": "API_KEY", "label": "Key", "secret": true, "required": true }],
        })
    }

    #[test]
    fn a_good_entry_has_its_defaults() {
        let m = parse_manifest(&entry()).unwrap();
        assert_eq!((m.port, m.path.as_str(), m.memory_mib), (8080, "/mcp", 256));
        assert!(m.network && !m.data);
        assert_eq!(m.env[0].name, "API_KEY");
        assert_eq!(m.view()["network"], "outbound");
    }

    #[test]
    fn a_bad_entry_is_refused_with_a_reason() {
        let tweak = |k: &str, v: Value| {
            let mut e = entry();
            e[k] = v;
            parse_manifest(&e).unwrap_err()
        };
        assert!(tweak("slug", json!("Web_Fetch")).contains("not an addon name"));
        assert!(tweak("slug", json!("x".repeat(25))).contains("short name"));
        assert!(tweak("image", json!("ghcr.io/ada/mcp:latest")).contains("pinned"));
        assert!(tweak("image", json!(format!("ada/mcp@sha256:{DIGEST}"))).contains("pinned"));
        assert!(tweak(
            "image",
            json!(format!("ghcr.io/ada/mcp@sha256:{}", DIGEST.to_uppercase()))
        )
        .contains("pinned"));
        assert!(tweak("homepage", json!("http://example.org")).contains("https"));
        assert!(tweak("path", json!("/../etc")).contains("path"));
        assert!(tweak("network", json!("host")).contains("outbound"));
        assert!(tweak("memoryMiB", json!(1)).contains("memoryMiB"));
        assert!(tweak("env", json!([{ "name": "1X" }])).contains("setting name"));
        assert!(tweak("env", json!([{ "name": "A" }, { "name": "A" }])).contains("twice"));
        assert!(tweak("title", json!("a\nb")).contains("title"));
    }

    #[test]
    fn pinned_images_are_exact() {
        for ok in [
            format!("ghcr.io/ada/mcp@sha256:{DIGEST}"),
            format!("docker.io/library/nginx@sha256:{DIGEST}"),
            format!("localhost:5000/x@sha256:{DIGEST}"),
            format!("quay.io/a/b/c@sha256:{DIGEST}"),
        ] {
            assert!(pinned_image(&ok), "{ok}");
        }
        for bad in [
            format!("ghcr.io/ada/mcp@sha256:{}", &DIGEST[1..]),
            format!("ghcr.io/ADA/mcp@sha256:{DIGEST}"),
            format!("ghcr.io/../mcp@sha256:{DIGEST}"),
            format!("ghcr.io//mcp@sha256:{DIGEST}"),
            format!("ghcr.io/ada/mcp:1@sha256:{DIGEST}"),
            format!("-x.io/a@sha256:{DIGEST}"),
            "ghcr.io/ada/mcp".to_string(),
        ] {
            assert!(!pinned_image(&bad), "{bad}");
        }
    }

    #[test]
    fn the_index_keeps_good_entries_and_counts_the_rest() {
        let doc = json!({ "addons": [entry(), { "slug": "bad" }, entry()] });
        let (list, refused) = parse_index(doc.to_string().as_bytes());
        assert_eq!(list.len(), 1);
        assert_eq!(refused, 2, "a bad entry and a repeated name");
        assert_eq!(parse_index(b"nope"), (Vec::new(), 0));
    }

    #[test]
    fn ports_are_taken_in_order() {
        assert_eq!(pick_port(41000, 41002, &[41000, 41002]), Some(41001));
        assert_eq!(pick_port(41000, 41001, &[41000, 41001]), None);
    }

    #[test]
    fn answers_come_as_json_or_as_an_event_stream() {
        let plain = br#"{"jsonrpc":"2.0","id":2,"result":{"tools":[]}}"#;
        assert_eq!(rpc_result(plain, 2).unwrap(), json!({ "tools": [] }));
        let sse = b"event: message\ndata: {\"jsonrpc\":\"2.0\",\"method\":\"notifications/progress\"}\n\nevent: message\ndata: {\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"ok\":true}}\n\n";
        assert_eq!(rpc_result(sse, 2).unwrap(), json!({ "ok": true }));
        let err = br#"{"jsonrpc":"2.0","id":2,"error":{"code":-32602,"message":"bad\u0007 args"}}"#;
        assert_eq!(rpc_result(err, 2).unwrap_err(), "bad args");
        assert!(rpc_result(plain, 3).is_err(), "someone else's answer");
        assert!(rpc_result(b"<html>", 2).is_err());
    }

    #[test]
    fn tools_and_their_names() {
        let tools = parse_tools(&json!({ "tools": [
            { "name": "fetch", "description": "Get a page", "inputSchema": { "type": "object", "properties": { "url": {} } } },
            { "name": "no schema" },
            { "name": "plain" },
        ] }));
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0]["inputSchema"]["properties"]["url"], json!({}));
        assert_eq!(tools[1]["inputSchema"]["type"], "object");
        assert_eq!(model_name("web-fetch", "fetch"), "addon_web-fetch__fetch");
        assert_eq!(
            split_model_name("addon_web-fetch__fetch"),
            Some(("web-fetch", "fetch"))
        );
        for bad in ["fetch", "addon_x", "addon_Web__x", "addon_x__a b"] {
            assert_eq!(split_model_name(bad), None, "{bad}");
        }
    }
}
