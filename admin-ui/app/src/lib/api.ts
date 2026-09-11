/* The lososd admin API, typed.
 *
 * Wire contract: backend/schema.json. Every shape here is transcribed from
 * it — if the two disagree, the schema is right and this file is a bug.
 *
 * lososd binds 127.0.0.1:8082; the front Nginx vhost proxies /api/* to it, so
 * from the browser everything is same-origin and relative. Every route except
 * /api/health wants `Authorization: Bearer <token>`, where the token is 64
 * lowercase hex characters lososd minted into /var/secrets/losos-admin-token
 * on first start. The user reads it off the box and pastes it in.
 *
 * Token storage is sessionStorage under 'losos-token' — the same key the old
 * plain-JS admin pages used, so a token pasted before an upgrade still works
 * after it. Per-tab is the point: closing the tab logs the box out.
 */

// ── Wire types (backend/schema.json) ──────────────────────────────────────

/** `local` = private Nextcloud; `mesh` = contributes storage to the pool. */
export type Mode = "local" | "mesh";

/** Lifecycle of the last or current rebuild. */
export type RebuildState = "idle" | "building" | "done" | "failed";

/** Where a service runs. Never spell either of these at the user. */
export type ServiceMode = "native" | "container";

export interface StateResponse {
  mode: Mode;
  /** True iff mode === "mesh". Mirrors losos.sharingMyStorage. */
  sharing: boolean;
}

export interface SettingsResponse {
  sharingMyStorage: boolean;
  nextcloudMode: ServiceMode;
  forgejoMode: ServiceMode;
  hostName: string;
  https: boolean;
  gpuEnable: boolean;
  apachePort: number;
  proxyEnable: boolean;
  clusterEnable: boolean;
  shareCompute: boolean;
  /** HH:MM, 24-hour, in the box's own time zone. */
  computeWindowStart: string;
  /** HH:MM. An end before the start wraps midnight. */
  computeWindowEnd: string;
}

export interface StatusResponse {
  state: RebuildState;
  /** 0 while building (coarse), 100 on success. */
  progress: number;
  /** Human-readable; on failure the last rebuild log line. */
  message: string;
  /** Which rebuild this describes. Absent when state is "idle". */
  job?: string;
}

export interface ChangeResponse {
  job: string;
}

export interface FactoryResetResponse {
  job: string;
  /** Always true; tells a reset ack apart from a change ack. */
  reset: boolean;
}

export interface GrowResponse {
  /** Measured, not inferred from exit status. The only field worth trusting. */
  grew: boolean;
  beforeBytes: number;
  afterBytes: number;
  claimedBytes: number;
}

export interface HealthResponse {
  ok: boolean;
}

// ── Errors ────────────────────────────────────────────────────────────────

/** A non-2xx from lososd, carrying its `{"error": "..."}` message. */
export class ApiError extends Error {
  readonly status: number;

  constructor(status: number, message: string) {
    super(message);
    this.name = "ApiError";
    this.status = status;
  }

  /** The token this tab holds is not the one lososd minted. Re-prompt. */
  get unauthorized(): boolean {
    return this.status === 401;
  }
}

export function isUnauthorized(error: unknown): boolean {
  return error instanceof ApiError && error.unauthorized;
}

/** The poller's own abort, or the tab navigating away. Not an outage. */
export function isAbort(error: unknown): boolean {
  return error instanceof DOMException && error.name === "AbortError";
}

// ── Token ─────────────────────────────────────────────────────────────────

const TOKEN_KEY = "losos-token";

/** Exactly what lososd accepts: 64 lowercase hex characters. */
export const TOKEN_PATTERN = /^[0-9a-f]{64}$/;

export function isWellFormedToken(candidate: string): boolean {
  return TOKEN_PATTERN.test(candidate.trim());
}

export function getToken(): string | null {
  try {
    return window.sessionStorage.getItem(TOKEN_KEY);
  } catch {
    return null;
  }
}

export function saveToken(token: string): void {
  try {
    window.sessionStorage.setItem(TOKEN_KEY, token);
  } catch {
    /* storage blocked; the tab stays signed in only in memory */
  }
  notifyAuth();
}

export function dropToken(): void {
  try {
    window.sessionStorage.removeItem(TOKEN_KEY);
  } catch {
    /* nothing to drop */
  }
  notifyAuth();
}

/* A 401 on a stored token is the session ending, and every screen has to
 * react to it — re-prompt, stop polling, disable the destructive buttons. So
 * the eviction happens once, here, and the app shell subscribes rather than
 * each caller remembering to check. `saveToken` fires the same signal, which
 * is what un-blocks the screens after a successful re-prompt. */
const authListeners = new Set<() => void>();

function notifyAuth(): void {
  for (const fn of authListeners) fn();
}

