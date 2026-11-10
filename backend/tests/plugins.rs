//! The plugin host as the box runs it: the real `lososd`, the real look
//! plugin and a probe plugin that tries every import and misbehaves on
//! request (`plugins/probe`), loaded from a sealed directory.
//!
//! The plugins are built here for wasm32 and wrapped as components, as
//! nix/packages.nix does with `wasm-tools component new`. A debug lososd
//! takes `LOSOS_PLUGIN_TRUST_ROOT` in place of the Nix store; the mount and
//! owner checks that a test cannot meet are covered by nix/tests/plugins.nix.

use assert_fs::TempDir;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// One plugin as a component: name, bytes, manifest.
type Built = (String, Vec<u8>, Vec<u8>);

/// The plugins as components.
fn components() -> &'static Vec<Built> {
    static BUILT: OnceLock<Vec<Built>> = OnceLock::new();
    BUILT.get_or_init(|| {
        let ws = Path::new(env!("CARGO_MANIFEST_DIR")).join("plugins");
        let status = Command::new(env!("CARGO"))
            .args([
                "build",
                "--release",
                "--target",
                "wasm32-unknown-unknown",
                "--manifest-path",
            ])
            .arg(ws.join("Cargo.toml"))
            // The outer cargo's flags are for the host, not wasm32.
            .env_remove("RUSTFLAGS")
            .env_remove("CARGO_ENCODED_RUSTFLAGS")
            // The modules are read from the workspace's own target below.
            .env_remove("CARGO_TARGET_DIR")
            .status()
            .expect("cargo builds the plugins");
        assert!(status.success(), "building the plugins for wasm32 failed");
        ["look", "pins", "hidden", "pages", "probe"]
            .iter()
            .map(|name| {
                let core = std::fs::read(
                    ws.join("target/wasm32-unknown-unknown/release")
                        .join(format!("losos_plugin_{name}.wasm")),
                )
                .unwrap();
                let component = wit_component::ComponentEncoder::default()
                    .module(&core)
                    .unwrap()
                    .validate(true)
                    .encode()
                    .unwrap();
                let manifest = std::fs::read(ws.join(name).join("manifest.json")).unwrap();
                (name.to_string(), component, manifest)
            })
            .collect()
    })
}

/// The plugins most tests load. The pages plugin carries the whole MDX
/// crate and slows lososd's start enough that fourteen daemons at once on a
/// CI runner missed the deadline, so only its own test loads it.
const PLUGINS: &[&str] = &["look", "pins", "hidden", "probe"];

/// A sealed "store" with one package of the named plugins in it.
fn store(root: &Path, plugins: &[&str]) -> PathBuf {
    let pkg = root.join("0000-losos-plugins");
    std::fs::create_dir_all(&pkg).unwrap();
    for (name, wasm, manifest) in components()
        .iter()
        .filter(|(name, ..)| plugins.contains(&name.as_str()))
    {
        for (file, bytes) in [
            (format!("{name}.wasm"), wasm),
            (format!("{name}.json"), manifest),
        ] {
            let path = pkg.join(file);
            std::fs::write(&path, bytes).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o444)).unwrap();
        }
    }
    std::fs::set_permissions(&pkg, std::fs::Permissions::from_mode(0o555)).unwrap();
    pkg
}

fn free_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

struct Daemon {
    child: Child,
    port: u16,
    dir: TempDir,
    store_root: PathBuf,
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        // A failing test shows what lososd logged.
        if std::thread::panicking() {
            if let Some(mut err) = self.child.stderr.take() {
                let mut log = String::new();
                let _ = err.read_to_string(&mut log);
                eprintln!("lososd said:\n{log}");
            }
        }
        // The sealed package is not removable until it is writable again.
        for entry in std::fs::read_dir(&self.store_root)
            .into_iter()
            .flatten()
            .flatten()
        {
            let _ = std::fs::set_permissions(entry.path(), std::fs::Permissions::from_mode(0o755));
        }
    }
}

struct Answer {
    status: u16,
    head: String,
    body: Vec<u8>,
}

impl Answer {
    fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap_or(Value::Null)
    }
}

impl Daemon {
    fn start() -> Daemon {
        Daemon::start_with(&[])
    }

    fn start_with(extra: &[(&str, &str)]) -> Daemon {
        Daemon::start_plugins(PLUGINS, extra)
    }

