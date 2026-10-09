//! Backups to an S3 bucket the owner names, and pulling them back.
//!
//! # What goes where
//!
//! The copy is made by `restic`, run by the `losos-backup` and
//! `losos-restore` scripts in `modules/backup.nix` as `systemd-run`
//! transient units, the same way a rebuild runs (`crate::supervisor`). lososd
//! never reads the owner's files: it is sandboxed away from both data
//! domains (`ProtectHome=true`), and a transient unit is started by PID 1
//! outside that sandbox. What lososd owns is the decision: which bucket,
//! whether a job may start, and what its outcome means.
//!
//! # Encrypted on the box, opened by the recovery code
//!
//! restic encrypts every chunk before it leaves the box (AES-256 in counter
//! mode with Poly1305), so the bucket only ever holds ciphertext and the
//! bucket's operator learns sizes and times, nothing else. The repository
//! password is the box's recovery code (`crate::recovery`), the UUID the
//! first-run wizard told the owner to write down. Two reasons, both about the
//! day the backup is needed:
//!
//!   * the box that made the backup may be gone, so the key cannot live only
//!     on it, and the recovery code is the one secret the owner already keeps
//!     somewhere else;
//!   * a second passphrase is a second thing to lose, and a backup whose key
//!     is lost is no backup.
//!
//! A restore brings the old code back with the data, so the restored box
//! keeps the identity it had (its box UUID is derived from the code,
//! `crate::boxid`) and its next backup lands in the same repository.
//!
//! # The fscrypt domain
//!
//! `/home/shared/data` is an fscrypt policy (`modules/fscrypt.nix`). Linux
//! hands out no ciphertext for a locked fscrypt file: `open()` fails with
//! `ENOKEY`. So the backup script unlocks the policy for the length of the
//! copy when sharing has left it locked, locks it again afterwards, and
//! restic encrypts what it read before upload. On restore the files are
//! written into the new box's own policy, under its own key. At no point is
//! the domain stored anywhere in plain text.
//!
//! # Glacier
//!
//! On AWS the owner can pick a storage class ([`StorageClass`]). restic
//! stores only the data packs in it; its own metadata (the config, keys,
//! index, snapshots and tree packs) stays in S3 Standard whatever the class,
//! so listing, checking a code and tidying old backups stay instant. Glacier
//! Instant Retrieval reads back like Standard. Glacier Flexible Retrieval and
//! Deep Archive objects must be thawed before they can be read, so a restore
//! from them asks AWS to thaw each pack and waits (restic's `s3-restore`
//! feature): hours, not minutes, and AWS charges for the retrieval. Pruning
//! there deletes only packs nothing uses any more and never repacks, which
//! would need a thaw too.
//!
//! # Secrets
//!
//! The bucket's secret key is written to a 0600 file under `/var/secrets`
//! and handed to the unit as an `EnvironmentFile=`; it is never an argument
//! vector, never in `overrides.nix` (which is committed to LosOS Git), and
//! never sent back to the browser, which sees only whether one is set.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Longest endpoint URL accepted.
const MAX_URL: usize = 200;
/// Longest bucket prefix accepted.
const MAX_PREFIX: usize = 100;
/// Longest access key id or secret accepted. AWS uses 20 and 40; other
/// providers use more, none this many.
const MAX_KEY: usize = 128;

/// How AWS stores the data. Only AWS knows the Glacier classes; every other
/// provider gets [`StorageClass::Standard`], which sends no class at all.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StorageClass {
    /// Whatever the bucket's default is: S3 Standard on AWS.
    #[default]
    Standard,
    /// `GLACIER_IR`: cheaper to keep, read back at once, billed per read.
    GlacierInstant,
    /// `GLACIER`: Glacier Flexible Retrieval, thawed in hours.
    Glacier,
    /// `DEEP_ARCHIVE`: the cheapest, thawed in up to two days.
    DeepArchive,
}