/** Subscribe to "the token changed" — signed in, signed out, or rejected. */
export function subscribeAuth(onChange: () => void): () => void {
  authListeners.add(onChange);
  return () => {
    authListeners.delete(onChange);
  };
}

/** For useSyncExternalStore: whether this tab currently holds a token. */
export function hasToken(): boolean {
  return getToken() !== null;
}

// ── Transport ─────────────────────────────────────────────────────────────

export interface RequestOptions {
  signal?: AbortSignal;
  /* Probe with a candidate token instead of the stored one. A rejected
   * candidate must NOT log the tab out — the unlock prompt tries a token
   * that is not the stored one, and a typo there would otherwise evict a
   * perfectly good session. */
  token?: string;
}

interface CallOptions extends RequestOptions {
  method?: "GET" | "POST";
  body?: string;
  contentType?: string;
  /** /api/health is the one route that takes no token. */
  anonymous?: boolean;
}

async function call<T>(path: string, options: CallOptions = {}): Promise<T> {
  const { method = "GET", body, contentType, signal, token, anonymous } = options;

  const headers: Record<string, string> = {};
  if (contentType !== undefined) headers["Content-Type"] = contentType;
  const bearer = anonymous === true ? null : (token ?? getToken());
  if (bearer !== null) headers["Authorization"] = `Bearer ${bearer}`;

  const init: RequestInit = { method, headers, cache: "no-store" };
  if (body !== undefined) init.body = body;
  if (signal !== undefined) init.signal = signal;

  const response = await fetch(path, init);

  if (response.status === 401) {
    // Only the stored token gets evicted; see RequestOptions.token.
    if (token === undefined && anonymous !== true) dropToken();
    throw new ApiError(401, "unauthorized");
  }

  if (!response.ok) {
    throw new ApiError(response.status, await errorMessage(response));
  }

  return (await response.json()) as T;
}

async function errorMessage(response: Response): Promise<string> {
  try {
    const body = (await response.json()) as { error?: unknown };
    if (typeof body.error === "string" && body.error.length > 0) return body.error;
  } catch {
    /* lososd restarting mid-rebuild answers through nginx, not as JSON */
  }
  return `HTTP ${response.status}`;
}

// ── Routes ────────────────────────────────────────────────────────────────

/** GET /api/health — public. Whether lososd is answering at all. */
export function getHealth(options: RequestOptions = {}): Promise<HealthResponse> {
  return call<HealthResponse>("/api/health", { ...options, anonymous: true });
}

export interface ClaimState {
  /** Whether an owner has ever set a password on this box. */
  claimed: boolean;
}

export interface ClaimResponse {
  claimed: true;
  user: string | null;
  /** The admin token, released exactly once, to whoever claimed the box. */
  token: string;
}

/* GET /api/setup/claim — public. Has this box got an owner yet?
 *
 * Public because it has to be: on an unclaimed box nobody holds a token, and
 * this is the question whose answer decides whether the page shows the setup
 * wizard or asks for one. It discloses a single bit about a machine the caller
 * has already reached on the LAN. */
export function getClaimState(options: RequestOptions = {}): Promise<ClaimState> {
  return call<ClaimState>("/api/setup/claim", { ...options, anonymous: true });
}

/* POST /api/setup/claim — public, once, and never again.
 *
 * Takes ownership of a box nobody has set up: sets the first password and
 * returns the admin token so this tab can carry on authenticated. The window
 * is open only while the box is unclaimed; afterwards lososd refuses, so this
 * cannot be used to re-read the token later.
 *
 * The reason it exists rather than a key prompt: `losos.admin.tokenFile` is 64
 * random hex characters written 0600 by lososd on first start, on an appliance
 * with no SSH and no shell logins. Nothing prints it anywhere. Asking a new
 * owner to paste it was asking for something they had no way to obtain. */
export function claimBox(
  password: string,
  user?: string,
  options: RequestOptions = {},
): Promise<ClaimResponse> {
  return call<ClaimResponse>("/api/setup/claim", {
    ...options,
    method: "POST",
    contentType: "application/json",
    body: JSON.stringify(user === undefined ? { password } : { user, password }),
    anonymous: true,
  });
}

/** GET /api/state — current mode and the sharing flag. */
export function getState(options: RequestOptions = {}): Promise<StateResponse> {
  return call<StateResponse>("/api/state", options);
}

/** GET /api/settings — the losos.* options parsed out of overrides.nix. */
export function getSettings(options: RequestOptions = {}): Promise<SettingsResponse> {
  return call<SettingsResponse>("/api/settings", options);
}

/** GET /api/status — rebuild progress. Poll it with {@link createStatusPoller}. */
export function getStatus(options: RequestOptions = {}): Promise<StatusResponse> {
  return call<StatusResponse>("/api/status", options);
}

