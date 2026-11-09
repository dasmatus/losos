#!/usr/bin/env bun
// Point LosOS Desktop's losos client and its box test at a release tag.

import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { resolve } from "node:path";
import { spawnSync } from "node:child_process";

function run(cmd: string[], opts: { cwd?: string; allowFail?: boolean } = {}) {
  const p = spawnSync(cmd[0], cmd.slice(1), { cwd: opts.cwd, encoding: "utf8", maxBuffer: 128 * 1024 * 1024 });
  if (p.error) throw p.error;
  if (p.status !== 0 && !opts.allowFail) throw new Error(p.stderr || p.stdout || `${cmd.join(" ")} failed`);
  return p;
}

if (Bun.argv.length !== 4) {
  process.stderr.write("usage: desktop-pin.ts TAG DESKTOP_DIR\n");
  process.exit(2);
}
const tag = Bun.argv[2];
const desktop = resolve(Bun.argv[3]);
const pin = resolve(desktop, "nixos/pkgs/losos-pin.nix");
const system = "x86_64-linux";
const placeholder = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";

if (!/^v[0-9]+(\.[0-9]+)*(-[0-9A-Za-z.-]+)?$/.test(tag)) {
  process.stderr.write(`${tag} is not a release tag (vX.Y.Z)\n`);
  process.exit(2);
}
const rev = run(["git", "rev-parse", `${tag}^{commit}`]).stdout.trim();
if (!existsSync(pin)) {
  process.stderr.write(`${pin} is missing: is ${desktop} a checkout of LosOS Desktop?\n`);
  process.exit(1);
}

function setValue(text: string, key: string, value: string): string {
  const re = new RegExp(`^  ${key} = "[^"]*";$`, "m");
  if (!re.test(text)) {
    process.stderr.write(`${pin} has no ${key} line\n`);
    process.exit(1);
  }
  return text.replace(re, `  ${key} = "${value}";`);
}

let text = readFileSync(pin, "utf8");
if (new RegExp(`^  rev = "${rev.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}";$`, "m").test(text)) {
  console.log(`LosOS Desktop already pins ${tag} (${rev}).`);
  process.exit(0);
}

for (const [k, v] of [
  ["rev", rev],
  ["version", tag.replace(/^v/, "")],
  ["release", tag],
  ["hash", placeholder],
  ["cargoHash", placeholder],
  ["boxHash", placeholder],
] as const) text = setValue(text, k, v);
writeFileSync(pin, text);

function reported(attr: string): string {
  const p = spawnSync(
    "nix",
    ["build", "--no-link", "--print-build-logs", `path:${desktop}#legacyPackages.${system}.losos.${attr}`],
    { encoding: "utf8", maxBuffer: 128 * 1024 * 1024 },
  );
  const log = (p.stdout || "") + (p.stderr || "");
  const got = [...log.matchAll(/^ *got: *(sha256-[A-Za-z0-9+/=]*)$/gm)].at(-1)?.[1];
  if (!got) {
    process.stderr.write(`${attr} did not report a hash:\n${log}\n`);
    process.exit(1);
  }
  return got;
}

for (const [k, attr] of [["hash", "src"], ["boxHash", "box"], ["cargoHash", "cargoDeps"]] as const) {
  text = readFileSync(pin, "utf8");
  writeFileSync(pin, setValue(text, k, reported(attr)));
}
process.stdout.write(readFileSync(pin, "utf8"));
