//! What Traefik counted, made into what the Statistics page draws.
//!
//! Traefik's counters only grow, from the moment it started. A sample is all
//! of them at one moment ([`Snapshot`]); the plugin keeps one a minute for
//! the last hour ([`Ring`]) and draws each minute as the difference between
//! two. A counter that went down means Traefik restarted, and the newer
//! value is then the whole minute.
//!
//! Only the apps' LAN ports count: the `port-*` entrypoints and the `app-*`
//! routers (`modules/apps.nix`, `backend/src/compose.rs`). The `traefik`
//! entrypoint, where this plugin itself reads the metrics, is left out.

use crate::prom::Sample;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

/// The last hour: 61 samples make 60 minutes.
pub const KEEP: usize = 61;

/// Counters of one router, or of every app port together.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Counters {
    /// Answers by class, 1xx to 5xx.
    pub codes: [f64; 5],
    pub bytes_in: f64,
    pub bytes_out: f64,
    /// Answer times as Prometheus keeps them: requests at or under each
    /// bound, the bound as Traefik wrote it (`0.1`, `+Inf`).
    pub buckets: BTreeMap<String, f64>,
}

fn grown(now: f64, before: f64) -> f64 {
    if now >= before {
        now - before
    } else {
        now
    }
}

impl Counters {
    #[must_use]
    pub fn requests(&self) -> f64 {
        self.codes.iter().sum()
    }

    /// What was added since `before`.
    #[must_use]
    pub fn since(&self, before: &Counters) -> Counters {
        let mut codes = [0.0; 5];
        for (i, c) in codes.iter_mut().enumerate() {
            *c = grown(self.codes[i], before.codes[i]);
        }
        Counters {
            codes,
            bytes_in: grown(self.bytes_in, before.bytes_in),
            bytes_out: grown(self.bytes_out, before.bytes_out),
            buckets: self
                .buckets
                .iter()
                .map(|(le, n)| {
                    (
                        le.clone(),
                        grown(*n, before.buckets.get(le).copied().unwrap_or(0.0)),
                    )
                })
                .collect(),
        }
    }

    /// The 95th percentile answer time in milliseconds, read off the
    /// buckets the way Prometheus' `histogram_quantile` does: linearly
    /// inside the bucket the rank falls in. None with no requests.
    #[must_use]
    pub fn p95_ms(&self) -> Option<f64> {
        let mut bounds: Vec<(f64, f64)> = self
            .buckets
            .iter()
            .filter_map(|(le, n)| {
                let le = if le == "+Inf" {
                    f64::INFINITY
                } else {
                    le.parse().ok()?
                };
                Some((le, *n))
            })
            .collect();
        bounds.sort_by(|a, b| a.0.total_cmp(&b.0));
        let total = bounds.last()?.1;
        if total <= 0.0 {
            return None;
        }
        let rank = 0.95 * total;
        let (mut lower, mut below) = (0.0_f64, 0.0_f64);
        for (le, count) in bounds {
            if count >= rank {
                if le.is_infinite() {
                    // Slower than every bound: the last bound is all that is known.
                    return Some((lower * 1000.0).round());
                }
                let inside = count - below;
                let share = if inside > 0.0 {
                    (rank - below) / inside
                } else {
                    1.0
                };
                return Some(((lower + (le - lower) * share) * 1000.0).round());
            }
            lower = le;
            below = count;
        }
        None
    }
}

/// Every counter at one moment.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    /// Seconds since the epoch.
    pub at: u64,
    /// When Traefik started, seconds since the epoch.
    pub started: Option<u64>,
    /// Every app port together.
    pub total: Counters,
    /// By app, its name without `app-` and the provider.
    pub apps: BTreeMap<String, Counters>,
    pub open_connections: f64,
}

/// The app a router serves: `app-notes@docker` is `notes`.
#[must_use]
pub fn app_of(router: &str) -> Option<&str> {
    let bare = router.split('@').next().unwrap_or(router);
    bare.strip_prefix("app-").filter(|n| !n.is_empty())
}

fn class(code: &str) -> Option<usize> {
    let first = code.chars().next()?.to_digit(10)?;
    (1..=5).contains(&first).then(|| first as usize - 1)
}

fn add(c: &mut Counters, s: &Sample, kind: &str) {
    match kind {
        "requests_total" => {
            if let Some(i) = class(s.label("code")) {
                c.codes[i] += s.value;
            }
        }
        "requests_bytes_total" => c.bytes_in += s.value,
        "responses_bytes_total" => c.bytes_out += s.value,
        "request_duration_seconds_bucket" => {
            *c.buckets.entry(s.label("le").to_string()).or_insert(0.0) += s.value;
        }
        _ => {}
    }
}

