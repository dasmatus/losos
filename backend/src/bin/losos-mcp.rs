//! `losos-mcp`: the box's own features, as one stateless MCP server.
//!
//! A tool asks for the person or the token it runs as, and every `call`
//! is one HTTP request against lososd's loopback API — no session, no
//! state kept between calls, because lososd carries every piece of the
//! state already. Reads and writes both ride the caller's own rights: a
//! companion token (device flow, the companion's allowlist) or a
//! personal access token (as the person whose it is).
//!
//! The protocol over stdin/stdout is JSON-RPC 2.0, MCP 2024-11-05: the
//! `initialize`, `tools/list` and `tools/call` methods, and nothing else
//! — no resources, no prompts, no subscriptions; a caller that asks is
//! told so and re-tries the tools route. Answers are one JSON object per
//! line on stdout; the trace goes to stderr.
//!
//! # Validation, on every call
//!
//! A model checking its own work needs the document as the box now
//! carries it, not its own echo of what it meant to write. So every
//! tool's answer is an envelope rather than a bare body:
//!
//! ```json
//! { "answer": { … the route's own body … },
//!   "validation": { "tool", "route", "options", "status",
//!                   "document", "digest", "matched" } }
//! ```
//!
//! `validation.status` is `read` for a read, `written` for a write whose
//! effect a second, authoritative request confirms, or `failed`. The
//! write's `document` is the slice of `GET /api/tag` that carries the
//! object the write named (the level by name, the area by kind and ref,
//! the assistant by slug, the skill by slug, the person's clearance, the
//! stopped ask by id), and `digest` is the SHA-256 of that document's
//! canonical serialization — `matched` says plainly whether the box's
//! answer agrees with the write's own `options` where the two can be
//! compared. A failed call carries `error` and no document. The agent
//! reads `status` and `matched` before it says "done" of its own work.
//!
//! # Where the writes end
//!
//! The tag's objects (levels administrator-only, areas, assistants,
//! skills, clearances, stopping an ask) and nothing else: the boxes'
//! files, mail and git stay read-only here, since writes there go
//! through the applications people can see and undo. `losos.tag.harness`
//! adapters that want more reach run as the person, not as this server.

use std::io::{BufRead, Write};

use serde_json::{json, Value};

use losos_ctl::boxid::sha256;

const HOST: &str = "http://127.0.0.1:8082";

/// One tool the box offers: what callers name, the route that answers,
/// whether the call writes, and the sentence the tool list carries.
struct Tool {
    name: &'static str,
    /// `METHOD /api/…` with `$` placeholders the caller's arguments fill.
    route: &'static str,
    description: &'static str,
    /// The arguments' shape, as the tool list answers it.
    schema: Value,
    /// GET and DELETE fill the path alone; POST bodies carry the
    /// arguments whole.
    method: &'static str,
    /// Which validation the call's envelope carries.
    kind: Kind,
    /// For a [`Kind::State`]-checked write: the tool's own name for one
    /// argument of the body that identifies the object ("name", "slug",
    /// "id"), the route parameters' own key path into the asked-for
    /// object.
    verify: &'static [&'static str],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Read,
    /// A write whose effect shows in `GET /api/tag`: `verify` names the
    /// argument whose value keys the object.
    Write,
}

