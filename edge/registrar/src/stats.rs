//! What Traefik counted, for the Statistics page of the edge's `/manage/`.
//!
//! Traefik exposes Prometheus counters on a loopback entrypoint
//! (`modules/edge.nix`, `losos.edge.traefikMetricsPort`). Counters only grow
//! from when Traefik started, so a page that wants "the last hour" needs
//! someone to remember earlier readings: the registrar reads them every
//! [`SCRAPE_EVERY`] and keeps one minute of differences per slot for the
//! last [`MINUTES`] minutes. A reading lower than the one before means
//! Traefik restarted, and counts from zero again.
//!
//! The document is the one the box's lososd serves at `/api/statistics`, and
//! `admin-ui/app/src/statistics/types.ts` reads both:
//!
//! ```text
//! { available, reason, since,
//!   totals { requests, clientErrors, serverErrors, bytesIn, bytesOut, openConnections },
//!   codes { 2xx, 3xx, 4xx, 5xx }, protocols {},
//!   perMinute [ { at, requests, serverErrors, bytesOut, p95Ms } ],   oldest first
//!   routes [ { name, host, requests, serverErrors, bytesOut, p95Ms } ] }
//! ```
//!
//! `totals`, `codes` and a route's counts run from `since` (Traefik's start);
//! a route's `p95Ms` is over the last hour. `protocols` is empty: Traefik's
//! `protocol` label is http, websocket, sse or grpc, not the HTTP version.
//! A route is one Traefik service, which is one box (its extra names
//! included), one tenant or one machine.

use std::collections::{BTreeMap, VecDeque};
use std::time::Duration;

use serde_json::{json, Value};

/// How often the registrar reads Traefik's counters.
pub const SCRAPE_EVERY: Duration = Duration::from_secs(15);

/// How many minutes the page can look back.
pub const MINUTES: usize = 60;

/// The entrypoints that are not traffic: the metrics one itself and the API.
const QUIET_ENTRYPOINTS: [&str; 2] = ["metrics", "traefik"];

/// One sample line of the Prometheus text format.
#[derive(Debug, Clone, PartialEq)]
pub struct Sample {
    pub name: String,
    pub labels: Vec<(String, String)>,
    pub value: f64,
}

impl Sample {
    fn label(&self, key: &str) -> Option<&str> {
        self.labels
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
}

/// Every sample in a Prometheus text exposition. Comments, blank lines and
/// lines that do not parse are skipped: the page is better with what could
/// be read than with nothing.
#[must_use]
pub fn parse(text: &str) -> Vec<Sample> {
    text.lines().filter_map(parse_line).collect()
}

fn parse_line(line: &str) -> Option<Sample> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let name_end = line
        .find(|c: char| c == '{' || c.is_whitespace())
        .unwrap_or(line.len());
    let name = &line[..name_end];
    if name.is_empty() {
        return None;
    }
    let mut rest = &line[name_end..];
    let mut labels = Vec::new();
    if let Some(after) = rest.strip_prefix('{') {
        let (parsed, tail) = parse_labels(after)?;
        labels = parsed;
        rest = tail;
    }
    // A timestamp may follow the value; only the value matters here.
    let value = rest.split_whitespace().next()?;
    let value = match value {
        "+Inf" => f64::INFINITY,
        "-Inf" => f64::NEG_INFINITY,
        v => v.parse().ok()?,
    };
    Some(Sample {
        name: name.to_string(),
        labels,
        value,
    })
}

/// `a="1",b="x\"y"}` and what follows the closing brace.
fn parse_labels(mut s: &str) -> Option<(Vec<(String, String)>, &str)> {
    let mut labels = Vec::new();
    loop {
        s = s.trim_start_matches([' ', ',']);
        if let Some(tail) = s.strip_prefix('}') {
            return Some((labels, tail));
        }
        let eq = s.find('=')?;
        let key = s[..eq].trim().to_string();
        s = s[eq + 1..].trim_start().strip_prefix('"')?;
        let mut value = String::new();
        let mut chars = s.char_indices();
        let end = loop {
            let (i, c) = chars.next()?;
            match c {
                '"' => break i,
                '\\' => match chars.next()?.1 {
                    'n' => value.push('\n'),
                    other => value.push(other),
                },
                c => value.push(c),
            }
        };
        labels.push((key, value));
        s = &s[end + 1..];
    }
}

