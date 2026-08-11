# losos

Stateless NixOS configuration: tmpfs root with impermanence on an encrypted
`/persist`, Nextcloud for the `notshared` user, Tahoe-LAFS for the `shared`
user, automatic upgrades, and a nightly reboot. Disk encryption unlocks via
TPM2 when the machine has one, or a random keyfile when it does not.

## Layout

| File / dir | Purpose |
| --- | --- |
| `flake.nix` | Flake entrypoint; `iso` (installer) and `install` (target) systems, plus `packages` and `devShells` outputs |
| `flake/packages.nix` | `losos-app` (the Nextcloud app as a store path) + `losos-ctl` (Haskell backend, `callCabal2nix`) |
| `flake/devshell.nix` | `mkShell` dev environment: full Haskell stdlib + PHP tooling + `occ`, host fish config minus Zellij |
| `modules/options.nix` | `losos.*` option declarations (incl. `losos.backend.package`) |
| `modules/configuration.nix` | Networking, locale, packages, users |
| `modules/disko.nix` | Disk layout: every drive → LVM PV → `persist-vg` → LUKS/btrfs `/persist`; tmpfs `/` |
| `modules/installer.nix` | Packages `losos-install` and bakes the flake source into the ISO |
| `install/losos-install.sh` | The minimal auto-installer: detect drives → LVM via disko → nixos-install |
| `tests/install.nix` | nixos-test-vms config that runs the installer against 3 VM disks |
| `modules/boot.nix` | systemd-boot, systemd initrd, TPM2, keyfile secret |
| `modules/impermanence.nix` | What survives reboot (bind-mounted from `/persist`) |
| `modules/services.nix` | Nextcloud (`notshared`) + Tahoe-LAFS (`shared`) + the losos app + sudoers grant |
| `modules/updates.nix` | `system.autoUpgrade` + midnight reboot timer |
| `modules/defaults.nix` | Project defaults (`sharingMyStorage`, `backend.package`) |
| `backend/` | Haskell `losos-ctl` backend (type-class effect design) + `schema.json` |
| `nextcloud-app/` | The losos Nextcloud admin app (PHP) — the Local/Mesh toggle UI |

## Build the installer ISO

```sh
nix build .#nixosConfigurations.iso.config.system.build.isoImage
# write the result ISO to a USB stick, boot it on the target machine
```

The ISO bakes the flake source at `/etc/losos/flake-source` and ships a
`losos-install` command that does the whole first install unattended (see
"First install" below).

## First install (auto-installer)

1. Boot the `iso` on the target machine.
2. Run the installer — it finds every fixed, non-removable disk that isn't
   already mounted, merges them all into one LVM volume group via the disko
   layout in `modules/disko.nix`, formats it, installs the `install` system,
   and lays the flake down at `/persist/etc/nixos` as a git repo so the box
   can auto-upgrade:
   ```sh
   sudo losos-install
   ```
   By default this uses a random keyfile (unattended). Add `--tpm` to use
   TPM2 instead (you'll be prompted for a passphrase at format time, then
   enroll TPM2 after first boot — see below). Override drive auto-detection
   with `--drives /dev/sdb,/dev/sdc`, or stop after disko with `--no-install`.
3. Reboot into the installed system.

The installer writes the detected drive list to `modules/install-target.nix`
(a file `flake.nix` imports only when it exists — *not* to `modules/overrides.nix`,
which `losos-ctl apply` rewrites wholesale from the admin form) and commits
it into the local `/persist/etc/nixos` git repo, so the default
`git+file:///etc/nixos` auto-upgrade keeps seeing it.

### Manual install (fallback)

If you'd rather drive disko by hand:

1. Set `losos.targetDrives` (in `modules/install-target.nix` or a host
   override) if the disks aren't `/dev/sda` — list several to merge their
   capacity into one LVM pool.