    fn start_plugins(plugins: &[&str], extra: &[(&str, &str)]) -> Daemon {
        let dir = TempDir::new().unwrap();
        let store_root = dir.path().join("store");
        let pkg = store(&store_root, plugins);
        let probe = dir.path().join("probe");
        std::fs::create_dir_all(probe.join("sealed")).unwrap();
        std::fs::write(dir.path().join("secret"), "not yours").unwrap();
        std::fs::write(dir.path().join("token"), TOKEN).unwrap();
        std::fs::write(
            dir.path().join("users.yml"),
            json!({"users": {
                "ada": {"displayname": "Ada", "password": "x", "groups": ["losos-look"]},
                "bob": {"displayname": "Bob", "password": "x", "groups": []},
                "ola": {"displayname": "Ola", "password": "x", "groups": ["losos-admins", "losos-owner"]},
                "root": {"displayname": "Root", "password": "x", "groups": ["losos-admins"]}
            }})
            .to_string(),
        )
        .unwrap();
        let port = free_port();
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_lososd"));
        cmd.env("LOSOS_NO_DBUS", "1")
            .env("LOSOS_STATE_DIR", dir.path().join("state"))
            .env("LOSOS_OVERRIDES", dir.path().join("overrides.nix"))
            .env("LOSOS_ADMIN_TOKEN_FILE", dir.path().join("token"))
            .env("LOSOS_ADMIN_PORT", port.to_string())
            .env("LOSOS_IDENTITY", "authelia")
            .env("LOSOS_PEOPLE_FILE", dir.path().join("users.yml"))
            .env("LOSOS_PEOPLE_DIR", dir.path())
            .env("LOSOS_PLUGIN_DIR", &pkg)
            .env("LOSOS_PLUGIN_TRUST_ROOT", &store_root)
            .env("LOSOS_PROBE_DIR", &probe)
            .env("RUST_LOG", "warn")
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        for (k, v) in extra {
            cmd.env(k, v);
        }
        std::fs::create_dir_all(dir.path().join("state")).unwrap();
        let child = cmd.spawn().unwrap();
        let mut d = Daemon {
            child,
            port,
            dir,
            store_root,
        };
        let deadline = Instant::now() + Duration::from_secs(60);
        while Instant::now() < deadline {
            if d.try_call("GET", "/api/health", &[], b"").is_some() {
                return d;
            }
            if let Ok(Some(_)) = d.child.try_wait() {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }

        let _ = d.child.kill();
        let mut log = String::new();
        if let Some(mut err) = d.child.stderr.take() {
            let _ = err.read_to_string(&mut log);
        }
        panic!("lososd did not come up:\n{log}");
    }

    fn try_call(
        &self,
        method: &str,
        path: &str,
        headers: &[(&str, &str)],
        body: &[u8],
    ) -> Option<Answer> {
        let mut s = TcpStream::connect(("127.0.0.1", self.port)).ok()?;
        s.set_read_timeout(Some(Duration::from_secs(30))).ok()?;
        let mut req = format!(
            "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Length: {}\r\n",
            body.len()
        );
        for (k, v) in headers {
            req.push_str(&format!("{k}: {v}\r\n"));
        }
        req.push_str("\r\n");
        s.write_all(req.as_bytes()).ok()?;
        s.write_all(body).ok()?;
        let mut raw = Vec::new();
        s.read_to_end(&mut raw).ok()?;
        let end = raw.windows(4).position(|w| w == b"\r\n\r\n")?;
        let head = String::from_utf8_lossy(&raw[..end]).into_owned();
        let status = head.split_whitespace().nth(1)?.parse().ok()?;
        let mut body = raw[end + 4..].to_vec();
        if head
            .to_ascii_lowercase()
            .contains("transfer-encoding: chunked")
        {
            body = dechunk(&body);
        }
        Some(Answer { status, head, body })
    }

    fn call(&self, method: &str, path: &str, headers: &[(&str, &str)], body: &[u8]) -> Answer {
        self.try_call(method, path, headers, body)
            .unwrap_or_else(|| panic!("{method} {path}: no answer"))
    }

    /// With the spare key.
    fn admin(&self, method: &str, path: &str, body: Value) -> Answer {
        let bearer = format!("Bearer {TOKEN}");
        let bytes = if body.is_null() {
            Vec::new()
        } else {
            body.to_string().into_bytes()
        };
        self.call(
            method,
            path,
            &[
                ("Authorization", &bearer),
                ("Content-Type", "application/json"),
            ],
            &bytes,
        )
    }

    /// As a person LosOS Login signed in.
    fn person(&self, name: &str, method: &str, path: &str) -> Answer {
        self.call(method, path, &[("Remote-User", name)], b"")
    }

    /// As a person LosOS Login signed in, with a body.
    fn person_send(&self, name: &str, method: &str, path: &str, body: &[u8]) -> Answer {
        self.call(
            method,
            path,
            &[("Remote-User", name), ("Content-Type", "application/json")],
            body,
        )
    }

    fn probe(&self) -> PathBuf {
        self.dir.path().join("probe")
    }

    /// The probe's answer to `route` with `body`, as the spare key.
    fn poke(&self, route: &str, body: Value) -> Value {
        let a = self.admin("POST", route, body);
        assert_eq!(
            a.status,
            200,
            "{route}: {}",
            String::from_utf8_lossy(&a.body)
        );
        a.json()
    }
}

fn dechunk(mut b: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let Some(nl) = b.windows(2).position(|w| w == b"\r\n") else {
            return out;
        };
        let size =
            usize::from_str_radix(std::str::from_utf8(&b[..nl]).unwrap().trim(), 16).unwrap_or(0);
        if size == 0 {
            return out;
        }
        out.extend_from_slice(&b[nl + 2..nl + 2 + size]);
        b = &b[nl + 2 + size + 2..];
    }
}

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n0000";

