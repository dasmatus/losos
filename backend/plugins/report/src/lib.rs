//! Problem reports as GitHub issues, for the box and for the installer.
//!
//! The box notices a service that failed, a program that crashed and one the
//! kernel killed for memory (the `problems` plugin); the installer notices an
//! install that stopped and memory or processor use that spiked while it ran
//! (`backend/src/install_report.rs`). Both write the report here, show the
//! whole text to the person first, and send it only once that person signs
//! in to GitHub and approves it.
//!
//! GitHub opens an issue only for a signed-in account, and a token in the
//! image would be anyone's who downloads it. So the person signs in with
//! GitHub's device flow ([`github::send`]): GitHub hands out a short code,
//! the page or the installer's screen shows it, and the person enters it at
//! github.com/login/device on a phone or another computer. The token that
//! comes back lives in memory for that one report. It belongs to a GitHub
//! App whose only permission is issues.
//!
//! The same problem on many machines should be one issue. Each report's
//! title ends with a [`fingerprint`] of what went wrong, numbers and device
//! names taken out, and a report whose fingerprint an open issue already
//! carries becomes a comment on that issue.
//!
//! A problem the handbook already explains is not reported at all: [`known`]
//! names the page instead.
//!
//! Ported from losos-project/desktop's `losos-report`, which files the
//! desktop's and its installer's problems the same way.

pub mod github;
pub mod install;
pub mod known;
pub mod problem;

pub use known::Known;

/// GitHub refuses an issue body over 65536 characters; this leaves room for
/// the note the person adds.
pub const BODY_LIMIT: usize = 60_000;

/// Where reports go and how to sign in to send them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    /// `owner/name`.
    pub repo: String,
    /// The GitHub App's client ID, which the device flow names it by. Not a
    /// secret: GitHub shows it on the sign-in page.
    pub client_id: String,
    /// Where GitHub signs people in, `https://github.com`.
    pub login: String,
    /// GitHub's REST API, `https://api.github.com`.
    pub api: String,
}

impl Target {
    /// From the four settings, when the first two say where to send. The
    /// last two default to GitHub's own and point at a stand-in in tests.
    #[must_use]
    pub fn from_settings(
        repo: Option<&str>,
        client_id: Option<&str>,
        login: Option<&str>,
        api: Option<&str>,
    ) -> Option<Target> {
        let set = |v: Option<&str>| {
            v.map(str::trim)
                .filter(|v| !v.is_empty())
                .map(str::to_owned)
        };
        let repo = set(repo).filter(|r| valid_repo(r))?;
        Some(Target {
            repo,
            client_id: set(client_id)?,
            login: set(login).unwrap_or_else(|| "https://github.com".to_owned()),
            api: set(api).unwrap_or_else(|| "https://api.github.com".to_owned()),
        })
    }
}

/// `owner/name` with GitHub's characters only.
#[must_use]
pub fn valid_repo(repo: &str) -> bool {
    let ok = |part: &str| {
        !part.is_empty()
            && part.len() <= 100
            && part
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
    };
    matches!(repo.split_once('/'), Some((owner, name)) if ok(owner) && ok(name) && !name.contains('/'))
}

/// What the machine and the image are, for the report's System table.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct System {
    /// The release and the architecture: `v0.6.1, x86_64`.
    pub release: String,
    pub firmware: String,
    pub machine: String,
    pub cpu: String,
    pub memory: String,
    pub kernel: String,
}

/// The files [`System::from_files`] reads, as their text.
#[derive(Debug, Clone, Default)]
pub struct SystemFiles {
    pub os_release: String,
    pub cpuinfo: String,
    pub meminfo: String,
    /// `/sys/class/dmi/id/sys_vendor` and `product_name`.
    pub vendor: String,
    pub product: String,
    pub kernel: String,
    /// Whether `/sys/firmware/efi` exists.
    pub efi: bool,
}

