//! The box's credential store, as this daemon sees it.
//!
//! Every secret the box keeps is a systemd credential: one file per secret
//! in `/etc/credstore.encrypted`, encrypted by `systemd-creds` to the TPM, or
//! to the host key in `/var/lib/systemd/credential.secret` on a box without
//! one. `modules/credentials.nix` declares them, makes the box's own on
//! first boot (or adopts the plaintext file an older box kept) and hands
//! each to the services that read it with `LoadCredentialEncrypted=`, so the
//! plaintext exists only in that service's credential directory, which is
//! memory that systemd removes when the service stops.
//!
//! What this daemon adds is the part a person sees: the list on the
//! Credentials pane and `losos credentials`, and making a new value for a
//! credential that can take one. A value never leaves through the API. The
//! list says what a credential is for, what reads it, what it is sealed to
//! and when it last changed, and the only write is "make a new one", whose
//! value is made here and sealed straight away.
//!
//! The registry is `/etc/losos/credentials.json`, rendered from the Nix
//! declarations (`$LOSOS_CREDENTIALS_FILE`). It is the whole list: a name
//! that is not in it is not a credential this box has, whatever files lie in
//! the store.
//!
//! Sealing goes through [`Seal`], so the bookkeeping is tested here without
//! a TPM; the real one runs `systemd-creds`, the value on stdin, never in an
//! argument vector.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

/// Where the registry is when `$LOSOS_CREDENTIALS_FILE` is unset.
pub const DEFAULT_REGISTRY: &str = "/etc/losos/credentials.json";

/// The largest value a credential takes. systemd's own limit is 1 MiB; a
/// bucket's settings are a few hundred bytes.
pub const MAX_VALUE: usize = 64 * 1024;

/// What a credential is sealed to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Key {
    /// The box's TPM, bound to no PCRs.
    Tpm2,
    /// `/var/lib/systemd/credential.secret`, root only, on a box without one.
    Host,
}

impl Key {
    /// `systemd-creds encrypt --with-key=` takes this.
    #[must_use]
    pub fn as_arg(self) -> &'static str {
        match self {
            Key::Tpm2 => "tpm2",
            Key::Host => "host",
        }
    }
}

/// How the box makes a value of its own, as the module's shell does; a new
/// value must have the shape the service expects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Format {
    /// 32 random bytes as 64 lowercase hex characters.
    #[serde(rename = "hex-32")]
    Hex32,
    /// 64 random bytes as 128 lowercase hex characters.
    #[serde(rename = "hex-64")]
    Hex64,
    /// 24 random bytes in standard base64.
    #[serde(rename = "base64-24")]
    Base64of24,
    /// An RSA private key in PEM, which only the boot unit makes.
    #[serde(rename = "rsa-2048")]
    Rsa2048,
}

impl Format {
    /// A new value of this shape.
    pub fn make(self) -> anyhow::Result<Vec<u8>> {
        use base64::Engine as _;
        Ok(match self {
            Format::Hex32 => crate::io_backend::hex(&crate::io_backend::random_bytes::<32>()?),
            Format::Hex64 => crate::io_backend::hex(&crate::io_backend::random_bytes::<64>()?),
            Format::Base64of24 => base64::engine::general_purpose::STANDARD
                .encode(crate::io_backend::random_bytes::<24>()?),
            Format::Rsa2048 => anyhow::bail!("an RSA key is made at boot only"),
        }
        .into_bytes())
    }
}

/// One credential, as `modules/credentials.nix` declares it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub name: String,
    /// What it belongs to, for grouping: `login`, `mail`, `admin`, ...
    pub app: String,
    /// One English sentence, for a name the page has no words of its own for.
    pub description: String,
    /// How the box makes it; `None` for a value the owner gives.
    pub format: Option<Format>,
    /// The units that read it.
    #[serde(default)]
    pub services: Vec<String>,
    /// The units restarted after a new value.
    #[serde(default)]
    pub restart: Vec<String>,
    /// Whether a person may make a new value.
    #[serde(default)]
    pub rotate: bool,
    /// The pane where the owner sets it, for one they give.
    #[serde(default)]
    pub pane: Option<String>,
}

/// The registry file.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Registry {
    /// The directory of sealed files, `/etc/credstore.encrypted`.
    pub store: PathBuf,
    /// Where the boot unit leaves its notes, on `/run`.
    pub runtime_dir: PathBuf,
    pub key: Key,
    pub entries: Vec<Entry>,
}

