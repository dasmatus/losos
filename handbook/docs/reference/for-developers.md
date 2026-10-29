---
title: For developers
sidebar_position: 6
---

# For developers

Most of this handbook is for owners. Building, testing and changing LosOS
is covered in two places:

- the [In depth](/in-depth/index.md) chapter: Install in depth,
  Administration, Mesh, Market, Virtual machines, Master proxy, Edge
  federation, Diagrams, Hardening, Architecture;
- [`.claude/CLAUDE.md`](https://github.com/dasmatus/losos/blob/main/.claude/CLAUDE.md) in the
  repository: the build commands, the cross-file architecture and the list of
  mistakes that are easy to make again.

## Before you commit

The repository carries git hooks in `nix/hooks/` that run CI's checks for
the files you change before a commit or a push leaves your machine.
`devenv shell` and `nix develop` turn them on. Without either, run this once
in your clone:

```sh
bash nix/hooks/install
```

The commit hook formats and lints the Rust and Nix you touched, typechecks
the admin UI and evaluates every flake output. The push hook adds the Rust
tests, the admin UI's browser tests and the VM tests of the files you
changed. Each run's logs are in `.git/losos-hooks/logs/`. The developer
notes in `docs/development.md` list every check.

## Changing this handbook

The source is `handbook/` in the repository, a [Docusaurus](https://docusaurus.io/)
site. Pages are Markdown under `handbook/docs/`. The sidebar follows the
folder tree, and each folder takes its name from its `_category_.json`.

```sh
cd handbook
bun install --frozen-lockfile --ignore-scripts
bun run start             # live preview on http://localhost:3000
bun run build             # fails on a broken link, on purpose
```

Every push to `main` publishes the site to [losos.dasmat.us](https://losos.dasmat.us)
(`.github/workflows/handbook.yml`), and every box builds its own copy
(`losos-handbook` in `nix/packages.nix`, served at `/handbook/` by
`modules/containers.nix`). Docusaurus runs on [Bun](https://bun.sh), not
Node. After changing `bun.lock`, refresh the handbook's `hash` in
`nix/packages.nix`:

```sh
python3 .github/scripts/fix-hashes.py
```

### Pictures

Pictures live in `handbook/docs/img/`. There are three kinds:

- **Admin pages and wizard.** `admin-ui/app/tests/handbook-screens.mjs`
  takes them from the built SPA against a stubbed lososd, so they show the
  current UI and logo without a box. It writes 2x PNGs, which go into the
  handbook downscaled to 1600 px wide and reduced to 256 colours. Most come
  in Slovak and German too, as `<name>-sk.png` and `<name>-de.png`, and the
  translated pages show those.
- **Screens of a real machine.** The installer, the tty1 banner, the
  passphrase prompt and the Secure Boot refusals come from VM runs
  (`nix/tests/secure-boot.nix` and installs into a VM). No script retakes them.
  The right-click menu's pictures come from a box in a VM too:
  `nix/tests/context-menu.nix` writes them, in all three languages, to
  `shots/after/handbook/` in its output.
- **Diagrams.** These are hand-written SVG on a white card, in the palette
  of the admin UI (`admin-ui/app/src/styles/tokens.css`), so they read the
  same in the light and the dark theme. Edit them as text.

```sh
cd admin-ui/app
bun run build
bun tests/handbook-screens.mjs /tmp/handbook-screens           # every shot
bun tests/handbook-screens.mjs /tmp/handbook-screens mesh      # names containing "mesh"
```

:::note[When Playwright has no Chromium]
Set `LOSOS_CHROMIUM` to a Chromium binary if Playwright's own is missing.
:::

Write alt text that says what the picture shows, not what the page says.

## Translating

Slovak and German are wired in. The interface strings live in
`handbook/i18n/<locale>/code.json`. To translate a page, copy it to
`handbook/i18n/<locale>/docusaurus-plugin-content-docs/current/<same path>`
and translate the copy. A page without a translation shows in English under
the translated chrome, so you can translate one page at a time.

:::tip[Keep the three languages in step]
Every page is translated in full. Change all three languages together.
:::

```sh
bun run write-translations --locale sk      # refresh the string files
bun run start --locale sk                   # preview one locale
```

## Writing rules this handbook follows

- Lead with what the reader sees; put the cause second and the fix third.
- Every troubleshooting page starts with the steps that need only the LAN.
- The IP address comes before the `.local` name, always.
- Name the pull request when a behaviour changed recently, so a reader with
  an older box knows why their screen differs.
- No em-dashes.
