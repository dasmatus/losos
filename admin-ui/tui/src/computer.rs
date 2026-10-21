//! Setting up this computer for LosOS Cloud and LosOS Git: `losos cloud
//! setup`, `losos git setup` and the This computer pane.
//!
//! The box makes a credential of this computer's own (`POST
//! /api/devices/{cloud,git}`, backend/src/devices.rs), named after the
//! computer so the owner can revoke it on its own, and the client puts it
//! where the desktop's own tools look:
//!
//! - LosOS Cloud: the app password goes into the Secret Service twice, once
//!   as this client's item and once in the shape GVfs looks up before it
//!   asks for a password (`org.gnome.keyring.NetworkPassword`: user, server,
//!   protocol `http`, the realm the server names, authtype `basic`, the port
//!   with the scheme's default filled in). A bookmark in the GTK bookmarks
//!   file puts LosOS Cloud in the file manager's sidebar, and opening it
//!   mounts the files with no password typed.
//! - LosOS Git: the token goes into the Secret Service, and git's global
//!   configuration gets a credential helper for the box's address that is
//!   this program (`losos git credential`), so a clone or a push asks the
//!   keychain and nothing is written to a file. A box with its own
//!   certificate gets the same pin git's libcurl understands: that public
//!   key, whatever name the box was reached by.

use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::envelope::Transport;
use crate::http::{Http, Trust};
use crate::keychain::Keychain;
use crate::problem::{Problem, Result};
use crate::tr;
use crate::vault::HostRecord;

const SERVICE: &str = "losos";
/// The schema GVfs, and libsecret's compatibility API with it, look up.
const NETWORK_SCHEMA: &str = "org.gnome.keyring.NetworkPassword";
/// What Nextcloud calls its WebDAV realm when the probe gets no answer.
const DEFAULT_REALM: &str = "Nextcloud";
/// What the bookmark is called in the file manager.
const BOOKMARK_NAME: &str = "LosOS Cloud";
/// The longest computer name the box takes (backend/src/devices.rs).
const MAX_DEVICE_CHARS: usize = 48;

/// Which of the two.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Service {
    Cloud,
    Git,
}

impl Service {
    fn path(self) -> &'static str {
        match self {
            Service::Cloud => "/api/devices/cloud",
            Service::Git => "/api/devices/git",
        }
    }
}

/// What a setup did, for the sentence after it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Done {
    /// Where the files or the repositories are.
    pub location: String,
    pub user: String,
    /// The box's own certificate was pinned, so a tool may ask about it.
    pub own_certificate: bool,
}

/// This client's own record of a setup, kept in the Secret Service.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Credential {
    pub url: String,
    pub user: String,
    pub secret: String,
    pub location: String,
}

/// The computer: its keychain, its environment and the programs it runs.
#[derive(Clone)]
pub struct Computer {
    pub keychain: Arc<dyn Keychain + Send + Sync>,
    pub env: HashMap<String, String>,
}

impl Computer {
    fn var(&self, name: &str) -> Option<String> {
        self.env.get(name).cloned().filter(|v| !v.is_empty())
    }

    /// The name this computer goes by in LosOS Cloud's and LosOS Git's
    /// lists: `--device`, else `$LOSOS_DEVICE`, else the hostname, made to
    /// fit the box's rule.
    pub fn device_name(&self, given: Option<&str>) -> String {
        let raw = given
            .map(str::to_string)
            .or_else(|| self.var("LOSOS_DEVICE"))
            .unwrap_or_else(|| {
                std::fs::read_to_string("/proc/sys/kernel/hostname").unwrap_or_default()
            });
        device_name(&raw)
    }