#[test]
fn the_look_is_served_by_its_plugin() {
    let d = Daemon::start();
    let a = d.admin("GET", "/api/look", Value::Null);
    assert_eq!(a.status, 200);
    let doc = a.json();
    assert_eq!(doc["background"]["kind"], "none");
    assert_eq!(doc["veil"], 60);
    assert_eq!(doc["shipped"][0], "tide");

    let a = d.admin("POST", "/api/look", json!({"veil": 35}));
    assert_eq!(a.status, 200);
    assert_eq!(a.json()["veil"], 35);
    let a = d.admin("POST", "/api/look", json!({"veil": 5}));
    assert_eq!(a.status, 400);
    assert!(a.json()["error"].as_str().unwrap().contains("veil"));
    let a = d.admin("POST", "/api/look", json!("nonsense"));
    assert_eq!(a.status, 400);

    let bearer = format!("Bearer {TOKEN}");
    let a = d.call(
        "PUT",
        "/api/look/background",
        &[("Authorization", &bearer)],
        PNG,
    );
    assert_eq!(a.status, 200, "{}", String::from_utf8_lossy(&a.body));
    assert_eq!(a.json()["background"]["url"], "/api/look/background?v=1");
    let a = d.call(
        "PUT",
        "/api/look/background",
        &[("Authorization", &bearer)],
        b"<svg/>",
    );
    assert_eq!(a.status, 400);

    // The picture is read back with no token, as a CSS background is.
    let a = d.call("GET", "/api/look/background?v=1", &[], b"");
    assert_eq!(a.status, 200);
    assert_eq!(a.body, PNG);
    let head = a.head.to_ascii_lowercase();
    assert!(head.contains("content-type: image/png"), "{head}");
    assert!(head.contains("cache-control: private, max-age=31536000, immutable"));
    assert!(head.contains("x-content-type-options: nosniff"));

    let a = d.admin(
        "POST",
        "/api/look/widgets",
        json!({"name": "Clock", "source": "<b>1</b>"}),
    );
    assert_eq!(a.status, 200);
    assert_eq!(a.json()["widget"]["id"], "w1");
    let a = d.admin("DELETE", "/api/look/widgets/w1", Value::Null);
    assert_eq!(a.json()["deleted"], true);
    let a = d.admin("DELETE", "/api/look/widgets/w1", Value::Null);
    assert_eq!(a.json()["deleted"], false);

    // Written where the native look kept it, so a box keeps its look.
    let on_disk: Value =
        serde_json::from_slice(&std::fs::read(d.dir.path().join("state/look.json")).unwrap())
            .unwrap();
    assert_eq!(on_disk["veil"], 35);
    assert_eq!(on_disk["nextId"], 2);
    assert_eq!(
        std::fs::read(d.dir.path().join("state/background.img")).unwrap(),
        PNG
    );
    // Only root reads them, as the backup's restore puts them back.
    for f in ["state/look.json", "state/background.img"] {
        let mode = std::fs::metadata(d.dir.path().join(f))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600, "{f}");
    }

    // Choosing nothing drops the upload.
    let a = d.admin("DELETE", "/api/look/background", Value::Null);
    assert_eq!(a.json()["background"]["kind"], "none");
    assert!(!d.dir.path().join("state/background.img").exists());
    let a = d.call("GET", "/api/look/background", &[], b"");
    assert_eq!(a.status, 404);
}

