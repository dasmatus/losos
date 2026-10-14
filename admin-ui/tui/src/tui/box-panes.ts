/* The box's panes: the web UI's home screen and its settings panes, as
 * models (./model.ts). Same panes, same order, same words: every title,
 * detail and caption below is a key of the web UI's own catalogue, apart
 * from the few `tui.*` strings this client adds.
 *
 * Left out on purpose: Access tokens (a token cannot manage tokens), the
 * Market and Machines (trading), the Lab, Look and Hardware (pictures and a
 * GPU switch nobody sets from a terminal), and the destructive two, factory
 * reset and erase, which stay in the web UI. About says so. */

import type {
  AppsResponse,
  BackupResponse,
  ConfigResponse,
  EdgeResponse,
  InstalledApp,
  OptionDoc,
  OptionEditor,
  OptionValue,
  ServiceMode,
  StateResponse,
  StatusResponse,
  StorageResponse,
  AppRunAs,
  AppSource,
  AppChartResponse,
} from "@app/lib/api";
import { get, post, type Transport } from "../api";
import { PANE_KEYWORDS } from "../generated/pane-keywords";
import { formatBytes, getLocale, INTL_TAG, t, template, type Key } from "../lang";
import type { HostRecord } from "../vault";
import type { Notice, SettingsForm } from "./form";
import type { PaneModel, Row, Section, Span } from "./model";
import type { Resource, Resources } from "./resources";

export type BoxPaneId =
  | "overview"
  | "storage"
  | "mesh"
  | "apps"
  | "network"
  | "security"
  | "advanced"
  | "history"
  | "backup"
  | "about";

export interface CatalogueHit {
  id: string;
  name: string;
  source: string;
  summary: string | null;
  version: string | null;
  chart: AppSource | null;
}

export type SearchState =
  | { kind: "idle" }
  | { kind: "searching"; query: string }
  | { kind: "results"; query: string; sources: string[]; apps: CatalogueHit[] }
  | { kind: "unsupported" }
  | { kind: "failed"; message: string };

export interface PaneCtx {
  host: HostRecord;
  transport: Transport;
  form: SettingsForm;
  res: Resources;
  search: SearchState;
  setSearch: (next: SearchState) => void;
  say: (notice: Notice) => void;
}

export interface PaneDef<Id extends string = string> {
  id: Id;
  label: () => string;
  summary: () => string;
  /** Extra search words in the language on screen. */
  keywords: () => string;
  /** English search words, always searched. */
  englishKeywords: readonly string[];
  englishLabel: () => string;
  englishSummary: () => string;
  needs: readonly string[];
  build: (ctx: PaneCtx) => PaneModel;
}

// ── Little helpers ──────────────────────────────────────────────────────

function loadingSection(key: string, resource: Resource<unknown>): Section | null {
  if (resource.state === "loading") return { key, rows: [{ key: `${key}-loading`, title: t("tui.loading"), tone: "muted" }] };
  if (resource.state === "error") {
    return { key, notice: { tone: "crit", title: t("tui.couldNotRead"), text: resource.message }, rows: [] };
  }
  return null;
}

function formLoading(form: SettingsForm): Section[] | null {
  if (form.loadError !== null) {
    return [{ key: "form-error", notice: { tone: "crit", title: t("tui.couldNotRead"), text: form.loadError }, rows: [] }];
  }
  if (!form.ready) return [{ key: "form-loading", rows: [{ key: "loading", title: t("tui.loading"), tone: "muted" }] }];
  return null;
}

/** A meter, the web UI's capacity bar: filled is yours, hatched is lent. */
export function meter(cols: number, used: number, lent: number, total: number): Span[] {
  const room = Math.max(10, cols);
  const usedCells = Math.round((Math.min(used, total) / total) * room);
  const lentCells = Math.min(room - usedCells, Math.round((Math.max(0, lent) / total) * room));
  return [
    { text: "█".repeat(usedCells), color: "accent" },
    { text: "▒".repeat(lentCells), color: "accent" },
    { text: "░".repeat(Math.max(0, room - usedCells - lentCells)), color: "line" },
  ];
}

export function whenIso(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  return date.toLocaleString(INTL_TAG[getLocale()], { dateStyle: "medium", timeStyle: "short" });
}

function whenUnix(seconds: number): string {
  return whenIso(new Date(seconds * 1000).toISOString());
}

function short(sha: string): string {
  return sha.slice(0, 7);
}

const MODE_LABEL: Record<ServiceMode, Key> = {
  native: "panes.apps.mode.native",
  container: "panes.apps.mode.container",
};

// ── The compute window (admin-ui/app/src/screens/settings/window.ts) ──────

function minutesOf(hhmm: string): number | null {
  const match = /^([01][0-9]|2[0-3]):([0-5][0-9])$/.exec(hhmm);
  if (match === null) return null;
  return Number(match[1]) * 60 + Number(match[2]);
}

function spansOf(start: number, end: number): { from: number; to: number }[] {
  if (start === end) return [];
  if (start < end) return [{ from: start, to: end }];
  return [
    { from: start, to: 1440 },
    { from: 0, to: end },
  ];
}

export function describeWindow(startHhmm: string, endHhmm: string): string {
  const start = minutesOf(startHhmm);
  const end = minutesOf(endHhmm);
  if (start === null || end === null) return t("settings.window.notSet");
  if (start === end) return t("settings.window.nothing");
  const total = spansOf(start, end).reduce((n, s) => n + s.to - s.from, 0);
  const hours = total / 60;
  const length =
    Number.isInteger(hours) && hours >= 1
      ? t("settings.format.hours", { count: hours })
      : t("settings.format.minutes", { count: Math.round(total) });
  return t(start > end ? "settings.window.sharedWraps" : "settings.window.shared", {
    start: startHhmm,
    end: endHhmm,
    length,
  });
}

