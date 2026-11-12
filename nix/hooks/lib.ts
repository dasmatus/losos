// Shared by the git hooks in this folder (pre-commit, pre-push, commit-msg).
// Imported, not run. The hooks replay CI's gates for the files a commit or a
// push touches, using .github/scripts/ci-changes.ts to pick the same lanes CI
// would, so a change that would turn main red is stopped on the machine that
// made it.
//
// What each hook runs is in its own header. Everything here is plumbing:
// finding the tools (cargo is the flake's pinned toolchain; bun comes from
// the flake when it is not on PATH), running one check with its output in a log file,
// and the summary at the end.
//
//   LOSOS_HOOKS_SKIP=name,name   skip these checks, by the names the hook
//                                prints; each skip is printed, never silent
//   LOSOS_HOOKS_VERBOSE=1        stream every check's output, not just a
//                                failing one's tail
//
// Logs: $(git rev-parse --git-common-dir)/losos-hooks/logs/<UTC time>-<hook>/,
// one file per check; the last 20 runs are kept.
//
// A check that passed is remembered against the tree it ran on (hook_tree,
// a git tree id), under losos-hooks/passed/, so pushing a commit whose tree
// pre-commit already checked does not run the same check twice. A tree id
// names the exact content, so a change anywhere runs everything again.