    fn config_home(&self) -> Option<PathBuf> {
        self.var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| self.var("HOME").map(|h| PathBuf::from(h).join(".config")))
    }

    /// Whether each of the two is set up for `host` on this computer.
    pub fn status(&self, host: &HostRecord) -> Result<(Option<Credential>, Option<Credential>)> {
        let read = |kind: &str| -> Result<Option<Credential>> {
            Ok(self
                .keychain
                .lookup(&[("service", SERVICE), ("kind", kind), ("url", &host.url)])?
                .and_then(|s| serde_json::from_str(&s).ok()))
        };
        Ok((read("cloud")?, read("git")?))
    }

    /// Ask the box for the credential, with the owner's password.
    fn mint(
        &self,
        transport: &dyn Transport,
        service: Service,
        password: &str,
        device: &str,
    ) -> Result<Value> {
        let answer = transport.request(
            "POST",
            service.path(),
            json!({ "password": password, "device": device }),
        )?;
        if answer.status == 403 && answer.body.get("wrongPassword") == Some(&json!(true)) {
            return Err(Problem::new(
                "losos::password",
                tr!("tui.computer.wrongPassword"),
            ));
        }
        if answer.status == 409 && answer.body.get("gitOff") == Some(&json!(true)) {
            return Err(Problem::new("losos::git", tr!("tui.computer.gitOff")));
        }
        if answer.status == 404 {
            return Err(Problem::new("losos::box", tr!("tui.computer.tooOld")));
        }
        answer.into_result()
    }

    /// `losos cloud setup`.
    pub fn cloud_setup(
        &self,
        host: &HostRecord,
        transport: &dyn Transport,
        password: &str,
        device: &str,
    ) -> Result<Done> {
        let answer = self.mint(transport, Service::Cloud, password, device)?;
        let field = |k: &str| {
            answer
                .get(k)
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string()
        };
        let (user, app_password, path) = (field("user"), field("appPassword"), field("path"));
        if user.is_empty() || app_password.is_empty() || !path.starts_with('/') {
            return Err(Problem::new("losos::box", tr!("tui.computer.oddAnswer")));
        }
        let base = url::Url::parse(&host.url)
            .map_err(|_| Problem::new("losos::url", tr!("tui.login.badUrl", url = &host.url)))?;
        let server = base.host_str().unwrap_or_default().to_string();
        let port = base.port_or_known_default().unwrap_or(443);
        let files = format!("{}{path}", host.url);
        let location = dav_location(&base, &user, &path);

        // GVfs takes the realm the server names in its challenge as the
        // item's `object`, so it is read off the same address Files opens.
        let http = Http::new(host.tls_pin.clone().map_or(Trust::Public, Trust::Pinned));
        let realm = http
            .get(&files, "*/*")
            .ok()
            .and_then(|r| r.authenticate)
            .and_then(|h| realm(&h))
            .unwrap_or_else(|| DEFAULT_REALM.to_string());
        tracing::info!(%realm, %server, port, "storing the app password for Files");

        let record = Credential {
            url: host.url.clone(),
            user: user.clone(),
            secret: app_password.clone(),
            location: location.clone(),
        };
        self.keychain.store(
            &[("service", SERVICE), ("kind", "cloud"), ("url", &host.url)],
            &tr!("tui.computer.cloudLabel", host = &server),
            &serde_json::to_string(&record).expect("a credential serialises"),
        )?;
        let port = port.to_string();
        self.keychain.store(
            &[
                ("xdg:schema", NETWORK_SCHEMA),
                ("user", &user),
                ("server", &server),
                ("protocol", "http"),
                ("object", &realm),
                ("authtype", "basic"),
                ("port", &port),
            ],
            &format!("{user}@{server} ({realm})"),
            &app_password,
        )?;
        if let Some(config) = self.config_home() {
            if let Err(e) = add_bookmark(&config.join("gtk-3.0/bookmarks"), &location) {
                tracing::warn!(error = %e, "the bookmark could not be written");
            }
        }
        Ok(Done {
            location,
            user,
            own_certificate: host.tls_pin.is_some(),
        })
    }

    /// `losos git setup`.
    pub fn git_setup(
        &self,
        host: &HostRecord,
        transport: &dyn Transport,
        password: &str,
        device: &str,
    ) -> Result<Done> {
        let answer = self.mint(transport, Service::Git, password, device)?;
        let field = |k: &str| {
            answer
                .get(k)
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string()
        };
        let (user, token, path) = (field("user"), field("token"), field("path"));
        if user.is_empty() || token.is_empty() || !path.starts_with('/') {
            return Err(Problem::new("losos::box", tr!("tui.computer.oddAnswer")));
        }
        let base = url::Url::parse(&host.url)
            .map_err(|_| Problem::new("losos::url", tr!("tui.login.badUrl", url = &host.url)))?;
        let git_url = format!("{}{}", host.url, path.trim_end_matches('/'));

        // A box's own certificate: the key it carries, as libcurl pins it.
        // Asked for here, over the pinned connection, so the key is the one
        // this client already trusts.
        let pinned_key = match &host.tls_pin {
            None => None,
            Some(pin) => {
                let http = Http::new(Trust::Pinned(pin.clone()));
                http.get(&format!("{git_url}/"), "*/*")
                    .map_err(|e| crate::envelope::unreachable(&host.url, e))?;
                let der = http.taken_certificate_der().ok_or_else(|| {
                    Problem::new("losos::certificate", tr!("tui.computer.noCertificate"))
                })?;
                Some(pinned_public_key(&der).ok_or_else(|| {
                    Problem::new("losos::certificate", tr!("tui.computer.noCertificate"))
                })?)
            }
        };

        let record = Credential {
            url: host.url.clone(),
            user: user.clone(),
            secret: token.clone(),
            location: git_url.clone(),
        };
        self.keychain.store(
            &[
                ("service", SERVICE),
                ("kind", "git"),
                ("url", &host.url),
                ("protocol", base.scheme()),
                ("server", &git_authority(&base)),
            ],
            &tr!(
                "tui.computer.gitLabel",
                host = base.host_str().unwrap_or_default()
            ),
            &serde_json::to_string(&record).expect("a credential serialises"),
        )?;

        let helper = format!("!{} git credential", shell_quote(&self.program()));
        let mut settings: Vec<(String, Vec<String>)> = vec![
            // An empty helper first: whatever helpers come before this one
            // in git's configuration do not answer for the box.
            (
                format!("credential.{git_url}.helper"),
                vec![String::new(), helper],
            ),
            (format!("credential.{git_url}.username"), vec![user.clone()]),
        ];
        if let Some(key) = pinned_key {
            settings.push((format!("http.{git_url}.sslVerify"), vec!["false".into()]));
            settings.push((format!("http.{git_url}.pinnedPubkey"), vec![key]));
        }
        for (name, values) in settings {
            self.git(&["config", "--global", "--unset-all", &name], true)?;
            for value in values {
                self.git(&["config", "--global", "--add", &name, &value], false)?;
            }
        }
        Ok(Done {
            location: format!("{git_url}/"),
            user,
            own_certificate: host.tls_pin.is_some(),
        })
    }

    /// How git runs this program again.
    fn program(&self) -> String {
        self.var("LOSOS_SELF").unwrap_or_else(|| {
            std::env::current_exe()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| "losos".into())
        })
    }

    fn git(&self, args: &[&str], may_fail: bool) -> Result<()> {
        let git = self.var("LOSOS_GIT").unwrap_or_else(|| "git".into());
        let status = Command::new(&git)
            .args(args)
            .env_clear()
            .envs(&self.env)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|_| Problem::new("losos::git", tr!("tui.computer.noGit")))?;
        if status.success() || may_fail {
            return Ok(());
        }
        Err(Problem::new(
            "losos::git",
            tr!(
                "tui.computer.gitFailed",
                command = format!("git {}", args.join(" "))
            ),
        ))
    }

    /// `losos git credential get`: git's credential protocol, answered from
    /// the keychain. `store` and `erase` are taken and ignored: the token is
    /// the box's to revoke and `losos git setup` makes it again.
    pub fn git_credential(&self, action: &str, input: &str) -> Result<String> {
        if action != "get" {
            return Ok(String::new());
        }
        let asked: HashMap<&str, &str> = input.lines().filter_map(|l| l.split_once('=')).collect();
        let (Some(protocol), Some(server)) = (asked.get("protocol"), asked.get("host")) else {
            return Ok(String::new());
        };
        let found = self.keychain.search(&[
            ("service", SERVICE),
            ("kind", "git"),
            ("protocol", protocol),
            ("server", server),
        ])?;
        let Some(record) = found
            .into_iter()
            .find_map(|i| serde_json::from_str::<Credential>(&i.secret).ok())
        else {
            return Ok(String::new());
        };
        Ok(format!(
            "username={}\npassword={}\n",
            record.user, record.secret
        ))
    }
}

