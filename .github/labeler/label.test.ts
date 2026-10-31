import { expect, test } from "bun:test";

import { compileRules, explain, pathMatches, type LabelInput } from "./label";
import { rules } from "./rules";

const compiled = compileRules(rules);

function names(input: LabelInput): string[] {
  return explain(compiled, input)
    .map((match) => match.label)
    .sort();
}

const bugTemplate = `
**Describe the bug**
The boot menu wraps the title.

**To Reproduce**
Steps to reproduce the behavior:
1. Go to '...'
`;

const featureTemplate = `
**Is your feature request related to a problem? Please describe.**
I'm always frustrated when the menu wraps.

**Describe the solution you'd like**
Wrap the title onto two lines.
`;

/* The headings a pull request is opened with. None of these is a label. */
const pullTemplates = `
Before: what a reader saw, or what the box did, before this change.
After: what they see, or what it does, now.
handbook and docs pages
how it was checked
What ran and what it showed
## Description
What this changes and why.
## Reviewer notes
how you tested it and on what, what to look at closely
No visible change.
`;

test("a bug report is a bug and not a feature", () => {
  expect(names({ title: "Boot menu wraps", body: bugTemplate })).toEqual(["bug"]);
});

test("a feature request is an enhancement and not a bug", () => {
  expect(names({ title: "Wrap the boot menu title", body: featureTemplate })).toEqual(["enhancement"]);
});

test("the pull request template alone labels nothing", () => {
  expect(names({ title: "refresh the hashes", body: pullTemplates })).toEqual([]);
});

test("a fix commit is a bug even when the body is only the template", () => {
  expect(
    names({
      title: "restore mail.nix syntax",
      body: pullTemplates,
      commits: ["fix: restore mail.nix syntax and add EdgeStatus::local_gateway"],
      files: ["modules/mail.nix"],
    }),
  ).toEqual(["bug", "nix"]);
});

test("feat, docs and deps prefixes come from the title or a commit", () => {
  expect(names({ title: "feat(mail): sign outgoing mail", body: "" })).toEqual(["enhancement"]);
  expect(names({ title: "docs: say how the cache is filled", body: "" })).toEqual(["documentation"]);
  expect(names({ title: "deps(bun): bump the bun group", body: "" })).toEqual(["dependencies"]);
  expect(names({ title: "chore(deps-dev): bump typescript", body: "" })).toEqual(["dependencies"]);
  expect(names({ title: "fix(ui)!: drop the clipped strip", body: "" })).toEqual(["bug"]);
});

test("dependabot's own word is a dependency update", () => {
  expect(names({ title: "Bump rustls", body: "Bumps rustls. Dependabot will resolve any conflicts." })).toEqual([
    "dependencies",
  ]);
});

test("paths label the language and the dependency file, not the patch", () => {
  expect(names({ title: "hashes", body: "", files: ["backend/src/main.rs", "backend/Cargo.lock"] })).toEqual([
    "dependencies",
    "rust",
  ]);
  expect(names({ title: "hashes", body: "", files: ["flake.nix", "flake.lock"] })).toEqual(["dependencies", "nix"]);
  expect(names({ title: "strip", body: "", files: ["admin-ui/app/src/screens/strip.tsx"] })).toEqual(["javascript"]);
  expect(names({ title: "handbook", body: "", files: ["handbook/install.md"] })).toEqual(["documentation"]);
  expect(names({ title: "site", body: "", files: ["website/src/wiki.js"] })).toEqual(["javascript"]);
});

test("a ts suffix is not a tsx file", () => {
  expect(pathMatches("**/*.ts", "admin-ui/app/src/theme-boot.ts")).toBe(true);
  expect(pathMatches("**/*.ts", "admin-ui/app/src/App.tsx")).toBe(false);
  expect(pathMatches("**/*.rs", "src/losos-vpn/src/main.rs")).toBe(true);
  expect(pathMatches("**/*.rs", "src/losos-vpn/src/main.rs.txt")).toBe(false);
  expect(pathMatches("docs/**", "docs/vm/boot.md")).toBe(true);
  expect(pathMatches("docs/**", "docs-extra/boot.md")).toBe(false);
  expect(pathMatches("**/Cargo.lock", "Cargo.lock")).toBe(true);
  expect(pathMatches("**/package.json", "admin-ui/app/package.json")).toBe(true);
});

test("prose that only says fix or debug is not a bug", () => {
  expect(names({ title: "Debug the boot menu", body: "please fix the wording in a follow-up" })).toEqual([]);
});

test("questions, access, and newcomer notes are explicit", () => {
  expect(names({ title: "Install", body: "How do I install this on a laptop?" })).toEqual(["question"]);
  expect(names({ title: "Reader", body: "The menu has no screen reader name." })).toEqual(["accessibility"]);
  expect(names({ title: "Menu", body: "This is a good first issue. Help wanted." })).toEqual([
    "good first issue",
    "help wanted",
  ]);
});

test("ci: full and the triage labels are never applied", () => {
  for (const name of ["ci: full", "wontfix", "invalid", "duplicate", "vote"]) {
    expect(rules[name]).toBeUndefined();
  }
  expect(
    names({
      title: "ci: full",
      body: "please run every job, label this ci: full",
      commits: ["ci: full"],
      files: [".github/workflows/ci.yml"],
    }),
  ).toEqual([]);
});

test("hotfix is only the word, and only when the repository has that label", () => {
  const got = names({ title: "the registrar is down", body: "this is a hotfix for the edge path" });
  if (rules.hotfix === undefined) expect(got).toEqual([]);
  else expect(got).toEqual(["hotfix"]);
});

test("a path match names the file and does not quote the body", () => {
  const matches = explain(compiled, {
    title: "strip",
    body: "secret token",
    files: ["admin-ui/app/src/screens/strip.tsx"],
  });
  expect(matches).toEqual([{ label: "javascript", reason: "path admin-ui/app/src/screens/strip.tsx" }]);
});
