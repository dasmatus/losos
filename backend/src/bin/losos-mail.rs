//! `losos-mail` — the two jobs LosOS Mail's server needs done around it.
//!
//! Each is a oneshot unit in `modules/mail.nix`, run as the `shared` user
//! in the mail server's own sandbox:
//!
//!   * `setup` starts Stalwart in recovery mode, brings its settings in line
//!     with the plan the module wrote, and stops it again. It runs before
//!     every start of the server and does nothing when the plan has not
//!     changed since it last ran.
//!   * `migrate` copies a box's mail from the Stalwart 0.15 store, which
//!     1.0 cannot open, into the running 1.0 server: it starts the old
//!     server on a free loopback port, makes each person's account in the
//!     new one and has Vandelay carry their folders, messages, identities
//!     and filters across. The old store is kept, renamed, never deleted.
//!
//! The rules are in [`losos_ctl::mail_setup`].

use clap::{Parser, Subcommand};
use losos_ctl::mail::{Loopback, Request, Transport, ADMIN_USER, MASTER_USER};
use losos_ctl::mail_setup::{self, Plan};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitCode};
use std::time::{Duration, Instant};

#[derive(Parser)]
#[command(version, about = "Set up and upgrade LosOS Mail's server")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Apply the plan to the server's registry, in recovery mode.
    Setup {
        /// The Stalwart 1.0 binary.
        #[arg(long)]
        stalwart: PathBuf,
        /// Its config.json, which names the data store.
        #[arg(long)]
        config: PathBuf,
        #[arg(long)]
        plan: PathBuf,
        /// The administrator's secret (`mail-master`).
        #[arg(long)]
        secret_file: PathBuf,
        /// Where the hash of the last plan applied is kept.
        #[arg(long)]
        marker: PathBuf,
    },
    /// Copy every mailbox of a 0.15 store into the running server.
    Migrate {
        /// The Stalwart 0.15 binary and its TOML configuration, which binds
        /// to `$LOSOS_MAIL_LEGACY_BIND`.
        #[arg(long)]
        legacy_stalwart: PathBuf,
        #[arg(long)]
        legacy_config: PathBuf,
        /// The 0.15 store; renamed to `<store>-0.15` once everything is
        /// across.
        #[arg(long)]
        legacy_store: PathBuf,
        /// The running 1.0 server's loopback listener.
        #[arg(long)]
        addr: String,
        #[arg(long)]
        domain: String,
        #[arg(long)]
        vandelay: PathBuf,
        /// Where each person's archive is kept while it is copied.
        #[arg(long)]
        work: PathBuf,
        #[arg(long)]
        secret_file: PathBuf,
    },
}

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .without_time()
        .init();
    let result = match Cli::parse().command {
        Cmd::Setup {
            stalwart,
            config,
            plan,
            secret_file,
            marker,
        } => setup(&stalwart, &config, &plan, &secret_file, &marker),
        Cmd::Migrate {
            legacy_stalwart,
            legacy_config,
            legacy_store,
            addr,
            domain,
            vandelay,
            work,
            secret_file,
        } => migrate(&Migration {
            legacy_stalwart,
            legacy_config,
            legacy_store,
            addr,
            domain,
            vandelay,
            work,
            secret_file,
        }),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("losos-mail: {e}");
            ExitCode::FAILURE
        }
    }
}

fn read_secret(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path)
        .map(|s| s.trim().to_string())
        .ok()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("no secret in {}", path.display()))
}

/// A loopback port nothing listens on right now.
fn free_port() -> Result<u16, String> {
    std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .map(|a| a.port())
        .map_err(|e| format!("no free loopback port: {e}"))
}

/// A server this program started, stopped when it goes out of scope.
struct Server {
    child: Child,
    what: &'static str,
}

