/* The lososd admin API, typed.
 *
 * Wire contract: backend/schema.json. Every shape here is transcribed from
 * it — if the two disagree, the schema is right and this file is a bug.
 *
 * lososd binds 127.0.0.1:8082; the front Nginx vhost proxies /api/* to it, so
 * from the browser everything is same-origin and relative. Every route except
 * /api/health wants `Authorization: Bearer <token>`, where the token is 64
 * lowercase hex characters the box made at first boot and keeps sealed as its
 * `admin-token` credential (modules/credentials.nix). The owner is shown it
 * once, at the claim, and pastes it in.
 *
 * Token storage is sessionStorage under 'losos-token' — the same key the old
 * plain-JS admin pages used, so a token pasted before an upgrade still works
 * after it. Per-tab is the point: closing the tab logs the box out.
 */

import * as React from "react";

// ── Wire types (backend/schema.json) ──────────────────────────────────────

/** `local` = private LosOS Cloud; `mesh` = contributes storage to the pool. */
export type Mode = "local" | "mesh";

/** Lifecycle of the last or current rebuild. */
export type RebuildState = "idle" | "building" | "done" | "failed";

export interface StateResponse {
  mode: Mode;
  /** True iff mode === "mesh". Mirrors losos.sharingMyStorage. */
  sharing: boolean;
}

export interface SettingsResponse {
  sharingMyStorage: boolean;
  hostName: string;
  gpuEnable: boolean;
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

/** GET /api/storage: df and vgs on the box. A reading it cannot take is null. */
export interface StorageResponse {
  totalBytes: number | null;
  usedBytes: number | null;
  /** 0 once the reserve has been claimed. */
  reserveBytes: number | null;
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
  /** A word for the refusal, where the route gives one (`code` in the
   *  body), so the page can say it in the reader's language. */
  readonly code: string | null;

  constructor(
    status: number,
    message: string,
    {
      edgeRequired = false,
      officialEdgeRequired = false,
      code = null,
    }: { edgeRequired?: boolean; officialEdgeRequired?: boolean; code?: string | null } = {},
  ) {
    super(message);
    this.name = "ApiError";
    this.status = status;
    this.edgeRequired = edgeRequired;
    this.officialEdgeRequired = officialEdgeRequired;
    this.code = code;
  }

  /** The token this tab holds is not the one lososd minted. Re-prompt. */
  get unauthorized(): boolean {
    return this.status === 401;
  }
}

export function isUnauthorized(error: unknown): boolean {
  return error instanceof ApiError && error.unauthorized;
}

/** The claim's "not yet": lososd answered 503 because the box is still
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
  me = null;
  notifyAuth();
}

// ── The signed-in person ──────────────────────────────────────────────────

/* People sign in at /auth/, Authelia's page on this box (modules/identity.nix),
 * and its session cookie rides on every /api/ request after that; nginx asks
 * Authelia about each one and lososd knows the person by name. The page holds
 * no secret for it, only what GET /api/me said. The spare key is the other
 * way in, kept in sessionStorage as before, and lososd answers /api/me for
 * it too (`spareKey: true`). */
export interface Me {
  /** The sign-in name, or null for the spare key. */
  name: string | null;
  displayName: string | null;
  /** Their profile picture (`/api/profiles/<name>/picture?v=N`), null for
   *  none, missing for the spare key or an older lososd. */
  picture?: string | null;
  /** Their colour, one of ACCENTS. */
  accent?: Accent;
  groups: string[];
  /** In losos-admins, or the spare key: every pane is theirs. */
  admin: boolean;
  spareKey: boolean;
  /** The sections of backend/sections.json this person can use. */
  sections: string[];
  /** Whether this box signs people in itself. Only then is there a Users
   *  pane: a box whose sign-in runs elsewhere has nobody to add here.
   *  Missing on a box whose lososd predates the field. */
  standalone?: boolean;
  /** What LosOS Login's rules hold this person to their own account for
   *  (backend/src/login.rs). Empty or missing: nothing. */
  locked?: LoginReason[];
  /** The apps on the shared side live on this box now ("mail", "chat"):
   *  they run only while the box shares its storage, and their menu
   *  entries are drawn only when listed here. */
  shared?: string[];
  /** Their role (backend/src/roles.rs); missing on an older lososd. */
  role?: Role;
  /** The custom role they hold, by its name. */
  roleName?: string | null;
  /** Reports waiting for a moderator, for someone who can act on them. */
  reports?: number;
}

/** What a person is on this box, in rank order (backend/src/roles.rs). */
export type Role = "guest" | "member" | "moderator" | "admin" | "owner";

/** A role an administrator saved under a name of their own. */
export interface CustomRole {
  id: string;
  name: string;
  /** Section names, without the `losos-` prefix. */
  sections: string[];
}

/** A mute or a suspension (backend/src/moderation.rs, `Hold`). */
export interface Hold {
  at: number;
  by: string;
  /** When it ends by itself, in seconds since 1970; null until lifted. */
  until: number | null;
  reason: string;
}

/** Why a reported thing troubles someone. */
export type ReportReason = "spam" | "abuse" | "sexual" | "child" | "other";

/** The apps a report can come from. */
export type ReportApp =
  | "mail"
  | "chat"
  | "profile"
  | "cloud"
  | "git"
  | "notes"
  | "diagrams"
  | "office"
  | "calls";

export type ReportState = "open" | "hidden" | "dismissed" | "resolved";

/** One report. Never the thing itself: a reference inside its app. */
export interface Report {
  id: string;
  at: number;
  by: string;
  app: ReportApp;
  about: string | null;
  ref: string;
  reason: ReportReason;
  note: string;
  state: ReportState;
  handledBy: string | null;
  handledAt: number | null;
}

export interface ModerationPerson extends Profile {
  role: Role;
  roleName: string | null;
  muted: Hold | null;
  suspended: Hold | null;
  disabled: boolean;
  /** Whether the caller may mute or suspend them. */
  actionable: boolean;
}

export interface ModerationLogEntry {
  at: number;
  by: string;
  action: string;
  about: string | null;
  detail: string;
}

export interface ModerationView {
  standalone: boolean;
  you: string | null;
  admin: boolean;
  /** The apps a report can be hidden in from the queue. */
  hides: ReportApp[];
  reports: Report[];
  people: ModerationPerson[];
  log: ModerationLogEntry[];
}

export type ReportAction = "hide" | "release" | "dismiss" | "resolve";

/** Why a rule holds someone to their own account. */
export type LoginReason = "twoFactor" | "password";

/** The rules an administrator sets on the Users pane. */
export interface LoginRules {
  twoFactor: boolean;
  /** A password runs out this many days after it was set; 0 never. */
  passwordDays: number;
}

let me: Me | null = null;

/** Who is signed in, once GET /api/me has said. */
export function getMe(): Me | null {
  return me;
}

/** GET /api/me: who this tab is signed in as, by cookie or by key. Null when
 *  nobody is (a 401). Anything else throws. */
export async function loadMe(options: RequestOptions = {}): Promise<Me | null> {
  try {
    const answer = await call<Partial<Me>>("/api/me", options);
    // Only an answer that says who it is counts as a sign-in.
    me =
      typeof answer.admin === "boolean" && Array.isArray(answer.sections)
        ? (answer as Me)
        : null;
  } catch (error) {
    if (isUnauthorized(error)) {
      me = null;
      return null;
    }
    throw error;
  }
  notifyAuth();
  return me;
}

/** Whether `section` (backend/sections.json) is open to whoever is signed
 *  in. The spare key is an administrator's. Before GET /api/me has answered
 *  everything counts as open, so the page does not flicker shut. */
export function canUse(section: string): boolean {
  if (me === null) return true;
  return me.admin || me.sections.includes(section);
}

/** Whether a shared-side app (GET /api/me's `shared`) is live for whoever
 *  is signed in. False until /api/me has said so, and follows its latest
 *  response. */
export function sharedApp(name: string): boolean {
  return me?.shared?.includes(name) === true;
}

/** What holds the signed-in person to their own account, if anything. */
export function heldBy(): LoginReason[] {
  return me?.locked ?? [];
}

/** Whether this box signs people in itself (GET /api/me's `standalone`).
 *  False until /api/me has said so. */
export function runsOwnSignIn(): boolean {
  return me?.standalone === true;
}

/** For components: who is signed in, re-read when that changes. */
export function useMe(): Me | null {
  return React.useSyncExternalStore(subscribeAuth, getMe, getMe);
}

/** The https address of this page, which Authelia sends the browser back to.
 *  Authelia works over https only; the box serves both. The fragment goes
 *  too: it names the open document in Documents. */
function httpsHere(): string {
  const { host, pathname, search, hash } = window.location;
  return `https://${host}${pathname}${search}${hash}`;
}

/* The session cookie is https only, so a page on plain http cannot tell a
 * signed-in person from a stranger. Sending them straight to Authelia would
 * show someone who already has a session its second-factor page, so the
 * plain page goes to its own https address first, marked `?signin`; there
 * the dialog goes on to Authelia by itself only if nobody is signed in. */
const SIGN_IN_MARK = "signin";

function takeSignInMark(): boolean {
  if (typeof window === "undefined") return false;
  const url = new URL(window.location.href);
  if (!url.searchParams.has(SIGN_IN_MARK)) return false;
  url.searchParams.delete(SIGN_IN_MARK);
  window.history.replaceState(window.history.state, "", url);
  return true;
}

const cameToSignIn = takeSignInMark();

/** Whether this page was opened from the plain http address to sign in. */
export function sentToSignIn(): boolean {
  return cameToSignIn;
}

/** Authelia's sign-in page, coming back here afterwards; from plain http,
 *  this page's https address first. */
export function signInUrl(): string {
  if (window.location.protocol === "http:") {
    const url = new URL(httpsHere());
    url.searchParams.set(SIGN_IN_MARK, "");
    return url.toString();
  }
  return `https://${window.location.host}/auth/?rd=${encodeURIComponent(httpsHere())}`;
}

/** Whether the box's sign-in page answers. Authelia starts after the box
 *  does, and until it answers nginx says 502; the sign-in dialog waits with
 *  a spinner rather than send anyone to an error page. */
export async function signInPageReady(signal?: AbortSignal): Promise<boolean> {
  try {
    const init: RequestInit = { cache: "no-store" };
    if (signal !== undefined) init.signal = signal;
    const response = await fetch("/auth/api/health", init);
    return response.ok;
  } catch {
    return false;
  }
}

/** Sign out: forget the key, and end the Authelia session if there is one. */
export function signOut(): void {
  const session = me !== null && !me.spareKey;
  dropToken();
  if (session) signOutTo("/");
}

/** End the LosOS Login session and come back to `path` on this box, which
 *  then asks for a sign-in again. Every app's Sign out button goes here, so
 *  signing out of one app signs the person out of all of them. */
export function signOutTo(path: string): void {
  const back = `https://${window.location.host}${path}`;
  window.location.assign(`https://${window.location.host}/auth/logout?rd=${encodeURIComponent(back)}`);
}

/** What LosOS Login said to a name and a password. */
export type PasswordSignIn = "signedIn" | "refused" | "notYet";

/* POST /auth/api/firstfactor, the call LosOS Login's own sign-in page makes,
 * from this page: same origin, so the session cookie it sets is this tab's
 * from the next request on. The wizard's last step signs the owner in this
 * way with the password they have just chosen, so the first sign-in needs
 * no second tab. "notYet" is LosOS Login still starting (nginx says 502
 * until it answers) or the request not getting through at all. */
export async function signInWithPassword(
  username: string,
  password: string,
  signal?: AbortSignal,
): Promise<PasswordSignIn> {
  try {
    const init: RequestInit = {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ username, password, keepMeLoggedIn: false }),
      cache: "no-store",
      credentials: "same-origin",
    };
    if (signal !== undefined) init.signal = signal;
    const response = await fetch("/auth/api/firstfactor", init);
    if (response.ok) return "signedIn";
    if (response.status === 401 || response.status === 403) return "refused";
    return "notYet";
  } catch (error) {
    if (isAbort(error)) throw error;
    return "notYet";
  }
}

/* Swap the spare key this tab holds for the LosOS Login session its cookie
 * now carries: GET /api/me without the key, and only if that names a person
 * does the key leave sessionStorage. One notification for both halves, so
 * nothing on the page sees a moment with neither. False, and the key kept,
 * when the cookie names nobody. */
export async function adoptSession(signal?: AbortSignal): Promise<boolean> {
  let answer: Partial<Me>;
  try {
    answer = await call<Partial<Me>>("/api/me", signal === undefined ? { anonymous: true } : { anonymous: true, signal });
  } catch (error) {
    if (isAbort(error)) throw error;
    return false;
  }
  if (typeof answer.name !== "string" || answer.spareKey === true || !Array.isArray(answer.sections)) return false;
  try {
    window.sessionStorage.removeItem(TOKEN_KEY);
  } catch {
    /* nothing stored */
  }
  me = answer as Me;
  notifyAuth();
  return true;
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

/** For useSyncExternalStore: whether this tab is signed in, by the spare
 *  key it holds or by an Authelia session GET /api/me confirmed. */
export function hasToken(): boolean {
  return getToken() !== null || me !== null;
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
  /** The ETag the change was made against (LosOS Calendar's events). */
  ifMatch?: string;
}

/* The edge's manage page (src/manage/) shows one box's panes from the
 * company edge: every /api/ path then goes to the edge, which relays it into
 * that box (`/manage/api/boxes/<id>/api/…`), and the stored token is the
 * edge's manage key rather than a box's. Null is the box's own page. */
let routeBase: string | null = null;

/** Send every /api/ call through `base` (the manage page's box), or stop. */
export function routeThrough(base: string | null): void {
  routeBase = base;
  me = null;
}

/** Where a call to `path` goes from this page: the box's own route, or the
 *  edge's relay into the box the manage page shows. */
export function apiPath(path: string): string {
  return routeBase !== null && path.startsWith("/api/") ? routeBase + path : path;
}

async function call<T>(path: string, options: CallOptions = {}): Promise<T> {
  const { method = "GET", body, contentType, signal, token, anonymous, ifMatch } = options;
  path = apiPath(path);

  const headers: Record<string, string> = {};
  if (contentType !== undefined) headers["Content-Type"] = contentType;
  if (ifMatch !== undefined) headers["If-Match"] = ifMatch;
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
    const { message, ...rest } = await errorBody(response);
    throw new ApiError(response.status, message, rest);
  }

  return (await response.json()) as T;
}

interface ErrorBody {
  message: string;
  edgeRequired: boolean;
  officialEdgeRequired: boolean;
  code: string | null;
}

async function errorBody(response: Response): Promise<ErrorBody> {
  try {
    const body = (await response.json()) as {
      error?: unknown;
      edgeRequired?: unknown;
      officialEdgeRequired?: unknown;
      code?: unknown;
    };
    const flags = {
      edgeRequired: body.edgeRequired === true,
      officialEdgeRequired: body.officialEdgeRequired === true,
      code: typeof body.code === "string" ? body.code : null,
    };
    if (typeof body.error === "string" && body.error.length > 0) {
      return { message: body.error, ...flags };
    }
    return { message: `HTTP ${response.status}`, ...flags };
  } catch {
    /* lososd restarting mid-rebuild answers through nginx, not as JSON */
  }
  return { message: `HTTP ${response.status}`, edgeRequired: false, officialEdgeRequired: false, code: null };
}

// ── Routes ────────────────────────────────────────────────────────────────

/** GET /api/health — public. Whether lososd is answering at all. */
export function getHealth(options: RequestOptions = {}): Promise<HealthResponse> {
  return call<HealthResponse>("/api/health", { ...options, anonymous: true });
}

export interface ClaimState {
  /** Whether an owner has ever set a password on this box. */
  claimed: boolean;
  /** Whether the first password can be set *now*. lososd answers true
   *  whenever the box is unclaimed; the wizard still polls on false. Absent from a lososd older than this field, which
   *  the wizard reads as ready, so an old box is not waited on forever. */
  ready?: boolean;
  /** Why not yet, in a sentence for the owner, when `ready` is false. */
  waitingFor?: string | null;
  /** Whether anyone can sign in as an administrator yet. False on a box set
   *  up before sign-in went through Authelia; its owner makes the first
   *  account with the spare key (`makeFirstAccount`). */
  people?: boolean;
}

