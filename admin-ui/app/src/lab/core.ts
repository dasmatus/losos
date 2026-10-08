/* LosOS Lab's core: the world model, the packet simulator, the clock, the
 * simulated consoles and pages, compiled from admin-ui/lab/core (Rust) to
 * WebAssembly. `npm run lab:core` writes the wasm-bindgen package into
 * ./core-pkg (gitignored).
 *
 * The contract is admin-ui/lab/core/README.md: strings in, JSON text out, a
 * refused call answers {"error": "…"} and changes nothing. This file is the
 * typed side of that contract and nothing more; the store (store.ts) decides
 * when to call what. */

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

/* Typed calls. Each one is the README's signature with the JSON parsed. */
export class Core {
  private constructor(private readonly lab: Lab) {}

  static async load(): Promise<Core> {
    await init({ module_or_path: wasmUrl });
    const lab = new Lab((Math.random() * 2 ** 31) | 0, Date.now(), -new Date().getTimezoneOffset());
    // A stub build (signatures only) answers "null" everywhere.
    if (lab.snapshot() === "null") throw new CoreError("this build of the Lab core is a stub: every call answers null");
    return new Core(lab);
  }

  catalog = (): Catalog => parse("catalog", this.lab.catalog());
  setPlate = (url: string): void => this.lab.set_plate(url);

  loadScenario = (key: string) => parse<{ focus: string | null } | Err>("load_scenario", this.lab.load_scenario(key));
  loadThisBox = (settings: string, edge: string) =>
    parse<{ focus: string | null; name: string; blurb: string } | Err>("load_this_box", this.lab.load_this_box(settings, edge));
  exportSetup = () => parse<{ name: string; filename: string; text: string }>("export_setup", this.lab.export_setup());
  importSetup = (text: string, fileName: string) =>
    parse<{ skipped: number; name: string; focus?: string | null } | Err>("import_setup", this.lab.import_setup(text, fileName));
  openLast = (text: string) =>
    parse<{ skipped: number; name: string; focus?: string | null } | (Err & { fallback?: string })>("open_last", this.lab.open_last(text));
  lastSetup = (): string | undefined => this.lab.last_setup();
  peekSetup = (text: string) => parse<{ name: string } | Err>("peek_setup", this.lab.peek_setup(text));

  addDevice = (type: string, x: number, y: number, opts: { site?: string; px?: number; py?: number }) =>
    parse<{ device: Device } | Err>("add_device", this.lab.add_device(type, x, y, JSON.stringify(opts)));
  connect = (a: string, b: string, kind: string, aPort?: string, bPort?: string) =>
    parse<{ link: Link } | Err>("connect", this.lab.connect(a, b, kind, aPort ?? null, bPort ?? null));
  removeDevice = (id: string) => parse<{ ok: true } | Err>("remove_device", this.lab.remove_device(id));
  removeLink = (id: string) => parse<{ ok: true } | Err>("remove_link", this.lab.remove_link(id));
  setPower = (id: string, on: boolean) => parse<{ power: boolean; banner: string[] } | Err>("set_power", this.lab.set_power(id, on));
  setCfg = (id: string, key: string, value: boolean | string) =>
    parse<{ cfg: Cfg } | Err>("set_cfg", this.lab.set_cfg(id, key, JSON.stringify(value)));
  setName = (id: string, name: string) => parse<{ name: string } | Err>("set_name", this.lab.set_name(id, name));
  setSite = (id: string, site: string) => parse<{ ok: true } | Err>("set_site", this.lab.set_site(id, site));
  moveDevice = (id: string, pos: { x?: number; y?: number; px?: number; py?: number; site?: string }) =>
    parse<{ ok: true } | Err>("move_device", this.lab.move_device(id, JSON.stringify(pos)));
  physPos = (id: string) => parse<{ px: number; py: number }>("phys_pos", this.lab.phys_pos(id));

  snapshot = (): Snapshot => parse("snapshot", this.lab.snapshot());
  suggestUrls = (id: string): string[] => parse("suggest_urls", this.lab.suggest_urls(id));

  tick = (elapsedMs: number): TickResult => parse("tick", this.lab.tick(elapsedMs));
  step = (): TickResult => parse("step", this.lab.step());
  setMode = (mode: "realtime" | "simulation") => parse<{ ok: true } | Err>("set_mode", this.lab.set_mode(mode));
  setPlaying = (on: boolean): void => this.lab.set_playing(on);
  setSimSpeed = (speed: number): void => this.lab.set_sim_speed(speed);
  setClockSpeed = (speed: number) => parse<{ ok: true } | Err>("set_clock_speed", this.lab.set_clock_speed(speed));
  skipToNextTimer = () => parse<{ ok: true; next: Timer } | Err>("skip_to_next_timer", this.lab.skip_to_next_timer());
  setFilter = (proto: string, on: boolean) => parse<unknown>("set_filter", this.lab.set_filter(proto, on));
  clear = (): void => this.lab.clear();
  scan = (id: string) => parse<unknown>("scan", this.lab.scan(id));
  startFlow = (stages: Pdu[][], title: string) => parse<unknown>("start_flow", this.lab.start_flow(JSON.stringify(stages), title));

  consoleInfo = (id: string) => parse<ConsoleInfo | Err>("console_info", this.lab.console_info(id));
  exec = (id: string, line: string) => parse<ExecResult | Err>("exec", this.lab.exec(id, line));
  http = (id: string, url: string) => parse<HttpStart | Err>("http", this.lab.http(id, url));
  errorPage = (error: string, host: string): string => this.lab.error_page(error, host);
}
