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


## TV

The native targets build for Apple TV and Android TV too, "for lols
sake" and because the panes are genuinely usable from a couch: the
`react-native` dependency is the [React Native TV fork][2]
(`react-native-tvos@0.81.5-2`, the 0.81-stable release matching this
Expo SDK 54's RN 0.81), which still ships the TV support core React
Native dropped, and `@react-native-tvos/config-tv` (app.config.ts's
plugins list) rewrites the native projects when prebuild runs with
`EXPO_TV=1` — the Android manifest loses its portrait lock and gains
the leanback launcher intent, and the iOS project targets tvOS instead
of iOS. Phone prebuilds are untouched: without `EXPO_TV` the plugin is
inert and everything is the Build section's flow, except the fork swap,
which is source-compatible (same version, TV additions; `Platform.isTV`
/ `isTVOS`, `TVFocusGuideView`, focus-carrying `Pressable` all exist
for when a TV layout wants them).

```sh
export EXPO_TV=1
npx expo prebuild --clean   # android/ and ios/ now carry the TV rewrite
bun run android             # on an Android TV emulator, or:
bun run ios                 # on an Apple TV simulator (Xcode 16+,
                            # `xcodebuild -downloadAllPlatforms` for the
                            # tvOS SDK)
unset EXPO_TV
```

`eas.json` carries the matching cloud profiles
(`eas build --profile development_tv`, `preview_tv`), which extend the
phone ones with `EXPO_TV=1`; the tvOS signing caveats in Expo's [TV
guide][3] (a tvOS provisioning profile the CLI cannot mint) are the
only extra step a store-shaped TV build adds. Android TV needs no
separate credential path: the same keystore, the leanback manifest is
the only difference.

[2]: https://github.com/react-native-tvos/react-native-tvos
[3]: https://docs.expo.dev/guides/building-for-tv/


## Other native targets

Two out-of-tree React Native ports ride in devDependencies so their
versions are pinned in-tree, but they are not interchangeable with the
app's default `react-native` dependency:

- **Windows** — `react-native-windows@0.81.38`, the v0.81-stable release
  for this RN line. The [Windows guide][4] takes it from there
  (`npx react-native-windows-init` on a Windows machine). In this repo it
  remains an eject-time/native-shell target: no Windows desktop build can
  be compiled or run on Linux CI/runners without Windows tooling.
- **visionOS** — `@callstack/react-native-visionos@0.79.6` (RN 0.79
  fork). This cannot replace the app's default `react-native` alias to
  `react-native-tvos@0.81.5-2` in-place, because both are full React
  Native forks for different target/version lines. Treat visionOS as a
  **separate, generatable dependency set/prebuild path**:
  - Config: `eas.json` includes `development_visionos` and
    `preview_visionos` profiles (with `EXPO_VISIONOS=1`) as named build
    entry points for a visionOS-prepared install.
  - Reproducible generation: `scripts/visionos-prebuild.sh` documents the
    generation flow. Native `ios`/visionOS project outputs are gitignored
    in this repo (CNG/EAS convention), so reproducibility is provided via
    committed config/scripts/docs rather than checked-in generated native
    projects.
  - Compilation/run: requires macOS + Xcode + visionOS SDK/simulator or
    EAS cloud macOS builders with appropriate Apple signing. This cannot
    be built or run on Linux; CI verification here is limited to
    dependency/config wiring, typecheck, and web/Playwright checks where
    applicable.

### visionOS UI adaptation (this tree)

The mobile shell includes visionOS-aware layout (`src/mobile/platform.ts`,
platform guards/files): roomier centred content, larger touch/selection
targets, and a **Living room** dashboard presentation
(`src/mobile/components/LivingRoomDashboard.tsx`) that reuses the same
box reads/data layer as the normal panes to show big, glanceable status
tiles with coarse previous/next navigation and an explicit return to
normal mode (no dense editing/forms in living-room mode). A Skia-backed
visual is used where `@shopify/react-native-skia` is available on native
targets, with a plain RN fallback.

**Persona / spatial APIs:** `@callstack/react-native-visionos@0.79.6`
exposes JS modules such as `WindowManager` (open/close/update windows,
`supportsMultipleScenes`) and `XR` request/end session in this package
version, but it does **not** expose an ARKit/visionOS Persona /
FaceTime-style shared-space avatar presentation API that this app can
wire from JS. Persona is therefore **not implemented/faked** here; it is
a follow-up that likely requires upstream fork support or a small native
patch in the generated visionOS app. Any richer ornaments/volumes/glass
treatments beyond what the fork's JS and generated Swift app provide are
also follow-ups unless added as reproducible native config.

[4]: https://microsoft.github.io/react-native-windows/


## Skia

`@shopify/react-native-skia@2.2.12` (the version Expo SDK 54 pins in
its `bundledNativeModules`) is in dependencies so canvases can be
drawn without reaching for the deps file first. It is not imported
anywhere yet: nothing in the SPA needs a GPU canvas this page does not
already have (the lab's wasm renderers own that lane), so the package
sits ready for the first pane that does. Its Reanimated/worklets peers
are optional and deliberately left out — the native targets keep
`newArchEnabled: false` (expo-build-properties above) for the old
architecture modules the panes carry, and Reanimated 4 is a new
architecture package.



Typecheck stays `bun run typecheck` (tsc --noEmit, one tsconfig for both
targets). The admin browser tests are `bun run test:browser`.
