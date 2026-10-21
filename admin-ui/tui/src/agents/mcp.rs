//! `losos mcp`: the companion as an MCP server on stdin and stdout, the way
//! an AI tool starts one. Its tools only look: each is one GET to the box
//! through the envelope, signed by the companion key, on a route an AI
//! agents token reaches (`losos_access::AGENT_ROUTES`; the test at the end
//! keeps the two lists together).
//!
//! The wire is JSON-RPC 2.0, one message per line. The server answers
//! `initialize`, `ping`, `tools/list` and `tools/call`, takes notifications
//! without a word, and answers anything else with "method not found". It
//! reaches the box only when a tool is called, so a tool that lists its
//! servers at start-up does not wait on the network.

use std::io::{BufRead, Write};

use serde_json::{json, Map, Value};

use crate::envelope::{Answer, Transport};
use crate::problem::Result;

/// The versions of the protocol this server speaks, newest first.
pub const PROTOCOL_VERSIONS: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

/// The longest argument a tool takes, in characters.
const MAX_ARGUMENT_CHARS: usize = 500;

/// One argument of a tool: what the agent calls it, and the query
/// parameter it becomes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Param {
    pub name: &'static str,
    pub query: &'static str,
    pub description: &'static str,
    pub required: bool,
}

/// One tool: a GET of `route` with its arguments as the query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct McpTool {
    pub name: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub route: &'static str,
    pub params: &'static [Param],
}

const QUERY: Param = Param {
    name: "query",
    query: "q",
    description: "The words to look for.",
    required: true,
};

pub const MCP_TOOLS: &[McpTool] = &[
    McpTool {
        name: "whoami",
        title: "Who the tools act as",
        description: "The LosOS account these tools read as, and its role on the box.",
        route: "/api/me",
        params: &[],
    },
    McpTool {
        name: "box_status",
        title: "Box status",
        description: "How the LosOS box is doing: its storage, its sharing and its last update.",
        route: "/api/status",
        params: &[],
    },
    McpTool {
        name: "cloud_list_folder",
        title: "List a LosOS Cloud folder",
        description: "The files and folders in one folder of LosOS Cloud, with their sizes and \
                      dates. Start at / and go down.",
        route: "/api/cloud/files",
        params: &[Param {
            name: "path",
            query: "path",
            description: "The folder, from the top of LosOS Cloud, such as / or /Documents.",
            required: false,
        }],
    },
    McpTool {
        name: "cloud_find_files",
        title: "Find files in LosOS Cloud",
        description: "Files and folders anywhere in LosOS Cloud whose name holds the words.",
        route: "/api/cloud/files",
        params: &[QUERY],
    },
    McpTool {
        name: "cloud_search_text",
        title: "Search inside LosOS Cloud files",
        description: "Files in LosOS Cloud whose text holds the words, with the lines that \
                      match.",
        route: "/api/cloud/text",
        params: &[QUERY],
    },
    McpTool {
        name: "git_list_repositories",
        title: "List LosOS Git repositories",
        description: "The owner's repositories on LosOS Git, or those whose name holds the \
                      words.",
        route: "/api/git/repos",
        params: &[Param {
            required: false,
            ..QUERY
        }],
    },
    McpTool {
        name: "git_search_code",
        title: "Search code in LosOS Git",
        description: "Lines on the default branch of every LosOS Git repository that hold the \
                      words, with the repository and file of each.",
        route: "/api/git/text",
        params: &[QUERY],
    },
];

/// What `initialize` tells the agent about the server as a whole.
const INSTRUCTIONS: &str = "Reads the owner's LosOS box: files in LosOS Cloud and repositories \
     in LosOS Git. Every tool only looks; none changes anything. When the owner asks about \
     their own files, notes or code, look here, and say which file or repository an answer \
     came from.";

