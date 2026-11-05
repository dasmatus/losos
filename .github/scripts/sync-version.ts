#!/usr/bin/env bun
// Put the newest release tag into nix/version.nix on main.
//   sync-version.ts            commit the tag to main through the GitHub API
//   sync-version.ts --dry-run  say what it would commit, change nothing
//
// Behavior-compatible shim target for .github/scripts/sync-version.sh.

import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
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
  // Match GNU sort -V enough for vX.Y.Z tags by splitting numeric parts.
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

const arg = Bun.argv[2] ?? "";
let dryRun = false;
if (arg === "--dry-run") dryRun = true;
else if (arg !== "") {
  process.stderr.write("usage: sync-version.sh [--dry-run]\n");
  process.exit(2);
}

let top = "";
const topRes = spawnSync("git", ["rev-parse", "--show-toplevel"], { encoding: "utf8" });
if (topRes.status === 0 && topRes.stdout.trim()) top = topRes.stdout.trim();
else top = resolve(import.meta.dir, "../..");
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

const readValue = (path: string) => {
  const m = readFileSync(path, "utf8").match(/^"(.*)"$/m);
  return m?.[1] ?? "";
};

const tmp = mkdtempSync(join(tmpdir(), "sync-version-"));
const nextPath = join(tmp, "version.nix");
try {
  for (let attempt = 1; attempt <= 3; attempt++) {
    const current = readValue(file);
    if (current === newest) {
      console.log(`${file} already says ${newest}`);
      process.exit(0);
    }
    if (current && versionSort([current, newest]).at(-1) === current) {
      console.log(`${file} says ${current}, ahead of the newest tag ${newest}; leaving it`);
      process.exit(0);
    }

    const next = readFileSync(file, "utf8").replace(/^".*"$/m, `"${newest}"`);
    writeFileSync(nextPath, next);
    if (readValue(nextPath) !== newest) {
      process.stderr.write(`::error::could not find the quoted version line in ${file}\n`);
      process.exit(1);
    }
    console.log(`${file}: ${current || "nothing"} -> ${newest}`);
    if (dryRun) {
      const diff = run(["diff", "-u", "--label", `a/${file}`, "--label", `b/${file}`, file, nextPath], { allowFail: true });
      if (diff.stdout) process.stdout.write(diff.stdout);
      process.exit(0);
    }

    const head = run(["git", "rev-parse", "HEAD"]).stdout.trim();
    const repo = Bun.env.REPO ?? "";
    const body = JSON.stringify({
      query:
        "mutation($input: CreateCommitOnBranchInput!) { createCommitOnBranch(input: $input) { commit { oid url } } }",
      variables: {
        input: {
          branch: { repositoryNameWithOwner: repo, branchName: "main" },
          expectedHeadOid: head,
          message: {
            headline: `Set the LosOS version to ${newest}`,
            body: `${newest} is the newest release tag. Boxes built from main show it on the boot menus, the boot screen, os-release and the console banners.`,
          },
          fileChanges: {
            additions: [{ path: file, contents: Buffer.from(next).toString("base64") }],
          },
        },
      },
    });
    const gh = spawnSync("gh", ["api", "graphql", "--input", "-"], {
      input: body,
      encoding: "utf8",
      maxBuffer: 64 * 1024 * 1024,
    });
    if (gh.status === 0) {
      const out = JSON.parse(gh.stdout || "{}");
      console.log(`committed: ${out?.data?.createCommitOnBranch?.commit?.url ?? ""}`);
      if (Bun.env.GITHUB_OUTPUT) {
        writeFileSync(Bun.env.GITHUB_OUTPUT, `committed=${newest}\n`, { flag: "a" });
      }
      process.exit(0);
    }
    process.stderr.write(`attempt ${attempt} refused: ${(gh.stderr || gh.stdout || "").trim()}\n`);
    run(["git", "fetch", "-q", "--depth", "1", "origin", "main"]);
    run(["git", "checkout", "-q", "--detach", "FETCH_HEAD"]);
  }
} finally {
  rmSync(tmp, { recursive: true, force: true });
}

process.stderr.write(
  `::error::could not commit ${newest} to ${file} on main. If the refusal above names a rule, the repository's ruleset has to let GitHub Actions commit to main (signed, fast-forward only).\n`,
);
process.exit(1);
