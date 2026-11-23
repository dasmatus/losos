#!/usr/bin/env bun
// check-crates: crates.json covers every Cargo.toml in the tree.
//
//   bun nix/check-crates.ts
//
// This repo is deliberately not a Cargo workspace
// (docs/decisions/0001-not-a-cargo-workspace.md), so "which crates are
// there" used to be typed by hand in devenv.nix and nix/hooks/lib.ts, and
// both lists could drift from the tree — a new crate linted nowhere, a
// removed one failing every script that named it. crates.json is now the
// one list and devenv.nix and the hooks read theirs from it; this script
// is the gate the list itself answers to. devenv's `check-crates`, the
// pre-commit hook (when crates.json or a Cargo.toml is staged) and CI's
// `pins` job all run it.
//
//   every manifest named in crates.json exists and carries a Cargo.lock
//   (the vendored tarball's hash is pinned against it in nix/packages.nix)
//   every tracked Cargo.toml is either a listed crate or a member of a
//   listed workspace
//   every member of a listed workspace has its Cargo.toml

import { spawnSync } from "node:child_process";
import { readdirSync, readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";

const root = resolve(dirname(import.meta.path), "..");
process.chdir(root);

type Crate = {
  path: string;
  fmt: boolean;
  fmtAll?: boolean;
  test: boolean;
  why?: string;
};

const errors: string[] = [];

let crates: Crate[];
try {
  crates = (JSON.parse(readFileSync("crates.json", "utf8")) as { crates: Crate[] }).crates;
} catch (e) {
  console.error(`crates.json does not parse: ${e}`);
  process.exit(1);
}
if (!Array.isArray(crates) || crates.length === 0) {
  console.error("crates.json has no `crates` array");
  process.exit(1);
}

// The paths the manifest owns: the listed crates, plus the members of any
// listed workspace (members are enumerated from the workspace's own
// Cargo.toml, so a crate can join it without touching this file — but a
// manifest not inside any listed workspace must be listed itself, which is
// the point: backend is a package, not a workspace root, so a stray
// backend/foo would otherwise hide behind backend's path).
const owned = new Set<string>();
for (const c of crates) {
  if (typeof c.path !== "string" || c.path.length === 0) {
    errors.push("an entry without a `path`");
    continue;
  }
  if (owned.has(c.path)) errors.push(`${c.path} is listed twice`);
  owned.add(c.path);
  if (typeof c.fmt !== "boolean" || typeof c.test !== "boolean") {
    errors.push(`${c.path} needs boolean "fmt" and "test"`);
  }
  for (const f of ["Cargo.toml", "Cargo.lock"]) {
    try {
      readFileSync(`${c.path}/${f}`);
    } catch {
      errors.push(`${c.path}/${f} is missing`);
    }
  }
}

for (const c of crates) {
  if (!owned.has(c.path)) continue; // already reported
  let toml: string;
  try {
    toml = readFileSync(`${c.path}/Cargo.toml`, "utf8");
  } catch {
    continue;
  }
  if (!/\[workspace\]/.test(toml)) continue;
  if (!/^members\s*=/m.test(toml)) continue; // a single-crate workspace: it covers itself
  const m = toml.match(/^members\s*=\s*\[([^\]]*)\]/m);
  if (!m) {
    errors.push(`${c.path}/Cargo.toml is a workspace but crates.json cannot read its members`);
    continue;
  }
  for (const raw of m[1].split(",")) {
    const mem = raw.trim().replace(/^["']|["']$/g, "");
    if (mem.length === 0) continue;
    if (mem.includes("*")) {
      // A glob member covers every direct child directory with a manifest.
      for (const e of readdirSync(c.path, { withFileTypes: true })) {
        if (!e.isDirectory()) continue;
        const dir = `${c.path}/${e.name}`;
        try {
          readFileSync(`${dir}/Cargo.toml`);
          owned.add(dir);
        } catch {
          // no manifest, not a member
        }
      }
      continue;
    }
    const dir = `${c.path}/${mem.replace(/\/+$/, "")}`;
    if (owned.has(dir)) continue;
    owned.add(dir);
    try {
      readFileSync(`${dir}/Cargo.toml`);
    } catch {
      errors.push(`${c.path} lists member ${mem}, which has no Cargo.toml`);
    }
  }
}

// Every tracked manifest must be owned.
const ls = spawnSync("git", ["ls-files", "-z", "*Cargo.toml"], { encoding: "utf8" });
if (ls.status !== 0) {
  console.error("git ls-files failed; run this inside a checkout");
  process.exit(1);
}
for (const f of ls.stdout.split("\0").filter(Boolean)) {
  const dir = f.slice(0, -"Cargo.toml".length).replace(/\/$/, "") || ".";
  if (owned.has(dir)) continue;
  errors.push(
    `${f} is not covered by crates.json: add its directory (or its workspace's member list) there`,
  );
}

if (errors.length > 0) {
  console.error("crates.json does not cover the tree:");
  for (const e of errors) console.error(`  ${e}`);
  console.error("crates.json is devenv.nix's and the git hooks' one crate list;");
  console.error("fix the manifest, not a copy of it elsewhere.");
  process.exit(1);
}

console.log(
  `crates.json covers every Cargo.toml in the tree ` +
    `(${crates.length} crates, ${owned.size} manifests)`,
);