/** The 24-hour strip, one cell per hour, lent hours in the accent. */
function hourStrip(startHhmm: string, endHhmm: string, muted: boolean): Span[][] {
  const start = minutesOf(startHhmm);
  const end = minutesOf(endHhmm);
  const spans = start === null || end === null ? [] : spansOf(start, end);
  const cells: Span[] = [];
  const labels: Span[] = [];
  for (let hour = 0; hour < 24; hour += 1) {
    let covered = 0;
    for (const s of spans) covered += Math.max(0, Math.min(s.to, hour * 60 + 60) - Math.max(s.from, hour * 60));
    const colour = muted ? "faint" : "accent";
    cells.push({ text: covered === 0 ? "░░ " : covered < 60 ? "▒▒ " : "██ ", color: covered === 0 ? "line" : colour });
    labels.push({ text: hour % 3 === 0 ? `${String(hour).padStart(2, "0")} ` : "   ", color: "faint" });
  }
  return [cells, labels];
}

// ── Overview: the web UI's home screen ───────────────────────────────────

/* The tiles of admin-ui/app/src/lib/apps.ts, in its order. Mail is left out:
 * it is drawn only once a probe of the Files app found it, and that probe
 * is not a route the envelope can reach. */
export const FILES_APPS = [
  "files",
  "dashboard",
  "photos",
  "activity",
  "contacts",
  "calendar",
  "notes",
  "bookmarks",
  "deck",
  "music",
  "collectives",
  "polls",
  "forms",
  "tables",
  "memories",
  "news",
  "tasks",
  "maps",
] as const;

function appGrid(names: string[], cols: number): Span[][] {
  const cell = 16;
  const perLine = Math.max(1, Math.floor((cols - 2) / cell));
  const out: Span[][] = [];
  for (let i = 0; i < names.length; i += perLine) {
    out.push(
      names.slice(i, i + perLine).flatMap((name): Span[] => [
        { text: "▪ ", color: "accent" },
        { text: name.padEnd(cell - 2).slice(0, cell - 2), color: "ink" },
      ]),
    );
  }
  return out;
}

const PHASE_KEY = {
  installing: "install.phase.installing",
  removing: "install.phase.removing",
  running: "install.phase.running",
  failed: "install.phase.failed",
} as const satisfies Record<InstalledApp["phase"], Key>;

function overview(ctx: PaneCtx, cols: number): PaneModel {
  const { form, res } = ctx;
  const settings = form.saved;
  const state = res.get<StateResponse>("/api/state");
  const status = res.get<StatusResponse>("/api/status");
  const storage = res.get<StorageResponse>("/api/storage");
  const apps = res.get<AppsResponse>("/api/apps");
  const health = res.get<{ ok: boolean }>("/api/health");

  const healthWord =
    health.state === "ok" && health.data.ok
      ? t("home.header.answering")
      : health.state === "loading"
        ? t("home.header.checking")
        : t("home.header.notAnswering");
  const sections: Section[] = [];

  sections.push({
    key: "box",
    rows: [
      {
        key: "name",
        title: settings?.hostName ?? t("home.header.thisBox"),
        detail: ctx.host.url,
        value: `● ${healthWord}`,
        tone: health.state === "ok" && health.data.ok ? "ok" : health.state === "loading" ? "muted" : "crit",
      },
    ],
  });

  if (status.state === "ok" && status.data.state === "building") {
    sections.push({
      key: "applying",
      notice: { tone: "accent", title: t("home.applying.title"), text: status.data.message },
      rows: [],
    });
  } else if (status.state === "ok" && status.data.state === "failed") {
    sections.push({
      key: "failed",
      notice: { tone: "crit", title: t("home.attention.changeFailed"), text: status.data.message },
      rows: [],
    });
  }

  // The apps, as the home screen's grid has them.
  const names: string[] = [];
  const filesHere = settings === null || settings.nextcloudMode === "container";
  const codeHere = settings === null || settings.forgejoMode === "container";
  if (filesHere) for (const id of FILES_APPS) names.push(t(`apps.${id}.name` as Key));
  if (codeHere) names.push(t("apps.code.name"));
  names.push(t("apps.settings.name"));
  const notice =
    !filesHere && !codeHere
      ? t("apps.notice.both")
      : !filesHere
        ? t("apps.notice.files")
        : !codeHere
          ? t("apps.notice.code")
          : undefined;
  const installedRows: Row[] = [];
  if (apps.state === "ok" && apps.data.available) {
    for (const app of apps.data.apps) {
      installedRows.push({
        key: `installed-${app.release}`,
        title: app.release,
        detail: t("install.runsAs", { chart: app.chart.name, version: app.chart.version, user: app.runAs }),
        value: t(PHASE_KEY[app.phase]),
        tone: app.phase === "running" ? "ok" : app.phase === "failed" ? "crit" : "muted",
      });
    }
  }
  sections.push({
    key: "apps",
    title: t("shell.nav.apps"),
    lines: appGrid(names, cols - 2).map((spans) => [{ text: "  " }, ...spans]),
    rows: installedRows,
    ...(notice === undefined ? {} : { caption: notice }),
  });

  // The storage strip.
  const sharing = state.state === "ok" ? state.data.sharing : (settings?.sharingMyStorage ?? false);
  const lending = !sharing ? "off" : settings?.clusterEnable === true ? "lent" : "waiting";
  const disk = storage.state === "ok" ? storage.data : null;
  const measured = disk !== null && disk.totalBytes !== null && disk.usedBytes !== null && disk.totalBytes > 0;
  const legend: Span[] = [{ text: "█ ", color: "accent" }, { text: t("home.storage.onThisBox"), color: "muted" }];
  if (measured) legend.push({ text: ` ${formatBytes(disk.usedBytes)}`, color: "faint" });
  if (lending === "lent") legend.push({ text: "   ▒ ", color: "accent" }, { text: t("home.storage.lent"), color: "muted" });
  if (measured) {
    legend.push(
      { text: "   ░ ", color: "line" },
      { text: t("home.storage.roomLeft"), color: "muted" },
      { text: ` ${formatBytes(Math.max(0, disk.totalBytes! - disk.usedBytes!))}`, color: "faint" },
    );
  }
  sections.push({
    key: "storage",
    title: t("shell.nav.storage"),
    lines: [
      [{ text: "  " }, ...(measured ? meter(cols - 6, disk.usedBytes!, 0, disk.totalBytes!) : [{ text: "░".repeat(cols - 6), color: "line" as const }])],
      [{ text: "  " }, ...legend],
    ],
    rows: [],
    caption:
      lending === "lent" ? t("home.storage.sharing") : lending === "waiting" ? t("home.storage.waiting") : t("home.storage.local"),
  });

  return { title: t("shell.nav.overview"), sections };
}

