//! The AI agent tools on this computer, and the LosOS companion in them:
//! `losos agents`, `losos agents install`, `losos agents remove`.
//!
//! Each tool LosOS knows is one row of [`TOOLS`]: what it is called, the
//! programs and folders that give it away, and how the companion goes in.
//! Where the tool has a plugin system of its own the companion is a plugin,
//! made under `~/.local/share/losos/companion/` and handed to the tool's own
//! command, so the tool keeps its own records: Claude Code and GitHub
//! Copilot CLI read one marketplace format, Codex another, Gemini CLI and
//! Qwen Code take an extension folder. Everywhere else it is one entry in
//! the tool's MCP settings, written by [`jsonc`] so the rest of the file
//! stays as the owner left it. Either way it runs `losos mcp` ([`mcp`]).
//!
//! Nothing here asks; the command asks about each tool before it calls
//! [`install`] or [`remove`], and every file a tool's settings live in is
//! copied under `backups/` first.

pub mod companion;
pub mod jsonc;
pub mod mcp;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{json, Value};

use crate::problem::{Problem, Result};
use crate::tr;

/// The one name the companion goes by everywhere: the MCP entry, the
/// plugin, its marketplace and the extension.
pub const NAME: &str = "losos";
const PLUGIN_ID: &str = "losos@losos";

/// A terminal program or a desktop app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Cli,
    App,
}

/// A path under the home folder, or under `$XDG_CONFIG_HOME` (`~/.config`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    Home(&'static str),
    Config(&'static str),
}

/// What an MCP entry looks like in a tool's settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// `{command, args}`, the shape most of them share.
    Plain,
    /// The same with a `type`, as VS Code writes it.
    Typed(&'static str),
    /// opencode's: the command and its arguments in one list.
    Opencode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// A plugin from a marketplace folder in Claude Code's format, added
    /// and installed with `<program> plugin …`. `settings` is where the
    /// program records which plugins are on.
    ClaudeMarketplace { settings: Place },
    /// A plugin from a marketplace folder in Codex's format.
    CodexMarketplace,
    /// An extension folder with its manifest, for `<program> extensions
    /// install`; `home` is the tool's own folder. `trust` is the tool's
    /// list of trusted folders, where the companion's folder goes first
    /// when the tool asks before it installs from a folder it does not trust.
    Extension {
        manifest: &'static str,
        home: Place,
        skip_settings: bool,
        trust: Option<Place>,
    },
    /// One entry in a JSON settings file: the first of `files` that exists,
    /// else the first; the entry goes in the object at `container`.
    Json {
        files: &'static [Place],
        container: &'static [&'static str],
        shape: Shape,
    },
}

/// One tool LosOS can put the companion in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tool {
    /// What `losos agents install <id>` takes.
    pub id: &'static str,
    /// What it is called on screen, as its makers write it.
    pub name: &'static str,
    pub kind: Kind,
    /// Programs on PATH that give it away; a plugin method runs the first.
    pub programs: &'static [&'static str],
    /// Folders that give it away when no program is on PATH.
    pub dirs: &'static [Place],
    pub method: Method,
}

