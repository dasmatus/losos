#!/usr/bin/env bun
// Tie every LosOS version to the newest release tag.
//
//   bun .github/scripts/sync-version.ts              commit tag + manifests to main via the GitHub API
//   bun .github/scripts/sync-version.ts --dry-run    show what would change, change nothing
//   bun .github/scripts/sync-version.ts --write      update files in the working tree, no commit
//
// Behavior matches the #510 version-sync contract (nix/version.nix + first-party
// Cargo.toml/package.json manifests).

import { readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { join, relative, resolve } from "node:path";
import { spawnSync } from "node:child_process";

function run(cmd: string[], opts: { cwd?: string; input?: string; allowFail?: boolean } = {}) {
  const p = spawnSync(cmd[0], cmd.slice(1), {
    cwd: opts.cwd,
    input: opts.input,
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
  });
  if (p.error) throw p.error;
  if (p.status !== 0 && !opts.allowFail) {
    throw Object.assign(new Error(`${cmd.join(" ")} failed: ${p.stderr || p.stdout}`), { result: p });
  }
  return p;
}

function versionSort(lines: string[]): string[] {
  return [...lines].sort((a, b) => {
    const pa = a.replace(/^v/, "").split(/[.-]/);
    const pb = b.replace(/^v/, "").split(/[.-]/);
    const n = Math.max(pa.length, pb.length);
    for (let i = 0; i < n; i++) {
      const x = pa[i] ?? "";
      const y = pb[i] ?? "";
      const nx = /^\d+$/.test(x) ? Number(x) : NaN;
      const ny = /^\d+$/.test(y) ? Number(y) : NaN;
      if (!Number.isNaN(nx) && !Number.isNaN(ny)) {
        if (nx !== ny) return nx - ny;
      } else if (x !== y) return x < y ? -1 : 1;
    }
    return 0;
  });
}

function walk(dir: string, name: string, out: string[] = []): string[] {
  for (const ent of readdirSync(dir, { withFileTypes: true })) {
    if (ent.name === ".git") continue;
    const p = join(dir, ent.name);
    if (ent.isDirectory()) {
      if (ent.name === "target") continue;
      walk(p, name, out);
    } else if (ent.isFile() && ent.name === name) out.push(p);
  }
  return out;
}

function excluded(p: string): boolean {
  const parts = p.split(/[/\\]/);
  return parts.includes("vendor") || parts.includes("node_modules") || parts.includes("target");
}

const arg = Bun.argv[2] ?? "";
let mode: "commit" | "dry" | "write" = "commit";
if (arg === "--dry-run") mode = "dry";
else if (arg === "--write") mode = "write";
else if (arg !== "") {
  process.stderr.write("usage: sync-version.ts [--dry-run|--write]\n");
  process.exit(2);
}

const topRes = spawnSync("git", ["rev-parse", "--show-toplevel"], { encoding: "utf8" });
const top = topRes.status === 0 && topRes.stdout.trim() ? topRes.stdout.trim() : resolve(import.meta.dir, "../..");
process.chdir(top);
const file = "nix/version.nix";
const tagShape = /^v[0-9]+(\.[0-9]+)*$/;

const refs = run(["git", "ls-remote", "--tags", "--refs", "origin", "v*"]).stdout;
const tags = refs
  .split("\n")
  .map((l) => l.replace(/.*refs\/tags\//, "").trim())
  .filter((t) => tagShape.test(t));
const newest = versionSort(tags).at(-1) ?? "";
if (!newest) {
  console.log("no release tag on the remote; nothing to do");
  process.exit(0);
}

const semver = newest.startsWith("v") ? newest.slice(1) : newest;
const readValue = (path: string) => (readFileSync(path, "utf8").match(/^"(.*)"$/m)?.[1] ?? "");

function rewrite(tag: string): string[] {
  const sem = tag.startsWith("v") ? tag.slice(1) : tag;
  const changed: string[] = [];
  // nix/version.nix
  {
    const p = file;
    const s = readFileSync(p, "utf8");
    const n = s.replace(/^".*"$/m, `"${tag}"`);
    if (n !== s) { writeFileSync(p, n); changed.push(p); }
  }
  // Cargo.toml: only line-start `version = "..."` (package/workspace.package)
  const cargoVer = /^version\s*=\s*"[^"]*"/gm;
  for (const abs of walk(".", "Cargo.toml").sort()) {
    const p = relative(".", abs);
    if (excluded(p)) continue;
    const s = readFileSync(p, "utf8");
    const n = s.replace(cargoVer, `version = "${sem}"`);
    if (n !== s) { writeFileSync(p, n); changed.push(p); }
  }
  // package.json: first-party only, replace "version" field
  const pkgVer = /^(\s*"version"\s*:\s*)"[^"]*"/m;
  for (const abs of walk(".", "package.json").sort()) {
    const p = relative(".", abs);
    if (excluded(p)) continue;
    const s = readFileSync(p, "utf8");
    if (!pkgVer.test(s)) continue;
    const n = s.replace(pkgVer, `$1"${sem}"`);
    if (n !== s) { writeFileSync(p, n); changed.push(p); }
  }
  return changed.sort();
}

for (let attempt = 1; attempt <= 3; attempt++) {
  const current = readValue(file);
  let changed: string[] = [];
  if (current === newest) {
    changed = rewrite(newest);
    if (changed.length === 0) {
      console.log(`${file} and manifests already at ${newest} (${semver})`);
      process.exit(0);
    }
    console.log(`manifests were out of sync with ${newest}; updated:`);
    for (const c of changed) console.log(c);
  } else {
    if (current && versionSort([current, newest]).at(-1) === current) {
      console.log(`${file} says ${current}, ahead of the newest tag ${newest}; leaving it`);
      process.exit(0);
    }
    changed = rewrite(newest);
    console.log(`${file}: ${current || "nothing"} -> ${newest} (manifests -> ${semver})`);
    for (const c of changed) console.log(c);
  }

  if (mode === "dry") {
    const diff = run(["git", "--no-pager", "diff", "--", "."], { allowFail: true });
    if (diff.stdout) process.stdout.write(diff.stdout);
    run(["git", "checkout", "-q", "--", "."]);
    process.exit(0);
  }
  if (mode === "write") {
    console.log(`wrote version ${newest} / ${semver} to the working tree`);
    process.exit(0);
  }

  const files = run(["git", "diff", "--name-only"]).stdout.split("\n").map((s) => s.trim()).filter(Boolean);
  if (files.length === 0) { console.log("nothing to commit"); process.exit(0); }
  const head = run(["git", "rev-parse", "HEAD"]).stdout.trim();
  const repo = Bun.env.REPO ?? "";
  const additions = files.map((p) => ({ path: p, contents: Buffer.from(readFileSync(p)).toString("base64") }));
  const body = JSON.stringify({
    query: "mutation($input: CreateCommitOnBranchInput!) { createCommitOnBranch(input: $input) { commit { oid url } } }",
    variables: {
      input: {
        branch: { repositoryNameWithOwner: repo, branchName: "main" },
        expectedHeadOid: head,
        message: {
          headline: `Set the LosOS version to ${newest}`,
          body: `${newest} is the newest release tag. Manifests and nix/version.nix show it (boot menus, boot screen, os-release, console banners, crates, packages).`,
        },
        fileChanges: { additions },
      },
    },
  });
  const out = run(["gh", "api", "graphql", "--input", "-"], { input: body, allowFail: true });
  if (out.status === 0) {
    const j = JSON.parse(out.stdout);
    const errs = j.errors;
    if (!errs?.length && j.data?.createCommitOnBranch?.commit?.oid) {
      console.log(j.data.createCommitOnBranch.commit.url || j.data.createCommitOnBranch.commit.oid);
      process.exit(0);
    }
    process.stderr.write(`attempt ${attempt} refused: ${out.stdout}\n`);
  } else {
    process.stderr.write(`attempt ${attempt} refused: ${out.stderr || out.stdout}\n`);
  }
  run(["git", "fetch", "-q", "--depth", "1", "origin", "main"]);
  run(["git", "checkout", "-q", "--detach", "FETCH_HEAD"]);
}

process.stderr.write(`::error::could not commit ${newest} to ${file}/manifests on main. If the refusal above names a rule, the repository's ruleset has to let GitHub Actions commit to main (signed, fast-forward only).\n`);
process.exit(1);
