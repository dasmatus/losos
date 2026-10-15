//! Everything this client keeps lives in the desktop's Secret Service,
//! reached through libsecret's `secret-tool`. Nothing is written to a plain
//! file, and when no Secret Service answers nothing is kept at all.
//!
//! Items carry `service=losos` and a `kind` (docs/terminal-access.md):
//!
//! ```text
//! kind=gpg-key  fingerprint=<40 hex>  secret: the armoured secret key
//! kind=host     url=<base url>        secret: JSON, see HostRecord
//! ```
//!
//! [`Keychain`] is the narrow interface the rest of the client uses; tests
//! swap in [`MemoryKeychain`], and the suite also runs [`SecretTool`]
//! against a fake `secret-tool` so this file's own code path is exercised.

use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::Mutex;

use crate::problem::{Problem, Result};
use crate::tr;

pub type Attributes<'a> = &'a [(&'a str, &'a str)];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub label: String,
    pub secret: String,
}

pub trait Keychain {
    fn store(&self, attributes: Attributes<'_>, label: &str, secret: &str) -> Result<()>;
    /// The secret of the first item matching every attribute.
    fn lookup(&self, attributes: Attributes<'_>) -> Result<Option<String>>;
    /// Every item matching every attribute.
    fn search(&self, attributes: Attributes<'_>) -> Result<Vec<Item>>;
    fn clear(&self, attributes: Attributes<'_>) -> Result<()>;
    /// Ask the store something harmless, so a missing Secret Service is
    /// reported before the user has typed a token.
    fn probe(&self) -> Result<()> {
        self.search(&[("service", "losos"), ("kind", "probe")])
            .map(|_| ())
    }
}

fn unavailable(detail: &str) -> Problem {
    let message = if detail.is_empty() {
        tr!("tui.keychain.failed")
    } else {
        tr!("tui.keychain.noService", detail = detail)
    };
    Problem::new("losos::keychain", message)
}

/// libsecret's command line tool.
pub struct SecretTool {
    command: String,
}

struct Run {
    code: i32,
    stdout: String,
    stderr: String,
}

impl SecretTool {
    pub fn new(command: impl Into<String>) -> SecretTool {
        SecretTool {
            command: command.into(),
        }
    }

    fn run(&self, args: &[String], input: Option<&str>) -> Result<Run> {
        tracing::debug!(command = %self.command, verb = %args[0], "keychain");
        let mut child = Command::new(&self.command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    Problem::new("losos::keychain", tr!("tui.keychain.noTool"))
                } else {
                    unavailable(&e.to_string())
                }
            })?;
        if let Some(mut stdin) = child.stdin.take() {
            // The process may go away before reading; its status says why.
            let _ = stdin.write_all(input.unwrap_or("").as_bytes());
        }
        let output = child
            .wait_with_output()
            .map_err(|e| unavailable(&e.to_string()))?;
        Ok(Run {
            code: output.status.code().unwrap_or(1),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }

    /// secret-tool answers "nothing found" with status 1 and nothing on
    /// stderr, and a missing or locked-out Secret Service with status 1 and
    /// a message on stderr. The message is what tells the two apart.
    fn fail(run: &Run) -> Problem {
        unavailable(run.stderr.trim())
    }
}

fn flatten(verb: &str, attributes: Attributes<'_>) -> Vec<String> {
    let mut args = vec![verb.to_string()];
    for (name, value) in attributes {
        args.push((*name).to_string());
        args.push((*value).to_string());
    }
    args
}

impl Keychain for SecretTool {
    fn store(&self, attributes: Attributes<'_>, label: &str, secret: &str) -> Result<()> {
        let mut args = vec!["store".to_string(), format!("--label={label}")];
        args.extend(flatten("", attributes).into_iter().skip(1));
        let run = self.run(&args, Some(secret))?;
        if run.code != 0 {
            return Err(Self::fail(&run));
        }
        Ok(())
    }

    fn lookup(&self, attributes: Attributes<'_>) -> Result<Option<String>> {
        let run = self.run(&flatten("lookup", attributes), None)?;
        if run.code == 0 {
            return Ok(Some(run.stdout));
        }
        if run.stderr.trim().is_empty() {
            return Ok(None);
        }
        Err(Self::fail(&run))
    }

    fn search(&self, attributes: Attributes<'_>) -> Result<Vec<Item>> {
        let mut args = vec![
            "search".to_string(),
            "--all".to_string(),
            "--unlock".to_string(),
        ];
        args.extend(flatten("", attributes).into_iter().skip(1));
        let run = self.run(&args, None)?;
        if run.code != 0 {
            let lower = run.stderr.to_lowercase();
            if run.stderr.trim().is_empty()
                || lower.contains("no matching")
                || lower.contains("not found")
            {
                return Ok(Vec::new());
            }
            return Err(Self::fail(&run));
        }
        Ok(parse_search(&run.stdout))
    }

    fn clear(&self, attributes: Attributes<'_>) -> Result<()> {
        let run = self.run(&flatten("clear", attributes), None)?;
        if run.code != 0 && !run.stderr.trim().is_empty() {
            return Err(Self::fail(&run));
        }
        Ok(())
    }
}

fn is_meta(line: &str) -> bool {
    let Some((name, _)) = line.split_once(" = ") else {
        return false;
    };
    matches!(name, "created" | "modified" | "schema")
        || name
            .strip_prefix("attribute.")
            .is_some_and(|rest| !rest.is_empty() && !rest.contains(' '))
}

/// `secret-tool search` prints one block per item:
///
/// ```text
/// [/org/freedesktop/secrets/collection/login/12]
/// label = losos host https://box.local
/// secret = <the secret, which may span lines>
/// created = 2026-10-14 09:00:00
/// modified = 2026-10-14 09:00:00
/// ```
///
/// followed by `schema = ` and `attribute.<name> = ` lines, which libsecret
/// writes to stderr. An armoured key spans many lines, so a secret runs
/// until the `created = ` line that follows it.
pub fn parse_search(stdout: &str) -> Vec<Item> {
    let mut items = Vec::new();
    let mut label = String::new();
    let mut secret: Option<Vec<&str>> = None;
    let mut in_item = false;
    for line in stdout.split('\n') {
        if line.starts_with("[/") && line.ends_with(']') {
            if let (true, Some(s)) = (in_item, secret.take()) {
                items.push(Item {
                    label: std::mem::take(&mut label),
                    secret: s.join("\n"),
                });
            }
            label.clear();
            in_item = true;
            continue;
        }
        if !in_item {
            continue;
        }
        if let Some(s) = secret.as_mut() {
            if !is_meta(line) {
                s.push(line);
                continue;
            }
        }
        if let Some(rest) = line.strip_prefix("label = ") {
            label = rest.to_string();
        } else if let Some(rest) = line.strip_prefix("secret = ") {
            secret = Some(vec![rest]);
        } else if let Some(s) = secret.take() {
            // created, modified, attributes: the secret has ended.
            items.push(Item {
                label: std::mem::take(&mut label),
                secret: s.join("\n"),
            });
            in_item = false;
        }
    }
    if let (true, Some(s)) = (in_item, secret) {
        items.push(Item {
            label,
            secret: s.join("\n"),
        });
    }
    items
}

type Stored = (Vec<(String, String)>, Item);

/// In memory, for tests and the screenshots.
#[derive(Default)]
pub struct MemoryKeychain {
    items: Mutex<Vec<Stored>>,
}

fn matches(have: &[(String, String)], want: Attributes<'_>) -> bool {
    want.iter()
        .all(|(k, v)| have.iter().any(|(hk, hv)| hk == k && hv == v))
}

impl MemoryKeychain {
    pub fn new() -> MemoryKeychain {
        MemoryKeychain::default()
    }

    pub fn len(&self) -> usize {
        self.items.lock().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Keychain for MemoryKeychain {
    fn store(&self, attributes: Attributes<'_>, label: &str, secret: &str) -> Result<()> {
        self.clear(attributes)?;
        let attrs = attributes
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.items.lock().unwrap().push((
            attrs,
            Item {
                label: label.to_string(),
                secret: secret.to_string(),
            },
        ));
        Ok(())
    }

    fn lookup(&self, attributes: Attributes<'_>) -> Result<Option<String>> {
        Ok(self
            .items
            .lock()
            .unwrap()
            .iter()
            .find(|(a, _)| matches(a, attributes))
            .map(|(_, i)| i.secret.clone()))
    }

    fn search(&self, attributes: Attributes<'_>) -> Result<Vec<Item>> {
        Ok(self
            .items
            .lock()
            .unwrap()
            .iter()
            .filter(|(a, _)| matches(a, attributes))
            .map(|(_, i)| i.clone())
            .collect())
    }

    fn clear(&self, attributes: Attributes<'_>) -> Result<()> {
        self.items
            .lock()
            .unwrap()
            .retain(|(a, _)| !matches(a, attributes));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_output_with_a_multi_line_secret() {
        let out = "[/org/freedesktop/secrets/collection/login/1]\n\
                   label = losos gpg-key Matus\n\
                   secret = -----BEGIN PGP PRIVATE KEY BLOCK-----\n\
                   \n\
                   abc = def\n\
                   -----END PGP PRIVATE KEY BLOCK-----\n\
                   created = 2026-10-14 09:00:00\n\
                   modified = 2026-10-14 09:00:00\n\
                   [/org/freedesktop/secrets/collection/login/2]\n\
                   label = losos host https://box.local\n\
                   secret = {\"url\":\"https://box.local\"}\n\
                   created = 2026-10-14 09:00:00\n";
        let items = parse_search(out);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].label, "losos gpg-key Matus");
        assert_eq!(
            items[0].secret,
            "-----BEGIN PGP PRIVATE KEY BLOCK-----\n\nabc = def\n-----END PGP PRIVATE KEY BLOCK-----"
        );
        assert_eq!(items[1].secret, "{\"url\":\"https://box.local\"}");
    }

    #[test]
    fn memory_replaces_an_item_with_the_same_attributes() {
        let k = MemoryKeychain::new();
        k.store(&[("service", "losos"), ("kind", "host")], "a", "1")
            .unwrap();
        k.store(&[("service", "losos"), ("kind", "host")], "b", "2")
            .unwrap();
        assert_eq!(k.len(), 1);
        assert_eq!(k.lookup(&[("kind", "host")]).unwrap().as_deref(), Some("2"));
        k.clear(&[("service", "losos")]).unwrap();
        assert!(k.is_empty());
    }
}
