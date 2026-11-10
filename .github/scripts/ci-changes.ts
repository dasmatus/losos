#!/usr/bin/env bun
// Decide which lanes of ci.yml a change needs.
//
//   bun .github/scripts/ci-changes.ts < changed-files   one repository path per line
//   bun .github/scripts/ci-changes.ts --full "<reason>" every lane, and why
//
// Prints `<lane>=true|false` for every lane, plus `full=` and `reason=`.
// With GITHUB_STEP_SUMMARY set it also writes a table. Callers depend on
// that output: the pick step of .github/workflows/ci.yml and
// nix/hooks/lib.ts's hookLanes.

import { appendFileSync } from "node:fs";

const lanes = [
  "rust",
  "wasm",
  "solana",
  "evm",
  "eval",
  "ctl",
  "ui",
  "tui",
  "images",
  "mail",
  "iso",
  "install",
  "discovery",
] as const;
type Lane = (typeof lanes)[number];

const fullPatterns = [
  ".github/workflows/ci.yml",
  ".github/actions/*",
  ".github/scripts/ci-changes.ts",
  "tools/losos-ghcr-cache.py",
  "flake.lock",
  "devenv.nix",
  "devenv.yaml",
  "devenv.lock",
  "nix/fast-build.nix",
];

const pkgs = ["flake.nix", "nix/packages.nix", "nix/bun-deps.nix"];

const tuiReads = [
  "admin-ui/app/src/i18n/*",
  "admin-ui/app/src/styles/tokens.css",
  "admin-ui/app/src/screens/settings/panes.ts",
  "admin-ui/themes/brand/salmon-128.png",
];

const installer = [
  "backend/src/installer*",
  "backend/src/install_screen.rs",
  "backend/src/install_report.rs",
  "backend/plugins/report/*",
  "backend/src/bin/losos-ctl.rs",
  "backend/Cargo.toml",
  "backend/Cargo.lock",
  "modules/options.nix",
  "modules/disko.nix",
  "modules/installer.nix",
];

const lanePatterns: Record<Lane, string[]> = {
  rust: [
    "backend/*",
    "edge/registrar/*",
    "edge/vercel/*",
    "edge/contracts/solana/*",
    "admin-ui/tui/*",
    "admin-ui/lab/*",
    "modules/overrides.nix",
    ...tuiReads,
  ],
  wasm: ["backend/plugins/*", "backend/mdx/*", "admin-ui/lab/*", "admin-ui/tui/*"],
  solana: ["edge/contracts/solana/*", "edge/registrar/*", "backend/access/*"],
  evm: ["edge/contracts/*", "!edge/contracts/solana/*", "edge/registrar/*", "backend/access/*"],
  eval: [
    "*",
    "!*.md",
    "!*.mdx",
    "!*.rs",
    "!*.png",
    "!*.jpg",
    "!*.jpeg",
    "!*.gif",
    "!*.webp",
    "!*.svg",
    "!*.mp4",
    "!*.webm",
    "!docs/*",
    "!handbook/docs/*",
    "!handbook/i18n/*",
    "!.github/*",
    "!.claude/*",
    "!LICENSES/*",
    "!REUSE.toml",
    "!edge/vercel/*",
    "!edge/contracts/solana/*",
    "backend/src/cloud/boards.rs",
  ],
  ctl: ["backend/*", "!backend/plugins/*", "backend/plugins/report/*", "modules/overrides.nix", ...pkgs],
  ui: [
    "admin-ui/*",
    "!admin-ui/design-system/*",
    "!admin-ui/tui/*",
    "handbook/*",
    "edge/registrar/*",
    "backend/access/*",
    "backend/plugins/page/*",
    "backend/plugins/basic/*",
    "backend/mdx/*",
    "modules/options.nix",
    "nix/options-doc.nix",
    "nix/tests/admin-ui.nix",
    "nix/tests/fonts.nix",
    ...pkgs,
  ],
  tui: ["admin-ui/tui/*", "backend/access/*", ...tuiReads, ...pkgs],
  images: ["nix/images.nix", ...pkgs],
  mail: ["nix/stalwart.nix", ...pkgs],
  iso: [
    ...installer,
    "modules/cache.nix",
    "modules/secure-boot.nix",
    "modules/branding.nix",
    "modules/live-branding.nix",
    "modules/splash.nix",
    "modules/brand-art.nix",
    "modules/splash/*",
    "admin-ui/themes/brand/*",
    "nix/sign-iso.ts",
    "nix/version.nix",
    "nix/keys/*",
    "nix/tests/iso-boot.py",
    ...pkgs,
  ],
  install: [
    ...installer,
    "modules/cache.nix",
    "modules/branding.nix",
    "modules/splash.nix",
    "modules/boot.nix",
    "modules/impermanence.nix",
    "modules/configuration.nix",
    "modules/credentials.nix",
    "modules/console.nix",
    "modules/defaults.nix",
    "nix/tests/iso-install.nix",
    ...pkgs,
  ],
  discovery: [
    "nix/tests/discovery/*",
    "modules/options.nix",
    "modules/mdns.nix",
    "modules/containers.nix",
    "modules/tls.nix",
    "modules/daemon.nix",
    "modules/identity.nix",
    "modules/cloud.nix",
    "modules/windows.nix",
    "modules/config-repo.nix",
    "backend/src/cloud/*",
    "backend/src/windows.rs",
    "backend/src/people.rs",
    "backend/src/setup.rs",
    "backend/src/signin.rs",
    "backend/src/login.rs",
    "backend/src/http.rs",
    "backend/src/guard.rs",
    "backend/src/sections.rs",
    "backend/sections.json",
    "backend/src/lib.rs",
    "backend/src/bin/lososd.rs",
    "backend/src/bin/losos-cloud.rs",
    "backend/Cargo.toml",
    "backend/Cargo.lock",
    ...pkgs,
  ],
};

