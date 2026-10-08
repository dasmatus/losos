//! Custom domains on an official edge.
//!
//! A box behind the edge is reached at the hostname the operator gave it
//! (`<name>.<publicDomain>`). This module lets its owner bring a domain of
//! their own (`cloud.example.org`) as well, and gives the box a second,
//! stable name inside a zone the edge itself serves, which is what the
//! owner's domain points at.
//!
//! # What the edge publishes, and why from Stripe data
//!
//! The edge's zone (`--dns-zone`, e.g. `boxes.losos.cfd`, rendered by
//! [`crate::zone`] and served by Knot on the edge) carries one name per box:
//! [`box_label`] of the box's UUID. That UUID is the one-way derivative of
//! the recovery code that the box wrote onto its Stripe connected account
//! when it onboarded (`backend/src/boxid.rs`); the edge has it from its
//! market store. The name is published only while that account is *ready*
//! (Stripe has checked the person behind it: details submitted, payouts
//! enabled, transfers active) and carries that box UUID. So a public name the
//! edge hands out always traces back to a person Stripe has identified, and
//! a box nobody has vouched for gets no name and no custom domain.
//!
//! None of the Stripe data is published as such. The account id appears only
//! inside [`challenge_token`], a hash the owner copies into their own zone.
//!
//! # Proving a domain
//!
//! The owner adds two records at their DNS provider:
//!
//! * `_losos-challenge.<domain> TXT "losos-domain-v1=<token>"`, where the
//!   token binds the domain, the box UUID and the Stripe account id. Only the
//!   box's owner can read it (it comes back on their own authenticated
//!   `/domains/list`), and it changes if any of the three changes, so a TXT
//!   record left behind by an earlier owner or another box proves nothing.
//! * `<domain> CNAME <label>.<zone>` (or, at a zone apex, A/AAAA records with
//!   the edge's addresses, which [`DomainsView::addresses`] lists).
//!
//! The reconciler looks both up over DNS-over-HTTPS ([`Doh`]) and the domain
//! goes live, i.e. gets a Traefik router and a Let's Encrypt certificate,
//! only once both are seen. Waiting for the second record too keeps Traefik
//! from asking Let's Encrypt for a name that does not reach this edge, which
//! would fail and spend the account's failed-validation allowance.
//!
//! A live domain is looked up again every [`LIVE_RECHECK_SECS`]. A record that
//! has gone away takes it offline after [`LAPSE_AFTER`] failed checks in a row
//! (one DNS hiccup must not take a website down); a Stripe account that is no
//! longer ready takes it offline at once, because that is local data and not
//! a guess.

use std::collections::BTreeMap;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::path::PathBuf;
use std::time::Duration;

use ring::digest::{digest, SHA256};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;

use crate::fsutil::atomic_write;

/// `domains.json` holds tenant ids and domain names, nothing secret, but no
/// one else on the edge needs to read it.
const STORE_FILE_MODE: u32 = 0o600;

/// Domains one box may claim, live or waiting.
pub const MAX_PER_TENANT: usize = 5;
/// Claims the whole edge keeps. Each one costs a few DNS lookups a minute
/// while it waits, so the store is bounded like everything else here.
pub const MAX_CLAIMS: usize = 2000;
/// A claim that never went live is forgotten after a week.
pub const PENDING_TTL_SECS: u64 = 7 * 24 * 3600;
/// How often a waiting claim is looked up.
pub const PENDING_RECHECK_SECS: u64 = 30;
/// How often a live claim is looked up again.
pub const LIVE_RECHECK_SECS: u64 = 3600;
/// Failed checks in a row that take a live domain offline.
pub const LAPSE_AFTER: u32 = 3;
/// Claims looked up per reconciler pass. Each costs three DNS-over-HTTPS
/// requests; the rest wait for the next pass.
pub const CHECKS_PER_PASS: usize = 16;
/// One DNS-over-HTTPS request, end to end.
pub const DOH_TIMEOUT: Duration = Duration::from_secs(5);
/// The record the owner publishes to prove the domain is theirs.
pub const CHALLENGE_LABEL: &str = "_losos-challenge";
/// Prefix of the TXT value; bump together with [`challenge_token`]'s domain
/// separation string if the token ever changes shape.
pub const CHALLENGE_PREFIX: &str = "losos-domain-v1=";
/// Default DNS-over-HTTPS endpoint (JSON API).
pub const DEFAULT_DOH_URL: &str = "https://cloudflare-dns.com/dns-query";

/// Everything `serve --dns-zone ...` was given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainsOpts {
    /// `domains.json`.
    pub state_file: String,
    /// The zone the edge is authoritative for, no trailing dot.
    pub zone: String,
    /// Where the rendered zone file goes; Knot loads it from there.
    pub zone_file: String,
    /// The zone's NS names, no trailing dot. A name inside the zone gets the
    /// edge's addresses as glue.
    pub nameservers: Vec<String>,
    /// SOA RNAME as a domain (`hostmaster.boxes.losos.cfd`).
    pub hostmaster: String,
    /// The edge's public addresses: what every box name resolves to.
    pub ipv4: Vec<Ipv4Addr>,
    pub ipv6: Vec<Ipv6Addr>,
    /// DNS-over-HTTPS (JSON) endpoint used to look the owner's records up.
    pub doh_url: String,
    /// The edge's own apex. Nobody may claim a name under it or the zone.
    pub public_domain: String,
}