#[test]
fn each_person_keeps_their_own_background() {
    let d = Daemon::start();
    // The box's own background, as a box from before kept it.
    let bearer = format!("Bearer {TOKEN}");
    let a = d.call(
        "PUT",
        "/api/look/background",
        &[("Authorization", &bearer)],
        PNG,
    );
    assert_eq!(a.status, 200);
    assert_eq!(a.json()["personal"], false);

    // Bob, who is in no section at all, picks his own.
    let a = d.person_send(
        "bob",
        "POST",
        "/api/look",
        br#"{"background": {"kind": "shipped", "name": "dusk"}, "veil": 30}"#,
    );
    assert_eq!(a.status, 200, "{}", String::from_utf8_lossy(&a.body));
    assert_eq!(a.json()["personal"], true);
    assert_eq!(a.json()["background"]["name"], "dusk");
    // Ada starts plain: the box's picture is not hers.
    let a = d.person("ada", "GET", "/api/look");
    assert_eq!(a.json()["background"]["kind"], "none");
    assert_eq!(a.json()["veil"], 60);
    // Her upload is hers, read back with her sign-in and nobody else's.
    let a = d.call(
        "PUT",
        "/api/look/background",
        &[("Remote-User", "ada")],
        b"\xff\xd8\xff\xe0JFIF",
    );
    assert_eq!(a.status, 200, "{}", String::from_utf8_lossy(&a.body));
    assert_eq!(
        a.json()["background"]["url"],
        "/api/look/background/ada?v=1"
    );
    let a = d.person("ada", "GET", "/api/look/background/ada?v=1");
    assert_eq!(a.status, 200);
    assert!(a
        .head
        .to_ascii_lowercase()
        .contains("content-type: image/jpeg"));
    assert_eq!(
        d.person("bob", "GET", "/api/look/background/ada?v=1")
            .status,
        404
    );
    assert_eq!(
        d.call("GET", "/api/look/background/ada?v=1", &[], b"")
            .status,
        401
    );
    // Bob's choice did not reach the spare key's page.
    let a = d.admin("GET", "/api/look", Value::Null);
    assert_eq!(a.json()["background"]["kind"], "upload");
    assert_eq!(a.json()["background"]["url"], "/api/look/background?v=1");

    // The owner's first look is a copy of the box's.
    let a = d.person("ola", "GET", "/api/look");
    assert_eq!(a.status, 200);
    assert_eq!(a.json()["background"]["kind"], "upload");
    assert_eq!(
        a.json()["background"]["url"],
        "/api/look/background/ola?v=1"
    );
    let a = d.person("ola", "GET", "/api/look/background/ola?v=1");
    assert_eq!(a.body, PNG);

    // Widgets stay the look section's.
    let a = d.person_send(
        "bob",
        "POST",
        "/api/look/widgets",
        br#"{"name": "x", "source": "<b/>"}"#,
    );
    assert_eq!(a.status, 403);

    // On disk, beside the box's, readable by root only.
    for f in [
        "state/looks/ada.json",
        "state/looks/ada.img",
        "state/looks/bob.json",
        "state/looks/ola.img",
    ] {
        let mode = std::fs::metadata(d.dir.path().join(f))
            .unwrap_or_else(|e| panic!("{f}: {e}"))
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600, "{f}");
    }
    let bob: Value =
        serde_json::from_slice(&std::fs::read(d.dir.path().join("state/looks/bob.json")).unwrap())
            .unwrap();
    assert_eq!(bob["veil"], 30);
}