/// A duration histogram: the count at or under each bound, `+Inf` last.
type Buckets = BTreeMap<Bound, f64>;

/// A bucket bound that sorts, `+Inf` included.
#[derive(Debug, Clone, Copy)]
struct Bound(f64);

impl PartialEq for Bound {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}

impl Eq for Bound {}

impl PartialOrd for Bound {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Bound {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}

/// One service's counters.
#[derive(Debug, Clone, Default, PartialEq)]
struct Service {
    requests: f64,
    server_errors: f64,
    bytes_out: f64,
    buckets: Buckets,
}

/// One reading of everything the page shows, as Traefik's running totals.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Reading {
    requests: f64,
    codes: [f64; 4],
    bytes_in: f64,
    bytes_out: f64,
    open_connections: f64,
    buckets: Buckets,
    services: BTreeMap<String, Service>,
    /// `process_start_time_seconds`: when these counters started.
    started: Option<f64>,
}

/// The class a status code is counted under, `None` for 1xx and nonsense.
fn class(code: Option<&str>) -> Option<usize> {
    match code?.as_bytes().first()? {
        b'2' => Some(0),
        b'3' => Some(1),
        b'4' => Some(2),
        b'5' => Some(3),
        _ => None,
    }
}

/// A service as the page names it: `nas@http` is `nas`, and Traefik's own
/// (`api@internal`, `prometheus@internal`) are not traffic.
fn service_name(label: Option<&str>) -> Option<String> {
    let label = label?;
    let (name, provider) = label.rsplit_once('@').unwrap_or((label, ""));
    (provider != "internal" && !name.is_empty()).then(|| name.to_string())
}

impl Reading {
    /// Fold Traefik's samples into one reading.
    #[must_use]
    pub fn from_samples(samples: &[Sample]) -> Self {
        let mut r = Reading::default();
        for s in samples {
            if !s.value.is_finite() || s.value < 0.0 {
                continue;
            }
            let traffic = s
                .label("entrypoint")
                .is_none_or(|e| !QUIET_ENTRYPOINTS.contains(&e));
            match s.name.as_str() {
                "process_start_time_seconds" => r.started = Some(s.value),
                "traefik_entrypoint_requests_total" if traffic => {
                    r.requests += s.value;
                    if let Some(c) = class(s.label("code")) {
                        r.codes[c] += s.value;
                    }
                }
                "traefik_entrypoint_requests_bytes_total" if traffic => r.bytes_in += s.value,
                "traefik_entrypoint_responses_bytes_total" if traffic => r.bytes_out += s.value,
                "traefik_open_connections" if traffic => r.open_connections += s.value,
                "traefik_entrypoint_request_duration_seconds_bucket" if traffic => {
                    if let Some(le) = bound(s) {
                        *r.buckets.entry(le).or_default() += s.value;
                    }
                }
                "traefik_service_requests_total" => {
                    if let Some(name) = service_name(s.label("service")) {
                        let svc = r.services.entry(name).or_default();
                        svc.requests += s.value;
                        if class(s.label("code")) == Some(3) {
                            svc.server_errors += s.value;
                        }
                    }
                }
                "traefik_service_responses_bytes_total" => {
                    if let Some(name) = service_name(s.label("service")) {
                        r.services.entry(name).or_default().bytes_out += s.value;
                    }
                }
                "traefik_service_request_duration_seconds_bucket" => {
                    if let (Some(name), Some(le)) = (service_name(s.label("service")), bound(s)) {
                        *r.services
                            .entry(name)
                            .or_default()
                            .buckets
                            .entry(le)
                            .or_default() += s.value;
                    }
                }
                _ => {}
            }
        }
        r
    }

    /// Whether `self` can follow `before` without Traefik having restarted
    /// in between: the same start, and no running total went down.
    fn follows(&self, before: &Reading) -> bool {
        self.started == before.started
            && self.requests >= before.requests
            && self.bytes_out >= before.bytes_out
    }
}

fn bound(s: &Sample) -> Option<Bound> {
    match s.label("le")? {
        "+Inf" => Some(Bound(f64::INFINITY)),
        le => le.parse().ok().map(Bound),
    }
}

