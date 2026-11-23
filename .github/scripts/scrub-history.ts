#!/usr/bin/env bun
// Rewrite history in a bare clone to scrub Claude/session credits.
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";

function git(repo: string, args: string[], opts: { allowFail?: boolean } = {}) {
  const p = spawnSync("git", ["-C", repo, ...args], { encoding: "utf8", maxBuffer: 128 * 1024 * 1024 });
  if (p.status !== 0 && !opts.allowFail) throw new Error(p.stderr || p.stdout || `git ${args.join(" ")} failed`);
  return p;
}

if (Bun.argv.length !== 4) {
  process.stderr.write("usage: scrub-history.ts <bare-clone> <report-file>\n");
  process.exit(2);
}
const repo = Bun.argv[2];
const report = Bun.argv[3];
const here = dirname(Bun.argv[1] === "bun" ? Bun.argv[2] : import.meta.path); // fallback-safe
const scriptDir = import.meta.dir;
const work = mkdtempSync(join(tmpdir(), "scrub-history-"));
process.on("exit", () => rmSync(work, { recursive: true, force: true }));
const mailmap = join(work, "mailmap");
const replacements = join(work, "replacements");

writeFileSync(join(work, "before"), git(repo, ["for-each-ref", "--format=%(objectname) %(refname)"]).stdout);

const privateFile = Bun.env.SCRUB_PRIVATE_RULES || "";
if (privateFile) {
  const lines = readFileSync(privateFile, "utf8").split("\n");
  let section = "";
  const sections: Record<string, string[]> = { paths: [], text: [], messages: [] };
  for (const raw of lines) {
    if (/^\s*(#|$)/.test(raw)) continue;
    const m = raw.match(/^\[(paths|text|messages)\]$/);
    if (m) { section = m[1]; continue; }
    if (/^\[/.test(raw)) { process.stderr.write(`unknown section ${raw}\n`); process.exit(1); }
    if (!section) { process.stderr.write(`rule outside a section: ${raw}\n`); process.exit(1); }
    sections[section].push(raw);
  }
  const args: string[] = [];
  if (sections.paths.length) { writeFileSync(join(work, "private-paths"), sections.paths.join("\n") + "\n"); args.push("--invert-paths", "--paths-from-file", join(work, "private-paths")); }
  if (sections.text.length) { writeFileSync(join(work, "private-text"), sections.text.join("\n") + "\n"); args.push("--replace-text", join(work, "private-text")); }
  if (sections.messages.length) { writeFileSync(join(work, "private-messages"), sections.messages.join("\n") + "\n"); args.push("--replace-message", join(work, "private-messages")); }
  if (args.length) git(repo, ["filter-repo", "--force", "--prune-empty", "auto", "--prune-degenerate", "auto", ...args]);
}

writeFileSync(mailmap, "Matus Mastena <330471626+dasmatus@users.noreply.github.com> Claude <noreply@anthropic.com>\n");
writeFileSync(replacements, "regex:https://claude\\.ai/code/[^\\s)]+==>\n");

const callback = readFileSync(join(scriptDir, "scrub-message.py"), "utf8");
git(repo, [
  "filter-repo", "--force", "--prune-empty", "never", "--prune-degenerate", "never",
  "--mailmap", mailmap, "--replace-text", replacements, "--message-callback", callback,
]);

writeFileSync(join(work, "after"), git(repo, ["for-each-ref", "--format=%(objectname) %(refname)"]).stdout);
const after = new Map<string, string>();
for (const line of readFileSync(join(work, "after"), "utf8").split("\n")) {
  if (!line.trim()) continue;
  const [oid, ...rest] = line.split(" ");
  after.set(rest.join(" "), oid);
}
const out: string[] = [];
for (const line of readFileSync(join(work, "before"), "utf8").split("\n")) {
  if (!line.trim()) continue;
  const [old, ...rest] = line.split(" ");
  const ref = rest.join(" ");
  const neu = after.get(ref) ?? "0000000000000000000000000000000000000000";
  if (neu !== old) out.push(`${old} ${neu} ${ref}`);
}
writeFileSync(report, out.length ? out.join("\n") + "\n" : "");