// ── Storage ──────────────────────────────────────────────────────────────

function storagePane(ctx: PaneCtx, cols: number): PaneModel {
  const storage = ctx.res.get<StorageResponse>("/api/storage");
  const base: PaneModel = { title: t("settings.panes.storage.label"), summary: t("settings.panes.storage.summary"), sections: [] };
  const waiting = loadingSection("storage", storage);
  if (waiting !== null || storage.state !== "ok") return { ...base, sections: [waiting!] };
  const facts = storage.data;
  const total = facts.totalBytes;
  const measured = total !== null && total > 0;
  const used = facts.usedBytes ?? 0;
  const reserve = facts.reserveBytes;
  const hasReserve = reserve !== null && reserve > 0;
  const spent = reserve === 0;

  const meterLines: Span[][] = measured
    ? [
        meter(cols - 8, used, 0, total),
        [
          { text: "█ ", color: "accent" },
          { text: `${t("settings.capacity.yours")} `, color: "muted" },
          { text: formatBytes(used), color: "ink" },
          { text: "   ░ ", color: "line" },
          { text: `${t("settings.capacity.free")} `, color: "muted" },
          { text: formatBytes(Math.max(0, total - used)), color: "ink" },
          { text: `   ${t("settings.capacity.total", { total: formatBytes(total) })}`, color: "muted" },
        ],
      ]
    : [[{ text: t("settings.capacity.noSize"), color: "muted" }]];

  return {
    ...base,
    sections: [
      {
        key: "disk",
        title: t("panes.storage.disk"),
        rows: [
          { key: "meter", title: t("settings.capacity.label", {
              used: formatBytes(used),
              lent: formatBytes(0),
              free: formatBytes(measured ? Math.max(0, total - used) : null),
              total: formatBytes(total),
            }), extra: meterLines },
          {
            key: "reserve",
            title: t("panes.storage.held.title"),
            detail: t(spent ? "panes.storage.held.spent" : "panes.storage.held.detail"),
            value: reserve === null ? t("panes.storage.notReported") : formatBytes(reserve),
            action: {
              kind: "run",
              label: spent ? t("panes.storage.reserveUsed") : t("panes.storage.useReserve"),
              disabled: !hasReserve,
              confirm: {
                title: t("panes.storage.dialogTitle"),
                body: [
                  `${reserve === null ? t("panes.storage.dialogUnknown") : t("panes.storage.dialogAdds", { size: formatBytes(reserve) })} ${t("panes.storage.dialogTail")}`,
                ],
                confirm: t("panes.storage.useIt"),
                cancel: t("panes.storage.cancel"),
              },
              run: async () => {
                const grown = await post<{ grew: boolean; afterBytes: number }>(ctx.transport, "/api/grow");
                ctx.res.reload(["/api/storage"]);
                return grown.grew ? t("tui.storage.grew", { size: formatBytes(grown.afterBytes) }) : t("tui.storage.notGrown");
              },
            },
          },
        ],
        caption: t("panes.storage.diskCaption"),
      },
    ],
  };
}

// ── Mesh ─────────────────────────────────────────────────────────────────

function meshPane(ctx: PaneCtx): PaneModel {
  const { form } = ctx;
  const base: PaneModel = { title: t("settings.panes.mesh.label"), summary: t("settings.panes.mesh.summary"), sections: [] };
  const edge = ctx.res.get<EdgeResponse>("/api/edge");
  const sections: Section[] = [];

  if (edge.state === "ok" && edge.data.reachable) {
    sections.push({
      key: "edge",
      title: t("panes.mesh.edge.title"),
      rows: edge.data.edges.map((e) => ({
        key: `edge-${e.url}`,
        title: `${t("panes.mesh.edge.found", { name: e.name })} ${e.official ? "✓" : "!"}`,
        detail: `${e.url}\n${t(e.official ? "panes.mesh.edge.officialTip" : "panes.mesh.edge.companyTip")}`,
        value: t(e.source === "configured" ? "panes.mesh.edge.viaInternet" : "panes.mesh.edge.viaLan"),
        tone: "ok",
      })),
      caption: t(edge.data.official ? "panes.mesh.edge.caption" : "panes.mesh.edge.captionCompany"),
    });
  } else {
    let title: string = t("panes.mesh.edge.looking");
    let detail: string | undefined;
    let value: string | undefined;
    if (edge.state === "error") {
      title = t("panes.mesh.edge.unknown");
      detail = edge.message;
    } else if (edge.state === "ok") {
      title = t("panes.mesh.edge.none");
      const tried = edge.data.configuredUrl;
      detail = edge.data.lanSearched
        ? tried === null
          ? t("panes.mesh.edge.noneDetail")
          : t("panes.mesh.edge.noneTried", { url: tried })
        : t("panes.mesh.edge.lanUnsearched");
      value = t("panes.mesh.edge.sharingOff");
    }
    sections.push({
      key: "edge",
      title: t("panes.mesh.edge.title"),
      rows: [{ key: "edge-none", title, ...(detail === undefined ? {} : { detail }), ...(value === undefined ? {} : { value, tone: "crit" as const }) }],
      caption: t("panes.mesh.edge.caption"),
    });
  }

  const waiting = formLoading(form);
  if (waiting !== null) return { ...base, sections: [...sections, ...waiting] };
  const draft = form.draft!;
  const blocked = edge.state === "ok" && !edge.data.reachable;
  const joinRefused = blocked && !(form.saved?.clusterEnable ?? false);
  const joined = draft.clusterEnable;

  sections.push({
    key: "join",
    title: t("panes.mesh.otherBoxes"),
    rows: [
      {
        key: "join",
        title: t("panes.mesh.join"),
        ...(joinRefused ? { detail: t("panes.mesh.edge.needed") } : {}),
        action: { kind: "toggle", on: joined, disabled: joinRefused, set: (next) => form.set("clusterEnable", next) },
      },
    ],
    caption: t("panes.mesh.joinCaption"),
  });

  const windowOff = !joined;
  sections.push({
    key: "spare",
    title: t("panes.mesh.spareTime"),
    rows: [
      {
        key: "lend",
        title: t("panes.mesh.lend"),
        ...(windowOff ? { detail: t("panes.mesh.joinFirst") } : {}),
        action: { kind: "toggle", on: draft.shareCompute, disabled: windowOff, set: (next) => form.set("shareCompute", next) },
        extra: [
          ...hourStrip(draft.computeWindowStart, draft.computeWindowEnd, !draft.shareCompute || windowOff),
          [{ text: describeWindow(draft.computeWindowStart, draft.computeWindowEnd), color: "muted" }],
        ],
      },
      {
        key: "from",
        title: t("panes.mesh.lendFrom"),
        action: {
          kind: "edit",
          value: draft.computeWindowStart,
          prompt: t("panes.mesh.lendFrom"),
          disabled: windowOff,
          submit: (next) => form.set("computeWindowStart", next.trim()),
        },
      },
      {
        key: "until",
        title: t("panes.mesh.lendUntil"),
        ...(form.problems.computeWindow === null ? {} : { detail: form.problems.computeWindow }),
        tone: form.problems.computeWindow === null ? undefined : "crit",
        action: {
          kind: "edit",
          value: draft.computeWindowEnd,
          prompt: t("panes.mesh.lendUntil"),
          disabled: windowOff,
          submit: (next) => form.set("computeWindowEnd", next.trim()),
        },
      } as Row,
    ],
    caption: t("panes.mesh.windowCaption", { your: t("panes.mesh.your") }),
  });
  return { ...base, sections };
}