impl DomainsOpts {
    /// `<label>.<zone>` for a box UUID.
    #[must_use]
    pub fn target_for(&self, box_uuid: &str) -> String {
        format!("{}.{}", box_label(box_uuid), self.zone)
    }
}

/// The name a box gets in the edge's zone: 16 hex characters of a SHA-256 of
/// its UUID. Stable for the life of the installation, short enough to type,
/// and not the UUID itself, which also sits on the Stripe account.
#[must_use]
pub fn box_label(box_uuid: &str) -> String {
    let d = digest(
        &SHA256,
        format!("losos-box-dns-v1\n{box_uuid}\n").as_bytes(),
    );
    hex(&d.as_ref()[..8])
}

/// The value of the owner's TXT record: binds the domain, the box UUID and
/// the Stripe account, opaque to anyone who reads the record.
#[must_use]
pub fn challenge_token(domain: &str, box_uuid: &str, account_id: &str) -> String {
    let d = digest(
        &SHA256,
        format!("losos-domain-v1\n{domain}\n{box_uuid}\n{account_id}\n").as_bytes(),
    );
    format!("{CHALLENGE_PREFIX}{}", hex(&d.as_ref()[..16]))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Lowercase, strip one trailing dot, and check `s` is a hostname a stranger
/// may route here: ASCII letters, digits and hyphens (an internationalised
/// name in its `xn--` form), at least two labels, no label over 63, 253 in
/// all, a top-level label that is not all digits (no IP addresses), and
/// nothing under the edge's own names.
///
/// # Errors
/// A sentence for the owner.
pub fn normalize_domain(raw: &str, opts: &DomainsOpts) -> Result<String, DomainError> {
    let s = raw.trim().to_ascii_lowercase();
    let s = s.strip_suffix('.').unwrap_or(&s).to_string();
    if s.is_empty() {
        return Err(DomainError::Invalid("type a domain name"));
    }
    if !s.is_ascii() {
        return Err(DomainError::Invalid(
            "write an internationalised name in its xn-- form",
        ));
    }
    if s.len() > 253 {
        return Err(DomainError::Invalid(
            "that name is longer than 253 characters",
        ));
    }
    let labels: Vec<&str> = s.split('.').collect();
    if labels.len() < 2 {
        return Err(DomainError::Invalid(
            "use a full domain name, such as cloud.example.org",
        ));
    }
    for label in &labels {
        let ok = !label.is_empty()
            && label.len() <= 63
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        if !ok {
            return Err(DomainError::Invalid(
                "a domain name has only letters, digits, hyphens and dots",
            ));
        }
    }
    if labels
        .last()
        .is_some_and(|tld| tld.bytes().all(|b| b.is_ascii_digit()))
    {
        return Err(DomainError::Invalid("an IP address is not a domain name"));
    }
    for own in [&opts.public_domain, &opts.zone] {
        if !own.is_empty() && (s == **own || s.ends_with(&format!(".{own}"))) {
            return Err(DomainError::Invalid(
                "that name belongs to the edge; use a domain of your own",
            ));
        }
    }
    Ok(s)
}

/// Why a claim is not live, as the owner should read it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Problem {
    /// The box has no ready Stripe account carrying its UUID.
    StripeAccount,
    /// No TXT record at `_losos-challenge.<domain>`.
    TxtMissing,
    /// A TXT record is there, but not this box's token.
    TxtWrong,
    /// The domain does not resolve to this edge.
    NotPointing,
    /// Another box already has this domain live.
    TakenElsewhere,
    /// The DNS lookup itself failed; it is retried.
    LookupFailed,
}

/// One box's claim on one domain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claim {
    pub domain: String,
    /// The tenant (appliance id) that made the claim.
    pub tenant: String,
    pub created_at: u64,
    #[serde(default)]
    pub checked_at: Option<u64>,
    /// Set while the domain is live (routed). Cleared when it lapses.
    #[serde(default)]
    pub verified_at: Option<u64>,
    /// Failed checks in a row since the domain went live.
    #[serde(default)]
    pub failures: u32,
    #[serde(default)]
    pub txt_found: bool,
    #[serde(default)]
    pub points_here: bool,
    #[serde(default)]
    pub problem: Option<Problem>,
}

impl Claim {
    #[must_use]
    pub fn live(&self) -> bool {
        self.verified_at.is_some()
    }

