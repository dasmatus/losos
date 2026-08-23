# design-sync notes for losos-ds

- NixOS host: `node`/`npm` exist only inside the flake dev shell. Run every
  npm/node command as `nix develop .# --command bash -c "…"` from the repo
  root. `nix develop -c` does not change directory.
- npm's allow-scripts blocks postinstall scripts (esbuild, playwright). That
  is fine. esbuild's platform binary rides in as an optional dep, and
  playwright browsers come from nix, not from its postinstall.
- Playwright browsers: the npm-downloaded chromium does not run on NixOS.
  Use nixpkgs `playwright-driver.browsers` from the flake's pinned nixpkgs
  (playwright 1.61.1 as of 2026-08-22). Keep the npm `playwright` version in
  `.ds-sync` AND in admin-ui/design-system/react's devDependencies in lockstep with
  `nix eval github:NixOS/nixpkgs/<rev>#playwright-driver.version`. The
  `losos-ds-render` flake check asserts the devDependency side.
  Run validate/capture with
  `PLAYWRIGHT_BROWSERS_PATH=$(nix build --print-out-paths --no-link github:NixOS/nixpkgs/<rev>#playwright-driver.browsers)`
  and `PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS=true`.
- flake.lock trap: this repo's lock has three nixpkgs nodes. The root
  flake's input is node `nixpkgs_3`, NOT the node named `nixpkgs` (that one
  belongs to disko/impermanence/home-manager). Read the rev with
  `jq -r '.nodes.nixpkgs_3.locked.rev' flake.lock`. Reading `.nodes.nixpkgs`
  gives a different nixpkgs and a mismatched playwright version.
- `tokens.css` and `losos.css` are two files with a required load order
  (tokens first) and no `@import` between them. The package build emits a
  concatenated `dist/styles.css` (added 2026-08-22, exported as
  `losos-ds/styles.css`) and `cfg.cssEntry` points at it. Don't point
  cssEntry at `dist/losos.css` alone or every `var(--ls-*)` comes up
  undefined.
- No Storybook anywhere in the repo (shape: package). Real usage examples
  live in `admin-ui/dashboard/` and `admin-ui/settings/` (plain-JS pages
  using the same class names) and `admin-ui/design-system/react/tests/render.test.tsx`.
- Composition gotcha from wave 1: a labeled `Switch` inside `Group` MUST be
  wrapped in `Row`. `.switch-row` has no padding or min-height of its own,
  only `.row` does, so bare Switch siblings in a Group visually overlap.
  `GroupHint` is a SIBLING after `Group`, never a child.
- `AuthOverlay` (`position: fixed; inset: 0`) vs the capture harness: every
  card cell wrapper carries `transform: translateZ(0)`, which becomes the
  containing block for fixed-position descendants and collapses the overlay
  to the wrapper box. The fix lives in the authored preview: wrap the
  overlay in an explicitly sized `position: relative; transform:
  translateZ(0)` div, a miniature viewport. Don't fork lib/emit.mjs for
  this.
- The `losos-ds-render` flake check (tests/design-system.nix) pins
  `npmDepsHash` over admin-ui/design-system/react/package-lock.json. Bump the hash
  whenever the lockfile changes:
  `nix run <nixpkgs>#prefetch-npm-deps -- admin-ui/design-system/react/package-lock.json`.

## Pull direction (designs → repo)

- Designs saved from Claude Design land in the project as
  `templates/<slug>/<Name>.dc.html` plus harness files (`ds-base.js`,
  `image-slot.js`, `support.js`). `templates/app-icon` is the source the
  2026-08-23 homepage sync was built from: its "In place" section carries
  the homepage design (drifting gradient, glass overrides, condensing
  float bar, launcher tiles, right-click address menu) even though the
  template is titled as an AppIcon component proposal.
- That template's AppIcon component proposal (`AppIconProps`, `.app-icon*`
  markup, sizes 20/28/40, five tones) is NOT implemented — the homepage
  tiles use their own 56px monograms. Still open as a losos-ds candidate.
- losos.css gained a `[hidden] { display: none !important }` guard
  (2026-08-23): the flex displays on `.error-banner`/`.auth-overlay` were
  overriding the UA's [hidden] rule. The uploaded styles.css is stale
  until the next re-sync; the driver will flag it via styleSha.

## Re-sync risks (watch-list for the next run)

- Playwright lockstep is three-sided. A nixpkgs bump moves
  `playwright-driver`, which must move (a) the `.ds-sync` npm playwright,
  (b) admin-ui/design-system/react's `playwright` devDependency, and (c)
  `npmDepsHash` in tests/design-system.nix. The flake check asserts (b);
  nothing asserts (a), and a stale `.ds-sync` fails at capture time with
  "Executable doesn't exist".
- The `PLAYWRIGHT_BROWSERS_PATH` store paths in this file's commands are
  rev-specific. Recompute from the current flake.lock (`nixpkgs_3`!) rather
  than reusing a path from an old NOTES revision.
- Component groups and `.prompt.md` content come from
  `admin-ui/design-system/react/docs/<Name>.md` (frontmatter `category`). A new
  component without a doc lands in group "general" with a synthesized
  prompt. Write its doc as part of adding it.
- `cfg.cssEntry` points at `dist/styles.css`, produced by the package
  build's `cat tokens.css losos.css` step. If that build-script step is
  removed, validate fails with `[CSS_PLACEHOLDER]`/`[CSS_IMPORT_MISSING]`.
- Preview content uses the appliance vocabulary (philae.local, Nextcloud /
  Forgejo / Tahoe-LAFS, Local↔Mesh, admin token). If the product vocabulary
  changes, previews still render but drift from reality. They are cheap to
  re-author.
- The `AuthOverlay.tsx` preview carries its own sized
  `transform: translateZ(0)` wrapper to contain `position: fixed` under the
  capture harness's transformed cell wrapper. If the harness ever drops that
  transform, the preview keeps working; its own wrapper still contains the
  overlay.
- Verified-state carry-forward lives in the uploaded `_ds_sync.json`, not in
  git. Fetch it to `.design-sync/.cache/remote-sync.json` and pass
  `--remote` on re-syncs (see the driver command in the skill).