#[test]
fn the_pins_are_served_by_their_plugin() {
    let d = Daemon::start();
    let a = d.admin("GET", "/api/pins", Value::Null);
    assert_eq!(a.status, 200);
    assert_eq!(a.json(), json!({"pins": [], "limit": 24}));

    let pins = json!({"pins": [
        {"kind": "tile", "app": "files"},
        {"kind": "repo", "owner": "notshared", "name": "site"},
        {"kind": "file", "id": 42, "path": "/Documents/plan.pdf", "folder": false},
    ]});
    let a = d.admin("PUT", "/api/pins", pins.clone());
    assert_eq!(a.status, 200);
    assert_eq!(a.json()["pins"], pins["pins"]);
    assert_eq!(
        d.admin("GET", "/api/pins", Value::Null).json()["pins"],
        pins["pins"]
    );

    // A refused list says why and leaves the kept one alone.
    for bad in [
        json!({"pins": [{"kind": "tile", "app": "files"}, {"kind": "tile", "app": "files"}]}),
        json!({"pins": [{"kind": "repo", "owner": "a", "name": "b", "href": "https://x"}]}),
        json!({"pins": [{"kind": "file", "id": 1, "path": "/a/../etc", "folder": true}]}),
        json!("nonsense"),
    ] {
        let a = d.admin("PUT", "/api/pins", bad.clone());
        assert_eq!(a.status, 400, "{bad}");
        assert!(
            a.json()["error"].as_str().is_some_and(|e| !e.is_empty()),
            "{bad}"
        );
    }
    assert_eq!(
        d.admin("GET", "/api/pins", Value::Null).json()["pins"],
        pins["pins"]
    );

    // Where the native list kept it, readable by root only.
    let path = d.dir.path().join("state/pins.json");
    let on_disk: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(on_disk["pins"], pins["pins"]);
    let mode = std::fs::metadata(&path).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600);

    // The sidebar is the administrators': sections.json says admin.
    let a = d.person("ada", "GET", "/api/pins");
    assert_eq!(a.status, 403);
    assert_eq!(d.person("root", "GET", "/api/pins").status, 200);
}

#[test]
fn the_pages_answer_on_a_box_where_nothing_was_saved_yet() {
    // The build's blocks.json and defaults, and a repository directory
    // that does not exist yet, as on a box nobody customized.
    let ui = TempDir::new().unwrap();
    let pages = Path::new(env!("CARGO_MANIFEST_DIR")).join("../admin-ui/app/src/pages");
    std::fs::create_dir_all(ui.path().join("pages")).unwrap();
    for file in ["blocks.json", "overview.mdx", "settings.storage.mdx"] {
        std::fs::copy(pages.join(file), ui.path().join("pages").join(file)).unwrap();
    }
    let repo = ui.path().join("repo");
    let d = Daemon::start_plugins(
        &["pages"],
        &[
            ("LOSOS_ADMIN_UI_DIR", ui.path().to_str().unwrap()),
            ("LOSOS_PAGES_DIR", repo.to_str().unwrap()),
        ],
    );

    let a = d.admin("GET", "/api/pages", Value::Null);
    assert_eq!(a.status, 200, "{}", String::from_utf8_lossy(&a.body));
    assert_eq!(a.json(), json!({"pages": {}, "style": null}));
    let a = d.admin("GET", "/api/pages/overview", Value::Null);
    assert_eq!(a.status, 200, "{}", String::from_utf8_lossy(&a.body));
    let shipped = std::fs::read_to_string(pages.join("overview.mdx")).unwrap();
    assert_eq!(a.json()["source"], json!(shipped));
    assert_eq!(a.json()["custom"], json!(false));
    assert_eq!(a.json()["history"], json!([]));
    assert!(!repo.exists(), "a read made the repository");

    let source = "<BoxName />\n\n## Welcome\n\n<Apps hideStorage />\n";
    let a = d.admin("PUT", "/api/pages/overview", json!({"source": source}));
    assert_eq!(a.status, 200, "{}", String::from_utf8_lossy(&a.body));
    assert!(repo.join(".git").is_dir());
    let a = d.admin("GET", "/api/pages/overview", Value::Null);
    assert_eq!(a.json()["source"], json!(source));
    assert_eq!(a.json()["custom"], json!(true));
    assert_eq!(a.json()["history"].as_array().map(Vec::len), Some(1));
}

