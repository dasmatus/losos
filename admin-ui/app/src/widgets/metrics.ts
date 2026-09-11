/* Where a widget's numbers come from, and how honest each one is.
 *
 * READ THIS BEFORE ADDING A METRIC
 * --------------------------------
 * backend/schema.json lists nine routes. Not one of them reports uptime
 * history, disk usage, or how much work this box has done for the mesh. There
 * is no /api/metrics, and inventing one from the browser is not possible.
 *
 * So every metric below is one of three things, and it says which in its
 * `source` field:
 *
 *   "box"     — the appliance said so. /api/settings, /api/status, and the
 *               measurements /api/grow returns after a claim.
 *   "browser" — this browser watched it happen. The uptime heatmap and the
 *               rebuild history are journals kept here, from what this tab
 *               could see while it was open.
 *   "unknown" — nothing has measured it. The field is null and the tile draws
 *               an em-dash.
 *
 * The third case is the important one. A dashboard that fabricates a
 * plausible figure is worse than one that admits the figure does not exist:
 * the owner cannot tell the two apart, and the whole point of this box is
 * that they do not have to trust anyone else's numbers about their own data.
 * Nothing in this file makes a number up. If it does not know, it says null.
 */

import {
  getHealth,
  getSettings,
  getStatus,
  isAbort,
  type GrowResponse,
  type SettingsResponse,
  type StatusResponse,
} from "@/lib/api";
import { CODE_BASE, FILES_BASE, recallSetup } from "@/lib/apps";
import type {
  AppInfo,
  AppsMetric,
  BoxSettingsMetric,
  BoxStatusMetric,
  HeatmapDay,
  MeshMetric,
  RebuildRecord,
  RebuildsMetric,
  StorageMetric,
  UptimeMetric,
} from "./types";

// ── Calendar helpers ──────────────────────────────────────────────────────

/* Local days, never UTC.
 *
 * toISOString() would be one line shorter and wrong: it converts to UTC
 * first, so for anyone west of Greenwich every evening lands on tomorrow's
 * key and the heatmap's last column is a day ahead of the calendar on the
 * wall. The box ships Europe/Berlin, but the browser looking at it does not
 * have to be in it. */
export function dayKey(date: Date): string {
  const year = date.getFullYear();
  const month = `${date.getMonth() + 1}`.padStart(2, "0");
  const day = `${date.getDate()}`.padStart(2, "0");
  return `${year}-${month}-${day}`;
}

function startOfDay(date: Date): Date {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate());
}

function addDays(date: Date, count: number): Date {
  const next = new Date(date.getFullYear(), date.getMonth(), date.getDate());
  next.setDate(next.getDate() + count);
  return next;
}

// ── A small store on top of localStorage ──────────────────────────────────

/* Every read is total and every write is best-effort: the dashboard is not
 * worth taking down over a storage quota or a private-mode exception. */
function readJson<T>(key: string, fallback: T): T {
  try {
    const raw = window.localStorage.getItem(key);
    if (raw === null) return fallback;
    const parsed: unknown = JSON.parse(raw);
    if (typeof parsed !== "object" || parsed === null) return fallback;
    return parsed as T;
  } catch {
    return fallback;
  }
}

function writeJson(key: string, value: unknown): void {
  try {
    window.localStorage.setItem(key, JSON.stringify(value));
  } catch {
    /* nothing to do about it, and nothing depends on it */
  }
}

// ── The uptime journal ────────────────────────────────────────────────────

export const UPTIME_KEY = "losos-uptime-journal";

/** Longest window the heatmap can ask for: 53 weeks. */
export const MAX_HEATMAP_DAYS = 371;

interface DayRecord {
  /** Probes that found the box answering. */
  u: number;
  /** Probes that did not. */
  d: number;
  /** Outages that began on this day. */
  o: number;
  /** Longest single outage touching this day, in seconds. */
  l: number;
}

interface UptimeJournal {
  v: 1;
  days: Record<string, DayRecord>;
  /** What the previous probe saw, so a transition can be counted once. */
  lastUp: boolean | null;
  /** Epoch ms the current outage began, or null when the box is answering. */
  outageStart: number | null;
}

const EMPTY_JOURNAL: UptimeJournal = { v: 1, days: {}, lastUp: null, outageStart: null };

