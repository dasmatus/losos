# The LosOS handbook

The owner's manual for a LosOS box, built with [Docusaurus](https://docusaurus.io/).

It is published twice from this one source:

- at <https://losos.dasmat.us> by `.github/workflows/handbook.yml` on every
  push to `main`;
- on every box at `http://<box>/handbook/`, built by `flake/packages.nix`
  (`losos-handbook`) and served by nginx (`modules/containers.nix`), so the
  manual is readable when only the local network is.

```sh
npm ci
npm start                         # live preview at http://localhost:3000
npm run build                     # the public site, into build/
LOSOS_HANDBOOK_BASE=/handbook/ npm run build   # what the box serves
npm run typecheck
```

The public site opens on a landing page, `src/pages/index.tsx`, and moves
the Welcome page to `/welcome`. The box's copy has no landing page and keeps
the Welcome page at its root (`docusaurus.config.ts` decides by the base).
Its strings are `losos.home.*` in each locale's `code.json`.

Pages live in `docs/`; the sidebar is generated from the folder tree, with
`_category_.json` naming each folder. `onBrokenLinks` is `throw`, so a dead
link fails the build. Translations go in `i18n/<locale>/`; see the handbook's
own "For developers" page for the two commands.

The colours, fonts and radii are the LosOS desktop's theme:
`src/css/derisk-theme.css` is generated from an mcsapi checkout by
`npm run theme -- /path/to/mcsapi` (needs cargo), and `src/css/palette.css`
names its values for `custom.css`.

After changing `package-lock.json`, refresh `npmDepsHash` in
`flake/packages.nix` (`nix run nixpkgs#prefetch-npm-deps -- handbook/package-lock.json`).
