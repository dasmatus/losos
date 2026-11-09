#!/usr/bin/env bun
// LosOS box, edge, box demo: stage one.
//
//   curl -fsSL https://raw.githubusercontent.com/dasmatus/losos/main/nix/demo/box-edge-box/bootstrap.ts | bun run -
//
// Looks at this machine (Linux, x86_64, KVM, processors, memory, free disk,
// Nix), picks how big a demo it can carry, writes stage two to
// $LOSOS_DEMO_DIR/stage2.ts with those choices at its top, and runs it.
// Stage two builds or fetches everything through Nix and starts three VMs on
// one private network: box A, an edge between them, box B. Nothing is
// installed on this machine outside the Nix store and the demo folder, and
// nothing needs root except --host-mdns's one network device.
//
// Two sizes:
//   full   two boxes installed from the LosOS installer ISO and an edge VM.
//          Needs KVM, about 14 GiB of memory and 60 GB of disk.
//   light  two boxes with LosOS's control plane and admin pages and an edge
//          gateway, as small test VMs. Runs anywhere Nix runs, KVM or not,
//          with about 6 GiB of memory and 15 GB of disk.
//
// Options (after `bun run - --` when piped):
//   --full | --light   pick the size instead of letting the checks pick
//   --pause            stop at each step of the light demo until Enter, so
//                      the admin pages can be opened and looked at
//   --host-mdns        put this machine on the VMs' network too, so its own
//                      mDNS responder finds boxa.local, boxb.local and
//                      edge.local and a browser here opens them by name.
//                      Makes one network device, the one step that needs
//                      root: through sudo, doas or run0, whichever is here
//                      (LOSOS_DEMO_SU names one).
//   --write-only       write stage two and stop
//   --dir DIR          where stage two and the VMs live
//                      (default ~/.local/share/losos-demo)
//   --ref REF          the LosOS flake to use (default github:dasmatus/losos)
//
// The same settings as environment variables: LOSOS_DEMO_MODE,
// LOSOS_DEMO_DIR, LOSOS_FLAKE, LOSOS_DEMO_HOST_MDNS=1, and LOSOS_NIX_ARGS for
// extra `nix` arguments.
import { spawnSync, spawn } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { cpus, homedir } from "node:os";
import { dirname, join, resolve } from "node:path";

const env = process.env as Record<string, string | undefined>;

function say(...lines: string[]): void {
  console.log(...lines);
}
function die(message: string): never {
  process.stderr.write(`losos-demo: ${message}\n`);
  process.exit(1);
}
function row(label: string, value: string, note: string = ""): void {
  say(`  ${label.padEnd(10)} ${value.padEnd(34)} ${note}`);
}

// ── Command line ────────────────────────────────────────────────────────────
let mode = env.LOSOS_DEMO_MODE ?? "auto";
let dir = env.LOSOS_DEMO_DIR ?? `${env.XDG_DATA_HOME ?? join(homedir(), ".local", "share")}/losos-demo`;
let flake = env.LOSOS_FLAKE ?? "github:dasmatus/losos";
const nixArgs = env.LOSOS_NIX_ARGS ?? "";
let pause = false;
let writeOnly = false;
let hostMdns = env.LOSOS_DEMO_HOST_MDNS === "1";

const argv = process.argv.slice(2);
for (let i = 0; i < argv.length; i++) {
  const a = argv[i];
  if (a === "--full") mode = "full";
  else if (a === "--light") mode = "light";
  else if (a === "--pause") pause = true;
  else if (a === "--host-mdns") hostMdns = true;
  else if (a === "--write-only") writeOnly = true;
  else if (a === "--dir") {
    if (i + 1 >= argv.length) die("--dir needs a folder");
    dir = argv[++i];
  } else if (a === "--ref") {
    if (i + 1 >= argv.length) die("--ref needs a flake reference");
    flake = argv[++i];
  } else if (a === "-h" || a === "--help") {
    // The help text is this file's own header, the way the bash stage one
    // printed lines 2-40 of itself.
    const head = readFileSync(resolve(process.argv[1] ?? import.meta.path), "utf8")
      .split("\n")
      .slice(1, 38)
      .map((line) => line.replace(/^\/\/ ?/, ""))
      .join("\n");
    console.log(head);
    process.exit(0);
  } else die(`unknown option ${a} (try --help)`);
}

// ── What this machine has ───────────────────────────────────────────────────
say("LosOS box, edge, box demo");
say("");
say("This machine:");