    /// Whether the reconciler should look this claim up now.
    #[must_use]
    pub fn due(&self, now: u64) -> bool {
        let every = if self.live() {
            LIVE_RECHECK_SECS
        } else {
            PENDING_RECHECK_SECS
        };
        self.checked_at
            .is_none_or(|t| now.saturating_sub(t) >= every)
    }
}

/// Everything persisted in `domains.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomainsState {
    #[serde(default)]
    pub claims: Vec<Claim>,
}

/// What the box's Stripe account looks like to the edge, reduced to the two
/// facts this module uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vouched {
    pub box_uuid: String,
    pub account_id: String,
}

impl Vouched {
    /// `Some` only for a ready account that carries a well-formed box UUID.
    #[must_use]
    pub fn from_seller(seller: Option<&crate::market::Seller>) -> Option<Self> {
        let s = seller?;
        let uuid = s.box_uuid.as_deref()?;
        (s.ready && crate::market::valid_box_uuid(uuid)).then(|| Self {
            box_uuid: uuid.to_string(),
            account_id: s.account_id.clone(),
        })
    }
}

/// What one DNS-over-HTTPS round found for a claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observed {
    /// TXT strings at `_losos-challenge.<domain>`, or why the lookup failed.
    pub txt: Result<Vec<String>, String>,
    /// `(type, data)` answers for the domain's A and AAAA queries, CNAME
    /// steps included, or why a lookup failed.
    pub addrs: Result<Vec<(u16, String)>, String>,
}

/// The pure half of a check: given what DNS said, the claim's next state.
///
/// `vouched` is the box's Stripe data right now (`None`: not ready),
/// `taken_elsewhere` whether another tenant has this domain live.
#[must_use]
pub fn evaluate(
    claim: &Claim,
    vouched: Option<&Vouched>,
    opts: &DomainsOpts,
    taken_elsewhere: bool,
    observed: &Observed,
    now: u64,
) -> Claim {
    let mut next = claim.clone();
    next.checked_at = Some(now);

    let Some(v) = vouched else {
        // Local data, not a guess: offline at once.
        next.verified_at = None;
        next.failures = 0;
        next.txt_found = false;
        next.points_here = false;
        next.problem = Some(Problem::StripeAccount);
        return next;
    };
    if taken_elsewhere {
        next.verified_at = None;
        next.failures = 0;
        next.problem = Some(Problem::TakenElsewhere);
        return next;
    }

    let expected = challenge_token(&claim.domain, &v.box_uuid, &v.account_id);
    let target = opts.target_for(&v.box_uuid);

    let txt = observed.txt.as_ref().map(|records| {
        if records.iter().any(|r| r.trim() == expected) {
            (true, None)
        } else if records
            .iter()
            .any(|r| r.trim().starts_with(CHALLENGE_PREFIX))
        {
            (false, Some(Problem::TxtWrong))
        } else {
            (false, Some(Problem::TxtMissing))
        }
    });
    let points = observed
        .addrs
        .as_ref()
        .map(|answers| points_here(answers, &target, opts));

    let problem = match (&txt, &points) {
        (Err(_), _) | (_, Err(_)) => Some(Problem::LookupFailed),
        (Ok((_, Some(p))), _) => Some(*p),
        (Ok((true, None)), Ok(false)) => Some(Problem::NotPointing),
        (Ok((true, None)), Ok(true)) => None,
        (Ok((false, None)), _) => Some(Problem::TxtMissing),
    };
    if let Ok((found, _)) = txt {
        next.txt_found = found;
    }
    if let Ok(p) = points {
        next.points_here = p;
    }
    next.problem = problem;

    match (problem, claim.live()) {
        (None, false) => {
            next.verified_at = Some(now);
            next.failures = 0;
        }
        (None, true) => next.failures = 0,
        (Some(_), false) => {}
        (Some(_), true) => {
            next.failures = claim.failures.saturating_add(1);
            if next.failures >= LAPSE_AFTER {
                next.verified_at = None;
                next.failures = 0;
            }
        }
    }
    next
}

/// Whether DNS answers for the domain lead here: a CNAME step to the box's
/// target name, or an address that is one of the edge's.
#[must_use]
pub fn points_here(answers: &[(u16, String)], target: &str, opts: &DomainsOpts) -> bool {
    answers.iter().any(|(rtype, data)| {
        let data = data.trim().trim_end_matches('.').to_ascii_lowercase();
        match *rtype {
            5 => data == target,
            1 => data
                .parse::<Ipv4Addr>()
                .is_ok_and(|a| opts.ipv4.contains(&a)),
            28 => data
                .parse::<Ipv6Addr>()
                .is_ok_and(|a| opts.ipv6.contains(&a)),
            _ => false,
        }
    })
}