/// `b - a` per bound, never below zero.
fn bucket_delta(now: &Buckets, before: Option<&Buckets>) -> Buckets {
    now.iter()
        .map(|(le, n)| {
            let was = before.and_then(|b| b.get(le)).copied().unwrap_or(0.0);
            (*le, (n - was).max(0.0))
        })
        .collect()
}

fn add_buckets(into: &mut Buckets, more: &Buckets) {
    for (le, n) in more {
        *into.entry(*le).or_default() += n;
    }
}

/// The 95th percentile of a cumulative histogram in milliseconds, by linear
/// interpolation inside the bucket it falls in, as Prometheus's
/// `histogram_quantile` does. `None` with nothing counted. Past the last
/// finite bound the answer is that bound: the histogram knows no more.
fn p95_ms(buckets: &Buckets) -> Option<f64> {
    let total = buckets
        .get(&Bound(f64::INFINITY))
        .copied()
        .or_else(|| buckets.values().copied().reduce(f64::max))?;
    if total <= 0.0 {
        return None;
    }
    let target = 0.95 * total;
    let (mut lower, mut below) = (0.0_f64, 0.0_f64);
    for (Bound(le), count) in buckets {
        if *count >= target {
            if le.is_infinite() {
                return Some(lower * 1000.0);
            }
            let share = if *count > below {
                (target - below) / (count - below)
            } else {
                1.0
            };
            return Some((lower + (le - lower) * share) * 1000.0);
        }
        if le.is_finite() {
            lower = *le;
        }
        below = below.max(*count);
    }
    Some(lower * 1000.0)
}

/// What one minute added.
#[derive(Debug, Clone, Default, PartialEq)]
struct Minute {
    /// Minutes since the Unix epoch.
    at: u64,
    requests: f64,
    server_errors: f64,
    bytes_out: f64,
    buckets: Buckets,
    services: BTreeMap<String, Buckets>,
}

/// The last readings, and the minutes between them.
#[derive(Debug, Default)]
pub struct Ring {
    last: Option<Reading>,
    minutes: VecDeque<Minute>,
    /// Why the last reading failed, until one succeeds.
    failing: Option<String>,
}

impl Ring {
    /// Take a reading made at `now` (seconds since the Unix epoch).
    pub fn record(&mut self, reading: Reading, now: u64) {
        let zero = Reading::default();
        let before = match &self.last {
            Some(last) if reading.follows(last) => last,
            // The first reading has no "before": what it holds happened
            // before this registrar was looking and belongs to no minute.
            None => {
                self.last = Some(reading);
                self.failing = None;
                return;
            }
            // Traefik restarted: everything it holds is new.
            Some(_) => &zero,
        };
        let minute = now / 60;
        let mut added = Minute {
            at: minute,
            requests: (reading.requests - before.requests).max(0.0),
            server_errors: (reading.codes[3] - before.codes[3]).max(0.0),
            bytes_out: (reading.bytes_out - before.bytes_out).max(0.0),
            buckets: bucket_delta(&reading.buckets, Some(&before.buckets)),
            services: BTreeMap::new(),
        };
        for (name, svc) in &reading.services {
            let was = before.services.get(name).map(|s| &s.buckets);
            added
                .services
                .insert(name.clone(), bucket_delta(&svc.buckets, was));
        }
        match self.minutes.back_mut() {
            Some(last) if last.at == minute => {
                last.requests += added.requests;
                last.server_errors += added.server_errors;
                last.bytes_out += added.bytes_out;
                add_buckets(&mut last.buckets, &added.buckets);
                for (name, b) in &added.services {
                    add_buckets(last.services.entry(name.clone()).or_default(), b);
                }
            }
            _ => self.minutes.push_back(added),
        }
        let oldest = minute.saturating_sub(MINUTES as u64 - 1);
        while self.minutes.front().is_some_and(|m| m.at < oldest) {
            self.minutes.pop_front();
        }
        self.last = Some(reading);
        self.failing = None;
    }

    /// Traefik could not be read at all.
    pub fn failed(&mut self, why: String) {
        self.failing = Some(why);
    }

