#!/usr/bin/env bun
// Driver for box-edge-box demo recording. Uses local record.ts.
import { spawn, spawnSync } from "node:child_process";
import { mkdirSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
const check = Bun.argv[2] || "losos-box-edge-box";
const here = dirname(import.meta.path);
const repo = resolve(here, "../../..");
const out = resolve(Bun.argv[3] || join(here, "recording"));
mkdirSync(out, { recursive: true });
const nixArgs = (Bun.env.LOSOS_NIX_ARGS || "").trim() ? (Bun.env.LOSOS_NIX_ARGS || "").trim().split(/\s+/) : [];
for (const f of readdirSync(out)) if (/^(ack-|timeline\.jsonl$|shots\.jsonl$|vm-finished$)|\.(token|port|key|png)$/.test(f)) try { rmSync(join(out, f)); } catch {}
const log = (m: string) => console.error(`\x1b[1;36m[${new Date().toTimeString().slice(0,8)}]\x1b[0m ${m}`);
log(`building the demo driver`);
let r = spawnSync("nix", ["build", ...nixArgs, "-o", join(out, "driver"), `${repo}#checks.x86_64-linux.${check}.driver`], { stdio: "inherit" });
if (r.status !== 0) process.exit(r.status ?? 1);
log("starting the recorder");
const recorder = spawn("bun", [join(here, "record.ts")], { env: { ...Bun.env, LOSOS_RECORD_DIR: out }, stdio: "ignore", cwd: out });
log(`running the demo (log in ${out}/driver.log)`);
const driver = spawnSync(join(out, "driver/bin/nixos-test-driver"), ["--no-interactive", "-o", out], { cwd: out, env: { ...Bun.env, LOSOS_RECORD_DIR: out }, encoding: "buffer", maxBuffer: 256*1024*1024 });
writeFileSync(join(out, "driver.log"), Buffer.concat([driver.stdout||Buffer.alloc(0), driver.stderr||Buffer.alloc(0)]));
writeFileSync(join(out, "vm-finished"), "");
await new Promise((res) => recorder.on("close", res)).catch(() => {});
if (driver.status !== 0) { log(`the demo FAILED (exit ${driver.status}); see ${out}/driver.log`); process.exit(driver.status ?? 1); }
log(`done: ${readdirSync(out).filter(f=>f.endsWith(".png")).length} pictures, captions in ${out}/shots.jsonl`);