/// Every tool LosOS knows, terminal programs first.
pub const TOOLS: &[Tool] = &[
    Tool {
        id: "claude-code",
        name: "Claude Code",
        kind: Kind::Cli,
        programs: &["claude"],
        dirs: &[Place::Home(".claude")],
        method: Method::ClaudeMarketplace {
            settings: Place::Home(".claude/settings.json"),
        },
    },
    Tool {
        id: "codex",
        name: "Codex",
        kind: Kind::Cli,
        programs: &["codex"],
        dirs: &[Place::Home(".codex")],
        method: Method::CodexMarketplace,
    },
    Tool {
        id: "gemini",
        name: "Gemini CLI",
        kind: Kind::Cli,
        programs: &["gemini"],
        dirs: &[Place::Home(".gemini")],
        method: Method::Extension {
            manifest: "gemini-extension.json",
            home: Place::Home(".gemini"),
            skip_settings: true,
            trust: Some(Place::Home(".gemini/trustedFolders.json")),
        },
    },
    Tool {
        id: "qwen",
        name: "Qwen Code",
        kind: Kind::Cli,
        programs: &["qwen"],
        dirs: &[Place::Home(".qwen")],
        method: Method::Extension {
            manifest: "qwen-extension.json",
            home: Place::Home(".qwen"),
            skip_settings: false,
            trust: None,
        },
    },
    Tool {
        id: "copilot",
        name: "GitHub Copilot CLI",
        kind: Kind::Cli,
        programs: &["copilot"],
        dirs: &[Place::Home(".copilot")],
        method: Method::ClaudeMarketplace {
            settings: Place::Home(".copilot/settings.json"),
        },
    },
    Tool {
        id: "opencode",
        name: "opencode",
        kind: Kind::Cli,
        programs: &["opencode"],
        dirs: &[Place::Config("opencode")],
        method: Method::Json {
            files: &[
                Place::Config("opencode/opencode.json"),
                Place::Config("opencode/opencode.jsonc"),
            ],
            container: &["mcp"],
            shape: Shape::Opencode,
        },
    },
    Tool {
        id: "claude-desktop",
        name: "Claude Desktop",
        kind: Kind::App,
        programs: &["claude-desktop"],
        dirs: &[Place::Config("Claude")],
        method: Method::Json {
            files: &[Place::Config("Claude/claude_desktop_config.json")],
            container: &["mcpServers"],
            shape: Shape::Plain,
        },
    },
    Tool {
        id: "cursor",
        name: "Cursor",
        kind: Kind::App,
        programs: &["cursor"],
        dirs: &[Place::Home(".cursor")],
        method: Method::Json {
            files: &[Place::Home(".cursor/mcp.json")],
            container: &["mcpServers"],
            shape: Shape::Plain,
        },
    },
    Tool {
        id: "vscode",
        name: "VS Code",
        kind: Kind::App,
        programs: &["code"],
        dirs: &[Place::Config("Code")],
        method: Method::Json {
            files: &[Place::Config("Code/User/mcp.json")],
            container: &["servers"],
            shape: Shape::Typed("stdio"),
        },
    },
    Tool {
        id: "vscodium",
        name: "VSCodium",
        kind: Kind::App,
        programs: &["codium"],
        dirs: &[Place::Config("VSCodium")],
        method: Method::Json {
            files: &[Place::Config("VSCodium/User/mcp.json")],
            container: &["servers"],
            shape: Shape::Typed("stdio"),
        },
    },
    Tool {
        id: "cline",
        name: "Cline",
        kind: Kind::App,
        programs: &[],
        dirs: &[Place::Config(
            "Code/User/globalStorage/saoudrizwan.claude-dev",
        )],
        method: Method::Json {
            files: &[Place::Config(
                "Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json",
            )],
            container: &["mcpServers"],
            shape: Shape::Plain,
        },
    },
    Tool {
        id: "windsurf",
        name: "Windsurf",
        kind: Kind::App,
        programs: &["windsurf"],
        dirs: &[Place::Home(".codeium/windsurf")],
        method: Method::Json {
            files: &[Place::Home(".codeium/windsurf/mcp_config.json")],
            container: &["mcpServers"],
            shape: Shape::Plain,
        },
    },
    Tool {
        id: "zed",
        name: "Zed",
        kind: Kind::App,
        programs: &["zed", "zeditor"],
        dirs: &[Place::Config("zed")],
        method: Method::Json {
            files: &[Place::Config("zed/settings.json")],
            container: &["context_servers"],
            shape: Shape::Plain,
        },
    },
    Tool {
        id: "lmstudio",
        name: "LM Studio",
        kind: Kind::App,
        programs: &["lms"],
        dirs: &[Place::Home(".lmstudio")],
        method: Method::Json {
            files: &[Place::Home(".lmstudio/mcp.json")],
            container: &["mcpServers"],
            shape: Shape::Plain,
        },
    },
];

/// The tool with this id.
pub fn tool(id: &str) -> Option<&'static Tool> {
    TOOLS.iter().find(|t| t.id.eq_ignore_ascii_case(id))
}

/// How a tool starts the companion: this program, with these arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Launch {
    pub command: String,
    pub args: Vec<String>,
}

impl Launch {
    /// `losos mcp --host <url>`, by the program's full path: a desktop app
    /// is started with whatever PATH the desktop has, which is often not
    /// the shell's.
    pub fn for_host(program: &str, url: &str) -> Launch {
        Launch {
            command: program.to_string(),
            args: vec!["mcp".into(), "--host".into(), url.into()],
        }
    }

