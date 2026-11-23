#!/usr/bin/env bun
// Write the static copy of the LosOS binary cache for GitHub Pages.
import { appendFileSync, chmodSync, existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, unlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";

function sh(cmd: string[], opts: { allowFail?: boolean } = {}) {
  const p = spawnSync(cmd[0], cmd.slice(1), { stdio: "inherit" });
  if (p.status !== 0 && !opts.allowFail) process.exit(p.status ?? 1);
  return p;
}
function cap(cmd: string[]) {
  return spawnSync(cmd[0], cmd.slice(1), { encoding: "utf8", maxBuffer: 64 * 1024 * 1024 });
}

const args = Bun.argv.slice(2);
if (args.length < 2) {
  process.stderr.write("usage: pages-cache.ts <out-dir> <installable>...\n");
  process.exit(2);
}
let out = resolve(args[0]);
const installables = args.slice(1);
const key = Bun.env.SIGNING_KEY;
if (!installables.length) { process.stderr.write("pages-cache: no installables\n"); process.exit(2); }
if (!key) { process.stderr.write("pages-cache: SIGNING_KEY is empty\n"); process.exit(1); }
const budget = Number(Bun.env.PAGES_CACHE_BUDGET_MIB || "800") * 1024 * 1024;
const summary = Bun.env.GITHUB_STEP_SUMMARY || "/dev/stderr";
const work = mkdtempSync(join(tmpdir(), "pages-cache-"));
process.on("exit", () => rmSync(work, { recursive: true, force: true }));
writeFileSync(join(work, "cache.sec"), key, { mode: 0o600 });

rmSync(out, { recursive: true, force: true });
mkdirSync(out, { recursive: true });
out = resolve(out);

sh(["nix", "copy", "--no-check-sigs", "--to", `file://${out}?compression=zstd&parallel-compression=true&secret-key=${work}/cache.sec`, ...installables]);
writeFileSync(join(out, "nix-cache-info"), "StoreDir: /nix/store\nWantMassQuery: 1\nPriority: 45\n");

function drop(hash: string) {
  const narinfo = join(out, `${hash}.narinfo`);
  if (!existsSync(narinfo)) return;
  const url = readFileSync(narinfo, "utf8").match(/^URL: (.*)$/m)?.[1];
  if (url) try { unlinkSync(join(out, url)); } catch {}
  try { unlinkSync(narinfo); } catch {}
}

const narinfos = readdirSync(out).filter((f) => f.endsWith(".narinfo")).map((f) => f.slice(0, -".narinfo".length));
writeFileSync(join(work, "all"), narinfos.join("\n") + (narinfos.length ? "\n" : ""));
const upstream: string[] = [];
for (const hash of narinfos) {
  const r = cap(["curl", "-s", "-o", "/dev/null", "-w", "%{http_code}", "-I", `https://cache.nixos.org/${hash}.narinfo`]);
  if (r.stdout.trim() === "200") { upstream.push(hash); drop(hash); }
}
writeFileSync(join(work, "upstream"), upstream.join("\n") + (upstream.length ? "\n" : ""));

type Entry = { size: number; hash: string; name: string };
const sizes: Entry[] = [];
for (const f of readdirSync(out)) {
  if (!f.endsWith(".narinfo")) continue;
  const txt = readFileSync(join(out, f), "utf8");
  const size = Number(txt.match(/^FileSize: (.*)$/m)?.[1] || 0);
  const hash = f.slice(0, -".narinfo".length);
  const name = txt.match(/^StorePath: \/nix\/store\/(.*)$/m)?.[1] || hash;
  sizes.push({ size, hash, name });
}
sizes.sort((a, b) => b.size - a.size);
let total = sizes.reduce((s, e) => s + e.size, 0);
const dropped: string[] = [];
for (const e of sizes) {
  if (total <= budget) break;
  drop(e.hash);
  total -= e.size;
  dropped.push(`${e.name} (${Math.floor(e.size / 1024 / 1024)} MiB)`);
}

const kept = readdirSync(out).filter((f) => f.endsWith(".narinfo")).length;
let md = "## Pages copy of the binary cache\n\n";
md += `${kept} path(s), ${Math.floor(total / 1024 / 1024)} MiB of NARs; ${upstream.length} left to cache.nixos.org.\n`;
if (dropped.length) {
  md += `\nOver the ${Math.floor(budget / 1024 / 1024)} MiB budget, so these are only on the LosOS proxy:\n`;
  for (const d of dropped) md += `- ${d}\n`;
}
appendFileSync(summary, md);
if (dropped.length) console.log(`::warning::the Pages copy of the cache left out ${dropped.length} path(s) over its size budget; see the step summary`);