import { spawn, spawnSync } from "node:child_process";
import {
  appendFileSync,
  closeSync,
  constants as fsConstants,
  cpSync,
  existsSync,
  mkdirSync,
  openSync,
  readdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import * as path from "node:path";

// ── Where things are ─────────────────────────────────────────────────────

function git(args: string[], opts: { cwd?: string; allowFail?: boolean } = {}): string {
  const p = spawnSync("git", args, { cwd: opts.cwd, encoding: "utf8" });
  if (p.status !== 0 && !opts.allowFail) {
    console.error(`git ${args.join(" ")} failed: ${(p.stderr ?? "").trim()}`);
    process.exit(1);
  }
  return (p.stdout ?? "").trim();
}

// The checkout the hook runs in. Rebound by pre-push when it checks a
// temporary worktree instead.
let hookRootDir = git(["rev-parse", "--show-toplevel"]);
const hookCommonDir = path.resolve(git(["rev-parse", "--git-common-dir"]));
const hookLogsBase = path.join(hookCommonDir, "losos-hooks/logs");
const hookPassedBase = path.join(hookCommonDir, "losos-hooks/passed");

/** The checkout the hook runs in. */
export function hookRoot(): string {
  return hookRootDir;
}

/** Point the hook at another checkout (pre-push's temporary worktree). */
export function setHookRoot(dir: string): void {
  hookRootDir = dir;
}

/** Set by a hook when what it checks is exactly one git tree. */
let hookTreeId = "";

/** Check the passed-marks of this tree (a git tree id). */
export function setHookTree(tree: string): void {
  hookTreeId = tree;
}

/** The flake of the tree being checked, or of the checkout given. A shallow
 *  clone, and a worktree of one, evaluates only with ?shallow=1. */
export function hookFlake(dir: string = hookRootDir): string {
  const shallow =
    spawnSync("git", ["-C", dir, "rev-parse", "--is-shallow-repository"], { encoding: "utf8" })
      .stdout?.trim() === "true";
  return `git+file://${dir}${shallow ? "?shallow=1" : ""}`;
}

// The crate manifest is the one list of this repo's Rust workspaces
// (crates.json; nix/check-crates.ts asserts it covers every Cargo.toml in
// the tree, devenv.nix derives its lists from it too, and
// docs/decisions/0001-not-a-cargo-workspace.md says why there is no
// workspace to begin with). Read lazily from the checkout being checked:
// pre-push re-points hookRoot at a temporary worktree.
type Crate = { path: string; fmt: boolean; fmtAll?: boolean; test: boolean };

function crateManifest(): Crate[] {
  return (JSON.parse(readFileSync(path.join(hookRootDir, "crates.json"), "utf8")) as {
    crates: Crate[];
  }).crates;
}

/** Every linted crate, as devenv.nix's `linted` and CI's clippy job list them. */
export function hookCrates(): string[] {
  return crateManifest()
    .filter((c) => c.fmt)
    .map((c) => c.path);
}

// git exports the repository to its hooks: GIT_DIR (in a linked worktree)
// and GIT_INDEX_FILE (in pre-commit). The hook's own git calls want them; a
// check must not see them. A test that makes a repository of its own (the
// backend's text search, config repo and forge suites) would otherwise
// commit into, and reconfigure, the repository being checked.
export const hookGitEnv = [
  "GIT_DIR",
  "GIT_WORK_TREE",
  "GIT_INDEX_FILE",
  "GIT_COMMON_DIR",
  "GIT_PREFIX",
  "GIT_OBJECT_DIRECTORY",
  "GIT_ALTERNATE_OBJECT_DIRECTORIES",
  "GIT_NAMESPACE",
];

/** Drop the repository variables git exported to this hook, for good. */
export function unexportGit(): void {
  for (const k of hookGitEnv) delete process.env[k];
}

function envWithoutGit(): NodeJS.ProcessEnv {
  const env = { ...process.env };
  for (const k of hookGitEnv) delete env[k];
  return env;
}

// ── The run summary ──────────────────────────────────────────────────────

let hookName = "";
let hookLogDir = "";
let hookT0 = 0;
const hookPassed: string[] = [];
const hookFailed: string[] = [];
const hookSkipped: string[] = [];

function nowSec(): number {
  return Math.floor(Date.now() / 1000);
}

function utcStamp(): string {
  const d = new Date();
  const p = (n: number) => String(n).padStart(2, "0");
  return (
    `${d.getUTCFullYear()}${p(d.getUTCMonth() + 1)}${p(d.getUTCDate())}` +
    `T${p(d.getUTCHours())}${p(d.getUTCMinutes())}${p(d.getUTCSeconds())}Z`
  );
}

/** Begin a hook run: a fresh log directory, the old ones pruned, the
 *  summary counters reset. */
export function hookStart(name: string): void {
  hookName = name;
  hookLogDir = path.join(hookLogsBase, `${utcStamp()}-${name}`);
  mkdirSync(hookLogDir, { recursive: true });
  // Keep the last 20 runs.
  const runs = readdirSync(hookLogsBase, { withFileTypes: true })
    .filter((e) => e.isDirectory())
    .map((e) => e.name)
    .sort();
  for (const old of runs.slice(0, Math.max(0, runs.length - 20)))
    rmSync(path.join(hookLogsBase, old), { recursive: true, force: true });
  // And what passed on trees not seen for a week.
  if (existsSync(hookPassedBase)) {
    const cutoff = Date.now() - 7 * 24 * 60 * 60 * 1000;
    for (const e of readdirSync(hookPassedBase)) {
      const p = path.join(hookPassedBase, e);
      try {
        if (statSync(p).mtimeMs < cutoff) rmSync(p, { recursive: true, force: true });
      } catch {
        // gone already
      }
    }
  }
  hookPassed.length = 0;
  hookFailed.length = 0;
  hookSkipped.length = 0;
  hookT0 = nowSec();
  console.log(`losos ${name}: logs in ${hookLogDir}`);
}

/** Would this check be skipped? Honors LOSOS_HOOKS_SKIP. */
export function hookSkipping(name: string): boolean {
  const skip = `,${process.env.LOSOS_HOOKS_SKIP ?? ""},`;
  return skip.includes(`,${name},`) || skip.includes(",all,");
}

/** Count a check as skipped without running it (the VM tests' own knob). */
export function addSkipped(name: string): void {
  hookSkipped.push(name);
}

/** The current run's log directory. */
export function hookLogdir(): string {
  return hookLogDir;
}

/** Print the summary; the failure count, for callers that go on (pre-push
 *  checks several refs and cleans up between them). */
export function hookSummary(): number {
  const dt = nowSec() - hookT0;
  console.log(
    `losos ${hookName}: ${hookPassed.length} passed, ${hookFailed.length} failed, ` +
      `${hookSkipped.length} skipped in ${dt}s`,
  );
  if (hookFailed.length > 0) {
    console.error(`losos ${hookName}: refused. Failed: ${hookFailed.join(" ")}`);
    console.error(`  Full logs: ${hookLogDir}`);
    console.error(`  Fix them, or skip one on purpose with LOSOS_HOOKS_SKIP=<name>.`);
    return 1;
  }
  return 0;
}

/** End the hook run: print the summary and exit. */
export function hookFinish(): never {
  process.exit(hookSummary());
}

// ── Running one check ────────────────────────────────────────────────────

const verbose = process.env.LOSOS_HOOKS_VERBOSE === "1";

// While a check runs, its output goes here. Outside a check (the lane
// picker, the summary) output goes to the hook's own streams.
let sink: { log: string } | null = null;

function toLog(line: string): void {
  if (sink) {
    appendFileSync(sink.log, line + "\n");
    if (verbose) process.stdout.write(line + "\n");
  } else {
    console.log(line);
  }
}

/** Spawn a command as part of a check: its output into the check's log
 *  (streamed too when LOSOS_HOOKS_VERBOSE=1), without git's repository
 *  variables. Outside a check, output is inherited. */
function spawnC(
  argv: string[],
  opts: { cwd?: string; input?: string } = {},
): Promise<number> {
  return new Promise((resolveP) => {
    const strip = sink !== null;
    const child = spawn(argv[0], argv.slice(1), {
      cwd: opts.cwd,
      env: strip ? envWithoutGit() : process.env,
      stdio: [opts.input !== undefined ? "pipe" : "inherit", "pipe", "pipe"],
    });
    if (opts.input !== undefined) child.stdin.end(opts.input);
    const logFd = sink ? openSync(sink.log, "a") : null;
    const tee = (chunk: Buffer) => {
      if (logFd !== null) {
        appendFileSync(logFd, chunk);
        if (verbose) process.stdout.write(chunk);
      } else {
        process.stdout.write(chunk);
      }
    };
    child.stdout.on("data", tee);
    child.stderr.on("data", tee);
    child.on("error", (e) => {
      tee(Buffer.from(`${e.message}\n`));
      resolveP(127);
    });
    child.on("close", (code) => {
      if (logFd !== null) closeSync(logFd);
      resolveP(code ?? 1);
    });
  });
}

/** Spawn a command whose stdout is captured (stderr still reaches the log
 *  or the terminal). Returns null when the command fails. */
function captureC(
  argv: string[],
  opts: { cwd?: string; input?: string } = {},
): Promise<{ status: number; stdout: string } | null> {
  return new Promise((resolveP) => {
    const child = spawn(argv[0], argv.slice(1), {
      cwd: opts.cwd,
      env: sink ? envWithoutGit() : process.env,
      stdio: [opts.input !== undefined ? "pipe" : "inherit", "pipe", "pipe"],
    });
    if (opts.input !== undefined) child.stdin.end(opts.input);
    const logFd = sink ? openSync(sink.log, "a") : null;
    let out = "";
    child.stdout.on("data", (d: Buffer) => (out += d.toString()));
    child.stderr.on("data", (d: Buffer) => {
      if (logFd !== null) {
        appendFileSync(logFd, d);
        if (verbose) process.stderr.write(d);
      } else {
        process.stderr.write(d);
      }
    });
    child.on("error", () => resolveP(null));
    child.on("close", (code) => {
      if (logFd !== null) closeSync(logFd);
      resolveP(code === 0 ? { status: 0, stdout: out } : null);
    });
  });
}

// A readable quoting for the logged command, in place of printf %q.
function q(s: string): string {
  return /^[A-Za-z0-9_@%+=:,./-]+$/.test(s)
    ? s
    : `"${s.replace(/([\\"$`])/g, "\\$1")}"`;
}