function loadJournal(): UptimeJournal {
  const raw = readJson<Partial<UptimeJournal>>(UPTIME_KEY, EMPTY_JOURNAL);
  const days = typeof raw.days === "object" && raw.days !== null ? raw.days : {};
  return {
    v: 1,
    days: days as Record<string, DayRecord>,
    lastUp: typeof raw.lastUp === "boolean" ? raw.lastUp : null,
    outageStart: typeof raw.outageStart === "number" ? raw.outageStart : null,
  };
}

function pruneJournal(journal: UptimeJournal): void {
  const cutoff = dayKey(addDays(new Date(), -MAX_HEATMAP_DAYS));
  for (const key of Object.keys(journal.days)) {
    // Lexicographic comparison is calendar order for YYYY-MM-DD, which is
    // the reason the keys are formatted that way rather than parsed here.
    if (key < cutoff) delete journal.days[key];
  }
}

/** Fold one health observation into the journal. Idempotent per call, not per
 *  moment: calling it twice in a second records two samples, which is fine —
 *  the ratio is over samples, not over wall-clock seconds. */
export function recordProbe(up: boolean, at: number = Date.now()): void {
  const journal = loadJournal();
  const key = dayKey(new Date(at));
  const day = journal.days[key] ?? { u: 0, d: 0, o: 0, l: 0 };

  if (up) {
    day.u += 1;
    if (journal.outageStart !== null) {
      day.l = Math.max(day.l, (at - journal.outageStart) / 1000);
      journal.outageStart = null;
    }
  } else {
    day.d += 1;
    if (journal.lastUp !== false || journal.outageStart === null) {
      // A new outage, which is a thing that STARTED — counted once, here.
      day.o += 1;
      journal.outageStart = at;
    }
    day.l = Math.max(day.l, (at - journal.outageStart) / 1000);
  }

  journal.days[key] = day;
  journal.lastUp = up;
  pruneJournal(journal);
  writeJson(UPTIME_KEY, journal);
}

/* The heartbeat.
 *
 * /api/health takes no token, so the journal keeps filling while the unlock
 * dialog is still up — which matters, because the moment worth recording is
 * exactly the one where the box is not answering and nobody can sign in.
 *
 * Only while the tab is visible: a backgrounded tab gets throttled to once a
 * minute at best anyway, and a hidden tab recording "down" because the
 * browser deferred its timer would be a fabricated outage. */
export const PROBE_PERIOD_MS = 60_000;

let probeTimer: ReturnType<typeof setInterval> | null = null;

async function probeOnce(): Promise<void> {
  if (document.visibilityState !== "visible") return;
  try {
    const health = await getHealth();
    recordProbe(health.ok === true);
  } catch (error) {
    if (isAbort(error)) return;
    recordProbe(false);
  }
}

/** Start the heartbeat. Safe to call more than once; returns a stopper. */
export function startUptimeProbe(): () => void {
  if (probeTimer !== null) return stopUptimeProbe;
  void probeOnce();
  probeTimer = setInterval(() => void probeOnce(), PROBE_PERIOD_MS);
  document.addEventListener("visibilitychange", onVisible);
  return stopUptimeProbe;
}

function onVisible(): void {
  if (document.visibilityState === "visible") void probeOnce();
}

export function stopUptimeProbe(): void {
  if (probeTimer !== null) clearInterval(probeTimer);
  probeTimer = null;
  document.removeEventListener("visibilitychange", onVisible);
}

function uptimeMetric(days: number): UptimeMetric {
  const journal = loadJournal();
  const window = Math.min(MAX_HEATMAP_DAYS, Math.max(1, Math.trunc(days)));
  const today = startOfDay(new Date());

  const out: HeatmapDay[] = [];
  let upSamples = 0;
  let downSamples = 0;
  let outages = 0;
  let longest = 0;
  let observed = 0;

  for (let offset = window - 1; offset >= 0; offset -= 1) {
    const date = dayKey(addDays(today, -offset));
    const record = journal.days[date];
    if (record === undefined) {
      // Never watched. NOT an outage, and drawn differently — see the
      // heatmap renderer's three cell states.
      out.push({ date, value: null });
      continue;
    }
    const total = record.u + record.d;
    observed += 1;
    upSamples += record.u;
    downSamples += record.d;
    outages += record.o;
    longest = Math.max(longest, record.l);
    out.push({ date, value: total === 0 ? null : record.u / total });
  }

  const samples = upSamples + downSamples;
  return {
    days: out,
    upRatio: samples === 0 ? 0 : upSamples / samples,
    outages,
    longestOutageSeconds: longest,
    observedDays: observed,
    source: observed === 0 ? "unknown" : "browser",
  };
}

