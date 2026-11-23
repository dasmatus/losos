#!/usr/bin/env bun
// Point this clone's git hooks at nix/hooks (pre-commit, commit-msg,
// pre-push).
//
//   bun nix/hooks/install.ts
//
// devenv's shell, `nix develop` and a Claude Code session in this repository
// run it on entry; anyone else runs it once. It sets `core.hooksPath` in this
// clone's config, which every worktree of the clone shares, and changes
// nothing else. `git config --unset core.hooksPath` undoes it.
import { spawnSync } from "node:child_process";

const root = spawnSync("git", ["rev-parse", "--show-toplevel"], { encoding: "utf8" });
if (root.status !== 0) process.exit(1);
process.chdir((root.stdout ?? "").trim());

const current = spawnSync("git", ["config", "--get", "core.hooksPath"], { encoding: "utf8" });
if ((current.stdout ?? "").trim() !== "nix/hooks") {
  const set = spawnSync("git", ["config", "core.hooksPath", "nix/hooks"], { stdio: "inherit" });
  if (set.status !== 0) process.exit(set.status ?? 1);
  console.log("git hooks: nix/hooks (pre-commit, commit-msg, pre-push)");
}
