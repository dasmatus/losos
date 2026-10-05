# Administration

## Admin UI

Open `https://<host>.local/` from the LAN. The admin UI is blocked for anything
outside the LAN, including traffic through the [master proxy](Master-Proxy).

Services on the same host:

| Service          | URL                       |
| ---------------- | ------------------------- |
| Admin UI         | `<host>.local/`           |
| Nextcloud        | `<host>.local/nextcloud`  |
| Forgejo (opt-in) | `<host>.local/forgejo/`   |

`losos.tls.enable` (on by default) adds HTTPS on :443 with a certificate the
box generates itself, valid for two years. Plain HTTP on :80 stays open.
Browsers do not trust the certificate until you install it; the first-run
wizard walks you through that.

## Admin token

`lososd` writes a random 64-hex-character token to
`/var/secrets/losos-admin-token` (mode 0600) on first start. The token can
change any setting, so it is equivalent to root.

On a new, unclaimed box, the first LAN caller to submit `POST /api/setup/claim`
sets the owner password and receives the token in the response. This route
requires no token, but stops accepting claims once the box has an owner. Set up
the box only on a trusted LAN; the first caller becomes its owner. The token is
handed back only once and cannot be fetched again through the UI.

The first-run wizard is that caller. After it sets the password it shows the
token as the **admin key**, with a Copy button, and prints it on the recovery
sheet in the next step under the recovery code. That is the only time the box
shows it: the browser tab remembers it until it is closed, and every later
visit to the admin pages asks for it. Keep it with the recovery code.

There is no way to rotate the token from the box: it has no shell, and a
claimed UI cannot fetch a replacement. Losing the key means reinstalling from
the install medium, which wipes the disk.

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