/// Failures of the domain routes.
#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    /// The edge serves no zone, or is not official.
    #[error("custom domains are not offered by this edge")]
    Unconfigured,
    #[error("{0}")]
    Invalid(&'static str),
    #[error("{0}")]
    Conflict(&'static str),
    #[error("no such domain on this box")]
    NotFound,
    #[error("domains store: {0}")]
    Store(String),
}

impl From<std::io::Error> for DomainError {
    fn from(e: std::io::Error) -> Self {
        DomainError::Store(e.to_string())
    }
}

impl From<serde_json::Error> for DomainError {
    fn from(e: serde_json::Error) -> Self {
        DomainError::Store(e.to_string())
    }
}

/// One domain as its own box sees it.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DomainView {
    pub domain: String,
    /// `"live"` or `"waiting"`.
    pub status: &'static str,
    /// `_losos-challenge.<domain>`.
    pub txt_name: String,
    /// The TXT value to publish; `None` while the box has no ready Stripe
    /// account, since the token is made from it.
    pub txt_value: Option<String>,
    pub txt_found: bool,
    pub points_here: bool,
    pub problem: Option<Problem>,
    pub checked_at: Option<u64>,
    pub verified_at: Option<u64>,
}

/// `POST /domains/list`, and the answer to every change.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DomainsView {
    /// Whether this box may add a domain: its Stripe account is ready and
    /// carries its UUID.
    pub eligible: bool,
    /// Why not, when not.
    pub reason: Option<Problem>,
    /// `<label>.<zone>`: what a CNAME points at. `None` until eligible.
    pub target: Option<String>,
    /// The edge's addresses, for a domain at a zone apex, where a CNAME is
    /// not allowed.
    pub addresses: Vec<String>,
    pub max_domains: usize,
    pub domains: Vec<DomainView>,
}

/// Build a box's view. Pure.
#[must_use]
pub fn view(
    state: &DomainsState,
    tenant: &str,
    vouched: Option<&Vouched>,
    opts: &DomainsOpts,
) -> DomainsView {
    let addresses = opts
        .ipv4
        .iter()
        .map(ToString::to_string)
        .chain(opts.ipv6.iter().map(ToString::to_string))
        .collect();
    let mut domains: Vec<DomainView> = state
        .claims
        .iter()
        .filter(|c| c.tenant == tenant)
        .map(|c| DomainView {
            domain: c.domain.clone(),
            status: if c.live() { "live" } else { "waiting" },
            txt_name: format!("{CHALLENGE_LABEL}.{}", c.domain),
            txt_value: vouched.map(|v| challenge_token(&c.domain, &v.box_uuid, &v.account_id)),
            txt_found: c.txt_found,
            points_here: c.points_here,
            problem: if vouched.is_none() {
                Some(Problem::StripeAccount)
            } else {
                c.problem
            },
            checked_at: c.checked_at,
            verified_at: c.verified_at,
        })
        .collect();
    domains.sort_by(|a, b| a.domain.cmp(&b.domain));
    DomainsView {
        eligible: vouched.is_some(),
        reason: vouched.is_none().then_some(Problem::StripeAccount),
        target: vouched.map(|v| opts.target_for(&v.box_uuid)),
        addresses,
        max_domains: MAX_PER_TENANT,
        domains,
    }
}

/// Add a claim. Pure; the caller persists.
///
/// # Errors
/// [`DomainError::Conflict`] for a full box, a full edge, or a domain another
/// box has live; nothing for a domain this box already claimed (idempotent).
pub fn add(
    state: &DomainsState,
    tenant: &str,
    domain: &str,
    now: u64,
) -> Result<DomainsState, DomainError> {
    let mut next = state.clone();
    if next
        .claims
        .iter()
        .any(|c| c.tenant == tenant && c.domain == domain)
    {
        return Ok(next);
    }
    if next
        .claims
        .iter()
        .any(|c| c.domain == domain && c.tenant != tenant && c.live())
    {
        return Err(DomainError::Conflict(
            "another box already serves that domain through this edge",
        ));
    }
    if next.claims.iter().filter(|c| c.tenant == tenant).count() >= MAX_PER_TENANT {
        return Err(DomainError::Conflict(
            "this box already has the most domains it may have; remove one first",
        ));
    }
    if next.claims.len() >= MAX_CLAIMS {
        return Err(DomainError::Conflict(
            "this edge takes no more domains right now; try again later",
        ));
    }
    next.claims.push(Claim {
        domain: domain.to_string(),
        tenant: tenant.to_string(),
        created_at: now,
        checked_at: None,
        verified_at: None,
        failures: 0,
        txt_found: false,
        points_here: false,
        problem: None,
    });
    Ok(next)
}

/// Remove a claim. Pure.
///
/// # Errors
/// [`DomainError::NotFound`] when this box has no claim on `domain`.
pub fn remove(
    state: &DomainsState,
    tenant: &str,
    domain: &str,
) -> Result<DomainsState, DomainError> {
    let mut next = state.clone();
    let before = next.claims.len();
    next.claims
        .retain(|c| !(c.tenant == tenant && c.domain == domain));
    if next.claims.len() == before {
        return Err(DomainError::NotFound);
    }
    Ok(next)
}