// ── Apps ─────────────────────────────────────────────────────────────────

function parseHits(body: unknown): { sources: string[]; apps: CatalogueHit[] } {
  const record = (v: unknown): Record<string, unknown> | null =>
    typeof v === "object" && v !== null && !Array.isArray(v) ? (v as Record<string, unknown>) : null;
  const str = (v: unknown): string | null => (typeof v === "string" && v.trim().length > 0 ? v : null);
  const root = record(body);
  const apps: CatalogueHit[] = [];
  const sources: string[] = [];
  if (root === null) return { sources, apps };
  for (const entry of Array.isArray(root["results"]) ? root["results"] : []) {
    const r = record(entry);
    if (r === null) continue;
    const id = str(r["id"]);
    const name = str(r["name"]);
    const source = str(r["source"]);
    if (id === null || name === null || source === null) continue;
    const c = record(r["chart"]);
    const repo = c === null ? null : str(c["repo"]);
    const chartName = c === null ? null : str(c["name"]);
    const version = c === null ? null : str(c["version"]);
    apps.push({
      id,
      name,
      source,
      summary: str(r["summary"]),
      version: str(r["version"]),
      chart: repo !== null && chartName !== null && version !== null ? { repo, name: chartName, version } : null,
    });
  }
  for (const s of Array.isArray(root["sources"]) ? root["sources"] : []) {
    const name = str(s);
    if (name !== null) sources.push(name);
  }
  if (sources.length === 0) for (const app of apps) if (!sources.includes(app.source)) sources.push(app.source);
  return { sources, apps };
}

export async function runSearch(ctx: PaneCtx, query: string): Promise<void> {
  const q = query.trim();
  if (q.length === 0) {
    ctx.setSearch({ kind: "idle" });
    return;
  }
  ctx.setSearch({ kind: "searching", query: q });
  const answer = await ctx.transport.request("GET", `/api/apps/search?q=${encodeURIComponent(q)}`);
  if (answer.status === 404) {
    ctx.setSearch({ kind: "unsupported" });
    return;
  }
  if (answer.status >= 400) {
    const body = answer.body as { error?: unknown } | null;
    ctx.setSearch({ kind: "failed", message: typeof body?.error === "string" ? body.error : `HTTP ${answer.status}` });
    return;
  }
  ctx.setSearch({ kind: "results", query: q, ...parseHits(answer.body) });
}

