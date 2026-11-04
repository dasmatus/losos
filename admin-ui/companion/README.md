# LosOS Companion

An Android client for the LosOS box, in Jetpack Compose: it reaches the box
over the LAN (`<name>.local` or its IP), links with a token from the admin
pages' Settings → Access tokens, and mirrors the admin homepage in a mobile
shape — connection card, storage card, app tiles, widget board, plus quick
links into LosOS Cloud.

It shares the admin front ends' palette (`admin-ui/app/src/styles/tokens.css`),
ported to Compose in `app/src/main/java/us/dasmat/losos/companion/LosTheme.kt`.

## Build

Needs JDK 17 and Gradle 8.10+ (or Android Studio, opening this folder):

```sh
gradle :app:assembleDebug
adb install app/build/outputs/apk/debug/app-debug.apk
```

## Status

The panes of the terminal client (`admin-ui/tui`), mapped to a phone's bottom
bar. When a box is linked every number is a read from the box over its JSON
API (`ApiClient.kt`, driven like the admin SPA's `call()`); while demo mode is
on, every card carrying invented numbers wears a **demo** chip. A 401 drops
the token and lands back on the connect screen, like the SPA's eviction.

| Tab | Reads | Status |
|---|---|---|
| Dashboard | `GET /api/health*`, `/api/status`, `/api/storage`, `/api/apps`, `/api/settings` | real, read-only |
| Storage | `GET /api/storage` | real, read-only; spending the reserve (`POST /api/grow`) stays on the web UI |
| Mesh | `GET /api/edge`, `GET /api/settings` | real, read-only; the sharing toggles (`POST /api/apply`) stay on the web UI |
| Apps | `GET /api/apps`, `GET /api/apps/search?q=` | real, read-only; install/remove (`POST /api/apps/install|remove`) stays on the web UI |
| Network | `GET /api/settings` | real, read-only |
| Security | `GET /api/settings` | real, read-only |
| Advanced | `GET /api/options` | real, read-only |
| History | `GET /api/config`, `POST /api/config/sync` | real, with the sync action |
| Backup | `GET /api/backup`, `POST /api/backup/run` | real, with the run action; restores stay on the web UI |
| Cloud | `GET /api/cloud/files?path=%2F` | real, top of the files only; deeper browsing is next |
| Settings | local | box, probe, theme |

`*` health is the one route asked without the token (api.ts `getHealth`), and
the connect screen's probe never carries it at all — reachability is not
identity.

Not ported, on purpose: the TUI's About pane lives as a card under Settings;
its This computer pane (the keychain of the machine `losos` runs on) has no
phone shape; the edge panes (`edge-overview`, `edge-secrets`) are for a
company edge, and this client links boxes. Social signing in with LosOS Login
(a cookie flow) is still ahead; the pasted access token is the way in, as in
the app's unlock prompt.

## Finding the box

The connect screen browses the LAN for the box itself: every box announces
`_losos._tcp` over DNS-SD — the front vhost's port and its release tag in TXT
(`modules/mdns.nix`), the same record LosOS Desktop lists boxes by
(`Discovery.kt`). What answers is listed under the address field — tap a box
and its `<name>.local` drops in; the token stays yours to paste.

The launcher icon is the brand's coho salmon on the plate colour
(`admin-ui/themes/brand/`, see that folder's CREDITS.md), as an adaptive
icon (`res/mipmap-anydpi-v26/`).
