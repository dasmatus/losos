# losos

A NixOS appliance for a mini-PC. It runs Nextcloud for your own files and can
lend spare disk and CPU to a mesh of other losos boxes. There is no SSH and no
login shell: you administer it from a web page, and it upgrades itself.

The root filesystem is a tmpfs rebuilt on every boot. Only listed directories
survive, on an encrypted `/persist` partition.

**Owner's handbook: [losos.dasmat.us](https://losos.dasmat.us)** (source in
[`handbook/`](handbook/); every box also serves it at `/handbook/`).
**Developer documentation: [In depth](https://losos.dasmat.us/in-depth)**,
the handbook's last chapter.

## What runs on it

| Service          | Notes                                                                    |
| ---------------- | ------------------------------------------------------------------------ |
| **Nextcloud**    | Files, calendar, contacts for the `notshared` user, at `<host>.local/nextcloud` |
| **Mesh storage** | Optional. Spare disk for the mesh as the `shared` user, replicated by Longhorn |
| **Mesh compute** | Optional. Spare CPU for the mesh inside a time window, only when idle    |
| **Forgejo**      | Optional git hosting at `<host>.local/forgejo/`                          |
| **Admin UI**     | Settings page at `<host>.local` (or the IP on the box's screen), LAN only |
| **Master proxy** | Optional. Public access through a tunnel to a VPS, no inbound port at home |

## Install

1. Download the ISO from the
   [latest release](https://github.com/dasmatus/losos/releases/latest), or
   build it with
   `nix build .#nixosConfigurations.iso.config.system.build.isoImage`.
2. Boot the target in UEFI mode, with Secure Boot off or with the LosOS
   certificate from the release page enrolled: release ISOs are signed and
   a firmware that trusts the certificate verifies the stick before it
   runs. The installed box needs Secure Boot off.
3. Boot the stick with a network connection. The installer wipes every fixed
   disk, encrypts them and installs without prompting.

Details, the demo VM image and disk growth: [Install in depth](https://losos.dasmat.us/in-depth/install).
The disk key is sealed to the machine's TPM chip; on a machine without one it
is stored unencrypted on the boot partition. See the
[security model](docs/security-model.md).

## Develop

```sh
devenv shell        # or: direnv allow
devenv test         # check-pins, lint, rust tests, flake eval
devenv shell vm-tests
```

See [docs/development.md](docs/development.md) and
`CLAUDE.md` for architecture notes and known pitfalls.

## Licence

AGPL-3.0-or-later. The repository is [REUSE](https://reuse.software)-compliant;
see `REUSE.toml` and `LICENSES/`.