#[test]
fn the_hidden_addresses_are_served_by_their_plugin() {
    const ONION: &str = "abcdefghijklmnopqrstuvwxyz234567abcdefghijklmnopqrstuvwx.onion";
    const B32: &str = "abcdefghijklmnopqrstuvwxyz234567abcdefghijklmnopqrst.b32.i2p";
    let hidden = TempDir::new().unwrap();
    let d = Daemon::start_with(&[("LOSOS_HIDDEN_DIR", hidden.path().to_str().unwrap())]);
    // Neither network on: no files, no names.
    let none = json!({"box": {"tor": null, "i2p": null}, "edge": {"tor": null, "i2p": null}});
    let a = d.admin("GET", "/api/hidden", Value::Null);
    assert_eq!(a.status, 200);
    assert_eq!(a.json(), none);

    // What losos-hidden-sync and the heartbeat wrote, with a name that is
    // not one dropped on the way.
    std::fs::write(
        hidden.path().join("addresses.json"),
        json!({"front": {"tor": ONION, "i2p": null}}).to_string(),
    )
    .unwrap();
    std::fs::write(
        hidden.path().join("edge.json"),
        json!({"tor": "javascript:alert(1)", "i2p": B32}).to_string(),
    )
    .unwrap();
    assert_eq!(
        d.admin("GET", "/api/hidden", Value::Null).json(),
        json!({"box": {"tor": ONION, "i2p": null}, "edge": {"tor": null, "i2p": B32}})
    );

    // The Network page's: sections.json says network.
    assert_eq!(d.person("ada", "GET", "/api/hidden").status, 403);
    assert_eq!(d.person("root", "GET", "/api/hidden").status, 200);
}

#[test]
fn a_plugin_route_is_held_to_the_same_sign_in_and_sections() {
    let d = Daemon::start();
    // Nobody: 401, before the plugin is ever called.
    assert_eq!(d.call("GET", "/api/look", &[], b"").status, 401);
    assert_eq!(
        d.call(
            "POST",
            "/api/look",
            &[("Authorization", "Bearer wrong")],
            b"{}"
        )
        .status,
        401
    );
    // Signed in: `GET /api/look` is signed-in in sections.json.
    assert_eq!(d.person("bob", "GET", "/api/look").status, 200);
    // A section the person lacks: the probe's routes are in none, so admin.
    let a = d.person("ada", "GET", "/api/probe/echo");
    assert_eq!(a.status, 403);
    assert_eq!(a.json()["section"], "admin");
    // An administrator reaches it, and the plugin sees who they are.
    let a = d.person("root", "GET", "/api/probe/echo?x=1&y=a+b");
    assert_eq!(a.status, 200);
    let echo = a.json();
    assert_eq!(echo["caller"]["name"], "root");
    assert_eq!(echo["caller"]["admin"], true);
    assert_eq!(echo["section"], "admin");
    assert_eq!(echo["query"]["y"], "a b");
    // The credentials stay with lososd.
    let a = d.admin("GET", "/api/probe/echo", Value::Null);
    assert_eq!(a.json()["caller"], "spare");
    let headers: Vec<String> = serde_json::from_value(a.json()["headers"].clone()).unwrap();
    assert!(!headers.iter().any(|h| h == "authorization"), "{headers:?}");
    // Someone the people file does not know.
    assert_eq!(d.person("mallory", "GET", "/api/look").status, 403);
    // An unguarded route needs nothing from lososd.
    let a = d.call("GET", "/api/probe/open", &[], b"");
    assert_eq!((a.status, a.body.as_slice()), (200, b"open".as_slice()));
    // Unknown routes are 404, known ones with the wrong method too.
    assert_eq!(d.admin("GET", "/api/nothing-here", Value::Null).status, 404);
    assert_eq!(d.admin("PATCH", "/api/look", Value::Null).status, 404);
}

#[test]
fn routes_params_and_limits() {
    let d = Daemon::start();
    let a = d.admin("GET", "/api/probe/items/a%20b", Value::Null);
    assert_eq!(a.json()["id"], "a b");
    assert!(a.head.to_ascii_lowercase().contains("x-probe: 1"));
    // The more literal route wins.
    let a = d.admin("GET", "/api/probe/items/new", Value::Null);
    assert_eq!(a.status, 200);
    assert_eq!(a.json()["new"], true);
    // A body over the route's maxBody is refused before the plugin runs.
    let bearer = format!("Bearer {TOKEN}");
    let big = vec![b'x'; 2048];
    let a = d.call(
        "POST",
        "/api/probe/echo",
        &[("Authorization", &bearer)],
        &big,
    );
    assert_eq!(a.status, 413);
    let a = d.call(
        "POST",
        "/api/probe/echo",
        &[("Authorization", &bearer)],
        &big[..1000],
    );
    assert_eq!(a.json()["bodyBytes"], 1000);
    let a = d.admin("GET", "/api/probe/time", Value::Null);
    assert!(a.json()["now"].as_u64().unwrap() > 1_700_000_000);
    assert_eq!(a.json()["random"].as_str().unwrap().len(), 16);
}