/// One row of the list. Everything about a credential but its value.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub name: String,
    pub app: String,
    pub description: String,
    /// `box` for one the box makes, `owner` for one a person gives.
    pub source: &'static str,
    /// Whether the sealed file is there.
    pub set: bool,
    /// When it last changed, Unix seconds.
    pub updated: Option<u64>,
    pub sealed_with: Key,
    pub services: Vec<String>,
    pub rotate: bool,
    pub pane: Option<String>,
    /// The old file would not open at boot, so the box made a new value.
    pub remade: bool,
    /// The box took the value over from a plaintext file at boot.
    pub adopted: bool,
}

/// A request the store refuses, in words the page shows as they are.
#[derive(Debug, Clone, PartialEq)]
pub enum Refused {
    NotFound,
    NotRotatable,
    TooLarge,
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Refused::NotFound => "this box has no credential by that name",
            Refused::NotRotatable => "this credential cannot be replaced from here",
            Refused::TooLarge => "the value is larger than a credential takes",
        })
    }
}

impl std::error::Error for Refused {}

/// Sealing, opening and restarting, behind one seam.
pub trait Seal: Send + Sync {
    /// Seal `value` under `name` into `out`, replacing it atomically.
    fn seal(&self, name: &str, key: Key, value: &[u8], out: &Path) -> anyhow::Result<()>;
    /// Open the file at `path`, sealed under `name`.
    fn open(&self, name: &str, path: &Path) -> anyhow::Result<Vec<u8>>;
    /// Restart the units that read a credential that changed.
    fn restart(&self, units: &[String]) -> anyhow::Result<()>;
}

/// The real one: `systemd-creds` and `systemctl`.
pub struct SystemdCreds;