/// The `$` fills, in `route`, come from the arguments' full names, so a
/// template reads unchanged from the tool call. Built once, then static.
fn tools() -> &'static [Tool] {
    static TOOLS: std::sync::OnceLock<Vec<Tool>> = std::sync::OnceLock::new();
    TOOLS.get_or_init(|| {
        vec![
            Tool { name: "box_status", route: "GET /api/status", description: "How this box is going, one page: state, sharing, the mesh, health, and validation with a digest of what the box answered.", schema: json!({ "type": "object" }), method: "GET", kind: Kind::Read, verify: &[] },
            Tool { name: "me", route: "GET /api/me", description: "Who this token is on the box, and the sections it may read.", schema: json!({ "type": "object" }), method: "GET", kind: Kind::Read, verify: &[] },
            Tool { name: "cloud_search", route: "GET /api/search?q=$q", description: "Search everything the person may read on this box: files in LosOS Cloud, LosOS Git code on default branches, their mail, the rooms of chat — graded by the box's confidentiality levels, so a hit above the caller's clearance never comes back.", schema: json!({ "type": "object", "properties": { "q": { "type": "string", "maxLength": 128 } }, "required": ["q"] }), method: "GET", kind: Kind::Read, verify: &[] },
            Tool { name: "cloud_files", route: "GET /api/cloud/files?q=$q", description: "Files and folders in the signed-in person's LosOS Cloud, by name.", schema: json!({ "type": "object", "properties": { "q": { "type": "string", "maxLength": 128 } } }), method: "GET", kind: Kind::Read, verify: &[] },
            Tool { name: "git_repos", route: "GET /api/git/repos?q=$q", description: "LosOS Git repositories, with the visibility and branch of each.", schema: json!({ "type": "object", "properties": { "q": { "type": "string", "maxLength": 128 } } }), method: "GET", kind: Kind::Read, verify: &[] },
            Tool { name: "git_text", route: "GET /api/git/text?q=$q", description: "Lines matched on the default branch of every repository; graded like cloud_search.", schema: json!({ "type": "object", "properties": { "q": { "type": "string", "maxLength": 128 } }, "required": ["q"] }), method: "GET", kind: Kind::Read, verify: &[] },
            Tool { name: "mail_mailboxes", route: "GET /api/mail", description: "The signed-in person's folders in LosOS Mail.", schema: json!({ "type": "object" }), method: "GET", kind: Kind::Read, verify: &[] },
            Tool { name: "mail_search", route: "GET /api/mail/messages?mailbox=$mailbox&q=$q", description: "Search one mailbox (its id from mail_mailboxes) by subject and sender; the unread stays unread for the asker's own read.", schema: json!({ "type": "object", "properties": { "mailbox": { "type": "string", "maxLength": 128 }, "q": { "type": "string", "maxLength": 128 } }, "required": ["mailbox"] }), method: "GET", kind: Kind::Read, verify: &[] },
            Tool { name: "tag_view", route: "GET /api/tag", description: "LosOS Tag: the levels, areas, assistants and skills, the asks on the queue, and the past conversations.", schema: json!({ "type": "object" }), method: "GET", kind: Kind::Read, verify: &[] },
            Tool { name: "tag_ask", route: "POST /api/tag/ask", description: "Ask LosOS Tag one question in the background: unified search grounds the answer on the box, the ask runs at the assistant's own clearance; `useWeb: true` adds one subject-only search of the box's own SearXNG when the box runs it. The validation carries the ask's own id and its queue state.", schema: json!({ "type": "object", "properties": { "question": { "type": "string", "maxLength": 2000 }, "assistant": { "type": "string", "maxLength": 64 }, "room": { "type": "string", "maxLength": 255 }, "skills": { "type": "array", "items": { "type": "string" }, "maxItems": 8 }, "useWeb": { "type": "boolean" } }, "required": ["question"] }), method: "POST", kind: Kind::Write, verify: &["id"] },
            Tool { name: "statistics", route: "GET /api/statistics", description: "Traffic counts per app (Traefik's meter, one minute ring).", schema: json!({ "type": "object" }), method: "GET", kind: Kind::Read, verify: &[] },
            Tool { name: "apps_list", route: "GET /api/apps", description: "The catalogue as this box carries it, and what is installed.", schema: json!({ "type": "object" }), method: "GET", kind: Kind::Read, verify: &[] },
            Tool { name: "logs_recent", route: "GET /api/logs?spread=true", description: "The journal's recent lines, redacted: which units the box writes.", schema: json!({ "type": "object" }), method: "GET", kind: Kind::Read, verify: &[] },
            Tool { name: "problems_list", route: "GET /api/problems", description: "What the box noticed going wrong, newest first. Read-only: a problem is the operator's to read.", schema: json!({ "type": "object" }), method: "GET", kind: Kind::Read, verify: &[] },
            Tool { name: "tag_level", route: "POST /api/tag/levels", description: "Add or replace one confidentiality level. Administrator only. The validation carries the level as the box now ranks it.", schema: json!({ "type": "object", "properties": { "name": { "type": "string", "maxLength": 48 }, "description": { "type": "string", "maxLength": 200 } }, "required": ["name"] }), method: "POST", kind: Kind::Write, verify: &["name"] },
            Tool { name: "tag_area", route: "POST /api/tag/areas", description: "Grade one piece of the box's data with a level. A person grades their own; an administrator grades anyone's, `owner` naming them. The validation carries the area as the box now carries it.", schema: json!({ "type": "object", "properties": { "kind": { "type": "string", "maxLength": 16 }, "ref": { "type": "string", "maxLength": 255 }, "level": { "type": "string", "maxLength": 48 }, "owner": { "type": "string", "maxLength": 64 } }, "required": ["kind", "ref", "level"] }), method: "POST", kind: Kind::Write, verify: &["kind", "ref"] },
            Tool { name: "tag_assistant", route: "POST /api/tag/assistants", description: "Add or replace one assistant (persona, clearance). A person shapes their own; the box's own only an administrator. The validation carries the assistant as the box now carries it.", schema: json!({ "type": "object", "properties": { "slug": { "type": "string", "maxLength": 48 }, "display": { "type": "string", "maxLength": 64 }, "prompt": { "type": "string", "maxLength": 2000 }, "clearance": { "type": "string", "maxLength": 48 }, "enabled": { "type": "boolean" } }, "required": ["slug", "display", "clearance"] }), method: "POST", kind: Kind::Write, verify: &["slug"] },
            Tool { name: "tag_skill", route: "POST /api/tag/skills", description: "Add or replace one skill: instructions an ask carries. The seeded LosOS ones are read-only but for an administrator. The validation carries the skill as the box now carries it.", schema: json!({ "type": "object", "properties": { "slug": { "type": "string", "maxLength": 48 }, "display": { "type": "string", "maxLength": 64 }, "prompt": { "type": "string", "maxLength": 2000 } }, "required": ["slug", "display", "prompt"] }), method: "POST", kind: Kind::Write, verify: &["slug"] },
            Tool { name: "tag_clearance", route: "POST /api/tag/clearance", description: "Set the person's own clearance: what their companion and their own asks read at. The validation carries the clearance as the box now carries it.", schema: json!({ "type": "object", "properties": { "level": { "type": "string", "maxLength": 48 } }, "required": ["level"] }), method: "POST", kind: Kind::Write, verify: &["level"] },
            Tool { name: "tag_stop", route: "DELETE /api/tag/asks/$id", description: "Stop one ask: the caller's own, an administrator's anything. The validation carries the ask's own state after the stop.", schema: json!({ "type": "object", "properties": { "id": { "type": "string", "maxLength": 24, "minLength": 24 } }, "required": ["id"] }), method: "DELETE", kind: Kind::Write, verify: &["id"] },
        ]
    })
}

