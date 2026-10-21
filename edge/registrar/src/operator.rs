//! The edge's half of `losos`, the terminal client (`docs/edge-terminal.md`).
//!
//! The envelope is the box's (`backend/access`, `docs/terminal-access.md`):
//! a request signed by a GPG key seeded in `losos.edge.access.gpgKeys` and
//! encrypted to this edge's own OpenPGP key, carrying a token made by
//! `provision token create`; the answer signed by the edge and encrypted back
//! to the signer. What differs is what a call can reach. A box forwards a
//! call to its own web API; an edge has none, so the paths here are the
//! whole surface: the overview, the list of sealed secrets, sealing one, a
//! reconcile, and the escrow's releaser key, which the edge makes itself so
//! it never exists anywhere else.
//!
//! Sealing runs `losos-seal-credential`, the script the Actions workflow
//! reaches over SSH, with the value on its stdin. It runs through
//! `systemd-run` so it is outside this unit's sandbox (the registrar cannot
//! even see the Stripe gate's blobs) and outside its cgroup (the script
//! restarts units, and one of them may be this one).

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use losos_access::{HostKey, Keyring, Opened, Replay, Tokens};
use miette::{miette, Result};
use serde::Deserialize;
use tokio::io::AsyncWriteExt;

/// The largest envelope `POST /operator/call` reads. A secret is at most
/// 64 KiB (the script's own limit); armour and encryption add a third.
pub const MAX_ENVELOPE_BYTES: usize = 256 * 1024;
/// The largest value `POST /secrets` passes on, the seal script's limit.
pub const MAX_SECRET_BYTES: usize = 64 * 1024;
/// How long the seal script may take: `systemd-creds` and one restart.
const SEAL_TIMEOUT: Duration = Duration::from_secs(20);
/// The unit the script must not restart in place: this one. It restarts it
/// a few seconds later instead, after the answer has gone out.
const SELF_UNIT: &str = "losos-registrar.service";

/// `serve --access-dir` and friends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperatorOpts {
    /// The edge's own key (`host-key.asc`, made on the first start) and the
    /// token file (`tokens.json`).
    pub access_dir: String,
    /// The operator's seeded public keys, written by `modules/edge.nix` from
    /// `losos.edge.access.gpgKeys`. Missing is an empty ring.
    pub keys_file: String,
    /// `--seal-credential`: the `losos-seal-credential` script. `None` and
    /// the two secret paths answer 503.
    pub seal_script: Option<String>,
    /// `--systemd-run`: run the script as a transient unit. `None` runs it
    /// as a child, which only the tests do.
    pub systemd_run: Option<String>,
}

/// What `serve` holds for the terminal.
pub struct Operator {
    pub host: HostKey,
    pub ring: Keyring,
    pub tokens: Tokens,
    pub replay: Replay,
    seal: Option<Seal>,
}

struct Seal {
    script: PathBuf,
    systemd_run: Option<PathBuf>,
}

impl Operator {
    /// Open the key (making it on the first start), the ring and the token
    /// file. A seeded key that does not parse stops the edge: a typo in the
    /// seed is loud at start, not a key that silently never works.
    pub fn open(opts: &OperatorOpts, hostname: &str) -> Result<Operator> {
        let dir = Path::new(&opts.access_dir);
        let host =
            HostKey::load_or_create(&dir.join("host-key.asc"), &format!("LosOS edge {hostname}"))
                .map_err(|e| miette!("the edge's terminal key: {e}"))?;
        let ring = Keyring::load(Path::new(&opts.keys_file))
            .map_err(|e| miette!("{}: {e}", opts.keys_file))?;
        Ok(Operator {
            host,
            ring,
            tokens: Tokens::new(dir.join("tokens.json")),
            replay: Replay::default(),
            seal: opts.seal_script.as_ref().map(|s| Seal {
                script: PathBuf::from(s),
                systemd_run: opts.systemd_run.as_ref().map(PathBuf::from),
            }),
        })
    }

    pub fn can_seal(&self) -> bool {
        self.seal.is_some()
    }
}

/// The credential the edge makes itself (`modules/edge-credentials.nix`).
pub const RELEASER_SECRET: &str = "usdc-releaser-key";

/// One of the things a call can ask for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Call {
    Overview,
    ListSecrets,
    SetSecret {
        name: String,
        value: String,
    },
    Reconcile,
    /// The releaser's address, gas and escrows.
    Releaser,
    /// Make a releaser key here and seal it. `replace` makes a new one over
    /// a sealed key; without it a sealed key is a 409.
    NewReleaser {
        replace: bool,
    },
}

/// A call that opened but asks for something this edge does not answer.
/// Sealed like any answer: the signer is known by now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotServed {
    pub status: u16,
    pub message: String,
}

