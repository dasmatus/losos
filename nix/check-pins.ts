#!/usr/bin/env bun
// check-pins: flake.lock, devenv.yaml and devenv.lock pin the same nixpkgs.
//
//   bun nix/check-pins.ts
//   bash nix/check-pins.sh   (shim; used by devenv/CI/hooks)
//
// Mirrors nix/check-pins.sh behavior/output exactly: devenv runs standalone,
// so two lock files pin nixpkgs and must agree (see script header in
// check-pins.sh).

import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";

function rev(lockPath: string): string {
  const data = JSON.parse(readFileSync(lockPath, "utf8")) as {
    nodes?: { nixpkgs?: { locked?: { rev?: string } } };
  };
  const value = data.nodes?.nixpkgs?.locked?.rev;
  if (typeof value !== "string" || value.length === 0) {
    throw new Error(`nixpkgs rev not found in ${lockPath}`);
  }
  return value;
}

const root = resolve(dirname(import.meta.path), "..");
process.chdir(root);

const lock = rev("flake.lock");
const yamlMatch = readFileSync("devenv.yaml", "utf8").match(/[0-9a-f]{40}/);
const yaml = yamlMatch?.[0] ?? "";
const dlock = rev("devenv.lock");

if (lock !== yaml || lock !== dlock) {
  process.stderr.write("nixpkgs pins disagree:\n");
  process.stderr.write(`  flake.lock  ${lock}\n`);
  process.stderr.write(`  devenv.yaml ${yaml}\n`);
  process.stderr.write(`  devenv.lock ${dlock}\n`);
  process.stderr.write(
    "Bump all three together: nix flake update nixpkgs, copy the rev into\n",
  );
  process.stderr.write("devenv.yaml, then devenv update.\n");
  process.exit(1);
}

console.log(
  `nixpkgs pin agrees across flake.lock, devenv.yaml and devenv.lock (${lock})`,
);