function appsPane(ctx: PaneCtx): PaneModel {
  const { form } = ctx;
  const base: PaneModel = { title: t("settings.panes.apps.label"), summary: t("settings.panes.apps.summary"), sections: [] };
  const sections: Section[] = [];
  const waiting = formLoading(form);
  if (waiting !== null) sections.push(...waiting);
  else {
    const draft = form.draft!;
    const modeOptions = (["container", "native"] as const).map((value) => ({ value, label: t(MODE_LABEL[value]) }));
    sections.push({
      key: "modes",
      title: t("panes.apps.onThisBox"),
      rows: [
        {
          key: "files-mode",
          title: t("panes.apps.files.title"),
          detail: t("panes.apps.files.detail"),
          action: {
            kind: "choice",
            value: draft.nextcloudMode,
            options: modeOptions,
            set: (next) => form.set("nextcloudMode", next as ServiceMode),
          },
        },
        {
          key: "code-mode",
          title: t("panes.apps.code.title"),
          detail: t("panes.apps.code.detail"),
          action: {
            kind: "choice",
            value: draft.forgejoMode,
            options: modeOptions,
            set: (next) => form.set("forgejoMode", next as ServiceMode),
          },
        },
      ],
      caption: t("panes.apps.modeCaption", { kept: t(MODE_LABEL.container), native: t(MODE_LABEL.native) }),
    });
  }

  const apps = ctx.res.get<AppsResponse>("/api/apps");
  const records = apps.state === "ok" && apps.data.available ? apps.data.apps : [];
  const sharedAvailable = apps.state === "ok" && apps.data.available ? apps.data.sharedAvailable : false;
  if (apps.state === "error") {
    sections.push({
      key: "installed",
      title: t("install.installed"),
      notice: { tone: "crit", text: t("install.listFailed", { message: apps.message }) },
      rows: [],
    });
  } else if (records.length > 0) {
    sections.push({
      key: "installed",
      title: t("install.installed"),
      rows: records.map((record) => ({
        key: `app-${record.release}`,
        title: `${record.release}  ·  ${t(PHASE_KEY[record.phase])}`,
        detail: [
          t("install.runsAs", { chart: record.chart.name, version: record.chart.version, user: record.runAs }),
          record.phase === "failed" && record.message !== null ? record.message : "",
          record.phase === "running" && record.appPort !== null ? `${new URL(ctx.host.url).hostname}:${record.frontPort}` : "",
        ]
          .filter((s) => s.length > 0)
          .join("\n"),
        tone: record.phase === "failed" ? "crit" : undefined,
        action: {
          kind: "run",
          label: t("install.remove"),
          disabled: record.phase === "installing" || record.phase === "removing",
          confirm: {
            title: t("install.removeTitle", { name: record.release }),
            body: [t("install.removeBody", { path: `/home/${record.runAs}/data/apps/${record.release}` })],
            confirm: t("install.removeConfirm"),
            cancel: t("install.cancel"),
          },
          run: async () => {
            await post<InstalledApp>(ctx.transport, "/api/apps/remove", { release: record.release });
            ctx.res.reload(["/api/apps"]);
            return t("tui.apps.removing", { name: record.release });
          },
        },
      })),
      caption: t("install.installedCaption"),
    });
  }

  const search = ctx.search;
  const searchRows: Row[] = [
    {
      key: "search",
      title: t("panes.apps.searchLabel"),
      action: {
        kind: "edit",
        value: search.kind === "results" || search.kind === "searching" ? search.query : "",
        prompt: t("panes.apps.searchPlaceholder"),
        disabled: search.kind === "unsupported",
        submit: (query) => runSearch(ctx, query),
      },
    },
  ];
  let notice: Section["notice"];
  if (search.kind === "searching") searchRows.push({ key: "searching", title: t("panes.apps.looking"), tone: "muted" });
  else if (search.kind === "unsupported") notice = { tone: "muted", text: t("panes.apps.unsupported") };
  else if (search.kind === "failed") notice = { tone: "crit", text: t("panes.apps.failed", { message: search.message }) };
  else if (search.kind === "results" && search.apps.length === 0) {
    searchRows.push({ key: "empty", title: t("panes.apps.empty"), tone: "muted" });
  } else if (search.kind === "results") {
    for (const hit of search.apps) {
      const chart = hit.chart;
      const existing =
        chart === null ? undefined : records.find((r) => r.chart.name === chart.name && r.chart.repo === chart.repo);
      const row: Row = {
        key: `hit-${hit.source}-${hit.id}`,
        title: hit.version === null ? hit.name : `${hit.name}  ${hit.version}`,
        detail: [hit.summary ?? "", `${t("panes.apps.col.source")}: ${hit.source}`].filter((s) => s.length > 0).join("\n"),
      };
      if (chart !== null && apps.state === "ok" && apps.data.available && existing === undefined) {
        row.action = {
          kind: "run",
          label: t("install.button"),
          confirm: async () => {
            const info = await get<AppChartResponse>(
              ctx.transport,
              `/api/apps/chart?${new URLSearchParams({ repo: chart.repo, name: chart.name, version: chart.version }).toString()}`,
            );
            pendingInstall.set(hit.id, { release: info.release, chart, runAs: "notshared" });
            return {
              title: t("install.confirmTitle", { name: info.release }),
              body: [
                t("install.pointUser", { user: "notshared", path: `/home/notshared/data/apps/${info.release}` }),
                sharedAvailable ? t("tui.apps.sharedInWeb") : "",
              ].filter((s) => s.length > 0),
              confirm: t("install.confirm"),
              cancel: t("install.cancel"),
            };
          },
          run: async () => {
            const plan = pendingInstall.get(hit.id);
            if (plan === undefined) return null;
            await post<InstalledApp>(ctx.transport, "/api/apps/install", {
              release: plan.release,
              chart: plan.chart,
              runAs: plan.runAs,
              values: {},
              valuesYaml: null,
            });
            ctx.res.reload(["/api/apps"]);
            return t("install.startedTitle", { name: plan.release });
          },
        };
      } else if (existing !== undefined) {
        row.value = t(PHASE_KEY[existing.phase]);
      }
      searchRows.push(row);
    }
  }
  const footer =
    search.kind === "results" && search.sources.length > 0
      ? t("panes.apps.footerSearched", { count: search.sources.length, sources: search.sources.join(", "), footer: t("panes.apps.footer") })
      : t("panes.apps.footer");
  sections.push({
    key: "find",
    title: t("panes.apps.findMore"),
    ...(notice === undefined ? {} : { notice }),
    rows: searchRows,
    caption: apps.state === "ok" && !apps.data.available && search.kind === "results" ? `${footer} ${t("install.noCluster")}` : footer,
  });
  return { ...base, sections };
}

const pendingInstall = new Map<string, { release: string; chart: AppSource; runAs: AppRunAs }>();

// ── Network ──────────────────────────────────────────────────────────────

function networkPane(ctx: PaneCtx): PaneModel {
  const { form } = ctx;
  const base: PaneModel = { title: t("settings.panes.network.label"), summary: t("settings.panes.network.summary"), sections: [] };
  const waiting = formLoading(form);
  if (waiting !== null) return { ...base, sections: waiting };
  const draft = form.draft!;
  return {
    ...base,
    sections: [
      {
        key: "name",
        title: t("panes.network.name"),
        rows: [
          {
            key: "called",
            title: t("panes.network.called"),
            ...(form.problems.hostName === null ? {} : { detail: form.problems.hostName, tone: "crit" as const }),
            action: {
              kind: "edit",
              value: draft.hostName,
              prompt: t("panes.network.called"),
              submit: (next) => form.set("hostName", next.trim()),
            },
          },
          { key: "reached", title: t("panes.network.reachedAt"), value: draft.hostName.length > 0 ? `${draft.hostName}.local` : "-" },
        ],
        caption: t("panes.network.nameCaption"),
      },
      {
        key: "reach",
        title: t("panes.network.reaching"),
        rows: [
          {
            key: "https",
            title: t("panes.network.encrypt"),
            detail: t("panes.network.encryptDetail"),
            action: { kind: "toggle", on: draft.https, set: (next) => form.set("https", next) },
          },
          {
            key: "outside",
            title: t("panes.network.outside"),
            action: { kind: "toggle", on: draft.proxyEnable, set: (next) => form.set("proxyEnable", next) },
          },
          {
            key: "port",
            title: t("panes.network.port"),
            ...(form.problems.apachePort === null ? {} : { detail: form.problems.apachePort, tone: "crit" as const }),
            action: {
              kind: "edit",
              value: Number.isFinite(draft.apachePort) ? String(draft.apachePort) : "",
              prompt: t("panes.network.port"),
              submit: (next) => form.set("apachePort", Number.parseInt(next.trim(), 10)),
            },
          },
        ],
        caption: t("panes.network.reachCaption"),
      },
    ],
  };
}