fn not_served(status: u16, message: impl Into<String>) -> NotServed {
    NotServed {
        status,
        message: message.into(),
    }
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct NewReleaserBody {
    #[serde(default)]
    replace: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SetBody {
    name: String,
    value: String,
}

/// A credential name as `losos.edge.credentials.secrets` keys them.
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !name.starts_with('-')
}

/// Which [`Call`] the opened request is. The token's scope was already
/// checked against the method (`Tokens::authenticate`).
pub fn route(opened: &Opened) -> Result<Call, NotServed> {
    let r = &opened.request;
    match (r.method.as_str(), r.path.as_str()) {
        ("GET", "/overview") => Ok(Call::Overview),
        ("GET", "/secrets") => Ok(Call::ListSecrets),
        ("POST", "/reconcile") => Ok(Call::Reconcile),
        ("GET", "/escrow/releaser") => Ok(Call::Releaser),
        ("POST", "/escrow/releaser") => {
            let body: NewReleaserBody = if r.body.is_null() {
                NewReleaserBody::default()
            } else {
                serde_json::from_value(r.body.clone())
                    .map_err(|e| not_served(400, format!("expected {{replace}}: {e}")))?
            };
            Ok(Call::NewReleaser {
                replace: body.replace,
            })
        }
        ("POST", "/secrets") => {
            let body: SetBody = serde_json::from_value(r.body.clone())
                .map_err(|e| not_served(400, format!("expected {{name, value}}: {e}")))?;
            if !valid_name(&body.name) {
                return Err(not_served(
                    400,
                    "a credential name is lowercase letters, digits and dashes",
                ));
            }
            if body.name == RELEASER_SECRET {
                return Err(not_served(
                    400,
                    "the edge makes the releaser key itself: POST /escrow/releaser",
                ));
            }
            if body.value.is_empty() || body.value.len() > MAX_SECRET_BYTES {
                return Err(not_served(400, "a secret is 1 byte to 64 KiB"));
            }
            if body.value.contains('\0') {
                return Err(not_served(400, "a secret cannot hold a NUL byte"));
            }
            Ok(Call::SetSecret {
                name: body.name,
                value: body.value,
            })
        }
        (_, "/overview" | "/secrets" | "/reconcile" | "/escrow/releaser") => {
            Err(not_served(405, "not with that method"))
        }
        (_, path) => Err(not_served(404, format!("an edge does not answer {path}"))),
    }
}

/// What the seal script said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sealed {
    pub ok: bool,
    /// stdout on success, stderr on failure, trimmed.
    pub output: String,
}