/// A hostname as the box takes it: letters, digits, dots, dashes and
/// underscores, at most 48 characters, never empty.
pub fn device_name(raw: &str) -> String {
    let cleaned: String = raw
        .trim()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '.' | '-' | '_' | ' ') {
                c
            } else {
                '-'
            }
        })
        .take(MAX_DEVICE_CHARS)
        .collect();
    let cleaned = cleaned.trim().to_string();
    if cleaned.is_empty() {
        "computer".into()
    } else {
        cleaned
    }
}

/// The address Files opens: `davs://user@host/path` (`dav://` over http).
pub fn dav_location(base: &url::Url, user: &str, path: &str) -> String {
    let scheme = if base.scheme() == "https" {
        "davs"
    } else {
        "dav"
    };
    let port = base.port().map(|p| format!(":{p}")).unwrap_or_default();
    format!(
        "{scheme}://{user}@{}{port}{path}",
        base.host_str().unwrap_or_default()
    )
}

/// The host as git's credential protocol names it: with the port when it
/// is not the scheme's default.
fn git_authority(base: &url::Url) -> String {
    let host = base.host_str().unwrap_or_default();
    match base.port() {
        Some(p) => format!("{host}:{p}"),
        None => host.to_string(),
    }
}

/// The `realm` of a `WWW-Authenticate: Basic realm="..."` challenge.
pub fn realm(header: &str) -> Option<String> {
    let at = header.to_ascii_lowercase().find("realm=")?;
    let rest = &header[at + "realm=".len()..];
    let value = match rest.strip_prefix('"') {
        Some(quoted) => quoted.split('"').next()?,
        None => rest.split([',', ' ']).next()?,
    };
    (!value.is_empty()).then(|| value.to_string())
}

