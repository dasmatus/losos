//! Searching a public app catalogue, on the box, for the settings screen.
//!
//! # Why the box searches and not the browser
//!
//! The admin pages are served under `connect-src 'self'` (see the CSP in
//! `modules/containers.nix`), so the tab cannot fetch a catalogue directly —
//! the request is refused before it leaves the browser, and that is the policy
//! working rather than something to route around. lososd is the only thing on
//! this appliance with an outbound path, so the box searches and the page
//! renders what it found. `admin-ui/app/src/screens/settings/catalogue.ts`
//! wrote the contract down before either half existed; this module implements
//! that side of it, field for field.
//!
//! # Why `curl` rather than an HTTP client crate
//!
//! `backend/Cargo.toml` has no outbound client, and adding one is not a free
//! choice here: `flake/packages.nix` pins `cargoHash`, so a new dependency is a
//! new `Cargo.lock` and a hash only a `nix build` can produce. Shelling out
//! also keeps a TLS stack out of the one process that terminates untrusted
//! HTTP, and the daemon already runs external commands for every other
//! effectful thing it does (`lvextend`, `cryptsetup`, `nixos-rebuild`,
//! `crictl`). `modules/daemon.nix` puts `curl` on the unit's `path`, which
//! *replaces* PATH — so if that line is dropped this fails at the exec with
//! ENOENT, the same way `grow` would.
//!
//! # The shape below is from Artifact Hub's documented schema, not observed
//!
//! It could not be observed: the network policy of the environment this was
//! written in refuses `artifacthub.io`, so nothing here has ever seen a live
//! response. The parser is written to survive being wrong about it —
//! [`parse_results`] looks each field up under several plausible names, and a
//! row missing any of the three the UI requires is dropped rather than
//! guessed at. The failure mode of a schema change is therefore "no results",
//! never a panic and never a row this box cannot attribute.
//!
//! **If you have a box with network, check this first**: fetch the URL
//! [`search_url`] builds and compare it against `BODY` in this file's own test
//! module. That is the one assertion here a unit test cannot make for itself.
//!
//! # Shape
//!
//! The same split as `grow.rs` and `recovery.rs`: pure URL building, pure
//! validation and pure parsing, with the single effect ([`Fetch`]) behind a
//! trait so the command is tested with no network at all.

use serde::Serialize;
use serde_json::Value;

/// The catalogue this module searches, named on every response so the page can
/// tell the reader where the rows came from.
pub const SOURCE: &str = "Artifact Hub";

/// Artifact Hub's search endpoint.
const SEARCH_ENDPOINT: &str = "https://artifacthub.io/api/v1/packages/search";

/// Where a package's own page lives, for the `homepage` link on a row whose
/// entry carries no `home_url` of its own.
const PACKAGE_PAGE: &str = "https://artifacthub.io/packages/helm";

/// `kind=0` is Helm charts.
///
/// The only kind worth offering: this box runs a local k3s cluster
/// (`modules/cluster.nix`) and installs workloads into it, so an OPA policy or
/// a Falco rule is not something the owner could act on from this screen.
const KIND_HELM: u8 = 0;

/// Rows asked for. The screen shows a scrollable list, not a paged one, and a
/// person scanning for an app they already have in mind does not read past
/// twenty.
const LIMIT: usize = 20;

/// Shortest query worth sending. Matches the SPA's own `trimmed.length < 2`
/// guard, so a stray keystroke never reaches the network.
pub const MIN_QUERY_CHARS: usize = 2;

/// Longest query accepted. Well past any real search term; the point is that
/// an unbounded one becomes an unbounded URL.
pub const MAX_QUERY_CHARS: usize = 128;

/// Seconds `curl` may spend on the whole request.
///
/// The screen debounces at 350ms and the owner is watching a spinner, so this
/// is a bound on how long "no network" takes to say so, not a performance
/// knob. A box with no route out fails on connect long before it.
pub const TIMEOUT_SECS: u32 = 10;

/// One row, in exactly the shape `catalogue.ts` parses.
///
/// `summary`, `version` and `homepage` are `Option` because that file treats
/// them as optional (`nonEmptyString` yields `null`); `id`, `name` and `source`
/// are not, and a row that cannot supply all three never gets built.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct App {
    pub id: String,
    pub name: String,
    /// Who published it. Named on every row because it is the only thing there
    /// that tells the reader whose code they are about to run — a result with
    /// nowhere to attribute it is dropped rather than shown as though this box
    /// vouched for it.
    pub source: String,
    pub summary: Option<String>,
    pub version: Option<String>,
    pub homepage: Option<String>,
}

