//! `losos agents` and `losos mcp` in a home folder of their own: each AI
//! tool's settings in its own format, the tools that take the companion
//! through a command of their own played by tests/fake-bin/agent-tool, and
//! a box on loopback that signs the companion in the way LosOS Login does
//! and answers its calls.

mod common;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use common::{fake_bin, mock, owner, Client, Mock, TOKEN};
use losos::agents::companion;
use losos::envelope::Transport;
use losos::keychain::{Keychain, MemoryKeychain};
use serde_json::{json, Value};

const SELF: &str = "/opt/losos/bin/losos";

/// Zed's settings as a person keeps them: comments, a trailing comma, a
/// context server of their own.
const ZED: &str = r#"// Zed settings
{
  "theme": "One Dark", // the dark one
  "context_servers": {
    /* the owner's own */
    "github": { "command": "gh-mcp", "args": [] },
  },
}
"#;

const OPENCODE: &str = r#"{
  // opencode, with JSONC
  "$schema": "https://opencode.ai/config.json",
  "model": "anthropic/claude"
}
"#;

/// Gemini CLI's list of trusted folders, as it writes it.
const GEMINI_TRUST: &str = r#"{
  "/home/me/work": "TRUST_FOLDER"
}"#;

struct Desk {
    host: Mock,
    keychain: Arc<MemoryKeychain>,
    client: Client,
    home: tempfile::TempDir,
}

impl Desk {
    fn path(&self, rel: &str) -> PathBuf {
        self.home.path().join(rel)
    }