#[test]
fn files_stay_inside_the_manifest() {
    let d = Daemon::start();
    let probe = d.probe();
    let inside = probe.join("note").display().to_string();
    assert_eq!(
        d.poke("/api/probe/write", json!({"path": inside, "data": "hi"}))["ok"],
        true
    );
    assert_eq!(
        d.poke("/api/probe/read", json!({"path": inside}))["ok"],
        "hi"
    );
    let mode = std::fs::metadata(&inside).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);

    let secret = d.dir.path().join("secret").display().to_string();
    for path in [
        secret.clone(),
        "/etc/passwd".to_string(),
        format!("{}/../secret", probe.display()),
    ] {
        let r = d.poke("/api/probe/read", json!({"path": path}));
        assert!(
            r["error"].as_str().unwrap().starts_with("denied"),
            "{path}: {r}"
        );
    }
    // The ro root inside the rw one holds.
    let sealed = probe.join("sealed/x").display().to_string();
    let r = d.poke("/api/probe/write", json!({"path": sealed, "data": "x"}));
    assert!(r["error"].as_str().unwrap().starts_with("denied"), "{r}");
    // A link planted inside the root is not followed.
    std::os::unix::fs::symlink(&secret, probe.join("link")).unwrap();
    let r = d.poke("/api/probe/read", json!({"path": probe.join("link")}));
    assert!(r["error"].as_str().unwrap().starts_with("denied"), "{r}");
    let r = d.poke(
        "/api/probe/write",
        json!({"path": probe.join("link"), "data": "pwned"}),
    );
    assert!(r["error"].as_str().unwrap().starts_with("denied"), "{r}");
    assert_eq!(std::fs::read_to_string(&secret).unwrap(), "not yours");
    let r = d.poke("/api/probe/read", json!({"path": probe.join("missing")}));
    assert_eq!(r["notFound"], true);
}

#[test]
fn programs_endpoints_and_locks_need_the_manifest() {
    let d = Daemon::start();
    let r = d.poke(
        "/api/probe/exec",
        json!({"program": "echo", "args": ["a", "$HOME"]}),
    );
    assert_eq!(r["ok"], "a $HOME\n");
    for program in ["cat", "/bin/sh", "sh"] {
        let r = d.poke(
            "/api/probe/exec",
            json!({"program": program, "args": ["/etc/passwd"]}),
        );
        assert!(
            r["error"].as_str().unwrap().starts_with("denied"),
            "{program}: {r}"
        );
    }
    // No endpoint is granted here: lososd's own API is not one either.
    let url = format!("http://127.0.0.1:{}/api/state", d.port);
    let r = d.poke("/api/probe/http", json!({"url": url}));
    assert!(r["error"].as_str().unwrap().starts_with("denied"), "{r}");
    let r = d.poke("/api/probe/lock", json!({"name": "probe"}));
    assert_eq!(r["ok"], true);
    let r = d.poke("/api/probe/lock", json!({"name": "state"}));
    assert!(r["error"].as_str().unwrap().starts_with("denied"), "{r}");
}

#[test]
fn http_reaches_a_granted_endpoint() {
    // A one-shot server standing in for Ollama or LosOS Cloud.
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let origin = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    std::thread::spawn(move || {
        for stream in listener.incoming().take(1) {
            let mut s = stream.unwrap();
            let mut buf = [0u8; 4096];
            let _ = s.read(&mut buf);
            let _ = s.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 5\r\n\r\nhello",
            );
        }
    });
    let d = Daemon::start_with(&[("LOSOS_PROBE_HTTP", &origin)]);
    let r = d.poke(
        "/api/probe/http",
        json!({"url": format!("{origin}/api/tags")}),
    );
    assert_eq!(r["ok"]["status"], 200, "{r}");
    assert_eq!(r["ok"]["body"], "hello");
    let r = d.poke(
        "/api/probe/http",
        json!({"url": format!("{origin}.evil.example/x")}),
    );
    assert!(r["error"].as_str().unwrap().starts_with("denied"), "{r}");
}

#[test]
fn a_runaway_plugin_is_stopped_and_the_daemon_carries_on() {
    let d = Daemon::start();
    for route in ["/api/probe/spin", "/api/probe/grow", "/api/probe/panic"] {
        let started = Instant::now();
        let a = d.admin("GET", route, Value::Null);
        assert_eq!(a.status, 500, "{route}");
        assert_eq!(a.json()["error"], "command failed; see the lososd journal");
        // The spin is cut at the manifest's 2 s.
        assert!(started.elapsed() < Duration::from_secs(10), "{route}");
    }
    // Nothing of a failed call leaks into the next one.
    assert_eq!(d.admin("GET", "/api/look", Value::Null).status, 200);
    assert_eq!(
        d.admin("GET", "/api/probe/items/x", Value::Null).json()["id"],
        "x"
    );
}

