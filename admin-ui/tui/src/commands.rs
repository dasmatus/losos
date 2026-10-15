//! The commands. `losos --help` lists them; docs/terminal-access.md and
//! docs/edge-terminal.md are the contract behind each one.

use std::collections::HashMap;
use std::io::{self, IsTerminal, Read, Write};
use std::process::{Command as Process, Stdio};
use std::sync::Arc;

use losos_access::{HostKey, SeededKey, Unlocked, UserKey};
use miette::{GraphicalReportHandler, GraphicalTheme};
use serde_json::{json, Value};

use crate::cli::{
    self, CiCommand, Cli, CloudCommand, Command, EdgeCommand, EscrowCommand, GitCommand,
    KeyCommand, SecretCommand,
};
use crate::computer::{Computer, Service};
use crate::envelope::{host_key_url, unreachable, Session, Transport};
use crate::http::{Http, Trust};
use crate::i18n::{self, Locale};
use crate::keychain::{Keychain, SecretTool};
use crate::problem::{Problem, Result};
use crate::prompt::{read_hidden, read_piped};
use crate::theme::{detect_mode, detect_truecolor, Theme};
use crate::tr;
use crate::tui::app::App;
use crate::tui::login::{KeyChoice, Login, Picker};
use crate::tui::term;
use crate::vault::{
    group_fingerprint, normalize_url, same_fingerprint, HostKind, HostRecord, Vault,
};

/// What a command reads and writes, so the suite can run every command
/// without a terminal, a Secret Service or a network of its own.
pub struct Io {
    pub out: Box<dyn Write>,
    pub err: Box<dyn Write>,
    pub stdin: Box<dyn Read>,
    pub env: HashMap<String, String>,
    pub keychain: Arc<dyn Keychain + Send + Sync>,
    pub stdin_tty: bool,
    pub stdout_tty: bool,
    pub stderr_tty: bool,
}

impl Io {
    /// The process's own terminal, environment and Secret Service.
    pub fn real() -> Io {
        let env: HashMap<String, String> = std::env::vars().collect();
        let tool = env
            .get("LOSOS_SECRET_TOOL")
            .cloned()
            .unwrap_or_else(|| "secret-tool".into());
        Io {
            out: Box::new(io::stdout()),
            err: Box::new(io::stderr()),
            stdin: Box::new(io::stdin()),
            env,
            keychain: Arc::new(SecretTool::new(tool)),
            stdin_tty: io::stdin().is_terminal(),
            stdout_tty: io::stdout().is_terminal(),
            stderr_tty: io::stderr().is_terminal(),
        }
    }

    fn var(&self, name: &str) -> Option<String> {
        self.env.get(name).cloned()
    }

    fn line(&mut self, text: impl AsRef<str>) {
        let _ = writeln!(self.out, "{}", text.as_ref());
    }

    fn vault(&self) -> Vault {
        Vault::new(self.keychain.clone())
    }

    /// A value typed without echo, or piped in by a script.
    fn secret(&mut self, prompt: &str) -> Result<String> {
        if self.stdin_tty {
            read_hidden(prompt, &mut self.err)
        } else {
            read_piped(&mut self.stdin)
        }
    }

    /// The full-screen view needs a terminal on both ends.
    fn terminal(&self) -> bool {
        self.stdin_tty && self.stdout_tty
    }

    /// This computer, for the commands that set it up.
    fn computer(&self) -> Computer {
        Computer {
            keychain: self.keychain.clone(),
            env: self.env.clone(),
        }
    }

    fn ci_mode(&self) -> bool {
        self.var("LOSOS_CI").as_deref() == Some("1")
    }
}

/// Run one command line (the program's name first). The exit status: 0, 1
/// for a failure, 2 for a refused host key or a usage error, 130 for a
/// cancel.
pub fn run(args: &[String], io: &mut Io) -> u8 {
    let lang = cli::lang_flag(args);
    let locale = Locale::detect(lang.as_deref(), |k| io.var(k));
    i18n::set_locale(locale);
    let cli = match cli::parse(args) {
        Ok(cli) => cli,
        Err(e) => {
            let text = if (e.use_stderr() && io.stderr_tty) || (!e.use_stderr() && io.stdout_tty) {
                e.render().ansi().to_string()
            } else {
                e.render().to_string()
            };
            let stream = if e.use_stderr() {
                &mut io.err
            } else {
                &mut io.out
            };
            let _ = write!(stream, "{text}");
            return u8::try_from(e.exit_code()).unwrap_or(2);
        }
    };
    crate::logging::init(cli.global.verbose, io.var("LOSOS_LOG"));
    tracing::debug!(
        locale = locale.code(),
        "losos {}",
        crate::tui::panes::VERSION
    );
    match dispatch(cli, io) {
        Ok(code) => code,
        Err(problem) => {
            report(io, &problem);
            problem.exit
        }
    }
}