impl Seal for SystemdCreds {
    fn seal(&self, name: &str, key: Key, value: &[u8], out: &Path) -> anyhow::Result<()> {
        let tmp = out.with_extension("new");
        let _ = std::fs::remove_file(&tmp);
        let mut child = Command::new("systemd-creds")
            .arg("--no-ask-password")
            .arg(format!("--with-key={}", key.as_arg()))
            // No PCR binding: the box updates its firmware and kernel with
            // nobody there to unseal by hand. modules/credentials.nix agrees.
            .arg("--tpm2-pcrs=")
            .arg(format!("--name={name}"))
            .arg("encrypt")
            .arg("-")
            .arg(&tmp)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| anyhow::anyhow!("running systemd-creds: {e}"))?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(value)?;
        }
        let output = child.wait_with_output()?;
        if !output.status.success() {
            let _ = std::fs::remove_file(&tmp);
            anyhow::bail!(
                "systemd-creds encrypt exited {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        // systemd-creds writes its output 0644. The store is 0700, but a
        // sealed secret is the owner's alone, as the boot unit leaves its own.
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))
                .map_err(|e| anyhow::anyhow!("making {} owner-only: {e}", tmp.display()))?;
        }
        std::fs::rename(&tmp, out)
            .map_err(|e| anyhow::anyhow!("putting {} in place: {e}", out.display()))?;
        Ok(())
    }

    fn open(&self, name: &str, path: &Path) -> anyhow::Result<Vec<u8>> {
        let output = Command::new("systemd-creds")
            .arg("--no-ask-password")
            .arg(format!("--name={name}"))
            .arg("decrypt")
            .arg(path)
            .arg("-")
            .stdin(Stdio::null())
            .output()
            .map_err(|e| anyhow::anyhow!("running systemd-creds: {e}"))?;
        if !output.status.success() {
            anyhow::bail!(
                "systemd-creds decrypt exited {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Ok(output.stdout)
    }

    fn restart(&self, units: &[String]) -> anyhow::Result<()> {
        if units.is_empty() {
            return Ok(());
        }
        // --no-block: a consumer that takes its time to stop (LosOS Login,
        // the mail server) must not hold this request open, and lososd
        // must never wait on a restart of itself.
        let status = Command::new("systemctl")
            .arg("--no-block")
            .arg("try-restart")
            .arg("--")
            .args(units.iter().map(|u| unit_name(u)))
            .status()
            .map_err(|e| anyhow::anyhow!("running systemctl: {e}"))?;
        anyhow::ensure!(status.success(), "systemctl try-restart exited {status}");
        Ok(())
    }
}

/// `authelia-losos` -> `authelia-losos.service`; a name with a suffix stays.
fn unit_name(unit: &str) -> String {
    if unit.contains('.') {
        unit.to_string()
    } else {
        format!("{unit}.service")
    }
}

/// The store, read from the registry each time it is asked, so a rebuild
/// that adds a credential needs no restart of this daemon to show it.
pub struct Credentials {
    registry: PathBuf,
    seal: Box<dyn Seal>,
}

impl Credentials {
    pub fn new(registry: PathBuf, seal: Box<dyn Seal>) -> Self {
        Credentials { registry, seal }
    }

    pub fn from_env() -> Self {
        let registry = std::env::var("LOSOS_CREDENTIALS_FILE")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .map_or_else(|| PathBuf::from(DEFAULT_REGISTRY), PathBuf::from);
        Credentials::new(registry, Box::new(SystemdCreds))
    }

    /// The registry, or `None` on a build without one.
    pub fn registry(&self) -> anyhow::Result<Option<Registry>> {
        match std::fs::read(&self.registry) {
            Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes).map_err(|e| {
                anyhow::anyhow!("reading {}: {e}", self.registry.display())
            })?)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(anyhow::anyhow!("reading {}: {e}", self.registry.display())),
        }
    }

    /// The registry, with the key the boot unit actually sealed with: a
    /// machine declared to have a TPM that has none seals to the host key.
    fn load(&self) -> anyhow::Result<Option<Registry>> {
        let Some(mut reg) = self.registry()? else {
            return Ok(None);
        };
        if let Ok(used) = std::fs::read_to_string(reg.runtime_dir.join("key")) {
            match used.trim() {
                "tpm2" => reg.key = Key::Tpm2,
                "host" => reg.key = Key::Host,
                _ => {}
            }
        }
        Ok(Some(reg))
    }

    fn require(&self) -> anyhow::Result<Registry> {
        self.load()?
            .ok_or_else(|| anyhow::anyhow!("this build has no credential store"))
    }

    /// Every credential the box has, in the registry's order.
    pub fn list(&self) -> anyhow::Result<Vec<Status>> {
        let Some(reg) = self.load()? else {
            return Ok(Vec::new());
        };
        Ok(reg.entries.iter().map(|e| status(&reg, e)).collect())
    }

    /// The key every credential here is sealed to.
    pub fn key(&self) -> anyhow::Result<Option<Key>> {
        Ok(self.load()?.map(|r| r.key))
    }

    /// Make a new value for `name`, seal it and restart what reads it.
    pub fn rotate(&self, name: &str) -> anyhow::Result<Result<Status, Refused>> {
        let reg = self.require()?;
        let Some(entry) = reg.entries.iter().find(|e| e.name == name) else {
            return Ok(Err(Refused::NotFound));
        };
        let (Some(format), true) = (entry.format, entry.rotate) else {
            return Ok(Err(Refused::NotRotatable));
        };
        let value = format.make()?;
        self.seal
            .seal(&entry.name, reg.key, &value, &reg.store.join(&entry.name))?;
        // The boot unit's notes describe the old value.
        for note in ["remade", "adopted"] {
            let _ = std::fs::remove_file(reg.runtime_dir.join(note).join(&entry.name));
        }
        let units = if entry.restart.is_empty() {
            &entry.services
        } else {
            &entry.restart
        };
        self.seal.restart(units)?;
        Ok(Ok(status(&reg, entry)))
    }

    /// Seal a value a person gave for `name`. For the panes that own one
    /// (Backups), which check its shape first; nothing here reads it back
    /// out through the API.
    pub fn put(&self, name: &str, value: &[u8]) -> anyhow::Result<Result<(), Refused>> {
        let reg = self.require()?;
        let Some(entry) = reg.entries.iter().find(|e| e.name == name) else {
            return Ok(Err(Refused::NotFound));
        };
        if value.len() > MAX_VALUE {
            return Ok(Err(Refused::TooLarge));
        }
        self.seal
            .seal(&entry.name, reg.key, value, &reg.store.join(&entry.name))?;
        Ok(Ok(()))
    }

    /// Open `name` for this daemon's own use, `None` if it is not set.
    pub fn get(&self, name: &str) -> anyhow::Result<Option<Vec<u8>>> {
        let reg = self.require()?;
        anyhow::ensure!(
            reg.entries.iter().any(|e| e.name == name),
            "{name} is not a credential of this box"
        );
        let path = reg.store.join(name);
        if !path.exists() {
            return Ok(None);
        }
        self.seal.open(name, &path).map(Some)
    }

    /// Forget `name`: the owner took the setting away.
    pub fn remove(&self, name: &str) -> anyhow::Result<()> {
        let reg = self.require()?;
        anyhow::ensure!(
            reg.entries.iter().any(|e| e.name == name),
            "{name} is not a credential of this box"
        );
        match std::fs::remove_file(reg.store.join(name)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(anyhow::anyhow!("removing {name}: {e}")),
        }
    }
}