impl StorageClass {
    /// The name AWS and restic use, or `None` for the bucket's default.
    #[must_use]
    pub fn aws_name(self) -> Option<&'static str> {
        match self {
            Self::Standard => None,
            Self::GlacierInstant => Some("GLACIER_IR"),
            Self::Glacier => Some("GLACIER"),
            Self::DeepArchive => Some("DEEP_ARCHIVE"),
        }
    }

    /// Whether a read has to wait for AWS to thaw the data first.
    #[must_use]
    pub fn needs_thaw(self) -> bool {
        matches!(self, Self::Glacier | Self::DeepArchive)
    }
}

/// Where backups go. Serialised whole only into the 0600 file; the browser
/// gets [`Target::public`].
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Target {
    /// `https://s3.example.org`, no path. Plain `http://` only for a host on
    /// the owner's own network ([`lan_host`]), where a NAS often has no
    /// certificate; the data is encrypted either way, the bucket keys are not.
    pub endpoint: String,
    pub bucket: String,
    /// A folder inside the bucket, so one bucket can hold several boxes.
    #[serde(default)]
    pub prefix: String,
    /// Some providers want one (AWS); most accept anything.
    #[serde(default)]
    pub region: String,
    pub access_key_id: String,
    pub secret_access_key: String,
    /// The storage class of the data packs; targets saved before there was a
    /// choice read as Standard.
    #[serde(default)]
    pub storage_class: StorageClass,
}

impl fmt::Debug for Target {
    /// Hand-written so the secret can never reach a log line.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Target")
            .field("endpoint", &self.endpoint)
            .field("bucket", &self.bucket)
            .field("prefix", &self.prefix)
            .field("region", &self.region)
            .field("access_key_id", &self.access_key_id)
            .field("storage_class", &self.storage_class)
            .finish_non_exhaustive()
    }
}

/// What the owner sends to set the target. The secret may be left out to keep
/// the one already stored, so the form never has to show it.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TargetInput {
    pub endpoint: String,
    pub bucket: String,
    #[serde(default)]
    pub prefix: String,
    #[serde(default)]
    pub region: String,
    pub access_key_id: String,
    #[serde(default)]
    pub secret_access_key: Option<String>,
    #[serde(default)]
    pub storage_class: StorageClass,
}

/// A target the owner can fix: the sentence names the field. Answered as a
/// 400 with the sentence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invalid(pub &'static str);

impl fmt::Display for Invalid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for Invalid {}

/// Something that cannot happen *now*: a job already running, an erase under
/// way. Answered as a 409 with the sentence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Busy(pub &'static str);

impl fmt::Display for Busy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for Busy {}

impl TargetInput {
    /// Check every field and merge in the stored secret when none was sent.
    ///
    /// # Errors
    /// [`Invalid`] naming the first field that is wrong.
    pub fn into_target(self, stored: Option<&Target>) -> Result<Target, Invalid> {
        let endpoint = self.endpoint.trim().trim_end_matches('/').to_string();
        if !valid_endpoint(&endpoint) {
            return Err(Invalid(
                "the address must look like https://s3.example.org (http:// only for a host on your own network)",
            ));
        }
        let bucket = self.bucket.trim().to_string();
        if !valid_bucket(&bucket) {
            return Err(Invalid(
                "a bucket name has 3 to 63 lowercase letters, digits, dots and hyphens",
            ));
        }
        let prefix = self.prefix.trim().trim_matches('/').to_string();
        if !valid_prefix(&prefix) {
            return Err(Invalid(
                "the folder may hold letters, digits, dots, hyphens, underscores and slashes",
            ));
        }
        let region = self.region.trim().to_string();
        if !region.is_empty() && !valid_region(&region) {
            return Err(Invalid(
                "a region has only lowercase letters, digits and hyphens, such as eu-central-1",
            ));
        }
        let access_key_id = self.access_key_id.trim().to_string();
        if !valid_key(&access_key_id) {
            return Err(Invalid(
                "the access key is missing or has characters no provider uses",
            ));
        }
        let secret_access_key = match self.secret_access_key.map(|s| s.trim().to_string()) {
            Some(s) if !s.is_empty() => s,
            _ => match stored {
                Some(t) => t.secret_access_key.clone(),
                None => return Err(Invalid("the secret key is missing")),
            },
        };
        if !valid_key(&secret_access_key) {
            return Err(Invalid("the secret key has characters no provider uses"));
        }
        if self.storage_class != StorageClass::Standard && !is_aws(&endpoint) {
            return Err(Invalid(
                "the Glacier storage classes are AWS's; other providers take Standard",
            ));
        }
        Ok(Target {
            endpoint,
            bucket,
            prefix,
            region,
            access_key_id,
            secret_access_key,
            storage_class: self.storage_class,
        })
    }
}