impl System {
    /// The table's rows from the files' text; `arch` is the build's.
    #[must_use]
    pub fn from_files(files: &SystemFiles, arch: &str) -> System {
        let machine = [files.vendor.trim(), files.product.trim()]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        let cpu = files
            .cpuinfo
            .lines()
            .find_map(|l| {
                let (key, value) = l.split_once(':')?;
                matches!(key.trim(), "model name" | "Model" | "Hardware")
                    .then(|| value.trim().to_owned())
            })
            .unwrap_or_default();
        let threads = files
            .cpuinfo
            .lines()
            .filter(|l| l.starts_with("processor"))
            .count();
        let memory = mem_total_kib(&files.meminfo)
            .map(|kib| format!("{:.1} GiB", kib as f64 / 1024.0 / 1024.0))
            .unwrap_or_default();
        System {
            release: match os_release_version(&files.os_release) {
                Some(version) => format!("{version}, {arch}"),
                None => arch.to_owned(),
            },
            firmware: if files.efi { "UEFI" } else { "BIOS" }.to_owned(),
            machine,
            cpu: if threads > 0 {
                format!("{cpu} ({threads} threads)")
            } else {
                cpu
            },
            memory,
            kernel: files.kernel.trim().to_owned(),
        }
    }

    /// The System table, in Markdown, with a row for each of `extra` after
    /// the machine's own. An empty value leaves its row out.
    #[must_use]
    pub fn table(&self, extra: &[(&str, &str)]) -> String {
        let mut table = "### System\n\n| | |\n| --- | --- |\n".to_owned();
        let rows = [
            ("LosOS", self.release.as_str()),
            ("Firmware", &self.firmware),
            ("Machine", &self.machine),
            ("CPU", &self.cpu),
            ("Memory", &self.memory),
            ("Kernel", &self.kernel),
        ];
        for (name, value) in rows.iter().chain(extra) {
            if !value.is_empty() {
                table.push_str(&format!("| {name} | {} |\n", value.replace('|', "\\|")));
            }
        }
        table
    }
}

/// `MemTotal` from `/proc/meminfo`'s text, in KiB.
#[must_use]
pub fn mem_total_kib(meminfo: &str) -> Option<u64> {
    meminfo_field(meminfo, "MemTotal")
}

/// One `Name:   123 kB` field of `/proc/meminfo`'s text, in KiB.
#[must_use]
pub fn meminfo_field(meminfo: &str, name: &str) -> Option<u64> {
    meminfo.lines().find_map(|l| {
        let rest = l.strip_prefix(name)?.strip_prefix(':')?;
        rest.trim().trim_end_matches("kB").trim().parse().ok()
    })
}

/// The release from an os-release file's text: `IMAGE_VERSION`, which
/// modules/branding.nix writes from nix/version.nix, else `VERSION_ID`.
#[must_use]
pub fn os_release_version(text: &str) -> Option<String> {
    let field = |key: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix(key)?.strip_prefix('='))
            .map(|v| v.trim().trim_matches('"').to_owned())
            .filter(|v| !v.is_empty())
    };
    field("IMAGE_VERSION").or_else(|| field("VERSION_ID"))
}

/// A report, as it is shown and as it is sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub title: String,
    pub body: String,
    pub fingerprint: String,
}