impl McpTool {
    fn schema(&self) -> Value {
        let mut properties = Map::new();
        for p in self.params {
            properties.insert(
                p.name.into(),
                json!({ "type": "string", "description": p.description, "maxLength": MAX_ARGUMENT_CHARS }),
            );
        }
        let required: Vec<&str> = self
            .params
            .iter()
            .filter(|p| p.required)
            .map(|p| p.name)
            .collect();
        json!({
            "type": "object",
            "properties": properties,
            "required": required,
            "additionalProperties": false,
        })
    }

    fn listing(&self) -> Value {
        json!({
            "name": self.name,
            "title": self.title,
            "description": self.description,
            "inputSchema": self.schema(),
            "annotations": {
                "title": self.title,
                "readOnlyHint": true,
                "destructiveHint": false,
                "idempotentHint": true,
                "openWorldHint": false,
            },
        })
    }

    /// The path and query for these arguments, or what is wrong with them.
    pub fn path(&self, arguments: &Value) -> std::result::Result<String, String> {
        let empty = Map::new();
        let given = match arguments {
            Value::Object(map) => map,
            Value::Null => &empty,
            _ => return Err("the arguments are not an object".into()),
        };
        if let Some(stranger) = given
            .keys()
            .find(|k| !self.params.iter().any(|p| p.name == *k))
        {
            return Err(format!("{} takes no argument {stranger}", self.name));
        }
        let mut query = url::form_urlencoded::Serializer::new(String::new());
        let mut any = false;
        for p in self.params {
            match given.get(p.name) {
                None | Some(Value::Null) if p.required => {
                    return Err(format!("{} needs {}", self.name, p.name))
                }
                None | Some(Value::Null) => {}
                Some(Value::String(s)) => {
                    let s = s.trim();
                    if s.is_empty() && p.required {
                        return Err(format!("{} needs {}", self.name, p.name));
                    }
                    if s.chars().count() > MAX_ARGUMENT_CHARS {
                        return Err(format!(
                            "{} is longer than {MAX_ARGUMENT_CHARS} characters",
                            p.name
                        ));
                    }
                    if !s.is_empty() {
                        query.append_pair(p.query, s);
                        any = true;
                    }
                }
                Some(_) => return Err(format!("{} is not a string", p.name)),
            }
        }
        Ok(if any {
            format!("{}?{}", self.route, query.finish())
        } else {
            self.route.to_string()
        })
    }
}

pub fn tool(name: &str) -> Option<&'static McpTool> {
    MCP_TOOLS.iter().find(|t| t.name == name)
}

/// Makes the connection to the box, once a tool is called.
pub type Connect<'a> = Box<dyn FnMut() -> Result<Box<dyn Transport>> + 'a>;

/// The server: how to reach the box, and the connection once made.
pub struct Server<'a> {
    connect: Connect<'a>,
    transport: Option<Box<dyn Transport>>,
}

