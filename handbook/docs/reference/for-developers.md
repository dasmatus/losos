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