impl Target {
    /// restic's name for the repository: `s3:<endpoint>/<bucket>[/<prefix>]`.
    #[must_use]
    pub fn repository(&self) -> String {
        if self.prefix.is_empty() {
            format!("s3:{}/{}", self.endpoint, self.bucket)
        } else {
            format!("s3:{}/{}/{}", self.endpoint, self.bucket, self.prefix)
        }
    }

    /// The `EnvironmentFile=` the units read. Every value was checked to hold
    /// none of the characters systemd's parser treats specially (quotes,
    /// backslashes, whitespace, `$`), so no quoting is needed and none can be
    /// got wrong.
    #[must_use]
    pub fn env_file(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("RESTIC_REPOSITORY={}\n", self.repository()));
        out.push_str(&format!("AWS_ACCESS_KEY_ID={}\n", self.access_key_id));
        out.push_str(&format!(
            "AWS_SECRET_ACCESS_KEY={}\n",
            self.secret_access_key
        ));
        if !self.region.is_empty() {
            out.push_str(&format!("AWS_DEFAULT_REGION={}\n", self.region));
        }
        // The scripts turn these into restic's `-o s3.*` options.
        if let Some(class) = self.storage_class.aws_name() {
            out.push_str(&format!("LOSOS_S3_STORAGE_CLASS={class}\n"));
        }
        if self.storage_class.needs_thaw() {
            out.push_str("LOSOS_S3_THAW=1\n");
        }
        out
    }

    /// What the browser may see: everything but the secret, and whether one
    /// is set.
    #[must_use]
    pub fn public(&self) -> serde_json::Value {
        serde_json::json!({
            "endpoint": self.endpoint,
            "bucket": self.bucket,
            "prefix": self.prefix,
            "region": self.region,
            "accessKeyId": self.access_key_id,
            "hasSecret": !self.secret_access_key.is_empty(),
            "storageClass": self.storage_class,
        })
    }
}

/// Whether the endpoint is AWS's own S3, the only place the Glacier classes
/// exist: `https://s3.amazonaws.com` or a regional `https://s3.<region>.amazonaws.com`
/// (and the dual-stack and FIPS forms, which share the suffix).
fn is_aws(endpoint: &str) -> bool {
    endpoint
        .strip_prefix("https://")
        .and_then(|host| host.split(['/', ':']).next())
        .is_some_and(|host| host.starts_with("s3") && host.ends_with(".amazonaws.com"))
}

fn valid_endpoint(url: &str) -> bool {
    if url.len() > MAX_URL {
        return false;
    }
    let (rest, plain) = if let Some(r) = url.strip_prefix("https://") {
        (r, false)
    } else if let Some(r) = url.strip_prefix("http://") {
        (r, true)
    } else {
        return false;
    };
    if rest.is_empty() || rest.contains('/') || rest.contains('@') {
        return false;
    }
    if !rest
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b':' | b'[' | b']'))
    {
        return false;
    }
    !plain || lan_host(rest)
}

/// Whether `host[:port]` is on the owner's own network: a private or
/// loopback IPv4 address, a unique-local or link-local IPv6 one, or a
/// `.local` / `.lan` / `.home.arpa` name.
#[must_use]
pub fn lan_host(host_port: &str) -> bool {
    let host = if let Some(v6) = host_port.strip_prefix('[') {
        match v6.split_once(']') {
            Some((h, _)) => h,
            None => return false,
        }
    } else {
        host_port.split(':').next().unwrap_or("")
    };
    if let Ok(ip) = host.parse::<std::net::Ipv4Addr>() {
        return ip.is_private() || ip.is_loopback() || ip.is_link_local();
    }
    if let Ok(ip) = host.parse::<std::net::Ipv6Addr>() {
        let first = ip.segments()[0];
        return ip.is_loopback() || (first & 0xfe00) == 0xfc00 || (first & 0xffc0) == 0xfe80;
    }
    let host = host.to_ascii_lowercase();
    ["local", "lan", "home.arpa"]
        .iter()
        .any(|zone| host.ends_with(&format!(".{zone}")))
}

