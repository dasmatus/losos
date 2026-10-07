# Administration

## Admin UI

Open `http://<ip>/` (the address on the box's tty1 banner) or
`https://<host>.local/` from the LAN. The admin UI is blocked for anything
outside the LAN, including traffic through the [master proxy](Master-Proxy).
`<host>.local` is an mDNS name: a computer that does not resolve it (typically
the host of a NAT'ed VM) uses the IP address, and every route below answers on
it the same way.

Services on the same host:

| Service          | URL                       |
| ---------------- | ------------------------- |
| Admin UI         | `<host>.local/`           |
| Nextcloud        | `<host>.local/nextcloud`  |
| Forgejo (opt-in) | `<host>.local/forgejo/`   |

The box's IP address (the one the tty1 banner shows) and its bare name work
in place of `<host>.local` everywhere, Nextcloud included. That is the way in
from a libvirt VM, which gets no mDNS name: nginx tells the Nextcloud pod
which address each request arrived on, and the pod trusts exactly that one,
never a wildcard (`modules/workloads.nix`). `localhost` and `127.0.0.1`
Nextcloud trusts on its own.

`losos.tls.enable` (on by default) adds HTTPS on :443 with a certificate the
box generates itself, valid for two years. Plain HTTP on :80 stays open.
Browsers do not trust the certificate until you install it; the first-run
wizard offers a one-line installer for macOS, Linux and Windows
(`/setup/trust.sh`, `/setup/trust.ps1`, both LAN-only) and the plain
download (`/setup/losos-ca.crt`). See [Install](Install).

## Look

**Settings → Look** changes how the admin pages look, for every browser that
opens the box. Nothing here is a setting in the Nix sense: it is a document
lososd keeps at `/var/lib/losos/look.json`, so a change takes effect the
moment it is saved, with no rebuild.

### Background

Pick one of the three shipped pictures, or upload one of your own (PNG, JPEG,
WebP, GIF or SVG, up to 8 MiB). The **Veil** slider lays the page colour over
the picture, from 20 % to 90 %, so text stays readable in both the light and
the dark theme. *Plain* removes the picture again.

An uploaded picture is served by lososd at `/api/look/background` without a
token, because a CSS `background-image` cannot send one. The route sits behind
the same LAN-only guard as the rest of the admin pages, and a wallpaper is
not a secret.

### Widgets written by hand

The Overview board's gallery (**Add a widget**) has two ways to make your
own. *Build one* composes a widget from the box's readings without any code.
*Write one* takes HTML, style and script of your own, kept on the box and
listed on this pane, where it can be edited and deleted.

A hand-written widget runs inside a sandboxed frame, an origin of its own
with no access to the admin pages, the admin token or the API. It talks to
the box through a small `losos` object the frame provides:

| Call                           | What it gives                                                        |
| ------------------------------ | -------------------------------------------------------------------- |
| `losos.metric(name)`           | A promise of one of the box's readings, the same names the gallery's built-in widgets use (`storage.bytes`, `box.settings`, …). |
| `losos.theme`, `losos.lang`    | `light`/`dark` and the language the admin page is in.                |
| `losos.palette`                | The box's colours, also set as CSS variables (`var(--accent)` works). |
| `losos.onTheme(fn)`            | Called whenever the owner switches the theme.                         |
| `losos.resize()`               | Ask the board to re-measure the tile after a change it cannot see.    |

The editor's **Help** tab repeats this with an example, and its preview is the
real frame, so what it shows is what the tile will show. A widget can fetch
the internet (a weather tile, say) but not the box; up to 24 widgets of
64 KiB each.

## Admin token

`lososd` writes a random 64-hex-character token to
`/var/secrets/losos-admin-token` (mode 0600) on first start. The token can
change any setting, so it is equivalent to root.

On a new, unclaimed box, the first LAN caller to submit `POST /api/setup/claim`
sets the owner password and receives the token in the response. This route
requires no token, but stops accepting claims once the box has an owner. Set up
the box only on a trusted LAN; the first caller becomes its owner. The token is
handed back only once and cannot be fetched again through the UI.

A claim sent before Nextcloud has finished its first start answers `503` with
`{"error": …, "ready": false, "waitingFor": …}` and changes nothing; the box
stays claimable. `GET /api/setup/claim` carries the same `ready` and
`waitingFor` fields, and the first-run wizard polls it and lets the owner
proceed on its own once they flip. On a fresh box this takes a few minutes:
the Nextcloud pod runs `occ maintenance:install` before it serves anything.

The first-run wizard is that caller. Every later visit to the admin pages
asks for the **password** the wizard set, the same one that signs in to
LosOS cloud: `POST /api/sign-in` asks Nextcloud whether it is right and hands
the tab the token on a yes. The box keeps no second copy of the password, so
changing it inside LosOS cloud changes it for the admin pages too.

The token itself is still shown once, after the password is set, as the
**spare admin key**, with Copy and Print buttons. It is for the one case the
password cannot cover: LosOS cloud not running, which is what checks the
password. The unlock dialog has a link to enter it instead.

A new password has to be at least 12 characters with a lower-case letter, an
upper-case letter, a digit and a symbol such as `-` or `!`; the wizard shows
the four rules with a tick each as you type. A password set before this rule existed still signs in.

There is no way to rotate the token from the box: it has no shell. Losing the
password *and* the spare key means reinstalling from the install medium,
which wipes the disk.

## Settings

Settings are Nix. The settings page generates `modules/overrides.nix` and
runs `nixos-rebuild switch`. So:

- A change takes a rebuild, not a restart.
- The box can be rebuilt from this repository plus that one file.
- `apply` replaces `overrides.nix` entirely. A setting the UI does not write
  is reset on the next save.

## Users

Two data users, both without passwords or shells:

| User        | Owns                |
| ----------- | ------------------- |
| `notshared` | Nextcloud           |
| `shared`    | Mesh storage        |

Each has its own primary group and a mode-700 home, so neither can read the
other's files. `tests/impermanence.nix` checks this.

## Upgrades and reboots

- **03:00** — `system.autoUpgrade` rebuilds from `losos.upgradeFlakeUri`. The
  default, `git+file:///etc/nixos#install`, does not pull new packages. Set a
  `github:` URI to get updates. Keep the `#install` fragment; without it the
  rebuild fails. A remote URI still uses this box's drive list, firmware
  mode, unlock mode and settings: the rebuild reads them from `/etc/nixos`
  on the box, not from the published repository.
- **00:07** — `midnight-reboot.timer` reboots unconditionally
  (`Persistent=true`, so a box that was off catches up). Because the root is
  rebuilt on boot, the reboot is how the box repairs itself.
