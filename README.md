# losos

A NixOS appliance for a mini-PC. It runs Nextcloud for your own files and can
lend spare disk and CPU to a mesh of other losos boxes. There is no SSH and no
login shell: you administer it from a web page, and it upgrades itself.

The root filesystem is a tmpfs rebuilt on every boot. Only listed directories
survive, on an encrypted `/persist` partition.

**Documentation: [the wiki](https://github.com/dasmatus/losos/wiki)**
(source in [`wiki/`](wiki/)).

## What runs on it

| Service          | Notes                                                                    |
| ---------------- | ------------------------------------------------------------------------ |
| **Nextcloud**    | Files, calendar, contacts for the `notshared` user, at `<host>.local/nextcloud` |
| **Mesh storage** | Optional. Spare disk for the mesh as the `shared` user, replicated by Longhorn |
| **Mesh compute** | Optional. Spare CPU for the mesh inside a time window, only when idle    |
| **Forgejo**      | Optional git hosting at `<host>.local/forgejo/`                          |
| **Admin UI**     | Settings page at `<host>.local`, LAN only                                |
| **Master proxy** | Optional. Public access through a tunnel to a VPS, no inbound port at home |

## Install

1. Download the ISO from the
   [latest release](https://github.com/dasmatus/losos/releases/latest), or
   build it with
   `nix build .#nixosConfigurations.iso.config.system.build.isoImage`.
2. Turn Secure Boot off and boot the target in UEFI mode.
3. Boot the stick with a network connection. The installer wipes every fixed
   disk, encrypts them and installs without prompting.

Details, the demo VM image and disk growth: [Install](https://github.com/dasmatus/losos/wiki/Install).
Without a TPM, the disk key is stored unencrypted on the boot partition; see
the [security model](docs/security-model.md).

## Develop

```sh
devenv shell        # or: direnv allow
devenv test         # pin checks, lint, Rust tests and Nix evaluation; not builds or VM tests
devenv shell vm-tests
```

See [Development](https://github.com/dasmatus/losos/wiki/Development) and
`CLAUDE.md` for architecture notes and known pitfalls.

## Licence

AGPL-3.0-or-later. The repository is [REUSE](https://reuse.software)-compliant;
see `REUSE.toml` and `LICENSES/`.
