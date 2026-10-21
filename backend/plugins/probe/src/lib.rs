//! A plugin that pokes at every import and misbehaves on request, so
//! lososd's tests (`backend/tests/plugins.rs`) can show what the host
//! allows, refuses and survives. Not in the losos-plugins package.

use losos_plugin::{cache, clock, config, dispatch, exec, fs, http, jobs, lock, Plugin};
use losos_plugin::{Principal, Request, Response};
use serde_json::{json, Value};

fn result<T: Into<Value>>(r: losos_plugin::Result<T>) -> Response {
    match r {
        Ok(v) => Response::ok(&json!({ "ok": v.into() })),
        Err(e) => Response::ok(&json!({ "error": e.to_string(), "notFound": e.is_not_found() })),
    }
}

fn s(doc: &Value, key: &str) -> String {
    doc.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

struct Probe;

impl Plugin for Probe {
    fn handle(req: Request) -> Response {
        let doc = req.json_value();
        match req.route.as_str() {
            "/api/probe/echo" => {
                let who = match &req.caller.principal {
                    Principal::Anonymous => json!("anonymous"),
                    Principal::Spare => json!("spare"),
                    Principal::Edge => json!("edge"),
                    Principal::Host => json!("host"),
                    Principal::Person(p) => {
                        json!({ "name": p.name, "groups": p.groups, "admin": p.admin })
                    }
                };
                Response::ok(&json!({
                    "method": req.method,
                    "path": req.path,
                    "route": req.route,
                    "query": req.query_map(),
                    "headers": req.headers.iter().map(|(k, _)| k.clone()).collect::<Vec<_>>(),
                    "caller": who,
                    "section": req.caller.section,
                    "bodyBytes": req.body.len(),
                }))
            }
            "/api/probe/items/{id}" => {
                Response::ok(&json!({ "id": req.param("id") })).header("x-probe", "1")
            }
            "/api/probe/items/new" => Response::ok(&json!({ "id": null, "new": true })),
            "/api/probe/open" => Response::bytes(200, "text/plain", b"open".to_vec()),
            "/api/probe/config" => Response::ok(&json!({ "dir": config::get("dir") })),
            "/api/probe/read" => {
                result(fs::read(&s(&doc, "path")).map(|b| String::from_utf8_lossy(&b).into_owned()))
            }
            "/api/probe/write" => result(
                fs::write(
                    &s(&doc, "path"),
                    s(&doc, "data").as_bytes(),
                    fs::WriteOptions::secret(),
                )
                .map(|()| true),
            ),
            "/api/probe/exec" => {
                let args: Vec<String> = doc
                    .get("args")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(Value::as_str)
                            .map(String::from)
                            .collect()
                    })
                    .unwrap_or_default();
                result(
                    exec::Command::new(&s(&doc, "program"))
                        .args(args)
                        .output()
                        .map(|o| o.stdout_string()),
                )
            }
            "/api/probe/http" => result(
                http::Request::get(&s(&doc, "url"))
                    .fetch(1024 * 1024)
                    .map(|r| json!({ "status": r.status, "body": r.text() })),
            ),
            "/api/probe/lock" => result(lock::acquire(&s(&doc, "name")).map(|_g| true)),
            "/api/probe/hold" => {
                // Hold the lock for a while, so a second call has to wait.
                let r = lock::acquire("probe").map(|_g| {
                    clock::sleep_ms(500);
                    true
                });
                result(r)
            }
            "/api/probe/cache" => {
                if let Some(v) = doc.get("set").and_then(Value::as_str) {
                    cache::set("k", v.as_bytes(), 0);
                }
                Response::ok(&json!({
                    "value": cache::get("k").map(|b| String::from_utf8_lossy(&b).into_owned())
                }))
            }
            "/api/probe/job" => {
                result(jobs::spawn("mark", s(&doc, "path").as_bytes()).map(|()| true))
            }
            "/api/probe/dispatch" => {
                let mut inner = req.clone();
                inner.method = "GET".to_string();
                inner.path = s(&doc, "path");
                inner.route = String::new();
                inner.body = Vec::new();
                result(
                    dispatch::call(inner)
                        .map(|r| json!({ "status": r.status, "body": r.json_value() })),
                )
            }
            "/api/probe/spin" => loop {
                std::hint::black_box(0u64);
            },
            "/api/probe/grow" => {
                let mut hoard: Vec<Vec<u8>> = Vec::new();
                loop {
                    hoard.push(vec![1u8; 1024 * 1024]);
                    std::hint::black_box(&hoard);
                }
            }
            "/api/probe/panic" => panic!("asked to"),
            "/api/probe/time" => Response::ok(
                &json!({ "now": clock::now(), "random": losos_plugin::random::hex(8) }),
            ),
            _ => Response::error(404, "not found"),
        }
    }

    fn tick(timer: &str) {
        if let Some(dir) = config::get("dir") {
            let _ = fs::append(&fs::join(&dir, "ticks"), format!("{timer}\n").as_bytes());
        }
    }

    fn run(kind: &str, input: &[u8]) {
        if kind == "mark" {
            let path = String::from_utf8_lossy(input).into_owned();
            let _ = fs::write(&path, b"done", fs::WriteOptions::secret());
        }
    }
}

losos_plugin::export!(Probe);