const run = (cmd: string, args: string[]) =>
  spawnSync(cmd, args, { encoding: "utf8" }).stdout?.trim() ?? "";

const osType = run("uname", ["-s"]);
const uarch = run("uname", ["-m"]);
if (osType !== "Linux") die(`the demo runs its VMs on Linux; this is ${osType || "unknown"}`);
if (uarch !== "x86_64") die(`LosOS is built for x86_64; this machine is ${uarch}`);
row("system", `${osType} ${uarch}`, "ok");

const kvmNode = "/dev/kvm";
const kvm = spawnSync("test", ["-r", kvmNode]).status === 0 && spawnSync("test", ["-w", kvmNode]).status === 0;
if (kvm) row("KVM", "/dev/kvm", "ok");
else if (exists(kvmNode)) row("KVM", "/dev/kvm is there, not usable", "add yourself to the kvm group for a fast demo");
else row("KVM", "none", "the VMs run in software emulation");

function exists(path: string): boolean {
  return spawnSync("test", ["-e", path]).status === 0;
}

const nprocs = Math.max(1, cpus().length);
row("processors", String(nprocs), "");

const procMem = readFileSync("/proc/meminfo", "utf8");
function memKbOf(name: string): number {
  const m = procMem.match(new RegExp(`^${name}:\\s+(\\d+) kB`, "m"));
  return m ? Number(m[1]) : 0;
}
const memGib = Math.floor(memKbOf("MemTotal") / 1048576);
const availGib = Math.floor(memKbOf("MemAvailable") / 1048576);
row("memory", `${memGib} GiB, ${availGib} GiB free`, "");

mkdirSync(dir, { recursive: true });
// Absolute, so stage two finds it from any folder, and from inside dir/out.
dir = resolve(dir);
function dfGiBFree(path: string): number {
  const p = spawnSync("df", ["-Pk", path], { encoding: "utf8" });
  if (p.status !== 0) return 0;
  const cols = (p.stdout?.split("\n")[1] ?? "").split(/\s+/);
  const free = Number(cols[3]);
  return Number.isFinite(free) ? Math.floor(free / 1e6) : 0;
}
const diskGb = dfGiBFree(dir);
// The Nix store fills up too, and it may be on another disk.
let storeGb = diskGb;
if (exists("/nix/store")) storeGb = dfGiBFree("/nix/store");
row("disk", `${diskGb} GB free in ${dir}`, "");
if (storeGb !== diskGb) row("", `${storeGb} GB free for /nix/store`, "");

let nix = "";
for (const candidate of ["nix", "/nix/var/nix/profiles/default/bin/nix"]) {
  const found = dirname(candidate) === "nix" ? run("which", ["nix"]) : "";
  if (found) {
    nix = found;
    break;
  }
  if (candidate !== "nix" && exists(candidate)) {
    nix = candidate;
    break;
  }
}
if (nix) row("Nix", run(nix, ["--version"]).split("\n")[0] ?? "", "ok");
else row("Nix", "not installed", "needed");

// mDNS here: a responder on the network (Avahi, or systemd-resolved with
// MulticastDNS on), and programs asking it for .local names (nss-mdns, or
// resolved's nss-resolve, in the hosts line of nsswitch.conf). The VMs answer
// for their own names, but only on their own network; --host-mdns joins
// this machine to it.
let mdnsHost = "";
if ((run("ps", ["-e", "-o", "comm="]).split("\n").some((c) => c.trim() === "avahi-daemon"))) mdnsHost = "avahi";
else {
  const st = spawnSync("resolvectl", ["status"], { encoding: "utf8" });
  if (st.status === 0 && (st.stdout ?? "").includes("+mDNS")) mdnsHost = "resolved";
}
let mdnsNss = false;
try {
  const nss = readFileSync("/etc/nsswitch.conf", "utf8");
  mdnsNss = /^hosts:.*?[\t ](mdns[46]?(_minimal)?|resolve)([\t ]|$)/m.test(nss);
} catch {
  // no nsswitch.conf on this system; the boxes' names stay inside the LAN
}