type CheckFn = (...args: string[]) => number | Promise<number>;

/** run <name> <command…>: one check, its output in <logdir>/<name>.log,
 *  without git's repository variables. The command is an argv array, or a
 *  function (its own spawns go through spawnC and land in the same log). */
export async function run(
  name: string,
  cmd: string[] | CheckFn,
  ...args: string[]
): Promise<void> {
  if (hookSkipping(name)) {
    console.log(`  - ${name}: SKIPPED (LOSOS_HOOKS_SKIP)`);
    hookSkipped.push(name);
    return;
  }
  const file = name.replaceAll("/", "_");
  const log = path.join(hookLogDir, `${file}.log`);
  let mark: string | null = null;
  if (hookTreeId) {
    mark = path.join(hookPassedBase, hookTreeId, file);
    if (existsSync(mark)) {
      console.log(
        `  ✓ ${name} (passed on this tree before: ${readFileSync(mark, "utf8").trim()})`,
      );
      hookPassed.push(name);
      return;
    }
  }
  const t0 = nowSec();
  console.log(`  … ${name}`);
  const shown = Array.isArray(cmd) ? cmd : [cmd.name, ...args];
  writeFileSync(log, `# ${name}\n# in ${process.cwd()}\n# ${shown.map(q).join(" ")} \n`);
  sink = { log };
  const rc = Array.isArray(cmd)
    ? await spawnC(cmd)
    : await (async () => {
        try {
          return await cmd(...args);
        } finally {
          sink = null;
        }
      })();
  sink = null;
  const dt = nowSec() - t0;
  if (rc === 0) {
    console.log(`  ✓ ${name} (${dt}s)`);
    hookPassed.push(name);
    if (mark) {
      mkdirSync(path.dirname(mark), { recursive: true });
      writeFileSync(mark, `${log}\n`);
    }
  } else {
    console.log(`  ✗ ${name} (${dt}s, exit ${rc}): ${log}`);
    const tail = readFileSync(log, "utf8").split("\n").slice(-40);
    for (const line of tail) console.error(`      ${line}`);
    hookFailed.push(name);
  }
}