    /// The entry in a tool's settings.
    fn entry(&self, shape: Shape) -> Value {
        match shape {
            Shape::Plain => json!({ "command": self.command, "args": self.args }),
            Shape::Typed(kind) => {
                json!({ "type": kind, "command": self.command, "args": self.args })
            }
            Shape::Opencode => {
                let mut command = vec![self.command.clone()];
                command.extend(self.args.iter().cloned());
                json!({ "type": "local", "command": command, "enabled": true })
            }
        }
    }

    /// The `mcpServers` a plugin or extension carries.
    fn servers(&self) -> Value {
        json!({ NAME: { "command": self.command, "args": self.args } })
    }
}

/// Is this settings entry one the companion wrote? Its program is a
/// `losos` and its first argument `mcp`, in either shape.
fn is_ours(entry: &Value) -> bool {
    let words: Vec<&str> = match (&entry["command"], &entry["args"]) {
        (Value::String(c), Value::Array(a)) => std::iter::once(c.as_str())
            .chain(a.iter().filter_map(Value::as_str))
            .collect(),
        (Value::Array(c), _) => c.iter().filter_map(Value::as_str).collect(),
        _ => return false,
    };
    let named_losos = words.first().is_some_and(|c| {
        Path::new(c)
            .file_name()
            .is_some_and(|n| n.to_string_lossy() == NAME)
    });
    named_losos && words.get(1) == Some(&"mcp")
}

/// The computer as far as the AI tools go: its environment, its home
/// folder and the programs on its PATH.
#[derive(Debug, Clone)]
pub struct Desk {
    pub env: HashMap<String, String>,
}

impl Desk {
    fn var(&self, name: &str) -> Option<String> {
        self.env.get(name).cloned().filter(|v| !v.is_empty())
    }

    fn home(&self) -> Result<PathBuf> {
        self.var("HOME")
            .map(PathBuf::from)
            .ok_or_else(|| Problem::new("losos::home", tr!("tui.agents.noHome")))
    }

    fn config_home(&self) -> Result<PathBuf> {
        match self.var("XDG_CONFIG_HOME") {
            Some(dir) => Ok(PathBuf::from(dir)),
            None => Ok(self.home()?.join(".config")),
        }
    }

    pub fn path(&self, place: Place) -> Result<PathBuf> {
        Ok(match place {
            Place::Home(rel) => self.home()?.join(rel),
            Place::Config(rel) => self.config_home()?.join(rel),
        })
    }

    /// `~/.local/share/losos/companion`: the plugins, the extensions and
    /// the backups.
    pub fn companion_dir(&self) -> Result<PathBuf> {
        let data = match self.var("XDG_DATA_HOME") {
            Some(dir) => PathBuf::from(dir),
            None => self.home()?.join(".local/share"),
        };
        Ok(data.join("losos/companion"))
    }

    /// The first `name` on PATH that is a file.
    pub fn which(&self, name: &str) -> Option<PathBuf> {
        let path = self.var("PATH")?;
        std::env::split_paths(&path)
            .map(|dir| dir.join(name))
            .find(|p| p.is_file())
    }

    /// Run `program` with `args` in `cwd`; the exit code and what it said.
    /// A tool that has not finished in [`TOOL_TIMEOUT`] is stopped, so a
    /// tool waiting on a network it cannot reach never holds the command.
    fn run(&self, program: &Path, args: &[&str], cwd: &Path) -> Result<(i32, String)> {
        tracing::debug!(program = %program.display(), ?args, "agent tool");
        let could_not = |e: std::io::Error| {
            Problem::new(
                "losos::agents",
                tr!(
                    "tui.agents.couldNotRun",
                    program = program.display().to_string(),
                    detail = e.to_string()
                ),
            )
        };
        let mut child = Command::new(program)
            .args(args)
            .current_dir(cwd)
            .env_clear()
            .envs(&self.env)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(could_not)?;
        let drain = |pipe: Option<Box<dyn std::io::Read + Send>>| {
            std::thread::spawn(move || {
                let mut text = Vec::new();
                if let Some(mut pipe) = pipe {
                    let _ = pipe.read_to_end(&mut text);
                }
                String::from_utf8_lossy(&text).into_owned()
            })
        };
        let out = drain(child.stdout.take().map(|p| Box::new(p) as _));
        let err = drain(child.stderr.take().map(|p| Box::new(p) as _));
        let deadline = std::time::Instant::now() + TOOL_TIMEOUT;
        let status = loop {
            if let Some(status) = child.try_wait().map_err(could_not)? {
                break Some(status);
            }
            if std::time::Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        };
        let said = format!(
            "{}{}",
            out.join().unwrap_or_default(),
            err.join().unwrap_or_default()
        );
        match status {
            Some(status) => Ok((status.code().unwrap_or(1), said)),
            None => Err(Problem::new(
                "losos::agents",
                tr!(
                    "tui.agents.toolTimedOut",
                    command = format!("{} {}", file_name(program), args.join(" ")),
                    seconds = TOOL_TIMEOUT.as_secs()
                ),
            )),
        }
    }

