#!/usr/bin/env bun
// The Nix lints over the tree, or over the files named on the command line.
import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { resolve } from "node:path";

function run(cmd: string[], opts: { allowFail?: boolean } = {}) {
  const p = spawnSync(cmd[0], cmd.slice(1), { stdio: "inherit" });
  if (p.status !== 0 && !opts.allowFail) process.exit(p.status ?? 1);
  return p.status === 0;
}

function cap(cmd: string[]) {
  return spawnSync(cmd[0], cmd.slice(1), { encoding: "utf8", maxBuffer: 64 * 1024 * 1024 });
}

const rootRes = cap(["git", "rev-parse", "--show-toplevel"]);
const root = rootRes.status === 0 && rootRes.stdout.trim() ? rootRes.stdout.trim() : resolve(import.meta.dir, "..");
process.chdir(root);

let files = Bun.argv.slice(2);
if (files.length === 0) {
  const ls = cap(["git", "ls-files", "--", "*.nix"]);
  files = ls.stdout.split("\n").map((s) => s.trim()).filter(Boolean);
}
if (files.length === 0) process.exit(0);

const has = (name: string) => {
  const p = spawnSync("bash", ["-lc", `command -v ${name}`], { stdio: "ignore" });
  return p.status === 0;
};

if (!has("nixfmt") || !has("deadnix") || !has("statix")) {
  let flake = `git+file://${root}`;
  const shallow = cap(["git", "rev-parse", "--is-shallow-repository"]).stdout.trim();
  if (shallow === "true") flake += "?shallow=1";
  const p = spawnSync(
    "nix",
    [
      "shell",
      "--inputs-from",
      flake,
      "nixpkgs#nixfmt-rfc-style",
      "nixpkgs#deadnix",
      "nixpkgs#statix",
      "-c",
      "bun",
      resolve(root, "nix/lint-nix.ts"),
      ...files,
    ],
    { stdio: "inherit" },
  );
  process.exit(p.status ?? 1);
}

let rc = 0;
console.log(`── nixfmt (${files.length} files)`);
if (!run(["nixfmt", "--check", ...files], { allowFail: true })) rc = 1;
console.log("── deadnix");
if (!run(["deadnix", "--fail", ...files], { allowFail: true })) rc = 1;
console.log("── statix");
for (const f of files) {
  if (!run(["statix", "check", "--config", "nix/statix.toml", f], { allowFail: true })) rc = 1;
}
if (rc === 0) console.log("Nix lints clean");
process.exit(rc);
