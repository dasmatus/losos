//! Who may call which route: `backend/sections.json`, compiled in.
//!
//! The admin pages are split into sections (storage, backup, apps, ...),
//! and each section is an Authelia group, `losos-<section>`. A route belongs
//! to one section, or is `public` (no sign-in at all, and never reaches
//! [`crate::http`]'s guard), `internal` (asked by nginx alone, for its own
//! decisions, and never proxied to a client), `signed-in` (anyone with an account) or
//! `admin`. `losos-admins` may do everything, `losos-viewers` may read every
//! named section, and a route the table does not name is `admin`, so a new
//! route is closed until someone puts it in a section.
//!
//! `modules/identity.nix` reads the same file for the public routes nginx
//! lets through without asking Authelia, so the two cannot disagree.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::people::Person;

const TABLE: &str = include_str!("../sections.json");

#[derive(Debug, Deserialize)]
struct Table {
    viewers: String,
    sections: Vec<String>,
    routes: BTreeMap<String, String>,
}

fn table() -> &'static Table {
    static T: OnceLock<Table> = OnceLock::new();
    T.get_or_init(|| serde_json::from_str(TABLE).expect("backend/sections.json parses"))
}

/// The section a route is in: `public`, `signed-in`, `admin` or a name.
pub fn section(method: &str, pattern: &str) -> &'static str {
    table()
        .routes
        .get(&format!("{} {pattern}", method.to_ascii_uppercase()))
        .map_or("admin", String::as_str)
}

/// Whether `person` may call `method` on a route in `section`.
pub fn allows(person: &Person, method: &str, section: &str) -> bool {
    if person.is_admin() {
        return true;
    }
    match section {
        "public" | "signed-in" => true,
        "admin" | "internal" => false,
        named => {
            let group = format!("losos-{named}");
            person.groups.contains(&group)
                || (method.eq_ignore_ascii_case("GET")
                    && person.groups.iter().any(|g| *g == table().viewers))
        }
    }
}

/// The named sections `person` can use, for `GET /api/me`: the admin UI
/// shows the panes these open and hides the rest.
pub fn usable(person: &Person) -> Vec<&'static str> {
    table()
        .sections
        .iter()
        .map(String::as_str)
        .filter(|s| allows(person, "GET", s))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::people::ADMINS;

    fn person(groups: &[&str]) -> Person {
        Person {
            name: "eva".into(),
            display_name: "Eva".into(),
            groups: groups.iter().map(ToString::to_string).collect(),
        }
    }

    #[test]
    fn every_route_lososd_serves_is_in_the_table_and_nothing_else_is() {
        let source = include_str!("http.rs");
        let mut served = std::collections::BTreeSet::new();
        // `.route("/api/x", web::get()...` across line breaks.
        let flat: String = source.split_whitespace().collect::<Vec<_>>().join(" ");
        for piece in flat.split(".route(").skip(1) {
            let Some(rest) = piece.trim_start().strip_prefix('"') else {
                continue;
            };
            let Some((path, after)) = rest.split_once('"') else {
                continue;
            };
            let Some(method) = after
                .trim_start_matches([',', ' '])
                .strip_prefix("web::")
                .and_then(|m| m.split_once('('))
                .map(|(m, _)| m.to_ascii_uppercase())
            else {
                continue;
            };
            served.insert(format!("{method} {path}"));
        }
        assert!(served.len() > 50, "found only {served:?}");
        let listed: std::collections::BTreeSet<String> = table().routes.keys().cloned().collect();
        let missing: Vec<_> = served.difference(&listed).collect();
        let stale: Vec<_> = listed.difference(&served).collect();
        assert!(missing.is_empty(), "routes with no section: {missing:?}");
        assert!(
            stale.is_empty(),
            "sections.json names routes nobody serves: {stale:?}"
        );
    }

    #[test]
    fn every_section_named_by_a_route_is_declared() {
        for (route, s) in &table().routes {
            assert!(
                ["public", "internal", "signed-in", "admin"].contains(&s.as_str())
                    || table().sections.contains(s),
                "{route} is in an undeclared section {s}"
            );
        }
    }

    #[test]
    fn an_unknown_route_is_for_admins_only() {
        assert_eq!(section("GET", "/api/brand-new"), "admin");
        assert!(!allows(&person(&["losos-storage"]), "GET", "admin"));
        assert!(allows(&person(&[ADMINS]), "POST", "admin"));
    }

    #[test]
    fn a_section_group_opens_its_section_and_no_other() {
        let p = person(&["losos-backup"]);
        assert!(allows(&p, "POST", section("POST", "/api/backup/run")));
        assert!(!allows(&p, "POST", section("POST", "/api/apps/install")));
        assert!(!allows(&p, "POST", section("POST", "/api/backup/restore")));
        assert!(!allows(&p, "GET", section("GET", "/api/recovery")));
        assert_eq!(usable(&p), vec!["backup"]);
    }

    #[test]
    fn viewers_read_every_section_and_change_nothing() {
        let p = person(&["losos-viewers"]);
        assert!(allows(&p, "GET", section("GET", "/api/storage")));
        assert!(!allows(&p, "POST", section("POST", "/api/grow")));
        assert!(!allows(&p, "GET", section("GET", "/api/recovery")));
        assert_eq!(usable(&p).len(), table().sections.len());
    }

    #[test]
    fn anyone_signed_in_sees_the_overview_and_changes_their_password() {
        let p = person(&[]);
        assert!(allows(&p, "GET", section("GET", "/api/state")));
        assert!(allows(&p, "POST", section("POST", "/api/set-password")));
        assert!(usable(&p).is_empty());
    }
}