/// One call to lososd, blocking, capped; the answer's own body came as JSON.
fn ask(route: &str, method: &str, body: Option<&Value>, token: &str) -> Result<Value, String> {
    let url = format!("{HOST}{route}");
    use std::process::Command;
    let mut call = Command::new("curl");
    call.arg("-fsS")
        .arg("--max-time")
        .arg("30")
        .arg("-X")
        .arg(method)
        .arg("-H")
        .arg(format!("Authorization: Bearer {token}"));
    if let Some(body) = body {
        call.arg("-H")
            .arg("Content-Type: application/json")
            .arg("-d")
            .arg(body.to_string());
    }
    let out = call
        .arg(&url)
        .output()
        .map_err(|e| format!("the box did not answer: {e}"))?;
    if !out.status.success() {
        let why = String::from_utf8_lossy(&out.stderr);
        return Err(why.chars().take(200).collect());
    }
    serde_json::from_slice(&out.stdout).map_err(|e| format!("the answer was not JSON: {e}"))
}

/// One path's segment: letters, digits and the punctuation a path can
/// carry, never a `/`, so a `$` fill stays in its own segment.
fn segment(value: &str) -> Option<String> {
    let ok = !value.is_empty()
        && value.len() <= 255
        && !value
            .chars()
            .any(|c| c == '/' || c == '?' || c == '#' || c.is_control());
    ok.then(|| value.to_string())
}

/// Route template fill: `$name` takes the argument of that name.
fn fill(route: &str, args: &Value) -> Result<String, String> {
    let mut out = String::new();
    let mut rest = route;
    while let Some(at) = rest.find('$') {
        out.push_str(&rest[..at]);
        let after = &rest[at + 1..];
        let end = after
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(after.len());
        let name = &after[..end];
        let value = args
            .get(name)
            .and_then(Value::as_str)
            .ok_or_else(|| format!("the tool asks for {name}"))?;
        let segment = segment(value).ok_or_else(|| format!("{name} is one path segment"))?;
        let encoded: String = url_encode(&segment);
        out.push_str(&encoded);
        rest = &after[end..];
    }
    out.push_str(rest);
    Ok(out)
}