// How --host-mdns's one step becomes root: it already is, or sudo, doas or
// run0 (systemd 256 and later), the first one here.
let suCmd = env.LOSOS_DEMO_SU ?? "";
if (!suCmd) {
  if (run("id", ["-u"]) === "0") suCmd = "none";
  else {
    for (const c of ["sudo", "doas", "run0"]) {
      if (run("which", [c])) {
        suCmd = c;
        break;
      }
    }
  }
}
if (mdnsHost === "avahi" && mdnsNss) row("mDNS", "avahi-daemon, nss-mdns", "ok");
else if (mdnsHost === "resolved" && mdnsNss) row("mDNS", "systemd-resolved", "ok");
else if (mdnsHost === "avahi") row("mDNS", "avahi-daemon, no nss-mdns", "programs here do not ask it for .local names");
else if (mdnsHost === "resolved") row("mDNS", "systemd-resolved, no nss-resolve", "programs here do not ask it for .local names");
else row("mDNS", "none", "the boxes' .local names stay inside the VMs' network");
say("");

if (!nix) {
  say("The demo needs Nix: it brings QEMU, swtpm and every image itself, with");
  say("nothing installed outside /nix. Install it with the official installer,");
  say("open a new terminal, and run this line again:");
  say("");
  say("  sh <(curl -L https://nixos.org/nix/install) --daemon");
  say("");
  process.exit(1);
}

// ── How big a demo ──────────────────────────────────────────────────────────
let fullOk = true;
let whyNotFull = "";
if (!kvm) {
  fullOk = false;
  whyNotFull = "no KVM (installing two boxes in software takes hours)";
}
if (memGib < 14) whyNotFull = `${whyNotFull ? `${whyNotFull}; ` : ""}${memGib} GiB of memory, 14 needed`;
if (!(diskGb >= 60 && storeGb >= 30)) whyNotFull = `${whyNotFull ? `${whyNotFull}; ` : ""}60 GB of free disk needed`;

let lightOk = true;
if (memGib < 6) {
  lightOk = false;
  say(`Only ${memGib} GiB of memory: the light demo needs 6.`);
}
if (storeGb < 15) {
  lightOk = false;
  say(`Only ${storeGb} GB free for /nix/store: the light demo needs 15.`);
}
if (mode === "auto") {
  if (fullOk) mode = "full";
  else if (lightOk) {
    mode = "light";
    say(`Light demo: ${whyNotFull}.`);
  } else die("this machine is too small for either demo");
} else if (mode === "full" && !fullOk) say(`Full demo asked for, though: ${whyNotFull}. Carrying on.`);
else if (mode === "light" && !lightOk) say("Light demo asked for on a machine below its needs. Carrying on.");
else if (mode !== "full" && mode !== "light") die(`LOSOS_DEMO_MODE is full, light or auto, not ${mode}`);

if (hostMdns && !mdnsHost) {
  say("--host-mdns needs an mDNS responder here: install Avahi and nss-mdns, or turn");
  say("on MulticastDNS in systemd-resolved. Carrying on without it.");
  hostMdns = false;
} else if (hostMdns && !suCmd) {
  say("--host-mdns makes one network device as root, through sudo, doas or run0,");
  say("and none of them is here. Carrying on without it.");
  hostMdns = false;
} else if (!hostMdns && mdnsHost && mdnsNss && suCmd) {
  say("To open the boxes as http://boxa.local/ from this machine, add --host-mdns");
  if (suCmd !== "none") say(`(it asks ${suCmd} once, to make one network device).`);
}

// Memory per installed box: 8 GiB where there is room, else 4, which is
// enough for the walk through.
const boxMem = memGib >= 24 ? 8192 : 4096;

// Pin the flake to the revision it points at now, so stage two builds the
// same thing every time it runs.
function nixf(...args: string[]): string {
  const extra = nixArgs ? nixArgs.trim().split(/\s+/) : [];
  const p = spawnSync(nix, ["--extra-experimental-features", "nix-command flakes", ...extra, ...args], { encoding: "utf8" });
  if (p.status !== 0) die(p.stderr?.trim() || `nix ${args.join(" ")} failed`);
  return p.stdout ?? "";
}
say(`Looking up ${flake} ...`);
let meta: { locked?: { rev?: string } };
try {
  meta = JSON.parse(nixf("flake", "metadata", "--json", flake)) as { locked?: { rev?: string } };
} catch (e) {
  die(`could not read the flake ${flake} (${(e as Error).message})`);
}
// The flake's own revision is the top-level locked.rev; every input has a
// locked.rev of its own further down.
const rev = meta?.locked?.rev ?? "";
if (flake.startsWith("github:") && rev) {
  const head = flake.split("/").slice(0, 2).join("/");
  flake = `${head}/${rev}`;
}
say(`Using ${flake}`);
say("");

