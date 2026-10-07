//! Reading and writing the two Nix files the control plane owns.
//!
//! This module is pure: it takes and returns strings, touches no filesystem,
//! and is where the whole `overrides.nix` contract lives. One file is
//! involved, `modules/overrides.nix` (`LOSOS_OVERRIDES`): read by `settings`,
//! overwritten wholesale by `apply` and `factory-reset`, and line-patched by
//! `change --mode` via [`inject_line`]. (`change` used to patch a second file,
//! `defaults.nix`, which no installed box has; see `cmd_change`.)
//!
//! Parsing is line-based on purpose. A real Nix parser would reject the
//! hand-edited files this appliance tolerates, and the only values that need
//! reading are scalar assignments on a single line.

use crate::model::Settings;

/// The committed default body. Returned when `overrides.nix` is missing, and
/// written back verbatim by `factory-reset`.
///
/// The assignment lines must stay byte-identical to the ones in
/// `modules/overrides.nix`, or `factory-reset` quietly changes settings it was
/// never asked to change. The two headers differ on purpose — the module reads
/// `_:` and this one `{ ... }:` — and that is fine, because
/// [`parse_settings`] never looks at the header. `parses_the_committed_default_body`
/// is the gate: it parses this constant and asserts the result equals
/// [`Settings::default`], so a line added here and forgotten there fails the
/// suite.
pub const DEFAULT_OVERRIDES_NIX: &str = r#"{ ... }:

{
  losos.sharingMyStorage = true;
  losos.nextcloud.mode = "container";
  losos.forgejo.mode = "container";
  losos.hostName = "mattbox";
  losos.nextcloud.https = false;
  losos.gpu.enable = true;
  losos.nextcloud.apachePort = 11000;
  losos.proxy.enable = false;
  losos.cluster.enable = false;
  losos.cluster.shareCompute = false;
  losos.cluster.computeWindow.start = "23:00";
  losos.cluster.computeWindow.end = "07:00";
  losos.hardening.apparmor = false;
  losos.hardening.malloc = false;
  losos.hardening.nosmt = false;
  losos.hardening.usbguard = false;
}
"#;

/// One `losos.<key> = <value>;` line, as `(key, value)`, or `None` when the
/// line is not an assignment of a `losos.*` option.
///
/// The left side must BE an option, not merely contain it: matching on
/// `contains` made `losos.proxy.enable` also match `losos.proxy.enabled`, and
/// `losos.gpu.enable = true; # see losos.hostName` match hostName. A compact
/// one-line body (`{ losos.hostName = "x"; }`) puts a brace before the key,
/// so that is stripped before the key is read. A trailing comment is dropped
/// before anything else, or `= 11000; # default` reads back as
/// `11000; # default`; then the statement terminator, then a closing brace
/// from a one-line body, then the terminator again in case the brace hid it.
pub fn parse_line(l: &str) -> Option<(String, String)> {
    if l.trim_start().starts_with('#') {
        return None;
    }
    let (lhs, rhs) = l.split_once('=')?;
    let key = lhs
        .trim()
        .trim_start_matches('{')
        .trim()
        .strip_prefix("losos.")?;
    if key.is_empty() || !key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.') {
        return None;
    }
    let v = strip_comment(rhs);
    let v = v.trim().trim_end_matches(';').trim();
    let v = v.trim_end_matches('}').trim().trim_end_matches(';').trim();
    Some((key.to_string(), v.to_string()))
}

/// `s` up to its first `#` outside a double-quoted string.
///
/// A `#` inside quotes is part of the value: `"git+file:///etc/nixos#install"`
/// is the default `upgradeFlakeUri`, and cutting it at the `#` left an
/// unterminated string that the Advanced pane's gate then refused.
fn strip_comment(s: &str) -> &str {
    let mut in_string = false;
    let mut escaped = false;
    for (i, c) in s.char_indices() {
        match c {
            _ if escaped => escaped = false,
            '\\' if in_string => escaped = true,
            '"' => in_string = !in_string,
            '#' if !in_string => return &s[..i],
            _ => {}
        }
    }
    s
}