    /// Run it and fail with what it said when it fails.
    fn must(&self, program: &Path, args: &[&str], cwd: &Path) -> Result<()> {
        let (code, said) = self.run(program, args, cwd)?;
        if code == 0 {
            return Ok(());
        }
        let name = file_name(program);
        let said: String = said
            .trim()
            .lines()
            .last()
            .unwrap_or("")
            .chars()
            .take(300)
            .collect();
        Err(Problem::new(
            "losos::agents",
            tr!(
                "tui.agents.toolFailed",
                command = format!("{name} {}", args.join(" ")),
                code = code,
                detail = said
            ),
        ))
    }
}

/// How long a tool's own command may take.
const TOOL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

fn file_name(program: &Path) -> String {
    program
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
}

/// A tool found on this computer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub tool: &'static Tool,
    /// The program a plugin method runs, when one is on PATH.
    pub program: Option<PathBuf>,
    /// The companion is in it now.
    pub installed: bool,
}

impl Found {
    /// A plugin method needs the tool's own program to put a plugin in.
    pub fn can_install(&self) -> bool {
        self.program.is_some() || matches!(self.tool.method, Method::Json { .. })
    }
}

/// Every tool in [`TOOLS`] this computer has.
pub fn detect(desk: &Desk) -> Result<Vec<Found>> {
    let mut found = Vec::new();
    for tool in TOOLS {
        let program = tool.programs.iter().find_map(|p| desk.which(p));
        let mut seen = program.is_some();
        for dir in tool.dirs {
            seen = seen || desk.path(*dir)?.is_dir();
        }
        if !seen {
            continue;
        }
        let installed = installed(desk, tool)?;
        found.push(Found {
            tool,
            program,
            installed,
        });
    }
    Ok(found)
}

/// Is the companion in `tool` now?
fn installed(desk: &Desk, tool: &Tool) -> Result<bool> {
    Ok(match tool.method {
        Method::ClaudeMarketplace { settings } => {
            let text = std::fs::read_to_string(desk.path(settings)?).unwrap_or_default();
            jsonc::get(&text, &["enabledPlugins"], PLUGIN_ID)
                .ok()
                .flatten()
                == Some(Value::Bool(true))
        }
        Method::CodexMarketplace => {
            let text = std::fs::read_to_string(desk.path(Place::Home(".codex/config.toml"))?)
                .unwrap_or_default();
            codex_enabled(&text)
        }
        Method::Extension { manifest, home, .. } => desk
            .path(home)?
            .join("extensions")
            .join(NAME)
            .join(manifest)
            .is_file(),
        Method::Json {
            files, container, ..
        } => {
            let text = std::fs::read_to_string(settings_file(desk, files)?).unwrap_or_default();
            jsonc::get(&text, container, NAME)
                .ok()
                .flatten()
                .is_some_and(|e| is_ours(&e))
        }
    })
}

/// Does Codex's `config.toml` have the plugin on? Codex writes
/// `[plugins."losos@losos"]` and `enabled = true` under it.
fn codex_enabled(text: &str) -> bool {
    let mut inside = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            inside = line == format!("[plugins.\"{PLUGIN_ID}\"]");
        } else if inside {
            if let Some((k, v)) = line.split_once('=') {
                if k.trim() == "enabled" {
                    return v.trim().starts_with("true");
                }
            }
        }
    }
    false
}

