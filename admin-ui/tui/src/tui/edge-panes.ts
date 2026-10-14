/* The edge's panes (docs/edge-terminal.md): Overview, with the tenants and
 * their last heartbeat, the opt-ins and a Reconcile action; and Secrets,
 * listed and set. The edge has no web UI, so these words are this client's.
 *
 * The overview's document is read field by field and anything missing is
 * left out, so an edge that reports less than this expects still renders. */

import { post, type Transport } from "../api";
import { formatAgo, t, template } from "../lang";
import type { HostRecord } from "../vault";
import { whenIso, type PaneDef } from "./box-panes";
import type { PaneModel, Row, Section, Span } from "./model";
import type { Resources } from "./resources";

export type EdgePaneId = "edge-overview" | "edge-secrets";

export interface EdgeCtx {
  host: HostRecord;
  transport: Transport;
  res: Resources;
}

type Rec = Record<string, unknown>;

function rec(value: unknown): Rec | null {
  return typeof value === "object" && value !== null && !Array.isArray(value) ? (value as Rec) : null;
}

function str(value: unknown): string | null {
  return typeof value === "string" && value.length > 0 ? value : typeof value === "number" ? String(value) : null;
}

function num(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function bool(value: unknown): boolean | null {
  return typeof value === "boolean" ? value : null;
}

function list(value: unknown): unknown[] {
  if (Array.isArray(value)) return value;
  const r = rec(value);
  return r === null ? [] : Object.entries(r).map(([id, v]) => ({ id, ...(rec(v) ?? {}) }));
}

/** Seconds since the tenant's last heartbeat, from whichever field the edge sent. */
export function heartbeatAgo(tenant: Rec, now: number): number | null {
  const ago = num(tenant["seenAgoSecs"]) ?? num(tenant["seen_ago_secs"]) ?? num(tenant["lastSeenSecs"]);
  if (ago !== null) return ago;
  const at = tenant["lastHeartbeat"] ?? tenant["lastSeen"] ?? tenant["last_heartbeat"];
  if (typeof at === "number") return Math.max(0, now - at);
  if (typeof at === "string") {
    const ms = Date.parse(at);
    if (!Number.isNaN(ms)) return Math.max(0, now - ms / 1000);
  }
  return null;
}

function pad(text: string, n: number): string {
  const chars = [...text];
  return chars.length >= n ? `${chars.slice(0, n - 1).join("")}… ` : text + " ".repeat(n - chars.length);
}

function tenantTable(tenants: Rec[], cols: number, now: number): Span[][] {
  const idW = Math.max(12, Math.floor((cols - 4) * 0.3));
  const hostW = Math.max(12, Math.floor((cols - 4) * 0.4));
  const lines: Span[][] = [
    [
      { text: "  " },
      { text: pad(t("tui.edge.tenant"), idW), color: "muted", bold: true },
      { text: pad(t("tui.edge.hostname"), hostW), color: "muted", bold: true },
      { text: t("tui.edge.heartbeat"), color: "muted", bold: true },
    ],
  ];
  for (const tenant of tenants) {
    const ago = heartbeatAgo(tenant, now);
    const fresh = ago !== null && ago < 120;
    lines.push([
      { text: "  " },
      { text: pad(str(tenant["id"]) ?? str(tenant["name"]) ?? "-", idW), color: "ink" },
      { text: pad(str(tenant["hostname"]) ?? "-", hostW), color: "muted" },
      { text: fresh ? "● " : "○ ", color: fresh ? "ok" : "warn" },
      { text: ago === null ? "-" : t("tui.edge.ago", { ago: formatAgo(ago) }), color: fresh ? "ink" : "warn" },
    ]);
  }
  return lines;
}

export function edgeOverview(ctx: EdgeCtx, cols: number, now = Date.now() / 1000): PaneModel {
  const base: PaneModel = { title: t("shell.nav.overview"), summary: t("tui.edge.overviewSummary"), sections: [] };
  const res = ctx.res.get<unknown>("/overview");
  if (res.state === "loading") return { ...base, sections: [{ key: "l", rows: [{ key: "l", title: t("tui.loading"), tone: "muted" }] }] };
  if (res.state === "error") {
    return { ...base, sections: [{ key: "e", notice: { tone: "crit", title: t("tui.couldNotRead"), text: res.message }, rows: [] }] };
  }
  const doc = rec(res.data) ?? {};
  const sections: Section[] = [];

  const health = doc["health"];
  const healthy = bool(health) ?? bool(rec(health)?.["ok"]) ?? null;
  const identity = rec(doc["identity"]) ?? {};
  const official = bool(identity["official"]);
  const rows: Row[] = [
    {
      key: "health",
      title: ctx.host.url,
      value: healthy === false ? `● ${t("home.header.notAnswering")}` : `● ${t("home.header.answering")}`,
      tone: healthy === false ? "crit" : "ok",
    },
  ];
  if (official !== null) {
    rows.push({ key: "official", title: t("tui.edge.official"), value: official ? t("panes.about.yes") : t("panes.about.no"), tone: official ? "ok" : "warn" });
  }
  // `notAfter` is Unix seconds (backend-registrar/src/server/operator.rs).
  const notAfter = num(identity["notAfter"]);
  const certUntil = notAfter !== null ? new Date(notAfter * 1000).toISOString() : str(identity["expires"]);
  if (certUntil !== null) rows.push({ key: "cert", title: t("tui.edge.certificate"), value: whenIso(certUntil) });
  for (const [label, field] of [
    [t("tui.edge.enrolled"), "enrolled"],
    [t("tui.edge.routes"), "routes"],
    [t("tui.edge.domains"), "domains"],
  ] as const) {
    const value = doc[field];
    const count = Array.isArray(value) ? value.length : num(value) ?? (rec(value) === null ? null : Object.keys(rec(value)!).length);
    if (count !== null) rows.push({ key: field, title: label, value: String(count), tone: "ink" });
  }
  rows.push({
    key: "reconcile",
    title: t("tui.edge.reconcile"),
    detail: t("tui.edge.reconcileDetail"),
    action: {
      kind: "run",
      label: t("tui.edge.reconcileButton"),
      confirm: {
        title: t("tui.edge.reconcile"),
        body: [t("tui.edge.reconcileDetail")],
        confirm: t("tui.edge.reconcileButton"),
        cancel: t("tui.cancel"),
      },
      run: async () => {
        const answer = await post<unknown>(ctx.transport, "/reconcile");
        ctx.res.reload(["/overview"]);
        const message = str(rec(answer)?.["message"]);
        return message ?? t("tui.edge.reconciled");
      },
    },
  });
  sections.push({ key: "edge", title: t("tui.edge.thisEdge"), rows });

  const tenants = list(doc["tenants"]).map((t0) => rec(t0) ?? {});
  sections.push({
    key: "tenants",
    title: t("tui.edge.tenants", { count: tenants.length }),
    rows: [],
    ...(tenants.length > 0 ? { lines: tenantTable(tenants, cols, now) } : { lines: [[{ text: `  ${t("tui.edge.noTenants")}`, color: "muted" }]] }),
    caption: t("tui.edge.tenantsCaption"),
  });

  const optIns = rec(doc["optIns"]) ?? rec(doc["opt_ins"]) ?? rec(doc["optins"]);
  if (optIns !== null) {
    sections.push({
      key: "optins",
      title: t("tui.edge.optIns"),
      rows: Object.entries(optIns).map(([name, value]) => ({
        key: `opt-${name}`,
        title: name,
        value: value === true ? t("advanced.on") : value === false ? t("advanced.off") : String(value),
        tone: value === true ? "ok" : "muted",
      })),
      caption: t("tui.edge.optInsCaption"),
    });
  }
  return { ...base, sections };
}

export interface EdgeSecret {
  name: string;
  sealed: boolean;
  updated: number | string | null;
}

export function edgeSecrets(ctx: EdgeCtx): PaneModel {
  const base: PaneModel = { title: t("tui.edge.secrets"), summary: t("tui.edge.secretsSummary"), sections: [] };
  const res = ctx.res.get<EdgeSecret[]>("/secrets");
  if (res.state === "loading") return { ...base, sections: [{ key: "l", rows: [{ key: "l", title: t("tui.loading"), tone: "muted" }] }] };
  if (res.state === "error") {
    return { ...base, sections: [{ key: "e", notice: { tone: "crit", title: t("tui.couldNotRead"), text: res.message }, rows: [] }] };
  }
  const secrets = Array.isArray(res.data) ? res.data : [];
  return {
    ...base,
    sections: [
      {
        key: "secrets",
        rows:
          secrets.length === 0
            ? [{ key: "none", title: t("tui.edge.noSecrets"), tone: "muted" }]
            : secrets.map((secret) => ({
                key: `secret-${secret.name}`,
                title: secret.name,
                detail: secret.sealed
                  ? secret.updated === null
                    ? t("tui.edge.sealed")
                    : t("tui.edge.sealedAt", {
                        when: whenIso(typeof secret.updated === "number" ? new Date(secret.updated * 1000).toISOString() : secret.updated),
                      })
                  : t("tui.edge.notSealed"),
                tone: secret.sealed ? undefined : "warn",
                action: {
                  kind: "edit",
                  value: "",
                  masked: true,
                  prompt: t("tui.edge.newValue", { name: secret.name }),
                  submit: async (value: string) => {
                    await post<unknown>(ctx.transport, "/secrets", { name: secret.name, value });
                    ctx.res.reload(["/secrets"]);
                    return t("tui.edge.secretSet", { name: secret.name });
                  },
                },
              })),
        caption: t("tui.edge.secretsCaption"),
      },
    ],
  };
}

export type EdgePane = PaneDef<EdgePaneId> & { buildEdge: (ctx: EdgeCtx, cols: number) => PaneModel };

export const EDGE_PANES: readonly EdgePane[] = [
  {
    id: "edge-overview",
    label: () => t("shell.nav.overview"),
    summary: () => t("tui.edge.overviewSummary"),
    keywords: () => t("tui.edge.overviewKeywords"),
    englishLabel: () => template("shell.nav.overview", undefined, "en"),
    englishSummary: () => template("tui.edge.overviewSummary", undefined, "en"),
    englishKeywords: ["tenants", "heartbeat", "boxes", "reconcile", "health"],
    needs: ["/overview"],
    build: () => ({ title: "", sections: [] }),
    buildEdge: (ctx, cols) => edgeOverview(ctx, cols),
  },
  {
    id: "edge-secrets",
    label: () => t("tui.edge.secrets"),
    summary: () => t("tui.edge.secretsSummary"),
    keywords: () => t("tui.edge.secretsKeywords"),
    englishLabel: () => template("tui.edge.secrets", undefined, "en"),
    englishSummary: () => template("tui.edge.secretsSummary", undefined, "en"),
    englishKeywords: ["secrets", "credentials", "stripe", "seal", "keys"],
    needs: ["/secrets"],
    build: () => ({ title: "", sections: [] }),
    buildEdge: (ctx) => edgeSecrets(ctx),
  },
];
