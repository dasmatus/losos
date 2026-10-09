/* LosOS Lab's core: the world model, the packet simulator, the clock, the
 * simulated consoles and pages, compiled from admin-ui/lab/core (Rust) to
 * WebAssembly. `npm run lab:core` writes the wasm-bindgen package into
 * ./core-pkg (gitignored).
 *
 * The contract is admin-ui/lab/core/README.md: strings in, JSON text out, a
 * refused call answers {"error": "…"} and changes nothing. This file is the
 * typed side of that contract, and the store (store.ts) decides when to call
 * what.
 *
 * The page starts on this small module and the SVG canvas, and may later
 * swap to the GPU canvas (render.ts), whose module carries a `Lab` of its
 * own. The state moves across by replay: every call that can
 * change the Lab is kept (`journal`), and `adopt` makes the other module's
 * `Lab` with the same seed, clock and zone and plays the calls into it. The
 * core is deterministic (its PRNG is seeded, its clock moves only by tick),
 * so the result is the same state, ids, consoles and all; `adopt` checks
 * that the two snapshots agree before it switches. */

import init, { Lab } from "./core-pkg/losos_lab_core.js";
import wasmUrl from "./core-pkg/losos_lab_core_bg.wasm?url";

export type Err = { error: string };

export type PortKind = "eth" | "wifi" | "wifi-ap";
export type Category = "losos" | "net" | "end";

export interface TypeInfo {
  label: string;
  short: string;
  cat: Category;
  ports: [string, PortKind][];
  endpoint?: true;
  emulate?: string;
  blurb: string;
}

export type TimerKey = "reboot" | "upgrade" | "gc" | "windowEnd" | "windowStart";
export interface Timer {
  at: string;
  key: TimerKey;
  title: string;
}

export interface Catalog {
  types: Record<string, TypeInfo>;
  sites: Record<string, { name: string; sub: string }>;
  linkKinds: Record<string, { label: string; hint: string }>;
  proto: Record<string, { color: string; label: string }>;
  speeds: number[];
  daily: Timer[];
  defaultCfg: Record<string, Cfg>;
  defaultName: Record<string, string>;
  defaultSite: Record<string, string>;
  scenarios: { key: string; name: string; blurb: string }[];
}

export type Cfg = Record<string, boolean | string>;

export interface Device {
  id: string;
  type: string;
  name: string;
  x: number;
  y: number;
  site: string;
  px: number | null;
  py: number | null;
  power: boolean;
  cfg: Cfg;
  mac: string;
  rebooting?: boolean;
  gen?: number;
  computeOpen?: boolean;
}

export type LinkKind = "copper" | "fiber" | "wan" | "wifi";
export interface Link {
  id: string;
  a: { dev: string; port: string };
  b: { dev: string; port: string };
  kind: LinkKind;
}

export interface Addr {
  ip: string;
  mask: number;
  gw?: string;
  src: "public" | "router" | "dhcp" | "link-local";
  router?: string;
}

export interface Edge {
  id: string;
  source: "lan" | "configured";
  official: boolean;
  url: string;
}

export interface BoxState {
  up: boolean;
  edges: Edge[];
  path: Edge | null;
  tunnel: "off" | "enrolled" | "registered" | "refused" | "stopped";
  publicName: string | null;
  mesh: string;
  market: string;
  reason: string;
  meshEdge?: string;
}

export interface SpokeState {
  up: boolean;
  uplink: string;
  relayed: string[];
  enrolled: string[];
  cluster: string[];
  hub?: string;
}

export interface HubState {
  up: boolean;
  tenants: string[];
  relays: string[];
  cluster: string[];
}

export interface Seg {
  id: string;
  members: string[];
  routers: string[];
  internet: boolean;
  kind: "internet" | "lan" | "isolated";
  cloud?: string;
  router?: string;
}

export interface Net {
  segs: Record<string, Seg>;
  iface: Record<string, { port: string; seg: string }>;
  addr: Record<string, Addr>;
  internet: Record<string, boolean>;
  subnet: Record<string, string>;
  gearSeg: Record<string, string>;
  mgmt: Record<string, { ip: string; mask: number; gw?: string }>;
  losos: {
    box: Record<string, BoxState>;
    spoke: Record<string, SpokeState>;
    hub: Record<string, HubState>;
  };
}

export interface Pdu {
  proto: string;
  from: string;
  to: string;
  info: string;
  local?: true;
  hops?: string[];
  real?: true;
}

export interface LabEvent {
  proto: string;
  last: string;
  at: string;
  info: string;
  local?: true;
  fail?: true;
  real?: true;
  final?: boolean;
  t: string;
}

