//! The edge's DNS zone, rendered as a master file for Knot.
//!
//! Like [`crate::config`], this is pure: the records go in, the exact bytes
//! come out, and the reconciler writes them only when they changed. Knot
//! (`services.knot` in `modules/edge.nix`) loads the file whole; a systemd
//! path unit tells it to reload when the registrar replaces it.
//!
//! What the zone carries:
//!
//! * the SOA and NS records, with the edge's addresses as glue for any name
//!   server inside the zone, and the same addresses at the apex;
//! * every whitelisted tenant hostname that falls inside the zone (so an edge
//!   whose zone is its whole `publicDomain` assigns `<name>.<domain>` itself
//!   rather than leaning on a hand-made wildcard);
//! * `<label>` for every box whose Stripe account vouches for it
//!   ([`crate::domains::Vouched`]): the name an owner's custom domain points
//!   at.
//!
//! Every name resolves to the edge, because the edge is where Traefik
//! terminates TLS and the tunnel to the box begins. A box's own address
//! never appears in public DNS.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use crate::domains::DomainsOpts;

/// TTL of every record. Short, so a box that loses its name loses it soon.
pub const TTL: u32 = 300;

/// The zone's records, apart from SOA, NS and glue: relative owner names
/// (labels under the zone), each resolving to the edge's addresses.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ZoneNames {
    pub names: BTreeSet<String>,
}

impl ZoneNames {
    /// Add `fqdn` if it lies strictly inside `zone`; returns whether it did.
    pub fn add_fqdn(&mut self, fqdn: &str, zone: &str) -> bool {
        let fqdn = fqdn.trim().trim_end_matches('.').to_ascii_lowercase();
        let Some(rel) = fqdn.strip_suffix(&format!(".{zone}")) else {
            return false;
        };
        if rel.is_empty() || !valid_relative(rel) {
            return false;
        }
        self.names.insert(rel.to_string());
        true
    }
}

/// A relative owner name a master file can carry unquoted.
fn valid_relative(rel: &str) -> bool {
    rel.split('.').all(|l| {
        !l.is_empty()
            && l.len() <= 63
            && l.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
    })
}

/// Render the zone. `serial` goes into the SOA; [`next_serial`] chooses it.
#[must_use]
pub fn render(opts: &DomainsOpts, names: &ZoneNames, serial: u32) -> String {
    let zone = &opts.zone;
    let mut out = String::new();
    let _ = writeln!(
        out,
        "; Written by losos-registrar. Do not edit: the next reconcile replaces it."
    );
    let _ = writeln!(out, "$ORIGIN {zone}.");
    let _ = writeln!(out, "$TTL {TTL}");
    let primary = opts
        .nameservers
        .first()
        .cloned()
        .unwrap_or_else(|| format!("ns1.{zone}"));
    let _ = writeln!(
        out,
        "@ IN SOA {primary}. {hm}. {serial} 3600 600 1209600 {TTL}",
        hm = opts.hostmaster.trim_end_matches('.'),
    );
    for ns in &opts.nameservers {
        let _ = writeln!(out, "@ IN NS {}.", ns.trim_end_matches('.'));
    }
    address_records(&mut out, "@", opts);

    // Glue for name servers inside the zone, then every box name. A set, so
    // a tenant hostname that happens to equal a name server is written once.
    let mut owners: BTreeSet<String> = BTreeSet::new();
    for ns in &opts.nameservers {
        let mut glue = ZoneNames::default();
        if glue.add_fqdn(ns, zone) {
            owners.extend(glue.names);
        }
    }
    owners.extend(names.names.iter().cloned());
    for owner in &owners {
        address_records(&mut out, owner, opts);
    }
    out
}

fn address_records(out: &mut String, owner: &str, opts: &DomainsOpts) {
    for a in &opts.ipv4 {
        let _ = writeln!(out, "{owner} IN A {a}");
    }
    for a in &opts.ipv6 {
        let _ = writeln!(out, "{owner} IN AAAA {a}");
    }
}

/// The serial in a zone this module rendered, if `text` is one.
#[must_use]
pub fn serial_of(text: &str) -> Option<u32> {
    let line = text.lines().find(|l| l.starts_with("@ IN SOA "))?;
    line.split_whitespace().nth(5)?.parse().ok()
}