/// The settings file of a JSON method: the first that exists, else the
/// first of the list.
fn settings_file(desk: &Desk, files: &[Place]) -> Result<PathBuf> {
    for place in files {
        let path = desk.path(*place)?;
        if path.is_file() {
            return Ok(path);
        }
    }
    desk.path(files[0])
}

/// What [`install`] or [`remove`] did.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Done {
    /// The settings files it changed, or the tool's own command changed.
    pub changed: Vec<PathBuf>,
    /// Where the copies of them went.
    pub backups: Vec<PathBuf>,
}

/// Copy `file` under `backups/<tool>/` before anything changes it.
fn back_up(desk: &Desk, tool: &Tool, file: &Path, done: &mut Done) -> Result<()> {
    if !file.is_file() {
        return Ok(());
    }
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let dir = desk.companion_dir()?.join("backups").join(tool.id);
    let name = file
        .file_name()
        .map_or_else(|| "settings".into(), |n| n.to_string_lossy().into_owned());
    let mut copy = dir.join(format!("{stamp}-{name}"));
    let mut n = 1;
    while copy.exists() {
        n += 1;
        copy = dir.join(format!("{stamp}-{n}-{name}"));
    }
    std::fs::create_dir_all(&dir)
        .and_then(|()| std::fs::copy(file, &copy).map(|_| ()))
        .map_err(|e| write_problem(&copy, &e))?;
    done.backups.push(copy);
    Ok(())
}

fn write_problem(path: &Path, e: &std::io::Error) -> Problem {
    Problem::new(
        "losos::agents",
        tr!(
            "tui.agents.couldNotWrite",
            path = path.display().to_string(),
            detail = e.to_string()
        ),
    )
}

fn write(path: &Path, text: &str) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| write_problem(dir, &e))?;
    }
    // A temporary file and a rename, so a tool never reads half a file.
    let tmp = path.with_extension(format!("losos-tmp.{}", std::process::id()));
    std::fs::write(&tmp, text)
        .and_then(|()| {
            if let Ok(meta) = std::fs::metadata(path) {
                std::fs::set_permissions(&tmp, meta.permissions())?;
            }
            std::fs::rename(&tmp, path)
        })
        .map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            write_problem(path, &e)
        })
}

fn write_json(path: &Path, value: &Value) -> Result<()> {
    let text = serde_json::to_string_pretty(value).expect("a Value always serialises");
    write(path, &format!("{text}\n"))
}

/// The plugin's version, strict semver as Codex wants it.
fn version() -> String {
    let v = crate::tui::panes::VERSION.trim_start_matches('v');
    let core = v.split(['-', '+']).next().unwrap_or("");
    if core.split('.').count() == 3 && core.split('.').all(|p| p.parse::<u64>().is_ok()) {
        core.to_string()
    } else {
        "0.1.0".to_string()
    }
}

const DESCRIPTION: &str =
    "Files in LosOS Cloud and repositories in LosOS Git, read from your LosOS box.";

/// What the agent is told about the tools, as a skill or a context file.
pub fn guide() -> String {
    format!(
        "# LosOS\n\n\
         The `{NAME}` MCP server reads the owner's LosOS box. It can look but \
         not change anything.\n\n\
         - `cloud_list_folder`, `cloud_find_files` and `cloud_search_text` \
         look through the files in LosOS Cloud.\n\
         - `git_list_repositories` and `git_search_code` look through the \
         repositories in LosOS Git.\n\
         - `box_status` says how the box is doing, and `whoami` which person \
         the tools act as.\n\n\
         When the owner asks about their own files, notes or code and does not \
         say where they are, look on the box with these tools first. Say which \
         file or repository an answer came from.\n"
    )
}

fn skill() -> String {
    format!(
        "---\nname: {NAME}\ndescription: Use when the owner asks about their own files, notes or repositories, which live on their LosOS box. Reads LosOS Cloud and LosOS Git through the {NAME} MCP tools.\n---\n\n{}",
        guide()
    )
}