    /// The page's document. `hosts` names each service's address
    /// ([`crate::config::Routes::hosts_by_service`]); `now` drops minutes
    /// older than an hour even when no reading came in since.
    #[must_use]
    pub fn document(&self, hosts: &BTreeMap<String, String>, now: u64) -> Value {
        let Some(r) = &self.last else {
            return json!({
                "available": false,
                "reason": self
                    .failing
                    .clone()
                    .unwrap_or_else(|| "Traefik has not been read yet".to_string()),
                "since": null,
                "totals": {}, "codes": {}, "protocols": {},
                "perMinute": [], "routes": [],
            });
        };
        let oldest = (now / 60).saturating_sub(MINUTES as u64 - 1);
        let minutes: Vec<&Minute> = self.minutes.iter().filter(|m| m.at >= oldest).collect();
        let per_minute: Vec<Value> = minutes
            .iter()
            .map(|m| {
                json!({
                    "at": rfc3339(m.at * 60),
                    "requests": m.requests.round(),
                    "serverErrors": m.server_errors.round(),
                    "bytesOut": m.bytes_out.round(),
                    "p95Ms": p95_ms(&m.buckets).map(round_ms),
                })
            })
            .collect();
        let mut routes: Vec<Value> = r
            .services
            .iter()
            .map(|(name, svc)| {
                let mut hour = Buckets::new();
                for m in &minutes {
                    if let Some(b) = m.services.get(name) {
                        add_buckets(&mut hour, b);
                    }
                }
                json!({
                    "name": name,
                    "host": hosts.get(name),
                    "requests": svc.requests.round(),
                    "serverErrors": svc.server_errors.round(),
                    "bytesOut": svc.bytes_out.round(),
                    "p95Ms": p95_ms(&hour).map(round_ms),
                })
            })
            .collect();
        routes.sort_by(|a, b| {
            b["requests"]
                .as_f64()
                .unwrap_or(0.0)
                .total_cmp(&a["requests"].as_f64().unwrap_or(0.0))
                .then_with(|| a["name"].as_str().cmp(&b["name"].as_str()))
        });
        json!({
            "available": true,
            "reason": null,
            "since": r.started.map(|s| rfc3339(s as u64)),
            "totals": {
                "requests": r.requests.round(),
                "clientErrors": r.codes[2].round(),
                "serverErrors": r.codes[3].round(),
                "bytesIn": r.bytes_in.round(),
                "bytesOut": r.bytes_out.round(),
                "openConnections": r.open_connections.round(),
            },
            "codes": {
                "2xx": r.codes[0].round(),
                "3xx": r.codes[1].round(),
                "4xx": r.codes[2].round(),
                "5xx": r.codes[3].round(),
            },
            "protocols": {},
            "perMinute": per_minute,
            "routes": routes,
        })
    }
}

fn round_ms(ms: f64) -> f64 {
    (ms * 10.0).round() / 10.0
}

