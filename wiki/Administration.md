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
change any setting, so it is equivalent to root. To reset it, delete the file
and restart `lososd`; editing it by hand does not work.

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
  rebuild fails.
- **00:07** — `midnight-reboot.timer` reboots unconditionally
  (`Persistent=true`, so a box that was off catches up). Because the root is
  rebuilt on boot, the reboot is how the box repairs itself.
