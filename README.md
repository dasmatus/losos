<h1><img src="admin-ui/themes/brand/salmon-128.png" alt="" width="56" height="56" align="center"> LosOS</h1>

A NixOS appliance for a mini-PC. It runs LosOS Cloud for your own files and can
lend spare disk and CPU to a mesh of other LosOS boxes. There is no SSH and no
login shell: you administer it from a web page, and it upgrades itself.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="handbook/docs/img/overview-dark.png">
  <img src="handbook/docs/img/overview.png" alt="The admin pages of a box: the app tiles for LosOS Cloud and LosOS Git, the storage card and a board of widgets." width="800">
</picture>

The root filesystem is a tmpfs rebuilt on every boot. Only listed directories
survive, on an encrypted `/persist` partition.

**Owner's handbook: [losos.dasmat.us](https://losos.dasmat.us)** (source in
[`handbook/`](handbook/); every box also serves it at `/handbook/`).
**Developer documentation: [In depth](https://losos.dasmat.us/in-depth)**,
the handbook's last chapter.

## What runs on it

| Service          | Notes                                                                    |
| ---------------- | ------------------------------------------------------------------------ |
| **LosOS Cloud**  | Files, photos, calendars, contacts and boards for everyone who signs in, at `<host>.local/cloud/`; the Nextcloud desktop and phone apps sync with it |
| **Mesh storage** | Optional. Spare disk for the mesh as the `shared` user, replicated by Longhorn |
| **Mesh compute** | Optional. Spare CPU for the mesh inside a time window, only when idle    |
| **LosOS Git**    | The box's own Git server at `<host>.local/git/`                          |
| **Admin UI**     | Settings page at `<host>.local` (or the IP on the box's screen), LAN only |
| **Master proxy** | Optional. Public access through a tunnel to a VPS, no inbound port at home |

## Install

1. Download the ISO from the newest release on the
   [releases page](https://github.com/dasmatus/losos/releases), or
   build it with
   `nix build .#nixosConfigurations.iso.config.system.build.isoImage`.
2. Boot the target in UEFI mode, with Secure Boot off or with the LosOS
   certificate from the release page enrolled. Release ISOs are signed, and
   a firmware that trusts the certificate verifies the stick before it
   runs. The installed box needs Secure Boot off.
3. Boot the stick with a network connection. The installer wipes every fixed
   disk, encrypts them and installs without prompting.

Details, the demo VM image and disk growth: [Install in depth](https://losos.dasmat.us/in-depth/install).
The disk key is sealed to the machine's TPM chip; on a machine without one it
is stored unencrypted on the boot partition. See the
[security model](docs/security-model.md).

## Manage it from a terminal

`losos` is a terminal client with the admin pages' panes, for a box you
reach over the network. It needs no SSH: every call is signed by your GPG
key and encrypted to the box. The box trusts only keys that were on the
install stick, so put yours there before you install. On 64-bit x86 Linux:

```sh
curl -fsSLO https://nightly.link/dasmatus/losos/workflows/ci/main/losos-pages-updates.zip
unzip -o losos-pages-updates.zip -d losos-pages-updates
sh losos-pages-updates/install.sh
losos key import --gpg you@example.com
losos login http://<address on the box's screen>
```

The token `login` asks for comes from **Settings → Access tokens** in the
admin pages. [The terminal client](https://losos.dasmat.us/manual/terminal/)
walks through all of it, including how to build a stick with your key.

## Develop

```sh
devenv shell        # or: direnv allow
devenv test         # check-pins, lint, rust tests, flake eval
devenv shell vm-tests
```

See [docs/development.md](docs/development.md) and
[`.claude/CLAUDE.md`](.claude/CLAUDE.md) for architecture notes and known
pitfalls.

| Folder      | What is in it                                                     |
| ----------- | ----------------------------------------------------------------- |
| `admin-ui/` | The admin pages, the `losos` terminal client and LosOS Diagrams   |
| `backend/`  | `lososd` and `losos-ctl`, the box's control plane (Rust)          |
| `edge/`     | The edge registrar, its Vercel demo host and the escrow contracts |
| `modules/`  | The NixOS modules both systems are built from                     |
| `nix/`      | Packages, images and the NixOS VM tests                           |
| `handbook/` | The owner's handbook                                              |
| `docs/`     | Developer notes, the security model and design records            |

## Licence

AGPL-3.0-or-later. The repository is [REUSE](https://reuse.software)-compliant;
see `REUSE.toml` and `LICENSES/`.
