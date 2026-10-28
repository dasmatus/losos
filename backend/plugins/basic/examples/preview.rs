//! The pages drawn from fixtures, for a look at them in a browser without a
//! box: `cargo run --example preview -- <admin-ui dist> <out dir>` writes one
//! HTML file per page under `<out>/basic/`, beside which the dist's own
//! `assets/` can be served.

use losos_plugin::{Caller, Person, Principal, Request};
use losos_plugin_basic::handle_with;
use losos_plugin_basic::host::{Answer, Forwarded, Host, LoginAnswer};
use serde_json::{json, Value};
use std::path::PathBuf;

struct Preview {
    dist: PathBuf,
    state: &'static str,
}

impl Host for Preview {
    fn call(&mut self, _method: &str, path: &str, _ct: Option<&str>, _body: &[u8]) -> Answer {
        let json = match path {
            "/api/me" => {
                json!({"name": "matus", "displayName": "Matus", "admin": true, "standalone": true, "sections": [], "accent": "teal"})
            }
            "/api/settings" => json!({
                "sharingMyStorage": false, "hostName": "mattbox", "gpuEnable": false,
                "proxyEnable": true, "clusterEnable": false, "shareCompute": false,
                "computeWindowStart": "23:00", "computeWindowEnd": "07:00",
                "hardeningApparmor": false, "hardeningMalloc": false,
                "hardeningNosmt": false, "hardeningUsbguard": false
            }),
            "/api/options" => json!({"options": [], "stray": []}),
            "/api/pins" => json!({"pins": [], "limit": 24}),
            "/api/domains" => json!({"available": false}),
            "/api/status" => {
                json!({"state": self.state, "progress": 0, "message": if self.state == "building" { "building '/nix/store/…-nixos-system-mattbox'" } else { "" }, "job": "j1"})
            }
            "/api/storage" => {
                json!({"totalBytes": 250_000_000_000u64, "usedBytes": 61_000_000_000u64, "reserveBytes": 24_000_000_000u64})
            }
            _ => {
                return Answer {
                    status: 404,
                    json: Value::Null,
                }
            }
        };
        Answer { status: 200, json }
    }

    fn login(
        &mut self,
        _m: &str,
        path: &str,
        _f: &Forwarded,
        _s: Option<&str>,
        _b: Option<&Value>,
    ) -> Option<LoginAnswer> {
        (path == "health" && self.state != "login-down").then(|| LoginAnswer {
            status: 200,
            ..LoginAnswer::default()
        })
    }

    fn key_opens(&mut self, _key: &str) -> bool {
        false
    }

    fn ui_file(&mut self, path: &str) -> Option<Vec<u8>> {
        std::fs::read(self.dist.join(path)).ok()
    }

    fn apps_running(&mut self) -> (bool, bool) {
        (true, true)
    }

    fn office_folder(&mut self) -> bool {
        true
    }
}

/// A page to write: its file name, the route, the path, the query, the
/// box's rebuild state and the extra request headers.
type Shot<'a> = (
    &'a str,
    &'a str,
    &'a str,
    &'a str,
    &'a str,
    &'a [(&'a str, &'a str)],
);

fn main() {
    let mut args = std::env::args().skip(1);
    let dist = PathBuf::from(args.next().expect("the admin UI's dist"));
    let out = PathBuf::from(args.next().expect("an out dir"));
    let caller = Caller {
        principal: Principal::Person(Person {
            name: "matus".into(),
            display_name: "Matus".into(),
            groups: vec![],
            admin: true,
        }),
        remote: "192.168.1.20".into(),
        section: "admin".into(),
    };
    let pages: &[Shot] = &[
        ("overview", "/api/basic/", "/api/basic/", "", "idle", &[]),
        (
            "storage",
            "/api/basic/{page}",
            "/api/basic/storage",
            "",
            "idle",
            &[],
        ),
        (
            "network",
            "/api/basic/settings/{pane}",
            "/api/basic/settings/network",
            "",
            "idle",
            &[],
        ),
        (
            "hardware",
            "/api/basic/settings/{pane}",
            "/api/basic/settings/hardware",
            "",
            "idle",
            &[],
        ),
        (
            "security",
            "/api/basic/settings/{pane}",
            "/api/basic/settings/security",
            "",
            "idle",
            &[],
        ),
        (
            "rebuilding",
            "/api/basic/settings/{pane}",
            "/api/basic/settings/hardware",
            "",
            "building",
            &[("x-losos-basic-job", "j1.0")],
        ),
        (
            "done",
            "/api/basic/settings/{pane}",
            "/api/basic/settings/hardware",
            "",
            "done",
            &[("x-losos-basic-job", "j1.0")],
        ),
        (
            "backup",
            "/api/basic/settings/{pane}",
            "/api/basic/settings/backup",
            "",
            "idle",
            &[],
        ),
        (
            "search",
            "/api/basic/{page}",
            "/api/basic/search",
            "q=st",
            "idle",
            &[],
        ),
        (
            "rail",
            "/api/basic/",
            "/api/basic/",
            "",
            "idle",
            &[
                ("x-losos-basic-prefs", "theme=dark&rail=1"),
                ("x-losos-basic-flash", "rail"),
            ],
        ),
        (
            "sign-in",
            "/api/basic/sign-in",
            "/api/basic/sign-in",
            "next=/basic/",
            "idle",
            &[],
        ),
        (
            "sign-in-key",
            "/api/basic/sign-in",
            "/api/basic/sign-in",
            "with=key&next=/basic/",
            "idle",
            &[],
        ),
        (
            "sign-in-code",
            "/api/basic/sign-in",
            "/api/basic/sign-in",
            "step=code&next=/basic/",
            "idle",
            &[],
        ),
        (
            "sign-in-starting",
            "/api/basic/sign-in",
            "/api/basic/sign-in",
            "next=/basic/",
            "login-down",
            &[],
        ),
    ];
    std::fs::create_dir_all(out.join("basic")).unwrap();
    for (name, route, path, query, state, extra) in pages {
        let mut headers: Vec<(String, String)> = vec![
            ("x-forwarded-host".into(), "mattbox.local".into()),
            ("x-forwarded-proto".into(), "https".into()),
            (
                "accept-language".into(),
                std::env::var("LANG_HEADER").unwrap_or_else(|_| "en".into()),
            ),
        ];
        headers.extend(
            extra
                .iter()
                .map(|(k, v)| ((*k).to_string(), (*v).to_string())),
        );
        let req = Request {
            method: "GET".into(),
            path: (*path).into(),
            route: (*route).into(),
            params: vec![],
            query: (*query).into(),
            headers,
            body: vec![],
            caller: caller.clone(),
        };
        let mut host = Preview {
            dist: dist.clone(),
            state,
        };
        let resp = handle_with(&mut host, &req, 10);
        std::fs::write(out.join("basic").join(format!("{name}.html")), &resp.body).unwrap();
        println!("{name}: {}", resp.status);
    }
    let css = Request {
        method: "GET".into(),
        path: "/api/basic/counts.css".into(),
        route: "/api/basic/counts.css".into(),
        params: vec![],
        query: "lang=en".into(),
        headers: vec![],
        body: vec![],
        caller,
    };
    let resp = handle_with(
        &mut Preview {
            dist,
            state: "idle",
        },
        &css,
        10,
    );
    std::fs::write(out.join("basic").join("counts.css"), &resp.body).unwrap();
}