/// Write the Claude Code marketplace: `.claude-plugin/marketplace.json` and
/// the one plugin under `plugins/losos`.
fn claude_marketplace(root: &Path, launch: &Launch) -> Result<()> {
    write_json(
        &root.join(".claude-plugin/marketplace.json"),
        &json!({
            "name": NAME,
            "owner": { "name": "LosOS" },
            "metadata": { "description": "The LosOS companion", "version": version() },
            "plugins": [{
                "name": NAME,
                "source": format!("./plugins/{NAME}"),
                "description": DESCRIPTION,
                "version": version(),
            }],
        }),
    )?;
    let plugin = root.join("plugins").join(NAME);
    write_json(
        &plugin.join(".claude-plugin/plugin.json"),
        &json!({
            "name": NAME,
            "version": version(),
            "description": DESCRIPTION,
            "author": { "name": "LosOS" },
            "homepage": "https://losos.dasmat.us/",
        }),
    )?;
    write_json(
        &plugin.join(".mcp.json"),
        &json!({ "mcpServers": launch.servers() }),
    )?;
    write(&plugin.join("skills").join(NAME).join("SKILL.md"), &skill())
}

/// Write the Codex marketplace: `.agents/plugins/marketplace.json` and the
/// plugin, with the fields Codex's own validator asks for.
fn codex_marketplace(root: &Path, launch: &Launch) -> Result<()> {
    write_json(
        &root.join(".agents/plugins/marketplace.json"),
        &json!({
            "name": NAME,
            "interface": { "displayName": "LosOS" },
            "plugins": [{
                "name": NAME,
                "source": { "source": "local", "path": format!("./plugins/{NAME}") },
                "policy": { "installation": "AVAILABLE", "authentication": "ON_INSTALL" },
                "category": "Productivity",
            }],
        }),
    )?;
    let plugin = root.join("plugins").join(NAME);
    write_json(
        &plugin.join(".codex-plugin/plugin.json"),
        &json!({
            "name": NAME,
            "version": version(),
            "description": DESCRIPTION,
            "author": { "name": "LosOS", "url": "https://losos.dasmat.us/" },
            "homepage": "https://losos.dasmat.us/",
            "skills": "./skills/",
            "mcpServers": "./.mcp.json",
            "interface": {
                "displayName": "LosOS",
                "shortDescription": "Your files and repositories on your LosOS box.",
                "longDescription": DESCRIPTION,
                "developerName": "LosOS",
                "category": "Productivity",
                "capabilities": [],
                "defaultPrompt": "What is in my LosOS Cloud?",
            },
        }),
    )?;
    write_json(
        &plugin.join(".mcp.json"),
        &json!({ "mcpServers": launch.servers() }),
    )?;
    write(&plugin.join("skills").join(NAME).join("SKILL.md"), &skill())
}

/// Write an extension folder: the manifest and its context file.
fn extension(root: &Path, manifest: &str, launch: &Launch) -> Result<()> {
    write_json(
        &root.join(manifest),
        &json!({
            "name": NAME,
            "version": version(),
            "description": DESCRIPTION,
            "contextFileName": "LOSOS.md",
            "mcpServers": launch.servers(),
        }),
    )?;
    write(&root.join("LOSOS.md"), &guide())
}