/// Find the value assigned to `losos.<key>`.
///
/// The first line that assigns the key wins (see [`parse_line`] for what
/// counts as one). The value is everything after the first `=`, minus
/// surrounding whitespace, a trailing comment and one trailing `;`.
pub fn lookup_nix(key: &str, content: &str) -> Option<String> {
    content
        .lines()
        .filter_map(parse_line)
        .find_map(|(k, v)| (k == key).then_some(v))
}

/// Every `losos.<key> = <value>;` assignment in `content`, in file order.
///
/// What [`crate::options::join`] shows as `set`, what [`crate::options::
/// check_body`] gates, and what [`describe_changes`] diffs. Deliberately not
/// a map: the first assignment of a key is the one [`lookup_nix`] reads, so
/// callers that need one value per key take the first too.
pub fn assignments(content: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for (k, v) in content.lines().filter_map(parse_line) {
        if !out.iter().any(|(seen, _)| *seen == k) {
            out.push((k, v));
        }
    }
    out
}

/// One option whose assignment differs between two `overrides.nix` bodies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub key: String,
    /// `None` is "not assigned", which the module reads as the default.
    pub before: Option<String>,
    pub after: Option<String>,
}

/// The assignments that differ between `old` and `new`, in the order `new`
/// lists them, with the keys `new` dropped at the end.
pub fn describe_changes(old: &str, new: &str) -> Vec<Change> {
    let before = assignments(old);
    let after = assignments(new);
    let lookup = |set: &[(String, String)], key: &str| -> Option<String> {
        set.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone())
    };
    let mut out = Vec::new();
    for (key, value) in &after {
        let was = lookup(&before, key);
        if was.as_deref() != Some(value) {
            out.push(Change {
                key: key.clone(),
                before: was,
                after: Some(value.clone()),
            });
        }
    }
    for (key, value) in &before {
        if lookup(&after, key).is_none() {
            out.push(Change {
                key: key.clone(),
                before: Some(value.clone()),
                after: None,
            });
        }
    }
    out
}

