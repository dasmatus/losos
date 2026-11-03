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
//! The toolset is the box's own surface at the shape a harness reads it:
//! one read and, where the feature has one, one write per feature
//! (backend/sections.json decides what the token may call; the manifest
//! below only mirrors it). T3 Code and OpenCode get this server as their
//! MCP source, whose access is through the box's own sign-in, so a tag's
//! ask runs with tools rather than one prompt.

use serde_json::{json, Value};
use std::io::{BufRead, Write};

const HOST: &str = "http://127.0.0.1:8082";

/// One tool the box offers: what callers name, the route that answers,
/// and the sentence the tool list carries.
struct Tool {
    name: &'static str,
    /// `METHOD /api/…` with `$` placeholders the caller's arguments fill.
    route: &'static str,
    description: &'static str,
    /// The arguments' shape, as the tool list answers it.
    schema: Value,
    /// Whether this tool's argument fills a path segment (and must be
    /// one plain segment) or reads the body.
    body: bool,
}

/// The `$` fills, in `route`, come from the arguments' full names, so a
/// template reads unchanged from the tool call. Built once, then static.
fn tools() -> &'static [Tool] {
    static TOOLS: std::sync::OnceLock<Vec<Tool>> = std::sync::OnceLock::new();
    TOOLS.get_or_init(|| vec![
        Tool { name: "box_status", route: "GET /api/status", description: "How this box is going, one page: state, sharing, the mesh, health.", schema: json!({ "type": "object" }), body: false },
        Tool { name: "me", route: "GET /api/me", description: "Who this token is on the box, and the sections it may read.", schema: json!({ "type": "object" }), body: false },
        Tool { name: "cloud_search", route: "GET /api/search?q=$q", description: "Search everything the person may read on this box: files in LosOS Cloud, LosOS Git code on default branches, their mail, the rooms of chat — graded by the box's confidentiality levels, so a hit above the caller's clearance never comes back.", schema: json!({ "type": "object", "properties": { "q": { "type": "string", "maxLength": 128 } }, "required": ["q"] }), body: false },
        Tool { name: "cloud_files", route: "GET /api/cloud/files?q=$q", description: "Files and folders in the signed-in person's LosOS Cloud, by name.", schema: json!({ "type": "object", "properties": { "q": { "type": "string", "maxLength": 128 } } }), body: false },
        Tool { name: "git_repos", route: "GET /api/git/repos?q=$q", description: "LosOS Git repositories, with the visibility and branch of each.", schema: json!({ "type": "object", "properties": { "q": { "type": "string", "maxLength": 128 } } }), body: false },
        Tool { name: "git_text", route: "GET /api/git/text?q=$q", description: "Lines matched on the default branch of every repository; graded like cloud_search.", schema: json!({ "type": "object", "properties": { "q": { "type": "string", "maxLength": 128 } }, "required": ["q"] }), body: false },
        Tool { name: "mail_mailboxes", route: "GET /api/mail", description: "The signed-in person's folders in LosOS Mail.", schema: json!({ "type": "object" }), body: false },
        Tool { name: "mail_search", route: "GET /api/mail/messages?mailbox=$mailbox&q=$q", description: "Search one mailbox (its id from mail_mailboxes) by subject and sender; the unread stays unread for the asker's own read.", schema: json!({ "type": "object", "properties": { "mailbox": { "type": "string", "maxLength": 128 }, "q": { "type": "string", "maxLength": 128 } }, "required": ["mailbox"] }), body: false },
        Tool { name: "tag_view", route: "GET /api/tag", description: "LosOS Tag: the levels, areas, assistants and skills, the asks on the queue, and the past conversations.", schema: json!({ "type": "object" }), body: false },
        Tool { name: "tag_ask", route: "POST /api/tag/ask", description: "Ask LosOS Tag one question in the background: unified search grounds the answer on the box, the ask runs at the assistant's own clearance.", schema: json!({ "type": "object", "properties": { "question": { "type": "string", "maxLength": 2000 }, "assistant": { "type": "string", "maxLength": 64 }, "room": { "type": "string", "maxLength": 255 }, "skills": { "type": "array", "items": { "type": "string" }, "maxItems": 8 } }, "required": ["question"] }), body: true },
        Tool { name: "statistics", route: "GET /api/statistics", description: "Traffic counts per app (Traefik's meter, one minute ring).", schema: json!({ "type": "object" }), body: false },
        Tool { name: "apps_list", route: "GET /api/apps", description: "The catalogue as this box carries it, and what is installed.", schema: json!({ "type": "object" }), body: false },
        Tool { name: "logs_recent", route: "GET /api/logs?spread=true", description: "The journal's recent lines, redacted: which units the box writes.", schema: json!({ "type": "object" }), body: false },
        Tool { name: "problems_list", route: "GET /api/problems", description: "What the box noticed going wrong, newest first. Read-only: a problem is the operator's to read.", schema: json!({ "type": "object" }), body: false },
    ])
}

/// `POST`s ride `X-Losos-As` through the admin token path alone: a tool
/// call made with a spare key answers as that person; a call answered
/// by the person's own token answers as themselves.
fn as_header(token: &str) -> String {
    format!("Bearer {token}")
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
        .arg(format!("Authorization: {}", as_header(token)));
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

/// The `$` fills the caller's own JSON body, the route's own path left
/// alone (no `$` in it).
fn with_body(tool: &Tool, args: &Value) -> Option<Value> {
    if tool.body {
        Some(args.clone())
    } else {
        None
    }
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

/// Call one tool by name.
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
    let method = if tool.body {
        "POST".into()
    } else {
        method.trim().to_uppercase()
    };
    let body = with_body(tool, args);
    let asked = token.trim();
    if asked.is_empty() {
        return Err("run losos-mcp with LOSOS_MCP_TOKEN: a token of the person the tools read as (losos_access::COMPANION or the Access tokens pane)".into());
    }
    ask(&path, &method, body.as_ref(), asked)
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
                    Ok(answer) => Some(json!({
                        "content": [{ "type": "text", "text": answer.to_string() }],
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
        assert!(names.contains(&"box_status"));
        assert!(names.contains(&"me"));
        assert!(names.contains(&"cloud_search"));
        assert!(names.contains(&"cloud_files"));
        assert!(names.contains(&"git_repos"));
        assert!(names.contains(&"git_text"));
        assert!(names.contains(&"mail_mailboxes"));
        assert!(names.contains(&"mail_search"));
        assert!(names.contains(&"tag_view"));
        assert!(names.contains(&"tag_ask"));
        assert!(names.contains(&"statistics"));
        assert!(names.contains(&"apps_list"));
        assert!(names.contains(&"logs_recent"));
        assert!(names.contains(&"problems_list"));
        for tool in tools() {
            // Every tool has a sentence to show, and a schema whatever
            // json! renders — the tools/list call is its shape's whole
            // test; the routes are the box's own, checked at conteúdo.
            assert!(!tool.description.trim().is_empty(), "{}", tool.name);
            assert!(tool.route.split_once(' ').is_some(), "{}", tool.name);
        }
    }

    #[test]
    fn a_fill_is_one_segment_and_described() {
        let args = json!({ "q": "tax 2025 ", "mailbox": "INBOX" });
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
    }

    #[test]
    fn a_body_tool_carries_the_arguments_without_the_route() {
        let tool = tools().iter().find(|t| t.name == "tag_ask").unwrap();
        let args = json!({ "question": "What is due?" });
        assert_eq!(with_body(tool, &args), Some(args.clone()));
        let read = tools().iter().find(|t| t.name == "box_status").unwrap();
        assert_eq!(with_body(read, &args), None);
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