fn valid_bucket(b: &str) -> bool {
    (3..=63).contains(&b.len())
        && b.bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'.' || c == b'-')
        && b.as_bytes()[0].is_ascii_alphanumeric()
}

fn valid_prefix(p: &str) -> bool {
    p.len() <= MAX_PREFIX
        && !p.contains("//")
        && !p.split('/').any(|seg| seg == "." || seg == "..")
        && p.bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'-' | b'_' | b'/'))
}

fn valid_region(r: &str) -> bool {
    r.len() <= 40
        && r.bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
}

fn valid_key(k: &str) -> bool {
    !k.is_empty()
        && k.len() <= MAX_KEY
        && k.bytes().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, b'/' | b'+' | b'=' | b'.' | b'_' | b'-')
        })
}

/// Which script a job runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Backup,
    Restore,
}

impl Kind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Backup => "backup",
            Kind::Restore => "restore",
        }
    }

    /// The transient unit a job runs as: `losos-backup-<job>` or
    /// `losos-restore-<job>`. Unique per job, so a poll can never read the
    /// outcome of an earlier run, which a fixed unit name would allow in the
    /// moment between `systemctl start --no-block` and the unit activating.
    #[must_use]
    pub fn unit(self, job: &str) -> String {
        format!("losos-{}-{job}", self.as_str())
    }
}

/// Where a job is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobState {
    Running,
    Done,
    Failed,
    Cancelled,
}

/// The last or current backup or restore. Persisted in `state.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub kind: Kind,
    pub job: String,
    pub state: JobState,
    pub started_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<u64>,
    /// The last line of the script's log on a failure, for the owner.
    #[serde(default)]
    pub message: String,
    /// Polls in a row that could not read the unit (`crate::supervisor`'s
    /// grace, counted here so a restart of lososd does not reset it).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub unknown_polls: u32,
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

impl Job {
    #[must_use]
    pub fn running(&self) -> bool {
        self.state == JobState::Running
    }
}

/// What the backup script writes after a successful run, read back for the
/// pane: when, which snapshot, how much.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    /// Unix seconds.
    pub time: u64,
    pub snapshot: String,
    #[serde(default)]
    pub files: u64,
    #[serde(default)]
    pub bytes: u64,
}

/// Read the report file. A missing or unreadable one is "no backup yet",
/// never an error: the pane must load on a box that has never backed up.
#[must_use]
pub fn parse_report(text: &str) -> Option<Report> {
    let r: Report = serde_json::from_str(text).ok()?;
    let ok = !r.snapshot.is_empty()
        && r.snapshot.len() <= 64
        && r.snapshot.bytes().all(|b| b.is_ascii_hexdigit());
    ok.then_some(r)
}

/// The recovery code the owner typed, normalised, or `None` when it is not
/// one. The restore unit reads it from a file as the repository password.
#[must_use]
pub fn normalise_code(code: &str) -> Option<String> {
    let code = code.trim().to_ascii_lowercase();
    crate::recovery::is_well_formed(&code).then_some(code)
}