/// The counters Traefik wrote at `at`.
#[must_use]
pub fn snapshot(samples: &[Sample], at: u64) -> Snapshot {
    let mut snap = Snapshot {
        at,
        ..Snapshot::default()
    };
    for s in samples {
        if s.name == "process_start_time_seconds" && s.value.is_finite() && s.value > 0.0 {
            snap.started = Some(s.value as u64);
        } else if let Some(kind) = s.name.strip_prefix("traefik_entrypoint_") {
            if s.label("entrypoint").starts_with("port-") {
                add(&mut snap.total, s, kind);
            }
        } else if let Some(kind) = s.name.strip_prefix("traefik_router_") {
            if let Some(app) = app_of(s.label("router")) {
                add(snap.apps.entry(app.to_string()).or_default(), s, kind);
            }
        } else if s.name == "traefik_open_connections" && s.label("entrypoint").starts_with("port-")
        {
            snap.open_connections += s.value;
        }
    }
    snap
}

/// One sample a minute, oldest first, at most [`KEEP`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Ring {
    pub samples: Vec<Snapshot>,
}

impl Ring {
    pub fn push(&mut self, snap: Snapshot) {
        // A sample from before Traefik's restart is no baseline for one after.
        if let (Some(last), Some(started)) = (self.samples.last(), snap.started) {
            if last.started.is_some_and(|s| s != started) {
                self.samples.clear();
            }
        }
        self.samples.push(snap);
        let extra = self.samples.len().saturating_sub(KEEP);
        self.samples.drain(..extra);
    }
}