/// A problem, as miette draws it.
pub fn report(io: &mut Io, problem: &Problem) {
    let theme = if io.stderr_tty && io.var("NO_COLOR").is_none() {
        GraphicalTheme::unicode()
    } else {
        GraphicalTheme::unicode_nocolor()
    };
    let handler = GraphicalReportHandler::new_themed(theme)
        .with_links(false)
        .with_width(100);
    let mut text = String::new();
    if handler.render_report(&mut text, problem).is_err() {
        text = format!("losos: {problem}\n");
    }
    let _ = write!(io.err, "{text}");
}

fn dispatch(cli: Cli, io: &mut Io) -> Result<u8> {
    let env = |k: &str| io.var(k);
    let theme = Theme::new(
        detect_mode(cli.global.theme.as_deref(), &env),
        detect_truecolor(&env),
    );
    let host = cli.global.host.clone();
    match cli.command {
        // The two commands CI runs never touch the keychain there.
        Some(Command::Call { method, path, json }) => {
            call(io, host.as_deref(), &method, &path, json.as_deref())
        }
        Some(Command::Edge { command }) => edge(io, host.as_deref(), command),
        command => {
            io.keychain.probe()?;
            match command {
                None => open(io, host.as_deref().or(cli.url.as_deref()), theme),
                Some(Command::Open { url }) => open(io, host.as_deref().or(url.as_deref()), theme),
                Some(Command::Login {
                    url,
                    key,
                    fingerprint,
                }) => login(io, &url, key.as_deref(), fingerprint.as_deref(), theme),
                Some(Command::Logout { url }) => logout(io, &url),
                Some(Command::Hosts) => hosts(io),
                Some(Command::Key { command }) => key(io, command),
                Some(Command::Ci {
                    command: CiCommand::Setup { edge, repo, print },
                }) => ci_setup(io, &edge, &repo, print),
                Some(Command::Cloud {
                    command: CloudCommand::Setup { device },
                }) => device_setup(io, host.as_deref(), Service::Cloud, device.as_deref()),
                Some(Command::Git {
                    command: GitCommand::Setup { device },
                }) => device_setup(io, host.as_deref(), Service::Git, device.as_deref()),
                Some(Command::Git {
                    command: GitCommand::Credential { action },
                }) => git_credential(io, &action),
                Some(Command::Call { .. } | Command::Edge { .. }) => unreachable!("handled above"),
            }
        }
    }
}

// ── Keys ─────────────────────────────────────────────────────────────────

fn parse_url(raw: &str) -> Result<String> {
    normalize_url(raw).map_err(|_| Problem::new("losos::url", tr!("tui.login.badUrl", url = raw)))
}

fn stored_keys(vault: &Vault) -> Result<Vec<UserKey>> {
    Ok(vault
        .list_keys()?
        .iter()
        .filter_map(|armored| UserKey::from_armored(armored).ok())
        .collect())
}

fn key_problem(e: losos_access::KeyError) -> Problem {
    Problem::new("losos::key", e.to_string())
}

fn import_key(io: &mut Io, armored: &str) -> Result<()> {
    let key = UserKey::from_armored(armored)?;
    let fingerprint = key.fingerprint().to_string();
    let label = format!(
        "losos gpg-key {}",
        key.user_ids()
            .first()
            .cloned()
            .unwrap_or_else(|| fingerprint.clone())
    );
    let secret = key.armored_secret().map_err(key_problem)?;
    io.vault().put_key(&fingerprint, &secret, &label)?;
    tracing::info!(%fingerprint, "stored the key");
    io.line(tr!(
        "tui.key.imported",
        fingerprint = group_fingerprint(&fingerprint)
    ));
    for id in key.user_ids() {
        io.line(format!("  {id}"));
    }
    if key.needs_passphrase() {
        io.line(tr!("tui.key.protected"));
    }
    Ok(())
}

fn export_from_gpg(io: &Io, id: &str) -> Result<String> {
    let gpg = io.var("LOSOS_GPG").unwrap_or_else(|| "gpg".into());
    let output = Process::new(&gpg)
        .args(["--export-secret-keys", "--armor", id])
        .env_clear()
        .envs(&io.env)
        .stdin(Stdio::inherit())
        .output()
        .map_err(|_| Problem::new("losos::gpg", tr!("tui.key.noGpg")))?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    if !output.status.success() || stdout.trim().is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let detail = if stderr.is_empty() {
            format!(
                "exit {}",
                output.status.code().map_or("?".into(), |c| c.to_string())
            )
        } else {
            stderr
        };
        return Err(Problem::new(
            "losos::gpg",
            tr!("tui.key.gpgFailed", id = id, detail = detail),
        ));
    }
    Ok(stdout)
}

