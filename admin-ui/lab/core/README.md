# losos-lab-core

LosOS Lab's non-DOM half in Rust, compiled to WebAssembly: the world model
and its evaluation (model.js), the packet simulator and lab clock (sim.js),
the built-in setups (scenarios.js), "This box" (thisbox.js), setup files
(files.js), the simulated web pages (pages.js) and the simulated console's
command interpreter (term.js). Rendering, xterm, key handling, `fetch`,
`localStorage` and the qemu-wasm engine stay in the browser. The core has no
transport of its own: the engine (or any other guest path) puts frames in
with `start_flow` and reads addressing from `snapshot()`.

This file is the contract the front end is written against.

## Building

```sh
cargo build --release --target wasm32-unknown-unknown
wasm-bindgen --target web --out-dir pkg \
  target/wasm32-unknown-unknown/release/losos_lab_core.wasm
```

`wasm-bindgen` in `Cargo.toml` is pinned to exactly the version of nixpkgs'
`wasm-bindgen-cli` (0.2.127); change both together or the CLI refuses the
module.

## Conventions

- One class, `Lab`, holds everything. Strings in, JSON strings out: every
  method that returns something returns `JSON.parse`-able text, except
  `last_setup` (the setup document's text, or `undefined`) and `error_page`
  (HTML).
- A refused call answers `{"error": "<sentence for a toast>"}` and changes
  nothing. Nothing throws on bad input.
- All output that happens *over time* (events, packets, console output of
  `ping`/`curl`/`avahi-browse`, finished page loads, timer toasts) is
  collected inside the core and handed out by the next `tick()` or `step()`.
- Ids are the JS ids: devices `d<n>`, links `l<n>`, both from `world.seq`.
- Field names are the JS field names, so the old globals map one to one:
  `world` → `snapshot().world`, `NET` → `snapshot().net`, `SIM` →
  `snapshot().sim`, `CLOCK` → `snapshot().clock`, `UI.scenario` /
  `UI.fileName` / `UI.dirty` → `snapshot().scenario` / `.fileName` / `.dirty`.

## The class

```ts
class Lab {
  // seed: PRNG seed (MAC suffixes, nonces, ping times). nowMs: Date.now().
  // tzOffsetMin: -new Date().getTimezoneOffset(), the lab clock's zone.
  constructor(seed: number, nowMs: number, tzOffsetMin: number);
  free(): void;

  // ── tables ──
  catalog(): string;                       // Catalog
  set_plate(url: string): void;            // LAB.plate, used in page HTML

  // ── setups ──
  load_scenario(key: string): string;      // {focus: string|null} | Err
  load_this_box(settings: string, edge: string): string;
                                           // settings, edge: the JSON text of
                                           // /api/settings and /api/edge
                                           // (edge may be "null").
                                           // → {focus, name, blurb} | Err
  export_setup(): string;                  // {name, filename, text}
  import_setup(text: string, fileName: string): string;
                                           // Open… : {skipped, name} | Err
  open_last(text: string): string;         // "Last setup": {skipped, name}
                                           // | {error, fallback: "two-sites"}
                                           // (two-sites is then loaded)
  last_setup(): string | undefined;        // keepLast(): doc text when dirty
  peek_setup(text: string): string;        // {name} | Err, for the picker

  // ── editing (each one re-evaluates and starts the traffic it implies) ──
  add_device(type: string, x: number, y: number, opts: string): string;
                                           // opts: {site?, px?, py?} → {device} | Err
  connect(a: string, b: string, kind: string, aPort?: string, bPort?: string): string;
                                           // → {link} | Err
  remove_device(id: string): string;       // {ok: true} | Err
  remove_link(id: string): string;         // {ok: true} | Err
  set_power(id: string, on: boolean): string;
                                           // {power, banner: string[]} | Err
  set_cfg(id: string, key: string, value: string): string;
                                           // value: JSON (true / "acme.cfd")
                                           // → {cfg} | Err
  set_name(id: string, name: string): string;   // {name} | Err
  set_site(id: string, site: string): string;   // {ok: true} | Err (px reset to null)
  move_device(id: string, pos: string): string; // pos: {x?, y?, px?, py?, site?}
                                                // → {ok: true} | Err (no re-evaluation)
  phys_pos(id: string): string;            // physPos(): {px, py}, placing it if null

  // ── reading ──
  snapshot(): string;                      // Snapshot
  suggest_urls(id: string): string;        // string[], the Danube suggestions

  // ── simulator and clock ──
  tick(elapsedMs: number): string;         // TickResult; call every frame
  step(): string;                          // TickResult (+ message when
                                           // nothing is queued)
  set_mode(mode: "realtime" | "simulation"): string;  // {ok} | Err
  set_playing(on: boolean): void;
  set_sim_speed(speed: number): void;      // 0.25 … 4 (clamped)
  set_clock_speed(speed: number): string;  // one of catalog.speeds, else Err
  skip_to_next_timer(): string;            // {ok, next: Timer}
  set_filter(proto: string, on: boolean): string;
  clear(): void;                           // simReset()
  scan(id: string): string;                // "Scan for edges now"
  start_flow(stages: string, title: string): string;
                                           // stages: Pdu[][] (from the engine,
                                           // usually with real: true)

  // ── consoles and pages ──
  console_info(id: string): string;        // {prompt, boot: string[], completions: string[]}
  exec(id: string, line: string): string;  // ExecResult
  http(id: string, url: string): string;   // HttpStart
  error_page(error: string, host: string): string;   // errorPage() HTML
}
```

`Err` is `{error: string}`.

## For a renderer in the same module

`admin-ui/lab/render` links this crate into its own module and draws the
same `Lab` the page drives. Not exported to JavaScript:

- `Lab::shared() -> Rc<RefCell<LabCore>>`: the state behind the class. The
  class holds it in an `Rc<RefCell<..>>` for this; every method borrows it
  for the length of the call only.
- `LabCore::world()`, `LabCore::net()`: the world and its last evaluation,
  read in place (`World`, `Device`, `Link`, `Net`, `OMap`, ... are
  re-exported at the crate root).
- `LabCore::packets_now()`: what `tick()` would answer as `packets`,
  without advancing anything.

## Shapes

```ts
type Catalog = {
  types: Record<string, { label; short; cat: "losos"|"net"|"end";
           ports: [string, "eth"|"wifi"|"wifi-ap"][]; endpoint?: true;
           emulate?: string; blurb }>;      // TYPES
  sites: Record<string, { name; sub }>;   // SITES
  linkKinds: Record<string, { label; hint }>;  // LINK_KINDS
  proto: Record<string, { color; label }>; // PROTO
  speeds: number[];                       // SPEEDS
  daily: Timer[];                         // DAILY
  defaultCfg: Record<string, Cfg>; defaultName: Record<string, string>;
  defaultSite: Record<string, string>;
  scenarios: { key; name; blurb }[];      // SCENARIOS in picker order
                                          // ("this-box" first once loaded)
};
type Timer = { at: "00:07"; key: "reboot"|"upgrade"|"gc"|"windowEnd"|"windowStart"; title };

type Device = {
  id; type; name; x: number; y: number; site; px: number|null; py: number|null;
  power: boolean; cfg: Cfg; mac;
  rebooting?: boolean; gen?: number; computeOpen?: boolean;   // set by timers
};
type Cfg = Record<string, boolean|string>;   // keys exactly DEFAULT_CFG[type]
type Link = { id; a: { dev; port }; b: { dev; port }; kind: "copper"|"fiber"|"wan"|"wifi" };

type Snapshot = {
  world: { devices: Device[]; links: Link[]; seq: number };
  net: Net;
  sim: Sim;
  clock: { t: number; speed: number; nextScan: number; nextBeat: number;
           text: string;           // clockText(), "Thu 8 Oct 10:00:00"
           next: Timer };          // nextTimer().e
  scenario: string;                // UI.scenario: a key, "file" or "last"
  fileName: string;                // UI.fileName
  dirty: boolean;                  // UI.dirty
};

// NET, with every Map turned into an object in the JS insertion order.
type Net = {
  segs: Record<string, { id; members: string[]; routers: string[]; internet: boolean;
                         kind: "internet"|"lan"|"isolated"; cloud?: string; router?: string }>;
  iface: Record<string, { port; seg }>;           // device id → active interface
  addr: Record<string, Addr>;                      // device id, and "<router>#lan"
  internet: Record<string, boolean>;
  subnet: Record<string, string>;                  // router id → "192.168.1"
  gearSeg: Record<string, string>;
  mgmt: Record<string, { ip; mask: number; gw?: string }>;
  losos: {
    box: Record<string, { up: boolean; edges: Edge[]; path: Edge|null;
           tunnel: "off"|"enrolled"|"registered"|"refused"|"stopped";
           publicName: string|null; mesh: string; market: string; reason: string;
           meshEdge?: string }>;
    spoke: Record<string, { up: boolean; uplink: string; relayed: string[];
           enrolled: string[]; cluster: string[]; hub?: string }>;
    hub: Record<string, { up: boolean; tenants: string[]; relays: string[]; cluster: string[] }>;
  };
};
type Addr = { ip; mask: number; gw?: string; src: "public"|"router"|"dhcp"|"link-local"; router?: string };
type Edge = { id; source: "lan"|"configured"; official: boolean; url };
// NET.segOf / segsOf / sameSeg / ip / hub are not functions here:
// segOf(id) = net.iface[id]?.seg, ip(id) = net.addr[id]?.ip,
// segsOf(id) = [net.iface[id]?.seg, net.gearSeg[id]].filter(Boolean).

type Sim = {
  mode: "realtime"|"simulation"; playing: boolean; tick: number; speed: number;
  log: Event[];                    // the last 600
  flows: { id: number; stages: Pdu[][]; stage: number; title: string }[];
  active: Active[];
  filters: Record<string, boolean>;
  frameFrac: number; seq: number; cursor: number;
};
type Pdu = { proto; from; to; info; local?: true; hops?: string[]; real?: true };
type Active = { flow: number; proto; info; hops: string[]; i: number; real?: true };
type Event = { proto; last; at; info; local?: true; fail?: true; real?: true;
               final?: boolean; t: string };   // t: seconds, "12.25"

type TickResult = {
  packets: Packet[];        // drawPdus(): only protocols whose filter is on
  events: Event[];          // logged since the last tick/step, all protocols
  reset: boolean;           // the log was cleared since the last tick
  console: { job: number; dev: string; lines: string[]; done: boolean }[];
  http: { job: number; dev: string; url: string; result: HttpResult }[];
  toasts: string[];         // "00:07 midnight-reboot.timer"
  worldChanged: boolean;    // a timer set rebooting/gen/computeOpen
  sim: { mode; playing; tick; frameFrac: number; inFlight: number;
         status: string };  // "t=1.25 s · 3 in flight"
  clock: { t: number; text: string; speed: number; next: Timer };
  message?: string;         // step() with nothing queued
};
type Packet = { flow: number; proto; color; info; from; to; i: number; t: number;
                x: number; y: number;   // logical-view centre, bus drops included
                real?: true };

type ExecResult = { job: number; lines: string[]; busy: boolean; clear: boolean };
// busy: the rest arrives as TickResult.console entries with the same job,
// the last one with done: true. clear: the command was `clear`.
// Lines carry ANSI colour codes (banner, boot text) as the JS did.

type HttpStart = { job: number; busy: boolean; result?: HttpResult };
type HttpResult =
  | { status: number; title: string; text?: string; html: string; error?: "refused" }
  | { error: "mdns"|"nodns"|"nxdomain"|"timeout"|"badurl"; host?: string };
// For an error, draw error_page(error, host). Danube's own "offline" check
// (no interface) stays in the browser: error_page("offline", "").
```

## Timing

`tick(elapsedMs)` is the rAF loop and the 200 ms clock interval in one:

- Realtime: the lab clock advances `elapsedMs × clock speed`; daily timers,
  the five-minute edge rescan (clock speed ≤ 60×) and the one-minute
  heartbeat (≤ 10×) fire on it. Packets hop every
  `max(40, 230 / √speed)` ms of real time; `frameFrac` is the fraction of
  that period gone.
- Simulation: nothing moves unless playing. While playing, a hop every
  `1100 / simSpeed` ms, and each hop advances the lab clock 250 ms. Playing
  stops by itself when nothing is queued. `step()` takes one hop and animates
  it over 450 ms of following ticks.
- Positions: a packet sits between `hops[i-1]` and `hops[i]` at fraction `t`;
  `x`/`y` are already interpolated on the logical canvas (a bus drop runs
  along the backbone, then down). Paused in simulation mode, `t` is 1.

## Setup files

`export_setup().text` is `JSON.stringify(setupDoc(), null, 2) + "\n"`;
`filename` ends in `.llf`. `import_setup` strips `.llf`, `.losos-lab.json`
or `.json` from the file name for the label, refuses text over 1 MiB, then
applies `docProblem` and `buildFromDoc`: every part goes back through
`newDevice` and `connect`, at most 200 devices and 400 links, names through
`deviceName`, cfg keys and types checked against the type's defaults.
`last_setup()` is `keepLast`: when dirty it clears the flag and returns the
document (named "Edited …" for a built-in setup) for `localStorage`.