// ── What /api/grow measured ───────────────────────────────────────────────

export const STORAGE_KEY = "losos-storage-observation";

interface GrowObservation {
  at: number;
  grew: boolean;
  beforeBytes: number;
  afterBytes: number;
  claimedBytes: number;
}

/* The only place the box ever states a size.
 *
 * POST /api/grow returns the filesystem size before and after, and how much
 * unallocated space it found to claim — real measurements taken on the box
 * with lvs, cryptsetup and resize2fs. It is also the one route that CHANGES
 * something, so a widget must never call it. The Storage screen calls it when
 * the owner asks for space; this is where it hands the numbers over.
 *
 * Integrator: call this from the grow flow. Without it the Disk tile is
 * honest and empty, which is correct but not interesting. */
export function recordGrow(response: GrowResponse): void {
  writeJson(STORAGE_KEY, {
    at: Date.now(),
    grew: response.grew,
    beforeBytes: response.beforeBytes,
    afterBytes: response.afterBytes,
    claimedBytes: response.claimedBytes,
  } satisfies GrowObservation);
}

/** What the owner's file manager says is in use, if they ever told this page.
 *  Nothing calls it yet; it exists so the Disk tile has somewhere to put a
 *  used-bytes figure the moment any route starts reporting one. */
export function recordUsedBytes(used: number, total: number): void {
  if (!Number.isFinite(used) || !Number.isFinite(total) || total <= 0) return;
  writeJson(USED_KEY, { at: Date.now(), used, total });
}

const USED_KEY = "losos-storage-used";

function storageMetric(): StorageMetric {
  const grow = readJson<Partial<GrowObservation>>(STORAGE_KEY, {});
  const used = readJson<{ at?: number; used?: number; total?: number }>(USED_KEY, {});

  const measuredTotal =
    typeof grow.afterBytes === "number" && grow.grew === true
      ? grow.afterBytes
      : typeof grow.beforeBytes === "number"
        ? grow.beforeBytes
        : null;

  // A claim consumes the reserve. Reporting the pre-grow figure afterwards
  // would tell the owner there is still room to claim when there is not.
  const reserve =
    typeof grow.claimedBytes === "number"
      ? grow.grew === true
        ? 0
        : grow.claimedBytes
      : null;

  const totalBytes = typeof used.total === "number" ? used.total : measuredTotal;
  const usedBytes = typeof used.used === "number" ? used.used : null;

  const usedRatio =
    usedBytes !== null && totalBytes !== null && totalBytes > 0 ? usedBytes / totalBytes : null;
  const freeBytes = usedBytes !== null && totalBytes !== null ? totalBytes - usedBytes : null;

  return {
    usedBytes,
    totalBytes,
    reserveBytes: reserve,
    freeBytes,
    usedRatio,
    source: totalBytes === null ? "unknown" : "box",
  };
}

// ── The rebuild journal ───────────────────────────────────────────────────

export const REBUILDS_KEY = "losos-rebuild-journal";

/** Rebuilds remembered. Short on purpose: this is a history of what this
 *  browser watched, not an audit log, and a long one invites being read as one. */
export const MAX_REBUILDS = 12;

interface RebuildJournal {
  v: 1;
  entries: RebuildRecord[];
}

/* Fold a /api/status poll into the history.
 *
 * lososd reports the CURRENT job, not a list, and it forgets the previous one
 * the moment a new rebuild starts. So the history only exists if something
 * writes each observation down as it goes past — which is what the status
 * poller already does on every screen. Integrator: call this from the poller's
 * onStatus. */
