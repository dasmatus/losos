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

The box's IP address (the one the tty1 banner shows) and its bare name work
in place of `<host>.local` everywhere, Nextcloud included. That is the way in
from a libvirt VM, which gets no mDNS name: nginx tells the Nextcloud pod
which address each request arrived on, and the pod trusts exactly that one,
never a wildcard (`modules/workloads.nix`). `localhost` and `127.0.0.1`
Nextcloud trusts on its own.

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