export interface Sim {
  mode: "realtime" | "simulation";
  playing: boolean;
  tick: number;
  speed: number;
  log: LabEvent[];
  flows: { id: number; stages: Pdu[][]; stage: number; title: string }[];
  active: unknown[];
  filters: Record<string, boolean>;
  frameFrac: number;
  seq: number;
  cursor: number;
}

export interface ClockInfo {
  t: number;
  speed: number;
  text: string;
  next: Timer;
}

export interface Snapshot {
  world: { devices: Device[]; links: Link[]; seq: number };
  net: Net;
  sim: Sim;
  clock: ClockInfo & { nextScan: number; nextBeat: number };
  scenario: string;
  fileName: string;
  dirty: boolean;
}

export interface Packet {
  flow: number;
  proto: string;
  color: string;
  info: string;
  from: string;
  to: string;
  i: number;
  t: number;
  x: number;
  y: number;
  real?: true;
}

export type HttpResult =
  | { status: number; title: string; text?: string; html: string; error?: "refused" }
  | { error: "mdns" | "nodns" | "nxdomain" | "timeout" | "badurl"; host?: string };

export interface TickResult {
  packets: Packet[];
  events: LabEvent[];
  reset: boolean;
  console: { job: number; dev: string; lines: string[]; done: boolean }[];
  http: { job: number; dev: string; url: string; result: HttpResult }[];
  toasts: string[];
  worldChanged: boolean;
  sim: {
    mode: "realtime" | "simulation";
    playing: boolean;
    tick: number;
    frameFrac: number;
    inFlight: number;
    status: string;
  };
  clock: ClockInfo;
  message?: string;
}

export interface ExecResult {
  job: number;
  lines: string[];
  busy: boolean;
  clear: boolean;
}

export interface HttpStart {
  job: number;
  busy: boolean;
  result?: HttpResult;
}

export interface ConsoleInfo {
  prompt: string;
  boot: string[];
  completions: string[];
}

export function isErr(value: unknown): value is Err {
  return typeof value === "object" && value !== null && typeof (value as Err).error === "string" && !("status" in value);
}

/** The core answered something that is not JSON: a contract break, said loudly. */
export class CoreError extends Error {}

function parse<T>(method: string, text: string | undefined): T {
  if (text === undefined) throw new CoreError(`${method}() returned nothing`);
  try {
    return JSON.parse(text) as T;
  } catch {
    throw new CoreError(`${method}() returned text that is not JSON: ${text.slice(0, 120)}`);
  }
}

/** A `Lab` from either module: core-pkg's, or the GPU canvas's, which
 *  exports the same class. A mapped type, so the two are interchangeable. */
export type LabApi = { [K in keyof Lab]: Lab[K] };
type LabMethod = { [K in keyof LabApi]: LabApi[K] extends (...a: never[]) => unknown ? K : never }[keyof LabApi];

/** How many calls the journal keeps before it gives up (about 25 minutes of
 *  frames); past that the page stays on the canvas it has. */
const JOURNAL_CAP = 100_000;

/* Typed calls. Each one is the README's signature with the JSON parsed. */
export class Core {
  /** Every call that can change the Lab, in order, until `adopt` or the cap. */
  private journal: [LabMethod, unknown[]][] | null = [];

  private constructor(
    private lab: LabApi,
    /** The constructor's arguments: seed, clock start, zone. */
    private readonly born: [number, number, number],
  ) {}

  static async load(): Promise<Core> {
    await init({ module_or_path: wasmUrl });
    const born: [number, number, number] = [(Math.random() * 2 ** 31) | 0, Date.now(), -new Date().getTimezoneOffset()];
    const lab = new Lab(...born);
    // A stub build (signatures only) answers "null" everywhere.
    if (lab.snapshot() === "null") throw new CoreError("this build of the Lab core is a stub: every call answers null");
    return new Core(lab, born);
  }

  /** A call that changes the Lab: kept, then made. */
  private m<K extends LabMethod>(name: K, ...args: Parameters<LabApi[K]>): ReturnType<LabApi[K]> {
    if (this.journal) {
      if (this.journal.length < JOURNAL_CAP) this.journal.push([name, args]);
      else this.journal = null;
    }
    return (this.lab[name] as (...a: unknown[]) => ReturnType<LabApi[K]>).apply(this.lab, args);
  }

  /** Moves this core onto a `Lab` that `make` builds (another module's) by
   *  replaying the journal into it. Returns that `Lab` once its snapshot
   *  equals this one's, and drives it from then on; returns null and changes
   *  nothing when the journal was given up or the two disagree. */
  adopt(make: (seed: number, nowMs: number, tzOffsetMin: number) => LabApi): LabApi | null {
    if (!this.journal) return null;
    const next = make(...this.born);
    for (const [name, args] of this.journal) (next[name] as (...a: unknown[]) => unknown).apply(next, args);
    if (next.snapshot() !== this.lab.snapshot()) {
      next.free();
      return null;
    }
    this.lab.free();
    this.lab = next;
    this.journal = null;
    return next;
  }