/// The failure message reduced to what stays the same from one machine to
/// the next: lowercase, each run of digits one `#`, and device names
/// without their number, so `/dev/nvme0n1p2` and `/dev/sda2` match.
#[must_use]
pub fn normalise(message: &str) -> String {
    let mut out = String::with_capacity(message.len());
    let lower = message.to_lowercase();
    let mut rest = lower.as_str();
    while let Some(c) = rest.chars().next() {
        if let Some(after) = rest.strip_prefix("/dev/") {
            let end = after
                .find(|c: char| !c.is_ascii_alphanumeric())
                .unwrap_or(after.len());
            out.push_str("/dev/#");
            rest = &after[end..];
            continue;
        }
        if c.is_ascii_digit() {
            out.push('#');
            rest = rest.trim_start_matches(|c: char| c.is_ascii_digit());
        } else {
            out.push(c);
            rest = &rest[c.len_utf8()..];
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Eight hex digits naming a failure: FNV-1a of [`normalise`]d text, which
/// is the same in every build, unlike std's hasher.
#[must_use]
pub fn fingerprint(step: &str, message: &str) -> String {
    let text = format!("{step}\n{}", normalise(message));
    let hash = text.bytes().fold(0x811c_9dc5_u32, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193)
    });
    format!("{hash:08x}")
}

/// Characters that end a word for [`redact`].
const BREAKS: &str = "()[],;'\"<>=`";

/// `text` with what says where a machine is, or whose it is, replaced: MAC,
/// IPv4 and IPv6 addresses, e-mail addresses, Tor and I2P names, long
/// runs of hex (a token someone logged by mistake) and each of `private`
/// (the box's name, the people on it), matched whole and without case.
/// A report is public. Hardware names, versions and paths stay: they are
/// what the report is for.
#[must_use]
pub fn redact(text: &str, private: &[String]) -> String {
    let private: Vec<String> = private
        .iter()
        .map(|p| p.trim().to_lowercase())
        // The default name is the product's: it says nothing about whose
        // box this is, and it is in every unit's name.
        .filter(|p| p.len() >= 3 && p != "losos")
        .collect();
    let mut out = String::with_capacity(text.len());
    for word in text.split_inclusive(|c: char| c.is_whitespace() || BREAKS.contains(c)) {
        let end = word
            .find(|c: char| c.is_whitespace() || BREAKS.contains(c))
            .unwrap_or(word.len());
        let (core, sep) = word.split_at(end);
        // A trailing full stop or colon is the sentence's, not the address's.
        let trimmed = core.trim_end_matches(['.', ':']);
        let tail = &core[trimmed.len()..];
        out.push_str(&redact_word(trimmed, &private));
        out.push_str(tail);
        out.push_str(sep);
    }
    out
}

fn redact_word(word: &str, private: &[String]) -> String {
    if word.is_empty() {
        return String::new();
    }
    if is_mac(word) {
        return "[MAC]".to_owned();
    }
    if let Some(after) = ipv4(word) {
        return format!("[IP]{after}");
    }
    if is_ipv6(word) {
        return "[IP]".to_owned();
    }
    if is_email(word) {
        return "[e-mail]".to_owned();
    }
    let lower = word.to_lowercase();
    if lower.ends_with(".onion") || lower.ends_with(".b32.i2p") {
        return "[hidden address]".to_owned();
    }
    if is_long_hex(word) {
        return "[hex]".to_owned();
    }
    // The box's name and the people on it, anywhere in a word: a host name
    // turns up as `box.local`, `box-1` and `user@box`.
    let mut out = word.to_owned();
    for p in private {
        let mut from = 0;
        loop {
            let lower = out.to_lowercase();
            // Lower case that changes a word's length would put the match
            // somewhere else in the original; such a word keeps its name.
            if lower.len() != out.len() || from >= out.len() {
                break;
            }
            let Some(at) = lower[from..].find(p.as_str()).map(|at| from + at) else {
                break;
            };
            let before = out[..at].chars().next_back();
            let after = out[at + p.len()..].chars().next();
            // `-` and `_` join a name into a longer one: a box called
            // `media` leaves `media-server.service` alone.
            let edge =
                |c: Option<char>| c.is_none_or(|c| !(c.is_alphanumeric() || c == '-' || c == '_'));
            if edge(before) && edge(after) {
                out.replace_range(at..at + p.len(), "[name]");
                from = at + "[name]".len();
            } else {
                from = at + p.len();
            }
        }
    }
    out
}

fn is_mac(word: &str) -> bool {
    let parts: Vec<&str> = word.split(':').collect();
    parts.len() == 6
        && parts
            .iter()
            .all(|p| p.len() == 2 && p.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// When `word` is an IPv4 address, with or without a `:port` or `/prefix`,
/// what follows the address. Loopback stays as it is: the installer's own
/// cache is on 127.0.0.1, and it says nothing about the machine.
fn ipv4(word: &str) -> Option<&str> {
    let end = word.find([':', '/']).unwrap_or(word.len());
    let (address, after) = word.split_at(end);
    let octets: Vec<&str> = address.split('.').collect();
    let valid = octets.len() == 4
        && octets
            .iter()
            .all(|o| !o.is_empty() && o.len() <= 3 && o.parse::<u8>().is_ok());
    (valid && octets[0] != "127").then_some(after)
}

/// An IPv6 address, bare or in brackets, with or without a `/prefix`;
/// never `::1`. A clock time has two colons and no hex letters run past
/// four, so `12:30:01` stays.
fn is_ipv6(word: &str) -> bool {
    let word = word.trim_start_matches('[').trim_end_matches(']');
    let address = word.split('/').next().unwrap_or(word);
    if address == "::1" || address == "::" {
        return false;
    }
    let colons = address.matches(':').count();
    let groups_ok = address
        .split(':')
        .all(|g| g.len() <= 4 && g.bytes().all(|b| b.is_ascii_hexdigit()));
    groups_ok && (address.contains("::") && colons >= 2 || colons >= 5) && address.len() >= 6
}

fn is_email(word: &str) -> bool {
    match word.split_once('@') {
        Some((user, domain)) => {
            !user.is_empty()
                && domain.contains('.')
                && !domain.starts_with('.')
                && !domain.ends_with('.')
                && domain
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "-.".contains(c))
        }
        None => false,
    }
}

/// 32 hex characters or more in one run: a key, a token or a hash that a
/// program logged. Store paths' hashes are not hex, so they stay.
fn is_long_hex(word: &str) -> bool {
    let mut run = 0;
    for b in word.bytes() {
        if b.is_ascii_hexdigit() {
            run += 1;
            if run >= 32 {
                return true;
            }
        } else {
            run = 0;
        }
    }
    false
}

/// A fenced block of `lines`, keeping the newest when they do not fit in
/// `budget` characters.
#[must_use]
pub fn fenced(lines: &[String], budget: usize) -> String {
    let mut kept = Vec::new();
    let mut used = 0;
    for line in lines.iter().rev() {
        used += line.len() + 1;
        if used > budget {
            kept.push("… (earlier lines left out)".to_owned());
            break;
        }
        kept.push(line.clone());
    }
    kept.reverse();
    // A line of backticks in the output would close a fence of three.
    format!(
        "````text\n{}\n````",
        kept.join("\n").replace("````", "` ` ` `")
    )
}

/// A `<details>` section holding `lines`, or nothing when there are none.
#[must_use]
pub fn details(summary: &str, lines: &[String], budget: usize) -> String {
    if lines.is_empty() {
        return String::new();
    }
    format!(
        "\n<details><summary>{summary}</summary>\n\n{}\n\n</details>\n",
        fenced(lines, budget)
    )
}

/// `report`'s body with the person's own words, when they wrote any, before
/// the rest under `heading` (**From the person installing:**).
#[must_use]
pub fn with_note(report: &Report, heading: &str, note: &str) -> String {
    let note = note.trim();
    if note.is_empty() {
        report.body.clone()
    } else {
        let quoted: Vec<String> = note.lines().map(|l| format!("> {l}")).collect();
        format!("**{heading}**\n\n{}\n\n{}", quoted.join("\n"), report.body)
    }
}

/// `text`'s first line cut to `max` characters, with an ellipsis when it
/// was longer.
#[must_use]
pub fn short(text: &str, max: usize) -> String {
    let first = text.lines().next().unwrap_or("").trim();
    if first.chars().count() > max {
        format!("{}…", first.chars().take(max).collect::<String>())
    } else {
        first.to_owned()
    }
}

/// `n` lines, in words.
#[must_use]
pub fn lines_word(n: usize) -> String {
    match n {
        1 => "1 line".to_owned(),
        n => format!("{n} lines"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_failure_on_two_machines_has_one_fingerprint() {
        assert_eq!(
            normalise("Failed to create   new partition '/dev/nvme0n1p2': busy (16)"),
            "failed to create new partition '/dev/#': busy (#)"
        );
        assert_eq!(
            fingerprint(
                "Partitioning the disk",
                "Failed to mount /dev/sda2 after 10 s"
            ),
            fingerprint(
                "Partitioning the disk",
                "Failed to mount /dev/vda2 after 30 s"
            ),
        );
        assert_ne!(fingerprint("a", "x"), fingerprint("b", "x"));
        assert_eq!(fingerprint("a", "b").len(), 8);
    }

    #[test]
    fn addresses_and_names_are_taken_out_and_the_rest_left() {
        assert_eq!(
            redact("eth0 52:54:00:12:34:56 got 10.0.2.15/24 via 10.0.2.2.", &[]),
            "eth0 [MAC] got [IP]/24 via [IP]."
        );
        assert_eq!(
            redact(
                "GET http://127.0.0.1:8787/nix-cache-info (2.6.1, v10.0.2)",
                &[]
            ),
            "GET http://127.0.0.1:8787/nix-cache-info (2.6.1, v10.0.2)"
        );
        assert_eq!(redact("time 12:30:01 ok", &[]), "time 12:30:01 ok");
        assert_eq!(
            redact(
                "from fe80::5054:ff:fe12:3456 and [2001:db8::1]:443, ::1 stays",
                &[]
            ),
            "from [IP] and [[IP]]:443, ::1 stays"
        );
        assert_eq!(
            redact("mail to eva@example.org failed", &[]),
            "mail to [e-mail] failed"
        );
        assert_eq!(
            redact(
                "token 0123456789abcdef0123456789abcdef01 at abcdefghijklmnopqrstuvwxyz234567abcdefghijklmnopqrstuvwx.onion",
                &[]
            ),
            "token [hex] at [hidden address]"
        );
        let private = vec!["Kitchen-Box".to_owned(), "eva".to_owned()];
        assert_eq!(
            redact(
                "kitchen-box.local: eva signed in; evangelist stays",
                &private
            ),
            "[name].local: [name] signed in; evangelist stays"
        );
        assert_eq!(
            redact(
                "losos.local eva-sync.service",
                &["losos".to_owned(), "eva".to_owned()]
            ),
            "losos.local eva-sync.service"
        );
        // A store path keeps its hash: it is not hex.
        let store = "/nix/store/3x9fvq9l1mbrqwkx7cp5ld5n0q1z1d2s-lososd/bin/lososd";
        assert_eq!(redact(store, &[]), store);
    }

    #[test]
    fn a_note_goes_first_under_its_heading() {
        let report = Report {
            title: "t".into(),
            body: "body".into(),
            fingerprint: "00000000".into(),
        };
        assert_eq!(
            with_note(&report, "From the box's owner:", "It froze\nagain"),
            "**From the box's owner:**\n\n> It froze\n> again\n\nbody"
        );
        assert_eq!(with_note(&report, "x", "  "), "body");
    }

    #[test]
    fn the_system_table_leaves_out_what_it_does_not_know() {
        let system = System::from_files(
            &SystemFiles {
                os_release: "NAME=LosOS\nVERSION_ID=\"26.11\"\nIMAGE_VERSION=\"v0.6.1\"\n".into(),
                cpuinfo: "processor\t: 0\nmodel name\t: Intel N100 | x\nprocessor\t: 1\n".into(),
                meminfo: "MemTotal:        4028756 kB\nMemFree: 1 kB\n".into(),
                kernel: "6.18.1\n".into(),
                efi: true,
                ..SystemFiles::default()
            },
            "x86_64",
        );
        let table = system.table(&[("Unit", "lososd.service"), ("App", "")]);
        assert!(table.contains("| LosOS | v0.6.1, x86_64 |"));
        assert!(table.contains("| CPU | Intel N100 \\| x (2 threads) |"));
        assert!(table.contains("| Memory | 3.8 GiB |"));
        assert!(table.contains("| Firmware | UEFI |"));
        assert!(table.contains("| Unit | lososd.service |"));
        assert!(!table.contains("| App |"));
        assert!(!table.contains("| Machine |"));
    }

    #[test]
    fn a_target_needs_a_repository_and_a_client() {
        assert_eq!(
            Target::from_settings(Some("dasmatus/losos"), Some(" "), None, None),
            None
        );
        assert_eq!(
            Target::from_settings(Some("not a repo"), Some("Iv1"), None, None),
            None
        );
        let t = Target::from_settings(Some("dasmatus/losos"), Some("Iv1"), None, Some("")).unwrap();
        assert_eq!(t.login, "https://github.com");
        assert_eq!(t.api, "https://api.github.com");
        assert!(!valid_repo("a/b/c"));
    }
}