fn status(reg: &Registry, e: &Entry) -> Status {
    let meta = std::fs::metadata(reg.store.join(&e.name)).ok();
    let updated = meta
        .as_ref()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs());
    let noted = |note: &str| reg.runtime_dir.join(note).join(&e.name).exists();
    Status {
        name: e.name.clone(),
        app: e.app.clone(),
        description: e.description.clone(),
        source: if e.format.is_some() { "box" } else { "owner" },
        set: meta.is_some(),
        updated,
        sealed_with: reg.key,
        services: e.services.clone(),
        rotate: e.rotate && e.format.is_some(),
        pane: e.pane.clone(),
        remade: noted("remade"),
        adopted: noted("adopted"),
    }
}

/// A seal for tests: the "sealed" file is the value behind a marker, so a
/// test can see that a value went in without a TPM.
#[derive(Default)]
pub struct FakeSeal {
    pub restarted: std::sync::Mutex<Vec<String>>,
}

impl Seal for FakeSeal {
    fn seal(&self, name: &str, key: Key, value: &[u8], out: &Path) -> anyhow::Result<()> {
        let mut sealed = format!("fake-sealed {name} {}\n", key.as_arg()).into_bytes();
        sealed.extend_from_slice(value);
        std::fs::write(out, sealed)?;
        Ok(())
    }

    fn open(&self, name: &str, path: &Path) -> anyhow::Result<Vec<u8>> {
        let bytes = std::fs::read(path)?;
        let split = bytes
            .iter()
            .position(|b| *b == b'\n')
            .ok_or_else(|| anyhow::anyhow!("not sealed"))?;
        let header = String::from_utf8_lossy(&bytes[..split]);
        anyhow::ensure!(
            header.split(' ').nth(1) == Some(name),
            "sealed under another name"
        );
        Ok(bytes[split + 1..].to_vec())
    }