fn key(io: &mut Io, command: KeyCommand) -> Result<u8> {
    let vault = io.vault();
    match command {
        KeyCommand::Import { file, gpg } => {
            let armored = match (gpg, file) {
                (Some(id), _) => export_from_gpg(io, &id)?,
                (None, Some(file)) => std::fs::read_to_string(&file).map_err(|_| {
                    Problem::new("losos::file", tr!("tui.key.unreadable", file = &file))
                })?,
                (None, None) => unreachable!("clap requires one"),
            };
            import_key(io, &armored)?;
        }
        KeyCommand::List => {
            let keys = stored_keys(&vault)?;
            if keys.is_empty() {
                io.line(tr!("tui.key.none"));
            }
            for k in keys {
                io.line(group_fingerprint(k.fingerprint()));
                for id in k.user_ids() {
                    io.line(format!("  {id}"));
                }
            }
        }
        KeyCommand::Remove { fingerprint } => {
            let keys = stored_keys(&vault)?;
            let Some(found) = keys
                .iter()
                .find(|k| same_fingerprint(k.fingerprint(), &fingerprint))
            else {
                return Err(Problem::new(
                    "losos::key",
                    tr!("tui.login.keyNotStored", fingerprint = &fingerprint),
                ));
            };
            let users: Vec<String> = vault
                .list_hosts()?
                .into_iter()
                .filter(|h| same_fingerprint(&h.key_fingerprint, found.fingerprint()))
                .map(|h| h.url)
                .collect();
            vault.remove_key(found.fingerprint())?;
            io.line(tr!(
                "tui.key.removed",
                fingerprint = group_fingerprint(found.fingerprint())
            ));
            if !users.is_empty() {
                io.line(tr!("tui.key.orphans", hosts = users.join(", ")));
            }
        }
    }
    Ok(0)
}

// ── Hosts ────────────────────────────────────────────────────────────────

/// What `/api/access/host-key` (a box) or `/operator/host-key` (an edge)
/// serves, and the certificate the host showed, when no authority vouched
/// for it.
struct Served {
    kind: HostKind,
    fingerprint: String,
    public_key: String,
    tls_pin: Option<String>,
}

fn fetch_host_key(url: &str) -> Result<Served> {
    let http = Http::new(Trust::FirstUse);
    let mut last = String::new();
    for kind in [HostKind::Box, HostKind::Edge] {
        let at = host_key_url(url, kind);
        tracing::info!(url = %at, "asking for the host key");
        let response = http.get(&at, "application/json").map_err(|e| match e {
            crate::http::HttpError::Unreachable(detail) => Problem::new(
                "losos::unreachable",
                tr!("tui.login.unreachable", url = url, detail = detail),
            ),
            other => unreachable(url, other),
        })?;
        if response.status != 200 {
            last = format!("{at}: HTTP {}", response.status);
            continue;
        }
        let Ok(doc) = serde_json::from_str::<Value>(&response.body) else {
            last = format!("{at}: not JSON");
            continue;
        };
        let Some(public_key) = doc.get("publicKey").and_then(Value::as_str) else {
            last = format!("{at}: no publicKey");
            continue;
        };
        let key = SeededKey::from_armored(public_key).map_err(|e| {
            Problem::new(
                "losos::host_key",
                tr!("tui.login.noHostKey", url = url, detail = e.to_string()),
            )
        })?;
        // The served fingerprint must be the key's own; a mismatch is not a
        // host key.
        if let Some(said) = doc.get("fingerprint").and_then(Value::as_str) {
            if !same_fingerprint(said, &key.fingerprint) {
                return Err(Problem::new(
                    "losos::host_key",
                    tr!("tui.login.inconsistent"),
                ));
            }
        }
        let tls_pin = http.taken_certificate();
        if tls_pin.is_some() {
            tracing::info!("no authority vouches for the certificate; pinning it");
        }
        return Ok(Served {
            kind,
            fingerprint: key.fingerprint,
            public_key: public_key.to_string(),
            tls_pin,
        });
    }
    Err(Problem::new(
        "losos::host_key",
        tr!("tui.login.noHostKey", url = url, detail = last),
    ))
}

