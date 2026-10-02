# Architecture

## Systems in the flake

| Output                        | What                                             |
| ----------------------------- | ------------------------------------------------ |
| `nixosConfigurations.iso`     | Live installer ISO (`options`, `disko`, `installer`) |
| `nixosConfigurations.install` | The installed appliance (all modules)            |
| `nixosModules.edge`           | The VPS side of the [master proxy](Master-Proxy) and mesh |

All project options live under `losos.*` in `modules/options.nix`, with
defaults in `defaults.nix`. Modules read `config.losos.*`.

## Impermanence

`/` is a tmpfs. `/persist` is LUKS-encrypted ext4. `impermanence` bind-mounts
these back from `/persist`: `/nix`, `/var`, `/etc/ssh`, `/etc/keys`,
`/etc/nixos`, `/etc/rancher`, the two data homes and `machine-id`.

**Anything not on that list is lost on reboot.** New state must be added to
`modules/impermanence.nix`.

## Control plane

| Component      | Role |
| -------------- | ---- |
| `lososd`       | Root systemd daemon (Rust). Sole writer of `/var/lib/losos/state.json`. D-Bus service `org.losos1` and a token-authenticated JSON API on `127.0.0.1:8082`. |
| `losos-ctl`    | CLI that forwards each subcommand to `lososd` over D-Bus. `losos-ctl install` runs standalone on the ISO. |
| `losos-admin-ui` | React + Vite + Tailwind SPA, served by nginx, which proxies `/api/*` to `lososd`. |

Rebuilds run as transient systemd units (`losos-rebuild-<job>`).
`nixos-rebuild switch` restarts `lososd`, so on startup it re-attaches to any
rebuild still recorded as `building`.

See [`backend/README.md`](https://github.com/dasmatus/losos/blob/main/backend/README.md)
for the crate layout and [`backend/schema.json`](https://github.com/dasmatus/losos/blob/main/backend/schema.json)
for the wire format.

## Workloads

Nextcloud and Forgejo run in the box's own k3s cluster
(`losos.<svc>.mode = "container"`) or natively on the host. The pods use
`hostNetwork`. nginx is the only service on public ports and routes by path.

## Repository layout

```
modules/                 NixOS modules; options in options.nix
backend/                 lososd + losos-ctl (Rust)
backend-registrar/       master-proxy edge registrar (Rust)
admin-ui/app/            the admin SPA
admin-ui/design-system/  dev-only tokens and React wrapper; not shipped
tests/                   NixOS VM tests
docs/                    security model, design specs and plans
wiki/                    source of this wiki
```

`CLAUDE.md` lists the failure modes that are easy to reintroduce.
