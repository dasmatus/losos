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
  /** losos.hardening.apparmor — confines less than it looks like it does. */
  hardeningApparmor: boolean;
  /** losos.hardening.malloc — hardened_malloc; host daemons only, not pods. */
  hardeningMalloc: boolean;
  /** losos.hardening.nosmt — halves the core count. */
  hardeningNosmt: boolean;
  /** losos.hardening.usbguard — blocks USB devices absent at boot. */
  hardeningUsbguard: boolean;
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
  /** The box refused a sharing setting because no edge proxy is in reach:
   *  a 409 whose body carries `edgeRequired: true` (backend/src/edge.rs). */
  readonly edgeRequired: boolean;
  /** The box refused a market action because no edge in reach is one LosOS
   *  runs: a 409 with `officialEdgeRequired: true`. */
  readonly officialEdgeRequired: boolean;

  constructor(
    status: number,
    message: string,
    { edgeRequired = false, officialEdgeRequired = false } = {},
  ) {
    super(message);
    this.name = "ApiError";
    this.status = status;
    this.edgeRequired = edgeRequired;
    this.officialEdgeRequired = officialEdgeRequired;
  }

  /** The token this tab holds is not the one lososd minted. Re-prompt. */
  get unauthorized(): boolean {
    return this.status === 401;
  }
}

export function isUnauthorized(error: unknown): boolean {
  return error instanceof ApiError && error.unauthorized;
}

/** The claim's "not yet": lososd answered 503 because Nextcloud is still
 *  starting. Nothing is wrong and nothing was changed; wait and ask again. */
export function isNotReady(error: unknown): boolean {
  return error instanceof ApiError && error.status === 503;
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
  method?: "GET" | "POST" | "PUT" | "DELETE";
  /** JSON or Nix as text; a picture upload sends its bytes as a Blob. */
  body?: string | Blob;
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
    const { message, edgeRequired, officialEdgeRequired } = await errorBody(response);
    throw new ApiError(response.status, message, { edgeRequired, officialEdgeRequired });
  }

  return (await response.json()) as T;
}

interface ErrorBody {
  message: string;
  edgeRequired: boolean;
  officialEdgeRequired: boolean;
}

async function errorBody(response: Response): Promise<ErrorBody> {
  try {
    const body = (await response.json()) as {
      error?: unknown;
      edgeRequired?: unknown;
      officialEdgeRequired?: unknown;
    };
    const flags = {
      edgeRequired: body.edgeRequired === true,
      officialEdgeRequired: body.officialEdgeRequired === true,
    };
    if (typeof body.error === "string" && body.error.length > 0) {
      return { message: body.error, ...flags };
    }
    return { message: `HTTP ${response.status}`, ...flags };
  } catch {
    /* lososd restarting mid-rebuild answers through nginx, not as JSON */
  }
  return { message: `HTTP ${response.status}`, edgeRequired: false, officialEdgeRequired: false };
}

// ── Routes ────────────────────────────────────────────────────────────────

/** GET /api/health — public. Whether lososd is answering at all. */
export function getHealth(options: RequestOptions = {}): Promise<HealthResponse> {
  return call<HealthResponse>("/api/health", { ...options, anonymous: true });
}

export interface ClaimState {
  /** Whether an owner has ever set a password on this box. */
  claimed: boolean;
  /** Whether the first password can be set *now*. False while Nextcloud is
   *  still installing itself on a fresh box, which takes minutes; the wizard
   *  polls until it flips. Absent from a lososd older than this field, which
   *  the wizard reads as ready, so an old box is not waited on forever. */
  ready?: boolean;
  /** Why not yet, in a sentence for the owner, when `ready` is false. */
  waitingFor?: string | null;
}