/// Seconds since the epoch as RFC 3339, in UTC.
#[must_use]
pub fn rfc3339(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rest = secs % 86_400;
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

fn whole(n: f64) -> Value {
    json!(n.max(0.0).round() as u64)
}

/// What GET /api/statistics answers when there is nothing to read.
#[must_use]
pub fn unavailable(reason: &str) -> Value {
    json!({
        "available": false,
        "reason": reason,
        "since": null,
        "totals": {},
        "codes": {},
        "protocols": {},
        "perMinute": [],
        "routes": [],
    })
}

/// The page's answer: `live` for the counts since Traefik started, the
/// ring for the hour, and each app's LAN port from Traefik's routers.
#[must_use]
pub fn report(live: &Snapshot, ring: &Ring, ports: &BTreeMap<String, u16>) -> Value {
    let t = &live.total;
    let mut per_minute = Vec::new();
    // The live sample closes the newest minute when it is a minute on.
    let mut samples: Vec<&Snapshot> = ring.samples.iter().collect();
    if samples.last().is_none_or(|last| live.at >= last.at + 60) {
        samples.push(live);
    }
    for pair in samples.windows(2) {
        let d = pair[1].total.since(&pair[0].total);
        per_minute.push(json!({
            "at": rfc3339(pair[0].at - pair[0].at % 60),
            "requests": whole(d.requests()),
            "serverErrors": whole(d.codes[4]),
            "bytesOut": whole(d.bytes_out),
            "p95Ms": d.p95_ms(),
        }));
    }
    let start = per_minute.len().saturating_sub(KEEP - 1);
    let per_minute: Vec<Value> = per_minute.split_off(start);

    let oldest = samples.first().copied().unwrap_or(live);
    let mut names: Vec<&String> = live.apps.keys().chain(ports.keys()).collect();
    names.sort();
    names.dedup();
    let routes: Vec<Value> = names
        .into_iter()
        .map(|name| {
            let now = live.apps.get(name).cloned().unwrap_or_default();
            let hour = now.since(&oldest.apps.get(name).cloned().unwrap_or_default());
            let mut r = Map::new();
            r.insert("name".into(), json!(name));
            r.insert("host".into(), Value::Null);
            if let Some(port) = ports.get(name) {
                r.insert("port".into(), json!(port));
            }
            r.insert("requests".into(), whole(now.requests()));
            r.insert("serverErrors".into(), whole(now.codes[4]));
            r.insert("bytesOut".into(), whole(now.bytes_out));
            r.insert("p95Ms".into(), json!(hour.p95_ms()));
            Value::Object(r)
        })
        .collect();

    json!({
        "available": true,
        "reason": null,
        "since": live.started.or(oldest.started).map(rfc3339),
        "totals": {
            "requests": whole(t.requests()),
            "clientErrors": whole(t.codes[3]),
            "serverErrors": whole(t.codes[4]),
            "bytesIn": whole(t.bytes_in),
            "bytesOut": whole(t.bytes_out),
            "openConnections": whole(live.open_connections),
        },
        "codes": {
            "1xx": whole(t.codes[0]),
            "2xx": whole(t.codes[1]),
            "3xx": whole(t.codes[2]),
            "4xx": whole(t.codes[3]),
            "5xx": whole(t.codes[4]),
        },
        // Traefik's metrics say http, websocket, sse or grpc, never which
        // HTTP version, and the app ports speak plain HTTP.
        "protocols": {},
        "perMinute": per_minute,
        "routes": routes,
    })
}

/// Each app's LAN port, from Traefik's `GET /api/http/routers`: a router
/// `app-<name>@…` on the entrypoint `port-<n>`.
#[must_use]
pub fn ports(routers: &Value) -> BTreeMap<String, u16> {
    let mut out = BTreeMap::new();
    for r in routers.as_array().into_iter().flatten() {
        let Some(app) = r.get("name").and_then(Value::as_str).and_then(app_of) else {
            continue;
        };
        let port = r
            .get("entryPoints")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .find_map(|ep| ep.strip_prefix("port-")?.parse::<u16>().ok());
        if let Some(port) = port {
            out.insert(app.to_string(), port);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prom::parse;

    fn metrics(notes_200: u32, notes_500: u32, started: u64) -> String {
        let total = notes_200 + notes_500;
        format!(
            r#"process_start_time_seconds {started}
traefik_entrypoint_requests_total{{code="200",entrypoint="port-30000",method="GET",protocol="http"}} {notes_200}
traefik_entrypoint_requests_total{{code="500",entrypoint="port-30000",method="GET",protocol="http"}} {notes_500}
traefik_entrypoint_requests_total{{code="404",entrypoint="port-30005",method="GET",protocol="http"}} 2
traefik_entrypoint_requests_total{{code="200",entrypoint="traefik",method="GET",protocol="http"}} 99
traefik_entrypoint_responses_bytes_total{{code="200",entrypoint="port-30000",method="GET",protocol="http"}} {bytes}
traefik_entrypoint_request_duration_seconds_bucket{{code="200",entrypoint="port-30000",method="GET",protocol="http",le="0.05"}} {notes_200}
traefik_entrypoint_request_duration_seconds_bucket{{code="200",entrypoint="port-30000",method="GET",protocol="http",le="+Inf"}} {notes_200}
traefik_entrypoint_request_duration_seconds_bucket{{code="500",entrypoint="port-30000",method="GET",protocol="http",le="0.05"}} 0
traefik_entrypoint_request_duration_seconds_bucket{{code="500",entrypoint="port-30000",method="GET",protocol="http",le="+Inf"}} {notes_500}
traefik_router_requests_total{{code="200",method="GET",protocol="http",router="app-notes@docker",service="app-notes@docker"}} {notes_200}
traefik_router_requests_total{{code="500",method="GET",protocol="http",router="app-notes@docker",service="app-notes@docker"}} {notes_500}
traefik_router_requests_total{{code="200",method="GET",protocol="http",router="dashboard@internal",service="api@internal"}} 7
traefik_router_responses_bytes_total{{code="200",method="GET",protocol="http",router="app-notes@docker",service="app-notes@docker"}} {bytes}
traefik_router_request_duration_seconds_bucket{{code="200",method="GET",protocol="http",router="app-notes@docker",service="app-notes@docker",le="0.05"}} {notes_200}
traefik_router_request_duration_seconds_bucket{{code="200",method="GET",protocol="http",router="app-notes@docker",service="app-notes@docker",le="+Inf"}} {total}
traefik_open_connections{{entrypoint="port-30000",protocol="TCP"}} 1
traefik_open_connections{{entrypoint="traefik",protocol="TCP"}} 1
"#,
            bytes = notes_200 * 100,
        )
    }

    #[test]
    fn only_the_app_ports_and_routers_count() {
        let s = snapshot(&parse(&metrics(10, 1, 1_760_000_000)), 1_760_000_100);
        assert_eq!(s.started, Some(1_760_000_000));
        assert_eq!(s.total.requests(), 13.0);
        assert_eq!(s.total.codes[3], 2.0);
        assert_eq!(s.total.codes[4], 1.0);
        assert_eq!(s.total.bytes_out, 1000.0);
        assert_eq!(s.open_connections, 1.0);
        assert_eq!(s.apps.keys().collect::<Vec<_>>(), vec!["notes"]);
        assert_eq!(s.apps["notes"].requests(), 11.0);
    }

    #[test]
    fn a_minute_is_the_difference_and_a_restart_a_new_baseline() {
        let mut ring = Ring::default();
        ring.push(snapshot(
            &parse(&metrics(10, 0, 1_760_000_000)),
            1_760_000_040,
        ));
        ring.push(snapshot(
            &parse(&metrics(25, 1, 1_760_000_000)),
            1_760_000_100,
        ));
        let live = snapshot(&parse(&metrics(30, 1, 1_760_000_000)), 1_760_000_130);
        let out = report(
            &live,
            &ring,
            &BTreeMap::from([("notes".to_string(), 30000)]),
        );
        let minutes = out["perMinute"].as_array().unwrap();
        // Two samples, and the live one only 30 s after the last: one minute.
        assert_eq!(minutes.len(), 1, "{out}");
        assert_eq!(minutes[0]["at"], "2025-10-09T08:54:00Z");
        assert_eq!(minutes[0]["requests"], 16);
        assert_eq!(minutes[0]["serverErrors"], 1);
        assert_eq!(out["totals"]["requests"], 33);
        assert_eq!(out["totals"]["clientErrors"], 2);
        assert_eq!(out["codes"]["5xx"], 1);
        assert_eq!(out["since"], "2025-10-09T08:53:20Z");
        assert_eq!(out["routes"][0]["name"], "notes");
        assert_eq!(out["routes"][0]["port"], 30000);
        assert_eq!(out["routes"][0]["host"], Value::Null);
        assert_eq!(out["routes"][0]["requests"], 31);

        // Traefik restarted: the old samples go, the new counters start over.
        ring.push(snapshot(
            &parse(&metrics(3, 0, 1_760_000_500)),
            1_760_000_560,
        ));
        assert_eq!(ring.samples.len(), 1);
    }

    #[test]
    fn the_ring_keeps_an_hour() {
        let mut ring = Ring::default();
        for i in 0..100 {
            ring.push(snapshot(
                &parse(&metrics(i, 0, 1_760_000_000)),
                1_760_000_000 + 60 * u64::from(i),
            ));
        }
        assert_eq!(ring.samples.len(), KEEP);
        let live = snapshot(
            &parse(&metrics(100, 0, 1_760_000_000)),
            1_760_000_000 + 6000,
        );
        let out = report(&live, &ring, &BTreeMap::new());
        assert_eq!(out["perMinute"].as_array().unwrap().len(), 60);
    }

    #[test]
    fn a_counter_that_went_down_counts_from_zero() {
        let before = Counters {
            codes: [0.0, 10.0, 0.0, 0.0, 0.0],
            ..Counters::default()
        };
        let after = Counters {
            codes: [0.0, 3.0, 0.0, 0.0, 0.0],
            ..Counters::default()
        };
        assert_eq!(after.since(&before).requests(), 3.0);
    }

    #[test]
    fn p95_interpolates_inside_its_bucket() {
        let c = Counters {
            buckets: BTreeMap::from([
                ("0.05".into(), 50.0),
                ("0.1".into(), 90.0),
                ("0.25".into(), 100.0),
                ("+Inf".into(), 100.0),
            ]),
            ..Counters::default()
        };
        // Rank 95 lies half way through the 0.1..0.25 bucket.
        assert_eq!(c.p95_ms(), Some(175.0));
        let slow = Counters {
            buckets: BTreeMap::from([("5".into(), 1.0), ("+Inf".into(), 10.0)]),
            ..Counters::default()
        };
        assert_eq!(slow.p95_ms(), Some(5000.0));
        assert_eq!(Counters::default().p95_ms(), None);
    }

    #[test]
    fn ports_come_from_the_routers_entrypoints() {
        let routers = json!([
            { "name": "app-notes@docker", "entryPoints": ["port-30000"] },
            { "name": "app-elsewhere@file", "entryPoints": ["port-30003"] },
            { "name": "api@internal", "entryPoints": ["traefik"] },
        ]);
        let p = ports(&routers);
        assert_eq!(p.len(), 2);
        assert_eq!(p["notes"], 30000);
        assert_eq!(p["elsewhere"], 30003);
    }

    #[test]
    fn dates_are_rfc3339_in_utc() {
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(rfc3339(1_761_134_400), "2025-10-22T12:00:00Z");
    }
}