#[test]
fn a_lock_makes_a_second_call_wait() {
    let d = std::sync::Arc::new(Daemon::start());
    let d2 = d.clone();
    let started = Instant::now();
    let other = std::thread::spawn(move || d2.poke("/api/probe/hold", Value::Null));
    std::thread::sleep(Duration::from_millis(100));
    let r = d.poke("/api/probe/hold", Value::Null);
    assert_eq!(r["ok"], true);
    assert_eq!(other.join().unwrap()["ok"], true);
    assert!(
        started.elapsed() >= Duration::from_millis(1000),
        "the two holds overlapped"
    );
}

#[test]
fn the_cache_jobs_and_timers_outlive_a_call() {
    let d = Daemon::start();
    assert_eq!(
        d.poke("/api/probe/cache", json!({"set": "kept"}))["value"],
        "kept"
    );
    assert_eq!(d.poke("/api/probe/cache", Value::Null)["value"], "kept");

    let marker = d.probe().join("job-done");
    let r = d.poke("/api/probe/job", json!({"path": marker}));
    assert_eq!(r["ok"], true, "{r}");
    let deadline = Instant::now() + Duration::from_secs(10);
    while !marker.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(std::fs::read_to_string(&marker).unwrap(), "done");

    // `start` once, `beat` every second.
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let ticks = std::fs::read_to_string(d.probe().join("ticks")).unwrap_or_default();
        if ticks.lines().filter(|l| *l == "beat").count() >= 2 {
            assert_eq!(
                ticks.lines().filter(|l| *l == "start").count(),
                1,
                "{ticks}"
            );
            break;
        }
        assert!(Instant::now() < deadline, "timers did not fire: {ticks:?}");
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn a_dispatched_call_is_checked_as_its_caller() {
    let d = Daemon::start();
    let r = d.poke("/api/probe/dispatch", json!({"path": "/api/probe/items/z"}));
    assert_eq!(r["ok"]["status"], 200, "{r}");
    assert_eq!(r["ok"]["body"]["id"], "z");
    // root may dispatch, but the call keeps root's caller and its checks.
    let bearer = format!("Bearer {TOKEN}");
    let body = json!({"path": "/api/probe/echo"}).to_string();
    let a = d.call(
        "POST",
        "/api/probe/dispatch",
        &[("Authorization", &bearer), ("X-Losos-As", "ada")],
        body.as_bytes(),
    );
    // ada may not reach the dispatch route itself (admin), so 403 first.
    assert_eq!(a.status, 403);
}

#[test]
fn a_bad_plugin_directory_stops_the_daemon() {
    // A writable package: lososd refuses to start rather than run without.
    let dir = TempDir::new().unwrap();
    let root = dir.path().join("store");
    let pkg = root.join("0000-losos-plugins");
    std::fs::create_dir_all(&pkg).unwrap();
    let (name, wasm, manifest) = &components()[0];
    std::fs::write(pkg.join(format!("{name}.wasm")), wasm).unwrap();
    std::fs::write(pkg.join(format!("{name}.json")), manifest).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_lososd"))
        .env("LOSOS_NO_DBUS", "1")
        .env("LOSOS_STATE_DIR", dir.path().join("state"))
        .env("LOSOS_ADMIN_TOKEN_FILE", dir.path().join("token"))
        .env("LOSOS_ADMIN_PORT", free_port().to_string())
        .env("LOSOS_PLUGIN_DIR", &pkg)
        .env("LOSOS_PLUGIN_TRUST_ROOT", &root)
        .output()
        .unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("writable"), "{stderr}");

    // And the same files outside the trusted root.
    let out = Command::new(env!("CARGO_BIN_EXE_lososd"))
        .env("LOSOS_NO_DBUS", "1")
        .env("LOSOS_STATE_DIR", dir.path().join("state"))
        .env("LOSOS_ADMIN_TOKEN_FILE", dir.path().join("token"))
        .env("LOSOS_ADMIN_PORT", free_port().to_string())
        .env("LOSOS_PLUGIN_DIR", &pkg)
        .output()
        .unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("outside"), "{stderr}");
}