// ── Security ─────────────────────────────────────────────────────────────

function securityPane(ctx: PaneCtx): PaneModel {
  const { form } = ctx;
  const base: PaneModel = { title: t("settings.panes.security.label"), summary: t("settings.panes.security.summary"), sections: [] };
  const waiting = formLoading(form);
  if (waiting !== null) return { ...base, sections: waiting };
  const draft = form.draft!;
  const noTpm = form.options?.options.find((o) => o.name === "tpm.enable")?.current === false;
  const sw = (key: "hardeningUsbguard" | "hardeningMalloc" | "hardeningApparmor" | "hardeningNosmt", name: string): Row => ({
    key,
    title: t(`panes.security.${name}.title` as Key),
    detail: t(`panes.security.${name}.detail` as Key),
    action: { kind: "toggle", on: draft[key], set: (next) => form.set(key, next) },
  });
  return {
    ...base,
    sections: [
      {
        key: "protections",
        title: t("panes.security.group"),
        ...(noTpm ? { notice: { tone: "warn" as const, title: t("ui.noTpm.title"), text: t("ui.noTpm.body") } } : {}),
        rows: [
          sw("hardeningUsbguard", "usbguard"),
          sw("hardeningMalloc", "malloc"),
          sw("hardeningApparmor", "apparmor"),
          sw("hardeningNosmt", "nosmt"),
        ],
        caption: t("panes.security.caption"),
      },
    ],
  };
}

// ── Advanced: every option, read only ────────────────────────────────────

function describeValue(editor: OptionEditor, value: OptionValue): string {
  if (value === null) return t("advanced.none");
  if (typeof value === "boolean") return value ? t("advanced.on") : t("advanced.off");
  if (typeof value === "number") return editor.kind === "float" && Number.isInteger(value) ? value.toFixed(1) : String(value);
  if (typeof value === "string") return value.length === 0 ? '""' : value;
  if (Array.isArray(value)) return value.length === 0 ? t("advanced.none") : value.map((v) => describeValue(editor, v)).join(", ");
  if ("package" in value) return value.package;
  return value.nix;
}

function advancedPane(ctx: PaneCtx): PaneModel {
  const { form } = ctx;
  const base: PaneModel = {
    title: t("settings.panes.advanced.label"),
    summary: t("tui.advanced.summary"),
    sections: [],
  };
  const waiting = formLoading(form);
  if (waiting !== null) return { ...base, sections: waiting };
  const options = form.options;
  if (options === null || !options.available) {
    return { ...base, sections: [{ key: "none", notice: { tone: "muted", text: t("advanced.unavailable") }, rows: [] }] };
  }
  const groups = new Map<string, OptionDoc[]>();
  for (const option of options.options) {
    if (option.hidden === true) continue;
    const list = groups.get(option.group);
    if (list === undefined) groups.set(option.group, [option]);
    else list.push(option);
  }
  const general = groups.get("general");
  groups.delete("general");
  const ordered = [...groups.entries()].sort(([a], [b]) => a.localeCompare(b));
  if (general !== undefined) ordered.unshift(["general", general]);
  const sections: Section[] = ordered.map(([group, list]) => ({
    key: `group-${group}`,
    title: group === "general" ? t("advanced.group.general") : `losos.${group}`,
    rows: list.map((option) => {
      const badges = [
        option.danger ? t("advanced.badge.danger") : "",
        option.fixed !== null ? (option.fixed === "installer" ? t("advanced.badge.installer") : t("advanced.badge.fixed", { owner: option.fixed })) : "",
        option.set !== null ? t("advanced.badge.set") : "",
      ].filter((s) => s.length > 0);
      return {
        key: `option-${option.name}`,
        title: `losos.${option.name}${badges.length > 0 ? `  [${badges.join(", ")}]` : ""}`,
        detail: [
          option.description,
          option.defaultText === null ? t("advanced.row.noDefault") : t("advanced.row.default", { text: option.defaultText }),
        ]
          .filter((s) => s.length > 0)
          .join("\n"),
        value: describeValue(option.editor, option.current),
        tone: "ink" as const,
      };
    }),
  }));
  if (options.stray.length > 0) {
    sections.push({
      key: "stray",
      title: t("advanced.stray.title"),
      rows: options.stray.map((s) => ({ key: `stray-${s.key}`, title: `losos.${s.key}`, value: s.value, tone: "crit" as const })),
      caption: t("advanced.stray.caption"),
    });
  }
  sections.push({ key: "caption", rows: [], caption: t("advanced.caption") });
  return { ...base, sections };
}

// ── History ──────────────────────────────────────────────────────────────

