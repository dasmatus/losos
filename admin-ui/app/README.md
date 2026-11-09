# LosOS admin UI — one Expo project

One front-end codebase, two targets: this folder is the box's admin web UI
(the SPA — the DOM tree of pages under `src/`, react-router, the pages'
editors, the same styles) **and** the box-in-a-phone's Expo app (the
terminal client's panes on iOS and Android, the code the separate
`admin-ui/companion` project carried before it was merged away).

`src/app` is the Expo Router tree, so Metro builds both from one graph:

- **Web** — `src/app/index.web.tsx` mounts the SPA itself (src/App.tsx and
  the pages under it unchanged: react-router-dom, @base-ui, codemirror,
  radix — all DOM libraries, and the `.web.tsx`/`.native.tsx` suffixes are
  what keep either side out of the other's bundle). The browser build is
  the static output of Expo, replacing Bun's own bundler output:

  ```sh
  bun run build:web   # expo export --platform web --output-dir dist
  ```

  What `bun build.ts` (`bun run build`) still makes is the auxiliary set,
  unchanged from a checkout: the guest/second pages' HTML files
  (chat, cloud, kb, git, wiki… `src/<page>/main.tsx` entries), the
  service worker, theme-boot.js, app-frame runtime, and dist/pages (the
  default pages the pages plugin serves an editor from). Those have a
  follow-up to land behind expo-router routes; until then the two outputs
  complement each other, and nix's losos-admin-ui buildPhase runs only the
  Expo one. Both write into `dist/`.
  On web `expo run`/`start` is `bun run web`; Metro serves through the
  same dev server.

- **Native (iOS/Android)** — `src/app/(mobile)` is the phone tree: the
  connect + dashboard shell and one route per pane, so a pane is a deep
  link (`losos:///<pane>`, the app.config.ts scheme). Sources live under
  `src/mobile` (panes, screens, components, theme), shared API under
  `src/api`, the box config store under `src/store` — the companion's
  trees, imported by their old-relative paths after their folders moved.
- **Metadata** — `app.config.ts` is one manifest for both targets. The web
  stanza (Metro, `output: "static"`) and the native metadata (the
  companion's bundle ids `us.dasmat.losos.companion`, the salmon icon, the
  Bonjour paperwork written by `plugins/with-losos-discovery.js`) came
  from the two projects; `extra.lososServiceType` names the DNS-SD type
  `src/api/discovery.ts` browses.

Palette: `src/mobile/theme/tokens.gen.ts` is generated from
`src/styles/tokens.css` — the admin front ends' one palette — by
`scripts/gen-tokens.ts` (`bun run tokens`; run it after any tokens.css
change, the parser fails on a slot that stops existing).

## Build

Needs Node and [Bun](https://bun.sh) (the repo standard; deps are
bun-managed), plus Android Studio / Xcode for the native shells. Expo Go
cannot run the native targets — the mDNS browse and the box's TLS trust
live in native code the Go runtime does not ship — so the workflow is
prebuild plus a development client, with `android/` and `ios/` kept out
of git (CNG):

```sh
bun install
bun run tokens      # regenerate src/mobile/theme/tokens.gen.ts
npx expo prebuild   # writes android/ and ios/ from app.config.ts + plugins/
bun run start       # expo start --dev-client
bun run android     # or: npx expo run:android
bun run web         # expo start --web, the SPA under Metro
```

Typecheck stays `bun run typecheck` (tsc --noEmit, one tsconfig for both
targets). The admin browser tests are `bun run test:browser`.