export function recordStatus(status: StatusResponse): void {
  const job = status.job;
  if (job === undefined || job.length === 0) return;

  const journal = readJson<Partial<RebuildJournal>>(REBUILDS_KEY, { v: 1, entries: [] });
  const entries = Array.isArray(journal.entries) ? [...journal.entries] : [];
  const now = Date.now();
  const index = entries.findIndex((entry) => entry.job === job);

  if (index >= 0) {
    const existing = entries[index];
    if (existing === undefined) return;
    entries[index] = {
      job,
      state: status.state,
      message: status.message,
      startedAt: existing.startedAt,
      seenAt: now,
    };
  } else {
    entries.unshift({
      job,
      state: status.state,
      message: status.message,
      startedAt: now,
      seenAt: now,
    });
  }

  entries.sort((a, b) => b.seenAt - a.seenAt);
  writeJson(REBUILDS_KEY, { v: 1, entries: entries.slice(0, MAX_REBUILDS) } satisfies RebuildJournal);
}

async function rebuildsMetric(limit: number, signal?: AbortSignal): Promise<RebuildsMetric> {
  // Ask once, so a board open while a rebuild runs is up to date even if no
  // other screen is polling.
  let live: StatusResponse | null = null;
  try {
    live = await cachedStatus(signal);
    recordStatus(live);
  } catch {
    /* the journal is still worth showing */
  }

  const journal = readJson<Partial<RebuildJournal>>(REBUILDS_KEY, { v: 1, entries: [] });
  const entries = (Array.isArray(journal.entries) ? journal.entries : []).slice(0, limit);
  const currentJob = live?.job;
  const current = entries.find((entry) => entry.job === currentJob) ?? null;

  return {
    entries,
    current,
    busy: live?.state === "building",
  };
}

// ── Settings-derived metrics ──────────────────────────────────────────────

/** Hours a day the compute window covers. Wraps midnight, because a window
 *  entered as 23:00–07:00 is eight hours and not minus sixteen. */
export function windowHours(start: string, end: string): number {
  const from = minutes(start);
  const to = minutes(end);
  if (from === null || to === null) return 0;
  const span = to >= from ? to - from : 24 * 60 - from + to;
  return span / 60;
}

function minutes(hhmm: string): number | null {
  const match = /^([01][0-9]|2[0-3]):([0-5][0-9])$/.exec(hhmm);
  if (match === null) return null;
  return Number(match[1]) * 60 + Number(match[2]);
}

function meshMetric(settings: SettingsResponse): MeshMetric {
  return {
    joined: settings.clusterEnable,
    sharingStorage: settings.sharingMyStorage,
    sharingCompute: settings.shareCompute,
    windowStart: settings.computeWindowStart,
    windowEnd: settings.computeWindowEnd,
    windowHours: settings.shareCompute
      ? windowHours(settings.computeWindowStart, settings.computeWindowEnd)
      : 0,
    // Nothing reports either figure. Not zero — zero is a claim.
    givenSeconds: null,
    takenSeconds: null,
    source: "box",
  };
}

function boxSettingsMetric(settings: SettingsResponse): BoxSettingsMetric {
  return {
    hostName: settings.hostName,
    https: settings.https,
    mode: settings.sharingMyStorage ? "mesh" : "local",
    gpu: settings.gpuEnable,
    proxy: settings.proxyEnable,
    clusterEnable: settings.clusterEnable,
    shareCompute: settings.shareCompute,
    computeWindowStart: settings.computeWindowStart,
    computeWindowEnd: settings.computeWindowEnd,
  };
}

function boxStatusMetric(status: StatusResponse): BoxStatusMetric {
  return {
    state: status.state,
    progress: status.progress,
    message: status.message,
    job: status.job ?? null,
  };
}

// ── Apps ──────────────────────────────────────────────────────────────────

/* Is the app answering?
 *
 * A same-origin request, which `connect-src 'self'` permits and which no
 * widget could have made on its own. Anything that comes back — including a
 * 404 or a redirect — means something is listening on that path; only a
 * transport failure or a 502 from Nginx means the app behind it is not up.
 *
 * HEAD rather than GET: Nextcloud's index is not small, and a dashboard tile
 * refreshing on a timer should not be pulling it down every minute. */
const PROBE_TIMEOUT_MS = 4000;