/// Drop claims of tenants the operator removed, and waiting claims past
/// [`PENDING_TTL_SECS`]. Pure; returns whether anything went.
pub fn prune(state: &mut DomainsState, tenant_exists: impl Fn(&str) -> bool, now: u64) -> bool {
    let before = state.claims.len();
    state.claims.retain(|c| {
        tenant_exists(&c.tenant)
            && (c.live() || now.saturating_sub(c.created_at) < PENDING_TTL_SECS)
    });
    state.claims.len() != before
}

/// Live `(tenant, domain)` pairs, each domain once: if two tenants somehow
/// both hold it live (a hand-edited store), the earlier verification wins.
#[must_use]
pub fn live_routes(state: &DomainsState) -> BTreeMap<String, Vec<String>> {
    let mut winners: BTreeMap<&str, &Claim> = BTreeMap::new();
    for c in state.claims.iter().filter(|c| c.live()) {
        winners
            .entry(c.domain.as_str())
            .and_modify(|w| {
                if c.verified_at < w.verified_at {
                    *w = c;
                }
            })
            .or_insert(c);
    }
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (domain, c) in winners {
        out.entry(c.tenant.clone())
            .or_default()
            .push(domain.to_string());
    }
    out
}

/// The persisted store.
pub struct Domains {
    pub opts: DomainsOpts,
    path: PathBuf,
    state: Mutex<DomainsState>,
}

impl Domains {
    /// Open the store. Missing is empty; unparseable is an error, like the
    /// market's: starting empty would silently take every live domain down.
    ///
    /// # Errors
    /// A store that exists and does not parse.
    pub async fn open(opts: DomainsOpts) -> Result<Self, DomainError> {
        let path = PathBuf::from(&opts.state_file);
        let state = match tokio::fs::read(&path).await {
            Ok(bytes) => serde_json::from_slice(&bytes)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => DomainsState::default(),
            Err(e) => return Err(e.into()),
        };
        Ok(Self {
            opts,
            path,
            state: Mutex::new(state),
        })
    }

    pub async fn snapshot(&self) -> DomainsState {
        self.state.lock().await.clone()
    }

    /// Apply `change` to the state and make it live only once it is on disk.
    ///
    /// # Errors
    /// Whatever `change` refuses, or a failed write.
    pub async fn update<F>(&self, change: F) -> Result<DomainsState, DomainError>
    where
        F: FnOnce(&DomainsState) -> Result<DomainsState, DomainError>,
    {
        let mut live = self.state.lock().await;
        let next = change(&live)?;
        if next != *live {
            let bytes = serde_json::to_vec_pretty(&next)?;
            atomic_write(&self.path, &bytes, STORE_FILE_MODE).await?;
            *live = next.clone();
        }
        Ok(next)
    }
}

/// A DNS-over-HTTPS (JSON API) client: `GET <url>?name=<n>&type=<T>` with
/// `accept: application/dns-json`, as Cloudflare, Google and Quad9 answer it.
#[derive(Clone)]
pub struct Doh {
    client: reqwest::Client,
    url: String,
}

impl Doh {
    /// # Errors
    /// The HTTP client could not be built.
    pub fn new(url: &str) -> Result<Self, reqwest::Error> {
        Ok(Self {
            client: reqwest::Client::builder()
                .timeout(DOH_TIMEOUT)
                .user_agent(concat!("losos-registrar/", env!("CARGO_PKG_VERSION")))
                .build()?,
            url: url.trim_end_matches('/').to_string(),
        })
    }