/// Decide what to write: `None` when the zone on disk already says the same
/// thing, else the new text with a serial that is larger than the old one
/// and, where it can be, the current Unix time (Knot's own `unixtime`
/// policy), so a serial read off a resolver says when the zone last changed.
#[must_use]
pub fn next_zone(
    opts: &DomainsOpts,
    names: &ZoneNames,
    existing: Option<&str>,
    now: u64,
) -> Option<String> {
    let old = existing.and_then(serial_of);
    if let (Some(text), Some(serial)) = (existing, old) {
        if render(opts, names, serial) == text {
            return None;
        }
    }
    let now = u32::try_from(now).unwrap_or(u32::MAX);
    Some(render(opts, names, next_serial(old, now)))
}

/// Strictly larger than `old` (wrapping is not RFC 1982-safe to rely on
/// here, so it saturates), at least `now`.
#[must_use]
pub fn next_serial(old: Option<u32>, now: u32) -> u32 {
    match old {
        None => now.max(1),
        Some(o) => o.saturating_add(1).max(now),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> DomainsOpts {
        DomainsOpts {
            state_file: String::new(),
            zone: "boxes.losos.cfd".to_string(),
            zone_file: String::new(),
            nameservers: vec![
                "ns1.boxes.losos.cfd".to_string(),
                "ns.other.example".to_string(),
            ],
            hostmaster: "hostmaster.boxes.losos.cfd".to_string(),
            ipv4: vec!["203.0.113.7".parse().unwrap()],
            ipv6: vec!["2001:db8::7".parse().unwrap()],
            doh_url: String::new(),
            public_domain: "losos.cfd".to_string(),
        }
    }

    #[test]
    fn the_zone_carries_soa_ns_glue_and_one_name_per_box() {
        let mut names = ZoneNames::default();
        assert!(names.add_fqdn("3f9a1c0e7b2d4a55.boxes.losos.cfd", "boxes.losos.cfd"));
        assert!(names.add_fqdn("Mattbox.Boxes.losos.cfd.", "boxes.losos.cfd"));
        assert!(!names.add_fqdn("mattbox.losos.cfd", "boxes.losos.cfd"));
        assert!(!names.add_fqdn("boxes.losos.cfd", "boxes.losos.cfd"));
        assert!(!names.add_fqdn("evil;x.boxes.losos.cfd", "boxes.losos.cfd"));
        let z = render(&opts(), &names, 1_700_000_000);
        assert_eq!(
            z,
            "; Written by losos-registrar. Do not edit: the next reconcile replaces it.\n\
             $ORIGIN boxes.losos.cfd.\n\
             $TTL 300\n\
             @ IN SOA ns1.boxes.losos.cfd. hostmaster.boxes.losos.cfd. 1700000000 3600 600 1209600 300\n\
             @ IN NS ns1.boxes.losos.cfd.\n\
             @ IN NS ns.other.example.\n\
             @ IN A 203.0.113.7\n\
             @ IN AAAA 2001:db8::7\n\
             3f9a1c0e7b2d4a55 IN A 203.0.113.7\n\
             3f9a1c0e7b2d4a55 IN AAAA 2001:db8::7\n\
             mattbox IN A 203.0.113.7\n\
             mattbox IN AAAA 2001:db8::7\n\
             ns1 IN A 203.0.113.7\n\
             ns1 IN AAAA 2001:db8::7\n"
        );
        assert_eq!(serial_of(&z), Some(1_700_000_000));
    }

    #[test]
    fn an_unchanged_zone_is_not_rewritten_and_a_changed_one_gets_a_larger_serial() {
        let o = opts();
        let mut names = ZoneNames::default();
        names.add_fqdn("a.boxes.losos.cfd", &o.zone);
        let first = next_zone(&o, &names, None, 1000).unwrap();
        assert_eq!(serial_of(&first), Some(1000));
        assert_eq!(next_zone(&o, &names, Some(&first), 5000), None);
        names.add_fqdn("b.boxes.losos.cfd", &o.zone);
        // Clock behind the old serial (two changes in one second, or a clock
        // step back): still strictly larger.
        let second = next_zone(&o, &names, Some(&first), 900).unwrap();
        assert_eq!(serial_of(&second), Some(1001));
        let third = next_zone(&o, &ZoneNames::default(), Some(&second), 7000).unwrap();
        assert_eq!(serial_of(&third), Some(7000));
        // A hand-edited file with no readable serial is replaced.
        assert!(next_zone(&o, &names, Some("garbage"), 10).is_some());
        assert_eq!(next_serial(Some(u32::MAX), 5), u32::MAX);
    }
}