export interface ClaimResponse {
  claimed: true;
  user: string | null;
  /** The admin token, released to whoever claimed the box — once, plus the
   *  replays below. */
  token: string;
  /** True when this is the reply to an earlier claim given again: the same
   *  password asked within lososd's grace window after a reply that was lost
   *  in transit (backend/src/receipt.rs). Absent on a first claim. */
  replayed?: boolean;
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
 * owner to paste it was asking for something they had no way to obtain.
 *
 * Answers 503 with `{error, ready: false, waitingFor}` while Nextcloud cannot
 * take the password yet (first boot, still installing); the box stays
 * claimable and the wizard goes back to waiting. See `isNotReady`. */
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

/* ── Edge proxies ─────────────────────────────────────────────────────────
 * What lososd found when it last looked for an edge proxy: on the LAN by
 * DNS-SD (`_losos-edge._tcp` over the Avahi the box already runs) and at the
 * configured registrar URL. Both are probed; a candidate is listed only once
 * its /health answered. The daemon refreshes this every few seconds, and the
 * same answer gates sharing: POST /api/change to mesh, or an apply that turns
 * `sharingMyStorage` or `clusterEnable` on, is a 409 `{edgeRequired: true}`
 * while `reachable` is false. See backend/src/edge.rs. */

export type EdgeSource = "lan" | "configured";

export interface EdgeProxy {
  /** The advertised instance name, or the configured URL's host. */
  name: string;
  /** Proved on this scan to be an edge LosOS runs (a certificate the LosOS
   *  root signed, a fresh nonce answered). Only official edges may process
   *  trading; a company's own edge is found and shares storage with this
   *  false. */
  official: boolean;
  /** Base URL of its registrar API. */
  url: string;
  source: EdgeSource;
}

export interface EdgeResponse {
  /** At least one edge answered. The one bit the gate reads. */
  reachable: boolean;
  /** At least one edge is official: the bit the market gate reads. */
  official: boolean;
  /** Every edge that answered, LAN first. */
  edges: EdgeProxy[];
  /** False when the LAN could not be searched at all (Avahi down). */
  lanSearched: boolean;
  /** The configured registrar URL that was tried, if any. */
  configuredUrl: string | null;
  /** Unix seconds of the scan; null before the first. */
  checkedAt: number | null;
}

/** GET /api/edge — the last edge scan. */
export function getEdge(options: RequestOptions = {}): Promise<EdgeResponse> {
  return call<EdgeResponse>("/api/edge", options);
}

/** The box refused because no edge proxy is in reach (a 409 with
 *  `edgeRequired: true`). */
export function isEdgeRequired(error: unknown): boolean {
  return error instanceof ApiError && error.status === 409 && error.edgeRequired;
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

// ── Market ────────────────────────────────────────────────────────────────

/* The optional storage/compute market, relayed by lososd to the edge (the
 * pages cannot call the edge themselves under `connect-src 'self'`). Shapes
 * are backend/schema.json's marketResponse and marketActionResponse; the
 * registrar's own documents sit inside them. Prices are in the minor unit
 * (cents) of `account.currency`. */

export type MarketKind = "storage" | "compute";

export interface MarketShelfListing {
  id: string;
  kind: MarketKind;
  unit: string;
  unit_price: number;
  currency: string;
  available: number;
}

export interface MarketOwnListing {
  id: string;
  kind: MarketKind;
  unit: string;
  unit_price: number;
  capacity: number;
  available: number;
  active: boolean;
}

export interface MarketOrder {
  id: string;
  kind: MarketKind;
  unit: string;
  quantity: number;
  amount: number;
  fee: number;
  seller_net: number;
  currency: string;
  status: "pending" | "paid" | "expired";
  created_at: number;
  paid_at: number | null;
  expires_at: number | null;
  expired: boolean;
  /** `<namespace>/<claim>` once a storage order has its volume. */
  volume: string | null;
}

export interface MarketAccount {
  fee_bps: number;
  currency: string;
  seller_onboarded: boolean;
  seller_ready: boolean;
  can_sell_storage: boolean;
  can_sell_compute: boolean;
  listings: MarketOwnListing[];
  entitlements: { storage_gib: number; compute_vcpu_hours: number; next_expiry: number | null };
  purchases: MarketOrder[];
  sales: MarketOrder[];
}

export type MarketResponse =
  | {
      available: false;
      /** Why, when the owner can see it on the Mesh pane: the edges in reach
       *  are none that LosOS runs, so trading is off while sharing works. */
      reason?: "noOfficialEdge";
    }
  | {
      available: true;
      listings: MarketShelfListing[];
      account: MarketAccount;
    };

export interface MarketActionResponse {
  available: true;
  checkout_url?: string;
  /** POST /api/market/onboard: Stripe's hosted onboarding page, when the
   *  account is not yet ready. */
  url?: string | null;
  ready?: boolean;
}

/** GET /api/market — the shelf and this box's own account, or
 *  `{ available: false }` when the market is not offered here. */
export function getMarket(options: RequestOptions = {}): Promise<MarketResponse> {
  return call<MarketResponse>("/api/market", options);
}

function marketPost(
  path: string,
  body: Record<string, unknown>,
  options: RequestOptions,
): Promise<MarketActionResponse> {
  return call<MarketActionResponse>(path, {
    ...options,
    method: "POST",
    contentType: "application/json",
    body: JSON.stringify(body),
  });
}

/** POST /api/market/onboard — start or resume Stripe onboarding. */
export function postMarketOnboard(options: RequestOptions = {}): Promise<MarketActionResponse> {
  return marketPost("/api/market/onboard", {}, options);
}

/** POST /api/market/listings — sell what this box already shares. */
export function postMarketListing(
  listing: { kind: MarketKind; unit_price: number; capacity: number },
  options: RequestOptions = {},
): Promise<MarketActionResponse> {
  return marketPost("/api/market/listings", listing, options);
}

/** POST /api/market/listings/close — stop selling a listing. */
export function postMarketClose(
  listingId: string,
  options: RequestOptions = {},
): Promise<MarketActionResponse> {
  return marketPost("/api/market/listings/close", { listing_id: listingId }, options);
}

/** POST /api/market/orders — buy; the reply carries the Checkout URL. */
export function postMarketOrder(
  listingId: string,
  quantity: number,
  options: RequestOptions = {},
): Promise<MarketActionResponse> {
  return marketPost("/api/market/orders", { listing_id: listingId, quantity }, options);
}

// ── Custom domains ────────────────────────────────────────────────────────

/* A domain the owner already has, pointed at this box through an official
 * edge (backend-registrar/src/domains.rs). lososd relays it like the market
 * and asks only an official edge. The edge gives the box a name in its own
 * DNS zone (`target`) once the box's Stripe account is ready, the owner
 * points a CNAME there and publishes the TXT value, and the edge routes the
 * domain when it sees both. Shape: backend/schema.json's domainsResponse. */

export type DomainProblem =
  | "stripeAccount"
  | "txtMissing"
  | "txtWrong"
  | "notPointing"
  | "takenElsewhere"
  | "lookupFailed";

export interface CustomDomain {
  domain: string;
  status: "live" | "waiting";
  /** `_losos-challenge.<domain>`. */
  txt_name: string;
  /** Null while the box's Stripe account is not ready. */
  txt_value: string | null;
  txt_found: boolean;
  points_here: boolean;
  problem: DomainProblem | null;
  checked_at: number | null;
  verified_at: number | null;
}

export type DomainsResponse =
  | { available: false; reason?: "noOfficialEdge" }
  | {
      available: true;
      eligible: boolean;
      reason: "stripeAccount" | null;
      /** `<label>.<zone>`: what a CNAME points at. Null until eligible. */
      target: string | null;
      /** The edge's addresses, for a domain at a zone apex. */
      addresses: string[];
      max_domains: number;
      domains: CustomDomain[];
    };

/** GET /api/domains — this box's domains on the edge. */
export function getDomains(options: RequestOptions = {}): Promise<DomainsResponse> {
  return call<DomainsResponse>("/api/domains", options);
}

/** POST /api/domains — claim a domain; the reply is the whole view. */
export function postDomainAdd(domain: string, options: RequestOptions = {}): Promise<DomainsResponse> {
  return call<DomainsResponse>("/api/domains", {
    ...options,
    method: "POST",
    contentType: "application/json",
    body: JSON.stringify({ domain }),
  });
}

/** POST /api/domains/remove — give a domain up. */
export function postDomainRemove(
  domain: string,
  options: RequestOptions = {},
): Promise<DomainsResponse> {
  return call<DomainsResponse>("/api/domains/remove", {
    ...options,
    method: "POST",
    contentType: "application/json",
    body: JSON.stringify({ domain }),
  });
}

// ── Sign-in ───────────────────────────────────────────────────────────────

export interface SignInResponse {
  user: string;
  /** The admin token, the same one the claim released. */
  token: string;
}

/* POST /api/sign-in — public. Unlock the tab with the owner's password.
 *
 * The password is the one the wizard set, and lososd checks it by asking
 * LosOS cloud (backend/src/signin.rs): there is no second copy to drift
 * from. A correct one is answered with the admin token, which is stored for
 * the tab exactly as a pasted key was. Resolves false on a wrong password
 * (401, which `call` does not treat as the session ending here — the request
 * is anonymous), throws an ApiError otherwise: a 503 means LosOS cloud could
 * not be asked, see `isNotReady`, and that is the case the spare key is for. */
export async function signInWithPassword(
  password: string,
  options: RequestOptions = {},
): Promise<boolean> {
  if (password.length === 0) return false;
  let reply: SignInResponse;
  try {
    reply = await call<SignInResponse>("/api/sign-in", {
      ...options,
      method: "POST",
      contentType: "application/json",
      body: JSON.stringify({ password }),
      anonymous: true,
    });
  } catch (error) {
    if (isUnauthorized(error)) return false;
    throw error;
  }
  saveToken(reply.token);
  return true;
}

/* Probe a pasted token against a cheap authed route and store it if lososd
 * accepts it. Resolves false on rejection, throws on anything else, and a
 * rejected candidate leaves the stored token alone. The spare way in, for
 * when LosOS cloud is not running to check the password. */
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
 * the UI has, and /api/change is never called from here.
 *
 * EVERY key the box should keep has to be emitted here. This function does not
 * patch overrides.nix, it REPLACES it — so a losos.* option that lososd parses
 * but this list omits is silently reset to its default the first time anyone
 * saves any setting, however unrelated. Adding a field to SettingsResponse
 * without adding a line here is therefore not a missing feature, it is a
 * regression in whatever that field controls. */
export function buildOverridesNix(
  settings: SettingsResponse,
  /* Every other `losos.<name>` the file should carry, as Nix literals the
   * Advanced pane produced (lib/option-value.ts): one line each, after the
   * sixteen above, sorted so the file is stable from one apply to the next
   * and the commit lososd makes shows only what changed. A name here that is
   * one of the sixteen would be a second line for the same key; the first
   * wins in lososd's parser, so the form's own value would silently win over
   * the Advanced pane's. The pane never hands one in (it edits those through
   * the form), and the filter below is the belt to that. */
  extra: Readonly<Record<string, string>> = {},
): string {
  const extraLines = Object.keys(extra)
    .filter((name) => !OWNED_NAMES.has(name))
    .sort()
    .map((name) => `  losos.${name} = ${extra[name]};`);
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
    `  losos.hardening.apparmor = ${nixBool(settings.hardeningApparmor)};`,
    `  losos.hardening.malloc = ${nixBool(settings.hardeningMalloc)};`,
    `  losos.hardening.nosmt = ${nixBool(settings.hardeningNosmt)};`,
    `  losos.hardening.usbguard = ${nixBool(settings.hardeningUsbguard)};`,
    ...extraLines,
    "}",
    "",
  ].join("\n");
}

/** The sixteen `losos.*` names the lines above always write. */
export const OWNED_NAMES: ReadonlySet<string> = new Set([
  "sharingMyStorage",
  "nextcloud.mode",
  "forgejo.mode",
  "hostName",
  "nextcloud.https",
  "gpu.enable",
  "nextcloud.apachePort",
  "proxy.enable",
  "cluster.enable",
  "cluster.shareCompute",
  "cluster.computeWindow.start",
  "cluster.computeWindow.end",
  "hardening.apparmor",
  "hardening.malloc",
  "hardening.nosmt",
  "hardening.usbguard",
]);

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

// ── The look ──────────────────────────────────────────────────────────────

/* A background picture and the widgets written by hand (backend/src/look.rs,
 * `lookResponse` in the schema). Appliance state, like the settings, but it
 * takes effect the moment it is saved: no rebuild, no Apply bar. The board
 * itself stays in the browser (lib/widgets.ts); what the box keeps is the
 * things worth seeing from a second browser. */

export type LookBackground =
  | { kind: "none" }
  | { kind: "shipped"; name: string }
  | { kind: "upload"; version: number; contentType: string; url: string };

export type HandSpan = "half" | "full";

/** One widget the owner wrote: a name, a width, and the HTML it is. */
export interface HandWidget {
  id: string;
  name: string;
  span: HandSpan;
  source: string;
}

export interface LookLimits {
  widgets: number;
  nameChars: number;
  sourceBytes: number;
  imageBytes: number;
  veil: { min: number; max: number };
}

export interface LookResponse {
  version: number;
  background: LookBackground;
  /** Percent of the theme's ground colour laid over the picture. */
  veil: number;
  widgets: HandWidget[];
  /** Names of the pictures the box ships, under /backgrounds/<name>.svg. */
  shipped: string[];
  limits: LookLimits;
}

export type LookBackgroundChoice = { kind: "none" } | { kind: "shipped"; name: string };

export interface LookPatch {
  background?: LookBackgroundChoice;
  veil?: number;
}

export interface HandWidgetDraft {
  /** Absent for a new widget; the box mints the id. */
  id?: string;
  name: string;
  span: HandSpan;
  source: string;
}

/** GET /api/look. */
export function getLook(options: RequestOptions = {}): Promise<LookResponse> {
  return call<LookResponse>("/api/look", options);
}

/** POST /api/look — pick a shipped picture or none, and/or set the veil. */
export function postLook(patch: LookPatch, options: RequestOptions = {}): Promise<LookResponse> {
  return call<LookResponse>("/api/look", {
    ...options,
    method: "POST",
    contentType: "application/json",
    body: JSON.stringify(patch),
  });
}

/** PUT /api/look/background — upload a picture, raw. lososd reads the type
 *  off the bytes, so the content type sent here is a courtesy. */
export function putBackground(file: Blob, options: RequestOptions = {}): Promise<LookResponse> {
  return call<LookResponse>("/api/look/background", {
    ...options,
    method: "PUT",
    contentType: file.type.length > 0 ? file.type : "application/octet-stream",
    body: file,
  });
}

/** DELETE /api/look/background — back to the plain ground colour. */
export function deleteBackground(options: RequestOptions = {}): Promise<LookResponse> {
  return call<LookResponse>("/api/look/background", { ...options, method: "DELETE" });
}

/** POST /api/look/widgets — add a widget, or replace the one `id` names. */
export function postHandWidget(
  draft: HandWidgetDraft,
  options: RequestOptions = {},
): Promise<{ widget: HandWidget }> {
  return call<{ widget: HandWidget }>("/api/look/widgets", {
    ...options,
    method: "POST",
    contentType: "application/json",
    body: JSON.stringify(draft),
  });
}

/** DELETE /api/look/widgets/{id}. Idempotent on the box's side. */
export function deleteHandWidget(
  id: string,
  options: RequestOptions = {},
): Promise<{ deleted: boolean }> {
  return call<{ deleted: boolean }>(`/api/look/widgets/${encodeURIComponent(id)}`, {
    ...options,
    method: "DELETE",
  });
}

// ── Every option, and the configuration repository ────────────────────────

/* The editor kind of one `losos.*` option, as flake/options-doc.nix
 * classifies its Nix type. `nullable` wraps another; `opaque` is a package,
 * a path or an attribute set, shown but never edited. The pane has to draw
 * every kind here, and a kind it cannot draw is what tests/advanced.browser.mjs
 * fails on. */
export type OptionEditor =
  | { kind: "bool" }
  | { kind: "str"; pattern?: string; form?: "path" }
  | { kind: "int"; min?: number; max?: number }
  | { kind: "float" }
  | { kind: "enum"; values: string[] }
  | { kind: "list" }
  | { kind: "nullable"; inner: OptionEditor }
  | { kind: "opaque"; reason: string };

/** A value as the option document carries it: JSON for the simple shapes, a
 *  package by its name, anything else as pretty-printed Nix. */
export type OptionValue =
  | boolean
  | number
  | string
  | null
  | OptionValue[]
  | { package: string }
  | { nix: string };

export interface OptionDoc {
  /** `losos.` is implied: "nextcloud.apachePort". */
  name: string;
  /** The first segment of the name, or "general" for a top-level option. */
  group: string;
  editor: OptionEditor;
  /** The Nix type's own description. */
  nixType: string;
  description: string;
  default: OptionValue;
  defaultText: string | null;
  /** What the last rebuild merged — the value the box runs with. */
  current: OptionValue;
  /** A wrong value leaves the box unreachable or locked out: ask first. */
  danger: boolean;
  /** Who sets it at normal priority ("installer"), or null. */
  fixed: string | null;
  readOnly: boolean;
  /** Declared `visible = false`: settable, but not drawn on the pane. */
  hidden?: boolean;
  /** The literal modules/overrides.nix assigns it, or null for none. */
  set: string | null;
}

export interface OptionsResponse {
  available: boolean;
  version: number;
  options: OptionDoc[];
  /** Assignments in overrides.nix for names the box declares no option for. */
  stray: { key: string; value: string }[];
  /** Prefixes left out of the document, with the reason. */
  excluded: Record<string, string>;
}

/** GET /api/options — every losos.* option with the current overrides joined in. */
export function getOptions(options: RequestOptions = {}): Promise<OptionsResponse> {
  return call<OptionsResponse>("/api/options", options);
}

/** How the last sync with LosOS Git went. `off`: LosOS Git is off on this
 *  box; `pending`: not synced yet; the rest are what sync_once reports. */
export type ConfigSyncState =
  | "off"
  | "pending"
  | "ok"
  | "waiting"
  | "refused"
  | "diverged"
  | "unavailable"
  | "error";

export interface ConfigRepository {
  owner: string;
  name: string;
  /** The repository's page on this box: /forgejo/<owner>/<name>. */
  url: string;
  /** The URL lososd pushes to (loopback; not for the owner). */
  clone: string;
  viaForgejo: boolean;
}

export interface ConfigHead {
  sha: string;
  branch: string | null;
}

export interface ConfigLogEntry {
  sha: string;
  /** ISO 8601 committer date. */
  when: string;
  subject: string;
}

export interface ConfigSync {
  state: ConfigSyncState;
  detail: string;
  syncedAt: string | null;
  remoteHead: string | null;
}

export interface ConfigResponse {
  enabled: boolean;
  repository: ConfigRepository | null;
  head: ConfigHead | null;
  log: ConfigLogEntry[];
  sync: ConfigSync;
}

/** GET /api/config — the configuration repository and its history. */
export function getConfig(options: RequestOptions = {}): Promise<ConfigResponse> {
  return call<ConfigResponse>("/api/config", options);
}

/** POST /api/config/sync — one sync with LosOS Git now, then the document. */
export function postConfigSync(options: RequestOptions = {}): Promise<ConfigResponse> {
  return call<ConfigResponse>("/api/config/sync", { ...options, method: "POST" });
}