    fn restart(&self, units: &[String]) -> anyhow::Result<()> {
        self.restarted.lock().unwrap().extend(units.iter().cloned());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    struct Shared(Arc<FakeSeal>);

    impl Seal for Shared {
        fn seal(&self, name: &str, key: Key, value: &[u8], out: &Path) -> anyhow::Result<()> {
            self.0.seal(name, key, value, out)
        }
        fn open(&self, name: &str, path: &Path) -> anyhow::Result<Vec<u8>> {
            self.0.open(name, path)
        }
        fn restart(&self, units: &[String]) -> anyhow::Result<()> {
            self.0.restart(units)
        }
    }

    fn fixture() -> (tempfile::TempDir, Credentials, Arc<FakeSeal>) {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("credstore.encrypted");
        let run = dir.path().join("run");
        std::fs::create_dir_all(&store).unwrap();
        std::fs::create_dir_all(run.join("adopted")).unwrap();
        let registry = serde_json::json!({
            "store": store,
            "runtimeDir": run,
            "key": "host",
            "entries": [
                {
                    "name": "authelia-session",
                    "app": "login",
                    "description": "Encrypts the sessions of LosOS Login.",
                    "format": "hex-64",
                    "services": ["authelia-losos"],
                    "restart": [],
                    "rotate": true,
                    "pane": null
                },
                {
                    "name": "authelia-storage",
                    "app": "login",
                    "description": "Encrypts what LosOS Login stores.",
                    "format": "hex-64",
                    "services": ["authelia-losos"],
                    "rotate": false
                },
                {
                    "name": "backup-bucket",
                    "app": "backup",
                    "description": "The bucket's keys.",
                    "format": null,
                    "pane": "/settings/backup"
                }
            ]
        });
        let path = dir.path().join("credentials.json");
        std::fs::write(&path, registry.to_string()).unwrap();
        std::fs::write(store.join("authelia-session"), b"old").unwrap();
        std::fs::write(run.join("adopted/authelia-session"), b"").unwrap();
        let fake = Arc::new(FakeSeal::default());
        let creds = Credentials::new(path, Box::new(Shared(fake.clone())));
        (dir, creds, fake)
    }

    #[test]
    fn the_list_says_everything_but_the_value() {
        let (_dir, creds, _) = fixture();
        let list = creds.list().unwrap();
        assert_eq!(list.len(), 3);
        let session = &list[0];
        assert!(session.set);
        assert!(session.adopted);
        assert_eq!(session.source, "box");
        assert_eq!(session.sealed_with, Key::Host);
        assert!(!list[1].set);
        assert_eq!(list[2].source, "owner");
        assert!(
            !list[2].rotate,
            "a value the owner gives is never made here"
        );
        let text = serde_json::to_string(&list).unwrap();
        assert!(!text.contains("old"), "the sealed bytes leaked: {text}");
    }

    #[test]
    fn a_new_value_is_sealed_under_its_name_and_its_readers_restart() {
        let (_dir, creds, fake) = fixture();
        let row = creds.rotate("authelia-session").unwrap().unwrap();
        assert!(!row.adopted, "the note about the old value is gone");
        let value = creds.get("authelia-session").unwrap().unwrap();
        assert_eq!(value.len(), 128);
        assert!(value.iter().all(u8::is_ascii_hexdigit));
        assert_eq!(*fake.restarted.lock().unwrap(), vec!["authelia-losos"]);
        let again = creds.rotate("authelia-session").unwrap().unwrap();
        assert!(again.set);
        assert_ne!(creds.get("authelia-session").unwrap().unwrap(), value);
    }

    #[test]
    fn some_credentials_cannot_be_replaced_from_here() {
        let (_dir, creds, fake) = fixture();
        assert_eq!(
            creds.rotate("authelia-storage").unwrap(),
            Err(Refused::NotRotatable)
        );
        assert_eq!(
            creds.rotate("backup-bucket").unwrap(),
            Err(Refused::NotRotatable)
        );
        assert_eq!(creds.rotate("nope").unwrap(), Err(Refused::NotFound));
        assert!(fake.restarted.lock().unwrap().is_empty());
    }

    #[test]
    fn a_value_the_owner_gives_round_trips_and_goes() {
        let (_dir, creds, _) = fixture();
        assert_eq!(creds.get("backup-bucket").unwrap(), None);
        creds
            .put("backup-bucket", b"AWS_SECRET_ACCESS_KEY=s\n")
            .unwrap()
            .unwrap();
        assert_eq!(
            creds.get("backup-bucket").unwrap().unwrap(),
            b"AWS_SECRET_ACCESS_KEY=s\n"
        );
        assert_eq!(
            creds
                .put("backup-bucket", &vec![b'a'; MAX_VALUE + 1])
                .unwrap(),
            Err(Refused::TooLarge)
        );
        creds.remove("backup-bucket").unwrap();
        assert_eq!(creds.get("backup-bucket").unwrap(), None);
        assert!(creds.get("not-ours").is_err());
    }

    #[test]
    fn the_three_shapes() {
        assert_eq!(Format::Hex32.make().unwrap().len(), 64);
        assert_eq!(Format::Hex64.make().unwrap().len(), 128);
        assert_eq!(Format::Base64of24.make().unwrap().len(), 32);
        assert_eq!(unit_name("authelia-losos"), "authelia-losos.service");
        assert_eq!(unit_name("k3s.service"), "k3s.service");
    }

    #[test]
    fn the_key_the_boot_unit_used_wins() {
        let (dir, creds, _) = fixture();
        assert_eq!(creds.key().unwrap(), Some(Key::Host));
        std::fs::write(dir.path().join("run/key"), "tpm2\n").unwrap();
        assert_eq!(creds.key().unwrap(), Some(Key::Tpm2));
        assert_eq!(creds.list().unwrap()[0].sealed_with, Key::Tpm2);
    }

    #[test]
    fn no_registry_is_an_empty_list() {
        let dir = tempfile::tempdir().unwrap();
        let creds = Credentials::new(dir.path().join("none.json"), Box::new(FakeSeal::default()));
        assert!(creds.list().unwrap().is_empty());
        assert!(creds.rotate("x").is_err());
    }
}