function globToRegex(pattern: string): RegExp {
  // Bash [[ x == pat ]] in ci-changes uses `*` matching across `/`.
  let re = "^";
  for (const ch of pattern) {
    if (ch === "*") re += ".*";
    else if (/[.+?^${}()|[\]\\]/.test(ch)) re += `\\${ch}`;
    else re += ch;
  }
  re += "$";
  return new RegExp(re);
}

function matches(file: string, patterns: string[]): boolean {
  let hit = false;
  for (const pat of patterns) {
    if (pat.startsWith("!")) {
      if (globToRegex(pat.slice(1)).test(file)) hit = false;
    } else if (globToRegex(pat).test(file)) {
      hit = true;
    }
  }
  return hit;
}

async function readChangedFiles(): Promise<string[]> {
  const text = await Bun.stdin.text();
  return text
    .split("\n")
    .map((s) => s.replace(/\r$/, ""))
    .filter((s) => s.length > 0);
}

const on = new Map<Lane, boolean>();
const why = new Map<Lane, string>();
let full = false;
let reason = "";

if (Bun.argv[2] === "--full") {
  full = true;
  reason = Bun.argv[3] ?? "asked for";
} else {
  const files = await readChangedFiles();
  if (files.length === 0) {
    full = true;
    reason = "no changed files listed";
  }
  for (const file of files) {
    if (matches(file, fullPatterns)) {
      full = true;
      reason = `${file} changes CI or the toolchain`;
      break;
    }
    for (const lane of lanes) {
      if (on.has(lane)) continue;
      if (matches(file, lanePatterns[lane])) {
        on.set(lane, true);
        why.set(lane, file);
      }
    }
  }
}

let out = `full=${full}\nreason=${reason}\n`;
for (const lane of lanes) {
  out += `${lane}=${full || on.has(lane)}\n`;
}
process.stdout.write(out);

const summaryPath = Bun.env.GITHUB_STEP_SUMMARY;
if (summaryPath) {
  let md = "## Lanes for this change\n\n";
  if (full) {
    md += `Every lane runs: ${reason}.\n`;
  } else {
    md += "| Lane | Runs | Because of |\n|---|---|---|\n";
    for (const lane of lanes) {
      if (on.has(lane)) md += `| ${lane} | yes | \`${why.get(lane)}\` |\n`;
      else md += `| ${lane} | no | | |\n`;
    }
    md +=
      "\nLabel the pull request `ci: full` and re-run the workflow to run every lane.\n";
  }
  appendFileSync(summaryPath, md);
}