// ── The lanes CI would run ───────────────────────────────────────────────

/** The lanes CI would run for the changed files, as ci-changes.ts prints
 *  them (`<lane>=true|false`, plus its own `full` and `reason`). */
export const lanes: Record<string, boolean> = {};

/** Whether a lane runs; a lane nobody set is off. */
export function laneOn(name: string): boolean {
  return lanes[name] === true;
}

/** Feed the changed files to .github/scripts/ci-changes.ts and slurp its
 *  `key=true` lines into the lanes record. */
export async function hookLanes(files: string): Promise<void> {
  const argv = toolArgv("bun", "bun", [path.join(hookRootDir, ".github/scripts/ci-changes.ts")]);
  if (!argv) return;
  const out = await captureC(argv, { input: files });
  if (!out) return;
  for (const line of out.stdout.split("\n")) {
    const eq = line.indexOf("=");
    if (eq <= 0) continue;
    const value = line.slice(eq + 1);
    if (value === "true" || value === "false") lanes[line.slice(0, eq)] = value === "true";
  }
}

// ── The crates a change reaches ──────────────────────────────────────────

/** The crates a set of changed files reaches, its dependents included: a crate
 *  that depends on another by path is linted and tested with it, as CI does
 *  by running every crate. */
export function hookCratesFor(files: string): string[] {
  const want = new Set<string>();
  for (const f of files.split("\n").filter(Boolean)) {
    for (const c of hookCrates()) {
      if (f.startsWith(`${c}/`)) want.add(c);
    }
    if (f.startsWith("backend/access/")) {
      want.add("backend");
      want.add("edge/registrar");
      want.add("edge/vercel");
      want.add("admin-ui/tui");
    }
    if (f.startsWith("backend/plugins/")) want.add("backend");
    if (f.startsWith("edge/registrar/")) want.add("edge/vercel");
    if (f.startsWith("admin-ui/lab/core/")) want.add("admin-ui/lab/render");
    if (f === "modules/overrides.nix") want.add("backend");
    if (
      f.startsWith("admin-ui/app/src/i18n/") ||
      f === "admin-ui/app/src/styles/tokens.css" ||
      f === "admin-ui/app/src/screens/settings/panes.ts" ||
      f === "admin-ui/themes/brand/salmon-128.png"
    ) {
      want.add("admin-ui/tui");
    }
  }
  // The backend crate's path dependencies live inside it.
  if (want.has("backend/access")) want.add("backend");
  return hookCrates().filter((c) => want.has(c));
}