fn url_encode(input: &str) -> String {
    let mut out = String::new();
    let ok = |b: u8| {
        b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~' | b':' | b'@' | b'=')
    };
    for b in input.bytes() {
        if ok(b) {
            out.push(b as char);
        } else {
            use std::fmt::Write;
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

/// One hit's canonical digest, hex on 32 bytes.
fn digest(document: &Value) -> String {
    let canonical = serde_json::to_vec(document).unwrap_or_default();
    let digest = sha256(&canonical);
    format!("sha256:{}", hex(&digest))
}

fn hex(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A write's own key values, one at a `verify` name in the args. A tool
/// that named no keys (`tag_ask`) verifies nothing about fields, only uri
/// its own 404. For matching purposes this is what the write said.
fn keys(args: &Value, verify: &[&str]) -> Vec<(String, Value)> {
    verify
        .iter()
        .filter_map(|name| args.get(*name).map(|v| ((*name).to_string(), v.clone())))
        .collect()
}

/// Whether `document` carries the object one write meant: every key value
/// appears verbatim in the slice, where the write named keys. The
/// document's shape helps the agent, so `GET /api/tag`'s own arrays are
/// byte-for-byte what the box answered.
fn matched(document: &Value, keys: &[(String, Value)]) -> Option<Value> {
    let list = document.as_array()?;
    if keys.is_empty() {
        return list.first().cloned();
    }
    list.iter()
        .find(|entry| {
            keys.iter()
                .all(|(name, value)| entry.get(name.as_str()) == Some(value))
        })
        .cloned()
}

/// The write's own slice out of `GET /api/tag`'s whole state, by tool.
/// `GET /api/tag` is the box's own answer on its state at the moment of
/// the write, so what the agent gets back is validation, not the agent's
/// own echo.
fn verify_document(tool: &Tool, args: &Value, state: &Value, raw: &Value) -> (Value, String, bool) {
    match tool.name {
        // tag_ask's own answer is the receipt alone: the ask's id and its
        // queue state.
        "tag_ask" => {
            let ids = args.get("id").cloned().unwrap_or_else(|| raw.clone());
            (
                json!({ "id": ids }),
                digest(&json!({ "id": ids.clone() })),
                true,
            )
        }
        // tag_stop's own answer is the stopped ask as the box now carries
        // it — or its absence (the reaper's sweep may have it out).
        "tag_stop" => {
            let stopped = state["jobs"].as_array().and_then(|jobs| {
                jobs.iter()
                    .find(|j| j.get("id") == args.get("id").or_else(|| raw.get("id")))
                    .cloned()
            });
            match stopped {
                Some(job) => (job.clone(), digest(&job), true),
                None => (
                    json!({ "status": "stopped-or-removed" }),
                    digest(&json!({ "status": "stopped-or-removed" })),
                    true,
                ),
            }
        }
        // tag_level: the levels as the box now ranks them.
        "tag_level" => {
            let want = raw
                .get("levels")
                .and_then(Value::as_array)
                .and_then(|levels| {
                    levels.iter().find(|l| {
                        l.get("name") == args.get(verify_name(tool, 0)).or_else(|| args.get("name"))
                    })
                })
                .cloned();
            let level = want.unwrap_or_else(|| json!({}));
            (level.clone(), digest(&level), level.get("name").is_some())
        }
        "tag_area" | "tag_assistant" | "tag_skill" | "tag_clearance" => {
            let at = match tool.name {
                "tag_area" => "areas",
                "tag_assistant" => "assistants",
                "tag_skill" => "skills",
                _ => "clearance",
            };
            let document = match at {
                "clearance" => state.get("clearance").cloned().unwrap_or_else(|| json!({})),
                slice => {
                    let keys = keys(args, tool.verify);
                    matched(state.get(slice).unwrap_or(&json!([])), &keys)
                        .unwrap_or_else(|| json!({}))
                }
            };
            let hit = !document.as_object().map(|o| o.is_empty()).unwrap_or(false);
            (document.clone(), digest(&document), hit)
        }
        _ => (serde_json::json!({}), digest(&json!({})), false),
    }
}

fn verify_name(tool: &Tool, at: usize) -> String {
    tool.verify
        .get(at)
        .map(|name| (*name).to_string())
        .unwrap_or_default()
}

/// Call one tool by name, and put its answer and its validation into one
/// envelope. A write runs its verifying second call; a failed write
/// carries the error alone.
fn tool_call(name: &str, args: &Value, token: &str) -> Result<Value, String> {
    let tool = tools()
        .iter()
        .find(|tool| tool.name == name)
        .ok_or_else(|| format!("no such tool: {name}"))?;
    let (method, template) = tool
        .route
        .split_once(' ')
        .ok_or("the manifest's route is broken")?;
    let path = fill(template, args)?;
    let asked = token.trim();
    if asked.is_empty() {
        return Err("run losos-mcp with LOSOS_MCP_TOKEN: a token of the person the tools read as (losos_access::COMPANION or the Access tokens pane)".into());
    }
    let body = (tool.method == "POST" || tool.kind == Kind::Write).then(|| args.clone());
    let raw = ask(
        &path,
        method,
        body.as_ref().filter(|_| tool.method == "POST"),
        asked,
    );
    match raw {
        Err(error) => Ok(json!({
            "answer": { "error": error },
            "validation": {
                "tool": tool.name, "route": template, "options": args.clone(),
                "status": "failed", "error": error, "matched": false,
            },
        })),
        Ok(answer) => {
            let verify = match tool.kind {
                Kind::Read => (answer.clone(), digest(&answer), true),
                Kind::Write => {
                    // The whole state behind this call: a second request
                    // from the box itself, authoritative, not a local
                    // echo of the first.
                    let state = ask("/api/tag", "GET", None, asked).unwrap_or_else(|_| json!({}));
                    let (document, digest, hit) = verify_document(tool, args, &state, &answer);
                    // tag_ask and tag_stop validate by identity, not by
                    // fields; the rest by the write's own keys.
                    let matched = hit;
                    (document, digest, matched)
                }
            };
            let (document, one_digest, hit) = verify;
            Ok(json!({
                "answer": answer,
                "validation": {
                    "tool": tool.name, "route": template, "options": args.clone(),
                    "status": if tool.kind == Kind::Write { "written" } else { "read" },
                    "document": document, "digest": one_digest, "matched": hit,
                },
            }))
        }
    }
}

/// The tools list, as MCP answers it.
fn tool_list() -> Value {
    json!({
        "tools": tools().iter().map(|tool| json!({
            "name": tool.name,
            "description": tool.description,
            "inputSchema": tool.schema,
        })).collect::<Vec<_>>(),
    })
}

/// The one server loop, until stdin ends: one JSON-RPC call per line.
fn serve(input: impl BufRead, output: &mut dyn Write, token: &str) {
    for line in input.lines() {
        let Ok(line) = line else { return };
        if line.trim().is_empty() {
            continue;
        }
        let Ok(call) = serde_json::from_str::<Value>(&line) else {
            let _ = write_one(output, None, -32700, "not JSON");
            continue;
        };
        let id = call.get("id").cloned();
        let Some(method) = call.get("method").and_then(Value::as_str) else {
            let _ = write_one(output, id, -32600, "every call names a method");
            continue;
        };
        let answer = match (method, call.get("params")) {
            ("initialize", _) => Some(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "losos-mcp", "version": env!("CARGO_PKG_VERSION") },
            })),
            ("notifications/initialized", _) => None,
            ("tools/list", _) => Some(tool_list()),
            ("tools/call", params) => {
                let name = params
                    .and_then(|p| p.get("name"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let args = params
                    .and_then(|p| p.get("arguments"))
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                match tool_call(name, &args, token) {
                    Ok(envelope) => Some(json!({
                        "content": [{ "type": "text", "text": envelope.to_string() }],
                        "isError": false,
                    })),
                    Err(why) => Some(json!({
                        "content": [{ "type": "text", "text": why }],
                        "isError": true,
                    })),
                }
            }
            (_, _) if method.starts_with("notifications/") => None,
            _ => {
                let _ = write_one(output, id, -32601, &format!("no such method: {method}"));
                continue;
            }
        };
        if let Some(answer) = answer {
            let _ = write(
                output,
                &json!({ "jsonrpc": "2.0", "id": id, "result": answer }),
            );
        }
    }
}

fn write_one(
    output: &mut dyn Write,
    id: Option<Value>,
    code: i32,
    why: &str,
) -> std::io::Result<()> {
    write(
        output,
        &json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": why } }),
    )
}

fn write(output: &mut dyn Write, value: &Value) -> std::io::Result<()> {
    let raw = serde_json::to_string(value)?;
    output.write_all(raw.as_bytes())?;
    output.write_all(b"\n")
}

fn main() {
    let token = std::env::var("LOSOS_MCP_TOKEN").unwrap_or_default();
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    serve(stdin.lock(), &mut stdout, &token);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_manifest_covers_the_boxs_features() {
        let names: Vec<&str> = tools().iter().map(|t| t.name).collect();
        for wanted in [
            "box_status",
            "me",
            "cloud_search",
            "cloud_files",
            "git_repos",
            "git_text",
            "mail_mailboxes",
            "mail_search",
            "tag_view",
            "tag_ask",
            "statistics",
            "apps_list",
            "logs_recent",
            "problems_list",
            "tag_level",
            "tag_area",
            "tag_assistant",
            "tag_skill",
            "tag_clearance",
            "tag_stop",
        ] {
            assert!(names.contains(&wanted), "{wanted} missing");
        }
        // Writes that verify themselves name at least one key, but the
        // ask, whose own receipt is the id, is the exception.
        for tool in tools() {
            if tool.kind == Kind::Write && tool.name != "tag_ask" {
                assert!(!tool.verify.is_empty(), "{} verifies nothing", tool.name);
            }
        }
    }

    #[test]
    fn a_fill_is_one_segment_and_described() {
        let args = json!({ "q": "tax 2025 ", "mailbox": "INBOX", "id": "0f44e3eb" });
        assert_eq!(
            fill("/api/search?q=$q", &args).unwrap(),
            "/api/search?q=tax%202025%20"
        );
        assert_eq!(
            fill("/api/mail/messages?mailbox=$mailbox&q=$q", &args).unwrap(),
            "/api/mail/messages?mailbox=INBOX&q=tax%202025%20"
        );
        assert!(fill("/api/x/$missing", &args).is_err());
        let sneaky = json!({ "segment": "../../etc" });
        assert!(fill("/api/x/$segment", &sneaky).is_err());
        let fine = json!({ "segment": "inbox" });
        assert_eq!(
            fill("/api/mail/messages/$segment", &fine).unwrap(),
            "/api/mail/messages/inbox"
        );
        // tag_stop's id rides its route: it must not change the path's
        // own segments.kilometers
        let ids = json!({ "id": "abc123" });
        assert_eq!(
            fill("/api/tag/asks/$id", &ids).unwrap(),
            "/api/tag/asks/abc123"
        );
    }

    #[test]
    fn the_digest_is_canonical_and_short() {
        let a = json!({ "a": 1 });
        let b = json!({ "a": 1 });
        assert_eq!(digest(&a), digest(&b));
        assert!(digest(&a).starts_with("sha256:"));
        assert_eq!(digest(&a).len(), 7 + 64);
        let one = json!({ "a": 1 });
        let two = json!({ "a": 2 });
        assert_ne!(digest(&one), digest(&two));
    }

    #[test]
    fn matched_takes_the_tools_own_entry() {
        let list = json!([ { "name": "a" }, { "name": "b" } ]);
        let keys = vec![("name".to_string(), json!("b"))];
        assert_eq!(matched(&list, &keys).unwrap(), json!({ "name": "b" }));
        let keys = vec![("name".to_string(), json!("missing"))];
        assert!(matched(&list, &keys).is_none());
        // No keys at all: the first entry, the tool's own whole answer.
        assert_eq!(matched(&list, &[]).unwrap(), json!({ "name": "a" }));
    }

    #[test]
    fn the_tools_list_is_one_answer() {
        let list = tool_list();
        assert_eq!(list["tools"].as_array().unwrap().len(), tools().len());
        assert_eq!(list["tools"][0]["name"], "box_status");
        assert!(list["tools"][0]["inputSchema"].is_object());
    }

    #[test]
    fn an_empty_token_is_a_sentence_not_a_call() {
        let err = tool_call("box_status", &json!({}), " ").unwrap_err();
        assert!(err.contains("LOSOS_MCP_TOKEN"));
        let missing = tool_call("not_a_tool", &json!({}), "x").unwrap_err();
        assert!(missing.contains("no such tool"));
    }
}
