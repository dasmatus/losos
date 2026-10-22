//! The statistics plugin: `GET /api/statistics`, what Traefik on the box
//! counted for the installed apps (`modules/apps.nix`).
//!
//! Traefik answers its Prometheus metrics and its API on a loopback
//! entrypoint, which lososd hands over as `LOSOS_TRAEFIK_URL`. A timer reads
//! the metrics once a minute and keeps the last hour in the plugin's cache
//! ([`model::Ring`]); a request reads them once more for the counts as they
//! are now. The cache is lososd's memory, so a lososd restart starts the
//! hour again; Traefik's own counters, and so the totals, carry on.

pub mod model;
pub mod prom;

use losos_plugin::{cache, clock, config, http, log, Plugin, Request, Response};
use model::Ring;

const RING: &str = "ring";

fn traefik() -> Option<String> {
    config::get("traefik").filter(|u| !u.trim().is_empty())
}

fn read_metrics(base: &str) -> Result<model::Snapshot, String> {
    let answer = http::Request::get(&format!("{base}/metrics"))
        .timeout_ms(5_000)
        .fetch(8 << 20)
        .map_err(|e| e.to_string())?;
    if !answer.is_success() {
        return Err(format!("Traefik answered {}", answer.status));
    }
    Ok(model::snapshot(&prom::parse(&answer.text()), clock::now()))
}

fn sample() {
    let Some(base) = traefik() else { return };
    match read_metrics(&base) {
        Ok(snap) => {
            let mut ring: Ring = cache::get_json(RING).unwrap_or_default();
            ring.push(snap);
            cache::set_json(RING, &ring, 0);
        }
        Err(e) => log::debug(&format!("no sample this minute: {e}")),
    }
}

pub struct StatisticsPlugin;

impl Plugin for StatisticsPlugin {
    fn handle(req: Request) -> Response {
        if (req.method.as_str(), req.route.as_str()) != ("GET", "/api/statistics") {
            return Response::error(404, "not found");
        }
        let Some(base) = traefik() else {
            return Response::ok(&model::unavailable(
                "This box opens no apps on the LAN, so there is nothing to count.",
            ));
        };
        let live = match read_metrics(&base) {
            Ok(s) => s,
            Err(e) => {
                log::warn(&format!("reading Traefik's metrics: {e}"));
                return Response::ok(&model::unavailable(
                    "The part of the box that opens the apps' ports is not answering yet.",
                ));
            }
        };
        let ports = http::Request::get(&format!("{base}/api/http/routers"))
            .timeout_ms(5_000)
            .fetch(4 << 20)
            .ok()
            .and_then(|a| a.json::<serde_json::Value>())
            .map(|v| model::ports(&v))
            .unwrap_or_default();
        let ring: Ring = cache::get_json(RING).unwrap_or_default();
        Response::ok(&model::report(&live, &ring, &ports))
    }

    fn tick(timer: &str) {
        if matches!(timer, "start" | "sample") {
            sample();
        }
    }
}

losos_plugin::export!(StatisticsPlugin);