/// Put the companion in `found`. Already there, it is put in again, so it
/// runs the program and box it is given now.
pub fn install(desk: &Desk, found: &Found, launch: &Launch) -> Result<Done> {
    let tool = found.tool;
    let mut done = Done::default();
    let need_program = || {
        found.program.clone().ok_or_else(|| {
            Problem::new(
                "losos::agents",
                tr!(
                    "tui.agents.noProgram",
                    tool = tool.name,
                    program = tool.programs.first().copied().unwrap_or("")
                ),
            )
        })
    };
    match tool.method {
        Method::ClaudeMarketplace { settings } => {
            let program = need_program()?;
            let root = desk.companion_dir()?.join(tool.id);
            claude_marketplace(&root, launch)?;
            let settings = desk.path(settings)?;
            back_up(desk, tool, &settings, &mut done)?;
            // Gone first, so a second install takes the new files.
            let _ = desk.run(&program, &["plugin", "uninstall", PLUGIN_ID], &root);
            let _ = desk.run(&program, &["plugin", "marketplace", "remove", NAME], &root);
            let dir = root.display().to_string();
            desk.must(&program, &["plugin", "marketplace", "add", &dir], &root)?;
            desk.must(&program, &["plugin", "install", PLUGIN_ID], &root)?;
            done.changed.push(settings);
        }
        Method::CodexMarketplace => {
            let program = need_program()?;
            let root = desk.companion_dir()?.join(tool.id);
            codex_marketplace(&root, launch)?;
            let settings = desk.path(Place::Home(".codex/config.toml"))?;
            back_up(desk, tool, &settings, &mut done)?;
            let _ = desk.run(&program, &["plugin", "remove", PLUGIN_ID], &root);
            let _ = desk.run(&program, &["plugin", "marketplace", "remove", NAME], &root);
            let dir = root.display().to_string();
            desk.must(&program, &["plugin", "marketplace", "add", &dir], &root)?;
            desk.must(&program, &["plugin", "add", PLUGIN_ID], &root)?;
            done.changed.push(settings);
        }
        Method::Extension {
            manifest,
            home,
            skip_settings,
            trust,
        } => {
            let program = need_program()?;
            let root = desk.companion_dir()?.join(tool.id);
            extension(&root, manifest, launch)?;
            let installed = desk.path(home)?.join("extensions").join(NAME);
            let _ = desk.run(&program, &["extensions", "uninstall", NAME], &root);
            let dir = root.display().to_string();
            // The yes to this tool is the consent the tool asks for, and the
            // trust it asks for in the folder it installs from: the
            // companion's own folder, and nothing else.
            if let Some(trust) = trust {
                let file = desk.path(trust)?;
                let text = read_or_empty(&file)?;
                let new = jsonc::upsert(&text, &[], &trusted(&root), &Value::from(TRUST_FOLDER))
                    .map_err(|e| read_problem(&file, &e.0))?;
                if new != text {
                    back_up(desk, tool, &file, &mut done)?;
                    write(&file, &new)?;
                    done.changed.push(file);
                }
            }
            let mut args = vec!["extensions", "install", "--consent"];
            if skip_settings {
                args.push("--skip-settings");
            }
            args.push(&dir);
            desk.must(&program, &args, &root)?;
            done.changed.push(installed);
        }
        Method::Json {
            files,
            container,
            shape,
        } => {
            let file = settings_file(desk, files)?;
            let text = match std::fs::read_to_string(&file) {
                Ok(text) => text,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
                Err(e) => return Err(read_problem(&file, &e.to_string())),
            };
            if let Some(entry) =
                jsonc::get(&text, container, NAME).map_err(|e| read_problem(&file, &e.0))?
            {
                if !is_ours(&entry) {
                    return Err(Problem::new(
                        "losos::agents",
                        tr!(
                            "tui.agents.notOurs",
                            tool = tool.name,
                            path = file.display().to_string()
                        ),
                    ));
                }
            }
            let new = jsonc::upsert(&text, container, NAME, &launch.entry(shape))
                .map_err(|e| read_problem(&file, &e.0))?;
            back_up(desk, tool, &file, &mut done)?;
            write(&file, &new)?;
            done.changed.push(file);
        }
    }
    Ok(done)
}

/// What a trusted-folders list says of a folder the tool may load.
const TRUST_FOLDER: &str = "TRUST_FOLDER";

/// The folder as the tool writes it in its list: with links resolved.
fn trusted(root: &Path) -> String {
    std::fs::canonicalize(root)
        .unwrap_or_else(|_| root.to_path_buf())
        .display()
        .to_string()
}

