//! The companion in the real AI tools, when they are installed: each one
//! takes it, starts `losos mcp` and sees its tools, and gives it back.
//! Skipped unless `LOSOS_AGENT_TOOLS` is a folder holding the tools'
//! programs (nix/tests/agents.nix does the same on a booted laptop):
//!
//!   LOSOS_AGENT_TOOLS=/path/to/bin cargo test --test real_tools -- --nocapture

mod common;

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use losos::agents::{self, Desk, Launch};

/// Each tool's own way of saying which MCP servers it runs, and what it
/// prints for ours once the server answered.
const LISTS: &[(&str, &[&str], &str)] = &[
    ("claude-code", &["mcp", "list"], "Connected"),
    ("gemini", &["mcp", "list"], "Connected"),
    ("qwen", &["mcp", "list"], "Connected"),
    ("opencode", &["mcp", "list"], "connected"),
];

fn run(desk: &Desk, program: &Path, args: &[&str], cwd: &Path) -> String {
    let out = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .env_clear()
        .envs(&desk.env)
        .output()
        .unwrap();
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

#[test]
fn the_real_tools_take_the_companion_start_it_and_give_it_back() {
    let Ok(tools) = std::env::var("LOSOS_AGENT_TOOLS") else {
        eprintln!("LOSOS_AGENT_TOOLS is not set; skipped");
        return;
    };
    let home = tempfile::tempdir().unwrap();
    let mut env: HashMap<String, String> = HashMap::new();
    env.insert("HOME".into(), home.path().display().to_string());
    env.insert(
        "PATH".into(),
        format!("{tools}:{}", std::env::var("PATH").unwrap_or_default()),
    );
    for keep in [
        "SSL_CERT_FILE",
        "NODE_EXTRA_CA_CERTS",
        "LANG",
        "HTTPS_PROXY",
        "HTTP_PROXY",
        "NO_PROXY",
        "https_proxy",
        "http_proxy",
        "no_proxy",
    ] {
        if let Ok(v) = std::env::var(keep) {
            env.insert(keep.into(), v);
        }
    }
    // Nothing reaches a box here: listing tools needs none.
    let desk = Desk { env };
    for dir in [
        ".claude",
        ".codex",
        ".gemini",
        ".qwen",
        ".copilot",
        ".config/opencode",
        ".config/zed",
    ] {
        std::fs::create_dir_all(home.path().join(dir)).unwrap();
    }
    std::fs::write(
        home.path().join(".config/zed/settings.json"),
        "// mine\n{\n  \"theme\": \"One Dark\"\n}\n",
    )
    .unwrap();
    // Gemini CLI starts no MCP server in a folder the owner has not
    // trusted, so the tools are asked from one the owner has.
    let work = home.path().join("work");
    std::fs::create_dir_all(&work).unwrap();
    let trust = format!(
        "{{\n  {:?}: \"TRUST_FOLDER\"\n}}",
        work.canonicalize().unwrap()
    );
    std::fs::write(home.path().join(".gemini/trustedFolders.json"), &trust).unwrap();
    let launch = Launch::for_host(env!("CARGO_BIN_EXE_losos"), "https://box.invalid");

    let found = agents::detect(&desk).unwrap();
    let mut tried = 0;
    for f in found.iter().filter(|f| f.can_install()) {
        let before =
            std::fs::read_to_string(home.path().join(".config/zed/settings.json")).unwrap();
        let done = agents::install(&desk, f, &launch)
            .unwrap_or_else(|p| panic!("{}: {p} {:?}", f.tool.name, p.help));
        eprintln!("{}: in, {:?}", f.tool.name, done.changed);
        let again = agents::detect(&desk).unwrap();
        let now = again.iter().find(|g| g.tool.id == f.tool.id).unwrap();
        assert!(now.installed, "{} does not read as installed", f.tool.name);
        if let Some((_, args, says)) = LISTS.iter().find(|(id, _, _)| *id == f.tool.id) {
            let program = f
                .program
                .clone()
                .unwrap_or_else(|| desk.which(f.tool.programs[0]).expect("its program"));
            let out = run(&desk, &program, args, &work);
            eprintln!("{} {}:\n{out}", f.tool.name, args.join(" "));
            let line = out
                .lines()
                .find(|l| l.contains("losos"))
                .unwrap_or_else(|| panic!("{} lists no losos:\n{out}", f.tool.name));
            assert!(line.contains(says), "{}: {line}", f.tool.name);
        }
        agents::remove(&desk, now).unwrap();
        let after = agents::detect(&desk).unwrap();
        assert!(
            !after
                .iter()
                .find(|g| g.tool.id == f.tool.id)
                .unwrap()
                .installed,
            "{} still reads as installed",
            f.tool.name
        );
        if f.tool.id == "gemini" {
            let back = std::fs::read_to_string(home.path().join(".gemini/trustedFolders.json"));
            assert_eq!(back.unwrap(), trust);
        }
        if f.tool.id == "zed" {
            let back =
                std::fs::read_to_string(home.path().join(".config/zed/settings.json")).unwrap();
            assert_eq!(back, before);
        }
        tried += 1;
    }
    eprintln!("{tried} tools tried");
    assert!(tried > 0);
}
