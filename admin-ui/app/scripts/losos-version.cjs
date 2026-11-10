/* The tag the tree is, for app.config.ts's version (backend/build.rs's
 * stamp_version reads the same file the same way; nix/version.nix's header
 * is the sync contract that keeps it). The helper is plain CommonJS
 * because the ways this file gets evaluated split at the package's ESM /
 * CommonJS seam: @expo/config transpiles app.config.ts itself, and a
 * real `import { readFileSync } from "node:fs"` at its top made the
 * transpile land on the ESM path and break. One .cjs beside the config
 * reads both ways: require() from the transpiled config under node, and
 * bun's own require from ESM under the checkout and the nix build.
 *
 * Order mirrors build.rs: LOSOS_VERSION first (nix's losos-admin-ui
 * exports it — its source fileset is admin-ui/app alone, so the in-nix
 * config eval cannot walk ../nix), then nix/version.nix (the checkout's
 * own file), then the plain counter (a source checkout without nix/).
 */
const fs = require("node:fs");
const path = require("node:path");

/** build.rs's tag(): trim, strip quotes, prefix a bare semver with v. */
function tag(raw) {
  const trimmed = String(raw ?? "").trim().replace(/^"|"$/g, "").trim();
  if (trimmed === "") {
    return "0.1.0";
  }
  return /^[0-9]/.test(trimmed) ? `v${trimmed}` : trimmed;
}

function lososTag() {
  if (process.env.LOSOS_VERSION) {
    return tag(process.env.LOSOS_VERSION);
  }
  try {
    if (module.exports.__probeLog) console.error("[losos-version] dir=" + __dirname);
    // scripts/ is two directories into the repo root, so the nix tree is two
    // (version file) three further; backend/build.rs walks the same chain
    const text = fs.readFileSync(
      path.join(__dirname || process.cwd(), "..", "..", "..", "nix", "version.nix"),
      "utf8",
    );
    const start = text.indexOf('"');
    if (start !== -1) {
      const end = text.indexOf('"', start + 1);
      if (end !== -1) return tag(text.slice(start + 1, end));
    }
  } catch {
    // A source checkout that does not carry the nix tree.
  }
  return "0.1.0";
}

/** Expo's version field wants semver; the tag's display v stays for the
 * consumers that print it (losos --version, the boot menus). */
function lososSemver() {
  return lososTag().replace(/^v/, "");
}

module.exports = { tag, lososTag, lososSemver };