function historyPane(ctx: PaneCtx): PaneModel {
  const base: PaneModel = { title: t("settings.panes.history.label"), summary: t("settings.panes.history.summary"), sections: [] };
  const config = ctx.res.get<ConfigResponse>("/api/config");
  const waiting = loadingSection("config", config);
  if (waiting !== null || config.state !== "ok") return { ...base, sections: [waiting!] };
  const c = config.data;
  const repo = c.repository;
  const sections: Section[] = [];
  const repoRows: Row[] = [];
  if (repo !== null) {
    repoRows.push({ key: "git", title: t("history.repo.onGit"), detail: `${repo.owner}/${repo.name}`, value: `${ctx.host.url}${repo.url}` });
  }
  if (c.head !== null) repoRows.push({ key: "branch", title: t("history.repo.branch"), value: c.head.branch ?? "-", tone: "ink" });
  repoRows.push({ key: "head", title: t("history.repo.head"), value: c.head === null ? "-" : short(c.head.sha), tone: "ink" });
  if (repo !== null) repoRows.push({ key: "clone", title: t("history.repo.clone"), detail: `${ctx.host.url}${repo.url}.git` });
  sections.push({ key: "repo", title: t("history.repo.title"), rows: repoRows, caption: repo === null ? t("history.repo.off") : t("history.repo.caption") });

  if (repo !== null) {
    sections.push({
      key: "sync",
      title: t("history.sync.title"),
      rows: [
        {
          key: "sync",
          title: [
            t(`history.sync.state.${c.sync.state}` as Key),
            c.sync.syncedAt === null ? "" : t("history.sync.lastAt", { when: whenIso(c.sync.syncedAt) }),
          ]
            .filter((s) => s.length > 0)
            .join("  ·  "),
          detail: [c.sync.detail, c.sync.remoteHead === null ? "" : t("history.sync.remote", { sha: short(c.sync.remoteHead) })]
            .filter((s) => s.length > 0)
            .join("\n"),
          action: {
            kind: "run",
            label: t("history.sync.now"),
            run: async () => {
              await post<ConfigResponse>(ctx.transport, "/api/config/sync");
              ctx.res.reload(["/api/config"]);
              return t("tui.history.synced");
            },
          },
        },
      ],
    });
  }

  const upstreamRows: Row[] = [];
  if (c.upstream.url !== null) {
    upstreamRows.push({ key: "source", title: t("history.update.source"), detail: c.upstream.url, value: c.upstream.branch ?? "-", tone: "ink" });
  }
  upstreamRows.push({
    key: "report",
    title: [
      t(`history.update.state.${c.upstream.report.state}` as Key),
      c.upstream.report.syncedAt === null ? "" : t("history.sync.lastAt", { when: whenIso(c.upstream.report.syncedAt) }),
    ]
      .filter((s) => s.length > 0)
      .join("  ·  "),
    detail: [c.upstream.report.detail, c.upstream.report.remoteHead === null ? "" : t("history.update.remote", { sha: short(c.upstream.report.remoteHead) })]
      .filter((s) => s.length > 0)
      .join("\n"),
  });
  sections.push({
    key: "update",
    title: t("history.update.title"),
    rows: upstreamRows,
    caption: c.upstream.url === null ? t("history.update.off") : t("history.update.caption"),
  });

  sections.push({
    key: "commits",
    title: t("history.commits.title"),
    rows:
      c.log.length === 0
        ? [{ key: "empty", title: t("history.commits.empty"), tone: "muted" }]
        : c.log.map((entry) => ({ key: `commit-${entry.sha}`, title: entry.subject, detail: whenIso(entry.when), value: short(entry.sha) })),
    caption: t("history.commits.caption"),
  });
  return { ...base, sections };
}

// ── Backup ───────────────────────────────────────────────────────────────

const CLASS_KEY: Record<string, Key> = {
  standard: "panes.backup.target.classStandard",
};

function backupPane(ctx: PaneCtx): PaneModel {
  const base: PaneModel = { title: t("settings.panes.backup.label"), summary: t("settings.panes.backup.summary"), sections: [] };
  const backup = ctx.res.get<BackupResponse>("/api/backup");
  if (backup.state === "loading") return { ...base, sections: [{ key: "l", rows: [{ key: "l", title: t("panes.backup.loading"), tone: "muted" }] }] };
  if (backup.state === "error") {
    return { ...base, sections: [{ key: "e", rows: [{ key: "e", title: t("panes.backup.unreachable"), detail: backup.message, tone: "crit" }] }] };
  }
  const view = backup.data;
  const target = view.target;
  const sections: Section[] = [];
  sections.push({
    key: "target",
    title: t("panes.backup.target.title"),
    rows:
      target === null
        ? [{ key: "none", title: t("tui.backup.noTarget"), tone: "muted" }]
        : [
            { key: "endpoint", title: t("panes.backup.target.endpoint"), value: target.endpoint, tone: "ink" },
            { key: "bucket", title: t("panes.backup.target.bucket"), value: target.prefix.length > 0 ? `${target.bucket}/${target.prefix}` : target.bucket, tone: "ink" },
            {
              key: "class",
              title: t("panes.backup.target.class"),
              value: (target.storageClass ?? "standard") === "standard" ? t(CLASS_KEY["standard"]!) : (target.storageClass ?? ""),
              tone: "ink",
            },
            { key: "key", title: t("panes.backup.target.key"), value: target.accessKeyId, tone: "ink" },
          ],
    caption: t("panes.backup.target.caption"),
  });
  const job = view.job?.kind === "backup" ? view.job : null;
  const last = view.last;
  const runRows: Row[] = [
    {
      key: "last",
      title: t("panes.backup.runs.last"),
      detail: last === null ? t("panes.backup.runs.never") : t("panes.backup.runs.lastDetail", { size: formatBytes(last.bytes), count: last.files }),
      ...(last === null ? {} : { value: whenUnix(last.time) }),
    },
  ];
  if (job?.state === "running") runRows.push({ key: "running", title: t("panes.backup.runs.running"), detail: t("panes.backup.runs.runningDetail"), tone: "accent" });
  if (job?.state === "failed") runRows.push({ key: "failed", title: t("panes.backup.runs.failed"), detail: job.message, tone: "crit" });
  runRows.push({
    key: "now",
    title: t("panes.backup.runs.now"),
    ...(target === null ? { detail: t("panes.backup.runs.needsTarget") } : {}),
    action: {
      kind: "run",
      label: t("panes.backup.runs.button"),
      disabled: target === null || job?.state === "running",
      run: async () => {
        await post<{ job: string }>(ctx.transport, "/api/backup/run");
        ctx.res.reload(["/api/backup"]);
        return t("panes.backup.runs.running");
      },
    },
  });
  sections.push({ key: "runs", title: t("panes.backup.runs.title"), rows: runRows, caption: t("panes.backup.runs.caption") });
  sections.push({ key: "web", rows: [], caption: t("tui.backup.inWeb") });
  return { ...base, sections };
}