/// Reject a query before it becomes a URL.
///
/// Control characters are the one that matters: the value is interpolated into
/// a URL that becomes an argv, and a newline in an argv is how a shell-quoting
/// mistake turns into a second command. Nothing here builds a shell string —
/// [`Fetch`] execs `curl` directly with an argument vector — but the check
/// costs nothing and the property should not depend on that staying true.
pub fn validate_query(raw: &str) -> Result<&str, String> {
    let q = raw.trim();
    let chars = q.chars().count();
    if chars < MIN_QUERY_CHARS {
        return Err(format!("Search for at least {MIN_QUERY_CHARS} characters."));
    }
    if chars > MAX_QUERY_CHARS {
        return Err(format!(
            "That search is longer than {MAX_QUERY_CHARS} characters."
        ));
    }
    if q.chars().any(char::is_control) {
        return Err("Remove the line breaks and control characters.".to_string());
    }
    Ok(q)
}

/// Percent-encode everything outside RFC 3986's unreserved set.
///
/// Hand-written because this crate has no URL dependency and cannot grow one
/// (see the module docs). The rule is small enough to state and test in full:
/// `A-Z a-z 0-9 - _ . ~` survive, every other byte becomes `%XX` uppercase.
/// Encoding per *byte* rather than per `char` is what makes it correct for
/// non-ASCII — a multi-byte character becomes several escapes, which is what
/// the standard asks for.
#[must_use]
pub fn percent_encode(raw: &str) -> String {
    const UNRESERVED: &[u8] = b"-_.~";
    let mut out = String::with_capacity(raw.len());
    for b in raw.as_bytes() {
        if b.is_ascii_alphanumeric() || UNRESERVED.contains(b) {
            out.push(*b as char);
        } else {
            out.push('%');
            out.push_str(&format!("{b:02X}"));
        }
    }
    out
}

/// The URL a search for `query` fetches.
///
/// `facets=false` because nothing on this screen renders them and they are the
/// bulk of the response body.
#[must_use]
pub fn search_url(query: &str) -> String {
    format!(
        "{SEARCH_ENDPOINT}?kind={KIND_HELM}&limit={LIMIT}&facets=false&ts_query_web={}",
        percent_encode(query)
    )
}

/// First non-empty string among `keys`, trimmed.
///
/// The tolerance the module docs promise lives here: each field is looked up
/// under every name the upstream schema has plausibly used, so a rename
/// upstream costs a field rather than the row.
fn field<'a>(obj: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .filter_map(|k| obj.get(*k))
        .filter_map(Value::as_str)
        .map(str::trim)
        .find(|s| !s.is_empty())
}

/// Turn one entry of the upstream array into a row, or `None` to drop it.
fn parse_app(entry: &Value) -> Option<App> {
    let name = field(entry, &["display_name", "name", "normalized_name"])?;

    // Attribution, in order of how informative it is to a reader: the
    // publishing organisation, then the repository's own display name, then
    // its bare name. A row with none of the three is dropped — see `App`.
    let repo = entry.get("repository");
    let source = repo
        .and_then(|r| {
            field(
                r,
                &[
                    "organization_display_name",
                    "display_name",
                    "organization_name",
                    "name",
                ],
            )
        })
        .or_else(|| field(entry, &["repository_name"]))?;

    // A stable identity for the row. `package_id` when upstream gives one;
    // otherwise repository plus package name, which is unique within the
    // catalogue and is what the package page is addressed by anyway.
    let repo_name = repo.and_then(|r| field(r, &["name"]));
    let slug = field(entry, &["normalized_name", "name"]);
    let id = field(entry, &["package_id", "id"])
        .map(str::to_string)
        .or_else(|| Some(format!("{}/{}", repo_name?, slug?)))?;

    // Prefer the package's own home page. Falling back to its Artifact Hub
    // page rather than to nothing keeps every row clickable, and a reader who
    // wants to know what they are installing is better served by the catalogue
    // entry than by a dead link.
    let homepage = field(entry, &["home_url", "homepage"])
        .map(str::to_string)
        .or_else(|| Some(format!("{PACKAGE_PAGE}/{}/{}", repo_name?, slug?)));

    Some(App {
        id,
        name: name.to_string(),
        source: source.to_string(),
        summary: field(entry, &["description", "summary"]).map(str::to_string),
        version: field(entry, &["version", "app_version"]).map(str::to_string),
        homepage,
    })
}

/// Parse a search response body into rows.
///
/// An unparseable body is an error — that is the catalogue misbehaving and the
/// screen should say the search failed. A *parseable* body with nothing usable
/// in it is an empty list, which is an ordinary "no matches".
///
/// The array is looked for under `packages` and then `data`, and finally the
/// body is accepted as a bare array, for the same reason the field lookups are
/// plural.
pub fn parse_results(body: &str) -> anyhow::Result<Vec<App>> {
    let doc: Value = serde_json::from_str(body)
        .map_err(|e| anyhow::anyhow!("the catalogue did not return JSON: {e}"))?;
    let entries = doc
        .get("packages")
        .or_else(|| doc.get("data"))
        .or(Some(&doc))
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    Ok(entries.iter().filter_map(parse_app).collect())
}