// ── Stage two ───────────────────────────────────────────────────────────────
// The part that builds and runs, written to the demo folder with the choices
// above at its top — mode, flake, KVM, memory — because an owner runs it
// again tomorrow with `bun stage2.ts down` or `… clean`, when the survey has
// long left the scrollback. The file carries the same header block the bash
// stage two used to write before its body.
const stamp = new Date().toISOString().replace(/\.\d+Z$/, "Z").slice(0, 16) + "Z";
const stage2Path = join(dir, "stage2.ts");
const SHEBANG = `#!/usr/bin/env bun
// LosOS box, edge, box demo: stage two, written by bootstrap.ts on ${stamp}
// for this machine. Run it again any time:
//
//   bun ${stage2Path}            build what is missing, then run the demo
//   bun ${stage2Path} build      only build
//   bun ${stage2Path} demo       only run the demo
//   bun ${stage2Path} down       stop the VMs (full demo; keeps the disks)
//   bun ${stage2Path} host-mdns-off   remove the network device --host-mdns made
//   bun ${stage2Path} clean      stop and delete everything in ${dir}
//
// The choices bootstrap.ts made; edit them here.
`;
const BODY = String.raw`
import { spawnSync, spawn } from "node:child_process";
import { mkdirSync, rmSync, existsSync, writeFileSync, readFileSync, symlinkSync } from "node:fs";
import { dirname, join } from "node:path";

const mode = "{{MODE}}";
const flake = "{{FLAKE}}";
const dir = "{{DIR}}";
const nix = "{{NIX}}";
const nixArgs = "{{NIXARGS}}";
const kvm = {{KVM}};
const boxMem = {{BOXMEM}};
const pause = {{PAUSE}};
const hostMdns = {{HOSTMDNS}};
const mdnsHost = "{{MDNSHOST}}";
const suCmd = "{{SUCMD}}";
const rerun = "bun {{STAGE2}}";

const tap = "vde-tap77";
function log(...parts: string[]): void {
  const stamp = new Date().toTimeString().slice(0, 8);
  process.stderr.write("\x1b[1;36m[" + stamp + "]\x1b[0m " + parts.join(" ") + "\n");
}
function die(message: string): never {
  process.stderr.write("\x1b[1;31merror:\x1b[0m " + message + "\n");
  process.exit(1);
}
function sleepFor(seconds: number): void {
  if (seconds > 0) spawnSync("sleep", [String(seconds)], { stdio: "ignore" });
}
mkdirSync(dir, { recursive: true });

// nix with flakes on, plus the owner's extra arguments from bootstrap.
function nixf(...args: string[]): string {
  const nargs = nixArgs ? nixArgs.trim().split(/\s+/) : [];
  const p = spawnSync(nix, ["--extra-experimental-features", "nix-command flakes", ...nargs, ...args], { encoding: "utf8" });
  if (p.status !== 0) die(p.stderr?.trim() || ("nix " + args.join(" ") + " failed"));
  return p.stdout ?? "";
}

// ── This machine on the VMs' network (--host-mdns) ───────────────────────────
// One tap device, vde-tap77, with an address of its own on the VMs' network,
// so this machine's mDNS responder hears the VMs and answers boxa.local,
// boxb.local and edge.local for the browser here. Making it is the only step
// that needs root, and it belongs to you afterwards. The light demo's test
// driver plugs a tap of that name (vlan 77) into its network itself; the
// full demo plugs it into run.ts's switch.
function asRoot(args: string[]): number {
  if (suCmd === "none") return spawnSync(args[0], args.slice(1), { stdio: "inherit" }).status ?? 1;
  return spawnSync(suCmd, args, { stdio: "inherit" }).status ?? 1;
}
function ipCmd(): string {
  const which = spawnSync("which", ["ip"], { encoding: "utf8" });
  if (which.status === 0 && which.stdout?.trim()) return which.stdout.trim();
  const out = nixf("build", "--no-link", "--print-out-paths", "nixpkgs#iproute2^out");
  const path = out.trim().split("\n").at(-1) ?? "";
  return join(path, "bin", "ip");
}
function hostMdnsUp(): void {
  if (!hostMdns) return;
  const ip = ipCmd();
  const addr = mode === "light" ? "192.168.77.254/24" : "10.77.0.254/24";
  const show = spawnSync(ip, ["-o", "-4", "addr", "show", "dev", tap], { encoding: "utf8" });
  if ((show.stdout ?? "").includes("inet " + addr + " ")) return;
  log("making " + tap + " at " + addr.split("/")[0] + " for this machine's mDNS"
    + (suCmd !== "none" ? " (" + suCmd + " may ask for your password)" : ""));
  asRoot(["sh", "-c", "'" + ip + "' link show " + tap + " >/dev/null 2>&1 || '" + ip + "' tuntap add dev " + tap + " mode tap user " + process.getuid() + "\n'" + ip + "' addr flush dev " + tap + " && '" + ip + "' addr add " + addr + " dev " + tap + " && '" + ip + "' link set " + tap + " up"]);
}
function hostMdnsLink(): void { // resolved keeps mDNS per link: switch it on once the tap is plugged in
  if (!(hostMdns && mdnsHost === "resolved")) return;
  for (let i = 0; i < 120; i++) {
    try {
      if (readFileSync("/sys/class/net/" + tap + "/carrier", "utf8").trim() === "1") break;
    } catch {
      break;
    }
    sleepFor(1);
  }
  if (asRoot(["resolvectl", "mdns", tap, "yes"]) !== 0) {
    log("could not switch mDNS on for " + tap + "; run as root: resolvectl mdns " + tap + " yes");
  }
}
function hostMdnsPlug(): void { // full: plug the tap into run.ts's switch, and keep it there
  if (!hostMdns) return;
  const pidFile = join(dir, "state", "plug2tap.pid");
  if (existsSync(pidFile)) {
    const lit = Number(readFileSync(pidFile, "utf8").trim());
    if (Number.isFinite(lit) && lit > 0 && spawnSync("kill", ["-0", String(lit)], { stdio: "ignore" }).status === 0) return;
    rmSync(pidFile, { force: true });
  }
  const nargs = nixArgs ? nixArgs.trim().split(/\s+/) : [];
  const child = spawn(nix, [...nargs, "shell", "nixpkgs#vde2", "-c", "vde_plug2tap", "-s", join(dir, "state", "lan.sock"), tap], {
    detached: true,
    stdio: ["ignore", "ignore", "ignore"],
  });
  child.unref();
  mkdirSync(dirname(pidFile), { recursive: true });
  writeFileSync(pidFile, String(child.pid) + "\n");
  hostMdnsLink();
}
function hostMdnsReport(...names: string[]): void { // what this machine's own resolver makes of the names
  if (!hostMdns) return;
  for (const name of names) {
    const got = spawnSync("getent", ["ahostsv4", name + ".local"], { encoding: "utf8" });
    const first = (got.stdout ?? "").split("\n")[0]?.split(/\s+/)[0] ?? "";
    if (first) log("this machine finds " + name + ".local at " + first + " by mDNS: http://" + name + ".local/");
    else log("this machine does not find " + name + ".local: is nss-mdns or nss-resolve in nsswitch.conf, and UDP 5353 open on " + tap + "?");
  }
}
function hostMdnsOff(): void {
  const pidFile = join(dir, "state", "plug2tap.pid");
  if (existsSync(pidFile)) {
    const lit = readFileSync(pidFile, "utf8").trim();
    if (lit) spawnSync("kill", [lit], { stdio: "ignore" });
    rmSync(pidFile, { force: true });
  }
  if (existsSync("/sys/class/net/" + tap)) {
    const ip = ipCmd();
    if (asRoot([ip, "link", "del", tap]) === 0) log("removed " + tap);
  }
}

// ── Light: three small VMs in one test run ───────────────────────────────────
// nix/demo/box-edge-box/demo.nix in the LosOS tree. The test driver brings
// its own QEMU and uses KVM when there is one.
// Every build goes through Nix, which hands back what it already has for this
// revision at once and builds anew when the flake has changed.
function lightBuild(): void {
  log("building the three VMs (the first time compiles LosOS's control plane: tens of minutes)");
  nixf("build", "-L", "-o", join(dir, "driver"), flake + "#checks.x86_64-linux.losos-box-edge-box.driver");
}

function lightPauser(rec: string, alive: () => boolean): void { // reads the demo's steps as they happen and waits for Enter
  let seen = 0;
  while (alive()) {
    const timeline = join(rec, "timeline.jsonl");
    let lines: string[] = [];
    if (existsSync(timeline)) lines = readFileSync(timeline, "utf8").trim().split("\n").filter(Boolean);
    for (; seen < lines.length; seen++) {
      const m = lines[seen].match(/"caption": "([^"]*)"/);
      const caption = m ? m[1] : "";
      const tty = openSync("/dev/tty", "w");
      try {
        writeFileSync(tty, "\n\x1b[1m" + caption + "\x1b[0m\n");
        if (hostMdns) {
          writeFileSync(tty, "  Box A: http://boxa.local/mesh   Box B: http://boxb.local/mesh\n");
        } else {
          writeFileSync(tty, "  Box A: http://127.0.0.1:18081/mesh   Box B: http://127.0.0.1:18082/mesh\n");
        }
        writeFileSync(tty, "  Sign in on each with \"Use the spare key\" and the key in " + dir + "/rec/boxa.token or boxb.token.\n");
        if (existsSync(join(rec, "edge.key"))) {
          writeFileSync(tty, "  The edge: http://127.0.0.1:18443/manage/ with the key in " + dir + "/rec/edge.key\n");
        }
        writeFileSync(tty, "  Press Enter for the next step. ");
      } finally {
        closeSync(tty);
      }
      readEnter();
      writeFileSync(join(rec, "ack-" + (seen + 1)), "");
    }
    sleepFor(1);
  }
}
function readEnter(): void { // one Enter on the console this ran from
  spawnSync("sh", ["-c", "read -r _ < /dev/tty"], { stdio: ["ignore", "ignore", "ignore"] });
}

function lightDemo(): void {
  lightBuild();
  const out = join(dir, "out");
  const rec = join(dir, "rec");
  const logf = join(dir, "demo.log");
  rmSync(out, { recursive: true, force: true });
  rmSync(rec, { recursive: true, force: true });
  mkdirSync(out, { recursive: true });
  mkdirSync(rec, { recursive: true });
  hostMdnsUp();
  log("starting box A, box B and the edge (" + (kvm ? "a few minutes" : "about 15 minutes without KVM") + ")");
  log("the full log goes to " + logf);
  const pausing = pause && existsSync("/dev/tty");
  const driver = spawn(join(dir, "driver", "bin", "nixos-test-driver"), ["--no-interactive", "-o", out], {
    cwd: out,
    env: pausing ? { ...process.env, LOSOS_RECORD_DIR: rec, LOSOS_RECORD_WAIT: "0" } : { ...process.env },
    stdio: ["ignore", "ignore", "ignore"],
  });
  // The narration: the demo log as it passes, exactly the pipeline the bash
  // stage two used, attached to this terminal. tail --pid ends itself when
  // the driver stops, however the demo ends.
  const narrate = spawn(
    "sh",
    ["-c", "tail --pid " + driver.pid + " -n +1 -F \"" + logf + "\" 2>/dev/null | grep --line-buffered -E \"^=== |^box[ab] asked|found [a-z]+\\.local\""],
    { stdio: ["ignore", "inherit", "inherit"], detached: true },
  );
  narrate.unref();
  if (pausing) lightPauser(rec, () => alive(String(driver.pid)));
  let rc = 0;
  driver.on("exit", (code) => {
    rc = code ?? 1;
  });
  while (alive(String(driver.pid))) sleepFor(1);
  sleepFor(2);
  if (rc === 0) log("the demo passed every step. Log: " + logf);
  else {
    spawnSync("tail", ["-n", "40", logf], { stdio: "inherit" });
    die("the demo stopped (exit " + rc + "); the whole log is " + logf);
  }
}
function alive(pid: string): boolean {
  return spawnSync("kill", ["-0", pid], { stdio: "ignore" }).status === 0;
}

// ── Full: two boxes installed from the ISO and an edge VM ────────────────────
// nix/demo/edge-lan/run.ts in the LosOS tree does the work; this fetches
// that tree at the pinned revision and runs it with the state kept here.
function fullSrc(): void {
  log("fetching the LosOS tree");
  const nargs = nixArgs ? nixArgs.trim().split(/\s+/) : [];
  const meta = spawnSync(nix, ["--extra-experimental-features", "nix-command flakes", ...nargs, "flake", "prefetch", "--json", flake], { encoding: "utf8" });
  if (meta.status !== 0) die("could not fetch " + flake);
  const m = (meta.stdout ?? "").match(/"storePath":"([^"]*)"/);
  const path = m ? m[1] : "";
  if (!path) die("could not fetch " + flake);
  rmSync(join(dir, "src"), { force: true, recursive: true });
  spawnSync("ln", ["-sfn", path, join(dir, "src")], { stdio: "ignore" });
}
function fullRun(args: string[]): void {
  if (!existsSync(join(dir, "src", "nix", "demo", "edge-lan"))) fullSrc();
  // run.ts calls plain nix: give it this one, with flakes on.
  const env: NodeJS.ProcessEnv = { ...process.env };
  env.PATH = dirname(nix) + ":" + (env.PATH ?? "");
  env.NIX_CONFIG = "extra-experimental-features = nix-command flakes" + (env.NIX_CONFIG ? "\n" + env.NIX_CONFIG : "");
  env.LOSOS_FLAKE = flake;
  env.LOSOS_DEMO_STATE = join(dir, "state");
  env.LOSOS_BOX_MEM = String(boxMem);
  env.LOSOS_NIX_ARGS = nixArgs;
  const p = spawnSync(process.execPath, [join(dir, "src", "nix", "demo", "edge-lan", "run.ts"), ...args], { stdio: "inherit", env });
  if (p.status !== 0) die("run.ts " + args.join(" "));
}

function doClean(): void {
  hostMdnsOff();
  if (mode === "full" && existsSync(join(dir, "src"))) {
    const env: NodeJS.ProcessEnv = { ...process.env };
    env.PATH = dirname(nix) + ":" + (env.PATH ?? "");
    env.LOSOS_FLAKE = flake;
    env.LOSOS_DEMO_STATE = join(dir, "state");
    env.LOSOS_BOX_MEM = String(boxMem);
    env.LOSOS_NIX_ARGS = nixArgs;
    spawnSync(process.execPath, [join(dir, "src", "nix", "demo", "edge-lan", "run.ts"), "clean"], { stdio: "inherit", env });
  }
  rmSync(dir, { recursive: true, force: true });
  log("deleted " + dir);
}

const cmd = process.argv[2] ?? "run";
if (mode === "light") {
  if (cmd === "build") lightBuild();
  else if (cmd === "demo" || cmd === "run") lightDemo();
  else if (cmd === "down") log("the light demo stops its VMs when it ends");
  else if (cmd === "host-mdns-off") hostMdnsOff();
  else if (cmd === "clean") doClean();
  else die("usage: " + rerun + " [build|demo|down|host-mdns-off|clean]");
} else {
  if (cmd === "build") { fullSrc(); fullRun(["build"]); }
  else if (cmd === "demo" || cmd === "run") {
    fullRun(["build"]);
    hostMdnsUp();
    fullRun(["up"]);
    hostMdnsPlug();
    fullRun(["walk"]);
    fullRun(["publish"]);
    hostMdnsReport("boxa", "boxb", "edge");
    log("the boxes keep running: box A http://127.0.0.1:8080/  box B http://127.0.0.1:8081/ (the password is printed above)");
    log("stop them with: " + rerun + " down");
  }
  else if (cmd === "down") { hostMdnsOff(); fullRun(["down"]); }
  else if (cmd === "host-mdns-off") hostMdnsOff();
  else if (cmd === "clean") doClean();
  else die("usage: " + rerun + " [build|demo|down|host-mdns-off|clean]");
}
`;
const STAGE2 = BODY.split("{{MODE}}").join(mode)
  .split("{{FLAKE}}").join(flake)
  .split("{{DIR}}").join(dir)
  .split("{{NIX}}").join(nix)
  .split("{{NIXARGS}}").join(nixArgs)
  .split("{{KVM}}").join(String(kvm))
  .split("{{BOXMEM}}").join(String(boxMem))
  .split("{{PAUSE}}").join(String(pause))
  .split("{{HOSTMDNS}}").join(String(hostMdns))
  .split("{{MDNSHOST}}").join(mdnsHost)
  .split("{{SUCMD}}").join(suCmd)
  .split("{{STAGE2}}").join(stage2Path);
const stage2File = SHEBANG + STAGE2;
writeFileSync(stage2Path, stage2File, { mode: 0o755 });

say(`Wrote ${stage2Path} (${mode} demo).`);
if (writeOnly) {
  say(`Run it with: bun ${stage2Path}`);
  process.exit(0);
}
say("");
const child = spawn(process.execPath, [stage2Path], { stdio: "inherit" });
child.on("exit", (code) => process.exit(code ?? 1));