    fn read(&self, rel: &str) -> String {
        std::fs::read_to_string(self.path(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
    }

    fn write(&self, rel: &str, text: &str) {
        let path = self.path(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn json(&self, rel: &str) -> Value {
        serde_json::from_str(&self.read(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
    }

    fn log(&self) -> String {
        std::fs::read_to_string(self.path("agent.log")).unwrap_or_default()
    }

    fn launch(&self) -> Value {
        json!({ "command": SELF, "args": ["mcp", "--host", self.host.url] })
    }
}

/// A PATH of its own holding `tools` (played by the fake) and the few
/// programs the fake runs, so nothing on the machine's own PATH is found.
fn bin(home: &Path, tools: &[&str]) -> String {
    let dir = home.join("bin");
    std::fs::create_dir_all(&dir).unwrap();
    for tool in tools {
        std::os::unix::fs::symlink(fake_bin("agent-tool"), dir.join(tool)).unwrap();
    }
    for program in ["basename", "dirname", "mkdir", "cp", "rm"] {
        let found = std::env::split_paths(&std::env::var("PATH").unwrap())
            .map(|d| d.join(program))
            .find(|p| p.is_file())
            .unwrap_or_else(|| panic!("{program} is not on PATH"));
        std::os::unix::fs::symlink(found, dir.join(program)).unwrap();
    }
    dir.display().to_string()
}

/// A logged-in box, and a home folder with `tools` on PATH.
fn desk(tools: &[&str]) -> Desk {
    let me = owner();
    let host = mock(
        false,
        me.key.armored_public(),
        Box::new(|method, path, _| match (method, path) {
            ("GET", "/api/me") => (
                200,
                json!({ "name": "matus", "displayName": "Matus", "admin": true }),
            ),
            ("GET", "/api/status") => (200, json!({ "health": "ok" })),
            ("GET", p) if p.starts_with("/api/cloud/files") => (
                200,
                json!({ "path": "/", "entries": [{ "name": "Notes", "kind": "folder" }] }),
            ),
            _ => (404, json!({ "error": "no such route" })),
        }),
    );
    let keychain = Arc::new(MemoryKeychain::new());
    let home = tempfile::tempdir().unwrap();
    let mut client = Client::with(keychain.clone());
    let at = |rel: &str| home.path().join(rel).display().to_string();
    client
        .set("HOME", &at(""))
        .set("XDG_CONFIG_HOME", &at(".config"))
        .set("XDG_DATA_HOME", &at(".local/share"))
        .set("PATH", &bin(home.path(), tools))
        .set("FAKE_AGENT_LOG", &at("agent.log"))
        .set("LOSOS_SELF", SELF)
        .set("LOSOS_DEVICE", "laptop");
    client.run(&["key", "import", me.file.path().to_str().unwrap()], "");
    let r = client.run(
        &["login", &host.url, "--fingerprint", &host.fingerprint()],
        TOKEN,
    );
    assert_eq!(r.code, 0, "{}", r.err);
    Desk {
        host,
        keychain,
        client,
        home,
    }
}

#[test]
fn the_tools_are_found_by_their_programs_and_their_folders() {
    let d = desk(&["claude", "codex"]);
    d.write(".config/zed/settings.json", ZED);
    std::fs::create_dir_all(d.path(".cursor")).unwrap();
    let r = d.client.run(&["agents"], "");
    assert_eq!(r.code, 0, "{}", r.err);
    for name in ["Claude Code", "Codex", "Cursor", "Zed"] {
        assert!(r.out.contains(name), "{name} missing:\n{}", r.out);
    }
    for name in ["Gemini CLI", "VS Code", "opencode", "Windsurf"] {
        assert!(!r.out.contains(name), "{name} is not here:\n{}", r.out);
    }
    assert!(r.out.contains("not yet"), "{}", r.out);
    assert!(r.out.contains("losos agents install"), "{}", r.out);
    // Looking writes nothing and runs no tool.
    assert_eq!(d.read(".config/zed/settings.json"), ZED);
    assert_eq!(d.log(), "");

    // The installer's one line names them, and says nothing once the
    // companion is in one.
    let hint = d.client.run(&["agents", "hint"], "");
    assert!(
        hint.out.contains("Claude Code, Codex, Cursor, Zed"),
        "{}",
        hint.out
    );
    let none = desk(&[]);
    assert_eq!(none.client.run(&["agents", "hint"], "").out, "");
    let listed = none.client.run(&["agents"], "");
    assert!(listed.out.contains("No AI tool"), "{}", listed.out);
}

#[test]
fn install_puts_the_companion_in_each_tool_and_remove_takes_it_out_again() {
    let d = desk(&["claude", "codex", "gemini", "copilot"]);
    d.write(".config/zed/settings.json", ZED);
    d.write(".config/opencode/opencode.jsonc", OPENCODE);
    d.write(".gemini/trustedFolders.json", GEMINI_TRUST);
    std::fs::create_dir_all(d.path(".cursor")).unwrap();
    std::fs::create_dir_all(d.path(".config/Code")).unwrap();

    let r = d.client.run(&["agents", "install", "--yes"], "");
    assert_eq!(r.code, 0, "{}\n{}", r.out, r.err);
    // The sign-in: a link with the code in it, and the code to compare.
    let link = format!(
        "{}/auth/consent/openid/device-authorization?user_code=WDJBMJHT",
        d.host.url
    );
    assert!(r.out.contains(&link), "{}", r.out);
    assert!(r.out.contains("shows the code WDJBMJHT"), "{}", r.out);
    assert!(r.out.contains("Signed in."), "{}", r.out);
    assert_eq!(d.host.login.lock().unwrap().polls, 2);
    for name in [
        "Claude Code",
        "Codex",
        "Gemini CLI",
        "GitHub Copilot CLI",
        "opencode",
        "Cursor",
        "VS Code",
        "Zed",
    ] {
        assert!(
            r.out
                .contains(&format!("The LosOS companion is in {name}.")),
            "{name}:\n{}",
            r.out
        );
    }

    // Zed: the entry in its own shape, everything else byte for byte.
    let zed = d.read(".config/zed/settings.json");
    for kept in [
        "// Zed settings",
        "// the dark one",
        "/* the owner's own */",
        "\"github\"",
    ] {
        assert!(zed.contains(kept), "{kept} lost:\n{zed}");
    }
    let zed_entry = losos::agents::jsonc::get(&zed, &["context_servers"], "losos")
        .unwrap()
        .unwrap();
    assert_eq!(zed_entry, d.launch());
    // ... and the file as it was is kept.
    let backups: Vec<PathBuf> =
        std::fs::read_dir(d.path(".local/share/losos/companion/backups/zed"))
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
    assert_eq!(backups.len(), 1);
    assert_eq!(std::fs::read_to_string(&backups[0]).unwrap(), ZED);

    // opencode: its JSONC file, its own shape.
    let opencode = d.read(".config/opencode/opencode.jsonc");
    assert!(opencode.contains("// opencode, with JSONC"), "{opencode}");
    assert_eq!(
        losos::agents::jsonc::get(&opencode, &["mcp"], "losos")
            .unwrap()
            .unwrap(),
        json!({ "type": "local", "command": [SELF, "mcp", "--host", d.host.url], "enabled": true })
    );
    // Cursor and VS Code: files made where there were none.
    assert_eq!(
        d.json(".cursor/mcp.json")["mcpServers"]["losos"],
        d.launch()
    );
    let mut typed = d.launch();
    typed["type"] = json!("stdio");
    assert_eq!(
        d.json(".config/Code/User/mcp.json")["servers"]["losos"],
        typed
    );

    // The plugin tools were handed the plugin through their own commands.
    let companion_dir = d.path(".local/share/losos/companion");
    let log = d.log();
    let at = |id: &str| companion_dir.join(id).display().to_string();
    for line in [
        format!("claude plugin marketplace add {0} @ {0}", at("claude-code")),
        format!("claude plugin install losos@losos @ {}", at("claude-code")),
        format!("codex plugin marketplace add {0} @ {0}", at("codex")),
        format!("codex plugin add losos@losos @ {}", at("codex")),
        format!(
            "gemini extensions install --consent --skip-settings {0} @ {0}",
            at("gemini")
        ),
        format!("copilot plugin install losos@losos @ {}", at("copilot")),
    ] {
        assert!(log.contains(&line), "{line} missing:\n{log}");
    }
    let plugin = companion_dir.join("claude-code/plugins/losos");
    let mcp: Value =
        serde_json::from_str(&std::fs::read_to_string(plugin.join(".mcp.json")).unwrap()).unwrap();
    assert_eq!(mcp["mcpServers"]["losos"], d.launch());
    assert!(plugin.join("skills/losos/SKILL.md").is_file());
    let codex: Value = serde_json::from_str(
        &std::fs::read_to_string(
            companion_dir.join("codex/plugins/losos/.codex-plugin/plugin.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(codex["mcpServers"], "./.mcp.json");
    assert!(codex["interface"]["displayName"].is_string());
    let gemini = d.json(".gemini/extensions/losos/gemini-extension.json");
    assert_eq!(gemini["mcpServers"]["losos"], d.launch());
    // Gemini CLI asks before it installs from a folder it does not trust:
    // the companion's folder is trusted, and the owner's own list stays.
    let trust = d.json(".gemini/trustedFolders.json");
    let gemini_dir = std::fs::canonicalize(at("gemini")).unwrap();
    assert_eq!(trust[gemini_dir.display().to_string()], "TRUST_FOLDER");
    assert_eq!(trust["/home/me/work"], "TRUST_FOLDER");

    // The companion's sign-in is in the keychain: the tokens LosOS Login
    // handed out, and nothing of the owner's.
    let kept = companion::list(d.keychain.as_ref()).unwrap();
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].url, d.host.url);
    assert_eq!(kept[0].access_token, "authelia_at_1.sig");
    assert_eq!(kept[0].refresh_token, "authelia_rt_1.sig");
    assert!(!d.read(".config/zed/settings.json").contains("authelia_"));

    let listed = d.client.run(&["agents"], "");
    assert_eq!(
        listed.out.matches("companion in").count(),
        8,
        "{}",
        listed.out
    );

    // A second install signs in no second time: the box still takes the
    // kept sign-in.
    let again = d.client.run(&["agents", "install", "zed", "--yes"], "");
    assert_eq!(again.code, 0, "{}", again.err);
    assert!(again.out.contains("already"), "{}", again.out);
    assert_eq!(d.host.login.lock().unwrap().polls, 2);

    let r = d.client.run(&["agents", "remove", "--yes"], "");
    assert_eq!(r.code, 0, "{}\n{}", r.out, r.err);
    // Everything the owner wrote is back as it was.
    assert_eq!(d.read(".config/zed/settings.json"), ZED);
    assert_eq!(d.read(".config/opencode/opencode.jsonc"), OPENCODE);
    assert_eq!(d.read(".gemini/trustedFolders.json"), GEMINI_TRUST);
    assert!(d.json(".cursor/mcp.json")["mcpServers"]
        .get("losos")
        .is_none());
    assert!(!d.path(".gemini/extensions/losos").exists());
    assert!(!companion_dir.join("claude-code").exists());
    let log = d.log();
    for line in [
        "claude plugin uninstall losos@losos",
        "claude plugin marketplace remove losos",
        "codex plugin remove losos@losos",
        "gemini extensions uninstall losos",
    ] {
        assert!(log.contains(line), "{line} missing:\n{log}");
    }
    // With the companion in no tool, it is signed out: both tokens are
    // handed back to the box and the keychain forgets them.
    assert!(companion::list(d.keychain.as_ref()).unwrap().is_empty());
    assert!(r.out.contains("signed out of"), "{}", r.out);
    let login = d.host.login.lock().unwrap();
    assert!(login.revoked.contains(&"authelia_rt_1.sig".to_string()));
    assert!(login.revoked.contains(&"authelia_at_1.sig".to_string()));
    assert!(login.access.is_empty() && login.refresh.is_none());
    drop(login);
    let listed = d.client.run(&["agents"], "");
    assert!(!listed.out.contains("companion in"), "{}", listed.out);
}

#[test]
fn each_tool_is_asked_about_and_a_no_leaves_it_alone() {
    let d = desk(&["claude"]);
    d.write(".config/zed/settings.json", ZED);
    // Claude Code comes first in the list, then Zed.
    let r = d.client.run(&["agents", "install"], "y\nn\n");
    assert_eq!(r.code, 0, "{}\n{}", r.out, r.err);
    assert!(
        r.err
            .contains("Put the LosOS companion in Claude Code? [y/N]"),
        "{}",
        r.err
    );
    assert!(
        r.err.contains("Put the LosOS companion in Zed? [y/N]"),
        "{}",
        r.err
    );
    assert!(r.out.contains("is in Claude Code"), "{}", r.out);
    assert!(!r.out.contains("is in Zed"), "{}", r.out);
    assert_eq!(d.read(".config/zed/settings.json"), ZED);

    // No to everything: no sign-in, nothing kept, nothing written.
    let d = desk(&["claude"]);
    d.write(".config/zed/settings.json", ZED);
    let r = d.client.run(&["agents", "install"], "n\n");
    assert_eq!(r.code, 0, "{}", r.err);
    assert!(r.out.contains("Nothing was changed."), "{}", r.out);
    assert!(!r.out.contains("LosOS Login"), "{}", r.out);
    assert_eq!(d.host.login.lock().unwrap().polls, 0);
    assert_eq!(d.log(), "");
    assert_eq!(d.read(".config/zed/settings.json"), ZED);
    assert!(companion::list(d.keychain.as_ref()).unwrap().is_empty());
}

#[test]
fn a_losos_entry_someone_else_wrote_is_left_alone_and_a_failing_tool_is_reported() {
    let mut d = desk(&["codex"]);
    let theirs = "{\n  \"mcpServers\": {\n    \"losos\": { \"command\": \"npx\", \"args\": [\"some-other-losos\"] }\n  }\n}\n";
    d.write(".cursor/mcp.json", theirs);
    d.write(".config/zed/settings.json", ZED);
    d.client.set("FAKE_AGENT_FAIL", "codex");
    let r = d.client.run(&["agents", "install", "--yes"], "");
    assert_eq!(r.code, 1, "{}\n{}", r.out, r.err);
    assert!(r.err.contains("did not write"), "{}", r.err);
    assert!(r.err.contains("exit status 3"), "{}", r.err);
    assert_eq!(d.read(".cursor/mcp.json"), theirs);
    // The others still went in.
    assert!(r.out.contains("is in Zed"), "{}", r.out);
}

#[test]
fn a_named_tool_must_be_one_losos_knows_and_a_denied_sign_in_writes_nothing() {
    let d = desk(&[]);
    let r = d.client.run(&["agents", "install", "notepad"], "");
    assert_eq!(r.code, 1);
    assert!(r.err.contains("notepad"), "{}", r.err);
    assert!(r.err.contains("claude-code"), "{}", r.err);
    // A named app is taken even where nothing gives it away; the sign-in
    // comes before anything is written.
    d.host.login.lock().unwrap().deny = true;
    let r = d
        .client
        .run(&["agents", "install", "windsurf", "--yes"], "");
    assert_eq!(r.code, 1);
    assert!(r.err.contains("denied in the browser"), "{}", r.err);
    assert!(!d.path(".codeium").exists());
    assert!(companion::list(d.keychain.as_ref()).unwrap().is_empty());
}

fn mcp_session(client: &Client, host: &str, calls: &[Value]) -> Vec<Value> {
    let mut lines = vec![
        json!({ "jsonrpc": "2.0", "id": 0, "method": "initialize",
            "params": { "protocolVersion": "2025-06-18", "capabilities": {},
                "clientInfo": { "name": "test", "version": "1" } } }),
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
    ];
    lines.extend(calls.iter().cloned());
    let input: String = lines.iter().map(|l| format!("{l}\n")).collect();
    let r = client.run(&["mcp", "--host", host], &input);
    assert_eq!(r.code, 0, "{}", r.err);
    r.out
        .lines()
        .map(|l| serde_json::from_str(l).unwrap_or_else(|_| panic!("not JSON on stdout: {l}")))
        .collect()
}

#[test]
fn the_mcp_server_reads_the_box_as_the_companion() {
    let d = desk(&[]);
    d.write(".config/zed/settings.json", ZED);
    let r = d.client.run(&["agents", "install", "zed", "--yes"], "");
    assert_eq!(r.code, 0, "{}", r.err);
    let before = d.host.calls.lock().unwrap().len();
    let answers = mcp_session(
        &d.client,
        &d.host.url,
        &[
            json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }),
            json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call",
                "params": { "name": "whoami", "arguments": {} } }),
            json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call",
                "params": { "name": "cloud_list_folder", "arguments": { "path": "/" } } }),
        ],
    );
    assert_eq!(answers.len(), 4);
    assert_eq!(answers[0]["result"]["serverInfo"]["name"], "losos");
    assert_eq!(answers[2]["result"]["isError"], false, "{}", answers[2]);
    assert_eq!(answers[2]["result"]["structuredContent"]["name"], "matus");
    assert!(answers[3]["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("Notes"));
    let calls = d.host.calls.lock().unwrap()[before..].to_vec();
    let paths: Vec<&str> = calls.iter().map(|c| c.1.as_str()).collect();
    assert_eq!(paths, ["/api/me", "/api/cloud/files?path=%2F"]);
    assert!(d
        .host
        .login
        .lock()
        .unwrap()
        .shown
        .iter()
        .all(|t| t == "authelia_at_1.sig"));

    // An hour on, the box no longer takes the access token: the server
    // refreshes once, keeps the new tokens and answers all the same.
    d.host.login.lock().unwrap().expire();
    let answers = mcp_session(
        &d.client,
        &d.host.url,
        &[json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": "box_status", "arguments": {} } })],
    );
    assert_eq!(answers[1]["result"]["isError"], false, "{}", answers[1]);
    assert_eq!(d.host.login.lock().unwrap().refreshes, 1);
    let kept = companion::get(d.keychain.as_ref(), &d.host.url)
        .unwrap()
        .unwrap();
    assert_eq!(kept.access_token, "authelia_at_2.sig");
    assert_eq!(kept.refresh_token, "authelia_rt_2.sig");

    // It only reads: a write is never sent, and a route off its list is
    // refused by the box.
    let before = d.host.calls.lock().unwrap().len();
    let r = d.client.run(&["agents", "call", "POST", "/api/apply"], "");
    assert_eq!(r.code, 1);
    assert!(r.err.contains("only reads"), "{}", r.err);
    let r = d
        .client
        .run(&["agents", "call", "GET", "/api/settings"], "");
    assert_eq!(r.code, 1, "{}", r.out);
    assert!(r.err.starts_with("403"), "{}", r.err);
    assert_eq!(d.host.calls.lock().unwrap().len(), before);
}

#[test]
fn two_servers_share_one_sign_in_and_never_show_an_old_refresh_token() {
    let d = desk(&[]);
    d.write(".config/zed/settings.json", ZED);
    let r = d.client.run(&["agents", "install", "zed", "--yes"], "");
    assert_eq!(r.code, 0, "{}", r.err);
    // Two AI tools, each with its own `losos mcp`, one keychain.
    let lock = d.path(".local/share/losos/companion/refresh.lock");
    let keychain: Arc<dyn Keychain + Send + Sync> = d.keychain.clone();
    let link = || {
        companion::Link::new(
            keychain.clone(),
            &d.host.url,
            lock.clone(),
            losos_access::now_secs,
        )
    };
    let (a, b) = (link(), link());
    assert!(a.get("/api/me").is_ok());
    assert!(b.get("/api/me").is_ok());
    d.host.login.lock().unwrap().expire();
    // The first refreshes; the second finds the new tokens in the keychain
    // rather than showing the refresh token the first one used up, which
    // would end the sign-in for both.
    assert!(a.get("/api/me").is_ok());
    assert!(b.get("/api/status").is_ok());
    let login = d.host.login.lock().unwrap();
    assert_eq!(login.refreshes, 1);
    assert_eq!(login.refresh.as_deref(), Some("authelia_rt_2.sig"));
}

#[test]
fn the_mcp_server_starts_with_no_companion_and_says_how_to_make_one() {
    let d = desk(&[]);
    let answers = mcp_session(
        &d.client,
        &d.host.url,
        &[json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": "box_status", "arguments": {} } })],
    );
    assert_eq!(answers.len(), 2);
    let result = &answers[1]["result"];
    assert_eq!(result["isError"], true);
    let text = result["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("losos agents install"), "{text}");
    assert!(d.host.calls.lock().unwrap().is_empty());
}

#[test]
fn a_box_this_computer_never_logged_in_to_is_held_to_the_certificate_the_person_took() {
    let cert = common::Cert::new();
    let host = common::mock_tls(
        false,
        "",
        Box::new(|_, _, _| (200, json!({ "name": "matus" }))),
        &cert,
    );
    let keychain = Arc::new(MemoryKeychain::new());
    let home = tempfile::tempdir().unwrap();
    let mut client = Client::with(keychain.clone());
    let at = |rel: &str| home.path().join(rel).display().to_string();
    client
        .set("HOME", &at(""))
        .set("XDG_CONFIG_HOME", &at(".config"))
        .set("XDG_DATA_HOME", &at(".local/share"))
        .set("PATH", &bin(home.path(), &[]))
        .set("LOSOS_SELF", SELF);
    std::fs::create_dir_all(home.path().join(".config/zed")).unwrap();
    std::fs::write(home.path().join(".config/zed/settings.json"), ZED).unwrap();
    let shown: String = cert
        .pin()
        .to_uppercase()
        .as_bytes()
        .chunks(2)
        .map(|p| std::str::from_utf8(p).unwrap())
        .collect::<Vec<_>>()
        .join(":");

    // No: nothing is signed in, nothing is written.
    let r = client.run(&["agents", "install", "zed", "--host", &host.url], "y\nn\n");
    assert_eq!(r.code, 1, "{}\n{}", r.out, r.err);
    assert!(r.out.contains(&shown), "{}", r.out);
    assert!(r.err.contains("certificate was not taken"), "{}", r.err);
    assert_eq!(host.login.lock().unwrap().polls, 0);
    assert_eq!(
        std::fs::read_to_string(home.path().join(".config/zed/settings.json")).unwrap(),
        ZED
    );

    // Yes: the companion keeps that certificate and is refused any other.
    let r = client.run(&["agents", "install", "zed", "--host", &host.url], "y\ny\n");
    assert_eq!(r.code, 0, "{}\n{}", r.out, r.err);
    let kept = companion::get(keychain.as_ref(), &host.url)
        .unwrap()
        .unwrap();
    assert_eq!(kept.tls_pin.as_deref(), Some(cert.pin().as_str()));
    let lock = home
        .path()
        .join(".local/share/losos/companion/refresh.lock");
    let link = companion::Link::new(keychain.clone(), &host.url, lock, losos_access::now_secs);
    assert!(link.get("/api/me").is_ok());
    let mut other = kept.clone();
    other.tls_pin = Some(common::Cert::new().pin());
    companion::put(keychain.as_ref(), &other).unwrap();
    let lock = home
        .path()
        .join(".local/share/losos/companion/refresh.lock");
    let link = companion::Link::new(keychain.clone(), &host.url, lock, losos_access::now_secs);
    let refused = link.get("/api/me").unwrap_err();
    assert!(
        format!("{refused:?}").contains("different TLS certificate"),
        "{refused:?}"
    );
}
