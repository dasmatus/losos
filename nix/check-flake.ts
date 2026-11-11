#!/usr/bin/env bun
// check-flake: evaluate every flake output, one `nix eval` per derivation.
// Shim target for nix/check-flake.sh.
import { mkdtempSync, readdirSync, rmSync, writeFileSync, readFileSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";
import { cpus } from "node:os";
import { spawn, spawnSync } from "node:child_process";

function cap(cmd: string[], args: string[] = []) {
  const p = spawnSync(cmd[0], [...cmd.slice(1), ...args], { encoding: "utf8", maxBuffer: 128 * 1024 * 1024 });
  if (p.status !== 0) throw new Error((p.stderr || p.stdout || `${cmd.join(" ")} failed`).trim());
  return p.stdout;
}

const rootRes = spawnSync("git", ["rev-parse", "--show-toplevel"], { encoding: "utf8" });
const root = rootRes.status === 0 && rootRes.stdout.trim() ? rootRes.stdout.trim() : resolve(import.meta.dir, "..");
process.chdir(root);

let flake = `git+file://${process.cwd()}`;
const shallow = spawnSync("git", ["rev-parse", "--is-shallow-repository"], { encoding: "utf8" }).stdout.trim();
if (shallow === "true") flake += "?shallow=1";

const extra = Bun.argv.slice(2);
const jobs = Number(Bun.env.CHECK_FLAKE_JOBS || String(cpus().length || 4));
const logs = mkdtempSync(join(tmpdir(), "check-flake-"));
process.on("exit", () => rmSync(logs, { recursive: true, force: true }));

function names(expr: string): string[] {
  const raw = cap(["nix", "eval", ...extra, "--raw", "--apply", 'xs: builtins.concatStringsSep "\\n" (builtins.attrNames xs)', expr]);
  return raw.split("\n").map((s) => s.trim()).filter(Boolean);
}

let active = 0;
const queue: Array<() => void> = [];
function limit<T>(fn: () => Promise<T>): Promise<T> {
  return new Promise((resolveP, rejectP) => {
    const run = () => {
      active++;
      fn().then(resolveP, rejectP).finally(() => {
        active--;
        queue.shift()?.();
      });
    };
    if (active < jobs) run();
    else queue.push(run);
  });
}

async function check(label: string, args: string[]): Promise<void> {
  await limit(async () => {
    console.log(`checking ${label}...`);
    const logPath = join(logs, `${label}.log`);
    const p = spawn("nix", ["eval", ...args], { stdio: ["ignore", "ignore", "pipe"] });
    const chunks: Buffer[] = [];
    p.stderr.on("data", (c) => chunks.push(c));
    const code: number = await new Promise((r) => p.on("close", (c) => r(c ?? 1)));
    writeFileSync(logPath, Buffer.concat(chunks));
    if (code !== 0) writeFileSync(join(logs, `${label}.failed`), "");
  });
}

const outputsRaw = cap([
  "nix",
  "eval",
  "--impure",
  "--raw",
  "--expr",
  `builtins.concatStringsSep "\\n" (builtins.attrNames (builtins.getFlake "${flake}").outputs)`,
]);
const outputs = outputsRaw.split("\n").map((s) => s.trim()).filter(Boolean);
const tasks: Promise<void>[] = [];

for (const output of outputs) {
  if (output === "checks" || output === "packages" || output === "devShells") {
    for (const system of names(`${flake}#${output}`)) {
      for (const name of names(`${flake}#${output}.${system}`)) {
        tasks.push(check(`${output}.${system}.${name}`, [...extra, "--raw", `${flake}#${output}.${system}."${name}".drvPath`]));
      }
    }
  } else if (output === "nixosConfigurations") {
    for (const name of names(`${flake}#${output}`)) {
      tasks.push(
        check(`${output}.${name}`, [
          ...extra,
          `${flake}#${output}."${name}".config.system.build.toplevel`,
          "--apply",
          'd: if d.type or null == "derivation" then true else throw "toplevel is not a derivation"',
        ]),
      );
    }
  } else if (output === "nixosModules") {
    for (const name of names(`${flake}#${output}`)) {
      tasks.push(
        check(`${output}.${name}`, [
          ...extra,
          `${flake}#${output}."${name}"`,
          "--apply",
          'm: if builtins.isFunction m || builtins.isAttrs m then true else throw "not a module"',
        ]),
      );
    }
  } else {
    writeFileSync(join(logs, `${output}.log`), `no check for the output '${output}'; add one to nix/check-flake.sh\n`);
    writeFileSync(join(logs, `${output}.failed`), "");
  }
}

await Promise.all(tasks);

let failed = 0;
for (const ent of readdirSync(logs)) {
  if (!ent.endsWith(".failed")) continue;
  const label = basename(ent, ".failed");
  process.stderr.write(`check-flake: ${label} failed:\n`);
  try { process.stderr.write(readFileSync(join(logs, `${label}.log`), "utf8")); } catch {}
  failed++;
}
if (failed > 0) {
  process.stderr.write(`check-flake: ${failed} output(s) failed to evaluate\n`);
  process.exit(1);
}
console.log("check-flake: every output evaluated");