fn reply(id: &Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn failure(id: &Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// A tool's answer: the text for the agent, the same data as structured
/// content where it is an object, and whether it failed.
fn tool_result(text: String, data: Option<Value>, error: bool) -> Value {
    let mut result = json!({
        "content": [{ "type": "text", "text": text }],
        "isError": error,
    });
    if let Some(data @ Value::Object(_)) = data {
        result["structuredContent"] = data;
    }
    result
}

impl<'a> Server<'a> {
    pub fn new(connect: Connect<'a>) -> Server<'a> {
        Server {
            connect,
            transport: None,
        }
    }

    /// Read messages from `input` until it ends, answering each on `output`.
    pub fn serve(&mut self, input: impl BufRead, output: &mut dyn Write) -> std::io::Result<()> {
        for line in input.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            if let Some(answer) = self.handle_line(&line) {
                writeln!(output, "{answer}")?;
                output.flush()?;
            }
        }
        Ok(())
    }

    /// The answer to one line, or nothing for a notification.
    pub fn handle_line(&mut self, line: &str) -> Option<Value> {
        match serde_json::from_str::<Value>(line) {
            Err(_) => Some(failure(&Value::Null, -32700, "parse error")),
            Ok(Value::Array(batch)) if !batch.is_empty() => {
                let answers: Vec<Value> = batch.iter().filter_map(|m| self.handle(m)).collect();
                (!answers.is_empty()).then_some(Value::Array(answers))
            }
            Ok(message) => self.handle(&message),
        }
    }

    /// The answer to one message, or nothing for a notification.
    pub fn handle(&mut self, message: &Value) -> Option<Value> {
        let id = message.get("id").cloned();
        let method = message.get("method").and_then(Value::as_str);
        let (Some(id), Some(method)) = (id, method) else {
            // A notification, or an answer to a request this server never
            // makes; neither is answered. Anything else is not a request.
            return match (message.get("id"), message.get("method")) {
                (None, Some(_)) => None,
                (Some(_), None)
                    if message.get("result").is_some() || message.get("error").is_some() =>
                {
                    None
                }
                (id, _) => Some(failure(
                    id.unwrap_or(&Value::Null),
                    -32600,
                    "invalid request",
                )),
            };
        };
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        tracing::debug!(%method, "mcp");
        Some(match method {
            "initialize" => reply(&id, initialize(&params)),
            "ping" => reply(&id, json!({})),
            "tools/list" => reply(
                &id,
                json!({ "tools": MCP_TOOLS.iter().map(McpTool::listing).collect::<Vec<_>>() }),
            ),
            "tools/call" => {
                let name = params.get("name").and_then(Value::as_str).unwrap_or("");
                match tool(name) {
                    Some(t) => reply(
                        &id,
                        self.call(t, params.get("arguments").unwrap_or(&Value::Null)),
                    ),
                    None => failure(&id, -32602, &format!("no tool is called {name:?}")),
                }
            }
            _ => failure(&id, -32601, "method not found"),
        })
    }

    fn call(&mut self, t: &McpTool, arguments: &Value) -> Value {
        let path = match t.path(arguments) {
            Ok(path) => path,
            Err(why) => return tool_result(why, None, true),
        };
        if self.transport.is_none() {
            match (self.connect)() {
                Ok(transport) => self.transport = Some(transport),
                Err(problem) => return tool_result(problem_text(&problem), None, true),
            }
        }
        let transport = self.transport.as_ref().expect("connected above");
        match transport.request("GET", &path, Value::Null) {
            Ok(answer) => answer_result(answer),
            Err(problem) => tool_result(problem_text(&problem), None, true),
        }
    }
}

fn problem_text(problem: &crate::problem::Problem) -> String {
    match &problem.help {
        Some(help) => format!("{problem} {help}"),
        None => problem.to_string(),
    }
}

fn answer_result(answer: Answer) -> Value {
    if !answer.ok() {
        return tool_result(answer.error_text(), None, true);
    }
    // A search that is off answers 200 with `available: false` and the
    // reason; the agent should hear that as a failure it can explain.
    let off = answer.body.get("available") == Some(&Value::Bool(false));
    let text = serde_json::to_string_pretty(&answer.body).unwrap_or_default();
    tool_result(text, Some(answer.body), off)
}

fn initialize(params: &Value) -> Value {
    let asked = params
        .get("protocolVersion")
        .and_then(Value::as_str)
        .unwrap_or("");
    let version = PROTOCOL_VERSIONS
        .iter()
        .find(|v| **v == asked)
        .unwrap_or(&PROTOCOL_VERSIONS[0]);
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": {
            "name": super::NAME,
            "title": "LosOS",
            "version": crate::tui::panes::VERSION,
        },
        "instructions": INSTRUCTIONS,
    })
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::problem::Problem;

    #[test]
    fn every_tool_reads_a_route_an_agents_token_reaches() {
        for t in MCP_TOOLS {
            assert!(
                losos_access::AGENT_ROUTES.contains(&t.route),
                "{} reads {}, which an AI agents token cannot",
                t.name,
                t.route
            );
            assert!(losos_access::agent_route(
                &t.path(&json!({ "query": "x" }))
                    .unwrap_or_else(|_| t.route.to_string())
            ));
        }
        for route in losos_access::AGENT_ROUTES {
            assert!(
                MCP_TOOLS.iter().any(|t| t.route == *route),
                "{route} is open to agents but no tool reads it"
            );
        }
    }

    #[test]
    fn arguments_become_a_query_and_nothing_else() {
        let find = tool("cloud_find_files").unwrap();
        assert_eq!(
            find.path(&json!({ "query": "tax 2025 & more" })).unwrap(),
            "/api/cloud/files?q=tax+2025+%26+more"
        );
        assert!(find.path(&json!({})).is_err());
        assert!(find.path(&json!({ "query": "  " })).is_err());
        assert!(find.path(&json!({ "query": 3 })).is_err());
        assert!(find.path(&json!({ "query": "a", "path": "/" })).is_err());
        assert!(find.path(&json!({ "query": "x".repeat(501) })).is_err());
        let list = tool("cloud_list_folder").unwrap();
        assert_eq!(list.path(&Value::Null).unwrap(), "/api/cloud/files");
        assert_eq!(
            list.path(&json!({ "path": "/Documents/Á b" })).unwrap(),
            "/api/cloud/files?path=%2FDocuments%2F%C3%81+b"
        );
        let repos = tool("git_list_repositories").unwrap();
        assert_eq!(repos.path(&json!({})).unwrap(), "/api/git/repos");
    }

    /// A box that answers from a table and remembers what it was asked.
    struct Table {
        asked: Arc<Mutex<Vec<String>>>,
    }

    impl Transport for Table {
        fn request(&self, method: &str, path: &str, _body: Value) -> Result<Answer> {
            self.asked.lock().unwrap().push(format!("{method} {path}"));
            let (status, body) = match path {
                "/api/me" => (200, json!({ "user": "matus", "role": "admin" })),
                p if p.starts_with("/api/git/text") => {
                    (200, json!({ "available": false, "reason": "noGit" }))
                }
                p if p.starts_with("/api/cloud/files") => (
                    200,
                    json!({ "path": "/", "entries": [{ "name": "notes.md" }] }),
                ),
                _ => (403, json!({ "error": "this token cannot reach that" })),
            };
            Ok(Answer {
                status,
                body,
                sealed: true,
            })
        }
    }

    fn server(asked: &Arc<Mutex<Vec<String>>>, connects: &Arc<Mutex<u32>>) -> Server<'static> {
        let asked = asked.clone();
        let connects = connects.clone();
        Server::new(Box::new(move || {
            *connects.lock().unwrap() += 1;
            Ok(Box::new(Table {
                asked: asked.clone(),
            }) as Box<dyn Transport>)
        }))
    }

    fn talk(server: &mut Server<'_>, lines: &[Value]) -> Vec<Value> {
        let input: String = lines.iter().map(|l| format!("{l}\n")).collect();
        let mut out = Vec::new();
        server.serve(input.as_bytes(), &mut out).unwrap();
        String::from_utf8(out)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    #[test]
    fn a_session_as_an_agent_tool_runs_it() {
        let asked = Arc::new(Mutex::new(Vec::new()));
        let connects = Arc::new(Mutex::new(0));
        let mut s = server(&asked, &connects);
        let answers = talk(
            &mut s,
            &[
                json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
                    "protocolVersion": "2025-06-18", "capabilities": {},
                    "clientInfo": { "name": "test", "version": "1" } } }),
                json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
                json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }),
                json!({ "jsonrpc": "2.0", "id": "three", "method": "tools/call", "params": {
                    "name": "whoami", "arguments": {} } }),
                json!({ "jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {
                    "name": "cloud_list_folder", "arguments": { "path": "/" } } }),
                json!({ "jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": {
                    "name": "git_search_code", "arguments": { "query": "main" } } }),
                json!({ "jsonrpc": "2.0", "id": 6, "method": "tools/call", "params": {
                    "name": "cloud_find_files", "arguments": {} } }),
                json!({ "jsonrpc": "2.0", "id": 7, "method": "tools/call", "params": {
                    "name": "delete_everything", "arguments": {} } }),
                json!({ "jsonrpc": "2.0", "id": 8, "method": "resources/list" }),
                json!({ "jsonrpc": "2.0", "id": 9, "method": "ping" }),
            ],
        );
        // The notification got no answer: nine requests, nine answers.
        assert_eq!(answers.len(), 9);
        let init = &answers[0]["result"];
        assert_eq!(init["protocolVersion"], "2025-06-18");
        assert_eq!(init["serverInfo"]["name"], "losos");
        assert!(init["capabilities"]["tools"].is_object());

        let tools = answers[1]["result"]["tools"].as_array().unwrap();
        assert_eq!(tools.len(), MCP_TOOLS.len());
        assert!(tools
            .iter()
            .all(|t| t["annotations"]["readOnlyHint"] == true
                && t["inputSchema"]["type"] == "object"));

        assert_eq!(answers[2]["id"], "three");
        assert_eq!(answers[2]["result"]["isError"], false);
        assert_eq!(answers[2]["result"]["structuredContent"]["user"], "matus");
        assert!(answers[2]["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("matus"));

        assert_eq!(answers[3]["result"]["isError"], false);
        assert_eq!(
            answers[4]["result"]["isError"], true,
            "a search that is off"
        );
        assert_eq!(answers[5]["result"]["isError"], true, "a missing argument");
        assert_eq!(answers[6]["error"]["code"], -32602);
        assert_eq!(answers[7]["error"]["code"], -32601);
        assert_eq!(answers[8]["result"], json!({}));

        // One connection for every call, made at the first call; a bad
        // argument never reached the box.
        assert_eq!(*connects.lock().unwrap(), 1);
        assert_eq!(
            *asked.lock().unwrap(),
            vec![
                "GET /api/me",
                "GET /api/cloud/files?path=%2F",
                "GET /api/git/text?q=main"
            ]
        );
    }

    #[test]
    fn an_unknown_version_gets_the_newest_and_junk_gets_errors() {
        let asked = Arc::new(Mutex::new(Vec::new()));
        let connects = Arc::new(Mutex::new(0));
        let mut s = server(&asked, &connects);
        let a = s
            .handle(&json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": { "protocolVersion": "1999-01-01" } }))
            .unwrap();
        assert_eq!(a["result"]["protocolVersion"], PROTOCOL_VERSIONS[0]);
        assert_eq!(s.handle_line("{not json").unwrap()["error"]["code"], -32700);
        assert_eq!(
            s.handle_line(r#"{"jsonrpc":"2.0","id":3}"#).unwrap()["error"]["code"],
            -32600
        );
        assert!(s
            .handle_line(r#"{"jsonrpc":"2.0","id":3,"result":{}}"#)
            .is_none());
        let batch = s
            .handle_line(
                r#"[{"jsonrpc":"2.0","id":1,"method":"ping"},{"jsonrpc":"2.0","method":"x"}]"#,
            )
            .unwrap();
        assert_eq!(batch.as_array().unwrap().len(), 1);
    }

    #[test]
    fn a_box_out_of_reach_is_a_tool_error_and_is_tried_again() {
        let tries = Arc::new(Mutex::new(0));
        let counted = tries.clone();
        let mut s = Server::new(Box::new(move || {
            *counted.lock().unwrap() += 1;
            Err(Problem::new("losos::agents", "no companion").help("run losos agents install"))
        }));
        for id in 1..=2 {
            let a = s
                .handle(&json!({ "jsonrpc": "2.0", "id": id, "method": "tools/call",
                    "params": { "name": "box_status" } }))
                .unwrap();
            assert_eq!(a["result"]["isError"], true);
            assert_eq!(
                a["result"]["content"][0]["text"],
                "no companion run losos agents install"
            );
        }
        assert_eq!(*tries.lock().unwrap(), 2);
    }
}