// ── About ────────────────────────────────────────────────────────────────

function aboutPane(ctx: PaneCtx): PaneModel {
  const { form } = ctx;
  const base: PaneModel = { title: t("settings.panes.about.label"), summary: t("settings.panes.about.summary"), sections: [] };
  const waiting = formLoading(form);
  if (waiting !== null) return { ...base, sections: waiting };
  const saved = form.saved!;
  const byName = `${saved.https ? "https" : "http"}://${saved.hostName}.local`;
  const storageKey: Key = !saved.sharingMyStorage
    ? "panes.about.storageLocal"
    : saved.clusterEnable
      ? "panes.about.storageShared"
      : "panes.about.storageWaiting";
  const lent = saved.clusterEnable && saved.shareCompute;
  return {
    ...base,
    sections: [
      {
        key: "box",
        title: t("panes.about.thisBox"),
        rows: [
          { key: "called", title: t("panes.about.called"), value: saved.hostName, tone: "ink" },
          { key: "reached", title: t("panes.about.reachedAt"), value: ctx.host.url, tone: "ink" },
          ...(byName !== ctx.host.url ? [{ key: "byname", title: t("panes.about.byName"), value: byName, tone: "ink" as const }] : []),
          { key: "storage", title: t("panes.about.storage"), value: t(storageKey), tone: "accent" },
          { key: "outside", title: t("panes.about.reachableOutside"), value: saved.proxyEnable ? t("panes.about.yes") : t("panes.about.no") },
        ],
        caption: t("panes.about.addressCaption", { name: `${saved.hostName}.local` }),
      },
      {
        key: "handbook",
        title: t("panes.about.handbook"),
        rows: [{ key: "handbook", title: t("panes.about.handbook"), value: `${ctx.host.url}/handbook/`, tone: "accent" }],
        caption: t("panes.about.handbookCaption"),
      },
      {
        key: "spare",
        title: t("panes.about.spareTime"),
        rows: [
          { key: "mesh", title: t("panes.about.onMesh"), value: saved.clusterEnable ? t("panes.about.joined") : t("panes.about.notJoined") },
          { key: "lent", title: t("panes.about.lentOut"), value: lent ? `${saved.computeWindowStart}-${saved.computeWindowEnd}` : t("panes.about.never") },
        ],
        caption: lent ? describeWindow(saved.computeWindowStart, saved.computeWindowEnd) : t("panes.about.idle"),
      },
      {
        key: "current",
        title: t("panes.about.current"),
        rows: [
          { key: "updates", title: t("panes.about.updates"), value: t("panes.about.everyNightAt", { time: "03:00" }) },
          { key: "restarts", title: t("panes.about.restarts"), value: t("panes.about.everyNightAt", { time: "00:07" }) },
        ],
        caption: t("panes.about.caption"),
      },
      {
        key: "terminal",
        title: t("tui.about.title"),
        rows: [
          { key: "client", title: t("tui.about.client"), value: `losos ${VERSION}`, tone: "ink" },
          { key: "hostkey", title: t("tui.about.hostKey"), detail: ctx.host.fingerprint.replace(/(.{4})(?=.)/g, "$1 ") },
          { key: "userkey", title: t("tui.about.userKey"), detail: ctx.host.keyFingerprint.replace(/(.{4})(?=.)/g, "$1 ") },
        ],
        caption: t("tui.about.webOnly"),
      },
    ],
  };
}

declare const __LOSOS_VERSION__: string | undefined;
export const VERSION: string = typeof __LOSOS_VERSION__ === "string" ? __LOSOS_VERSION__ : "dev";

// ── The list ─────────────────────────────────────────────────────────────


function settingsPane(
  id: Exclude<BoxPaneId, "overview">,
  needs: readonly string[],
  build: (ctx: PaneCtx, cols: number) => PaneModel,
): PaneDef<BoxPaneId> & { buildAt: (ctx: PaneCtx, cols: number) => PaneModel } {
  const labelKey = `settings.panes.${id}.label` as Key;
  const summaryKey = `settings.panes.${id}.summary` as Key;
  const keywordsKey = `settings.panes.${id}.keywords` as Key;
  return {
    id,
    label: () => t(labelKey),
    summary: () => t(summaryKey),
    keywords: () => t(keywordsKey),
    englishLabel: () => template(labelKey, undefined, "en"),
    englishSummary: () => template(summaryKey, undefined, "en"),
    englishKeywords: PANE_KEYWORDS[id] ?? [],
    needs,
    build: (ctx) => build(ctx, 80),
    buildAt: build,
  };
}

export type BoxPane = PaneDef<BoxPaneId> & { buildAt: (ctx: PaneCtx, cols: number) => PaneModel };

export const BOX_PANES: readonly BoxPane[] = [
  {
    id: "overview",
    label: () => t("shell.nav.overview"),
    summary: () => t("tui.overview.summary"),
    keywords: () => t("tui.overview.keywords"),
    englishLabel: () => template("shell.nav.overview", undefined, "en"),
    englishSummary: () => template("tui.overview.summary", undefined, "en"),
    englishKeywords: ["home", "status", "apps", "health"],
    needs: ["/api/health", "/api/state", "/api/status", "/api/storage", "/api/apps"],
    build: (ctx) => overview(ctx, 80),
    buildAt: overview,
  },
  settingsPane("storage", ["/api/storage"], storagePane),
  settingsPane("mesh", ["/api/edge"], (ctx) => meshPane(ctx)),
  settingsPane("apps", ["/api/apps"], (ctx) => appsPane(ctx)),
  settingsPane("network", [], (ctx) => networkPane(ctx)),
  settingsPane("security", [], (ctx) => securityPane(ctx)),
  settingsPane("advanced", [], (ctx) => advancedPane(ctx)),
  settingsPane("history", ["/api/config"], (ctx) => historyPane(ctx)),
  settingsPane("backup", ["/api/backup"], (ctx) => backupPane(ctx)),
  settingsPane("about", [], (ctx) => aboutPane(ctx)),
];

