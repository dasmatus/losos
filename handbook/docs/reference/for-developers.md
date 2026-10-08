---
title: For developers
sidebar_position: 6
---

# For developers

This handbook is for owners. Everything about building, testing and changing
LosOS is in two other places:

- the [wiki](https://github.com/dasmatus/losos/wiki): Install (in depth),
  Administration, Mesh, Market, Master proxy, Hardening, Security model,
  Architecture, Development, CI and releases;
- [`CLAUDE.md`](https://github.com/dasmatus/losos/blob/main/CLAUDE.md) in the
  repository: the build commands, the cross-file architecture and the list of
  mistakes that are easy to make again.

## Changing this handbook

The source is `handbook/` in the repository, a [Docusaurus](https://docusaurus.io/)
site. Pages are Markdown under `handbook/docs/`; the sidebar follows the
folder tree, each folder named by its `_category_.json`.

```sh
cd handbook
npm ci
npm start                 # live preview on http://localhost:3000
npm run build             # fails on a broken link, on purpose
```

Every push to `main` publishes the site to [losos.dasmat.us](https://losos.dasmat.us)
(`.github/workflows/handbook.yml`), and every box builds its own copy
(`losos-handbook` in `flake/packages.nix`, served at `/handbook/` by
`modules/containers.nix`). After changing `package-lock.json`, refresh
`npmDepsHash` in `flake/packages.nix`:

```sh
nix run nixpkgs#prefetch-npm-deps -- handbook/package-lock.json
```

### Pictures

Pictures live in `handbook/docs/img/`, and the Slovak KOP's in
`handbook/docs/project/kop/img/`. There are three kinds:

- **Admin pages and wizard.** `admin-ui/app/tests/handbook-screens.mjs`
  takes them from the built SPA against a stubbed lososd, so they show the
  current UI and logo without a box. It writes 2x PNGs, which go into the
  handbook downscaled to 1600 px wide and reduced to 256 colours.
- **Screens of a real machine.** The installer, the tty1 banner, the
  passphrase prompt and the Secure Boot refusals come from VM runs
  (`tests/secure-boot.nix`, the install demos). They are not retaken by a
  script.
- **Diagrams.** These are hand-written SVG on a white card, in the palette
  of `src/css/tokens.css`, so they read the same in the light and the dark
  theme. Edit them as text.

```sh
cd admin-ui/app
npm run build
node tests/handbook-screens.mjs /tmp/handbook-screens          # every shot
node tests/handbook-screens.mjs /tmp/handbook-screens mesh     # names containing "mesh"
```

Set `LOSOS_CHROMIUM` to a Chromium binary if Playwright's own is missing.
The Market pane is planned and its route falls back, so the two market
shots run only with `LOSOS_SHOOT_MARKET=1` against a build with
`planned: false` in `src/screens/settings/panes.ts`. Never commit that
change.

Write alt text that says what the picture shows, not what the page says.

## Translating

Slovak and German are wired in. The interface strings live in
`handbook/i18n/<locale>/code.json`; a page is translated by copying it to
`handbook/i18n/<locale>/docusaurus-plugin-content-docs/current/<same path>`
and translating the copy. A page without a translation shows in English under
the translated chrome, so the work can be done one page at a time.

```sh
npm run write-translations -- --locale sk   # refresh the string files
npm run start -- --locale sk                # preview one locale
```

## Writing rules this handbook follows

- Lead with what the reader sees; put the cause second and the fix third.
- Every troubleshooting page starts with the steps that need only the LAN.
- The IP address comes before the `.local` name, always.
- Name the pull request when a behaviour changed recently, so a reader with
  an older box knows why their screen differs.
- No em-dashes.