async function reachable(path: string): Promise<boolean | null> {
  try {
    const response = await fetch(path, {
      method: "HEAD",
      cache: "no-store",
      redirect: "manual",
      signal: AbortSignal.timeout(PROBE_TIMEOUT_MS),
    });
    if (response.status === 502 || response.status === 503 || response.status === 504) {
      return false;
    }
    return true;
  } catch {
    // A timeout, a refused connection, or an opaque-redirect quirk. "Cannot
    // tell", not "down": the tile says nothing rather than the wrong thing.
    return null;
  }
}

async function appsMetric(settings: SettingsResponse): Promise<AppsMetric> {
  const codeOn = settings.forgejoMode === "container" || settings.forgejoMode === "native";
  const setupDone = recallSetup() === "complete";

  const [filesUp, codeUp] = await Promise.all([
    setupDone ? reachable(FILES_BASE) : Promise.resolve(null),
    setupDone && codeOn ? reachable(`${CODE_BASE}/`) : Promise.resolve(null),
  ]);

  const apps: AppInfo[] = [
    {
      id: "files",
      name: "Files",
      path: FILES_BASE,
      reachable: filesUp,
      onMesh: settings.sharingMyStorage,
    },
  ];

  if (codeOn) {
    apps.push({
      id: "code",
      name: "Code",
      path: `${CODE_BASE}/`,
      reachable: codeUp,
      onMesh: false,
    });
  }

  return { apps, hostName: settings.hostName };
}

// ── Caching ───────────────────────────────────────────────────────────────

/* Eight tiles refreshing on their own timers would otherwise make eight
 * identical /api/settings requests a minute at a box whose CPU is also
 * serving the owner's photos. One in-flight request per key, shared. */
interface CacheEntry {
  at: number;
  value: unknown;
  pending: Promise<unknown> | null;
}

const cache = new Map<string, CacheEntry>();

const SETTINGS_TTL_MS = 30_000;
const STATUS_TTL_MS = 2_000;
const APPS_TTL_MS = 60_000;

async function cached<T>(key: string, ttl: number, load: () => Promise<T>, fresh: boolean): Promise<T> {
  const now = Date.now();
  const entry = cache.get(key);

  if (!fresh && entry !== undefined) {
    if (entry.pending !== null) return entry.pending as Promise<T>;
    if (now - entry.at < ttl) return entry.value as T;
  }

  const pending = load();
  cache.set(key, { at: now, value: entry?.value, pending });
  try {
    const value = await pending;
    cache.set(key, { at: Date.now(), value, pending: null });
    return value;
  } catch (error) {
    cache.delete(key);
    throw error;
  }
}

function cachedSettings(signal: AbortSignal | undefined, fresh = false): Promise<SettingsResponse> {
  return cached("settings", SETTINGS_TTL_MS, () => getSettings(signal === undefined ? {} : { signal }), fresh);
}

function cachedStatus(signal: AbortSignal | undefined, fresh = false): Promise<StatusResponse> {
  return cached("status", STATUS_TTL_MS, () => getStatus(signal === undefined ? {} : { signal }), fresh);
}

/** Drop everything cached. Call after an action that changes the box. */
export function invalidateMetrics(): void {
  cache.clear();
}

// ── The dispatcher ────────────────────────────────────────────────────────

export interface LoadOptions {
  days?: number;
  limit?: number;
  fresh?: boolean;
  signal?: AbortSignal;
}

/** Load one metric by name. The sandbox's `losos.metric` is a thin wrapper
 *  around this that clamps the options and refuses an unknown name. */
export async function loadMetric(name: string, options: LoadOptions = {}): Promise<unknown> {
  const fresh = options.fresh === true;

  switch (name) {
    case "uptime.days":
      return uptimeMetric(options.days ?? MAX_HEATMAP_DAYS);

    case "storage.bytes":
      return storageMetric();

    case "mesh.compute":
      return meshMetric(await cachedSettings(options.signal, fresh));

    case "apps.list":
      return cached(
        "apps",
        APPS_TTL_MS,
        async () => appsMetric(await cachedSettings(options.signal, fresh)),
        fresh,
      );

    case "rebuilds.recent":
      return rebuildsMetric(options.limit ?? MAX_REBUILDS, options.signal);

    case "box.settings":
      return boxSettingsMetric(await cachedSettings(options.signal, fresh));

    case "box.status":
      return boxStatusMetric(await cachedStatus(options.signal, fresh));

    default:
      throw new Error(`There is no metric called ${name}`);
  }
}