impl Operator {
    async fn run_seal(&self, args: &[&str], stdin: Option<&str>) -> Result<Sealed, NotServed> {
        let Some(seal) = &self.seal else {
            return Err(not_served(
                503,
                "this edge has no seal script (--seal-credential)",
            ));
        };
        let mut cmd = match &seal.systemd_run {
            Some(run) => {
                let mut c = tokio::process::Command::new(run);
                c.args([
                    "--pipe",
                    "--wait",
                    "--collect",
                    "--quiet",
                    "--service-type=exec",
                    &format!("--setenv=LOSOS_SEAL_DEFER={SELF_UNIT}"),
                    "--",
                ])
                .arg(&seal.script);
                c
            }
            None => {
                let mut c = tokio::process::Command::new(&seal.script);
                c.env("LOSOS_SEAL_DEFER", SELF_UNIT);
                c
            }
        };
        cmd.args(args)
            .stdin(if stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = cmd
            .spawn()
            .map_err(|e| not_served(500, format!("start the seal script: {e}")))?;
        if let (Some(value), Some(mut pipe)) = (stdin, child.stdin.take()) {
            // The value travels on this pipe only: never an argument, an
            // environment variable or a file.
            pipe.write_all(value.as_bytes())
                .await
                .map_err(|e| not_served(500, format!("hand the value over: {e}")))?;
            drop(pipe);
        }
        let out = tokio::time::timeout(SEAL_TIMEOUT, child.wait_with_output())
            .await
            .map_err(|_| not_served(504, "the seal script did not finish"))?
            .map_err(|e| not_served(500, format!("the seal script: {e}")))?;
        let text = |b: &[u8]| String::from_utf8_lossy(b).trim().to_string();
        Ok(if out.status.success() {
            Sealed {
                ok: true,
                output: text(&out.stdout),
            }
        } else {
            Sealed {
                ok: false,
                output: text(&out.stderr),
            }
        })
    }

    /// Seal `value` as `name`.
    pub async fn set_secret(&self, name: &str, value: &str) -> Result<Sealed, NotServed> {
        self.run_seal(&[name], Some(value)).await
    }

    /// `[{name, sealed, updated}]`, from `losos-seal-credential --list`,
    /// which prints one `name<TAB>0|1<TAB>mtime` line per credential.
    pub async fn list_secrets(&self) -> Result<serde_json::Value, NotServed> {
        let out = self.run_seal(&["--list"], None).await?;
        if !out.ok {
            return Err(not_served(500, out.output));
        }
        Ok(serde_json::Value::Array(parse_list(&out.output)))
    }

    /// Whether `name` has a sealed blob, from the same list.
    pub async fn is_sealed(&self, name: &str) -> Result<bool, NotServed> {
        let rows = self.list_secrets().await?;
        Ok(rows
            .as_array()
            .into_iter()
            .flatten()
            .any(|row| row["name"] == name && row["sealed"] == true))
    }
}

/// The `--list` lines as the contract's rows. A line that does not parse is
/// left out rather than failing the list.
pub fn parse_list(text: &str) -> Vec<serde_json::Value> {
    text.lines()
        .filter_map(|line| {
            let mut parts = line.split('\t');
            let name = parts.next()?.trim();
            let sealed = parts.next()?.trim() == "1";
            let updated = parts.next().and_then(|m| m.trim().parse::<i64>().ok());
            valid_name(name).then(|| {
                serde_json::json!({
                    "name": name,
                    "sealed": sealed,
                    "updated": if sealed { updated } else { None },
                })
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use losos_access::Request;

    fn opened(method: &str, path: &str, body: serde_json::Value) -> Opened {
        let key = HostKey::generate("t").unwrap();
        Opened {
            signer: key.as_seeded().unwrap(),
            request: Request {
                v: 1,
                host: String::new(),
                token: String::new(),
                method: method.into(),
                path: path.into(),
                ts: 0,
                nonce: String::new(),
                body,
                delegation: None,
            },
            delegated: None,
        }
    }

    #[test]
    fn the_four_paths_and_nothing_else() {
        assert_eq!(
            route(&opened("GET", "/overview", serde_json::Value::Null)),
            Ok(Call::Overview)
        );
        assert_eq!(
            route(&opened("GET", "/secrets", serde_json::Value::Null)),
            Ok(Call::ListSecrets)
        );
        assert_eq!(
            route(&opened("POST", "/reconcile", serde_json::Value::Null)),
            Ok(Call::Reconcile)
        );
        assert_eq!(
            route(&opened(
                "POST",
                "/secrets",
                serde_json::json!({"name": "stripe-webhook-secret", "value": "whsec_x"})
            )),
            Ok(Call::SetSecret {
                name: "stripe-webhook-secret".into(),
                value: "whsec_x".into()
            })
        );
        assert_eq!(
            route(&opened("DELETE", "/secrets", serde_json::Value::Null))
                .unwrap_err()
                .status,
            405
        );
        assert_eq!(
            route(&opened("GET", "/api/state", serde_json::Value::Null))
                .unwrap_err()
                .status,
            404
        );
        for bad in [
            serde_json::json!({"name": "../etc", "value": "x"}),
            serde_json::json!({"name": "-x", "value": "x"}),
            serde_json::json!({"name": "ok", "value": ""}),
            serde_json::json!({"name": "ok", "value": "a\u{0}b"}),
            serde_json::json!({"name": "ok", "value": "x", "extra": 1}),
        ] {
            assert_eq!(
                route(&opened("POST", "/secrets", bad)).unwrap_err().status,
                400
            );
        }
    }

    #[test]
    fn the_releaser_is_made_here_never_sent() {
        assert_eq!(
            route(&opened("GET", "/escrow/releaser", serde_json::Value::Null)),
            Ok(Call::Releaser)
        );
        assert_eq!(
            route(&opened("POST", "/escrow/releaser", serde_json::Value::Null)),
            Ok(Call::NewReleaser { replace: false })
        );
        assert_eq!(
            route(&opened(
                "POST",
                "/escrow/releaser",
                serde_json::json!({"replace": true})
            )),
            Ok(Call::NewReleaser { replace: true })
        );
        assert_eq!(
            route(&opened(
                "DELETE",
                "/escrow/releaser",
                serde_json::Value::Null
            ))
            .unwrap_err()
            .status,
            405
        );
        assert_eq!(
            route(&opened(
                "POST",
                "/escrow/releaser",
                serde_json::json!({"key": "0x01"})
            ))
            .unwrap_err()
            .status,
            400
        );
        // A key typed or pasted in would have existed somewhere else first.
        assert_eq!(
            route(&opened(
                "POST",
                "/secrets",
                serde_json::json!({"name": RELEASER_SECRET, "value": "ab".repeat(32)})
            ))
            .unwrap_err()
            .status,
            400
        );
    }

    #[test]
    fn the_list_lines_become_rows() {
        let rows = parse_list(
            "stripe-webhook-secret\t1\t1790000000\nstripe-secret-key\t0\t\nnot a line\n",
        );
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["name"], "stripe-webhook-secret");
        assert_eq!(rows[0]["sealed"], true);
        assert_eq!(rows[0]["updated"], 1_790_000_000);
        assert_eq!(rows[1]["sealed"], false);
        assert!(rows[1]["updated"].is_null());
    }
}