fn pick_host(vault: &Vault, wanted: Option<&str>, kind: Option<HostKind>) -> Result<HostRecord> {
    if let Some(wanted) = wanted {
        let url = parse_url(wanted)?;
        return vault
            .get_host(&url)?
            .ok_or_else(|| Problem::new("losos::host", tr!("tui.host.unknown", url = &url)));
    }
    let hosts: Vec<HostRecord> = vault
        .list_hosts()?
        .into_iter()
        .filter(|h| kind.is_none_or(|k| h.kind == k))
        .collect();
    match hosts.len() {
        0 => Err(Problem::new(
            "losos::host",
            tr!(if kind == Some(HostKind::Edge) {
                "tui.host.noEdge"
            } else {
                "tui.host.none"
            }),
        )),
        1 => Ok(hosts.into_iter().next().expect("one host")),
        _ => Err(Problem::new(
            "losos::host",
            tr!(
                "tui.host.several",
                hosts = hosts
                    .iter()
                    .map(|h| h.url.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        )
        .help(tr!("tui.help.opt.host"))),
    }
}

fn unlock(io: &mut Io, key: &UserKey) -> Result<Unlocked> {
    if !key.needs_passphrase() {
        return Ok(key.unlock("")?);
    }
    if let Some(passphrase) = io.var("LOSOS_PASSPHRASE") {
        return Ok(key.unlock(&passphrase)?);
    }
    if !io.stdin_tty {
        return Err(Problem::new("losos::key", tr!("tui.key.needsPassphrase")));
    }
    let grouped = group_fingerprint(key.fingerprint());
    let tail: String = grouped
        .chars()
        .skip(grouped.chars().count().saturating_sub(14))
        .collect();
    let passphrase = read_hidden(&tr!("tui.key.passphrase", fingerprint = tail), &mut io.err)?;
    Ok(key.unlock(&passphrase)?)
}

fn session_for(io: &mut Io, host: &HostRecord) -> Result<Session> {
    let armored = io.vault().get_key(&host.key_fingerprint)?.ok_or_else(|| {
        Problem::new(
            "losos::key",
            tr!(
                "tui.host.keyGone",
                fingerprint = group_fingerprint(&host.key_fingerprint)
            ),
        )
    })?;
    let key = UserKey::from_armored(&armored)?;
    let user = unlock(io, &key)?;
    let trust = host.tls_pin.clone().map_or(Trust::Public, Trust::Pinned);
    Session::new(host, user, Http::new(trust))
}

/// CI mode (`LOSOS_CI=1`): no Secret Service on a runner, so the key, token,
/// URL and pinned host key come from four variables `losos ci setup` put
/// into the repository's Actions settings. The keychain is not touched.
fn ci_session(io: &Io) -> Result<Session> {
    let need = |name: &str| -> Result<String> {
        io.var(name)
            .filter(|v| !v.trim().is_empty())
            .ok_or_else(|| Problem::new("losos::ci", tr!("tui.ci.missingVar", name = name)))
    };
    let url = parse_url(&need("LOSOS_EDGE_URL")?)?;
    let key = UserKey::from_armored(&need("LOSOS_CI_GPG_KEY")?)?;
    if key.needs_passphrase() {
        return Err(Problem::new("losos::ci", tr!("tui.ci.keyProtected")));
    }
    let public_key = need("LOSOS_EDGE_HOST_KEY")?;
    let host_key = SeededKey::from_armored(&public_key)
        .map_err(|_| Problem::new("losos::pin", tr!("tui.host.pinBroken")))?;
    let record = HostRecord {
        url,
        kind: HostKind::Edge,
        fingerprint: host_key.fingerprint.clone(),
        public_key,
        token: need("LOSOS_EDGE_TOKEN")?.trim().to_string(),
        key_fingerprint: key.fingerprint().to_string(),
        tls_pin: None,
    };
    Session::new(&record, key.unlock("")?, Http::new(Trust::Public))
}

fn is_token(token: &str) -> bool {
    let Some(rest) = token.strip_prefix("losos_pat_") else {
        return false;
    };
    let hex = |s: &str, n: usize| {
        s.len() == n && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    };
    matches!(rest.split_once('_'), Some((id, secret)) if hex(id, 8) && hex(secret, 40))
}

/// A certificate's SHA-256, in groups of four.
fn group_pin(pin: &str) -> String {
    pin.to_ascii_uppercase()
        .as_bytes()
        .chunks(4)
        .map(|c| String::from_utf8_lossy(c).into_owned())
        .collect::<Vec<_>>()
        .join(" ")
}

fn login(
    io: &mut Io,
    raw: &str,
    wanted_key: Option<&str>,
    expected: Option<&str>,
    theme: Theme,
) -> Result<u8> {
    let url = parse_url(raw)?;
    let served = fetch_host_key(&url)?;
    let vault = io.vault();
    let pinned = vault.get_host(&url)?;
    if let Some(pinned) = &pinned {
        if !same_fingerprint(&pinned.fingerprint, &served.fingerprint) {
            let text = [
                String::new(),
                tr!("tui.login.changed1"),
                tr!("tui.login.changed2", url = &url),
                String::new(),
                format!(
                    "  {}  {}",
                    tr!("tui.login.pinned"),
                    group_fingerprint(&pinned.fingerprint)
                ),
                format!(
                    "  {}  {}",
                    tr!("tui.login.served"),
                    group_fingerprint(&served.fingerprint)
                ),
                String::new(),
                tr!("tui.login.changed3"),
                tr!("tui.login.changed4", url = &url),
                String::new(),
            ]
            .join("\n");
            let _ = writeln!(io.err, "{text}");
            return Err(Problem::new("losos::host_key_changed", tr!("tui.login.refused")).exit(2));
        }
    }

    let keys = stored_keys(&vault)?;
    if keys.is_empty() {
        return Err(Problem::new("losos::key", tr!("tui.login.noKeys")));
    }
    let mut key_fingerprint = match wanted_key {
        Some(wanted) => {
            let compact: String = wanted
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>()
                .to_ascii_uppercase();
            let found = keys.iter().find(|k| {
                same_fingerprint(k.fingerprint(), wanted)
                    || (!compact.is_empty() && k.fingerprint().ends_with(&compact))
            });
            let Some(found) = found else {
                return Err(Problem::new(
                    "losos::key",
                    tr!("tui.login.keyNotStored", fingerprint = wanted),
                ));
            };
            Some(found.fingerprint().to_string())
        }
        None => None,
    };
    if let Some(expected) = expected {
        if !same_fingerprint(expected, &served.fingerprint) {
            return Err(Problem::new(
                "losos::host_key",
                tr!(
                    "tui.login.flagMismatch",
                    served = group_fingerprint(&served.fingerprint)
                ),
            )
            .exit(2));
        }
    }
    let confirmed = expected.is_some()
        || pinned
            .as_ref()
            .is_some_and(|p| same_fingerprint(&p.fingerprint, &served.fingerprint));

    let token = if io.terminal() {
        let choices = keys
            .iter()
            .map(|k| KeyChoice {
                fingerprint: k.fingerprint().to_string(),
                user_ids: k.user_ids().to_vec(),
            })
            .collect();
        let mut screen = Login::new(
            url.clone(),
            served.kind,
            served.fingerprint.clone(),
            choices,
            confirmed,
            key_fingerprint.clone(),
        );
        term::run(&mut screen, theme)?;
        let Some(Some(answer)) = screen.outcome else {
            return Err(Problem::cancelled());
        };
        key_fingerprint = Some(answer.key_fingerprint);
        answer.token
    } else {
        // A script: the fingerprint is confirmed on the command line, the
        // token comes on stdin, the key is named or is the only one.
        if !confirmed {
            return Err(Problem::new(
                "losos::host_key",
                tr!(
                    "tui.login.needsFingerprint",
                    served = group_fingerprint(&served.fingerprint)
                ),
            ));
        }
        let token = read_piped(&mut io.stdin)?;
        if key_fingerprint.is_none() {
            if keys.len() > 1 {
                return Err(Problem::new("losos::key", tr!("tui.login.whichKeyFlag")));
            }
            key_fingerprint = Some(keys[0].fingerprint().to_string());
        }
        token
    };
    let token = token.trim().to_string();
    if !is_token(&token) {
        return Err(Problem::new("losos::token", tr!("tui.login.badToken")));
    }
    vault.put_host(&HostRecord {
        url: url.clone(),
        kind: served.kind,
        fingerprint: served.fingerprint,
        public_key: served.public_key,
        token,
        key_fingerprint: key_fingerprint.expect("a key was chosen"),
        tls_pin: served.tls_pin.clone(),
    })?;
    io.line(tr!("tui.login.done", url = &url, kind = served.kind.word()));
    if let Some(pin) = served.tls_pin {
        io.line(tr!("tui.login.certificatePinned", pin = group_pin(&pin)));
    }
    Ok(0)
}

fn logout(io: &mut Io, raw: &str) -> Result<u8> {
    let url = parse_url(raw)?;
    let vault = io.vault();
    if vault.get_host(&url)?.is_none() {
        return Err(Problem::new(
            "losos::host",
            tr!("tui.host.unknown", url = &url),
        ));
    }
    vault.remove_host(&url)?;
    io.line(tr!("tui.logout.done", url = &url));
    Ok(0)
}

fn hosts(io: &mut Io) -> Result<u8> {
    let hosts = io.vault().list_hosts()?;
    if hosts.is_empty() {
        io.line(tr!("tui.host.none"));
    }
    for h in hosts {
        io.line(format!("{}  {}", h.url, h.kind.word()));
        io.line(format!(
            "  {}: {}",
            tr!("tui.login.hostKey"),
            group_fingerprint(&h.fingerprint)
        ));
        io.line(format!(
            "  {}: {}",
            tr!("tui.about.userKey"),
            group_fingerprint(&h.key_fingerprint)
        ));
    }
    Ok(0)
}

fn open(io: &mut Io, named: Option<&str>, theme: Theme) -> Result<u8> {
    if !io.terminal() {
        return Err(Problem::new(
            "losos::terminal",
            tr!("tui.open.needsTerminal"),
        ));
    }
    let vault = io.vault();
    let host = match named {
        Some(named) => pick_host(&vault, Some(named), None)?,
        None => {
            let mut hosts = vault.list_hosts()?;
            match hosts.len() {
                0 => return Err(Problem::new("losos::host", tr!("tui.host.none"))),
                1 => hosts.remove(0),
                _ => {
                    let mut picker = Picker::new(hosts.clone());
                    term::run(&mut picker, theme)?;
                    match picker.outcome {
                        Some(Some(i)) => hosts.remove(i),
                        _ => return Ok(0),
                    }
                }
            }
        }
    };
    let session = session_for(io, &host)?;
    let mut app = App::new(host, Arc::new(session)).with_computer(io.computer());
    term::run(&mut app, theme)?;
    Ok(0)
}

// ── One call ─────────────────────────────────────────────────────────────

fn call(
    io: &mut Io,
    host: Option<&str>,
    method: &str,
    path: &str,
    json: Option<&str>,
) -> Result<u8> {
    let body = match json {
        None => Value::Null,
        Some(raw) => serde_json::from_str(raw)
            .map_err(|_| Problem::new("losos::json", tr!("tui.call.badJson")))?,
    };
    let session = if io.ci_mode() {
        ci_session(io)?
    } else {
        let host = pick_host(&io.vault(), host, None)?;
        session_for(io, &host)?
    };
    let answer = session.request(method, path, body)?;
    let note = if answer.sealed {
        String::new()
    } else {
        format!(", {}", tr!("tui.call.notSealed"))
    };
    let _ = writeln!(io.err, "{}{note}", answer.status);
    io.line(show(&answer.body));
    Ok(if answer.ok() { 0 } else { 1 })
}

fn show(body: &Value) -> String {
    match body {
        Value::String(s) => s.clone(),
        other => serde_json::to_string_pretty(other).unwrap_or_default(),
    }
}

// ── The edge ─────────────────────────────────────────────────────────────

fn edge_session(io: &mut Io, host: Option<&str>) -> Result<Session> {
    if io.ci_mode() {
        return ci_session(io);
    }
    let record = pick_host(&io.vault(), host, Some(HostKind::Edge))?;
    if record.kind != HostKind::Edge {
        return Err(Problem::new(
            "losos::host",
            tr!("tui.edge.notEdge", url = &record.url),
        ));
    }
    session_for(io, &record)
}

fn edge(io: &mut Io, host: Option<&str>, command: EdgeCommand) -> Result<u8> {
    match command {
        EdgeCommand::Reconcile => {
            edge_session(io, host)?.post("/reconcile", Value::Null)?;
            io.line(tr!("tui.edge.reconciled"));
        }
        EdgeCommand::Secret {
            command: SecretCommand::List,
        } => {
            let list = edge_session(io, host)?.get("/secrets")?;
            for s in list.as_array().into_iter().flatten() {
                let name = s.get("name").and_then(Value::as_str).unwrap_or("");
                let sealed = s.get("sealed").and_then(Value::as_bool).unwrap_or(false);
                let when = match s.get("updated") {
                    Some(Value::Number(n)) => n
                        .as_f64()
                        .and_then(|secs| chrono::DateTime::from_timestamp(secs as i64, 0))
                        .map(|d| d.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
                        .unwrap_or_else(|| "-".into()),
                    Some(Value::String(s)) => s.clone(),
                    _ => "-".into(),
                };
                let state = tr!(if sealed {
                    "tui.edge.sealed"
                } else {
                    "tui.edge.notSealed"
                });
                io.line(format!("{name:<32} {state}  {when}"));
            }
        }
        EdgeCommand::Secret {
            command: SecretCommand::Set { name },
        } => {
            let session = edge_session(io, host)?;
            let value = io.secret(&tr!("tui.edge.valuePrompt", name = &name))?;
            if value.is_empty() {
                return Err(Problem::new("losos::secret", tr!("tui.edge.emptyValue")));
            }
            let answer = session.post("/secrets", json!({ "name": name, "value": value }))?;
            io.line(tr!("tui.edge.secretSet", name = &name));
            if !(answer.is_null() || answer.as_str() == Some("")) {
                io.line(show(&answer));
            }
        }
        EdgeCommand::Escrow {
            command: EscrowCommand::Releaser { make, replace },
        } => return releaser(io, host, make || replace, replace),
    }
    Ok(0)
}

/// Wei as ETH with up to six decimals, without floating point.
pub fn format_eth(wei: &str) -> String {
    let Ok(n) = wei.trim().parse::<u128>() else {
        return format!("{wei} wei");
    };
    let unit = 10u128.pow(18);
    let whole = n / unit;
    let frac = format!("{:06}", (n % unit) / 10u128.pow(12));
    let frac = frac.trim_end_matches('0');
    if frac.is_empty() {
        format!("{whole} ETH")
    } else {
        format!("{whole}.{frac} ETH")
    }
}

/// The escrow's releaser: the edge makes the key, so only its address is
/// ever shown, here or anywhere.
fn releaser(io: &mut Io, host: Option<&str>, make: bool, replace: bool) -> Result<u8> {
    let session = edge_session(io, host)?;
    if make {
        let body = if replace {
            json!({ "replace": true })
        } else {
            Value::Null
        };
        let answer = session.post("/escrow/releaser", body)?;
        let address = answer.get("address").and_then(Value::as_str).unwrap_or("");
        io.line(tr!("tui.edge.releaserMade", address = address));
        io.line(tr!("tui.edge.releaserNext", address = address));
        return Ok(0);
    }
    let report = session.get("/escrow/releaser")?;
    let Some(address) = report.get("address").and_then(Value::as_str) else {
        io.line(tr!("tui.edge.releaserNone"));
        return Ok(0);
    };
    io.line(tr!("tui.edge.releaserAddress", address = address));
    for n in report
        .get("networks")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let field = |k: &str| n.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let state = match n.get("error").and_then(Value::as_str) {
            Some(error) => error.to_string(),
            None => {
                let balance = format_eth(
                    &n.get("balanceWei")
                        .and_then(Value::as_str)
                        .map_or("0".into(), str::to_string),
                );
                let agrees = if n.get("escrowAgrees").and_then(Value::as_bool) == Some(true) {
                    tr!("tui.edge.releaserAgrees")
                } else {
                    tr!("tui.edge.releaserDisagrees", escrow = field("escrow"))
                };
                format!("{balance}  {agrees}")
            }
        };
        io.line(format!("  {:<20} {state}", field("network")));
    }
    Ok(0)
}

// ── This computer ────────────────────────────────────────────────────────

/// `losos cloud setup` and `losos git setup`.
fn device_setup(
    io: &mut Io,
    host: Option<&str>,
    service: Service,
    device: Option<&str>,
) -> Result<u8> {
    let record = pick_host(&io.vault(), host, Some(HostKind::Box))?;
    if record.kind != HostKind::Box {
        return Err(Problem::new(
            "losos::host",
            tr!("tui.computer.notBox", url = &record.url),
        ));
    }
    let session = session_for(io, &record)?;
    let computer = io.computer();
    let device = computer.device_name(device);
    let password = io.secret(&tr!("tui.computer.passwordPrompt"))?;
    if password.is_empty() {
        return Err(Problem::new(
            "losos::password",
            tr!("tui.computer.emptyPassword"),
        ));
    }
    match service {
        Service::Cloud => {
            let done = computer.cloud_setup(&record, &session, &password, &device)?;
            io.line(tr!("tui.computer.cloudDone", device = &device));
            io.line(tr!("tui.computer.cloudWhere", location = &done.location));
            if done.own_certificate {
                io.line(tr!("tui.computer.cloudCertificate"));
            }
            io.line(tr!("tui.computer.cloudRevoke", device = &device));
        }
        Service::Git => {
            let done = computer.git_setup(&record, &session, &password, &device)?;
            io.line(tr!("tui.computer.gitDone", device = &device));
            io.line(tr!(
                "tui.computer.gitWhere",
                location = format!("{}{}/<repository>.git", done.location, done.user)
            ));
            io.line(tr!("tui.computer.gitRevoke", device = &device));
        }
    }
    Ok(0)
}

/// `losos git credential <get|store|erase>`, as git runs it.
fn git_credential(io: &mut Io, action: &str) -> Result<u8> {
    let input = read_piped(&mut io.stdin)?;
    let answer = io.computer().git_credential(action, &input)?;
    let _ = write!(io.out, "{answer}");
    Ok(0)
}

// ── CI setup ─────────────────────────────────────────────────────────────

struct Value4 {
    name: &'static str,
    value: String,
    secret: bool,
}

fn gh_command(io: &Io) -> String {
    io.var("LOSOS_GH").unwrap_or_else(|| "gh".into())
}

fn gh_available(io: &Io) -> bool {
    Process::new(gh_command(io))
        .args(["auth", "status"])
        .env_clear()
        .envs(&io.env)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

fn gh_set(io: &Io, v: &Value4, repo: &str) -> i32 {
    let child = Process::new(gh_command(io))
        .args([
            if v.secret { "secret" } else { "variable" },
            "set",
            v.name,
            "--repo",
            repo,
        ])
        .env_clear()
        .envs(&io.env)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn();
    let Ok(mut child) = child else {
        return 127;
    };
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(v.value.as_bytes());
    }
    child.wait().ok().and_then(|s| s.code()).unwrap_or(1)
}

fn ci_setup(io: &mut Io, edge: &str, repo: &str, print: bool) -> Result<u8> {
    let vault = io.vault();
    let url = parse_url(edge)?;
    let host = vault
        .get_host(&url)?
        .ok_or_else(|| Problem::new("losos::ci", tr!("tui.ci.notLoggedIn", url = &url)))?;
    if host.kind != HostKind::Edge {
        return Err(Problem::new(
            "losos::host",
            tr!("tui.edge.notEdge", url = &url),
        ));
    }

    // A key for CI alone, never the owner's own, and never one the edge made.
    let user_id = format!("LosOS CI <{repo}>");
    let ci_key = HostKey::generate(&user_id).map_err(key_problem)?;
    let fingerprint = ci_key.fingerprint().to_string();
    let secret_armor = ci_key.armored_secret().map_err(key_problem)?;
    vault.put_key(
        &fingerprint,
        &secret_armor,
        &format!("losos gpg-key {user_id}"),
    )?;
    tracing::info!(%fingerprint, "made the CI key");

    io.line(tr!("tui.ci.generated", repo = repo));
    io.line(format!("  {}", group_fingerprint(&fingerprint)));
    io.line("");
    io.line(ci_key.armored_public().trim());
    io.line("");
    io.line(tr!("tui.ci.seed"));
    io.line("  losos.edge.access.gpgKeys");
    io.line(tr!("tui.ci.token"));
    io.line(format!(
        "  losos-registrar provision token create --edge {url} --name ci --key {fingerprint} --scope full"
    ));
    io.line("");

    let token = io.secret(&tr!("tui.ci.tokenPrompt"))?.trim().to_string();
    if !is_token(&token) {
        return Err(Problem::new("losos::token", tr!("tui.login.badToken")));
    }

    let values = [
        Value4 {
            name: "LOSOS_CI_GPG_KEY",
            value: secret_armor,
            secret: true,
        },
        Value4 {
            name: "LOSOS_EDGE_TOKEN",
            value: token,
            secret: true,
        },
        Value4 {
            name: "LOSOS_EDGE_URL",
            value: url.clone(),
            secret: false,
        },
        Value4 {
            name: "LOSOS_EDGE_HOST_KEY",
            value: host.public_key.clone(),
            secret: false,
        },
    ];
    if gh_available(io) {
        for v in &values {
            let code = gh_set(io, v, repo);
            if code != 0 {
                return Err(Problem::new(
                    "losos::gh",
                    tr!("tui.ci.ghFailed", name = v.name, code = code),
                ));
            }
            let said = if v.secret {
                "tui.ci.setSecret"
            } else {
                "tui.ci.setVariable"
            };
            io.line(tr!(said, name = v.name, repo = repo));
        }
    } else {
        io.line(tr!("tui.ci.noGh", repo = repo));
        for v in &values {
            let kind = tr!(if v.secret {
                "tui.ci.kindSecret"
            } else {
                "tui.ci.kindVariable"
            });
            io.line(format!("  {}  ({kind})", v.name));
        }
        if print {
            for v in &values {
                io.line("");
                io.line(format!("{}:", v.name));
                io.line(v.value.trim());
            }
        } else {
            io.line(tr!("tui.ci.printHint"));
        }
    }
    io.line(tr!(
        "tui.ci.rotate",
        fingerprint = group_fingerprint(&fingerprint)
    ));
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eth_is_exact() {
        assert_eq!(format_eth("0"), "0 ETH");
        assert_eq!(format_eth("1000000000000000000"), "1 ETH");
        assert_eq!(format_eth("1234567890000000000"), "1.234567 ETH");
        assert_eq!(format_eth("500000000000000"), "0.0005 ETH");
        assert_eq!(format_eth("x"), "x wei");
    }

    #[test]
    fn tokens_have_one_shape() {
        let good = format!("losos_pat_{}_{}", "0123abcd", "a".repeat(40));
        assert!(is_token(&good));
        assert!(!is_token(&good.to_uppercase()));
        assert!(!is_token(&format!("losos_pat_0123abcd_{}", "a".repeat(39))));
        assert!(!is_token("losos_pat_"));
    }

    #[test]
    fn a_pin_reads_in_groups() {
        assert_eq!(group_pin("abcdef0123"), "ABCD EF01 23");
    }
}