fn read_or_empty(file: &Path) -> Result<String> {
    match std::fs::read_to_string(file) {
        Ok(text) => Ok(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(read_problem(file, &e.to_string())),
    }
}

fn read_problem(path: &Path, detail: &str) -> Problem {
    Problem::new(
        "losos::agents",
        tr!(
            "tui.agents.couldNotRead",
            path = path.display().to_string(),
            detail = detail
        ),
    )
}

/// Take the companion out of `found`, and leave everything else.
pub fn remove(desk: &Desk, found: &Found) -> Result<Done> {
    let tool = found.tool;
    let mut done = Done::default();
    let root = desk.companion_dir()?.join(tool.id);
    let cwd = if root.is_dir() {
        root.clone()
    } else {
        desk.home()?
    };
    match tool.method {
        Method::ClaudeMarketplace { settings } => {
            if let Some(program) = &found.program {
                let settings = desk.path(settings)?;
                back_up(desk, tool, &settings, &mut done)?;
                desk.must(program, &["plugin", "uninstall", PLUGIN_ID], &cwd)?;
                desk.must(program, &["plugin", "marketplace", "remove", NAME], &cwd)?;
                done.changed.push(settings);
            }
        }
        Method::CodexMarketplace => {
            if let Some(program) = &found.program {
                let settings = desk.path(Place::Home(".codex/config.toml"))?;
                back_up(desk, tool, &settings, &mut done)?;
                desk.must(program, &["plugin", "remove", PLUGIN_ID], &cwd)?;
                desk.must(program, &["plugin", "marketplace", "remove", NAME], &cwd)?;
                done.changed.push(settings);
            }
        }
        Method::Extension { home, trust, .. } => {
            if let Some(program) = &found.program {
                desk.must(program, &["extensions", "uninstall", NAME], &cwd)?;
                done.changed
                    .push(desk.path(home)?.join("extensions").join(NAME));
            }
            if let Some(trust) = trust {
                let file = desk.path(trust)?;
                let text = read_or_empty(&file)?;
                if let Some(new) = jsonc::remove(&text, &[], &trusted(&root))
                    .map_err(|e| read_problem(&file, &e.0))?
                {
                    back_up(desk, tool, &file, &mut done)?;
                    // A list the companion started, and left empty, goes.
                    if jsonc::read(&new).is_ok_and(|v| v == json!({})) {
                        std::fs::remove_file(&file).map_err(|e| write_problem(&file, &e))?;
                    } else {
                        write(&file, &new)?;
                    }
                    done.changed.push(file);
                }
            }
        }
        Method::Json {
            files, container, ..
        } => {
            let file = settings_file(desk, files)?;
            let Ok(text) = std::fs::read_to_string(&file) else {
                return Ok(done);
            };
            match jsonc::get(&text, container, NAME).map_err(|e| read_problem(&file, &e.0))? {
                Some(entry) if is_ours(&entry) => {}
                _ => return Ok(done),
            }
            if let Some(new) =
                jsonc::remove(&text, container, NAME).map_err(|e| read_problem(&file, &e.0))?
            {
                back_up(desk, tool, &file, &mut done)?;
                write(&file, &new)?;
                done.changed.push(file);
            }
        }
    }
    if root.is_dir() {
        let _ = std::fs::remove_dir_all(&root);
    }
    Ok(done)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_has_one_id_and_a_way_to_be_found() {
        for (i, t) in TOOLS.iter().enumerate() {
            assert!(!t.programs.is_empty() || !t.dirs.is_empty(), "{}", t.id);
            assert!(TOOLS[..i].iter().all(|o| o.id != t.id), "{}", t.id);
            assert!(
                t.id.bytes().all(|b| b.is_ascii_lowercase() || b == b'-'),
                "{}",
                t.id
            );
            if !matches!(t.method, Method::Json { .. }) {
                assert!(!t.programs.is_empty(), "{} needs its program", t.id);
            }
        }
    }

    #[test]
    fn only_a_losos_mcp_entry_is_ours() {
        let launch = Launch::for_host("/home/me/.local/bin/losos", "https://box.local");
        for shape in [Shape::Plain, Shape::Typed("stdio"), Shape::Opencode] {
            assert!(is_ours(&launch.entry(shape)), "{shape:?}");
        }
        assert!(!is_ours(
            &json!({ "command": "npx", "args": ["losos", "mcp"] })
        ));
        assert!(!is_ours(
            &json!({ "command": "/usr/bin/losos", "args": ["open"] })
        ));
        assert!(!is_ours(&json!({ "url": "https://example.org/mcp" })));
    }

    #[test]
    fn codex_records_the_plugin_under_its_own_header() {
        let on =
            "[marketplaces.losos]\nsource = \"/x\"\n\n[plugins.\"losos@losos\"]\nenabled = true\n";
        assert!(codex_enabled(on));
        assert!(!codex_enabled(&on.replace("true", "false")));
        assert!(!codex_enabled("[plugins.\"other@x\"]\nenabled = true\n"));
        assert!(!codex_enabled(""));
    }

    #[test]
    fn the_plugin_version_is_strict_semver() {
        let v = version();
        assert_eq!(v.split('.').count(), 3);
        assert!(v.split('.').all(|p| p.parse::<u64>().is_ok()));
    }
}