impl Server {
    /// Wait until `addr` answers HTTP, or the server exits, or two minutes
    /// pass.
    fn wait(&mut self, addr: &str) -> Result<(), String> {
        let transport = Loopback {
            addr: addr.to_string(),
        };
        let until = Instant::now() + Duration::from_secs(120);
        loop {
            if let Ok(Some(status)) = self.child.try_wait() {
                return Err(format!("{} exited before it answered: {status}", self.what));
            }
            let probe = Request {
                method: "GET",
                path: "/jmap/session".into(),
                user: String::new(),
                secret: String::new(),
                body: None,
                content_type: None,
            };
            if transport.send(&probe).is_ok() {
                return Ok(());
            }
            if Instant::now() > until {
                return Err(format!("{} did not answer on {addr}", self.what));
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        // SIGINT is Stalwart's clean shutdown (the units' KillSignal).
        let pid = rustix::process::Pid::from_raw(self.child.id() as i32);
        if let Some(pid) = pid {
            let _ = rustix::process::kill_process(pid, rustix::process::Signal::INT);
        }
        let until = Instant::now() + Duration::from_secs(60);
        while Instant::now() < until {
            if let Ok(Some(_)) = self.child.try_wait() {
                return;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The hash the marker holds: the plan and the binary that applied it.
fn plan_hash(stalwart: &Path, plan: &str) -> String {
    let mut ctx = ring::digest::Context::new(&ring::digest::SHA256);
    ctx.update(stalwart.as_os_str().as_encoded_bytes());
    ctx.update(b"\0");
    ctx.update(plan.as_bytes());
    ctx.finish()
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn setup(
    stalwart: &Path,
    config: &Path,
    plan_file: &Path,
    secret_file: &Path,
    marker: &Path,
) -> Result<(), String> {
    let text =
        std::fs::read_to_string(plan_file).map_err(|e| format!("{}: {e}", plan_file.display()))?;
    let plan = Plan::parse(&text)?;
    let hash = plan_hash(stalwart, &text);
    if std::fs::read_to_string(marker)
        .ok()
        .as_deref()
        .map(str::trim)
        == Some(hash.as_str())
    {
        tracing::info!("the mail server's settings are already this plan's");
        return Ok(());
    }
    let secret = read_secret(secret_file)?;
    let port = free_port()?;
    let addr = format!("127.0.0.1:{port}");
    tracing::info!(%addr, "starting the mail server in recovery mode");
    let child = Command::new(stalwart)
        .arg(format!("--config={}", config.display()))
        .env("STALWART_RECOVERY_MODE", "1")
        .env("STALWART_RECOVERY_MODE_PORT", port.to_string())
        .env("STALWART_RECOVERY_ADMIN", format!("{MASTER_USER}:{secret}"))
        .spawn()
        .map_err(|e| format!("{}: {e}", stalwart.display()))?;
    let mut server = Server {
        child,
        what: "the mail server",
    };
    server.wait(&addr)?;
    let changed = mail_setup::apply(&Loopback { addr }, MASTER_USER, &secret, &plan)?;
    drop(server);
    tracing::info!(changed, "the mail server's settings follow the plan");
    let tmp = marker.with_extension("tmp");
    std::fs::write(&tmp, format!("{hash}\n"))
        .and_then(|()| std::fs::rename(&tmp, marker))
        .map_err(|e| format!("{}: {e}", marker.display()))
}

struct Migration {
    legacy_stalwart: PathBuf,
    legacy_config: PathBuf,
    legacy_store: PathBuf,
    addr: String,
    domain: String,
    vandelay: PathBuf,
    work: PathBuf,
    secret_file: PathBuf,
}

/// One JSON call to a server: `GET` or `POST` with a body.
fn call(
    addr: &str,
    method: &'static str,
    path: &str,
    user: &str,
    secret: &str,
    body: Option<&Value>,
) -> Result<Value, String> {
    let resp = Loopback {
        addr: addr.to_string(),
    }
    .send(&Request {
        method,
        path: path.into(),
        user: user.into(),
        secret: secret.into(),
        body: body.map(|b| serde_json::to_vec(b).unwrap_or_default()),
        content_type: None,
    })
    .map_err(|e| e.message)?;
    if resp.status != 200 {
        return Err(format!(
            "{path} answered {}: {}",
            resp.status,
            String::from_utf8_lossy(&resp.body)
        ));
    }
    serde_json::from_slice(&resp.body).map_err(|e| format!("{path}: not JSON: {e}"))
}

fn vandelay(m: &Migration, secret: &str, args: &[&str]) -> Result<(), String> {
    let status = Command::new(&m.vandelay)
        .args(args)
        // The secret never goes on the command line, where `ps` shows it.
        .env("VANDELAY_PASSWORD", secret)
        .env("HOME", &m.work)
        .status()
        .map_err(|e| format!("{}: {e}", m.vandelay.display()))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("vandelay {} failed: {status}", args[0]))
    }
}

fn migrate(m: &Migration) -> Result<(), String> {
    if !m.legacy_store.exists() {
        tracing::info!("no Stalwart 0.15 store; nothing to copy");
        return Ok(());
    }
    let secret = read_secret(&m.secret_file)?;
    std::fs::create_dir_all(&m.work).map_err(|e| format!("{}: {e}", m.work.display()))?;

    let port = free_port()?;
    let legacy = format!("127.0.0.1:{port}");
    tracing::info!(%legacy, "starting the Stalwart 0.15 server to copy from");
    let child = Command::new(&m.legacy_stalwart)
        .arg(format!("--config={}", m.legacy_config.display()))
        .env("LOSOS_MAIL_LEGACY_BIND", &legacy)
        .spawn()
        .map_err(|e| format!("{}: {e}", m.legacy_stalwart.display()))?;
    let mut old = Server {
        child,
        what: "the Stalwart 0.15 server",
    };
    old.wait(&legacy)?;

    let listed = call(
        &legacy,
        "GET",
        "/api/principal?types=individual&limit=0",
        ADMIN_USER,
        &secret,
        None,
    )?;
    let people = mail_setup::legacy_people(&listed);
    tracing::info!(count = people.len(), "people with mail to copy");

    let domains = call(
        &m.addr,
        "POST",
        "/jmap/",
        MASTER_USER,
        &secret,
        Some(&mail_setup::envelope(vec![mail_setup::domains_call()])),
    )?;
    let domain_id = mail_setup::domain_id(&domains, &m.domain)?;

    for (name, shown) in &people {
        let address = format!("{name}@{}", m.domain);
        let made = call(
            &m.addr,
            "POST",
            "/jmap/",
            MASTER_USER,
            &secret,
            Some(&mail_setup::envelope(vec![mail_setup::account_call(
                &domain_id, name, shown,
            )])),
        )?;
        mail_setup::account_made(&made)?;

        let archive = m.work.join(format!("{name}.sqlite"));
        let archive = archive.to_string_lossy();
        tracing::info!(%name, "copying their mail");
        vandelay(
            m,
            &secret,
            &[
                "import",
                "jmap",
                "--url",
                &format!("http://{legacy}"),
                "--auth-basic",
                &format!("{name}%{MASTER_USER}"),
                "--account-name",
                name,
                "--objects",
                mail_setup::COPIED,
                &archive,
            ],
        )?;
        vandelay(
            m,
            &secret,
            &[
                "export",
                "--url",
                &format!("http://{}", m.addr),
                "--auth-basic",
                &mail_setup::as_person(&address),
                "--account-name",
                &address,
                "--objects",
                mail_setup::COPIED,
                &archive,
            ],
        )?;
        let _ = std::fs::remove_file(&*archive);
    }
    drop(old);

    // Kept, not deleted: the owner's mail as it was, until they let it go.
    let kept = m.legacy_store.with_file_name(format!(
        "{}-0.15",
        m.legacy_store
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    ));
    std::fs::rename(&m.legacy_store, &kept)
        .map_err(|e| format!("{}: {e}", m.legacy_store.display()))?;
    let _ = std::fs::remove_dir(&m.work);
    tracing::info!(kept = %kept.display(), "every mailbox is across; the old store is kept");
    Ok(())
}