/// Turn a script's last log line into the sentence the owner sees. restic's
/// two common refusals get plain words; anything else is passed on, capped.
#[must_use]
pub fn explain_failure(kind: Kind, code: i32, tail: &str) -> String {
    let lower = tail.to_ascii_lowercase();
    if lower.contains("wrong password") || lower.contains("no key found") {
        return "the recovery code does not open this backup".to_string();
    }
    if lower.contains("access denied") || lower.contains("invalidaccesskeyid") {
        return "the bucket refused the keys".to_string();
    }
    if lower.contains("no such host") || lower.contains("connection refused") {
        return "the bucket's address could not be reached".to_string();
    }
    if lower.contains("nosuchbucket") || lower.contains("bucket does not exist") {
        return "the bucket does not exist".to_string();
    }
    if lower.contains("unable to open config file") || lower.contains("is there a repository") {
        return "there is no backup in this bucket yet".to_string();
    }
    let mut msg = format!("{} failed (exit {code})", kind.as_str());
    if !tail.is_empty() {
        msg.push_str(": ");
        msg.push_str(tail);
    }
    msg
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> TargetInput {
        TargetInput {
            endpoint: "https://s3.eu-central-1.amazonaws.com/".into(),
            bucket: "losos-backups".into(),
            prefix: "/kitchen-box/".into(),
            region: "eu-central-1".into(),
            access_key_id: "AKIAIOSFODNN7EXAMPLE".into(),
            secret_access_key: Some("wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".into()),
            storage_class: StorageClass::Standard,
        }
    }

    #[test]
    fn standard_sends_no_class_and_no_thaw() {
        let env = input().into_target(None).unwrap().env_file();
        assert!(!env.contains("LOSOS_S3_STORAGE_CLASS"), "{env}");
        assert!(!env.contains("LOSOS_S3_THAW"), "{env}");
    }

    #[test]
    fn a_glacier_class_reaches_restic_and_the_browser() {
        for (class, name, thaw) in [
            (StorageClass::GlacierInstant, "GLACIER_IR", false),
            (StorageClass::Glacier, "GLACIER", true),
            (StorageClass::DeepArchive, "DEEP_ARCHIVE", true),
        ] {
            let t = TargetInput {
                storage_class: class,
                ..input()
            }
            .into_target(None)
            .unwrap();
            let env = t.env_file();
            assert!(
                env.contains(&format!("LOSOS_S3_STORAGE_CLASS={name}\n")),
                "{env}"
            );
            assert_eq!(env.contains("LOSOS_S3_THAW=1\n"), thaw, "{env}");
            assert_eq!(
                t.public()["storageClass"],
                serde_json::to_value(class).unwrap()
            );
        }
    }

    #[test]
    fn glacier_only_on_aws() {
        for ok in [
            "https://s3.amazonaws.com",
            "https://s3.eu-central-1.amazonaws.com",
            "https://s3.dualstack.us-east-1.amazonaws.com",
        ] {
            let t = TargetInput {
                endpoint: ok.into(),
                storage_class: StorageClass::DeepArchive,
                ..input()
            };
            assert!(t.into_target(None).is_ok(), "{ok}");
        }
        for bad in [
            "https://minio.example.org:9000",
            "http://192.168.1.20:9000",
            "https://s3.amazonaws.com.attacker.example",
            "https://fakes3.example.org",
        ] {
            let t = TargetInput {
                endpoint: bad.into(),
                storage_class: StorageClass::Glacier,
                ..input()
            };
            assert!(t.into_target(None).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_target_saved_before_the_choice_reads_as_standard() {
        let old = r#"{"endpoint":"https://s3.example.org","bucket":"b-1","accessKeyId":"AKID","secretAccessKey":"s"}"#;
        let t: Target = serde_json::from_str(old).unwrap();
        assert_eq!(t.storage_class, StorageClass::Standard);
        let input: TargetInput = serde_json::from_str(
            r#"{"endpoint":"https://s3.amazonaws.com","bucket":"b-1","accessKeyId":"AKID","storageClass":"deepArchive"}"#,
        )
        .unwrap();
        assert_eq!(input.storage_class, StorageClass::DeepArchive);
    }

    #[test]
    fn a_target_is_trimmed_and_named_for_restic() {
        let t = input().into_target(None).unwrap();
        assert_eq!(t.endpoint, "https://s3.eu-central-1.amazonaws.com");
        assert_eq!(t.prefix, "kitchen-box");
        assert_eq!(
            t.repository(),
            "s3:https://s3.eu-central-1.amazonaws.com/losos-backups/kitchen-box"
        );
        let env = t.env_file();
        assert!(env.contains("AWS_SECRET_ACCESS_KEY=wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY\n"));
        assert!(env.contains("AWS_DEFAULT_REGION=eu-central-1\n"));
    }

    #[test]
    fn the_secret_never_reaches_the_browser_or_a_log() {
        let t = input().into_target(None).unwrap();
        let public = t.public().to_string();
        assert!(!public.contains("EXAMPLEKEY"));
        assert_eq!(t.public()["hasSecret"], true);
        assert!(!format!("{t:?}").contains("EXAMPLEKEY"));
    }

    #[test]
    fn leaving_the_secret_out_keeps_the_stored_one() {
        let stored = input().into_target(None).unwrap();
        let again = TargetInput {
            secret_access_key: None,
            bucket: "other-bucket".into(),
            ..input()
        }
        .into_target(Some(&stored))
        .unwrap();
        assert_eq!(again.secret_access_key, stored.secret_access_key);
        assert_eq!(again.bucket, "other-bucket");
        // With nothing stored, it is required.
        assert!(TargetInput {
            secret_access_key: None,
            ..input()
        }
        .into_target(None)
        .is_err());
    }

    #[test]
    fn plain_http_only_inside_the_owners_network() {
        for ok in [
            "http://192.168.1.20:9000",
            "http://10.0.0.5",
            "http://nas.local:9000",
            "http://[fd00::1]:9000",
            "https://minio.example.org:9000",
        ] {
            let t = TargetInput {
                endpoint: ok.into(),
                ..input()
            };
            assert!(t.into_target(None).is_ok(), "{ok}");
        }
        for bad in [
            "http://s3.amazonaws.com",
            "http://8.8.8.8",
            "ftp://nas.local",
            "https://user@host",
            "https://host/path",
            "https://",
            "s3.amazonaws.com",
        ] {
            let t = TargetInput {
                endpoint: bad.into(),
                ..input()
            };
            assert!(t.into_target(None).is_err(), "{bad}");
        }
    }

    #[test]
    fn nothing_that_could_break_the_environment_file_gets_in() {
        for bad in ["a b", "a\"b", "a\\b", "a$b", "a\nb", ""] {
            let t = TargetInput {
                access_key_id: bad.into(),
                ..input()
            };
            assert!(t.into_target(None).is_err(), "{bad:?}");
        }
        for bad in ["Upper", "ab", "a_b", "-ab"] {
            let t = TargetInput {
                bucket: bad.into(),
                ..input()
            };
            assert!(t.into_target(None).is_err(), "{bad:?}");
        }
        for bad in ["../up", "a/./b", "a//b", "a b"] {
            let t = TargetInput {
                prefix: bad.into(),
                ..input()
            };
            assert!(t.into_target(None).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn each_job_has_its_own_unit() {
        assert_eq!(Kind::Backup.unit("job-3"), "losos-backup-job-3");
        assert_eq!(Kind::Restore.unit("job-4"), "losos-restore-job-4");
    }

    #[test]
    fn a_report_needs_a_snapshot_id() {
        let r =
            parse_report(r#"{"time":1700000000,"snapshot":"4f2a9c1e","files":12,"bytes":3400}"#)
                .unwrap();
        assert_eq!(r.snapshot, "4f2a9c1e");
        assert!(parse_report(r#"{"time":1,"snapshot":""}"#).is_none());
        assert!(parse_report(r#"{"time":1,"snapshot":"../x"}"#).is_none());
        assert!(parse_report("not json").is_none());
    }

    #[test]
    fn a_typed_code_is_normalised() {
        assert_eq!(
            normalise_code(" 0F8FAD5B-D9CB-469F-A165-70867728950E \n").as_deref(),
            Some("0f8fad5b-d9cb-469f-a165-70867728950e")
        );
        assert!(normalise_code("not a code").is_none());
    }

    #[test]
    fn restics_refusals_are_said_plainly() {
        assert_eq!(
            explain_failure(Kind::Restore, 1, "Fatal: wrong password or no key found"),
            "the recovery code does not open this backup"
        );
        assert_eq!(
            explain_failure(Kind::Backup, 1, "Fatal: unable to open config file: Stat: The Access Key Id you provided does not exist in our records. (InvalidAccessKeyId)"),
            "the bucket refused the keys"
        );
        assert_eq!(
            explain_failure(Kind::Backup, 2, ""),
            "backup failed (exit 2)"
        );
    }
}
