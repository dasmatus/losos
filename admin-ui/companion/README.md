# LosOS Companion

A React Native (Expo) client for the LosOS box: it finds the box on the LAN
by its `_losos._tcp` announcement (modules/configuration.nix,
modules/mdns.nix) or takes `<name>.local` typing, links with a token from
the admin pages' Settings → Access tokens, and mirrors the terminal
client's panes on a phone.

It shares the admin front ends' palette
(`admin-ui/app/src/styles/tokens.css`), generated into
`src/theme/tokens.gen.ts` by `scripts/gen-tokens.ts` — the same colours the
Compose app's `LosTheme.kt` carried, light and dark, the ok/warn/crit
triple included. The launcher icon and splash are the coho salmon of
`admin-ui/themes/brand/` (`salmon.png`, NOAA Fisheries; credits beside the
original), the splash on the brand plate (`#DCBDAA`).

## Build

Needs Node and [Bun](https://bun.sh) (the repo standard; deps are
bun-managed), plus Android Studio / Xcode for the native shells. Expo Go
cannot run this app — the mDNS browse and the box's TLS trust live in
native code the Go runtime does not ship — so the workflow is prebuild plus
a development client, with `android/` and `ios/` kept out of git (CNG):

```sh
bun install
bun run tokens     # regenerate src/theme/tokens.gen.ts from tokens.css
npx expo prebuild  # writes android/ and ios/ from app.config.ts + plugins/
bun run start      # expo start --dev-client
bun run android    # or: npx expo run:android
bun run ios
```

Scripts: `dev`/`start` (Expo CLI), `typecheck` (`tsc --noEmit`), `build`
(`expo export`, the offline bundle). There is no `.sh` in this folder and
none is needed; the token generator is a Bun TypeScript script like the
repo's other TS tooling.

## Design notes

- **Types:** TypeScript throughout; first-party sources are pure
  TypeScript (only `babel.config.js` and the Expo config plugin
  `plugins/with-losos-discovery.js` carry CommonJS, both build-time files
  Expo loads itself).
- **Store** (`src/store/config.ts`): the BoxConfig port — host, token,
  demo, themeMode — on **AsyncStorage**, not MMKV: the config is four
  fields read a handful of times per launch, and MMKV's native module
  trails every React Native major; the token rides in the app's own
  sandbox, the protection level SharedPreferences had.
- **API** (`src/api/api.ts`): the ApiClient.kt port onto fetch. Bearer
  token on every call except anonymous `/api/health` (api.ts
  `get(path, { anonymous: true })`, the route OverviewPane's health card
  asks); 4s connect / 20s read timeouts (the trusty transport carries one
  wall-clock deadline of their sum); `ApiError(status, message)` with the
  box's own `{"error": …}` word; a 401 evicts the token and lands back on
  the connect screen, like the SPA's eviction
  (`admin-ui/app/src/lib/api.ts`, dropToken subscribed by the app shell).
  Routes as in the Compose app: `/api/health*`, `/api/status`,
  `/api/storage` + `POST /api/grow`, `/api/edge`, `/api/apps` (+
  `search?q=` and `chart?`, `POST install/remove`), `/api/settings`,
  `/api/options`, `/api/config` + `POST /api/config/sync`, `/api/backup` +
  `POST /api/backup/run`, and `/api/cloud/files?path=%2F` for the Cloud
  tab's root listing.
- **TLS** (`src/api/transport.ts`): the box serves a self-signed
  certificate until the owner enrols a CA (BoxTrust.kt). The hook into
  fetch that Compose installed per-connection is a transport here: plain
  `http://` rides the platform fetch (cleartext to the LAN is permitted,
  the port of the old manifest's `res/xml/network.xml`, spelled by
  expo-build-properties); `https://` rides
  **react-native-blob-util** with `trusty: true`. In its 1.x semantics the
  chain is not verified but hostname and validity dates are — close to
  BoxTrust, which also let the hostname check go — and permissive where
  the cert pinning libraries cannot follow: pinning is stricter than an
  unclaimed box's certificate, which the box mints before its owner sees
  it, so no pre-baked pin could make the first connection. Once a CA is
  enrolled, the strict default path is the one a future revision should
  pin; the TTL-tracked follow-up is noted in the result note.