/// The commit a change to `overrides.nix` gets in the box's configuration
/// repository (`crate::config_repo`): a subject naming the settings, and a
/// body with one `losos.<key>: before -> after` line per change.
///
/// `title` is what the caller was asked to do — "Change" for an Apply or a
/// storage switch, "Reset" for a factory reset — so the subject reads
/// `Change hostName, cluster.enable` or `Reset settings to the defaults`.
pub fn commit_message(title: &str, changes: &[Change]) -> (String, String) {
    const NAMED: usize = 3;
    let names: Vec<&str> = changes.iter().map(|c| c.key.as_str()).collect();
    let subject = match names.len() {
        0 => format!("{title} settings (no assignment changed)"),
        n if n <= NAMED => format!("{title} {}", names.join(", ")),
        n => format!(
            "{title} {} and {} more",
            names[..NAMED].join(", "),
            n - NAMED
        ),
    };
    let body = changes
        .iter()
        .map(|c| {
            format!(
                "losos.{}: {} -> {}",
                c.key,
                c.before.as_deref().unwrap_or("(default)"),
                c.after.as_deref().unwrap_or("(default)")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    (subject, body)
}

/// Strip one matching pair of surrounding double quotes, if present.
fn strip_quotes(v: &str) -> &str {
    let b = v.as_bytes();
    if b.len() >= 2 && b[0] == b'"' && b[b.len() - 1] == b'"' {
        &v[1..v.len() - 1]
    } else {
        v
    }
}

/// `"true"`/`"false"`; anything else (including a missing key) keeps `default`.
fn read_bool(default: bool, v: Option<String>) -> bool {
    match v.as_deref() {
        Some("true") => true,
        Some("false") => false,
        _ => default,
    }
}

/// Quote-stripped string, or `default` when the key is absent.
fn read_str(default: &str, v: Option<String>) -> String {
    v.map(|s| strip_quotes(&s).to_string())
        .unwrap_or_else(|| default.to_string())
}

/// Like [`read_str`], but folds the retired `"aio"` deployment mode into the
/// `"container"` it became, so a pre-migration overrides file still reads back
/// as something the admin UI's `<select>` can display.
fn read_mode(default: &str, v: Option<String>) -> String {
    let s = read_str(default, v);
    if s == "aio" {
        "container".to_string()
    } else {
        s
    }
}

/// Integer, or `default` when the key is absent or unparseable.
fn read_int(default: i64, v: Option<String>) -> i64 {
    v.and_then(|s| s.trim().parse::<i64>().ok())
        .unwrap_or(default)
}

/// Parse a whole `overrides.nix` body into [`Settings`].
///
/// Missing keys fall back to [`Settings::default`] field by field, so a partial
/// file is still readable. Two keys carry legacy aliases that are honoured only
/// when the current spelling is absent: `losos.aio.apachePort` (pre-rename
/// Nextcloud AIO) and `losos.cfd.enable` (the retired Cloudflare tunnel, whose
/// role the master proxy took over).
pub fn parse_settings(content: &str) -> Settings {
    let d = Settings::default();
    Settings {
        sharing_my_storage: read_bool(
            d.sharing_my_storage,
            lookup_nix("sharingMyStorage", content),
        ),
        nextcloud_mode: read_mode(&d.nextcloud_mode, lookup_nix("nextcloud.mode", content)),
        forgejo_mode: read_str(&d.forgejo_mode, lookup_nix("forgejo.mode", content)),
        host_name: read_str(&d.host_name, lookup_nix("hostName", content)),
        https: read_bool(d.https, lookup_nix("nextcloud.https", content)),
        gpu_enable: read_bool(d.gpu_enable, lookup_nix("gpu.enable", content)),
        apache_port: read_int(
            d.apache_port,
            lookup_nix("nextcloud.apachePort", content)
                .or_else(|| lookup_nix("aio.apachePort", content)),
        ),
        proxy_enable: read_bool(
            d.proxy_enable,
            lookup_nix("proxy.enable", content).or_else(|| lookup_nix("cfd.enable", content)),
        ),
        // No legacy aliases below: these four keys have never had an earlier
        // spelling, so an `.or_else` fallback here would only be a place for a
        // typo to hide.
        cluster_enable: read_bool(d.cluster_enable, lookup_nix("cluster.enable", content)),
        share_compute: read_bool(d.share_compute, lookup_nix("cluster.shareCompute", content)),
        compute_window_start: read_str(
            &d.compute_window_start,
            lookup_nix("cluster.computeWindow.start", content),
        ),
        compute_window_end: read_str(
            &d.compute_window_end,
            lookup_nix("cluster.computeWindow.end", content),
        ),
        // Each missing key reads FALSE, which is exactly what options.nix
        // declares as its default. So an overrides.nix written before these
        // four existed parses to "all off" — the behaviour that box already
        // had — rather than to anything surprising. That property is what lets
        // this land without a migration.
        hardening_apparmor: read_bool(
            d.hardening_apparmor,
            lookup_nix("hardening.apparmor", content),
        ),
        hardening_malloc: read_bool(d.hardening_malloc, lookup_nix("hardening.malloc", content)),
        hardening_nosmt: read_bool(d.hardening_nosmt, lookup_nix("hardening.nosmt", content)),
        hardening_usbguard: read_bool(
            d.hardening_usbguard,
            lookup_nix("hardening.usbguard", content),
        ),
    }
}

/// Patch the `losos.sharingMyStorage` assignment in `overrides.nix`.
///
/// Replaces the first real assignment in place. When there is none, the line is
/// inserted before the closing brace, or appended if the file doesn't end in
/// one. The `contains('=')` guard means a comment merely *mentioning* the
/// option is left alone.
pub fn inject_line(sharing: bool, lines: &[String]) -> Vec<String> {
    let new = format!(
        "  losos.sharingMyStorage = {};",
        if sharing { "true" } else { "false" }
    );
    let is_assign = |l: &String| l.contains("losos.sharingMyStorage") && l.contains('=');

    if let Some(i) = lines.iter().position(is_assign) {
        let mut out = lines.to_vec();
        out[i] = new;
        return out;
    }
    let mut out = lines.to_vec();
    match out.last() {
        Some(last) if last.contains('}') => out.insert(out.len() - 1, new),
        _ => out.push(new),
    }
    out
}

/// Whether `h` is a valid single DNS label, per RFC 1123: 1-63 characters,
/// alphanumeric at both ends, hyphens allowed in between.
///
/// `losos.hostName` becomes `networking.hostName`, the mDNS name the box is
/// reached by, Nextcloud's `trusted_domains` and `overwrite.cli.url`, and
/// Forgejo's `ROOT_URL`. A space, a slash, a leading hyphen or a 64th
/// character produces either a config that will not evaluate or an appliance
/// that boots unreachable — and there is no SSH and no login shell to repair
/// it from. The admin UI checks this too, but a browser is not a trust
/// boundary: `POST /api/apply` accepts a body from anything holding the token.
pub fn valid_host_name(h: &str) -> bool {
    let b = h.as_bytes();
    !h.is_empty()
        && h.len() <= 63
        && b[0].is_ascii_alphanumeric()
        && b[b.len() - 1].is_ascii_alphanumeric()
        && b.iter().all(|c| c.is_ascii_alphanumeric() || *c == b'-')
}

/// Whether `s` is a wall-clock time of day spelled exactly `HH:MM`, 00:00
/// through 23:59.
///
/// This is the format `<input type="time">` emits, which is why the SPA's
/// regex and this function can agree without either side normalising. The
/// value ends up in `losos.cluster.computeWindow.{start,end}` and from there in
/// a systemd `OnCalendar`-shaped window the edge uses to taint this node; a
/// value that is not `HH:MM` produces a config that either fails to evaluate or
/// evaluates to a window that never opens, on a box with no shell to notice it
/// from. The admin UI checks this too, but a browser is not a trust boundary —
/// `POST /api/apply` accepts a body from anything holding the token.
///
/// Length is checked first, so the byte indexing below cannot panic.
pub fn valid_hhmm(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 5
        && b[2] == b':'
        && b[0].is_ascii_digit()
        && b[1].is_ascii_digit()
        && b[3].is_ascii_digit()
        && b[4].is_ascii_digit()
        && (b[0] - b'0') * 10 + (b[1] - b'0') < 24
        && (b[3] - b'0') * 10 + (b[4] - b'0') < 60
}

/// Gate an `apply` payload before it overwrites `overrides.nix`.
///
/// Deliberately shallow on syntax — no brace balancing, no Nix parsing.
/// `nixos-rebuild` is the real syntax checker; this only blocks payloads that
/// are obviously empty or aimed at the wrong file, which would otherwise
/// silently blank the appliance's configuration.
///
/// It is *not* shallow about `hostName`, because a bad one is the single value
/// here that can leave the box unreachable with no way back in. Rules are
/// checked in order and the messages are part of the HTTP contract.
pub fn validate_apply(t: &str) -> Result<&str, &'static str> {
    if t.trim().is_empty() {
        return Err("empty nix config");
    }
    if !t.contains("losos.") {
        return Err("nix config must reference losos.* options");
    }
    if !t.contains('{') {
        return Err("nix config must be a module body (missing '{')");
    }
    // Only checked when the body actually sets it; an apply that leaves
    // hostName alone is none of this function's business.
    if let Some(raw) = lookup_nix("hostName", t) {
        let h = strip_quotes(&raw);
        // A `${...}` here is Nix interpolation, evaluated as root by the
        // rebuild — `${builtins.readFile "/var/secrets/losos-admin-token"}`
        // would splice the token into the config. The character class below
        // already excludes `$`, `{` and `}`; this arm exists to give that
        // case a message that says what is wrong rather than "invalid".
        if h.contains("${") {
            return Err("hostName must not contain Nix interpolation");
        }
        if !valid_host_name(h) {
            return Err(
                "hostName must be 1-63 chars, alphanumeric at both ends, hyphens allowed between",
            );
        }
    }
    // Same "only when the body sets it" rule as hostName: the two window keys
    // are checked independently, so an apply that moves only the start time is
    // not forced to restate the end.
    if let Some(raw) = lookup_nix("cluster.computeWindow.start", t) {
        if !valid_hhmm(strip_quotes(&raw)) {
            return Err("computeWindowStart must be HH:MM, 00:00-23:59");
        }
    }
    if let Some(raw) = lookup_nix("cluster.computeWindow.end", t) {
        if !valid_hhmm(strip_quotes(&raw)) {
            return Err("computeWindowEnd must be HH:MM, 00:00-23:59");
        }
    }
    Ok(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_inside_a_string_is_part_of_the_value() {
        let line = r#"  losos.upgradeFlakeUri = "git+file:///etc/nixos#install"; # the default"#;
        assert_eq!(
            parse_line(line),
            Some((
                "upgradeFlakeUri".to_string(),
                r#""git+file:///etc/nixos#install""#.to_string()
            ))
        );
        assert_eq!(
            parse_line(r##"  losos.hostName = "a\"#b"; # c"##),
            Some(("hostName".to_string(), r##""a\"#b""##.to_string()))
        );
        assert_eq!(
            parse_line("  losos.x = true; # a # b"),
            Some(("x".to_string(), "true".to_string()))
        );
    }

    #[test]
    fn parses_the_committed_default_body() {
        let s = parse_settings(DEFAULT_OVERRIDES_NIX);
        assert_eq!(s, Settings::default());
    }

    /// The constant above and the real `modules/overrides.nix` must agree.
    ///
    /// The doc comment on [`DEFAULT_OVERRIDES_NIX`] has always *asked* for
    /// this, and nothing enforced it: the test above parses the constant, so
    /// the two could drift apart silently and `factory-reset` — which writes
    /// the constant — would quietly change settings it was never asked to
    /// change. Reading the module through `include_str!` makes the compiler
    /// carry the invariant.
    ///
    /// Compares parses, not bytes: the two headers differ on purpose (`_:`
    /// against `{ ... }:`) and the module carries comments the constant does
    /// not, so a byte comparison would fail on the arrangement the design
    /// intends.
    ///
    /// The `include_str!` reaches OUTSIDE this crate, and `flake/packages.nix`
    /// copies only `backend/` into the store. That is fine as long as
    /// `doCheck` stays off — the shipping build never compiles this module —
    /// and `cargo test` / `cargo clippy --all-targets` always run from the
    /// repository root, which is how CI and `devenv test` invoke them. If
    /// anyone ever turns `doCheck` on, this is the line that will fail, and
    /// the fix is to widen that derivation's `src`, not to delete the test.
    #[test]
    fn the_committed_module_and_the_constant_agree() {
        let module = include_str!("../../modules/overrides.nix");
        assert_eq!(
            parse_settings(module),
            parse_settings(DEFAULT_OVERRIDES_NIX),
            "modules/overrides.nix and DEFAULT_OVERRIDES_NIX have drifted; \
             factory-reset would silently change settings"
        );
    }

    /// The keys are spelled right and actually reach the fields.
    ///
    /// `parses_the_committed_default_body` cannot catch a typo here, because
    /// every hardening default is `false` and a key that never matches also
    /// reads `false`. Flipping all four to `true` is what tells a working
    /// lookup apart from one that silently falls through.
    #[test]
    fn hardening_flags_are_read_from_the_file() {
        let body = "{ ... }:\n{\n  \
             losos.hardening.apparmor = true;\n  \
             losos.hardening.malloc = true;\n  \
             losos.hardening.nosmt = true;\n  \
             losos.hardening.usbguard = true;\n}\n";
        let s = parse_settings(body);
        assert!(s.hardening_apparmor);
        assert!(s.hardening_malloc);
        assert!(s.hardening_nosmt);
        assert!(s.hardening_usbguard);
    }

    #[test]
    fn missing_keys_fall_back_per_field() {
        let s = parse_settings("{ ... }:\n{\n  losos.hostName = \"box2\";\n}\n");
        assert_eq!(s.host_name, "box2");
        // everything else stays at the default
        assert_eq!(s.apache_port, 11000);
        assert!(s.sharing_my_storage);
    }

    #[test]
    fn commented_out_assignments_are_ignored() {
        let s =
            parse_settings("{\n  # losos.hostName = \"ghost\";\n  losos.hostName = \"real\";\n}");
        assert_eq!(s.host_name, "real");
    }

    #[test]
    fn legacy_aio_apache_port_is_honoured() {
        let s = parse_settings("{\n  losos.aio.apachePort = 12345;\n}");
        assert_eq!(s.apache_port, 12345);
    }

    #[test]
    fn current_apache_port_wins_over_legacy_alias() {
        let s =
            parse_settings("{\n  losos.nextcloud.apachePort = 1;\n  losos.aio.apachePort = 2;\n}");
        assert_eq!(s.apache_port, 1);
    }

    #[test]
    fn legacy_cfd_enable_feeds_proxy_enable() {
        let s = parse_settings("{\n  losos.cfd.enable = true;\n}");
        assert!(s.proxy_enable);
    }

    #[test]
    fn legacy_aio_mode_reads_back_as_container() {
        let s = parse_settings("{\n  losos.nextcloud.mode = \"aio\";\n}");
        assert_eq!(s.nextcloud_mode, "container");
    }

    #[test]
    fn inject_replaces_an_existing_assignment_in_place() {
        let lines: Vec<String> = vec![
            "{".into(),
            "  losos.sharingMyStorage = true;".into(),
            "}".into(),
        ];
        let out = inject_line(false, &lines);
        assert_eq!(out[1], "  losos.sharingMyStorage = false;");
        assert_eq!(out.len(), 3);
    }

    #[test]
    fn inject_inserts_before_the_closing_brace_when_absent() {
        let lines: Vec<String> = vec!["{".into(), "  losos.hostName = \"x\";".into(), "}".into()];
        let out = inject_line(true, &lines);
        assert_eq!(out[2], "  losos.sharingMyStorage = true;");
        assert_eq!(out[3], "}");
    }

    #[test]
    fn inject_ignores_a_comment_that_only_mentions_the_option() {
        let lines: Vec<String> = vec![
            "{".into(),
            "  # losos.sharingMyStorage is set by the admin UI".into(),
            "}".into(),
        ];
        let out = inject_line(true, &lines);
        // the comment survives untouched; the assignment is inserted separately
        assert!(out[1].starts_with("  #"));
        assert_eq!(out[2], "  losos.sharingMyStorage = true;");
    }

    #[test]
    fn validate_apply_rules_fire_in_order() {
        assert_eq!(validate_apply("   \n "), Err("empty nix config"));
        assert_eq!(
            validate_apply("{ services.foo = 1; }"),
            Err("nix config must reference losos.* options")
        );
        assert_eq!(
            validate_apply("losos.hostName = \"x\";"),
            Err("nix config must be a module body (missing '{')")
        );
        assert!(validate_apply("{ losos.hostName = \"x\"; }").is_ok());
    }

    #[test]
    fn a_trailing_comment_does_not_poison_the_value() {
        // Split on the first `=` and this read back as `11000; # default`,
        // which parsed to the default by luck. `= 12345; # x` would have
        // silently returned the wrong port.
        let s = parse_settings("{\n  losos.nextcloud.apachePort = 12345; # default\n}\n");
        assert_eq!(s.apache_port, 12345);
    }

    #[test]
    fn a_key_is_not_matched_by_a_longer_key_that_contains_it() {
        // `contains("losos.proxy.enable")` also matched `losos.proxy.enabled`.
        let s = parse_settings("{\n  losos.proxy.enabled = true;\n}\n");
        assert!(!s.proxy_enable, "a different option must not be read");
    }

    #[test]
    fn a_mention_in_a_trailing_comment_is_not_an_assignment() {
        // This line assigns gpu.enable; it merely *mentions* hostName.
        let s = parse_settings("{\n  losos.gpu.enable = true; # see losos.hostName\n}\n");
        assert_eq!(s.host_name, Settings::default().host_name);
        assert!(s.gpu_enable);
    }

    #[test]
    fn a_one_line_module_body_parses() {
        // The closing brace used to end up inside the value.
        let s = parse_settings("{ losos.hostName = \"box2\"; }");
        assert_eq!(s.host_name, "box2");
    }

    #[test]
    fn valid_host_names_are_accepted() {
        for h in ["x", "mattbox", "box-2", "a-b-c", &"a".repeat(63)] {
            assert!(valid_host_name(h), "{h:?} should be valid");
        }
    }

    #[test]
    fn invalid_host_names_are_rejected() {
        for h in [
            "",              // empty
            "-box",          // leading hyphen
            "box-",          // trailing hyphen
            "my box",        // space
            "a/b",           // slash
            "box.local",     // dot: this is one label, not an FQDN
            "box_2",         // underscore is not legal in a hostname
            &"a".repeat(64), // 63 is the limit
        ] {
            assert!(!valid_host_name(h), "{h:?} should be rejected");
        }
    }

    #[test]
    fn apply_rejects_a_hostname_that_would_brick_the_box() {
        // The browser checks these too, but a token holder can POST directly.
        assert!(validate_apply("{ losos.hostName = \"my box\"; }").is_err());
        assert!(validate_apply("{ losos.hostName = \"-box\"; }").is_err());
        assert!(validate_apply("{ losos.hostName = \"\"; }").is_err());
    }

    #[test]
    fn apply_rejects_nix_interpolation_in_the_hostname() {
        // Evaluated as root by nixos-rebuild; this one splices the admin token
        // into the generated config.
        let body =
            "{ losos.hostName = \"a${builtins.readFile \"/var/secrets/losos-admin-token\"}b\"; }";
        assert_eq!(
            validate_apply(body),
            Err("hostName must not contain Nix interpolation")
        );
    }

    #[test]
    fn apply_without_a_hostname_is_left_alone() {
        // Not every apply touches hostName; the check must not demand one.
        assert!(validate_apply("{ losos.sharingMyStorage = true; }").is_ok());
    }

    #[test]
    fn the_compute_window_reads_back_from_the_two_keys() {
        let s = parse_settings(
            "{\n  losos.cluster.enable = true;\n  losos.cluster.shareCompute = true;\n  losos.cluster.computeWindow.start = \"01:30\";\n  losos.cluster.computeWindow.end = \"05:45\";\n}\n",
        );
        assert!(s.cluster_enable);
        assert!(s.share_compute);
        assert_eq!(s.compute_window_start, "01:30");
        assert_eq!(s.compute_window_end, "05:45");
    }

    #[test]
    fn valid_times_of_day_are_accepted() {
        for t in ["00:00", "23:59", "07:00", "23:00"] {
            assert!(valid_hhmm(t), "{t:?} should be valid");
        }
    }

    #[test]
    fn invalid_times_of_day_are_rejected() {
        for t in [
            "24:00",  // hour out of range
            "7:00",   // unpadded hour: not what <input type="time"> emits
            "23:60",  // minute out of range
            "23:5",   // too short
            "",       // empty
            "1:2:3",  // seconds, and the wrong length
            "23-00",  // right length, wrong separator
            "ab:cd",  // right shape, not digits
            "23:00 ", // trailing space
        ] {
            assert!(!valid_hhmm(t), "{t:?} should be rejected");
        }
    }

    #[test]
    fn apply_rejects_a_window_that_is_not_a_time_of_day() {
        // The messages are part of the HTTP contract; the SPA shows them
        // verbatim.
        assert_eq!(
            validate_apply("{ losos.cluster.computeWindow.start = \"24:00\"; }"),
            Err("computeWindowStart must be HH:MM, 00:00-23:59")
        );
        assert_eq!(
            validate_apply("{ losos.cluster.computeWindow.end = \"7:0\"; }"),
            Err("computeWindowEnd must be HH:MM, 00:00-23:59")
        );
    }

    #[test]
    fn apply_accepts_one_window_key_on_its_own() {
        // Each key is checked only when the body sets it, so moving the start
        // time must not require restating the end.
        assert!(validate_apply("{ losos.cluster.computeWindow.start = \"23:00\"; }").is_ok());
        assert!(validate_apply("{ losos.cluster.computeWindow.end = \"07:00\"; }").is_ok());
        // A window that wraps midnight is the default, not an error. One
        // assignment per line: `lookup_nix` splits on the *first* `=`, so two
        // assignments crammed onto one line are not a shape this parser reads
        // and not a shape the SPA ever generates.
        assert!(
            validate_apply(
                "{\n  losos.cluster.computeWindow.start = \"23:00\";\n  losos.cluster.computeWindow.end = \"07:00\";\n}\n"
            )
            .is_ok()
        );
    }

    #[test]
    fn assignments_lists_each_key_once_in_file_order() {
        let body = "{ ... }:\n{\n  losos.a = 1;\n  # losos.b = 2;\n  losos.c = \"x\"; # losos.d = 4\n  losos.a = 5;\n}\n";
        assert_eq!(
            assignments(body),
            vec![
                ("a".to_string(), "1".to_string()),
                ("c".to_string(), "\"x\"".to_string())
            ]
        );
    }

    #[test]
    fn describe_changes_names_changed_added_and_dropped_keys() {
        let old = "{\n  losos.hostName = \"a\";\n  losos.gpu.enable = true;\n  losos.x = 1;\n}";
        let new = "{\n  losos.hostName = \"b\";\n  losos.gpu.enable = true;\n  losos.y = 2;\n}";
        let changes = describe_changes(old, new);
        assert_eq!(
            changes,
            vec![
                Change {
                    key: "hostName".into(),
                    before: Some("\"a\"".into()),
                    after: Some("\"b\"".into())
                },
                Change {
                    key: "y".into(),
                    before: None,
                    after: Some("2".into())
                },
                Change {
                    key: "x".into(),
                    before: Some("1".into()),
                    after: None
                },
            ]
        );
        assert!(describe_changes(old, old).is_empty());
    }

    #[test]
    fn commit_messages_name_the_settings() {
        let changes = describe_changes(
            "{ losos.hostName = \"a\"; }",
            "{\n  losos.hostName = \"b\";\n  losos.cluster.enable = true;\n}",
        );
        let (subject, body) = commit_message("Change", &changes);
        assert_eq!(subject, "Change hostName, cluster.enable");
        assert_eq!(
            body,
            "losos.hostName: \"a\" -> \"b\"\nlosos.cluster.enable: (default) -> true"
        );
        let many: Vec<Change> = (0..5)
            .map(|i| Change {
                key: format!("k{i}"),
                before: None,
                after: Some("1".into()),
            })
            .collect();
        assert_eq!(
            commit_message("Change", &many).0,
            "Change k0, k1, k2 and 2 more"
        );
        assert_eq!(
            commit_message("Reset", &[]).0,
            "Reset settings (no assignment changed)"
        );
    }
}