/// One line in the GTK bookmarks file, unless it is there already.
fn add_bookmark(file: &std::path::Path, location: &str) -> std::io::Result<()> {
    let existing = std::fs::read_to_string(file).unwrap_or_default();
    if existing
        .lines()
        .any(|l| l.split(' ').next() == Some(location))
    {
        return Ok(());
    }
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut out = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(file)?;
    if !existing.is_empty() && !existing.ends_with('\n') {
        writeln!(out)?;
    }
    writeln!(out, "{location} {BOOKMARK_NAME}")
}

/// A word for `sh -c`, quoted.
fn shell_quote(s: &str) -> String {
    if !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "/._-+".contains(c))
    {
        return s.to_string();
    }
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// `sha256//<base64>` of a certificate's public key, as git's
/// `http.pinnedPubkey` (libcurl's `CURLOPT_PINNEDPUBLICKEY`) takes it.
pub fn pinned_public_key(cert_der: &[u8]) -> Option<String> {
    let spki = subject_public_key_info(cert_der)?;
    Some(format!("sha256//{}", base64(&Sha256::digest(spki))))
}

/// The `subjectPublicKeyInfo` of an X.509 certificate, DER: the seventh
/// element of `tbsCertificate`, after the optional version.
pub fn subject_public_key_info(cert: &[u8]) -> Option<&[u8]> {
    let (_, certificate, _) = element(cert)?;
    let (_, tbs, _) = element(certificate)?;
    let mut rest = tbs;
    if rest.first() == Some(&0xA0) {
        rest = element(rest)?.2;
    }
    // serialNumber, signature, issuer, validity, subject.
    for _ in 0..5 {
        rest = element(rest)?.2;
    }
    let (whole, _, _) = element(rest)?;
    (whole.first() == Some(&0x30)).then_some(whole)
}

/// One DER element at the start of `data`: the whole of it, its content,
/// and what follows.
fn element(data: &[u8]) -> Option<(&[u8], &[u8], &[u8])> {
    let first = *data.get(1)?;
    let (len, header) = if first < 0x80 {
        (usize::from(first), 2)
    } else {
        let n = usize::from(first & 0x7f);
        if n == 0 || n > 4 {
            return None;
        }
        let mut len = 0usize;
        for b in data.get(2..2 + n)? {
            len = (len << 8) | usize::from(*b);
        }
        (len, 2 + n)
    };
    let end = header.checked_add(len)?;
    if end > data.len() {
        return None;
    }
    Some((&data[..end], &data[header..end], &data[end..]))
}

fn base64(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hostnames_become_device_names() {
        assert_eq!(device_name("jana-laptop\n"), "jana-laptop");
        assert_eq!(device_name("a/b:c"), "a-b-c");
        assert_eq!(device_name("  "), "computer");
        assert_eq!(device_name(&"x".repeat(60)).len(), 48);
    }

    #[test]
    fn the_files_address_follows_the_box_address() {
        let https = url::Url::parse("https://box.local").unwrap();
        assert_eq!(
            dav_location(
                &https,
                "notshared",
                "/nextcloud/remote.php/dav/files/notshared/"
            ),
            "davs://notshared@box.local/nextcloud/remote.php/dav/files/notshared/"
        );
        let http = url::Url::parse("http://192.168.1.4:8080").unwrap();
        assert_eq!(
            dav_location(&http, "u", "/p/"),
            "dav://u@192.168.1.4:8080/p/"
        );
        assert_eq!(git_authority(&http), "192.168.1.4:8080");
        assert_eq!(git_authority(&https), "box.local");
    }

    #[test]
    fn realms_are_read_off_the_challenge() {
        assert_eq!(
            realm(r#"Basic realm="LosOS Cloud", charset="UTF-8""#).as_deref(),
            Some("LosOS Cloud")
        );
        assert_eq!(realm("Basic realm=Nextcloud").as_deref(), Some("Nextcloud"));
        assert_eq!(realm("Bearer"), None);
    }

    #[test]
    fn helpers_are_quoted_for_the_shell() {
        assert_eq!(shell_quote("/usr/bin/losos"), "/usr/bin/losos");
        assert_eq!(shell_quote("/home/a b/losos"), "'/home/a b/losos'");
        assert_eq!(shell_quote("/x/it's"), r"'/x/it'\''s'");
    }

    #[test]
    fn base64_matches_the_rfc() {
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
        assert_eq!(base64(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64(b"foob"), "Zm9vYg==");
    }

    #[test]
    fn bookmarks_are_added_once() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("gtk-3.0/bookmarks");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, "file:///home/me/Music").unwrap();
        add_bookmark(&file, "davs://u@box.local/p/").unwrap();
        add_bookmark(&file, "davs://u@box.local/p/").unwrap();
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            "file:///home/me/Music\ndavs://u@box.local/p/ LosOS Cloud\n"
        );
    }
}