// ── Finding the tools ────────────────────────────────────────────────────

function findOnPath(prog: string): string | null {
  for (const dir of (process.env.PATH ?? "").split(":")) {
    if (!dir) continue;
    const p = path.join(dir, prog);
    try {
      const st = statSync(p);
      if (st.isFile() && (st.mode & fsConstants.X_OK) !== 0) return p;
    } catch {
      // not here
    }
  }
  return null;
}

// cargo from the flake's pinned toolchain, the rustc CI lints, tests and
// ships with: the one on PATH inside devenv or nix develop, else the flake's
// CI shell. A cargo from elsewhere (rustup) is a different compiler, and its
// clippy can disagree with CI's; it is used only when there is no nix.
export async function hookCargo(args: string[]): Promise<number> {
  const found = findOnPath("cargo");
  if (found?.startsWith("/nix/store/")) return spawnC(["cargo", ...args]);
  if (findOnPath("nix")) return spawnC(["nix", "develop", `${hookFlake()}#ci`, "-c", "cargo", ...args]);
  if (found) {
    console.error(`note: ${found} is not the flake's toolchain; CI may disagree`);
    return spawnC(["cargo", ...args]);
  }
  console.error("cargo is not on PATH and nix is not installed: enter devenv shell or nix develop");
  return 127;
}

// The argv that runs a program from the flake's nixpkgs when it is not on
// PATH, or null when there is no way to run it.
function toolArgv(prog: string, attr: string, args: string[]): string[] | null {
  if (findOnPath(prog)) return [prog, ...args];
  if (findOnPath("nix"))
    return ["nix", "shell", "--inputs-from", hookFlake(), `nixpkgs#${attr}`, "-c", prog, ...args];
  return null;
}

/** hook_tool <program> <nixpkgs attribute> <args…>: a program from the
 *  flake's nixpkgs when it is not on PATH. */
export async function hookTool(prog: string, attr: string, ...args: string[]): Promise<number> {
  const argv = toolArgv(prog, attr, args);
  if (!argv) {
    console.error(`${prog} is not on PATH and nix is not installed`);
    return 127;
  }
  return spawnC(argv);
}

export function hookBun(args: string[]): Promise<number> {
  return hookTool("bun", "bun", ...args);
}

// ── The WebAssembly packages the admin UI typechecks against ─────────────

// The WebAssembly packages admin-ui/app imports and git ignores: Diagrams'
// three (src/lab/{core,virt,render}-pkg) and losos-mdx (src/mdx/pkg). tsc
// needs them, so a checkout without one gets the package the flake builds
// (`bun run lab:<name>` and `bun run mdx` would need cargo for wasm32 and
// the pinned wasm-bindgen). A copy made here names its store path in
// .losos-hooks-out and is replaced once the crate builds to another one,
// since a stale package fails tsc on whatever the crate added since. One
// made by `bun run` has no such file and is left alone.
const wasmPkgs: Array<[string, string]> = [
  ["src/lab/core-pkg", "losos-lab-core"],
  ["src/lab/virt-pkg", "losos-lab-virt"],
  ["src/lab/render-pkg", "losos-lab-render"],
  ["src/mdx/pkg", "losos-mdx-wasm"],
];