    /// `(type, data)` for every answer. NXDOMAIN and NODATA are an empty
    /// list, not an error: "no record" is an answer.
    ///
    /// # Errors
    /// A transport failure, a non-200, a body that is not the JSON API, or a
    /// resolver status other than NOERROR/NXDOMAIN (SERVFAIL, REFUSED).
    pub async fn query(&self, name: &str, rtype: &str) -> Result<Vec<(u16, String)>, String> {
        let resp = self
            .client
            .get(&self.url)
            .query(&[("name", name), ("type", rtype)])
            .header("accept", "application/dns-json")
            .send()
            .await
            .map_err(|e| format!("DNS-over-HTTPS request failed: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("DNS-over-HTTPS answered {}", resp.status()));
        }
        let body: Value = resp
            .json()
            .await
            .map_err(|e| format!("DNS-over-HTTPS answer is not JSON: {e}"))?;
        parse_doh(&body)
    }

    /// Run the three lookups one claim needs.
    pub async fn observe(&self, domain: &str) -> Observed {
        let txt = self
            .query(&format!("{CHALLENGE_LABEL}.{domain}"), "TXT")
            .await
            .map(|answers| {
                answers
                    .into_iter()
                    .filter(|(t, _)| *t == 16)
                    .map(|(_, data)| txt_text(&data))
                    .collect()
            });
        let a = self.query(domain, "A").await;
        let aaaa = self.query(domain, "AAAA").await;
        let addrs = match (a, aaaa) {
            (Ok(mut a), Ok(b)) => {
                a.extend(b);
                Ok(a)
            }
            // One family answering is enough to say where the name points.
            (Ok(a), Err(_)) => Ok(a),
            (Err(_), Ok(b)) => Ok(b),
            (Err(e), Err(_)) => Err(e),
        };
        Observed { txt, addrs }
    }
}

/// Read a DNS JSON API body.
///
/// # Errors
/// See [`Doh::query`].
pub fn parse_doh(body: &Value) -> Result<Vec<(u16, String)>, String> {
    match body.get("Status").and_then(Value::as_u64) {
        // NOERROR, NXDOMAIN
        Some(0 | 3) => {}
        Some(n) => return Err(format!("resolver status {n}")),
        None => return Err("DNS-over-HTTPS answer has no Status".to_string()),
    }
    Ok(body
        .get("Answer")
        .and_then(Value::as_array)
        .map(|answers| {
            answers
                .iter()
                .filter_map(|a| {
                    let t = u16::try_from(a.get("type")?.as_u64()?).ok()?;
                    let data = a.get("data")?.as_str()?.to_string();
                    Some((t, data))
                })
                .collect()
        })
        .unwrap_or_default())
}

/// A TXT record's text from its presentation form: `"a" "b"` is `ab`, an
/// unquoted value is taken as is.
#[must_use]
pub fn txt_text(data: &str) -> String {
    let data = data.trim();
    if !data.starts_with('"') {
        return data.to_string();
    }
    let mut out = String::new();
    let mut in_quote = false;
    let mut escaped = false;
    for c in data.chars() {
        if escaped {
            out.push(c);
            escaped = false;
        } else if c == '\\' && in_quote {
            escaped = true;
        } else if c == '"' {
            in_quote = !in_quote;
        } else if in_quote {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const UUID: &str = "0b1c2d3e-4f50-4617-8899-aabbccddeeff";

    fn opts() -> DomainsOpts {
        DomainsOpts {
            state_file: "/x/domains.json".to_string(),
            zone: "boxes.losos.cfd".to_string(),
            zone_file: "/x/zone".to_string(),
            nameservers: vec!["ns1.boxes.losos.cfd".to_string()],
            hostmaster: "hostmaster.boxes.losos.cfd".to_string(),
            ipv4: vec!["203.0.113.7".parse().unwrap()],
            ipv6: vec!["2001:db8::7".parse().unwrap()],
            doh_url: DEFAULT_DOH_URL.to_string(),
            public_domain: "losos.cfd".to_string(),
        }
    }

    fn vouched() -> Vouched {
        Vouched {
            box_uuid: UUID.to_string(),
            account_id: "acct_1Abc".to_string(),
        }
    }

    fn claim(domain: &str, tenant: &str) -> Claim {
        add(&DomainsState::default(), tenant, domain, 100)
            .unwrap()
            .claims
            .remove(0)
    }

    fn good(o: &DomainsOpts, domain: &str) -> Observed {
        Observed {
            txt: Ok(vec![challenge_token(domain, UUID, "acct_1Abc")]),
            addrs: Ok(vec![
                (5, format!("{}.", o.target_for(UUID))),
                (1, "203.0.113.7".to_string()),
            ]),
        }
    }

    #[test]
    fn the_label_is_stable_short_and_not_the_uuid() {
        let l = box_label(UUID);
        assert_eq!(l.len(), 16);
        assert_eq!(l, box_label(UUID));
        assert!(!UUID.contains(&l));
        assert_ne!(l, box_label("0b1c2d3e-4f50-4617-8899-aabbccddeef0"));
        assert!(l.bytes().all(|b| b.is_ascii_hexdigit()));
    }

    #[test]
    fn the_token_binds_domain_box_and_stripe_account() {
        let t = challenge_token("cloud.example.org", UUID, "acct_1");
        assert!(t.starts_with(CHALLENGE_PREFIX));
        assert_eq!(t.len(), CHALLENGE_PREFIX.len() + 32);
        assert!(!t.contains("acct_1"));
        assert_ne!(t, challenge_token("www.example.org", UUID, "acct_1"));
        assert_ne!(t, challenge_token("cloud.example.org", UUID, "acct_2"));
        assert_ne!(
            t,
            challenge_token(
                "cloud.example.org",
                "0b1c2d3e-4f50-4617-8899-aabbccddeef0",
                "acct_1"
            )
        );
    }

    #[test]
    fn domain_names_are_normalised_and_checked() {
        let o = opts();
        assert_eq!(
            normalize_domain(" Cloud.Example.ORG. ", &o).unwrap(),
            "cloud.example.org"
        );
        assert_eq!(normalize_domain("example.org", &o).unwrap(), "example.org");
        assert_eq!(
            normalize_domain("xn--bcher-kva.example", &o).unwrap(),
            "xn--bcher-kva.example"
        );
        for bad in [
            "",
            "localhost",
            "büecher.example",
            "-a.example.org",
            "a-.example.org",
            "a..example.org",
            "a_b.example.org",
            "*.example.org",
            "10.0.0.1",
            "a.example.org/path",
            "mattbox.losos.cfd",
            "losos.cfd",
            "x.boxes.losos.cfd",
        ] {
            assert!(normalize_domain(bad, &o).is_err(), "{bad:?} was accepted");
        }
        let long = format!("{}.org", "a".repeat(64));
        assert!(normalize_domain(&long, &o).is_err());
    }

    #[test]
    fn a_domain_goes_live_only_with_both_records_and_a_ready_account() {
        let o = opts();
        let c = claim("cloud.example.org", "mattbox");
        let v = vouched();

        let live = evaluate(&c, Some(&v), &o, false, &good(&o, "cloud.example.org"), 200);
        assert_eq!(live.verified_at, Some(200));
        assert_eq!(live.problem, None);
        assert!(live.txt_found && live.points_here);

        let no_cname = Observed {
            addrs: Ok(vec![(1, "198.51.100.1".to_string())]),
            ..good(&o, "cloud.example.org")
        };
        let w = evaluate(&c, Some(&v), &o, false, &no_cname, 200);
        assert_eq!(w.verified_at, None);
        assert_eq!(w.problem, Some(Problem::NotPointing));
        assert!(w.txt_found);

        let no_txt = Observed {
            txt: Ok(vec!["v=spf1 -all".to_string()]),
            ..good(&o, "cloud.example.org")
        };
        let w = evaluate(&c, Some(&v), &o, false, &no_txt, 200);
        assert_eq!(w.problem, Some(Problem::TxtMissing));

        // Another box's token, or this box's for another domain, is wrong.
        let other = Observed {
            txt: Ok(vec![challenge_token(
                "cloud.example.org",
                UUID,
                "acct_other",
            )]),
            ..good(&o, "cloud.example.org")
        };
        let w = evaluate(&c, Some(&v), &o, false, &other, 200);
        assert_eq!(w.problem, Some(Problem::TxtWrong));
        assert_eq!(w.verified_at, None);

        let w = evaluate(&c, None, &o, false, &good(&o, "cloud.example.org"), 200);
        assert_eq!(w.problem, Some(Problem::StripeAccount));
        assert_eq!(w.verified_at, None);

        let w = evaluate(&c, Some(&v), &o, true, &good(&o, "cloud.example.org"), 200);
        assert_eq!(w.problem, Some(Problem::TakenElsewhere));
    }

    #[test]
    fn an_apex_with_the_edge_address_counts_as_pointing() {
        let o = opts();
        assert!(points_here(&[(1, "203.0.113.7".into())], "x", &o));
        assert!(points_here(&[(28, "2001:db8::7".into())], "x", &o));
        assert!(!points_here(&[(1, "203.0.113.8".into())], "x", &o));
        assert!(points_here(&[(5, "X.".into())], "x", &o));
        assert!(!points_here(&[(16, "203.0.113.7".into())], "x", &o));
    }

    #[test]
    fn a_live_domain_survives_two_bad_lookups_and_lapses_on_the_third() {
        let o = opts();
        let v = vouched();
        let c = evaluate(
            &claim("cloud.example.org", "mattbox"),
            Some(&v),
            &o,
            false,
            &good(&o, "cloud.example.org"),
            200,
        );
        let gone = Observed {
            txt: Ok(vec![]),
            addrs: Err("timeout".into()),
        };
        let c1 = evaluate(&c, Some(&v), &o, false, &gone, 300);
        assert!(c1.live() && c1.failures == 1);
        assert_eq!(c1.problem, Some(Problem::LookupFailed));
        let c2 = evaluate(&c1, Some(&v), &o, false, &gone, 400);
        assert!(c2.live() && c2.failures == 2);
        // A good answer in between resets the count.
        let back = evaluate(
            &c2,
            Some(&v),
            &o,
            false,
            &good(&o, "cloud.example.org"),
            450,
        );
        assert!(back.live() && back.failures == 0);
        let c3 = evaluate(
            &evaluate(&c2, Some(&v), &o, false, &gone, 500),
            Some(&v),
            &o,
            false,
            &gone,
            600,
        );
        assert!(!c3.live());
        // Stripe no longer vouching: offline at once, no grace.
        let s = evaluate(&c, None, &o, false, &good(&o, "cloud.example.org"), 300);
        assert!(!s.live());
    }

    #[test]
    fn claims_are_bounded_and_a_live_domain_is_not_taken_twice() {
        let mut s = DomainsState::default();
        for i in 0..MAX_PER_TENANT {
            s = add(&s, "a", &format!("d{i}.example.org"), 1).unwrap();
        }
        assert!(matches!(
            add(&s, "a", "more.example.org", 1),
            Err(DomainError::Conflict(_))
        ));
        // Idempotent for the same box.
        assert_eq!(add(&s, "a", "d0.example.org", 2).unwrap(), s);
        // Waiting claims do not block another box; a live one does.
        let s = add(&s, "b", "d0.example.org", 3).unwrap();
        let mut s2 = s.clone();
        s2.claims[0].verified_at = Some(5);
        assert!(matches!(
            add(&s2, "c", "d0.example.org", 6),
            Err(DomainError::Conflict(_))
        ));
        assert!(matches!(
            remove(&s, "c", "d0.example.org"),
            Err(DomainError::NotFound)
        ));
        let s3 = remove(&s, "b", "d0.example.org").unwrap();
        assert_eq!(s3.claims.len(), MAX_PER_TENANT);
    }

    #[test]
    fn stale_claims_and_removed_tenants_are_pruned() {
        let mut s = add(&DomainsState::default(), "a", "x.example.org", 0).unwrap();
        s = add(&s, "gone", "y.example.org", 0).unwrap();
        s = add(&s, "a", "z.example.org", 0).unwrap();
        s.claims[2].verified_at = Some(1);
        assert!(prune(&mut s, |t| t == "a", PENDING_TTL_SECS + 1));
        let left: Vec<&str> = s.claims.iter().map(|c| c.domain.as_str()).collect();
        assert_eq!(left, ["z.example.org"]);
    }

    #[test]
    fn live_routes_give_each_domain_to_one_tenant() {
        let mut s = add(&DomainsState::default(), "a", "x.example.org", 0).unwrap();
        s = add(&s, "b", "x.example.org", 0).unwrap();
        s = add(&s, "b", "y.example.org", 0).unwrap();
        s.claims[0].verified_at = Some(20);
        s.claims[1].verified_at = Some(10);
        s.claims[2].verified_at = Some(30);
        let r = live_routes(&s);
        assert_eq!(r.get("a"), None);
        assert_eq!(
            r.get("b").unwrap(),
            &vec!["x.example.org".to_string(), "y.example.org".to_string()]
        );
    }

    #[test]
    fn the_view_hides_the_token_until_stripe_vouches() {
        let o = opts();
        let s = add(&DomainsState::default(), "a", "x.example.org", 0).unwrap();
        let v = view(&s, "a", None, &o);
        assert!(!v.eligible);
        assert_eq!(v.reason, Some(Problem::StripeAccount));
        assert_eq!(v.target, None);
        assert_eq!(v.domains[0].txt_value, None);
        assert_eq!(v.domains[0].txt_name, "_losos-challenge.x.example.org");
        let v = view(&s, "a", Some(&vouched()), &o);
        assert!(v.eligible);
        assert_eq!(v.target.as_deref(), Some(o.target_for(UUID).as_str()));
        assert_eq!(
            v.domains[0].txt_value.as_deref(),
            Some(challenge_token("x.example.org", UUID, "acct_1Abc").as_str())
        );
        assert_eq!(v.addresses, ["203.0.113.7", "2001:db8::7"]);
        assert!(view(&s, "b", Some(&vouched()), &o).domains.is_empty());
    }

    #[test]
    fn doh_answers_are_read_with_nxdomain_as_empty() {
        let body = serde_json::json!({
            "Status": 0,
            "Answer": [
                {"name": "cloud.example.org", "type": 5, "TTL": 300, "data": "abc.boxes.losos.cfd."},
                {"name": "abc.boxes.losos.cfd", "type": 1, "TTL": 300, "data": "203.0.113.7"}
            ]
        });
        assert_eq!(
            parse_doh(&body).unwrap(),
            vec![
                (5, "abc.boxes.losos.cfd.".to_string()),
                (1, "203.0.113.7".to_string())
            ]
        );
        assert_eq!(
            parse_doh(&serde_json::json!({"Status": 3})).unwrap(),
            vec![]
        );
        assert!(parse_doh(&serde_json::json!({"Status": 2})).is_err());
        assert!(parse_doh(&serde_json::json!({})).is_err());
    }

    #[test]
    fn txt_presentation_form_is_unquoted_and_joined() {
        assert_eq!(txt_text("\"losos-domain-v1=ab\""), "losos-domain-v1=ab");
        assert_eq!(txt_text("\"losos-\" \"domain\""), "losos-domain");
        assert_eq!(txt_text("plain"), "plain");
        assert_eq!(txt_text("\"a\\\"b\""), "a\"b");
    }

    #[test]
    fn only_a_ready_account_with_the_box_uuid_vouches() {
        use crate::market::Seller;
        let s = |ready, uuid: Option<&str>| Seller {
            account_id: "acct_1".to_string(),
            ready,
            box_uuid: uuid.map(str::to_string),
        };
        assert!(Vouched::from_seller(Some(&s(true, Some(UUID)))).is_some());
        assert!(Vouched::from_seller(Some(&s(false, Some(UUID)))).is_none());
        assert!(Vouched::from_seller(Some(&s(true, None))).is_none());
        assert!(Vouched::from_seller(Some(&s(true, Some("not-a-uuid")))).is_none());
        assert!(Vouched::from_seller(None).is_none());
    }
}