/** POST /api/change — flip local/mesh and start a rebuild. Returns at once. */
export function postChange(mode: Mode, options: RequestOptions = {}): Promise<ChangeResponse> {
  return call<ChangeResponse>("/api/change", {
    ...options,
    method: "POST",
    contentType: "application/json",
    body: JSON.stringify({ mode }),
  });
}

/** POST /api/apply — overwrite overrides.nix with `nix` and rebuild.
 *
 * The body is raw Nix, not JSON. Build it with {@link buildOverridesNix} —
 * never by hand, and never by interpolating a user value into a template
 * without {@link nixString}. */
export function postApply(nix: string, options: RequestOptions = {}): Promise<ChangeResponse> {
  return call<ChangeResponse>("/api/apply", {
    ...options,
    method: "POST",
    contentType: "text/plain",
    body: nix,
  });
}

/** POST /api/factory-reset — restore the default settings and rebuild. */
export function postFactoryReset(options: RequestOptions = {}): Promise<FactoryResetResponse> {
  return call<FactoryResetResponse>("/api/factory-reset", { ...options, method: "POST" });
}

/** POST /api/grow — extend the storage into unallocated space, online.
 *
 * Slow (three tools run in sequence against a mounted filesystem) and it
 * triggers no rebuild, so there is no job to poll: the response IS the
 * outcome. Give it a generous timeout and read `grew`. */
export function postGrow(options: RequestOptions = {}): Promise<GrowResponse> {
  return call<GrowResponse>("/api/grow", { ...options, method: "POST" });
}

// ── Sign-in ───────────────────────────────────────────────────────────────

/* Probe a pasted token against a cheap authed route and store it if lososd
 * accepts it. Resolves false on rejection, throws on anything else, and a
 * rejected candidate leaves the stored token alone. */
export async function signIn(candidate: string, options: RequestOptions = {}): Promise<boolean> {
  const token = candidate.trim();
  if (token.length === 0) return false;
  try {
    await getState({ ...options, token });
  } catch (error) {
    if (isUnauthorized(error)) return false;
    throw error;
  }
  saveToken(token);
  return true;
}

// ── Status polling ────────────────────────────────────────────────────────

export interface StatusPollerHandlers {
  onStatus: (status: StatusResponse) => void;
  /** The stored token stopped working. Stop, re-prompt, do not keep asking. */
  onUnauthorized?: () => void;
  /** Anything that is not a 401 and not an abort. Optional: transport blips
   *  are expected mid-rebuild and painting them as outages is noise. */
  onError?: (error: unknown) => void;
  periodMs?: number;
}

export interface StatusPoller {
  start: () => void;
  stop: () => void;
  setPeriod: (ms: number) => void;
  isRunning: () => boolean;
}

/** Poll /api/status. Two seconds keeps up with a rebuild log. */
export const STATUS_PERIOD_MS = 2000;
/** Nothing running: watch for one starting, do not hammer the box. */
export const IDLE_STATUS_PERIOD_MS = 30000;

/* A self-scheduling timer, not setInterval.
 *
 * setInterval fires on the clock whether or not the previous round finished,
 * so rounds stack and land out of order. That is not theoretical here:
 * `nixos-rebuild switch` restarts lososd mid-rebuild, so polls routinely
 * outlive their period, and a late `building` landing after a fresh `done`
 * walks the UI backwards.
 *
 * So: one round at a time, the period counted from when a round finishes, the
 * in-flight request aborted on stop(), and the round skipped outright while
 * the tab is hidden. Each round carries a generation number that stop() and
 * start() bump, so a response arriving after either is dropped rather than
 * applied on top of newer state.
 *
 * Ported from the plain-JS admin UI's common.js, which earned every one of
 * those properties the hard way. */