- **mDNS** (`src/api/discovery.ts`): DNS-SD browse of `_losos._tcp` over
  **react-native-zeroconf** (0.17; `FoundBox {name, ip, port, version}`,
  `address = <name>.local`), with `plugins/with-losos-discovery.js`
  writing the platform paperwork: Android's four network permissions and
  iOS's `NSBonjourServices` (both spellings) +
  `NSLocalNetworkUsageDescription`. iOS browsing needs no multicast
  entitlement (Bonjour's browser API does it); a pure-mDNS reader added
  later would want `com.apple.developer.networking.multicast`, and that is
  Apple's own review. Android runs on the library's NSD backend, as the
  Compose  app walked NsdManager; its embedded DNSSD responder is the
  fallback for vendors with a broken NsdManager.
- **Old vs new architecture:** `expo-build-properties` pins
  `newArchEnabled: false` on both platforms — react-native-zeroconf 0.17
  and blob-util's current line are interop-era modules; zeroconf's 1.0 C++
  module (React Native 0.82+/Expo SDK 55+) is the upgrade that flips the
  switch.
- **Demo mode:** every invented number wears the **demo** chip, exactly
  the Compose rule; the invented set (apps tiles, widgets, the standing
  512 GB disk) landed in `src/store/config.ts` (`DEMO_STORAGE`) and the
  panes.

## Status

The panes of the terminal client (`admin-ui/tui`), mapped to a phone's
bottom bar. When a box is linked every number is a read from the box over
its JSON API (`src/api/api.ts`, driven like the admin SPA's `call()`);
while demo mode is on, every card carrying invented numbers wears a
**demo** chip. A 401 drops the token and lands back on the connect screen,
like the SPA's eviction.

| Tab | Reads | Status |
|---|---|---|
| Dashboard | `GET /api/health*`, `/api/status`, `/api/storage`, `/api/apps`, `/api/settings` | real, read-only |
| Storage | `GET /api/storage`, `POST /api/grow` | real, with a guarded reserve-spend action (online grow, no rebuild) |
| Mesh | `GET /api/edge`, `GET /api/settings` | real, read-only; the sharing toggles (`POST /api/apply`) stay on the web UI |
| Apps | `GET /api/apps`, `GET /api/apps/search?q=`, `GET /api/apps/chart?...`, `POST /api/apps/install`, `POST /api/apps/remove` | real, with guarded install/remove actions (install uses chart details, confirms release, runs as `notshared` with default values) |
| Network | `GET /api/settings` | real, read-only |
| Security | `GET /api/settings` | real, read-only |
| Advanced | `GET /api/options` | real, read-only |
| History | `GET /api/config`, `POST /api/config/sync` | real, with the sync action |
| Backup | `GET /api/backup`, `POST /api/backup/run` | real, with the run action; restores stay on the web UI |
| Cloud | `GET /api/cloud/files?path=%2F` | real, top of the files only; deeper browsing is next |
| Settings | local | box, probe, theme (persisted, as the Compose tab was) |

`*` health is the one route asked without the token (api.ts
`get(path, { anonymous: true })`),
and the connect screen's probe never carries it at all — reachability is
not identity. The connect screen also browses `_losos._tcp` (zeroconf in
`src/api/discovery.ts`): every announced box lists its `.local` name,
release tag and an `IP` pill for phones whose own `.local` lookup answers
nothing.

Not ported, on purpose: the TUI's About pane lives as a card under
Settings; its This computer pane (the keychain of the machine `losos` runs
on) has no phone shape; the edge panes (`edge-overview`,
`edge-secrets`) are for a company edge, and this client links boxes.
Social signing in with LosOS Login (a cookie flow) is still ahead; the
pasted access token is the way in, as in the app's unlock prompt. tcp over
UWB, widgets on the phone's own home screen, and the edge's manage pages
have no part here yet.

## Result note — the Kotlin → RN rewrite

Everything the task's plan named is in the tree, in TypeScript only:

- Deleted in place: the Compose app (`app/` with all 9 Kotlin files,
  `build.gradle.kts`, `settings.gradle.kts`, `gradle.properties`, and the
  `res/` tree — icon, splash, colors, network config re-carried by
  `app.config.ts` + the plugin). The README carried over and grew the
  sections above.
- `src/theme/tokens.gen.ts` is generated from
  `admin-ui/app/src/styles/tokens.css` by `scripts/gen-tokens.ts` and
  matches LosTheme.kt's palette slot for slot, light and dark, including
  the ok/warn/crit triples. tokens.css postdates the Compose port in a few
  dark slots (secondary family, ok/warn/crit) and one light hex
  (`hair`); where the two agreed the generator reads the document, and
  where they disagree the generator pins the shipped Compose value with a
  commented row in its `OVERRIDES` table, so the palette stays exactly
  what the app shipped until the colour change is confirmed.
- `src/store/config.ts` (BoxStore.kt), `src/api/{api,transport,probe,
  discovery,wire}.ts` (ApiClient.kt, BoxTrust, BoxProbe, Discovery.kt, the
  wire helpers), `src/theme/{tokens,useTokens}.ts` (LosTheme.kt wrapper
  + types + ThemeMode), screens `src/screens/{ConnectScreen,
  DashboardScreen}.tsx` with the bottom-bar tabs (the Compose app's
  eleven: Dashboard, Storage, Mesh, Apps, Network, Security, Advanced,
  History, Backup, Cloud, Settings), components `src/components/` under
  the plan's names (DemoChip, PaneHeader, ReadCard, PaneRow,
  StorageMeter, EdgeCard, SearchCard with SearchHitRow, AppTile, Widget,
  SecurityRow, OptionRow) plus the Material pieces the panes called
  (`primitives.tsx`: buttons, text fields, filter chips, the alert as a
  Modal).
- Assets: `assets/images/salmon.png` (copied from `themes/brand/`) for
  icon, adaptive-icon foreground and splash; `app.config.ts` carries
  versionName 0.1.0 parity (`version: "0.1.0"`, `versionCode: 1`).
- `package.json` scripts: `tokens`, `start`, `android`, `ios`,
  `prebuild`, `typecheck`, `build`; `tsconfig.json` extends
  `expo/tsconfig.base` strict. No `.sh` anywhere in `admin-ui/companion`.
- Licensing: the tree's REUSE.toml blanket (`AGPL-3.0-or-later`,
  precedence closest) governs every file here; nothing in the folder
  needs its own SPDX header under the spec, and none of the repo's other
  TS sources carry one.

Open follow-ups, on the parent's plan next: an on-device smoke pass (the
typecheck gate ran here; the dev-client build plus a signed AP Iz box loop
belongs to the program's device work), pinning-heavier transport once CA
enrolment is the norm, and the App Store-safe narrowing of the iOS ATS
allow-list.