export interface ClaimResponse {
  claimed: true;
  user: string | null;
  /** The first person's sign-in name, an administrator. */
  name?: string;
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
 * Takes ownership of a box nobody has set up: sets the first password, makes
 * `name` the first person (an administrator, who signs in at /auth/ with that
 * password) and returns the admin token so this tab can carry on
 * authenticated. The window
 * is open only while the box is unclaimed; afterwards lososd refuses, so this
 * cannot be used to re-read the token later.
 *
 * The reason it exists rather than a key prompt: `losos.admin.tokenFile` is 64
 * random hex characters written 0600 by lososd on first start, on an appliance
 * with no SSH and no shell logins. Nothing prints it anywhere. Asking a new
 * owner to paste it was asking for something they had no way to obtain.
 *
 * Answers 503 with `{error, ready: false, waitingFor}` while the box cannot
 * take the password yet; the box stays
 * claimable and the wizard goes back to waiting. See `isNotReady`. */
export function claimBox(
  password: string,
  name: string,
  options: RequestOptions = {},
): Promise<ClaimResponse> {
  return call<ClaimResponse>("/api/setup/claim", {
    ...options,
    method: "POST",
    contentType: "application/json",
    body: JSON.stringify({ name, password }),
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

/** GET /api/statistics — what Traefik counted for the apps' LAN ports
 *  (lososd's statistics plugin). Read with readStatistics (src/statistics). */
export function getStatistics(options: RequestOptions = {}): Promise<unknown> {
  return call<unknown>("/api/statistics", options);
}

export function getStorage(options: RequestOptions = {}): Promise<StorageResponse> {
  return call<StorageResponse>("/api/storage", options);
}

/* ── Edge proxies ─────────────────────────────────────────────────────────
 * What lososd found when it last looked for an edge proxy: on the LAN by
 * DNS-SD (`_losos-edge._tcp`, browsed through the systemd-resolved the box
 * already runs for mDNS) and at the configured registrar URL. Both are
 * probed; a candidate is listed only once its /health answered. The daemon
 * refreshes this every few seconds, and the same answer gates sharing:
 * POST /api/change to mesh, or an apply that turns
 * `sharingMyStorage` or `clusterEnable` on, is a 409 `{edgeRequired: true}`
 * while `reachable` is false. See backend/src/edge.rs. */

export type EdgeSource = "lan" | "configured";

/** What the box's gateway checks concluded about one edge
 *  (backend/src/edge.rs, "gateways"). */
export type EdgeGateway =
  | { state: "trusted"; owner: string; fingerprint: string; image: string }
  | { state: "refused"; code: string; reason: string }
  | { state: "disabled"; code: string; reason: string; since: number };
/** Which kind of link (handbook in-depth/edge-federation, "Three tiers").
 *  primary: edge to edge, the main cluster. secondary: a box to a local
 *  gateway of its secondary cluster. tertiary: a box straight to an official
 *  edge, only while no gateway of its own cluster answers. A box's own link
 *  is never primary. */
export type EdgeTier = "primary" | "secondary" | "tertiary";

export interface EdgeProxy {
  /** The advertised instance name, or the configured URL's host. */
  name: string;
  /** Passed every gateway check on this scan: its TPM showed a published
   *  LosOS image, its owner's signed passport and LosOS's fee addresses.
   *  Only such an edge may process trading; any edge shares storage. */
  official: boolean;
  /** The checks' outcome, when the edge showed a passport. */
  gateway?: EdgeGateway;
  /** Base URL of its registrar API. */
  url: string;
  source: EdgeSource;
  /** Found on the LAN with its registrar and tunnel on this box's own
   *  networks: the only kind of edge that may manage the box. */
  onNetwork?: boolean;
  /** Secondary for a local gateway, tertiary for an official edge. Missing
   *  on a box whose lososd predates the tiers. */
  tier?: EdgeTier;
  /** The secondary cluster a gateway belongs to. */
  cluster?: string;
  /** The gateway's name in it. */
  member?: string;
  /** The official cluster (edges behind one load balancer) an official
   *  edge sits in. */
  officialCluster?: string;
}

/** The edge this box uses (backend/src/edge.rs, choose_path). */
export interface EdgePath {
  name: string;
  url: string;
  source: EdgeSource;
  tunnel: string;
  tier?: EdgeTier;
  cluster?: string;
  member?: string;
  officialCluster?: string;
}

/** A secondary cluster in reach and its gateways that answered. */
export interface EdgeCluster {
  name: string;
  gateways: string[];
}

export interface EdgeResponse {
  /** At least one edge answered. The one bit the gate reads. */
  reachable: boolean;
  /** At least one edge is official: the bit the market gate reads. */
  official: boolean;
  /** Every edge that answered, LAN first. */
  edges: EdgeProxy[];
  /** False when the LAN could not be searched at all (systemd-resolved down,
   *  or it refused the browse). */
  lanSearched: boolean;
  /** The configured registrar URL that was tried, if any. */
  configuredUrl: string | null;
  /** Unix seconds of the scan; null before the first. */
  checkedAt: number | null;
  /** The edge this box uses, or null when none answers. */
  path?: EdgePath | null;
  /** The secondary clusters in reach. */
  clusters?: EdgeCluster[];
  /** The secondary cluster this box belongs to; kept while none of its
   *  gateways answers. */
  cluster?: string | null;
  /** The edge a live custom domain holds this box to, or null. */
  held?: string | null;
  /** The banned edge this box moved off on the last choice, or null. */
  leftBanned?: string | null;
  /** Whether a company edge may manage this box. Missing on a box whose
   *  lososd predates the switch. */
  manage?: EdgeManage;
  /** Unix seconds when the box turned sharing off by itself because no edge
   *  answered; it turns it back on a minute after one does. Null when not
   *  paused, missing from an older lososd. */
  sharingPausedAt?: number | null;
  /** Whether this box ends TLS for its public names itself, so the edge only
   *  passes encrypted bytes through (modules/public-tls.nix). Missing from an
   *  older lososd. */
  publicTls?: PublicTls;
}

/** What a look at one public name from outside found: this box's own
 *  certificate, nothing, or a certificate this box did not make. */
export type TlsSeen = "ok" | "unreachable" | "foreign";

export interface PublicTls {
  /** The names this box holds its own certificate for; null while the edge
   *  still ends TLS for every name. */
  own: string[] | null;
  /** RFC 3339 time of the last look from outside, or null. */
  checkedAt: string | null;
  seen: Record<string, TlsSeen>;
}

/** The switch that lets a company edge on this LAN manage LosOS Cloud,
 *  LosOS Git and the apps (backend/src/edge.rs, "management"). */
export interface EdgeManage {
  /** The edge in use that could manage the box, or null. */
  candidate: { name: string; url: string } | null;
  /** Why none can: the box uses no edge on this network. */
  refused: "noLocalEdge" | "outsideNetwork" | null;
  /** The registrar URL the owner switched management on for, or null. */
  managedBy: string | null;
  /** That edge is the one in use, so it manages the box now. */
  active: boolean;
}

/** GET /api/edge — the last edge scan. */
export function getEdge(options: RequestOptions = {}): Promise<EdgeResponse> {
  return call<EdgeResponse>("/api/edge", options);
}

/** POST /api/edge/manage — let the company edge in use manage the box, or
 *  stop it. Takes effect at once; it is not part of Apply. */
export function postEdgeManage(
  enabled: boolean,
  options: RequestOptions = {},
): Promise<{ manage: EdgeManage }> {
  return call<{ manage: EdgeManage }>("/api/edge/manage", {
    ...options,
    method: "POST",
    contentType: "application/json",
    body: JSON.stringify({ enabled }),
  });
}

/** POST /api/edge/cluster — move the box to another secondary cluster in
 *  reach, or with null let it settle anew. Takes effect at once. */
export function postEdgeCluster(
  cluster: string | null,
  options: RequestOptions = {},
): Promise<{ cluster: string | null; path: EdgePath | null }> {
  return call<{ cluster: string | null; path: EdgePath | null }>("/api/edge/cluster", {
    ...options,
    method: "POST",
    contentType: "application/json",
    body: JSON.stringify({ cluster }),
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

/** `vm` is a virtual machine replica, sold on the Machines pane. */
export type MarketKind = "storage" | "compute" | "vm";

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
  /** Unix seconds. Someone paid for a slot on this listing that runs until
   *  then, and it cannot be closed before. */
  locked_until?: number | null;
}

/** Where a held payment stands: kept for the slot, on its way, or done.
 *  `split` and `settled` send the buyer part back and the seller the rest. */
export type HoldStep = "held" | "release" | "released" | "refund" | "refunded" | "split" | "settled";

export type RefundReason =
  | "sellerCannotReceive"
  | "alreadyPaid"
  | "lapsed"
  | "buyerReclaimed"
  | "notDelivered"
  | "sellerOffline";

export interface MarketOrder {
  id: string;
  kind: MarketKind;
  unit: string;
  quantity: number;
  amount: number;
  fee: number;
  /** On a USDC sale, the part of the fee that paid platform fees owed on
   *  earlier coin sales. `seller_net` already leaves it out. */
  fee_offset?: number;
  seller_net: number;
  currency: string;
  status: "pending" | "paid" | "expired";
  created_at: number;
  paid_at: number | null;
  expires_at: number | null;
  expired: boolean;
  /** `<namespace>/<claim>` once a storage order has its volume. */
  volume: string | null;
  /** A machine order's image, name and address. Absent on other kinds. */
  vm?: VmOrderInfo | null;
  /** The transaction that paid a USDC order. */
  tx?: string | null;
  /** The network it was paid on; absent on an order from before the edge
   *  named networks, which was paid on its first EVM network. */
  network?: string | null;
  /** The coin payment, when the buyer chose to pay in a coin. */
  coin?: CoinPayment | null;
  /** Where an EVM payment held in the escrow is going. */
  escrow?: MarketEscrow | null;
  /** Where a card payment the platform holds is going. */
  card?: MarketCardHold | null;
  /** Unix seconds. While the payment is held: when the edge pays the
   *  seller by itself, if the seller has not asked sooner. */
  payout_at?: number | null;
}

export interface MarketEscrow {
  network: string;
  deposit: number;
  step: HoldStep;
  reason?: RefundReason | null;
  /** On a split, what goes back to the buyer, minor units. */
  refund?: number | null;
}

export interface MarketCardHold {
  step: HoldStep;
  reason?: RefundReason | null;
  /** On a split, what goes back to the buyer, minor units. */
  refund?: number | null;
}

/** A seller's held money, minor units. */
export interface MarketPayouts {
  /** Held for slots still running. */
  held: number;
  /** From slots that are over: Pay out sends it now. */
  ready: number;
  /** On its way to the seller. */
  paying: number;
  /** Unix seconds: when the next running slot ends. */
  next_at?: number | null;
  /** The edge pays out by itself this long after a slot ends. */
  auto_after_secs: number;
}

/** Coins other than USDC an order can be paid in. */
export type Coin = "btc" | "xmr" | "zec";

/** POST /api/market/coins/pay — where to pay an order in a coin, and what
 *  reached that address so far. Amounts are whole coins as decimal text. */
export interface CoinPayment {
  coin: Coin;
  address: string;
  amount: string;
  /** The payment link with the exact amount (BIP 21 for Bitcoin). */
  uri: string;
  /** Unix seconds. The quote holds until then. */
  pay_before: number;
  state: "waiting" | "seen" | "underpaid" | "confirming" | "paid" | "late" | "repricing";
  paid: string;
  /** What is owed when short: the quote, or the price now when late. */
  due?: string;
  /** Paid over the quote, for a person to refund. */
  extra?: string;
  confirmations?: number;
  required: number;
  txs: string[];
}

export interface MarketAccount {
  fee_bps: number;
  currency: string;
  seller_onboarded: boolean;
  seller_ready: boolean;
  can_sell_storage: boolean;
  can_sell_compute: boolean;
  /** Enrolled and hosting machines: what lets this box list them. */
  can_host_vms?: boolean;
  /** The platform's share of a machine sale, when the edge runs machines. */
  vm_fee_bps?: number | null;
  /** This box's own machine images on the edge. */
  uploads?: VmUpload[];
  listings: MarketOwnListing[];
  entitlements: { storage_gib: number; compute_vcpu_hours: number; next_expiry: number | null };
  purchases: MarketOrder[];
  sales: MarketOrder[];
  /** What this box's sales hold for it. */
  payouts?: MarketPayouts;
  /** The edge also takes payments it cannot hold for the slot (coins, USDC
   *  on Solana). Off, it offers only cards and USDC on EVM networks. */
  direct_pay?: boolean;
  /** A seller's box offline this long in a row during a slot refunds its
   *  buyer the share not delivered. */
  offline_refund_secs?: number;
  /** The seller's USDC address on EVM chains, once they have signed it.
   *  Absent on Stripe. */
  payout?: string | null;
  /** The seller's USDC owner on Solana, once they have signed it. */
  payout_solana?: string | null;
  /** Present when this edge settles the market in USDC. No RPC URL. */
  usdc?: UsdcPublic | null;
  /** Coins other than USDC this edge takes for orders. */
  coins?: Coin[];
  /** This box's watch-only keys for them, as the edge's node read them. */
  coin_keys?: Partial<Record<Coin, string>>;
  /** Platform fees owed on this box's coin sales, in the currency's minor
   *  unit: the coins went straight to this box's wallet. */
  coin_fees_owed?: number;
  /** Unix seconds: when this box began to owe them. */
  coin_fees_since?: number | null;
  /** A USDC sale that took more than was still owed leaves the rest here;
   *  the next coin sale's fee comes out of it first. */
  coin_fee_credit?: number;
  /** Above this many minor units owed, coin payments stop. Present when the
   *  edge takes coins. */
  coin_fee_limit?: number | null;
  /** Owed for this long, any amount stops them too. */
  coin_fee_grace_secs?: number | null;
  /** Buyers cannot pay this box in a coin until the fees are settled. USDC
   *  still works, and each USDC sale pays toward them. */
  coin_sales_paused?: boolean;
}

/** POST /api/market/fees/pay: what the transfer counted for. Minor units. */
export interface FeePayment {
  paid: number;
  credited: number;
  owed: number;
}

export type ChainFamily = "evm" | "solana";

/** One network the edge settles USDC on. */
export interface UsdcChain {
  /** `base-sepolia`, `arbitrum-sepolia`, `solana-devnet`, ... */
  network: string;
  name: string;
  family: ChainFamily;
  /** EVM chain id; null on Solana. */
  chain_id: number | null;
  /** Circle's USDC: an 0x address, or the SPL mint on Solana. */
  usdc: string;
  /** The escrow payments wait in: LososEscrow on an EVM chain, the escrow
   *  program on Solana. Null on Solana when the edge takes direct transfers. */
  pay: string | null;
  treasury: string;
}

/** What the browser may know about USDC settlement. No RPC URL. */
export interface UsdcPublic {
  test: boolean;
  /** The chain id an EVM payout signature names; null with no EVM chain. */
  chain_id: number | null;
  /** The network a Solana payout signature names; null with no Solana. */
  solana: string | null;
  chains: UsdcChain[];
  /** Where ENS names can be looked up through the relay, when the edge can. */
  ens_chain_id: number | null;
  /** Dollar prices of the fee coins, micro-dollars per whole coin as text,
   *  when the edge has a fresh one. Ranks networks by what their fee costs. */
  prices?: { eth?: string; sol?: string };
  /** Who sells USDC on Base for euros through this edge, when anyone does. */
  onramp?: "binance";
}

/** What the buyer's wallet pays. Amounts are USDC base units, as decimal
 *  strings. `chains` are the networks it may be paid on, each with the
 *  seller's address there. */
export interface StablecoinInvoice {
  order_id: string;
  order_id_hash: string;
  gross: string;
  /** The whole fee the escrow or the split sends the treasury, `fee_offset`
   *  included. */
  fee: string;
  /** The part of `fee` that pays the seller's platform fees owed on coin
   *  sales. The buyer pays `gross` either way. */
  fee_offset?: string;
  /** Unix seconds. The order holds its units until then; start no payment after it. */
  pay_before: number;
  /** Unix seconds, the `deadline` of the escrow's `pay` on either family:
   *  after it the buyer can take a deposit the edge never released back. A
   *  direct transfer on Solana ignores it. */
  deadline: number;
  chains: (UsdcChain & { seller: string })[];
  /** Coins other than USDC the seller takes for this order. */
  coins?: Coin[];
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
  /** Set when the order is paid in USDC. There is no hosted page. */
  stablecoin?: StablecoinInvoice;
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

/** POST /api/market/release — pay this box now for the sales whose slots
 *  are over. 409 while none are. The reply carries the new payouts. */
export function postMarketRelease(
  options: RequestOptions = {},
): Promise<MarketActionResponse> {
  return marketPost("/api/market/release", {}, options);
}

/** POST /api/market/orders — buy. Stripe replies with a Checkout URL; USDC
 *  replies with `stablecoin` and no page. */
export function postMarketOrder(
  listingId: string,
  quantity: number,
  options: RequestOptions = {},
): Promise<MarketActionResponse> {
  return marketPost("/api/market/orders", { listing_id: listingId, quantity }, options);
}

/** POST /api/market/payout — the wallet proved it controls `address` on
 *  one family of networks. */
export function postMarketPayout(
  address: string,
  signature: string,
  family: ChainFamily = "evm",
  options: RequestOptions = {},
): Promise<MarketActionResponse> {
  return marketPost("/api/market/payout", { address, signature, family }, options);
}

/** POST /api/market/confirm — a mined USDC transaction paid this order. */
export function postMarketConfirm(
  orderId: string,
  tx: string,
  network: string,
  options: RequestOptions = {},
): Promise<MarketActionResponse> {
  return marketPost("/api/market/confirm", { order_id: orderId, tx, network }, options);
}

/** POST /api/market/fees/pay: a mined USDC transfer from this box's payout
 *  address to the edge's treasury paid platform fees owed on coin sales. */
export function postMarketFeesPay(
  tx: string,
  network: string,
  options: RequestOptions = {},
): Promise<MarketActionResponse & FeePayment> {
  return call<MarketActionResponse & FeePayment>("/api/market/fees/pay", {
    ...options,
    method: "POST",
    contentType: "application/json",
    body: JSON.stringify({ tx, network }),
  });
}

/** POST /api/market/coins/key — the watch-only key this box is paid to in
 *  `coin`: for Bitcoin an extended public key (xpub, zpub) or a ranged
 *  descriptor, for Monero the primary address with its private view key,
 *  for Zcash the unified viewing key (uview… or uivk…) alone. */
export function postMarketCoinKey(
  coin: Coin,
  key: string,
  viewKey?: string,
  options: RequestOptions = {},
): Promise<MarketActionResponse> {
  const body = viewKey === undefined ? { coin, key } : { coin, key, view_key: viewKey };
  return marketPost("/api/market/coins/key", body, options);
}

/** POST /api/market/coins/pay — an address and amount to pay this order in
 *  `coin`, or, asked again, what has reached it. */
export function postMarketCoinPay(
  orderId: string,
  coin: Coin,
  options: RequestOptions = {},
): Promise<CoinPayment> {
  return call<CoinPayment>("/api/market/coins/pay", {
    ...options,
    method: "POST",
    contentType: "application/json",
    body: JSON.stringify({ order_id: orderId, coin }),
  });
}

/** Where an order is, in the edge's words. */
export type OnrampStage = "open" | "paying" | "sending" | "done" | "kept" | "failed";

export interface OnrampOrder {
  order_id: string;
  /** Binance's checkout, on binance.com. */
  link: string;
  /** Unix milliseconds; 0 when Binance gave none. */
  link_expires: number;
  euros: string;
  address: string;
  network: "base";
  stage: OnrampStage;
  /** Binance's own status number. */
  status: number;
  /** USDC bought, as Binance writes it. */
  usdc: string | null;
  /** The withdrawal's transaction on Base. */
  tx: string | null;
}

/** POST /api/market/onramp — a Binance order for `euros` of USDC on Base
 *  to `address`, this box's wallet, with Binance's checkout link. */
export function postMarketOnramp(
  address: string,
  euros: string,
  options: RequestOptions = {},
): Promise<OnrampOrder> {
  return call<OnrampOrder>("/api/market/onramp", {
    ...options,
    method: "POST",
    contentType: "application/json",
    body: JSON.stringify({ address, euros }),
  });
}

/** POST /api/market/onramp/status — where one of this box's Binance orders is. */
export function postMarketOnrampStatus(orderId: string, options: RequestOptions = {}): Promise<OnrampOrder> {
  return call<OnrampOrder>("/api/market/onramp/status", {
    ...options,
    method: "POST",
    contentType: "application/json",
    body: JSON.stringify({ order_id: orderId }),
  });
}

export interface RpcReply {
  result?: unknown;
  error?: { code?: number; message?: string; data?: unknown };
}

/** POST /api/market/rpc — one allowlisted chain call to `network`, or to
 *  the name endpoint for `ens`. The box forwards it; the page never sees the
 *  RPC URL. */
export function postMarketRpc(
  network: string,
  method: string,
  params: unknown[],
  options: RequestOptions = {},
): Promise<RpcReply> {
  return call<RpcReply>("/api/market/rpc", {
    ...options,
    method: "POST",
    contentType: "application/json",
    body: JSON.stringify({ network, method, params }),
  });
}

// ── AI models on lent memory ──────────────────────────────────────────────

/* Large models run split across the memory boxes lend, paid per token
 * through the edge (edge/registrar/src/server/llm.rs, relayed by lososd's
 * market relay). Amounts are USDC base units (millionths of a dollar) as
 * decimal text, except `billedE6`, which has six more decimals, and a
 * wallet's balance, asked and notBefore, which are numbers. */

export type ModelGroupState = "loading" | "ready" | "down";
export type MeterState = "quoted" | "open" | "ending" | "ended" | "lapsed";

export interface LlmModel {
  id: string;
  name: string;
  /** Memory the boxes running it must lend together, MiB. */
  needMib: number;
  /** Context length, tokens. */
  context: number;
  /** Base units per million prompt tokens. */
  priceIn: string;
  /** Base units per million answer tokens. */
  priceOut: string;
  licence: string;
  /** Memory the boxes lending now offer together, MiB. */
  lentMib: number;
  group?: ModelGroupState;
  boxes?: number;
}

export interface LlmMeter {
  id: string;
  model: string;
  network: string;
  state: MeterState;
  /** Why it ended, in the edge's words. */
  reason?: string;
  wallet?: string;
  spendPerHour: string;
  boxes: number;
  group?: ModelGroupState;
  requests: number;
  tokensIn: number;
  tokensOut: number;
  billedE6: string;
  paid: string;
  createdAt: number;
}

export interface LlmWallet {
  network: string;
  wallet: string;
  balance: number;
  asked: number;
  /** Unix seconds the ask can be taken from; 0 without one. */
  notBefore: number;
  /** The balance less what is used and not yet settled. */
  available: string;
  runwaySecs?: number;
  /** The runway is under a day. */
  low: boolean;
}

export interface LlmLending {
  paid: string;
  owed: string;
  model?: string;
  group?: ModelGroupState;
  lentMib?: number;
}

export type LlmResponse =
  | { available: false; reason?: "noOfficialEdge" }
  | {
      available: true;
      network: string;
      feeBps: number;
      models: LlmModel[];
      meters: LlmMeter[];
      wallets: LlmWallet[];
      lending: LlmLending;
      noticeSecs: number;
      minSpendPerHour: string;
      maxSpendPerHour: string;
    };

export interface TapQuote {
  meter: string;
  model: string;
  network: string;
  chain: { network: string; name: string; family: ChainFamily; chain_id: number | null; usdc: string; pay: string | null; treasury: string };
  feeBps: number;
  spendPerHour: string;
  /** One per lending box: `key` is the 32-byte id the wallet opens it
   *  under, `seller` the box's payout address, `rate` base units an hour. */
  taps: { id: string; key: string; seller: string; rate: string }[];
  /** Unix seconds: the taps must be open by then. */
  openBefore: number;
}

export interface ChatMessage {
  role: "system" | "user" | "assistant";
  content: string;
}

export interface ChatAnswer {
  content: string;
  finishReason: string;
  tokensIn: number;
  tokensOut: number;
  /** What this answer cost, millionths of a base unit. */
  costE6: string;
  /** The meter's total so far, millionths of a base unit. */
  billedE6: string;
}

/** POST /api/market/llm/models — the models, this box's meters, balances
 *  and lending, or `{ available: false }`. */
export function postLlmModels(network: string | null, options: RequestOptions = {}): Promise<LlmResponse> {
  return postJson<LlmResponse>("/api/market/llm/models", network === null ? {} : { network }, options);
}

/** POST /api/market/taps — quote a meter for `model`. */
export function postTapQuote(
  model: string,
  network: string,
  spendPerHour: string,
  options: RequestOptions = {},
): Promise<TapQuote> {
  return postJson<TapQuote>("/api/market/taps", { model, network, spendPerHour }, options);
}

/** POST /api/market/taps/confirm — `wallet` opened the meter's taps. */
export function postTapConfirm(meter: string, wallet: string, options: RequestOptions = {}): Promise<{ meter: string; state: MeterState }> {
  return postJson("/api/market/taps/confirm", { meter, wallet }, options);
}

/** POST /api/market/taps/stop — stop a meter; the edge closes its taps.
 *  `wallet` began opening a quote's taps and stopped partway: the edge
 *  closes the ones it opened and keeps its balance in view. */
export function postTapStop(
  meter: string,
  wallet?: string,
  options: RequestOptions = {},
): Promise<{ meter: string; state: MeterState }> {
  return postJson("/api/market/taps/stop", wallet === undefined ? { meter } : { meter, wallet }, options);
}

/** POST /api/market/llm/chat — one request to a running model. Minutes,
 *  not seconds, on a large model. */
export function postLlmChat(
  meter: string,
  messages: ChatMessage[],
  maxTokens: number,
  options: RequestOptions = {},
): Promise<ChatAnswer> {
  return postJson<ChatAnswer>("/api/market/llm/chat", { meter, messages, maxTokens }, options);
}

// ── The builder ───────────────────────────────────────────────────────────

/* A language model on this box writes a widget or an app from the owner's
 * description (backend/src/builder.rs, Ollama on loopback). Nothing leaves
 * the box and a build costs nothing. A build takes minutes on the box's
 * processor, so it runs in the background and the page polls it. */

export type BuildStatus = "running" | "done" | "failed" | "stopped";
export type BuildFault = "noWidget" | "noApp" | "unusable" | "upstream" | "memory";
export type BuildKind = "widget" | "app";

export interface BuildSummary {
  id: string;
  kind: BuildKind;
  status: BuildStatus;
  prompt: string;
  revision: boolean;
  created_at: number;
  finished_at: number | null;
  /** Lines of the answer written so far. */
  lines: number;
  /** The box found a mistake and the model is writing the code again. */
  fixing: boolean;
  /** How many steps the build has taken. */
  steps: number;
  fault: BuildFault | null;
  has_source: boolean;
}

/** One step of a build: the model writes or fixes, the box checks the
 *  code, the page tries the app. */
export interface BuildStep {
  kind: "write" | "check" | "try" | "fix";
  running: boolean;
  /** What it found wrong, as sentences. */
  problems: string[];
  /** For a try: what the app showed, a line per element. */
  shown: string[];
  /** For a try: no page was open to try the app in. */
  skipped: boolean;
  started_at: number;
}

export interface BuildView extends Omit<BuildSummary, "has_source" | "steps"> {
  steps: BuildStep[];
  /** An app the build asks the page to try in its preview now. */
  trial: { id: string; code: string } | null;
  /** The answer ran into its length limit; the widget may be unfinished. */
  at_limit: boolean;
  files: WidgetFile[] | null;
  /** The name the model gave an app. */
  name: string | null;
  /** An app compiled for the app frame, when it compiles. */
  code: string | null;
  /** What the box's checks still find wrong, as sentences. */
  problems: string[];
  /** What the model says it made, in the page's language. */
  notes: string | null;
}

/** Why the builder cannot take a build: the box runs none, the model is not
 *  answering yet, or the box is still downloading it. */
export type BuilderReason = "off" | "starting" | "downloading";

export type BuilderResponse =
  | { available: false; reason: BuilderReason; model?: string }
  | { available: true; model: string; builds: BuildSummary[] };

/** GET /api/builder. */
export function getBuilder(options: RequestOptions = {}): Promise<BuilderResponse> {
  return call<BuilderResponse>("/api/builder", options);
}

/** POST /api/builder/builds — start a build; `base` is the widget to
 *  change, or the source of the app to change. */
export function postBuilderBuild(
  request: { prompt: string; kind?: BuildKind; base?: WidgetFile[] | string; lang: string },
  options: RequestOptions = {},
): Promise<BuildView> {
  return call<BuildView>("/api/builder/builds", {
    ...options,
    method: "POST",
    contentType: "application/json",
    body: JSON.stringify(request),
  });
}

/** GET /api/builder/builds/{id}. */
export function getBuilderBuild(id: string, options: RequestOptions = {}): Promise<BuildView> {
  return call<BuildView>(`/api/builder/builds/${encodeURIComponent(id)}`, options);
}

/** POST /api/builder/builds/{id}/tried — what the preview found when it
 *  tried the app the build asked it to. */
export function postBuilderTried(
  id: string,
  report: {
    trial: string;
    shown: string[];
    pressed: string;
    kept: string[];
    chose: string[];
    keptAfterChoosing: string[];
    errors: string[];
    threwOnPress: string[];
  },
  options: RequestOptions = {},
): Promise<{ taken: boolean }> {
  return call<{ taken: boolean }>(`/api/builder/builds/${encodeURIComponent(id)}/tried`, {
    ...options,
    method: "POST",
    contentType: "application/json",
    body: JSON.stringify(report),
  });
}

/** DELETE /api/builder/builds/{id} — stop a running build. */
export function stopBuilderBuild(id: string, options: RequestOptions = {}): Promise<BuildView> {
  return call<BuildView>(`/api/builder/builds/${encodeURIComponent(id)}`, { ...options, method: "DELETE" });
}

// ── The owner's own apps ──────────────────────────────────────────────────

/* Apps kept on the box (backend/src/user_apps.rs): a React component in JSX
 * the box compiles, and the data it keeps. Making and changing one is the
 * apps section's; opening one is anyone's who is signed in. */

export interface UserAppSummary {
  id: string;
  name: string;
  summary: string | null;
  createdAt: number;
  updatedAt: number;
}

/** An app's repository on LosOS Git (backend/src/app_repo.rs). */
export interface UserAppRepo {
  /** `owner/name`, once the box has made it. */
  name?: string;
  /** `synced`, `local`, `diverged` or `refused`. */
  state: "synced" | "local" | "diverged" | "refused";
  detail?: string;
}

export interface UserApp extends UserAppSummary {
  prompt: string | null;
  /** The JSX the owner keeps. */
  source: string;
  /** The script the app frame runs. */
  code: string;
  /** Absent until the box's first sync after the app was saved. */
  repo?: UserAppRepo;
}

export interface UserAppsResponse {
  apps: UserAppSummary[];
  limits: { apps: number; nameChars: number; sourceBytes: number; dataBytes: number };
}

export type AppItem = Record<string, unknown> & { id: string };
export interface AppData {
  collections: Record<string, AppItem[]>;
  stored: Record<string, unknown>;
}

export type AppDataOp =
  | { op: "add"; collection: string; value: Record<string, unknown> }
  | { op: "update"; collection: string; id: string; value: Record<string, unknown> }
  | { op: "remove"; collection: string; id: string }
  | { op: "replace"; collection: string; items: Record<string, unknown>[] }
  | { op: "set"; key: string; value: unknown };

/** GET /api/user-apps. */
export function getUserApps(options: RequestOptions = {}): Promise<UserAppsResponse> {
  return call<UserAppsResponse>("/api/user-apps", options);
}

/** GET /api/user-apps/{id}. */
export function getUserApp(id: string, options: RequestOptions = {}): Promise<{ app: UserApp }> {
  return call<{ app: UserApp }>(`/api/user-apps/${encodeURIComponent(id)}`, options);
}

/** POST /api/user-apps — add an app, or a new version of the one `id` names. */
export function saveUserApp(
  draft: { id?: string; name: string; summary?: string | null; prompt?: string | null; source: string },
  options: RequestOptions = {},
): Promise<{ app: UserApp; problems: string[] }> {
  return call<{ app: UserApp; problems: string[] }>("/api/user-apps", {
    ...options,
    method: "POST",
    contentType: "application/json",
    body: JSON.stringify(draft),
  });
}

/** POST /api/user-apps/check — compile without saving. */
export function checkUserApp(
  source: string,
  options: RequestOptions = {},
): Promise<{ code: string | null; problems: string[] }> {
  return call<{ code: string | null; problems: string[] }>("/api/user-apps/check", {
    ...options,
    method: "POST",
    contentType: "application/json",
    body: JSON.stringify({ source }),
  });
}

/** DELETE /api/user-apps/{id}. */
export function deleteUserApp(id: string, options: RequestOptions = {}): Promise<{ deleted: boolean }> {
  return call<{ deleted: boolean }>(`/api/user-apps/${encodeURIComponent(id)}`, { ...options, method: "DELETE" });
}

/** GET /api/user-apps/{id}/data. */
export function getUserAppData(id: string, options: RequestOptions = {}): Promise<AppData> {
  return call<AppData>(`/api/user-apps/${encodeURIComponent(id)}/data`, options);
}

/** POST /api/user-apps/{id}/data — one write. */
export function writeUserAppData(
  id: string,
  op: AppDataOp,
  options: RequestOptions = {},
): Promise<Record<string, unknown>> {
  return call<Record<string, unknown>>(`/api/user-apps/${encodeURIComponent(id)}/data`, {
    ...options,
    method: "POST",
    contentType: "application/json",
    body: JSON.stringify(op),
  });
}

// ── Virtual machines ──────────────────────────────────────────────────────

/* Virtual machines on the mesh, sold by the replica through the market and
 * offered only while this box shares its disk (lososd's /api/vms; the
 * registrar's edge/registrar/src/vms.rs). The shapes are the registrar's
 * documents, passed through. */

export interface VmOrderInfo {
  /** A catalogue id, or one of this box's upload ids (`upl_…`). */
  image: string;
  name: string;
  /** `https://<order>.<domain>`, when the edge publishes machines. */
  address: string | null;
  /** The replicas exist on the hosting box. */
  created: boolean;
  /** The order lapsed and every replica was stopped. */
  halted: boolean;
}

export interface VmImage {
  id: string;
  name: string;
  /** Quickemu's name for the system (`quickget <family>`), or `losos`. */
  family: string;
  /** Whether the image reads cloud-init. */
  cloud_init: boolean;
  efi: boolean;
  /** What one replica gets: the edge's size or the image's minimum. */
  disk_gib: number;
  memory_mib: number;
}

export interface VmCatalogue {
  images: VmImage[];
  /** Systems that ship installer ISOs only, so are not offered yet. */
  installer_only: string[];
  not_offered: { family: string; why: string }[];
  shape: { cpu: number; memory_mib: number; disk_gib: number };
  /** The platform's share of a machine sale: half. */
  fee_bps: number;
  max_replicas: number;
  max_upload_bytes: number;
  /** Whether a machine gets an https address on the edge. */
  published: boolean;
}

export interface VmUpload {
  id: string;
  name: string;
  /** The file is on the edge. False while an upload is under way. */
  stored: boolean;
  size: number | null;
  /** The image's virtual disk size, in GiB rounded up. */
  min_disk_gib: number;
  created_at: number;
}

/** One replica, in the few words the page draws. */
export type VmReplicaStatus = "Running" | "Starting" | "Preparing" | "Stopped" | "Paused" | "Failed";

export interface VmMachine {
  order_id: string;
  replicas: { name: string; status: VmReplicaStatus | string }[];
}

export type VmUnavailableReason = "notSharing" | "noOfficialEdge" | "notOffered";

export type VmsResponse =
  | { available: false; reason: VmUnavailableReason }
  | {
      available: true;
      catalogue: VmCatalogue;
      /** Machine listings only. */
      listings: MarketShelfListing[];
      account: MarketAccount;
      machines: VmMachine[];
    };

/** GET /api/vms — the catalogue, the machine listings, this box's account
 *  and what its machines are doing; `{available: false}` with a reason when
 *  machines are not offered here. */
export function getVms(options: RequestOptions = {}): Promise<VmsResponse> {
  return call<VmsResponse>("/api/vms", options);
}

/** POST /api/vms/orders — the reply carries the Checkout URL. */
export function postVmOrder(
  order: { listing_id: string; quantity: number; image: string; name: string; user_data?: string },
  options: RequestOptions = {},
): Promise<MarketActionResponse> {
  return marketPost("/api/vms/orders", order, options);
}

/** POST /api/vms/listings — host machines on this box, priced per
 *  replica-month. */
export function postVmListing(
  listing: { unit_price: number; capacity: number },
  options: RequestOptions = {},
): Promise<MarketActionResponse> {
  return marketPost("/api/vms/listings", listing, options);
}

/** POST /api/vms/listings/close */
export function postVmListingClose(
  listingId: string,
  options: RequestOptions = {},
): Promise<MarketActionResponse> {
  return marketPost("/api/vms/listings/close", { listing_id: listingId }, options);
}

/** POST /api/vms/images/remove — delete one of this box's images. */
export function postVmImageRemove(
  uploadId: string,
  options: RequestOptions = {},
): Promise<MarketActionResponse> {
  return marketPost("/api/vms/images/remove", { upload_id: uploadId }, options);
}

export interface VmImageStored {
  upload_id: string;
  size: number;
  min_disk_gib: number;
}

/** PUT /api/vms/images?name=&efi= — stream a QCOW2 to the edge.
 *
 * XMLHttpRequest rather than fetch, because a disk image is gigabytes and
 * fetch reports no upload progress. `onProgress` gets 0 to 100. The token
 * rules are `call`'s: a 401 drops the stored token. */
export function putVmImage(
  file: Blob,
  { name, efi }: { name: string; efi: boolean },
  { onProgress, signal }: { onProgress?: (percent: number) => void; signal?: AbortSignal } = {},
): Promise<VmImageStored> {
  return new Promise((resolve, reject) => {
    const xhr = new XMLHttpRequest();
    const query = new URLSearchParams({ name });
    if (efi) query.set("efi", "1");
    xhr.open("PUT", `/api/vms/images?${query.toString()}`);
    const bearer = getToken();
    if (bearer !== null) xhr.setRequestHeader("Authorization", `Bearer ${bearer}`);
    xhr.setRequestHeader("Content-Type", "application/octet-stream");
    xhr.upload.onprogress = (event) => {
      if (event.lengthComputable && onProgress !== undefined) {
        onProgress((event.loaded / event.total) * 100);
      }
    };
    xhr.onload = () => {
      let body: unknown = null;
      try {
        body = JSON.parse(xhr.responseText);
      } catch {
        /* nginx's own page, not lososd's JSON */
      }
      if (xhr.status === 401) {
        dropToken();
        reject(new ApiError(401, "unauthorized"));
      } else if (xhr.status >= 200 && xhr.status < 300) {
        resolve(body as VmImageStored);
      } else {
        const said = (body as { error?: unknown } | null)?.error;
        reject(new ApiError(xhr.status, typeof said === "string" && said.length > 0 ? said : `HTTP ${xhr.status}`));
      }
    };
    xhr.onerror = () => reject(new ApiError(0, "the upload was cut off"));
    xhr.onabort = () => reject(new DOMException("aborted", "AbortError"));
    signal?.addEventListener("abort", () => xhr.abort(), { once: true });
    xhr.send(file);
  });
}

// ── Installed apps ────────────────────────────────────────────────────────

/* Apps the owner installed from the catalogue search, run in the box's own
 * cluster as one of its two data users (backend/src/apps.rs). */

/** Where the box fetches an app from. */
export interface AppSource {
  repo: string;
  name: string;
  version: string;
}

/** The two data users: the private side and the side lent to the mesh. */
export type AppRunAs = "notshared" | "shared";

export type AppPhase = "installing" | "running" | "failed" | "removing";

export interface InstalledApp {
  release: string;
  /** A chart from the search, or an app in the LosOS app format (a Compose
   *  file, from the AI builder or sent by the owner). */
  kind: "chart" | "compose";
  /** Null for a compose app. */
  chart: AppSource | null;
  /** What a compose app is called on the page, when it says. */
  title: string | null;
  runAs: AppRunAs;
  /** What runs it, which follows from runAs. The page never shows it. */
  runtime: "podman" | "kube";
  /** What the property form changed when it was last installed. */
  values: Record<string, unknown>;
  valuesYaml: string | null;
  /** The port the box opens for it on the LAN. */
  frontPort: number;
  /** The port it listens on, once known; null for an app with no page. */
  appPort: number | null;
  phase: AppPhase;
  /** Why it failed, in the box's words. */
  message: string | null;
  updatedAt: number;
}

export type AppsResponse =
  | { available: false; reason: "off"; apps: [] }
  | { available: true; sharedAvailable: boolean; apps: InstalledApp[] };

/** GET /api/apps */
export function getApps(options: RequestOptions = {}): Promise<AppsResponse> {
  return call<AppsResponse>("/api/apps", options);
}

export interface AppChartResponse {
  chart: AppSource;
  /** The name the box suggests. */
  release: string;
  valuesYaml: string;
  values: unknown;
  /** A JSON schema for the values, when the app ships one. */
  schema: unknown;
}

/** GET /api/apps/chart — the app's properties and their defaults. Slow: the
 *  box fetches the app to read them. */
export function getAppChart(source: AppSource, options: RequestOptions = {}): Promise<AppChartResponse> {
  const query = new URLSearchParams({ repo: source.repo, name: source.name, version: source.version });
  return call<AppChartResponse>(`/api/apps/chart?${query.toString()}`, options);
}

export interface AppInstallRequest {
  release: string;
  chart: AppSource;
  runAs: AppRunAs;
  values: Record<string, unknown>;
  valuesYaml: string | null;
}

/** POST /api/apps/install — install, or change an app under the same name. */
export function postAppInstall(request: AppInstallRequest, options: RequestOptions = {}): Promise<InstalledApp> {
  return call<InstalledApp>("/api/apps/install", {
    ...options,
    method: "POST",
    contentType: "application/json",
    body: JSON.stringify(request),
  });
}

/** POST /api/apps/remove — its files stay in the user's data folder. */
export function postAppRemove(release: string, options: RequestOptions = {}): Promise<InstalledApp> {
  return call<InstalledApp>("/api/apps/remove", {
    ...options,
    method: "POST",
    contentType: "application/json",
    body: JSON.stringify({ release }),
  });
}

// ── Custom domains ────────────────────────────────────────────────────────

/* A domain the owner already has, pointed at this box through an official
 * edge (edge/registrar/src/domains.rs). lososd relays it like the market
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

/** What the edge last saw of the wallet record at an owner's domain. */
export type DomainWalletSeen = "found" | "unsigned" | "cname" | "other" | "missing";

/** The wallet bound to this box's names on the edge. */
export interface BoundWallet {
  address: string;
  bound_at: number;
  /** The TXT value (`ENS1 …`) at the box's names, and for the owner's own
   *  domains. */
  record: string;
  /** What public DNS showed of each of the box's names when the edge last
   *  looked: `found` means a buyer's wallet finds it. A name not looked up
   *  yet is absent, and so is the map on an edge that predates the check. */
  published?: Record<string, DomainWalletSeen>;
  published_at?: number | null;
}

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
  /** The wallet record at the domain itself, while a wallet is bound:
   *  `found` means ENS resolves the domain to the wallet. Absent from an
   *  edge that predates wallets. */
  wallet?: DomainWalletSeen | null;
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
      /** This box's id on the edge and the edge's zone, which the wallet
       *  binding message names (`walletMessage`). This and the three below
       *  are absent from an edge that predates wallets. */
      box?: string;
      zone?: string;
      /** This box's names in the edge's zone; they carry the wallet record
       *  while one is bound. */
      names?: string[];
      wallet?: BoundWallet | null;
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

/** POST /api/domains/wallet — publish the wallet at the box's names, with
 *  its signature of `walletMessage`; no address takes it off. */
export function postDomainWallet(
  binding: { address: string; signature: string } | null,
  options: RequestOptions = {},
): Promise<DomainsResponse> {
  return call<DomainsResponse>("/api/domains/wallet", {
    ...options,
    method: "POST",
    contentType: "application/json",
    body: JSON.stringify(binding ?? {}),
  });
}

// ── Backups and erasing the box ───────────────────────────────────────────

/** The bucket backups go to, as lososd shows it: never the secret key. */
/** How AWS keeps the data; anything but `standard` is Amazon S3 only. */
export type StorageClass = "standard" | "glacierInstant" | "glacier" | "deepArchive";

export interface BackupTarget {
  endpoint: string;
  bucket: string;
  prefix: string;
  region: string;
  accessKeyId: string;
  hasSecret: boolean;
  /** Missing from a box older than the choice: read it as `standard`. */
  storageClass?: StorageClass;
  /** Set when the backups go to storage this box bought on the mesh. The
   *  S3 fields are empty then. */
  bought?: BoughtStorage | null;
}

/** A storage order of this box's, reached through the edge that sold it. */
export interface BoughtStorage {
  order: string;
  edge: string;
}

/** What the owner types. Leave `secretAccessKey` out to keep the stored one. */
export interface BackupTargetInput {
  endpoint: string;
  bucket: string;
  prefix: string;
  region: string;
  accessKeyId: string;
  secretAccessKey?: string;
  storageClass?: StorageClass;
}

export interface BackupReport {
  /** Unix seconds. */
  time: number;
  snapshot: string;
  files: number;
  bytes: number;
}

export interface BackupJob {
  kind: "backup" | "restore";
  job: string;
  state: "running" | "done" | "failed" | "cancelled";
  startedAt: number;
  finishedAt?: number;
  /** Why it failed, in lososd's words. */
  message: string;
}

export type ErasePhase =
  | "backingUp"
  | "waiting"
  | "leaving"
  | "resetting"
  | "restarting"
  | "failed";

/** What the box gave up outside itself. Counts only. */
export interface EraseOutside {
  hadEdge: boolean;
  domainsRemoved: number;
  listingsClosed: number;
  leftEdge: boolean;
  problems: string[];
  /** Unix seconds; 0 until the erase reached the wipe. */
  erasedAt: number;
}

export interface Erase {
  phase: ErasePhase;
  backup: boolean;
  startedAt: number;
  /** Set once the countdown starts. */
  deadline?: number;
  secondsLeft?: number;
  cancellable: boolean;
  message: string;
  outside: EraseOutside;
}

export interface BackupResponse {
  target: BackupTarget | null;
  last: BackupReport | null;
  job: BackupJob | null;
  erase: Erase | null;
  /** The report the last erase left behind, kept past the wipe. */
  lastErase: EraseOutside | null;
  graceSeconds: number;
}

/** GET /api/backup — the bucket, the last backup, the job, any erase. */
export function getBackup(options: RequestOptions = {}): Promise<BackupResponse> {
  return call<BackupResponse>("/api/backup", options);
}

/** POST /api/backup/target — set the bucket. */
export function postBackupTarget(
  target: BackupTargetInput,
  options: RequestOptions = {},
): Promise<{ target: BackupTarget }> {
  return call<{ target: BackupTarget }>("/api/backup/target", {
    ...options,
    method: "POST",
    contentType: "application/json",
    body: JSON.stringify(target),
  });
}

/** POST /api/backup/target/bought — keep backups on storage this box bought
 *  on the mesh. `keyed` is false when the edge would not take the key the
 *  recovery code makes; the target is kept and the next backup says why. */
export function postBackupTargetBought(
  order: string,
  options: RequestOptions = {},
): Promise<{ target: BackupTarget; keyed: boolean }> {
  return call<{ target: BackupTarget; keyed: boolean }>("/api/backup/target/bought", {
    ...options,
    method: "POST",
    contentType: "application/json",
    body: JSON.stringify({ order }),
  });
}

/** DELETE /api/backup/target — forget the bucket. The backups stay in it. */
export function deleteBackupTarget(options: RequestOptions = {}): Promise<{ target: null }> {
  return call<{ target: null }>("/api/backup/target", { ...options, method: "DELETE" });
}

/** POST /api/backup/run — back up now. */
export function postBackupRun(options: RequestOptions = {}): Promise<{ job: string }> {
  return call<{ job: string }>("/api/backup/run", { ...options, method: "POST" });
}

/** POST /api/backup/restore — pull the latest backup back with the
 *  recovery code of the box that made it. */
export function postBackupRestore(
  code: string,
  options: RequestOptions = {},
): Promise<{ job: string }> {
  return call<{ job: string }>("/api/backup/restore", {
    ...options,
    method: "POST",
    contentType: "application/json",
    body: JSON.stringify({ code }),
  });
}

/** GET /api/recovery — the box's recovery code, which is also the key to
 *  its backups. */
export function getRecoveryCode(options: RequestOptions = {}): Promise<{ code: string }> {
  return call<{ code: string }>("/api/recovery", options);
}

/** POST /api/erase — start erasing the box, with or without a backup first. */
export function postErase(backup: boolean, options: RequestOptions = {}): Promise<{ erase: Erase }> {
  return call<{ erase: Erase }>("/api/erase", {
    ...options,
    method: "POST",
    contentType: "application/json",
    body: JSON.stringify({ backup }),
  });
}

/** POST /api/erase/cancel — stop an erase before it reaches the edge. */
export function postEraseCancel(options: RequestOptions = {}): Promise<unknown> {
  return call<unknown>("/api/erase/cancel", { ...options, method: "POST" });
}

// ── Terminal access ───────────────────────────────────────────────────────

/* Personal access tokens for the `losos` terminal client, each bound to one
 * of the GPG keys the installer seeded (docs/terminal-access.md, "The web
 * UI's side"). The web UI lists the keys and cannot change them; it makes
 * and revokes tokens, and sees a token's secret only in the reply that made
 * it. The box keeps the SHA-256 alone. */

/** `read` allows GET only; `full` allows every method. */
export type AccessScope = "read" | "full";

/** A public key seeded at install (`losos.access.gpgKeys`). */
export interface AccessKey {
  /** 40 hex characters, upper case. */
  fingerprint: string;
  userIds: string[];
  /** Unix seconds, or null when the key does not say. */
  created: number | null;
}

/** A token as lososd lists it: never the secret. */
export interface AccessToken {
  id: string;
  name: string;
  /** The fingerprint of the seeded key the token is bound to. */
  fingerprint: string;
  scope: AccessScope;
  /** Unix seconds. */
  created: number;
  /** Unix seconds, or null for a token that does not expire. */
  expires: number | null;
  /** Unix seconds, or null for a token never used. */
  lastUsed: number | null;
}

export interface AccessResponse {
  /** This box's own OpenPGP key, which the client pins on first contact. */
  hostKey: { fingerprint: string } | null;
  keys: AccessKey[];
  tokens: AccessToken[];
}

/** The expiry choices lososd accepts; null is "never". */
export type AccessExpiry = 30 | 90 | 365 | null;

export interface AccessTokenInput {
  name: string;
  fingerprint: string;
  scope: AccessScope;
  expiresInDays: AccessExpiry;
}

export interface AccessTokenCreated {
  /** `losos_pat_<id>_<secret>`. Shown once, never served again. */
  token: string;
  record: AccessToken;
}

/** GET /api/access — the host key, the seeded keys and the tokens. */
export function getAccess(options: RequestOptions = {}): Promise<AccessResponse> {
  return call<AccessResponse>("/api/access", options);
}

/** POST /api/access/tokens — make a token. 400 for a bad name, key or
 *  scope, 409 when the box already holds 32. */
export function postAccessToken(
  input: AccessTokenInput,
  options: RequestOptions = {},
): Promise<AccessTokenCreated> {
  return call<AccessTokenCreated>("/api/access/tokens", {
    ...options,
    method: "POST",
    contentType: "application/json",
    body: JSON.stringify(input),
  });
}

/** POST /api/access/tokens/revoke — revoke a token. 404 when it is gone. */
export function postAccessRevoke(
  id: string,
  options: RequestOptions = {},
): Promise<{ revoked: boolean }> {
  return call<{ revoked: boolean }>("/api/access/tokens/revoke", {
    ...options,
    method: "POST",
    contentType: "application/json",
    body: JSON.stringify({ id }),
  });
}

// ── Credentials (the Credentials pane) ───────────────────────────────────

/** What every credential on the box is sealed to: the TPM, or the host key
 *  on a box without one. */
export type CredentialKey = "tpm2" | "host";

/** A credential as lososd lists it (backend/src/credentials.rs). Never its
 *  value: the API has no route that returns one. */
export interface Credential {
  name: string;
  /** What it belongs to: "login", "mail", "admin", "backup", "cloud", … */
  app: string;
  /** lososd's English sentence, for a name this page has no words for. */
  description: string;
  /** "box" for one the box makes itself, "owner" for one a person types. */
  source: "box" | "owner";
  /** Whether the sealed file is there. */
  set: boolean;
  /** Unix seconds of the last change, or null when it is not set. */
  updated: number | null;
  sealedWith: CredentialKey;
  /** The units that read it. */
  services: string[];
  /** Whether a new value can be made from here. */
  rotate: boolean;
  /** For one the owner gives: the admin page where they set it. */
  pane: string | null;
  /** At boot the old file would not open, so the box made a new value. */
  remade: boolean;
  /** At boot the box sealed a value an older version kept as a plain file. */
  adopted: boolean;
}

export interface CredentialsResponse {
  /** null on a build without a credential store. */
  key: CredentialKey | null;
  credentials: Credential[];
}

/** GET /api/credentials — every credential, without a value. */
export function getCredentials(options: RequestOptions = {}): Promise<CredentialsResponse> {
  return call<CredentialsResponse>("/api/credentials", options);
}

/** POST /api/credentials/{name}/rotate — make a new value. 409 for one the
 *  box does not make itself. The answer is the row, never the value. */
export function postCredentialRotate(
  name: string,
  options: RequestOptions = {},
): Promise<{ credential: Credential }> {
  return call<{ credential: Credential }>(`/api/credentials/${encodeURIComponent(name)}/rotate`, {
    ...options,
    method: "POST",
  });
}

// ── Logs (Settings › Logs) ────────────────────────────────────────────────

/* The journal of the box's services, from the log reader (backend/src/logs/,
 * modules/log-reader.nix) rather than lososd, so the pane answers while
 * lososd does not. Every line is redacted on the box before it is sent. */

/** How far back: this boot, the one before, or a stretch of time. */
export type LogSpan = "boot" | "previous" | "hour" | "day" | "week";

/** What the pane shows. `unit` is a source's id (`lososd`, `cloud`, `losos`,
 *  `everything`, …) or a unit's own name. `priority` is journald's: at most
 *  this, so 3 is errors and worse. */
export interface LogFilter {
  unit: string;
  priority: number;
  span: LogSpan;
  text: string;
}

export interface LogLine {
  /** The journal's cursor, to ask for what came before or after. */
  cursor: string;
  /** In the box's own time zone: `2026-10-29` and `12:00:01`. */
  date: string;
  time: string;
  /** RFC 3339. */
  stamp: string;
  unit: string;
  pid: number | null;
  /** 0 (emergency) to 7 (debug). */
  priority: number;
  message: string;
}

export interface LogsResponse {
  /** The ids of the Service list, in its order. */
  sources: string[];
  unit: string;
  /** Oldest first. */
  lines: LogLine[];
  /** Ask with `before` for the lines older than these, when there are any. */
  older: string | null;
  /** Ask with `after` for the lines that came since. */
  newer: string | null;
  /** The journal could not be read: why. */
  error: string | null;
}

/** The filter as the reader reads it, without `?`; the page without
 *  JavaScript takes the same one (backend/plugins/page/src/logs.rs). */
export function logsQuery(filter: LogFilter): string {
  const q = new URLSearchParams({
    unit: filter.unit,
    priority: String(filter.priority),
    span: filter.span,
  });
  if (filter.text.trim() !== "") q.set("q", filter.text.trim());
  return q.toString();
}

/** GET /api/logs — the newest lines, the ones before `before`, or the ones
 *  after `after` (following). */
export function getLogs(
  filter: LogFilter,
  place: { before?: string; after?: string } = {},
  options: RequestOptions = {},
): Promise<LogsResponse> {
  const q = new URLSearchParams(logsQuery(filter));
  if (place.before !== undefined) q.set("before", place.before);
  if (place.after !== undefined) q.set("after", place.after);
  return call<LogsResponse>(`/api/logs?${q.toString()}`, options);
}

/** GET /api/logs/download — up to 5000 lines as a text file, for a bug
 *  report, with the name the box gave it. */
export async function downloadLogs(filter: LogFilter): Promise<{ blob: Blob; name: string }> {
  const headers: Record<string, string> = {};
  const bearer = getToken();
  if (bearer !== null) headers["Authorization"] = `Bearer ${bearer}`;
  const res = await fetch(apiPath(`/api/logs/download?${logsQuery(filter)}`), {
    headers,
    cache: "no-store",
    credentials: "same-origin",
  });
  if (!res.ok) throw new ApiError(res.status, res.statusText);
  const name = /filename="([^"]+)"/.exec(res.headers.get("Content-Disposition") ?? "")?.[1] ?? "losos.log";
  return { blob: await res.blob(), name };
}

/** The pane's address for one unit, as a problem report links to it. */
export function logsPath(unit: string): string {
  return `/settings/logs?unit=${encodeURIComponent(unit)}`;
}

// ── People (the Users pane) ───────────────────────────────────────────────

/** Someone who may sign in to this box, as GET /api/people lists them. */
export interface PersonEntry {
  name: string;
  displayName: string;
  email: string;
  /** losos-admins, losos-viewers, losos-<section>, and losos-owner for the
   *  person who claimed the box. */
  groups: string[];
  disabled: boolean;
  /** Has an authenticator app or a passkey on LosOS Login. */
  twoFactor?: boolean;
  /** When their password was last set, in seconds since 1970. */
  passwordSetAt?: number;
  /** What the rules hold them to their own account for. */
  locked?: LoginReason[];
  /** Their profile picture, banner and colour (`GET /api/profiles` has
   *  the rest). */
  picture?: string | null;
  banner?: string | null;
  accent?: Accent;
  role?: Role;
  roleName?: string | null;
  muted?: Hold | null;
  suspended?: Hold | null;
  /** Marked as an adult or a child (`GET /api/pictures`), or null. */
  age?: AgeMark | null;
}

export interface PeopleResponse {
  /** False on a box whose sign-in runs elsewhere; `people` is empty then. */
  standalone: boolean;
  /** The caller's own name, null for the spare key. */
  you: string | null;
  /** The parts of the box a person can be given one by one. */
  sections: string[];
  /** LosOS Login's rules; missing on a lososd that predates them. */
  rules?: LoginRules;
  /** Roles saved under a name; missing on an older lososd. */
  roles?: CustomRole[];
  people: PersonEntry[];
}

export interface NewPerson {
  name: string;
  displayName: string;
  password: string;
  groups: string[];
}

/** GET /api/people — everyone who may sign in, never a password. */
export function getPeople(options: RequestOptions = {}): Promise<PeopleResponse> {
  return call<PeopleResponse>("/api/people", options);
}

function postJson<T>(path: string, body: unknown, options: RequestOptions): Promise<T> {
  return call<T>(path, {
    ...options,
    method: "POST",
    contentType: "application/json",
    body: JSON.stringify(body),
  });
}

/** POST /api/people — add someone. 400 for a bad name or password, 409 for
 *  a name already taken. */
export function postPerson(person: NewPerson, options: RequestOptions = {}): Promise<{ name: string }> {
  return postJson("/api/people", person, options);
}

/** POST /api/people/access — replace someone's access groups. */
export function postPersonAccess(
  name: string,
  groups: string[],
  options: RequestOptions = {},
): Promise<{ name: string; groups: string[] }> {
  return postJson("/api/people/access", { name, groups }, options);
}

/** POST /api/people/password — a new password for someone who forgot theirs. */
export function postPersonPassword(
  name: string,
  password: string,
  options: RequestOptions = {},
): Promise<{ name: string; changed: boolean }> {
  return postJson("/api/people/password", { name, password }, options);
}

/** POST /api/people/remove — they can no longer sign in; their files stay. */
export function postPersonRemove(
  name: string,
  options: RequestOptions = {},
): Promise<{ name: string; removed: boolean }> {
  return postJson("/api/people/remove", { name }, options);
}

/** POST /api/people/rules — LosOS Login's two rules. 409 `needsOwnFactor`
 *  when the caller turns two-factor sign-in on without a second factor of
 *  their own. */
export function postPeopleRules(rules: LoginRules, options: RequestOptions = {}): Promise<LoginRules> {
  return postJson("/api/people/rules", rules, options);
}

/** POST /api/people/two-factor/reset — remove someone's authenticator app
 *  and passkeys, for a lost phone or key. */
export function postPersonFactorsReset(
  name: string,
  options: RequestOptions = {},
): Promise<{ name: string; reset: boolean }> {
  return postJson("/api/people/two-factor/reset", { name }, options);
}

// ── Your own account ──────────────────────────────────────────────────────

/** GET /api/account: the caller's own account on LosOS Login. */
export interface Account {
  name: string;
  displayName: string;
  email: string;
  factors: { app: boolean; passkeys: string[] };
  /** Seconds since 1970. */
  passwordSetAt: number;
  /** When the password runs out, or null when it does not. */
  passwordExpiresAt: number | null;
  rules: LoginRules;
  locked: LoginReason[];
}

export function getAccount(options: RequestOptions = {}): Promise<Account> {
  return call<Account>("/api/account", options);
}

/** POST /api/account — their own name and email address. */
export function postAccount(
  details: { displayName: string; email: string },
  options: RequestOptions = {},
): Promise<{ name: string; displayName: string; email: string }> {
  return postJson("/api/account", details, options);
}

/** POST /api/account/code — the one-time code LosOS Login last made for
 *  them, behind their password. 403 `wrongPassword`, 404 `noCode`. */
export function postAccountCode(password: string, options: RequestOptions = {}): Promise<{ code: string }> {
  return postJson("/api/account/code", { password }, options);
}

// ── Windows network share ─────────────────────────────────────────────────

/* Each person's LosOS Cloud files as `\\<box>\<share>` in Windows'
 * Network view, Finder and Linux file managers (backend/src/windows.rs,
 * modules/windows.nix). Windows signs in with the LosOS password once the
 * box has seen it in the clear. */
export type WindowsShare =
  | { on: false }
  | {
      on: true;
      /** Windows takes the caller's LosOS password now. */
      ready: boolean;
      /** The share's name under `\\<host>\`. */
      share: string;
      host: string;
    };

/** GET /api/windows — the caller's share and whether it is ready. */
export function getWindowsShare(options: RequestOptions = {}): Promise<WindowsShare> {
  return call<WindowsShare>("/api/windows", options);
}

/** POST /api/windows — hand the box the caller's password once, so Windows
 *  takes it. 403 for a wrong password. */
export function postWindowsShare(password: string, options: RequestOptions = {}): Promise<WindowsShare> {
  return postJson("/api/windows", { password }, options);
}

// ── Tor and I2P ───────────────────────────────────────────────────────────

/* The box's addresses on Tor and I2P (modules/hidden-services.nix,
 * backend/plugins/hidden): those its own Tor and I2P made, and those the
 * edge it reports to made for it (modules/edge-hidden.nix). A name is null
 * until it is made, and while its network is off. */
export interface HiddenPair {
  tor: string | null;
  i2p: string | null;
}

export interface HiddenAddresses {
  box: HiddenPair;
  edge: HiddenPair;
}

/** GET /api/hidden — this box's Tor and I2P addresses. */
export function getHidden(options: RequestOptions = {}): Promise<HiddenAddresses> {
  return call<HiddenAddresses>("/api/hidden", options);
}

// ── Profiles ──────────────────────────────────────────────────────────────

/* A person's picture, banner, colour, pronouns and a line about them
 * (backend/src/profiles.rs, docs/profiles.md). Everyone signed in reads
 * everyone's; a person changes only their own. lososd crops, scales and
 * re-encodes every picture, so what comes back has no camera tags. */

/** The colours a profile can have, in the order the picker shows them. */
export const ACCENTS = ["teal", "blue", "violet", "rose", "amber", "green"] as const;
export type Accent = (typeof ACCENTS)[number];

export interface Profile {
  name: string;
  displayName: string;
  pronouns: string;
  about: string;
  accent: Accent;
  /** False while the colour is the one their name gives them. */
  accentChosen: boolean;
  /** `/api/profiles/<name>/picture?v=N`, or null. */
  picture: string | null;
  /** `/api/profiles/<name>/banner?v=N`, or null. */
  banner: string | null;
  /** Their role; missing on an older lososd. */
  role?: Role;
  roleName?: string | null;
}

export type ProfileImage = "picture" | "banner";

/** The most characters lososd keeps in each line (profiles.rs). */
export const MAX_PRONOUNS_CHARS = 40;
export const MAX_ABOUT_CHARS = 160;

/** GET /api/profiles — everyone LosOS Login lets in. */
export function getProfiles(options: RequestOptions = {}): Promise<{ people: Profile[] }> {
  return call<{ people: Profile[] }>("/api/profiles", options);
}

/** GET /api/profiles/{name}. */
export function getProfile(name: string, options: RequestOptions = {}): Promise<Profile> {
  return call<Profile>(`/api/profiles/${encodeURIComponent(name)}`, options);
}

/** POST /api/account/profile — the caller's own words and colour. Absent
 *  fields stay; `accent: null` goes back to the colour of their name. */
export function postAccountProfile(
  details: { pronouns?: string; about?: string; accent?: Accent | null },
  options: RequestOptions = {},
): Promise<Profile> {
  return postJson("/api/account/profile", details, options);
}

/** PUT /api/account/{picture,banner} — the bytes; lososd reads the type
 *  off them. */
export function putAccountImage(kind: ProfileImage, file: Blob, options: RequestOptions = {}): Promise<Profile> {
  return call<Profile>(`/api/account/${kind}`, {
    ...options,
    method: "PUT",
    contentType: file.type.length > 0 ? file.type : "application/octet-stream",
    body: file,
  });
}

/** DELETE /api/account/{picture,banner}. */
export function deleteAccountImage(kind: ProfileImage, options: RequestOptions = {}): Promise<Profile> {
  return call<Profile>(`/api/account/${kind}`, { ...options, method: "DELETE" });
}

/** POST /api/people/profile/clear — an administrator takes a picture down. */
export function clearPersonImage(
  name: string,
  kind: ProfileImage,
  options: RequestOptions = {},
): Promise<{ name: string; kind: ProfileImage; cleared: boolean }> {
  return postJson("/api/people/profile/clear", { name, kind }, options);
}

/** POST /api/roles — save a custom role. */
export function postRole(
  role: { id?: string; name: string; sections: string[] },
  options: RequestOptions = {},
): Promise<{ id: string; roles: CustomRole[] }> {
  return postJson("/api/roles", role, options);
}

/** POST /api/roles/remove — forget a custom role. */
export function postRoleRemove(id: string, options: RequestOptions = {}): Promise<{ roles: CustomRole[] }> {
  return postJson("/api/roles/remove", { id }, options);
}

/** GET /api/moderation — the queue, everyone's holds, the log. */
export function getModeration(options: RequestOptions = {}): Promise<ModerationView> {
  return call<ModerationView>("/api/moderation", options);
}

/** POST /api/moderation/reports/{id} */
export function postReportAction(id: string, action: ReportAction, options: RequestOptions = {}): Promise<Report> {
  return postJson(`/api/moderation/reports/${encodeURIComponent(id)}`, { action }, options);
}

/** POST /api/moderation/{mute,unmute,suspend,unsuspend}. `minutes` null:
 *  until lifted. */
export function postHold(
  kind: "mute" | "unmute" | "suspend" | "unsuspend",
  body: { name: string; minutes?: number | null; reason?: string },
  options: RequestOptions = {},
): Promise<{ name: string }> {
  return postJson(`/api/moderation/${kind}`, body, options);
}

/** LosOS Login's own page for adding an authenticator app or a passkey. */
export function twoFactorSettingsUrl(): string {
  return `https://${window.location.host}/auth/settings/two-factor-authentication`;
}

// ── Sign-in ───────────────────────────────────────────────────────────────

/* Probe a pasted token against a cheap authed route and store it if lososd
 * accepts it. Resolves false on rejection, throws on anything else, and a
 * rejected candidate leaves the stored token alone. The spare way in, for
 * a day LosOS Login does not answer. */
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
  await loadMe().catch(() => null);
  return true;
}

/* POST /api/setup/claim with the spare key: the first account on a box that
 * was set up before people signed in through Authelia. The password becomes
 * that person's sign-in password. */
export async function makeFirstAccount(
  name: string,
  password: string,
  options: RequestOptions = {},
): Promise<{ name: string }> {
  return call<{ name: string }>("/api/setup/claim", {
    ...options,
    method: "POST",
    contentType: "application/json",
    body: JSON.stringify({ name, password }),
  });
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

    if (!hasToken()) {
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
   * twelve above, sorted so the file is stable from one apply to the next
   * and the commit lososd makes shows only what changed. A name here that is
   * one of the twelve would be a second line for the same key; the first
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
    `  losos.hostName = ${nixString(settings.hostName)};`,
    `  losos.gpu.enable = ${nixBool(settings.gpuEnable)};`,
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

/** The twelve `losos.*` names the lines above always write. */
export const OWNED_NAMES: ReadonlySet<string> = new Set([
  "sharingMyStorage",
  "hostName",
  "gpu.enable",
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
 * the address LosOS Cloud hands its apps and the addresses LosOS Git hands out. A space or a slash
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

/** One file of a widget: `index.html`, or a file it links. */
export interface WidgetFile {
  name: string;
  content: string;
}

/** One widget the owner wrote: a name, a width, and its files. */
export interface HandWidget {
  id: string;
  name: string;
  span: HandSpan;
  files: WidgetFile[];
}

export interface LookLimits {
  widgets: number;
  nameChars: number;
  /** All of one widget's files together. */
  widgetBytes: number;
  /** Files in one widget. */
  files: number;
  fileNameChars: number;
  /** Extensions a widget's file may have. */
  fileKinds: string[];
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
  files: WidgetFile[];
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

/* The editor kind of one `losos.*` option, as nix/options-doc.nix
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
  /** `losos.` is implied: "cloud.trashDays". */
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
  /** The bool option this one takes effect under; while that is off the
   *  pane shows this row as unavailable. */
  needs?: string;
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

/** GET /api/site: the settings of LosOS Cloud and LosOS Git only, in the
 *  shape of GET /api/options. Open to the apps section and to a company edge
 *  the owner lets manage the box. */
export function getSite(options: RequestOptions = {}): Promise<OptionsResponse> {
  return call<OptionsResponse>("/api/site", options);
}

/** POST /api/site: set each named option to its Nix literal, or back to its
 *  default with null. The box changes those lines and rebuilds. */
export function postSite(set: Readonly<Record<string, string | null>>): Promise<ChangeResponse> {
  return call<ChangeResponse>("/api/site", {
    method: "POST",
    body: JSON.stringify({ set }),
    contentType: "application/json",
  });
}

/* ── The company edge's manage page ───────────────────────────────────────
 * Served by the edge itself (edge/registrar/src/server/manage.rs) with the
 * edge's manage key as the Bearer token. */

/** A box this edge routes, and whether its owner lets the edge manage it. */
export interface ManagedBox {
  id: string;
  hostname: string;
  seenAgoSecs: number;
  managed: boolean;
}

/** GET /manage/api/boxes on the edge. */
export function getManagedBoxes(options: RequestOptions = {}): Promise<{ boxes: ManagedBox[] }> {
  return call<{ boxes: ManagedBox[] }>("/manage/api/boxes", options);
}

/** GET /manage/api/statistics on the edge: what its Traefik counted, read
 *  by `readStatistics` (src/statistics/types.ts). */
export function getEdgeStatistics(options: RequestOptions = {}): Promise<unknown> {
  return call<unknown>("/manage/api/statistics", options);
}

/** The gateway itself, as GET /manage/api/gateway on the edge reports it. */
export interface GatewaySettings {
  name: string;
  /** Null on an edge that is not an open gateway (no TPM, no passport). */
  passport: {
    edgeKey: string;
    claimed: boolean;
    owner: { fingerprint: string; userId: string } | null;
  } | null;
  /** Null on an edge that relays to no hub. */
  uplink: {
    set: { id: string; registrarUrl: string; tunnelEndpoint: string } | null;
    relaying: number;
  } | null;
  /** Boxes that enrolled here on first contact; only these can be forgotten. */
  enrolled: string[];
  /** Null on an edge that runs no secondary cluster. */
  cluster?: { joined: GatewayCluster | null } | null;
  /** The main cluster as the hub last answered: gateways on primary links. */
  main?: GatewayMainRow[];
}

/** The secondary cluster this gateway is in (edge/registrar/src/secondary.rs). */
export interface GatewayCluster {
  name: string;
  /** This gateway's name in it. */
  member: string;
  /** What another gateway of this network joins with. */
  secret: string;
  /** This gateway's address, which another gateway joins through. */
  url: string;
  /** The gateways that answer, this one first. */
  gateways: { member: string; url: string }[];
}

export interface GatewayMainRow {
  site: string;
  gateway: string;
  cluster: string | null;
  boxes: number;
}

/** PUT /manage/api/gateway/cluster: a name alone starts a cluster; a name,
 *  the founder's secret and its address join that one. */
export function setGatewayCluster(form: { name: string; secret?: string; peer?: string }): Promise<GatewayCluster> {
  return call<GatewayCluster>("/manage/api/gateway/cluster", {
    method: "PUT",
    body: JSON.stringify(form),
    contentType: "application/json",
  });
}

/** DELETE /manage/api/gateway/cluster: stand alone again. */
export function leaveGatewayCluster(): Promise<{ joined: null }> {
  return call<{ joined: null }>("/manage/api/gateway/cluster", { method: "DELETE" });
}

export interface UplinkForm {
  id: string;
  token: string;
  registrarUrl: string;
  tunnelEndpoint: string;
}

/** GET /manage/api/gateway on the edge. */
export function getGatewaySettings(options: RequestOptions = {}): Promise<GatewaySettings> {
  return call<GatewaySettings>("/manage/api/gateway", options);
}

/** PUT /manage/api/gateway/uplink: relay this gateway's boxes through a hub. */
export function setGatewayUplink(form: UplinkForm): Promise<{ set: true }> {
  return call<{ set: true }>("/manage/api/gateway/uplink", {
    method: "PUT",
    body: JSON.stringify(form),
    contentType: "application/json",
  });
}

/** DELETE /manage/api/gateway/uplink: the boxes stay reachable on the LAN only. */
export function clearGatewayUplink(): Promise<{ set: false }> {
  return call<{ set: false }>("/manage/api/gateway/uplink", { method: "DELETE" });
}

/** DELETE /manage/api/gateway/boxes/<id>: forget a box that enrolled here. */
export function forgetGatewayBox(id: string): Promise<{ forgotten: string }> {
  return call<{ forgotten: string }>(`/manage/api/gateway/boxes/${encodeURIComponent(id)}`, {
    method: "DELETE",
  });
}

/** POST /passport/settings on the edge: the owner's signed settings. The
 *  signature is the proof, so no key goes with it; the gateway reboots to
 *  measure them. */
export function claimGateway(settings: string, signature: string): Promise<{ accepted: true }> {
  return call<{ accepted: true }>("/passport/settings", {
    method: "POST",
    body: JSON.stringify({ settings, signature }),
    contentType: "application/json",
    anonymous: true,
  });
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
  /** The repository's page on this box: /git/<owner>/<name>. */
  url: string;
  /** The URL lososd pushes to (loopback; not for the owner). */
  clone: string;
  viaServer: boolean;
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

/** How the last look at the published branch went (backend/schema.json). */
export type ConfigUpdateState = "off" | "pending" | "ok" | "kept" | "waiting" | "unavailable" | "error";

export interface ConfigUpdateReport {
  state: ConfigUpdateState;
  detail: string;
  syncedAt: string | null;
  /** The published commit the last look fetched. */
  remoteHead: string | null;
}

/** Where /etc/nixos follows the published tree from; both null when it does not. */
export interface ConfigUpstream {
  url: string | null;
  branch: string | null;
  report: ConfigUpdateReport;
}

export interface ConfigResponse {
  enabled: boolean;
  repository: ConfigRepository | null;
  head: ConfigHead | null;
  log: ConfigLogEntry[];
  sync: ConfigSync;
  upstream: ConfigUpstream;
}

/** GET /api/config — the configuration repository and its history. */
export function getConfig(options: RequestOptions = {}): Promise<ConfigResponse> {
  return call<ConfigResponse>("/api/config", options);
}

/** POST /api/config/sync — one sync with LosOS Git now, then the document. */
export function postConfigSync(options: RequestOptions = {}): Promise<ConfigResponse> {
  return call<ConfigResponse>("/api/config/sync", { ...options, method: "POST" });
}

// ── Pins on the sidebar ───────────────────────────────────────────────────

/* What the owner pinned to the sidebar, kept on the box (backend/src/pins.rs)
 * so every browser shows the same list. A pin is a reference, never an
 * address: lib/pins.ts decides where each one opens. */

export type Pin =
  /** An app tile of the Overview, by its id in lib/apps.ts's catalogue. */
  | { kind: "tile"; app: string }
  /** An app installed from the Apps search, by release name. */
  | { kind: "app"; release: string }
  /** A file or folder in LosOS Cloud. */
  | { kind: "file"; id: number; path: string; folder: boolean }
  /** A repository on LosOS Git. */
  | { kind: "repo"; owner: string; name: string }
  /** An issue of LosOS Git's tracker, by its key (`LOS-12`). */
  | { kind: "issue"; key: string }
  /** A project of LosOS Git's tracker, by its key (`LOS`). */
  | { kind: "project"; key: string };

export interface PinsResponse {
  pins: Pin[];
  /** The most the box keeps. */
  limit: number;
}

/** GET /api/pins */
export function getPins(options: RequestOptions = {}): Promise<PinsResponse> {
  return call<PinsResponse>("/api/pins", options);
}

/** PUT /api/pins — the whole list, in order. */
export function putPins(pins: readonly Pin[], options: RequestOptions = {}): Promise<PinsResponse> {
  return call<PinsResponse>("/api/pins", {
    ...options,
    method: "PUT",
    contentType: "application/json",
    body: JSON.stringify({ pins }),
  });
}

export interface CloudEntry {
  name: string;
  /** From the top of LosOS Cloud: "/Documents/plan.pdf". */
  path: string;
  id: number;
  folder: boolean;
  size: number | null;
}

export type CloudFilesResponse =
  | { available: true; path: string; query: string | null; entries: CloudEntry[]; more: boolean }
  /** signIn: the box holds no key to the owner's files until they sign in
   *  with their password once. notUp: LosOS Cloud did not answer. */
  | { available: false; reason: "signIn" | "notUp"; path: string; query: string | null; entries: [] };

/** GET /api/cloud/files — one folder of LosOS Cloud, folders first; with a
 *  query, the files and folders anywhere whose name holds it. */
export function getCloudFiles(
  where: { path: string } | { query: string },
  options: RequestOptions = {},
): Promise<CloudFilesResponse> {
  const params: Record<string, string> = "query" in where ? { q: where.query } : { path: where.path };
  return call<CloudFilesResponse>(`/api/cloud/files?${new URLSearchParams(params).toString()}`, options);
}

export interface GitRepo {
  owner: string;
  name: string;
  private: boolean;
  description: string;
}

export type GitReposResponse =
  | { available: true; repos: GitRepo[] }
  /** noGit: this box runs no LosOS Git. notUp: it did not answer. */
  | { available: false; reason: "noGit" | "notUp"; repos: [] };

/** GET /api/git/repos — the owner's repositories on LosOS Git; with a
 *  query, those whose name or description holds it. */
export function getGitRepos(query: string | null = null, options: RequestOptions = {}): Promise<GitReposResponse> {
  const search = query === null ? "" : `?${new URLSearchParams({ q: query }).toString()}`;
  return call<GitReposResponse>(`/api/git/repos${search}`, options);
}

/* ── LosOS Mail ────────────────────────────────────────────────────────────
 * backend/src/mail.rs. Every call is made as the person LosOS Login signed
 * in; the spare key has no mailbox (409). With sharing off there is no mail
 * on the box and every route is a 404. */

export type MailRole = "inbox" | "drafts" | "sent" | "archive" | "junk" | "trash";

export interface MailFolder {
  id: string;
  name: string;
  role: MailRole | null;
  /** The folder it sits in; null at the top. */
  parentId: string | null;
  total: number;
  unread: number;
}

export interface MailAddress {
  name: string | null;
  email: string;
}

export interface MailSummary {
  id: string;
  threadId: string;
  subject: string;
  from: MailAddress[];
  to: MailAddress[];
  receivedAt: string;
  preview: string;
  seen: boolean;
  flagged: boolean;
  hasAttachment: boolean;
  /** OpenPGP mail: PGP/MIME, or an armoured block in the text. */
  pgp: "encrypted" | "signed" | null;
}

export interface MailAttachment {
  blobId: string;
  name: string;
  type: string;
  size: number;
}

export interface MailMessage extends MailSummary {
  cc: MailAddress[];
  replyTo: MailAddress[];
  mailboxIds: string[];
  text: string;
  hasHtml: boolean;
  attachments: MailAttachment[];
}

export interface MailNeighbour {
  name: string;
  displayName: string;
  address: string;
}

export interface MailHome {
  address: string;
  name: string;
  mailboxes: MailFolder[];
  people: MailNeighbour[];
}

export interface MailPage {
  messages: MailSummary[];
  position: number;
  total: number | null;
}

export interface MailDraft {
  to: string[];
  cc: string[];
  subject: string;
  text: string;
  inReplyTo?: string;
  /** Files the message carries, read by the page (from the computer, from
   *  LosOS Cloud, or a forwarded message's own). */
  attachments?: MailFile[];
  /** An OpenPGP/MIME body made in the browser (src/mail/pgp.ts), sent in
   *  place of `text`. */
  pgp?: MailPgpBody;
}

export type MailPgpBody =
  | { mime: "encrypted"; armored: string }
  | { mime: "signed"; part: string; signature: string; micalg: string };

/** The person's own OpenPGP key (backend/src/mail_pgp.rs). */
export interface MailOwnKey {
  fingerprint: string;
  userIds: string[];
  /** Unix seconds. */
  created: number;
  publicKey: string;
  /** The private key, locked with the person's passphrase. */
  lockedKey: string;
  /** What the box sealed the locked key to, or null without a store. */
  sealed: "tpm2" | "host" | null;
  /** Where the box's Web Key Directory publishes the public key. */
  wkd: string;
}

export type MailKeySource = "box" | "contacts" | "autocrypt" | "wkd" | "keyserver";

export interface MailFoundKey {
  source: MailKeySource;
  fingerprint: string;
  userIds: string[];
  created: number;
  publicKey: string;
}

/** One attached file on its way to lososd. */
export interface MailFile {
  name: string;
  type: string;
  /** The bytes, base64. */
  data: string;
}

/** What lososd takes in all (backend/src/mail.rs MAX_ATTACHMENT_BYTES). */
export const MAIL_ATTACHMENT_BYTES = 20 * 1024 * 1024;

/** A file as POST /api/mail/send carries it. */
export async function mailFile(file: File): Promise<MailFile> {
  const url = await new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result));
    reader.onerror = () => reject(reader.error ?? new Error("could not read the file"));
    reader.readAsDataURL(file);
  });
  return { name: file.name, type: file.type || "application/octet-stream", data: url.slice(url.indexOf(",") + 1) };
}

/** An attachment of a message the person can read, as a File, to forward
 *  or to save into LosOS Cloud. */
export async function fetchMailAttachment(attachment: MailAttachment): Promise<File> {
  const res = await fetch(mailAttachmentUrl(attachment), { credentials: "same-origin" });
  if (!res.ok) throw new ApiError(res.status, res.statusText);
  const blob = await res.blob();
  return new File([blob], attachment.name, { type: attachment.type || blob.type || "application/octet-stream" });
}

export interface MailChange {
  seen?: boolean;
  flagged?: boolean;
  mailbox?: string;
}

/** GET /api/mail — the address, the folders and the people on this box. */
export function getMail(options: RequestOptions = {}): Promise<MailHome> {
  return call<MailHome>("/api/mail", options);
}

/** GET /api/mail/messages — a page of a folder, newest first. */
export function getMailMessages(
  mailbox: string,
  query = "",
  position = 0,
  options: RequestOptions = {},
): Promise<MailPage> {
  const params = new URLSearchParams({ mailbox, position: String(position) });
  if (query.trim() !== "") params.set("q", query.trim());
  return call<MailPage>(`/api/mail/messages?${params.toString()}`, options);
}

/** GET /api/mail/messages/{id} — open a message; it is marked read. */
export function getMailMessage(id: string, options: RequestOptions = {}): Promise<MailMessage> {
  return call<MailMessage>(`/api/mail/messages/${encodeURIComponent(id)}`, options);
}

/** Where the message's HTML is served, for the sandboxed frame. Its
 *  pictures load only with `pictures` (lososd's MAIL_FRAME_CSP_NO_PICTURES
 *  holds them back otherwise). */
export function mailHtmlUrl(id: string, pictures = false): string {
  return apiPath(`/api/mail/messages/${encodeURIComponent(id)}/html${pictures ? "?pictures=1" : ""}`);
}

/** Where an attachment downloads from. */
export function mailAttachmentUrl(attachment: MailAttachment): string {
  return apiPath(
    `/api/mail/attachments/${encodeURIComponent(attachment.blobId)}?name=${encodeURIComponent(attachment.name)}`,
  );
}

/** POST /api/mail/messages/{id} — read or unread, flag, move. */
export function changeMailMessage(id: string, change: MailChange): Promise<{ ok: true }> {
  return call<{ ok: true }>(`/api/mail/messages/${encodeURIComponent(id)}`, {
    method: "POST",
    contentType: "application/json",
    body: JSON.stringify(change),
  });
}

/** DELETE /api/mail/messages/{id} — to the bin, or from the bin for good. */
export function deleteMailMessage(id: string): Promise<{ ok: true }> {
  return call<{ ok: true }>(`/api/mail/messages/${encodeURIComponent(id)}`, { method: "DELETE" });
}

/** GET /api/mail/keys — the person's own key, or null before they set one. */
export function getMailKey(options: RequestOptions = {}): Promise<{ key: MailOwnKey | null }> {
  return call<{ key: MailOwnKey | null }>("/api/mail/keys", options);
}

/** PUT /api/mail/keys — keep a key; the private half must be locked. */
export function putMailKey(publicKey: string, lockedKey: string): Promise<{ key: MailOwnKey }> {
  return call<{ key: MailOwnKey }>("/api/mail/keys", {
    method: "PUT",
    contentType: "application/json",
    body: JSON.stringify({ publicKey, lockedKey }),
  });
}

/** DELETE /api/mail/keys — forget the key and take it out of the WKD. */
export function deleteMailKey(): Promise<{ key: null }> {
  return call<{ key: null }>("/api/mail/keys", { method: "DELETE" });
}

/** GET /api/mail/keys/find — keys for an address, best first. */
export function findMailKeys(email: string, options: RequestOptions = {}): Promise<{ email: string; keys: MailFoundKey[] }> {
  return call<{ email: string; keys: MailFoundKey[] }>(
    `/api/mail/keys/find?${new URLSearchParams({ email }).toString()}`,
    options,
  );
}

/** GET /api/mail/messages/{id}/raw — the whole message, as it arrived. */
export async function getMailRaw(id: string, options: RequestOptions = {}): Promise<Uint8Array> {
  const headers: Record<string, string> = {};
  const bearer = options.token ?? getToken();
  if (bearer !== null) headers["Authorization"] = `Bearer ${bearer}`;
  const init: RequestInit = { headers, cache: "no-store" };
  if (options.signal !== undefined) init.signal = options.signal;
  const response = await fetch(apiPath(`/api/mail/messages/${encodeURIComponent(id)}/raw`), init);
  if (!response.ok) {
    const { message } = await errorBody(response);
    throw new ApiError(response.status, message);
  }
  return new Uint8Array(await response.arrayBuffer());
}

/** POST /api/mail/send. */
export function sendMail(draft: MailDraft): Promise<{ ok: true }> {
  return call<{ ok: true }>("/api/mail/send", {
    method: "POST",
    contentType: "application/json",
    body: JSON.stringify(draft),
  });
}

// ── LosOS Calendar ─────────────────────────────────────────────────────────
// The person's calendars in LosOS Cloud (backend/src/cloud/calendar.rs),
// under /cloud/api/calendar/, the same ones phones sync over CalDAV, plus
// subscriptions to other calendars' addresses and calendars published as
// a feed. Signed in by LosOS Login's cookie, like Contacts.

export interface CalendarFeed {
  url: string;
  /** Seconds between reads. */
  refresh: number;
  /** The last read, and the last good one, in milliseconds since the epoch. */
  fetched?: number;
  ok?: number;
  /** Why the last read failed; empty when it did not. */
  error: string;
}

export interface CalendarInfo {
  uri: string;
  name: string;
  color?: string;
  count: number;
  /** A subscription: its events change only in the feed. */
  readOnly: boolean;
  feed?: CalendarFeed;
  /** `/cloud/feed/<token>.ics` while the calendar is published. */
  published?: string;
}

export interface CalendarPerson {
  email: string;
  name?: string;
  /** NEEDS-ACTION, ACCEPTED, DECLINED, TENTATIVE. */
  status?: string;
  /** Their Matrix ID, for a chat or a call with them. */
  matrix?: string;
}

export type CalendarFreq = "daily" | "weekly" | "monthly" | "yearly";

export interface CalendarRepeat {
  freq: CalendarFreq;
  interval?: number;
  /** The last day, `2026-12-31`. */
  until?: string;
  count?: number;
  /** MO, TU, …, for a weekly repeat. */
  days?: string[];
  /** The zone's offset on the last day, minutes east: UNTIL is UTC. */
  offset?: number;
}

export interface CalendarEvent {
  id: string;
  calendar: string;
  uid: string;
  summary: string;
  location?: string;
  description?: string;
  /** `2026-10-22` (all day) or `2026-10-22T08:00:00`, in `zone`. */
  start: string;
  end: string;
  /** `UTC`, an IANA zone, or none: the reader's own. */
  zone?: string;
  allDay: boolean;
  repeat?: CalendarRepeat;
  /** It repeats, perhaps in a way the editor cannot show. */
  repeats: boolean;
  /** This occurrence's start, for a repeat. */
  occurrence?: string;
  status?: string;
  organizer?: CalendarPerson;
  attendees: CalendarPerson[];
  /** Reminders, minutes before the start. */
  alarms: number[];
  /** `video`, `voice` or none: the event is a call in LosOS Chat. */
  call?: string;
  url?: string;
  readOnly: boolean;
  etag: string;
}

export interface CalendarEventFields {
  calendar: string;
  summary: string;
  location: string;
  description: string;
  start: string;
  end: string;
  zone?: string;
  repeat?: CalendarRepeat;
  organizer?: CalendarPerson;
  attendees: CalendarPerson[];
  alarms: number[];
  call: string;
}

export interface CalendarReport {
  added: number;
  updated: number;
  removed: number;
  skipped: number;
  leftOut: number;
}

export interface CalendarInvitation {
  /** REQUEST, REPLY, CANCEL, PUBLISH. */
  method: string;
  summary: string;
  start: string;
  end: string;
  zone?: string;
  allDay: boolean;
  repeats: boolean;
  location?: string;
  organizer?: CalendarPerson;
  attendees: CalendarPerson[];
  /** The event already in one of the person's calendars. */
  stored?: CalendarEvent;
  /** The person's answer so far. */
  status?: string;
}

export interface CalendarAnswered {
  /** The METHOD:REPLY to mail the organizer. */
  reply: string;
  to: string;
  subject: string;
  event?: CalendarEvent;
}

const CALENDAR = "/cloud/api/calendar";

function calendarCall<T>(path: string, options: CallOptions = {}): Promise<T> {
  return call<T>(`${CALENDAR}${path}`, { ...options, anonymous: true });
}

function json(body: unknown): Pick<CallOptions, "contentType" | "body"> {
  return { contentType: "application/json", body: JSON.stringify(body) };
}

/** GET /cloud/api/calendar/calendars. */
export async function getCalendars(options: RequestOptions = {}): Promise<CalendarInfo[]> {
  return (await calendarCall<{ calendars?: CalendarInfo[] }>("/calendars", options)).calendars ?? [];
}

/** POST /cloud/api/calendar/calendars — a new calendar of the person's own. */
export function createCalendar(fields: { name: string; color?: string }): Promise<CalendarInfo> {
  return calendarCall<CalendarInfo>("/calendars", { method: "POST", ...json(fields) });
}

/** PUT /cloud/api/calendar/calendars/{uri} — name, colour, or how often a
 *  subscription is read. */
export function changeCalendar(uri: string, fields: { name?: string; color?: string; refresh?: number }): Promise<CalendarInfo> {
  return calendarCall<CalendarInfo>(`/calendars/${encodeURIComponent(uri)}`, { method: "PUT", ...json(fields) });
}

/** DELETE /cloud/api/calendar/calendars/{uri}. */
export function deleteCalendar(uri: string): Promise<{ removed: string }> {
  return calendarCall<{ removed: string }>(`/calendars/${encodeURIComponent(uri)}`, { method: "DELETE" });
}

/** POST or DELETE /cloud/api/calendar/calendars/{uri}/publish. */
export function publishCalendar(uri: string, on: boolean): Promise<CalendarInfo> {
  return calendarCall<CalendarInfo>(`/calendars/${encodeURIComponent(uri)}/publish`, { method: on ? "POST" : "DELETE" });
}

/** POST /cloud/api/calendar/calendars/{uri}/refresh — read a subscription now. */
export function refreshCalendar(uri: string): Promise<{ calendar: CalendarInfo; report: CalendarReport }> {
  return calendarCall(`/calendars/${encodeURIComponent(uri)}/refresh`, { method: "POST" });
}

/** POST /cloud/api/calendar/subscriptions — subscribe to a webcal:// or
 *  https address; the box reads it once before keeping it. */
export function subscribeCalendar(sub: {
  url: string;
  name?: string;
  color?: string;
  refresh?: number;
}): Promise<{ calendar: CalendarInfo; report: CalendarReport }> {
  return calendarCall("/subscriptions", { method: "POST", ...json(sub) });
}

/** POST /cloud/api/calendar/import — an .ics file into a calendar, or a new one. */
export function importCalendar(file: Blob, calendar?: string, name?: string): Promise<{ calendar: CalendarInfo; report: CalendarReport }> {
  const q =
    calendar !== undefined && calendar !== ""
      ? `?calendar=${encodeURIComponent(calendar)}`
      : name
        ? `?name=${encodeURIComponent(name)}`
        : "";
  return calendarCall(`/import${q}`, { method: "POST", contentType: "text/calendar", body: file });
}

/** GET /cloud/api/calendar/events — every occurrence in [from, to), days,
 *  give or take a day the page trims. */
export async function getCalendarEvents(from: string, to: string, options: RequestOptions = {}): Promise<CalendarEvent[]> {
  const q = new URLSearchParams({ from, to });
  return (await calendarCall<{ events?: CalendarEvent[] }>(`/events?${q.toString()}`, options)).events ?? [];
}

/** GET /cloud/api/calendar/events/{id} — one event as it repeats. */
export function getCalendarEvent(id: string): Promise<CalendarEvent> {
  return calendarCall<CalendarEvent>(`/events/${encodeURIComponent(id)}`);
}

/** POST or PUT /cloud/api/calendar/events — 412 when a phone changed the
 *  event since it was read. */
export function saveCalendarEvent(id: string | null, fields: CalendarEventFields, etag?: string): Promise<CalendarEvent> {
  return calendarCall<CalendarEvent>(id === null ? "/events" : `/events/${encodeURIComponent(id)}`, {
    method: id === null ? "POST" : "PUT",
    ...json(fields),
    ...(etag !== undefined ? { ifMatch: etag } : {}),
  });
}

/** DELETE /cloud/api/calendar/events/{id}, or one occurrence of a repeat. */
export function deleteCalendarEvent(id: string, occurrence?: string): Promise<{ removed: string }> {
  const q = occurrence !== undefined ? `?occurrence=${encodeURIComponent(occurrence)}` : "";
  return calendarCall(`/events/${encodeURIComponent(id)}${q}`, { method: "DELETE" });
}

/** GET /cloud/api/calendar/events/{id}/invite — the METHOD:REQUEST to mail. */
export async function calendarInvite(id: string): Promise<string> {
  const response = await fetch(`${CALENDAR}/events/${encodeURIComponent(id)}/invite`, { cache: "no-store" });
  if (!response.ok) {
    const { message, ...rest } = await errorBody(response);
    throw new ApiError(response.status, message, rest);
  }
  return response.text();
}

/** POST /cloud/api/calendar/invitations/read — what an .ics in a message asks. */
export function readInvitation(ics: string, email: string): Promise<CalendarInvitation> {
  return calendarCall(`/invitations/read?email=${encodeURIComponent(email)}`, {
    method: "POST",
    contentType: "text/calendar",
    body: ics,
  });
}

/** POST /cloud/api/calendar/invitations/answer — yes, maybe or no: the event
 *  lands in the person's calendar and the reply comes back to mail. */
export function answerInvitation(body: { ics: string; email: string; status: string; calendar?: string }): Promise<CalendarAnswered> {
  return calendarCall("/invitations/answer", { method: "POST", ...json(body) });
}

/** POST /cloud/api/calendar/invitations/apply — a guest's reply or an
 *  organizer's cancellation, onto the person's own copy. */
export function applyInvitation(ics: string): Promise<CalendarEvent> {
  return calendarCall("/invitations/apply", { method: "POST", contentType: "text/calendar", body: ics });
}

/** A text as an attachment for /api/mail/send. */
export function mailTextFile(name: string, type: string, text: string): MailFile {
  const bytes = new TextEncoder().encode(text);
  let binary = "";
  for (const b of bytes) binary += String.fromCharCode(b);
  return { name, type, data: btoa(binary) };
}

// ── LosOS Contacts ─────────────────────────────────────────────────────────
// The book is LosOS Cloud's address books (backend/src/cloud/contacts.rs),
// under /cloud/api/contacts/, the same cards phones sync over CardDAV.
// LosOS Cloud knows the caller from LosOS Login's cookie, so these calls
// carry no admin token, and a 401 from it leaves the stored one alone.
// The pickers are lososd's (backend/src/contacts.rs).

export interface ContactLabelled {
  value: string;
  label: string;
}

export interface ContactAddress {
  street: string;
  locality: string;
  region: string;
  code: string;
  country: string;
  label: string;
}

export interface ContactName {
  family: string;
  given: string;
  additional: string;
  prefix: string;
  suffix: string;
}

/** A contact's fields, as the page sends them to be saved. */
export interface ContactFields {
  name: string;
  n?: ContactName;
  nickname?: string;
  org?: string;
  title?: string;
  emails?: ContactLabelled[];
  phones?: ContactLabelled[];
  addresses?: ContactAddress[];
  urls?: ContactLabelled[];
  /** `YYYY-MM-DD`, or `--MM-DD` without the year. */
  birthday?: string;
  note?: string;
  /** A Matrix user ID, `@name:server`. */
  matrix?: string;
  /** A mesh address, `name@box`, for sharing files. */
  mesh?: string;
  uid?: string;
}

/** A contact as the book keeps it. */
export interface Contact extends ContactFields {
  id: string;
  /** The address book it is in, by name. */
  book?: string;
  /** Where the photo loads from, versioned; absent without one. */
  photo?: string;
  updated: number;
}

/** One person a picker offers: a contact, or someone on this box. */
export interface ContactSuggestion {
  key: string;
  source: "contact" | "box";
  name: string;
  picture?: string;
  emails: string[];
  matrixId?: string;
  meshAddress?: string;
  contactId?: string;
}

export interface ContactsImport {
  added: number;
  updated: number;
  skipped: number;
  photosDropped: number;
  leftOut: number;
}

const CONTACTS = "/cloud/api/contacts";

function contactsCall<T>(path: string, options: CallOptions = {}): Promise<T> {
  return call<T>(`${CONTACTS}${path}`, { ...options, anonymous: true });
}

/** GET /cloud/api/contacts — the caller's contacts in every address book, by name. */
export async function getContacts(options: RequestOptions = {}): Promise<{ contacts: Contact[] }> {
  const answer = await contactsCall<{ contacts?: Contact[] }>("", options);
  return { contacts: answer.contacts ?? [] };
}

/** POST /cloud/api/contacts or PUT /cloud/api/contacts/{id} — save a
 *  contact's fields. 412 when a phone changed the card since it was read. */
export function saveContact(id: string | null, fields: ContactFields): Promise<Contact> {
  return contactsCall<Contact>(id === null ? "" : `/${encodeURIComponent(id)}`, {
    method: id === null ? "POST" : "PUT",
    contentType: "application/json",
    body: JSON.stringify(fields),
  });
}

/** DELETE /cloud/api/contacts/{id}. */
export function deleteContact(id: string): Promise<{ removed: string }> {
  return contactsCall<{ removed: string }>(`/${encodeURIComponent(id)}`, { method: "DELETE" });
}

/** PUT /cloud/api/contacts/{id}/photo — a picture, scaled and squared by the box. */
export function setContactPhoto(id: string, picture: Blob): Promise<Contact> {
  return contactsCall<Contact>(`/${encodeURIComponent(id)}/photo`, {
    method: "PUT",
    contentType: picture.type || "application/octet-stream",
    body: picture,
  });
}

/** DELETE /cloud/api/contacts/{id}/photo. */
export function removeContactPhoto(id: string): Promise<Contact> {
  return contactsCall<Contact>(`/${encodeURIComponent(id)}/photo`, { method: "DELETE" });
}

/** POST /cloud/api/contacts/import — a .vcf file, one card or many. */
export function importContacts(file: Blob): Promise<ContactsImport> {
  return contactsCall<ContactsImport>("/import", {
    method: "POST",
    contentType: "text/vcard",
    body: file,
  });
}

/** Where the book, or some contacts of it, downloads as a .vcf file. */
export function contactsExportUrl(version: "3.0" | "4.0", ids?: readonly string[]): string {
  const params = new URLSearchParams({ version });
  if (ids !== undefined && ids.length > 0) params.set("ids", ids.join(","));
  return `${CONTACTS}/export?${params.toString()}`;
}

/** GET /api/contacts/suggest — the people a picker offers for `q`: the
 *  caller's contacts and the people on this box. Empty for the spare key. */
export async function suggestContacts(
  q: string,
  want: "any" | "email" | "matrix" | "mesh" | "box" = "any",
  options: RequestOptions & { limit?: number } = {},
): Promise<ContactSuggestion[]> {
  const params = new URLSearchParams({ q, want, limit: String(options.limit ?? 8) });
  try {
    const answer = await call<{ contacts?: ContactSuggestion[] }>(`/api/contacts/suggest?${params.toString()}`, options);
    return answer.contacts ?? [];
  } catch (error) {
    if (error instanceof ApiError && (error.status === 404 || error.status === 403)) return [];
    throw error;
  }
}

// ── The box's AI on what the person shows it (backend/plugins/assistant) ──

export interface AssistantStatus {
  /** The box runs a language model at all. */
  available: boolean;
  /** Its model is pulled and answering. */
  ready?: boolean;
  model?: string;
}

/** Something the person chose to show the model: a file, a note, a thread. */
export interface AssistantSource {
  title: string;
  text: string;
}

export interface AssistantAsk {
  id: string;
  state: "queued" | "running" | "done" | "failed" | "stopped";
  answer: string;
  /** Some of the material did not fit and was cut. */
  cut: boolean;
  error?: string;
}

/** GET /api/assistant. */
export function getAssistant(options: RequestOptions = {}): Promise<AssistantStatus> {
  return call<AssistantStatus>("/api/assistant", options);
}

/** POST /api/assistant/asks. A 429 means the model is busy with another. */
export function askAssistant(
  ask: { question: string; sources: AssistantSource[]; lang: string },
  options: RequestOptions = {},
): Promise<AssistantAsk> {
  return call<AssistantAsk>("/api/assistant/asks", {
    ...options,
    method: "POST",
    contentType: "application/json",
    body: JSON.stringify(ask),
  });
}

/** GET /api/assistant/asks/{id}. */
export function getAssistantAsk(id: string, options: RequestOptions = {}): Promise<AssistantAsk> {
  return call<AssistantAsk>(`/api/assistant/asks/${encodeURIComponent(id)}`, options);
}

/** DELETE /api/assistant/asks/{id}: stop it. */
export function stopAssistantAsk(id: string): Promise<AssistantAsk> {
  return call<AssistantAsk>(`/api/assistant/asks/${encodeURIComponent(id)}`, { method: "DELETE" });
}

/* A person's own folders and filters: the mail plugin
 * (backend/plugins/mail). LosOS Mail's own folders, the ones with a role,
 * keep their names and places. */

/** POST /api/mail/folders — a folder of one's own, at the top or inside one. */
export function createMailFolder(name: string, parentId: string | null): Promise<{ id: string }> {
  return call<{ id: string }>("/api/mail/folders", {
    method: "POST",
    contentType: "application/json",
    body: JSON.stringify({ name, parentId }),
  });
}

/** PUT /api/mail/folders/{id} — rename it, or move it (`parentId: null` is the top). */
export function updateMailFolder(
  id: string,
  change: { name?: string; parentId?: string | null },
): Promise<{ ok: true }> {
  return call<{ ok: true }>(`/api/mail/folders/${encodeURIComponent(id)}`, {
    method: "PUT",
    contentType: "application/json",
    body: JSON.stringify(change),
  });
}

/** DELETE /api/mail/folders/{id} — its mail goes to the bin first. */
export function deleteMailFolder(id: string): Promise<{ moved: number }> {
  return call<{ moved: number }>(`/api/mail/folders/${encodeURIComponent(id)}`, { method: "DELETE" });
}

export type MailFilterField = "from" | "to" | "subject";

export interface MailFilter {
  name: string;
  enabled: boolean;
  /** Every condition must hold, or one is enough. */
  match: "all" | "any";
  conditions: { field: MailFilterField; value: string }[];
  /** The folder it files into, the bin included; null leaves it in the inbox. */
  moveTo: string | null;
  markRead: boolean;
  flag: boolean;
}

/** GET /api/mail/filters — in the order they run. */
export function getMailFilters(options: RequestOptions = {}): Promise<{ filters: MailFilter[] }> {
  return call<{ filters: MailFilter[] }>("/api/mail/filters", options);
}

/** PUT /api/mail/filters — the whole list, which the mail server runs as mail arrives. */
export function putMailFilters(filters: MailFilter[]): Promise<{ filters: MailFilter[] }> {
  return call<{ filters: MailFilter[] }>("/api/mail/filters", {
    method: "PUT",
    contentType: "application/json",
    body: JSON.stringify({ filters }),
  });
}

/* ── Pictures in Chat and Mail (backend/src/pictures.rs) ──────────────────
 * Which pictures arrive blurred, whom the person trusts, the nudity warning
 * and the adult or child mark. The box never looks at a picture: the blur
 * and the warning are drawn in this page. */

/** Which pictures arrive blurred: every one but those from trusted
 *  senders, only those from strangers, or none (adults only). */
export type PictureBlur = "everyone" | "strangers" | "nobody";

export interface AgeMark {
  age: "adult" | "child";
  /** A finished Binance purchase, or an administrator in People. */
  by: "binance" | "admin";
  /** Seconds since 1970. */
  since: number;
}

export interface PictureSettings {
  blur: PictureBlur;
  /** Chat IDs and mail addresses, in lower case. */
  trusted: string[];
  nudityCheck: boolean;
  /** Private details blacked out and asked about (components/details.tsx). */
  detailsCheck: boolean;
  age: AgeMark | null;
  /** Marked as a child: the warning stays on and `nobody` is refused. */
  child: boolean;
  /** Marked as an adult: `nobody` may be chosen. */
  mayShowAll: boolean;
}

export interface PictureSender {
  shown: boolean;
  because: "trusted" | "known" | "setting" | null;
  nudityCheck: boolean;
}

/** GET /api/pictures — the caller's own settings. */
export function getPictures(options: RequestOptions = {}): Promise<PictureSettings> {
  return call<PictureSettings>("/api/pictures", options);
}

/** POST /api/pictures — 403 with code `adultRequired` or `childAccount`
 *  when the mark does not allow the change. */
export function postPictures(
  change: { blur?: PictureBlur; nudityCheck?: boolean; detailsCheck?: boolean },
  options: RequestOptions = {},
): Promise<PictureSettings> {
  return postJson("/api/pictures", change, options);
}

/** POST /api/pictures/trust — always show (or stop showing) one sender's
 *  pictures at once. */
export function postPictureTrust(id: string, trusted: boolean, options: RequestOptions = {}): Promise<PictureSettings> {
  return postJson("/api/pictures/trust", { id, trusted }, options);
}

/** GET /api/pictures/sender?id= — whether this chat ID or address's
 *  pictures show at once. */
export function getPictureSender(id: string, options: RequestOptions = {}): Promise<PictureSender> {
  return call<PictureSender>(`/api/pictures/sender?id=${encodeURIComponent(id)}`, options);
}

/** POST /api/people/age — an administrator's mark. */
export function postPersonAge(
  name: string,
  age: AgeMark["age"] | null,
  options: RequestOptions = {},
): Promise<{ name: string; age: AgeMark | null }> {
  return postJson("/api/people/age", { name, age }, options);
}

/* ── Reports to the box's moderators ──────────────────────────────────────
 * `POST /api/reports` belongs to moderation (Settings › Moderation). It
 * carries a reference to what is reported, never the thing itself. A box
 * without moderation answers 404. */

export interface ReportTarget {
  app: ReportApp;
  /** The sign-in name of the person reported, when they are on this box. */
  about: string | null;
  /** A chat event ID, a mail message's ID, a path. */
  ref: string;
}

/** POST /api/reports — `{"id"}`. */
export function postReport(
  report: ReportTarget & { reason: ReportReason; note: string },
  options: RequestOptions = {},
): Promise<{ id: string }> {
  return postJson("/api/reports", report, options);
}

// ── Mesh CI (backend/src/ci/, through lososd's ci plugin) ───────────────

export type CiState = "queued" | "running" | "success" | "failure" | "cancelled" | "skipped" | "error";
export type CiLend = "always" | "window" | "never";

/** GET /api/ci — this box's runner and whether the mesh's queue answers. */
export interface CiOverview {
  box: string;
  lend: CiLend;
  lendingNow: boolean;
  window: { start: string; end: string } | null;
  mesh: boolean;
  privateOnMesh: boolean;
  canChange: boolean;
}

export interface CiJobSummary {
  name: string;
  state: CiState;
  runner: string;
}

export interface CiRun {
  id: number;
  owner: string;
  repo: string;
  private: boolean;
  workflowFile: string;
  workflowName: string;
  number: number;
  event: string;
  sha: string;
  gitRef: string;
  title: string;
  actor: string;
  state: CiState;
  message: string;
  created: number;
  started: number | null;
  finished: number | null;
}

export interface CiJob {
  id: number;
  runId: number;
  jobKey: string;
  name: string;
  matrix: Record<string, unknown>;
  state: CiState;
  /** `box`: this box's runner. `mesh`: any box of the mesh. */
  placement: "box" | "mesh";
  runner: string;
  exitCode: number | null;
  outputs: Record<string, unknown>;
  continueOnError: boolean;
  logLen: number;
  created: number;
  started: number | null;
  finished: number | null;
}

export interface CiRunDetail extends CiRun {
  jobs: CiJob[];
  canChange: boolean;
}

export interface CiLog {
  state: CiState;
  from: number;
  next: number;
  text: string;
}

export interface CiWorkflows {
  ref: string | null;
  sha: string | null;
  canRun: boolean;
  workflows: { file: string; name: string }[];
}

export function getCi(options: RequestOptions = {}): Promise<CiOverview> {
  return call<CiOverview>("/api/ci", options);
}

/** PUT /api/ci/runner — when this box lends its runner. Administrators. */
export function setCiLend(lend: CiLend): Promise<{ lend: CiLend }> {
  return call<{ lend: CiLend }>("/api/ci/runner", {
    method: "PUT",
    contentType: "application/json",
    body: JSON.stringify({ lend }),
  });
}

/** GET /api/ci/runs — newest first, only repositories this person reads. */
export function getCiRuns(
  filter: { owner?: string; repo?: string; before?: number } = {},
  options: RequestOptions = {},
): Promise<{ runs: (CiRun & { jobs: CiJobSummary[] })[] }> {
  const params = new URLSearchParams();
  if (filter.owner !== undefined && filter.repo !== undefined) {
    params.set("owner", filter.owner);
    params.set("repo", filter.repo);
  }
  if (filter.before !== undefined) params.set("before", String(filter.before));
  const query = params.toString();
  return call(`/api/ci/runs${query === "" ? "" : `?${query}`}`, options);
}

export function getCiRun(id: number, options: RequestOptions = {}): Promise<CiRunDetail> {
  return call<CiRunDetail>(`/api/ci/runs/${id}`, options);
}

/** GET /api/ci/jobs/{id}/log — the log from byte `from` on. */
export function getCiLog(job: number, from = 0, options: RequestOptions = {}): Promise<CiLog> {
  return call<CiLog>(`/api/ci/jobs/${job}/log?from=${from}`, options);
}

export function cancelCiRun(id: number): Promise<{ id: number }> {
  return call<{ id: number }>(`/api/ci/runs/${id}/cancel`, { method: "POST" });
}

/** POST /api/ci/runs/{id}/rerun — a new run of the same commit. */
export function rerunCiRun(id: number): Promise<{ id: number }> {
  return call<{ id: number }>(`/api/ci/runs/${id}/rerun`, { method: "POST" });
}

/** GET /api/ci/workflows — the workflows of a branch that take workflow_dispatch. */
export function getCiWorkflows(
  owner: string,
  repo: string,
  ref = "",
  options: RequestOptions = {},
): Promise<CiWorkflows> {
  const params = new URLSearchParams({ owner, repo });
  if (ref !== "") params.set("ref", ref);
  return call<CiWorkflows>(`/api/ci/workflows?${params.toString()}`, options);
}

/** POST /api/ci/dispatch — run a workflow by hand. */
export function dispatchCi(owner: string, repo: string, workflow: string, ref = ""): Promise<{ id: number }> {
  return call<{ id: number }>("/api/ci/dispatch", {
    method: "POST",
    contentType: "application/json",
    body: JSON.stringify({ owner, repo, workflow, ref }),
  });
}