export function createStatusPoller(handlers: StatusPollerHandlers): StatusPoller {
  let period = handlers.periodMs ?? STATUS_PERIOD_MS;
  let timer: ReturnType<typeof setTimeout> | null = null;
  let controller: AbortController | null = null;
  let generation = 0;
  let running = false;

  /* Coming back to a visible tab refreshes now, not at the end of a period
   * that was already counting down when it was backgrounded. Registered in
   * start() and torn down in stop() so a screen that unmounts its poller
   * leaves nothing on the document — a route change would otherwise stack one
   * of these per visit, each holding a dead poller alive. */
  function onVisibilityChange(): void {
    if (!running || document.visibilityState !== "visible" || timer === null) return;
    clearTimeout(timer);
    timer = null;
    void round(generation);
  }

  function stop(): void {
    running = false;
    generation += 1;
    document.removeEventListener("visibilitychange", onVisibilityChange);
    if (timer !== null) {
      clearTimeout(timer);
      timer = null;
    }
    if (controller !== null) {
      controller.abort();
      controller = null;
    }
  }

  function schedule(mine: number): void {
    if (!running || mine !== generation) return;
    timer = setTimeout(() => void round(mine), period);
  }

  async function round(mine: number): Promise<void> {
    timer = null;
    if (!running || mine !== generation) return;
    // A backgrounded tab has nothing to render and no business asking.
    if (document.visibilityState === "hidden") {
      schedule(mine);
      return;
    }

    if (getToken() === null) {
      stop();
      handlers.onUnauthorized?.();
      return;
    }

    controller = new AbortController();
    try {
      const status = await getStatus({ signal: controller.signal });
      if (running && mine === generation) handlers.onStatus(status);
    } catch (error) {
      if (isUnauthorized(error)) {
        stop();
        handlers.onUnauthorized?.();
        return;
      }
      // A 502 while lososd restarts mid-rebuild is expected. Keep polling;
      // a real outage shows up in the status, not as a dead page.
      if (!isAbort(error)) handlers.onError?.(error);
    }
    if (mine === generation) controller = null;
    schedule(mine);
  }

  function start(): void {
    stop();
    running = true;
    document.addEventListener("visibilitychange", onVisibilityChange);
    void round(generation);
  }

  return {
    start,
    stop,
    setPeriod: (ms: number) => {
      period = ms;
    },
    isRunning: () => running,
  };
}

// ── Writing overrides.nix ─────────────────────────────────────────────────

/* Escape a value into a Nix double-quoted string literal.
 *
 * Nix interpolates ${...} inside double quotes, so escaping \ and " alone is
 * not escaping: a value containing ${...} lands in overrides.nix verbatim and
 * is evaluated AS ROOT on the next rebuild. Order matters — backslashes
 * first, or the escapes introduced below get escaped in turn.
 *
 * Every string written into the file goes through here, including ones a
 * validator has already reduced to five characters from a fixed alphabet.
 * The escaping is what makes generating this file from a browser safe at all;
 * a value exempted because today's validator happens to run first is how that
 * property gets quietly lost. */
export function nixString(value: string): string {
  return `"${value.replace(/\\/g, "\\\\").replace(/"/g, '\\"').replace(/\$\{/g, "\\${")}"`;
}

function nixBool(value: boolean): string {
  return value ? "true" : "false";
}

/* Build the full modules/overrides.nix body lososd writes.
 *
 * The format matches the backend's line-based parser and its
 * defaultOverridesNix exactly: a `{ ... }:` header, then one
 * `losos.<key> = <value>;` per line, booleans and integers bare, strings
 * double-quoted. POST the result to /api/apply; that is the only write path
 * the UI has, and /api/change is never called from here. */
export function buildOverridesNix(settings: SettingsResponse): string {
  return [
    "{ ... }:",
    "{",
    `  losos.sharingMyStorage = ${nixBool(settings.sharingMyStorage)};`,
    `  losos.nextcloud.mode = ${nixString(settings.nextcloudMode)};`,
    `  losos.forgejo.mode = ${nixString(settings.forgejoMode)};`,
    `  losos.hostName = ${nixString(settings.hostName)};`,
    `  losos.nextcloud.https = ${nixBool(settings.https)};`,
    `  losos.gpu.enable = ${nixBool(settings.gpuEnable)};`,
    `  losos.nextcloud.apachePort = ${settings.apachePort};`,
    `  losos.proxy.enable = ${nixBool(settings.proxyEnable)};`,
    `  losos.cluster.enable = ${nixBool(settings.clusterEnable)};`,
    `  losos.cluster.shareCompute = ${nixBool(settings.shareCompute)};`,
    `  losos.cluster.computeWindow.start = ${nixString(settings.computeWindowStart)};`,
    `  losos.cluster.computeWindow.end = ${nixString(settings.computeWindowEnd)};`,
    "}",
    "",
  ].join("\n");
}

// ── Validators the server also enforces ───────────────────────────────────

/** HH:MM on a 24-hour clock. Anchored: "23:00 " is a rejection here rather
 *  than a 400 from /api/apply after the button was already armed. */
export const HHMM_PATTERN = /^([01][0-9]|2[0-3]):[0-5][0-9]$/;

export function isValidTime(value: string): boolean {
  return HHMM_PATTERN.test(value);
}

/* An RFC 1123 label. The value becomes the system hostname, the mDNS name,
 * Nextcloud's trusted_domains and Forgejo's ROOT_URL. A space or a slash
 * fails evaluation; a leading hyphen or a 64th character gives a box that
 * boots unreachable — and there is no SSH and no shell login to fix it. */
export const HOSTNAME_PATTERN = /^[A-Za-z0-9]([A-Za-z0-9-]{0,61}[A-Za-z0-9])?$/;

export function isValidHostName(value: string): boolean {
  return HOSTNAME_PATTERN.test(value);
}

export function isValidPort(value: number): boolean {
  return Number.isInteger(value) && value >= 1024 && value <= 65535;
}