  catalog = (): Catalog => parse("catalog", this.lab.catalog());
  setPlate = (url: string): void => this.m("set_plate", url);

  loadScenario = (key: string) => parse<{ focus: string | null } | Err>("load_scenario", this.m("load_scenario", key));
  loadThisBox = (settings: string, edge: string) =>
    parse<{ focus: string | null; name: string; blurb: string } | Err>("load_this_box", this.m("load_this_box", settings, edge));
  exportSetup = () => parse<{ name: string; filename: string; text: string }>("export_setup", this.lab.export_setup());
  importSetup = (text: string, fileName: string) =>
    parse<{ skipped: number; name: string; focus?: string | null } | Err>("import_setup", this.m("import_setup", text, fileName));
  openLast = (text: string) =>
    parse<{ skipped: number; name: string; focus?: string | null } | (Err & { fallback?: string })>("open_last", this.m("open_last", text));
  lastSetup = (): string | undefined => this.m("last_setup");
  peekSetup = (text: string) => parse<{ name: string } | Err>("peek_setup", this.lab.peek_setup(text));

  addDevice = (type: string, x: number, y: number, opts: { site?: string; px?: number; py?: number }) =>
    parse<{ device: Device } | Err>("add_device", this.m("add_device", type, x, y, JSON.stringify(opts)));
  connect = (a: string, b: string, kind: string, aPort?: string, bPort?: string) =>
    parse<{ link: Link } | Err>("connect", this.m("connect", a, b, kind, aPort ?? null, bPort ?? null));
  removeDevice = (id: string) => parse<{ ok: true } | Err>("remove_device", this.m("remove_device", id));
  removeLink = (id: string) => parse<{ ok: true } | Err>("remove_link", this.m("remove_link", id));
  setPower = (id: string, on: boolean) => parse<{ power: boolean; banner: string[] } | Err>("set_power", this.m("set_power", id, on));
  setCfg = (id: string, key: string, value: boolean | string) =>
    parse<{ cfg: Cfg } | Err>("set_cfg", this.m("set_cfg", id, key, JSON.stringify(value)));
  setName = (id: string, name: string) => parse<{ name: string } | Err>("set_name", this.m("set_name", id, name));
  setSite = (id: string, site: string) => parse<{ ok: true } | Err>("set_site", this.m("set_site", id, site));
  moveDevice = (id: string, pos: { x?: number; y?: number; px?: number; py?: number; site?: string }) =>
    parse<{ ok: true } | Err>("move_device", this.m("move_device", id, JSON.stringify(pos)));
  physPos = (id: string) => parse<{ px: number; py: number }>("phys_pos", this.m("phys_pos", id));

  snapshot = (): Snapshot => parse("snapshot", this.lab.snapshot());
  suggestUrls = (id: string): string[] => parse("suggest_urls", this.lab.suggest_urls(id));

  tick = (elapsedMs: number): TickResult => parse("tick", this.m("tick", elapsedMs));
  step = (): TickResult => parse("step", this.m("step"));
  setMode = (mode: "realtime" | "simulation") => parse<{ ok: true } | Err>("set_mode", this.m("set_mode", mode));
  setPlaying = (on: boolean): void => this.m("set_playing", on);
  setSimSpeed = (speed: number): void => this.m("set_sim_speed", speed);
  setClockSpeed = (speed: number) => parse<{ ok: true } | Err>("set_clock_speed", this.m("set_clock_speed", speed));
  skipToNextTimer = () => parse<{ ok: true; next: Timer } | Err>("skip_to_next_timer", this.m("skip_to_next_timer"));
  setFilter = (proto: string, on: boolean) => parse<unknown>("set_filter", this.m("set_filter", proto, on));
  clear = (): void => this.m("clear");
  scan = (id: string) => parse<unknown>("scan", this.m("scan", id));
  startFlow = (stages: Pdu[][], title: string) => parse<unknown>("start_flow", this.m("start_flow", JSON.stringify(stages), title));

  consoleInfo = (id: string) => parse<ConsoleInfo | Err>("console_info", this.lab.console_info(id));
  exec = (id: string, line: string) => parse<ExecResult | Err>("exec", this.m("exec", id, line));
  http = (id: string, url: string) => parse<HttpStart | Err>("http", this.m("http", id, url));
  errorPage = (error: string, host: string): string => this.lab.error_page(error, host);
}