2. If the machine has **no TPM2**, generate the keyfile **before** formatting
   and **back it up** (you'll need the exact bytes again after first boot):
   ```sh
   sudo install -d /etc/keys
   sudo dd if=/dev/urandom of=/etc/keys/persist-keyfile bs=512 count=8
   sudo chmod 600 /etc/keys/persist-keyfile
   cp /etc/keys/persist-keyfile /run/media/$USER/usb-stick/persist-keyfile  # backup
   ```
3. Format and install using disko:
   ```sh
   sudo disko --mode destroy,format,mount --flake .#install
   sudo nixos-install --flake .#install --no-root-passwd
   ```
4. Lay the flake at `/persist/etc/nixos` as a git repo (so auto-upgrade works):
   ```sh
   sudo cp -a /etc/losos/flake-source /mnt/persist/etc/nixos
   sudo cp /etc/keys/persist-keyfile /mnt/persist/etc/keys/persist-keyfile   # non-TPM only
   cd /mnt/persist/etc/nixos && sudo git init && sudo git add -A \
     && sudo git -c user.email=losos@local -c user.name=losos commit -m "losos install"
   ```
5. Create the Nextcloud admin password file (persisted via `/var`):
   ```sh
   sudo mkdir -p /mnt/persist/var/secrets
   printf 'change-me' | sudo tee /mnt/persist/var/secrets/nextcloud-admin-pass >/dev/null
   sudo chmod 600 /mnt/persist/var/secrets/nextcloud-admin-pass
   ```

## Test the installer in VMs

```sh
nix build .#checks.x86_64-linux.losos-install
```

`tests/install.nix` boots a VM with three empty disks (`/dev/vdb`, `/dev/vdc`,
`/dev/vdd`) plus its own root on `/dev/vda`, runs `losos-install`, and asserts
detection skips the VM root, disko merges all three disks into one LVM volume
group `persist-vg`, the `persist` LV opens as LUKS and mounts as btrfs, and
the ESP lands on the first target drive only.

## Enroll the TPM2 unlock (TPM machines only)

After first boot into the installed system:

```sh
sudo systemd-cryptenroll --tpm2-device=auto --tpm2-pcrs=0+7 /dev/persist-vg/persist
# /dev/persist-vg/persist is the logical volume holding the LUKS header
```

The initrd then unlocks `/persist` via TPM2 (`crypttabExtraOpts =
[ "tpm2-device=auto" ]`). The keyfile path is not used on TPM machines. On
non-TPM machines the initrd uses `/crypto_keyfile.bin` (injected from
`/etc/keys/persist-keyfile` via `boot.initrd.secrets`); a password prompt is the
last-resort fallback (implied by systemd stage 1).

### Keyfile path: restore the keyfile after first boot (non-TPM only)

The first-boot initrd was baked from the keyfile on the installer ISO. After
first boot, `/etc/keys` (persisted via impermanence) is empty, so restore the
backup so future rebuilds (including auto-upgrade) can re-bake the initrd:

```sh
sudo install -d /etc/keys
sudo cp /run/media/$USER/usb-stick/persist-keyfile /etc/keys/persist-keyfile
sudo chmod 600 /etc/keys/persist-keyfile
```

From now on `/etc/keys/persist-keyfile` lives on the encrypted `/persist`, so
it is only readable once the volume is already unlocked (no extra at-rest
exposure).

## Connect the Tahoe-LAFS node

The local introducer generates its `introducer.furl` on first start:

```sh
sudo cat /var/db/tahoe-lafs/introducer-local/private/introducer.furl
```

Paste that into `losos.tahoe.introducerFurl` (in `modules/configuration.nix` or a host
override) and rebuild. The `shared` node then joins the local grid and, with
`losos.sharingMyStorage = true`, provides storage. The web UI is on
`http://<host>:3456`.

## Auto-upgrade and reboot

- `system.autoUpgrade` rebuilds from `losos.upgradeFlakeUri` (default
  `git+file:///etc/nixos`) at 03:00, rebooting only if the kernel/initrd
  changed. The flake source is persisted under `/persist/etc/nixos` so the box
  can rebuild itself with no human present.
- `midnight-reboot.timer` reboots every day at 00:07 (`Persistent = true` catches
  up if the machine was asleep).
- For the box to actually pull **new** nixpkgs versions unattended, publish the
  flake to GitHub and set `losos.upgradeFlakeUri = "github:owner/losos";`. The
  local default only rebuilds the pinned `flake.lock` (it keeps the system
  consistent, it doesn't advance the nixpkgs pin).

## Set-and-forget / no login

This is an appliance: **SSH is disabled** and neither `notshared` nor `shared`
has a password, so neither Linux account can be logged into (NixOS never
allows empty-password login). The two data domains are isolated — each home is
mode `750`, so neither user can read the other's files — and you reach your
data only through the service web UIs:

- `notshared` → Nextcloud web UI (`http://<host>/`, admin login `notshared`)
- `shared` → Tahoe-LAFS web UI (`http://<host>:3456`)

There is deliberately **no way to get a shell** on the installed system. The
one config change an admin can make from the running box is the **Local ↔ On
the mesh** toggle in the Nextcloud admin panel (see below): it flips
`losos.sharingMyStorage` and triggers a rebuild. Anything beyond that —
hostname, HTTPS, the upgrade flake URI, Tahoe introducer — still requires
reinstalling from the ISO, or pushing a new commit to a published flake and
letting `system.autoUpgrade` pick it up.

## Machine configuration from the web UI (the losos plugin)

The `losos` Nextcloud app adds an admin-settings panel that toggles this box
between **Local only** (private Nextcloud, `sharingMyStorage = false`) and **On
the mesh** (contributing storage to the Tahoe grid, `sharingMyStorage = true`)
and triggers a NixOS rebuild, with progress shown as a Nextcloud notification.

- The panel is admin-only (the controller re-checks admin membership on every
  endpoint) and POST routes are CSRF-protected by the AppFramework.
- The PHP app (`nextcloud-app/`) talks to a privileged Haskell backend,
  `losos-ctl`, over a locked-down sudoers rule: user `nextcloud` → root,
  NOPASSWD, command pinned to exactly the `losos-ctl` binary. So the PHP-FPM
  process can trigger a rebuild without ever holding root or a shell.
- `losos-ctl` (`backend/`, built via `callCabal2nix`) rewrites the
  `losos.sharingMyStorage` line in the persisted flake, spawns a detached
  `nixos-rebuild switch` (`setsid -f`, so it outlives the request), and records
  rebuild progress in a state file the panel polls. Its logic is written
  against a `Losos` effect type class with an `IO` interpreter (production) and
  a pure `TestM` interpreter (tests), so the state machine is unit-tested
  without a filesystem or a real rebuild. The wire contract is formalised in
  `backend/schema.json`.
- Set `losos.backend.package = null` (in `modules/defaults.nix` or a host override) to
  run without the backend; the panel then shows "backend not installed" instead
  of failing.

## Development environment

A `devShells.<system>.default` output gives a full Haskell + PHP environment for
working on the plugin and backend, using the host's fish config **minus
Zellij**:

```sh
nix develop .#
```

This gives GHC with a broad standard-library set (`ghcWithPackages`), `cabal-install`,
HLS, PHP 8.3 with the extensions the app needs, Composer, `nextcloud34` (for
`occ`), and the shell UX tools (bat/eza/zoxide/fastfetch/starship). Helper
commands inside the shell: `losos-test` / `losos-lint` (Haskell), `ncd` (enter
the Nextcloud app dir), `bcd` (enter the backend dir). To build and check
everything from the host without entering the shell:

```sh
nix build .#losos-ctl .#losos-app                         # backend + app
nix build .#nixosConfigurations.install.config.system.build.toplevel  # the system
```

## Notes / things to set for real use

- `losos.nextcloud.hostName` (default `localhost`) and
  `losos.nextcloud.https` — set a real domain and enable HTTPS for remote use.
- `losos.upgradeFlakeUri` — set to your published flake for unattended upgrades.
- Consider pinning `nixpkgs` to a stable branch instead of `nixos-unstable`
  for a server that upgrades itself automatically.