/// The one effect: fetch a URL and hand back its body.
///
/// Behind a trait so [`crate::losos::cmd_apps_search`] is tested against a
/// recorded body with no network, exactly as `CodeStore` does for the recovery
/// code. An `Err` is a fetch that could not happen or did not succeed — the
/// command turns it into a message the screen shows and offers to retry.
pub trait Fetch {
    fn get(&mut self, url: &str) -> anyhow::Result<String>;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A response in the shape Artifact Hub documents. The caveat in the
    /// module docs applies: this was written from the schema, not recorded off
    /// the wire.
    const BODY: &str = r#"{
      "packages": [
        {
          "package_id": "a1b2",
          "name": "nextcloud",
          "normalized_name": "nextcloud",
          "display_name": "Nextcloud",
          "description": "A safe home for all your data",
          "version": "6.6.10",
          "app_version": "31.0.5",
          "repository": {
            "name": "nextcloud",
            "display_name": "Nextcloud",
            "organization_display_name": "Nextcloud GmbH",
            "url": "https://nextcloud.github.io/helm/"
          }
        }
      ]
    }"#;

    #[test]
    fn a_documented_response_maps_onto_the_contract_the_spa_parses() {
        let apps = parse_results(BODY).unwrap();
        assert_eq!(apps.len(), 1);
        let a = &apps[0];
        assert_eq!(a.id, "a1b2");
        assert_eq!(a.name, "Nextcloud");
        // The publishing organisation, not the catalogue: the row has to say
        // whose code it is.
        assert_eq!(a.source, "Nextcloud GmbH");
        assert_eq!(a.summary.as_deref(), Some("A safe home for all your data"));
        assert_eq!(a.version.as_deref(), Some("6.6.10"));
        assert_eq!(
            a.homepage.as_deref(),
            Some("https://artifacthub.io/packages/helm/nextcloud/nextcloud")
        );
    }

    #[test]
    fn a_row_that_cannot_be_attributed_is_dropped_not_shown() {
        let body = r#"{"packages":[{"name":"orphan","normalized_name":"orphan"}]}"#;
        assert!(parse_results(body).unwrap().is_empty());
    }

    #[test]
    fn a_row_with_no_name_is_dropped() {
        let body = r#"{"packages":[{"repository":{"name":"r","display_name":"R"}}]}"#;
        assert!(parse_results(body).unwrap().is_empty());
    }

    #[test]
    fn the_packages_own_home_url_wins_over_the_catalogue_page() {
        let body = r#"{"packages":[{"name":"n","normalized_name":"n","home_url":"https://example.org/",
          "repository":{"name":"r","display_name":"R"}}]}"#;
        let apps = parse_results(body).unwrap();
        assert_eq!(apps[0].homepage.as_deref(), Some("https://example.org/"));
    }

    /// The tolerance is the point: an upstream that renames the array or the
    /// attribution field costs a field or a fallback, never a panic.
    #[test]
    fn an_unexpected_envelope_still_yields_rows() {
        let body = r#"[{"name":"n","normalized_name":"n","repository":{"name":"r"}}]"#;
        let apps = parse_results(body).unwrap();
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].source, "r");
    }

    #[test]
    fn a_parseable_body_with_no_matches_is_empty_not_an_error() {
        assert!(parse_results(r#"{"packages":[]}"#).unwrap().is_empty());
    }

    #[test]
    fn a_body_that_is_not_json_is_an_error_the_screen_can_retry() {
        assert!(parse_results("<html>502 Bad Gateway</html>").is_err());
    }

    #[test]
    fn empty_strings_upstream_are_absent_fields_not_empty_ones() {
        // `nonEmptyString` on the SPA side turns "" into null; the box must not
        // send "" and call it a summary.
        let body = r#"{"packages":[{"name":"n","normalized_name":"n","description":"  ",
          "version":"","repository":{"name":"r"}}]}"#;
        let apps = parse_results(body).unwrap();
        assert_eq!(apps[0].summary, None);
        assert_eq!(apps[0].version, None);
    }

    #[test]
    fn the_query_is_percent_encoded_per_byte() {
        assert_eq!(percent_encode("nextcloud"), "nextcloud");
        assert_eq!(percent_encode("a b&c=d"), "a%20b%26c%3Dd");
        assert_eq!(percent_encode("-_.~"), "-_.~");
        // Two bytes in, two escapes out.
        assert_eq!(percent_encode("é"), "%C3%A9");
    }

    #[test]
    fn the_url_carries_the_encoded_query_and_nothing_unescaped() {
        let url = search_url("file sync");
        assert!(url.contains("ts_query_web=file%20sync"), "{url}");
        assert!(url.starts_with(SEARCH_ENDPOINT));
        assert!(!url.contains(' '));
    }

    #[test]
    fn queries_too_short_too_long_or_carrying_control_characters_are_refused() {
        assert!(validate_query("n").is_err());
        assert!(validate_query(&"n".repeat(MAX_QUERY_CHARS + 1)).is_err());
        assert!(validate_query("next\ncloud").is_err());
        assert_eq!(validate_query("  nextcloud  ").unwrap(), "nextcloud");
    }
}