export async function hookWasmPkgs(): Promise<number> {
  for (const [rel, attr] of wasmPkgs) {
    const dir = path.join(hookRootDir, "admin-ui/app", rel);
    const stamp = path.join(dir, ".losos-hooks-out");
    if (existsSync(dir) && !existsSync(stamp)) continue;
    if (hookNeedNix() !== 0) return 1;
    const built = await captureC(["nix", "build", "--no-link", "--print-out-paths", `${hookFlake()}#${attr}`]);
    if (!built) return 1;
    const out = built.stdout.trim();
    if (existsSync(stamp) && readFileSync(stamp, "utf8").trim() === out) continue;
    toLog(`── ${rel} from .#${attr}`);
    rmSync(dir, { recursive: true, force: true });
    mkdirSync(dir, { recursive: true });
    cpSync(out, dir, { recursive: true });
    await spawnC(["chmod", "-R", "u+w", dir]);
    writeFileSync(stamp, `${out}\n`);
  }
  return 0;
}

/** The eval and build checks need nix itself. */
export function hookNeedNix(): number {
  if (!findOnPath("nix")) {
    console.error("nix is not installed; this check evaluates or builds the flake");
    return 127;
  }
  return 0;
}

// Make the flake's GitHub-hosted inputs resolvable where GitHub's tarball
// endpoint is not (a proxy that allows git but not codeload): fetch the
// locked rev over git and add it to the store under the locked NAR hash.
// A no-op where the inputs fetch normally or are already in the store.
export async function hookInputs(): Promise<number> {
  if (!findOnPath("python3")) return 0;
  return spawnC(["python3", path.join(hookRootDir, "nix/hooks/flake-inputs.py"), hookRootDir]);
}

// For check functions that spawn more subprocesses of their own (pre-push's
// vm_run): the same log plumbing run() uses.
export const spawnForCheck = spawnC;
export const captureForCheck = captureC;
export const logLine = toLog;

// ── Whole gates ──────────────────────────────────────────────────────────

/** rustfmt --check over every crate, as CI's clippy job does on a PR. */
export async function hookFmtAll(): Promise<number> {
  let rc = 0;
  for (const c of crateManifest()) {
    if (!c.fmt) continue;
    const all = c.fmtAll ? ["--all"] : [];
    toLog(`── ${c.path}`);
    const st = await hookCargo(["fmt", ...all, "--manifest-path", `${c.path}/Cargo.toml`, "--check"]);
    if (st !== 0) rc = 1;
  }
  return rc;
}

/** nixfmt, deadnix and statix over these .nix files: nix/lint-nix.ts, the
 *  script CI's eval job and devenv's lint-nix run over the whole tree. */
export function hookNixLints(files: string[]): Promise<void> {
  if (files.length === 0) return Promise.resolve();
  return run("nix-lints", () => hookBun([path.join(hookRootDir, "nix/lint-nix.ts"), ...files]));
}

/** The eval lane: CI's eval job, every step of it. */
export async function hookEvalChecks(): Promise<void> {
  await run("flake-inputs", hookInputs);
  await run("check-flake", () => hookBun([path.join(hookRootDir, "nix/check-flake.ts")]));
  for (const cfg of ["install", "iso", "edge-gateway"]) {
    await run(`eval-${cfg}`, [
      "nix",
      "eval",
      "--raw",
      `${hookFlake()}#nixosConfigurations.${cfg}.config.system.build.toplevel.drvPath`,
    ]);
  }
  await run("eval-iso-image", [
    "nix",
    "path-info",
    "--derivation",
    `${hookFlake()}#nixosConfigurations.iso.config.system.build.isoImage`,
  ]);
  await run("eval-edge-qcow2", ["nix", "path-info", "--derivation", `${hookFlake()}#losos-disk-edge-qcow2`]);
}
