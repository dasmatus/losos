/* What a Traefik in front of something has counted, as the box (lososd's
 * GET /api/statistics) and the edge (GET /manage/api/statistics) both serve
 * it. Both read Traefik's Prometheus metrics on a loopback entrypoint and keep
 * the last 60 minutes as per-minute deltas, so the page draws the same thing
 * in either place.
 *
 * `totals`, `codes` and `protocols` count from `since`, when Traefik last
 * started, and so do a route's counts (its `p95Ms` is the last hour);
 * `perMinute` is oldest first and holds at most 60 minutes. `protocols` may be
 * empty: Traefik's metrics carry no HTTP version. A route
 * is a box or a name on the edge (`host`) and an app on the box (`port`, its
 * LAN port, with no host). */

export interface StatisticsTotals {
  requests: number;
  clientErrors: number;
  serverErrors: number;
  bytesIn: number;
  bytesOut: number;
  openConnections: number;
}

export interface StatisticsMinute {
  /** RFC 3339, the start of the minute. */
  at: string;
  requests: number;
  serverErrors: number;
  bytesOut: number;
  /** The 95th percentile answer time in milliseconds, null with no requests. */
  p95Ms: number | null;
}

export interface StatisticsRoute {
  name: string;
  host: string | null;
  port?: number;
  requests: number;
  serverErrors: number;
  bytesOut: number;
  p95Ms: number | null;
}

export const CODE_CLASSES = ["2xx", "3xx", "4xx", "5xx"] as const;
export type CodeClass = (typeof CODE_CLASSES)[number];

export const PROTOCOLS = ["http/1.1", "http/2", "http/3"] as const;
export type Protocol = (typeof PROTOCOLS)[number];

export interface Statistics {
  available: boolean;
  /** Why `available` is false, in words for the owner. */
  reason: string | null;
  /** RFC 3339: when the counters started. */
  since: string | null;
  totals: StatisticsTotals;
  codes: Record<CodeClass, number>;
  protocols: Record<Protocol, number>;
  perMinute: StatisticsMinute[];
  routes: StatisticsRoute[];
}

function count(value: unknown): number {
  return typeof value === "number" && Number.isFinite(value) && value >= 0 ? value : 0;
}

function maybeCount(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) && value >= 0 ? value : null;
}

function text(value: unknown): string | null {
  return typeof value === "string" && value !== "" ? value : null;
}

function record(value: unknown): Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};
}

function list(value: unknown): unknown[] {
  return Array.isArray(value) ? value : [];
}

/** Read a server's answer. Anything missing or of the wrong type counts as
 * zero or nothing, so an older server never takes the page down. */
export function readStatistics(json: unknown): Statistics {
  const body = record(json);
  const totals = record(body.totals);
  const codes = record(body.codes);
  const protocols = record(body.protocols);
  return {
    available: body.available === true,
    reason: text(body.reason),
    since: text(body.since),
    totals: {
      requests: count(totals.requests),
      clientErrors: count(totals.clientErrors),
      serverErrors: count(totals.serverErrors),
      bytesIn: count(totals.bytesIn),
      bytesOut: count(totals.bytesOut),
      openConnections: count(totals.openConnections),
    },
    codes: Object.fromEntries(CODE_CLASSES.map((c) => [c, count(codes[c])])) as Record<
      CodeClass,
      number
    >,
    protocols: Object.fromEntries(PROTOCOLS.map((p) => [p, count(protocols[p])])) as Record<
      Protocol,
      number
    >,
    perMinute: list(body.perMinute)
      .map(record)
      .flatMap((m) => {
        const at = text(m.at);
        return at === null
          ? []
          : [
              {
                at,
                requests: count(m.requests),
                serverErrors: count(m.serverErrors),
                bytesOut: count(m.bytesOut),
                p95Ms: maybeCount(m.p95Ms),
              },
            ];
      })
      .slice(-60),
    routes: list(body.routes)
      .map(record)
      .flatMap((r) => {
        const name = text(r.name);
        if (name === null) return [];
        const port = maybeCount(r.port);
        return [
          {
            name,
            host: text(r.host),
            ...(port !== null ? { port } : {}),
            requests: count(r.requests),
            serverErrors: count(r.serverErrors),
            bytesOut: count(r.bytesOut),
            p95Ms: maybeCount(r.p95Ms),
          },
        ];
      }),
  };
}

/** The last hour, summed from `perMinute`. */
export function lastHour(stats: Statistics): {
  requests: number;
  serverErrors: number;
  bytesOut: number;
  /** 0–1, null with no requests. */
  errorRate: number | null;
} {
  let requests = 0;
  let serverErrors = 0;
  let bytesOut = 0;
  for (const minute of stats.perMinute) {
    requests += minute.requests;
    serverErrors += minute.serverErrors;
    bytesOut += minute.bytesOut;
  }
  return {
    requests,
    serverErrors,
    bytesOut,
    errorRate: requests > 0 ? serverErrors / requests : null,
  };
}
