//! The edge's panes (docs/edge-terminal.md): Overview, with the tenants and
//! their last heartbeat, the opt-ins and a Reconcile action; and Secrets,
//! listed and set. The edge has no web UI, so these words are this client's.
//!
//! The overview's document is read field by field and anything missing is
//! left out, so an edge that reports less than this expects still renders.

use serde_json::{Map, Value};

use super::model::{
    Confirm, ConfirmSpec, Control, EditTarget, Job, PaneModel, Row, Section, Span, Tone,
};
use super::panes::{cell, str_of, Ctx};
use super::resources::Resource;
use crate::i18n::{format_ago, format_iso, format_unix, t};
use crate::theme::C;
use crate::tr;

fn text_of(v: &Value) -> Option<String> {
    match v {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// A list, or an object of objects keyed by id.
fn list(value: &Value) -> Vec<Value> {
    match value {
        Value::Array(a) => a.clone(),
        Value::Object(o) => o
            .iter()
            .map(|(id, v)| {
                let mut entry = Map::new();
                entry.insert("id".into(), Value::String(id.clone()));
                if let Value::Object(fields) = v {
                    entry.extend(fields.clone());
                }
                Value::Object(entry)
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Seconds since the tenant's last heartbeat, from whichever field the edge
/// sent.
pub fn heartbeat_ago(tenant: &Value, now: f64) -> Option<f64> {
    for field in ["seenAgoSecs", "seen_ago_secs", "lastSeenSecs"] {
        if let Some(ago) = tenant[field].as_f64() {
            return Some(ago);
        }
    }
    let at = ["lastHeartbeat", "lastSeen", "last_heartbeat"]
        .iter()
        .map(|f| &tenant[*f])
        .find(|v| !v.is_null())?;
    if let Some(secs) = at.as_f64() {
        return Some((now - secs).max(0.0));
    }
    let parsed = chrono::DateTime::parse_from_rfc3339(at.as_str()?).ok()?;
    Some((now - parsed.timestamp() as f64).max(0.0))
}

fn tenant_table(tenants: &[Value], cols: usize, now: f64) -> Vec<Vec<Span>> {
    let room = cols.saturating_sub(4) as f64;
    let id_w = ((room * 0.3) as usize).max(12);
    let host_w = ((room * 0.4) as usize).max(12);
    let mut lines = vec![vec![
        Span::raw("  "),
        Span::new(cell(&t("tui.edge.tenant"), id_w), C::Muted).bold(),
        Span::new(cell(&t("tui.edge.hostname"), host_w), C::Muted).bold(),
        Span::new(t("tui.edge.heartbeat"), C::Muted).bold(),
    ]];
    for tenant in tenants {
        let ago = heartbeat_ago(tenant, now);
        let fresh = ago.is_some_and(|a| a < 120.0);
        let id = text_of(&tenant["id"])
            .or_else(|| text_of(&tenant["name"]))
            .unwrap_or_else(|| "-".into());
        lines.push(vec![
            Span::raw("  "),
            Span::new(cell(&id, id_w), C::Ink),
            Span::new(
                cell(
                    &text_of(&tenant["hostname"]).unwrap_or_else(|| "-".into()),
                    host_w,
                ),
                C::Muted,
            ),
            Span::new(
                if fresh { "● " } else { "○ " },
                if fresh { C::Ok } else { C::Warn },
            ),
            Span::new(
                match ago {
                    Some(a) => tr!("tui.edge.ago", ago = format_ago(a)),
                    None => "-".into(),
                },
                if fresh { C::Ink } else { C::Warn },
            ),
        ]);
    }
    lines
}

fn waiting(base: PaneModel, resource: &Resource) -> Result<&Value, PaneModel> {
    match resource {
        Resource::Loading => Err(base.with(vec![
            Section::new("l").row(Row::new("l", t("tui.loading")).tone(Tone::Muted))
        ])),
        Resource::Error { message, .. } => Err(base.with(vec![Section::new("e").notice(
            Tone::Crit,
            Some(t("tui.couldNotRead")),
            message.clone(),
        )])),
        Resource::Ok(v) => Ok(v),
    }
}

pub fn overview(ctx: &Ctx<'_>, cols: usize) -> PaneModel {
    let base = PaneModel::new(t("shell.nav.overview"), t("tui.edge.overviewSummary"));
    let doc = match waiting(base.clone(), ctx.res.get("/overview")) {
        Ok(v) => v,
        Err(model) => return model,
    };
    let mut sections = Vec::new();

    let health = &doc["health"];
    let healthy = health.as_bool().or_else(|| health["ok"].as_bool());
    let identity = &doc["identity"];
    let mut rows = vec![Row::new("health", ctx.host.url.clone())
        .value(format!(
            "● {}",
            t(if healthy == Some(false) {
                "home.header.notAnswering"
            } else {
                "home.header.answering"
            })
        ))
        .tone(if healthy == Some(false) {
            Tone::Crit
        } else {
            Tone::Ok
        })];
    if let Some(official) = identity["official"].as_bool() {
        rows.push(
            Row::new("official", t("tui.edge.official"))
                .value(t(if official {
                    "panes.about.yes"
                } else {
                    "panes.about.no"
                }))
                .tone(if official { Tone::Ok } else { Tone::Warn }),
        );
    }
    // `notAfter` is Unix seconds (backend-registrar/src/server/operator.rs).
    let cert_until = match identity["notAfter"].as_f64() {
        Some(secs) => Some(format_unix(secs)),
        None => str_of(&identity["expires"]).map(format_iso),
    };
    if let Some(until) = cert_until {
        rows.push(Row::new("cert", t("tui.edge.certificate")).value(until));
    }
    for (label, field) in [
        ("tui.edge.enrolled", "enrolled"),
        ("tui.edge.routes", "routes"),
        ("tui.edge.domains", "domains"),
    ] {
        let value = &doc[field];
        let count = match value {
            Value::Array(a) => Some(a.len() as f64),
            Value::Number(n) => n.as_f64(),
            Value::Object(o) => Some(o.len() as f64),
            _ => None,
        };
        if let Some(count) = count {
            rows.push(
                Row::new(field, t(label))
                    .value(count.to_string())
                    .tone(Tone::Ink),
            );
        }
    }
    rows.push(
        Row::new("reconcile", t("tui.edge.reconcile"))
            .detail(t("tui.edge.reconcileDetail"))
            .control(Control::Run {
                label: t("tui.edge.reconcileButton"),
                disabled: false,
                confirm: Some(Confirm::Ready(ConfirmSpec {
                    title: t("tui.edge.reconcile"),
                    body: vec![t("tui.edge.reconcileDetail")],
                    confirm: t("tui.edge.reconcileButton"),
                    cancel: t("tui.cancel"),
                })),
                job: Some(Job::Reconcile),
            }),
    );
    sections.push(
        Section::new("edge")
            .title(t("tui.edge.thisEdge"))
            .rows(rows),
    );

    let tenants = list(&doc["tenants"]);
    let lines = if tenants.is_empty() {
        vec![vec![Span::new(
            format!("  {}", t("tui.edge.noTenants")),
            C::Muted,
        )]]
    } else {
        tenant_table(&tenants, cols, ctx.now)
    };
    sections.push(
        Section::new("tenants")
            .title(tr!("tui.edge.tenants", count = tenants.len()))
            .lines(lines)
            .caption(t("tui.edge.tenantsCaption")),
    );

    let opt_ins = ["optIns", "opt_ins", "optins"]
        .iter()
        .find_map(|f| doc[*f].as_object());
    if let Some(opt_ins) = opt_ins {
        let rows = opt_ins
            .iter()
            .map(|(name, value)| {
                Row::new(format!("opt-{name}"), name.clone())
                    .value(match value {
                        Value::Bool(true) => t("advanced.on"),
                        Value::Bool(false) => t("advanced.off"),
                        Value::String(s) => s.clone(),
                        other => other.to_string(),
                    })
                    .tone(if *value == Value::Bool(true) {
                        Tone::Ok
                    } else {
                        Tone::Muted
                    })
            })
            .collect();
        sections.push(
            Section::new("optins")
                .title(t("tui.edge.optIns"))
                .rows(rows)
                .caption(t("tui.edge.optInsCaption")),
        );
    }
    base.with(sections)
}

/// When a secret was sealed, as the edge reports it: Unix seconds or RFC 3339.
pub fn sealed_when(updated: &Value) -> Option<String> {
    match updated {
        Value::Number(n) => n.as_f64().map(format_unix),
        Value::String(s) => Some(format_iso(s)),
        _ => None,
    }
}

pub fn secrets(ctx: &Ctx<'_>) -> PaneModel {
    let base = PaneModel::new(t("tui.edge.secrets"), t("tui.edge.secretsSummary"));
    let doc = match waiting(base.clone(), ctx.res.get("/secrets")) {
        Ok(v) => v,
        Err(model) => return model,
    };
    let secrets = doc.as_array().cloned().unwrap_or_default();
    let rows = if secrets.is_empty() {
        vec![Row::new("none", t("tui.edge.noSecrets")).tone(Tone::Muted)]
    } else {
        secrets
            .iter()
            .map(|secret| {
                let name = secret["name"].as_str().unwrap_or("-").to_string();
                let sealed = secret["sealed"] == Value::Bool(true);
                let detail = if !sealed {
                    t("tui.edge.notSealed")
                } else {
                    match sealed_when(&secret["updated"]) {
                        Some(when) => tr!("tui.edge.sealedAt", when = when),
                        None => t("tui.edge.sealed"),
                    }
                };
                let mut row = Row::new(format!("secret-{name}"), name.clone())
                    .detail(detail)
                    .control(Control::Edit {
                        value: String::new(),
                        prompt: tr!("tui.edge.newValue", name = name.clone()),
                        masked: true,
                        disabled: false,
                        target: EditTarget::Secret(name),
                    });
                if !sealed {
                    row = row.tone(Tone::Warn);
                }
                row
            })
            .collect()
    };
    base.with(vec![Section::new("secrets")
        .rows(rows)
        .caption(t("tui.edge.secretsCaption"))])
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn heartbeats_from_any_field_the_edge_sends() {
        assert_eq!(
            heartbeat_ago(&json!({"seenAgoSecs": 12}), 1000.0),
            Some(12.0)
        );
        assert_eq!(
            heartbeat_ago(&json!({"lastHeartbeat": 900}), 1000.0),
            Some(100.0)
        );
        assert_eq!(
            heartbeat_ago(&json!({"lastSeen": "1970-01-01T00:16:00Z"}), 1000.0),
            Some(40.0)
        );
        assert_eq!(heartbeat_ago(&json!({}), 1000.0), None);
    }

    #[test]
    fn a_map_of_tenants_reads_as_a_list() {
        let tenants = list(&json!({"a": {"hostname": "x"}, "b": {}}));
        assert_eq!(tenants.len(), 2);
        assert_eq!(tenants[0]["id"], "a");
        assert_eq!(tenants[0]["hostname"], "x");
    }
}
