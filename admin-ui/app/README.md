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
prebuild plus a development client
([the docs' two routes][1]), with `android/` and `ios/` kept out
of git (CNG): `npx expo prebuild --clean` writes them the first time and
again only when a native dependency or a config plugin changes; `eas.json`
is the profiles file, and it carries no credentials — the CI token is the
repo secret `EXPO_TOKEN`, an `eas login` stays on the developer.

```sh
bun install
bun run tokens          # regenerate src/mobile/theme/tokens.gen.ts
npx expo prebuild --clean   # writes android/ and ios/, first time and
                            # after any native-side change only
```

**With Expo's services (the cloud route):** EAS builds the development
client on its workers — no Android SDK or macOS on this machine — and
`npx expo start` then feeds it running JS from this checkout:

```sh
eas build --platform android --profile development   # CI runs
                                                     # --platform all
bun run start        # expo start --dev-client; the launcher lists
                     # this project, and "open" boots the panes
```

The launcher screen the profile builds lists the dev server and its
deep links; a fresh install signs up under this project's name the first
launch (the docs' [development builds][1] third step). CI's
`expo-eas` job runs the same shape over both targets in one submission
(`eas build --platform all --profile development --non-interactive
--no-wait`), gated on the repo secret EXPO_TOKEN; without it the job
prints its skip notice and exits 0.

**Or on the local machine (the plain route):** Android Studio/Xcode
make the debug build locally, same launcher story:

```sh
bun run android      # npx expo run:android (or: bun run ios)
```

A local simulator build for iOS through EAS (`ios.simulator` in the
development profile) needs only Apple's tooling, not the paid account;
the same-account device build is the TODO eas.json's comment carries.

```sh
bun run web          # expo start --web, the SPA under Metro
```

[1]: https://docs.expo.dev/develop/development-builds/introduction/


Typecheck stays `bun run typecheck` (tsc --noEmit, one tsconfig for both
targets). The admin browser tests are `bun run test:browser`.
