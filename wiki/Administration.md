# Administration

## Admin UI

Open `http://<ip>/` (the address on the box's tty1 banner) or
`https://<host>.local/` from the LAN. The admin UI is blocked for anything
outside the LAN, including traffic through the [master proxy](Master-Proxy).
`<host>.local` is an mDNS name. A computer that does not resolve it, typically
the host of a NAT'ed VM, uses the IP address, and every route below answers
on it the same way.

Services on the same host:

| Service          | URL                       |
| ---------------- | ------------------------- |
| Admin UI         | `<host>.local/`           |
| Nextcloud        | `<host>.local/nextcloud`  |
| Forgejo (opt-in) | `<host>.local/forgejo/`   |

The box's IP address (the one on the tty1 banner) and its bare name work in
place of `<host>.local` everywhere, Nextcloud included. That is the way in
from a libvirt VM, which gets no mDNS name. nginx tells the Nextcloud pod
which address each request arrived on, and the pod trusts exactly that one,
never a wildcard (`modules/workloads.nix`). Nextcloud trusts `localhost` and
`127.0.0.1` on its own.

`losos.tls.enable` (on by default) adds HTTPS on :443 with a certificate the
box generates itself, valid for two years. Plain HTTP on :80 stays open.
Browsers do not trust the certificate until you install it. The first-run
wizard offers a one-line installer for macOS, Linux and Windows
(`/setup/trust.sh`, `/setup/trust.ps1`, both LAN-only) and the plain download
(`/setup/losos-ca.crt`). See [Install](Install).

## Look

**Settings, Look** changes how the admin pages look, for every browser that
opens the box. Nothing here is a setting in the Nix sense. It is a document
lososd keeps at `/var/lib/losos/look.json`, so a change takes effect the
moment it is saved, with no rebuild.

### Background

Pick one of the three shipped pictures, or upload one of your own (PNG, JPEG,
WebP, GIF or SVG, up to 8 MiB). The **Veil** slider lays the page colour over
the picture, from 20 % to 90 %, so text stays readable in both the light and
the dark theme. *Plain* removes the picture again.

lososd serves an uploaded picture at `/api/look/background` without a token,
because a CSS `background-image` cannot send one. The route sits behind the
same LAN-only guard as the rest of the admin pages, and a wallpaper is not a
secret.

### Widgets written by hand

The Overview board's gallery (**Add a widget**) has two ways to make your
own. *Build one* composes a widget from the box's readings without any code.
*Write one* takes HTML, style and script of your own, kept on the box and
listed on this pane, where you can edit and delete it.

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

The editor's **Help** tab repeats this with an example. Its preview is the
real frame, so what it shows is what the tile will show. A widget can fetch
the internet (a weather tile, say) but not the box. The limit is 24 widgets
of 64 KiB each.

## Admin token

`lososd` writes a random 64-hex-character token to
`/var/secrets/losos-admin-token` (mode 0600) on first start. The token can
change any setting, so it is root.

On a new, unclaimed box, the first LAN caller to submit `POST /api/setup/claim`
sets the owner password and receives the token in the response. This route
requires no token, but stops accepting claims once the box has an owner. Set
up the box only on a trusted LAN, because the first caller becomes its owner.
The token is handed back only once and cannot be fetched again through the UI.

A claim sent before Nextcloud has finished its first start answers `503` with
`{"error": …, "ready": false, "waitingFor": …}` and changes nothing. The box
stays claimable. `GET /api/setup/claim` carries the same `ready` and
`waitingFor` fields. The first-run wizard polls it and lets the owner proceed
on its own once they flip. On a fresh box this takes a few minutes, because
the Nextcloud pod runs `occ maintenance:install` before it serves anything.

`ready` flips when that install finishes, which is before LosOS cloud answers
a browser. The pod then enables its 26 apps and only after that starts
Apache. Measured natively on a 4-core host (2026-10-07), the install takes
about 11 s, enabling the apps about 13 s, and the first page answers 27 s
after the pod started. The wizard's sign-in step shows "Your files are still
starting" for that gap. A reboot repeats the same steps against an installed
instance in about 2 s. In a VM without KVM (QEMU's TCG emulation) the same
steps take 10 to 30 minutes, so give a demo VM hardware virtualisation.

The first-run wizard is that caller. Every later visit to the admin pages
asks for the **password** the wizard set, the same one that signs in to LosOS
cloud. `POST /api/sign-in` asks Nextcloud whether it is right and hands the
tab the token on a yes. The box keeps no second copy of the password, so
changing it inside LosOS cloud changes it for the admin pages too.

The token itself is still shown once, after the password is set, as the
**spare admin key**, with Copy and Print buttons. It covers the one case the
password cannot: LosOS cloud not running, since LosOS cloud is what checks the
password. The unlock dialog has a link to enter it instead.

A new password has to be at least 12 characters with a lower-case letter, an
upper-case letter, a digit and a symbol such as `-` or `!`. The wizard shows
the four rules with a tick each as you type. A password set before this rule
existed still signs in.

There is no way to rotate the token from the box, since it has no shell.
Losing the password *and* the spare key means reinstalling from the install
medium, which wipes the disk.

## Settings

Settings are Nix. The settings page generates `modules/overrides.nix` and
runs `nixos-rebuild switch`. So:

- A change takes a rebuild, not a restart.
- The box can be rebuilt from this repository plus that one file.
- `apply` replaces `overrides.nix` entirely. A setting the UI does not write
  is reset on the next save.

### Advanced

**Settings, Advanced** lists every `losos.*` option the box declares, read
from the modules it runs. `flake/options-doc.nix` generates the document at
build time, and lososd serves it at `GET /api/options` with the current
`overrides.nix` joined in. Each row shows the option's description, its
default, the value the box is running with, and an editor for its type: a
switch, a number with its bounds, a choice, a text field, or one item per
line for a list.

- An option one of the other panes already edits (the name, the apps' run
  modes, the hardening switches) is shown with its value and a link to that
  pane, so there is one place to change it.
- The three the installer writes (`targetDrives`, `tpm.enable`, `bios`) and
  the packages the build chooses are read-only. A line for one of them in
  `overrides.nix` would fail the next rebuild.
- Options marked **Careful** (`admin.*`, `cluster.*`, `hostName`,
  `storage.*`, `tls.*`, `upgradeFlakeUri`, …) ask once before the first
  change. A wrong value leaves a box with no shell to fix it from.
- **Use default** removes the line rather than writing the default's value,
  so a default that moves with an update is followed.
- A line in `overrides.nix` the box has no option for (a typo, a setting
  from another version) is shown in its own group and blocks Apply until it
  is removed. lososd refuses such a body too (`backend/src/options.rs`,
  `check_body`). It checks every line of an apply against the document:
  declared, not read-only, a value of the right kind, no `${`.

Everything written here goes through the same Apply and the same rebuild as
the other panes. `losos.edge.*` is left out because the appliance never reads
it.

### History and LosOS Git

Every Apply, storage change and factory reset is one commit in `/etc/nixos`,
the repository `losos-ctl install` made. The commit is named after the
settings it changed, `Change hostName, cluster.enable`, with the before and
after of each in the body. **Settings, History** lists the last forty.

With `losos.configRepo.enable` (the default) and LosOS Git on, lososd also
keeps the configuration in a private repository on LosOS Git,
`<owner>/losos-config`, owned by the admin's own account (`notshared`). The
account is created on the first sync with the admin password and kept in
step with it on every password change and sign-in. A reconciler in lososd
runs every 30 s:

- it commits anything uncommitted and pushes the box's branch when LosOS
  Git is behind;
- when LosOS Git is ahead, because you cloned the repository, edited
  `modules/overrides.nix` and pushed, it fast-forwards to it, checks the new
  `overrides.nix` line by line the way Apply is checked, and starts a
  rebuild. A refused push is reported under History and the box stays on
  its own commit;
- when the two have diverged (a rewritten history) it does nothing and says
  so. Sort it out from a clone;
- while a rebuild is running, the push waits for the next tick.

**Sync now** on the History pane runs one tick immediately. `losos-ctl
config` prints the same document, and `losos-ctl config --sync` runs a tick.
Secrets never enter the repository. `overrides.nix` carries option values
only, and the token lososd authenticates to LosOS Git with lives in
`/var/lib/forgejo/.losos-token`, outside `/etc/nixos`. There is no Forgejo
Actions runner behind this. The box polls.

The clone address is shown on the pane (`http://<box>/forgejo/<owner>/
losos-config.git`). The repository is private, so it needs the admin's
password. The daemon talks to Forgejo on loopback with a bot account,
`losos`, that the Forgejo start-up script creates and mints a token for
(`flake/forgejo-bootstrap.nix`).

## Users

Two data users, both without passwords or shells:

| User        | Owns                |
| ----------- | ------------------- |
| `notshared` | Nextcloud           |
| `shared`    | Mesh storage        |

Each has its own primary group and a mode-700 home, so neither can read the
other's files. `tests/impermanence.nix` checks this.

## Upgrades and reboots

- **03:00.** `system.autoUpgrade` rebuilds from `losos.upgradeFlakeUri`. The
  default, `git+file:///etc/nixos#install`, does not pull new packages. Set a
  `github:` URI to get updates. Keep the `#install` fragment; without it the
  rebuild fails. A remote URI still uses this box's drive list, firmware
  mode, unlock mode and settings, because the rebuild reads them from
  `/etc/nixos` on the box, not from the published repository.
- **00:07.** `midnight-reboot.timer` reboots unconditionally
  (`Persistent=true`, so a box that was off catches up). Because the root is
  rebuilt on boot, the reboot is how the box repairs itself.
