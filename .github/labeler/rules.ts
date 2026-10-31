/* Which label an issue or a pull request gets.
 *
 * Text patterns are JavaScript regular expressions. The matcher adds the
 * i and m flags, so they ignore case and ^ is the start of any line.
 * The text is the title, the body, and for a pull request every commit
 * message. Path globs match the files those commits change.
 *
 * Keep this in step with losos-project/desktop, which has the same rules
 * apart from hotfix. That label is only used here.
 *
 * ci: full is not here. It runs every CI job, and a person puts it on a
 * pull request by hand. wontfix, invalid, duplicate and vote are
 * decisions the text cannot settle, so they are not here either.
 */
import type { LabelRules } from "./label";

const bug = String.raw`^(?:fix|bug|bugfix)(?:\([^)\n]*\))?!?:`;
const feat = String.raw`^(?:feat|feature)(?:\([^)\n]*\))?!?:`;
const docs = String.raw`^(?:docs?)(?:\([^)\n]*\))?!?:`;
const depsScope = String.raw`^(?:deps|dependenc(?:y|ies))(?:\([^)\n]*\))?!?:`;
const choreDeps = String.raw`^(?:chore|build)\(deps[^)\n]*\)!?:`;

export const rules: LabelRules = {
  bug: {
    description: "Something isn't working",
    color: "d73a4a",
    text: [bug, String.raw`\b(?:bug|regression|crash|broken|stack trace)\b`, String.raw`\bsteps to reproduce\b`],
  },
  enhancement: {
    description: "New feature or request",
    color: "a2eeef",
    text: [feat, String.raw`\b(?:feature request|new feature)\b`],
  },
  documentation: {
    description: "Improvements or additions to documentation",
    color: "0075ca",
    text: [docs, String.raw`\bdocumentation\b`],
    paths: ["**/*.md", "**/*.mdx", "docs/**", "handbook/**"],
  },
  dependencies: {
    description: "Pull requests that update a dependency file",
    color: "0366d6",
    text: [depsScope, choreDeps, String.raw`\bdependabot\b`],
    /* The manifest is the dependency file, not just the lock. A pull
     * request that also changes source still gets the language label. */
    paths: [
      "**/package.json",
      "**/package-lock.json",
      "**/bun.lock",
      "**/bun.lockb",
      "**/pnpm-lock.yaml",
      "**/yarn.lock",
      "**/Cargo.toml",
      "**/Cargo.lock",
      "**/flake.lock",
      "**/devenv.lock",
    ],
  },
  javascript: {
    description: "Pull requests that update javascript code",
    color: "168700",
    paths: ["**/*.{js,jsx,mjs,cjs,ts,tsx,mts,cts}"],
  },
  nix: {
    description: "Pull requests that update nix code",
    color: "3E6399",
    paths: ["**/*.nix"],
  },
  rust: {
    description: "Pull requests that update rust code",
    color: "000000",
    paths: ["**/*.rs"],
  },
  accessibility: {
    description: "Barrier affecting people with disabilities",
    color: "f143ab",
    text: [String.raw`\b(?:a11y|accessibility|screen[\s-]?reader|wcag)\b`, String.raw`\baria-`],
  },
  question: {
    description: "Further information is requested",
    color: "d876e3",
    text: [String.raw`^question\b`, String.raw`\b(?:how do i|how can i|is there a way)\b`],
  },
  "help wanted": {
    description: "Extra attention is needed",
    color: "008672",
    text: [String.raw`\bhelp wanted\b`],
  },
  "good first issue": {
    description: "Good for newcomers",
    color: "7057ff",
    text: [String.raw`\bgood first issue\b`],
  },
  hotfix: {
    description: "For PRs: if some code breaks the infra",
    color: "bd5e17",
    text: [String.raw`\bhotfix\b`],
  },
};