/// Seconds since the Unix epoch as RFC 3339 in UTC, `1970-01-01T00:00:00Z`.
#[must_use]
pub fn rfc3339(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lines as Traefik 3.7 writes them, trimmed to what the page reads.
    fn exposition(requests: u64, errors: u64, fast: u64, slow: u64, started: u64) -> String {
        let total = fast + slow;
        format!(
            "# HELP traefik_entrypoint_requests_total How many HTTP requests processed on an entrypoint.\n\
             # TYPE traefik_entrypoint_requests_total counter\n\
             traefik_entrypoint_requests_total{{code=\"200\",entrypoint=\"websecure\",method=\"GET\",protocol=\"http\"}} {requests}\n\
             traefik_entrypoint_requests_total{{code=\"502\",entrypoint=\"websecure\",method=\"GET\",protocol=\"http\"}} {errors}\n\
             traefik_entrypoint_requests_total{{code=\"200\",entrypoint=\"metrics\",method=\"GET\",protocol=\"http\"}} 999\n\
             traefik_entrypoint_requests_bytes_total{{code=\"200\",entrypoint=\"websecure\",method=\"GET\",protocol=\"http\"}} 100\n\
             traefik_entrypoint_responses_bytes_total{{code=\"200\",entrypoint=\"websecure\",method=\"GET\",protocol=\"http\"}} {}\n\
             traefik_entrypoint_request_duration_seconds_bucket{{code=\"200\",entrypoint=\"websecure\",method=\"GET\",protocol=\"http\",le=\"0.1\"}} {fast}\n\
             traefik_entrypoint_request_duration_seconds_bucket{{code=\"200\",entrypoint=\"websecure\",method=\"GET\",protocol=\"http\",le=\"1\"}} {total}\n\
             traefik_entrypoint_request_duration_seconds_bucket{{code=\"200\",entrypoint=\"websecure\",method=\"GET\",protocol=\"http\",le=\"+Inf\"}} {total}\n\
             traefik_open_connections{{entrypoint=\"websecure\",protocol=\"TCP\"}} 3\n\
             traefik_service_requests_total{{code=\"200\",method=\"GET\",protocol=\"http\",service=\"nas@http\"}} {requests}\n\
             traefik_service_requests_total{{code=\"502\",method=\"GET\",protocol=\"http\",service=\"nas@http\"}} {errors}\n\
             traefik_service_requests_total{{code=\"200\",method=\"GET\",protocol=\"http\",service=\"prometheus@internal\"}} 5\n\
             traefik_service_responses_bytes_total{{code=\"200\",method=\"GET\",protocol=\"http\",service=\"nas@http\"}} {}\n\
             traefik_service_request_duration_seconds_bucket{{code=\"200\",method=\"GET\",protocol=\"http\",service=\"nas@http\",le=\"0.1\"}} {fast}\n\
             traefik_service_request_duration_seconds_bucket{{code=\"200\",method=\"GET\",protocol=\"http\",service=\"nas@http\",le=\"1\"}} {total}\n\
             traefik_service_request_duration_seconds_bucket{{code=\"200\",method=\"GET\",protocol=\"http\",service=\"nas@http\",le=\"+Inf\"}} {total}\n\
             process_start_time_seconds {started}\n",
            requests * 1000,
            requests * 1000,
        )
    }

    fn reading(requests: u64, errors: u64, fast: u64, slow: u64, started: u64) -> Reading {
        Reading::from_samples(&parse(&exposition(requests, errors, fast, slow, started)))
    }

    #[test]
    fn labels_with_escapes_and_timestamps_parse() {
        let s = parse(
            "x{a=\"q\\\"uote\",b=\"back\\\\slash\",c=\"line\\nbreak\"} 2.5 1700000000000\n\
             y 3\n\
             # a comment\n\
             broken{a=\"1\" 4\n\
             z{le=\"+Inf\"} +Inf\n",
        );
        assert_eq!(s.len(), 3);
        assert_eq!(s[0].label("a"), Some("q\"uote"));
        assert_eq!(s[0].label("b"), Some("back\\slash"));
        assert_eq!(s[0].label("c"), Some("line\nbreak"));
        assert!((s[0].value - 2.5).abs() < f64::EPSILON);
        assert_eq!(s[1].name, "y");
        assert!(s[2].value.is_infinite());
    }

    #[test]
    fn a_reading_leaves_out_the_metrics_entrypoint_and_traefiks_own_services() {
        let r = reading(10, 2, 9, 3, 1_000);
        assert!((r.requests - 12.0).abs() < f64::EPSILON);
        assert!((r.codes[0] - 10.0).abs() < f64::EPSILON);
        assert!((r.codes[3] - 2.0).abs() < f64::EPSILON);
        assert!((r.open_connections - 3.0).abs() < f64::EPSILON);
        assert_eq!(r.services.keys().collect::<Vec<_>>(), ["nas"]);
        assert_eq!(r.started, Some(1_000.0));
    }

    #[test]
    fn the_95th_percentile_interpolates_inside_its_bucket() {
        // 90 of 100 under 0.1 s, all under 1 s: the 95th falls halfway
        // through the 0.1–1 s bucket.
        let b: Buckets = [(0.1, 90.0), (1.0, 100.0), (f64::INFINITY, 100.0)]
            .into_iter()
            .map(|(le, n)| (Bound(le), n))
            .collect();
        let p = p95_ms(&b).unwrap();
        assert!((p - 550.0).abs() < 1e-6, "{p}");
        // Past the last finite bound: that bound.
        let b: Buckets = [(0.1, 1.0), (f64::INFINITY, 100.0)]
            .into_iter()
            .map(|(le, n)| (Bound(le), n))
            .collect();
        assert!((p95_ms(&b).unwrap() - 100.0).abs() < 1e-6);
        assert_eq!(p95_ms(&Buckets::new()), None);
    }

    #[test]
    fn minutes_hold_what_was_added_and_a_restart_counts_from_zero() {
        let mut ring = Ring::default();
        // A minute boundary: 1_800_000_000 is a multiple of 60.
        let t0 = 1_800_000_000;
        ring.record(reading(100, 0, 100, 0, 1_000), t0);
        // The first reading belongs to no minute.
        let doc = ring.document(&BTreeMap::new(), t0);
        assert_eq!(doc["perMinute"].as_array().unwrap().len(), 0);
        assert_eq!(doc["totals"]["requests"], 100.0);

        ring.record(reading(110, 1, 108, 3, 1_000), t0 + 15);
        ring.record(reading(130, 1, 125, 6, 1_000), t0 + 30);
        ring.record(reading(140, 2, 130, 12, 1_000), t0 + 75);
        let doc = ring.document(&BTreeMap::new(), t0 + 75);
        let minutes = doc["perMinute"].as_array().unwrap();
        assert_eq!(minutes.len(), 2);
        assert_eq!(minutes[0]["at"], rfc3339(t0));
        assert_eq!(minutes[0]["requests"], 31.0);
        assert_eq!(minutes[0]["serverErrors"], 1.0);
        assert_eq!(minutes[1]["requests"], 11.0);

        // Traefik restarted: its counters start again, and all of it is new.
        ring.record(reading(5, 0, 5, 0, 2_000), t0 + 130);
        let doc = ring.document(&BTreeMap::new(), t0 + 130);
        let minutes = doc["perMinute"].as_array().unwrap();
        assert_eq!(minutes.len(), 3);
        assert_eq!(minutes[2]["requests"], 5.0);
        assert_eq!(doc["totals"]["requests"], 5.0);
        assert_eq!(doc["since"], rfc3339(2_000));
    }

    #[test]
    fn minutes_older_than_an_hour_drop_off() {
        let mut ring = Ring::default();
        // A minute boundary: 1_800_000_000 is a multiple of 60.
        let t0 = 1_800_000_000;
        ring.record(reading(0, 0, 0, 0, 1), t0);
        for i in 1..=70_u64 {
            ring.record(reading(i, 0, i, 0, 1), t0 + i * 60);
        }
        let now = t0 + 70 * 60;
        let doc = ring.document(&BTreeMap::new(), now);
        let minutes = doc["perMinute"].as_array().unwrap();
        assert_eq!(minutes.len(), MINUTES);
        assert_eq!(minutes.last().unwrap()["at"], rfc3339(now));
        // An hour with no reading leaves nothing on the chart.
        let doc = ring.document(&BTreeMap::new(), now + 3_600);
        assert_eq!(doc["perMinute"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn routes_carry_their_host_and_the_hours_percentile() {
        let mut ring = Ring::default();
        // A minute boundary: 1_800_000_000 is a multiple of 60.
        let t0 = 1_800_000_000;
        ring.record(reading(0, 0, 0, 0, 1), t0);
        ring.record(reading(20, 0, 19, 1, 1), t0 + 20);
        let hosts: BTreeMap<String, String> =
            [("nas".to_string(), "nas.example.org".to_string())].into();
        let doc = ring.document(&hosts, t0 + 20);
        let route = &doc["routes"][0];
        assert_eq!(route["name"], "nas");
        assert_eq!(route["host"], "nas.example.org");
        assert_eq!(route["requests"], 20.0);
        assert!(route["p95Ms"].as_f64().is_some());
        assert_eq!(doc["routes"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn nothing_read_yet_says_why() {
        let mut ring = Ring::default();
        let doc = ring.document(&BTreeMap::new(), 0);
        assert_eq!(doc["available"], false);
        ring.failed("Traefik did not answer on 127.0.0.1:8091".into());
        let doc = ring.document(&BTreeMap::new(), 0);
        assert_eq!(doc["reason"], "Traefik did not answer on 127.0.0.1:8091");
    }

    #[test]
    fn rfc3339_formats_utc() {
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(rfc3339(1_792_675_451), "2026-10-22T13:24:11Z");
    }
}
