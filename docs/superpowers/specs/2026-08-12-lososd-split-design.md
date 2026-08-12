# lososd daemon + facade split, standalone admin endpoint, and nspawn containers

Date: 2026-08-12
Status: design

## Context

`losos-ctl` is today a single Haskell binary that the losos Nextcloud app invokes
via `sudo -n` (sudoers: `nextcloud` → root, NOPASSWD, pinned binary). It rewrites
`losos.*` config lines, spawns a `nixos-rebuild` as a detached `setsid -f sh -c`
shell, and tracks progress in `/var/lib/losos/state.json`, with the detached shell
re-entering the binary via `rebuild-done <ec> <job>` when the rebuild finishes.

Problems this creates:

- **The whole web stack is one NOPASSWD rule away from root.** The sudo bridge is
  the only hardening between PHP-FPM and `nixos-rebuild switch`.
- **The rebuild is unsupervised.** `setsid` orphans it deliberately; two writers
  (the CLI and the detached shell) race on `state.json`; a reboot mid-rebuild
  leaves `building` forever.
- **Rootless Podman carries real complexity**: a dedicated `containers` user,
  subuid/subgid plumbing, linger markers, an image registry dependency
  (docker.io/codeberg.org) at runtime, and AIO's mastercontainer spawning its
  siblings over a podman socket inside its own container.
- **The OS settings UI lives inside Nextcloud**, coupling appliance
  administration to the cloud app (and to the cloud actually being up).

## Goals

1. Split `losos-ctl` into a **systemd daemon (`lososd`)** holding all privileged
   logic, and a **facade (`losos-ctl`)** that unprivileged callers use.
2. Supervise rebuilds as real systemd units; expose live progress.
3. Move the OS settings UI out of Nextcloud to **its own Nginx endpoint** backed
   by a loopback JSON API on `lososd`.
4. Replace rootless Podman with **declarative NixOS Containers**
   (systemd-nspawn) running the native service stacks.

Non-goals: polkit integration (group ACL is enough for a single-admin
appliance); keeping any Podman path; a JS build pipeline for the admin UI.

## Part A — daemon + facade over D-Bus

### Components

The cabal project keeps one package, gains a second executable:

- **`lososd`** (new) — root systemd service, claims a well-known name on the
  **system bus**: `org.nixos.losos1` at object path `/org/nixos/losos1`,
  interface `org.nixos.Losos.Control1`. One method per current web-facing
  subcommand:

  | method             | in            | out                       |
  |--------------------|---------------|---------------------------|
  | `State`            | —             | `(mode s, sharing b)`     |
  | `Settings`         | —             | `a{sv}` of the Settings   |
  | `Change`           | `mode s`      | `job s`                   |
  | `Apply`            | `nixBody s`   | `job s`                   |
  | `FactoryReset`     | —             | `job s`                   |
  | `Status`           | —             | `(state s, progress i, message s)` |

- **`losos-ctl`** (facade) — unchanged CLI surface (`state --json`,
  `change --mode …`, `status --json`, `settings --json`, `apply` (stdin),
  `factory-reset`) and unchanged JSON responses (`backend/schema.json` stays
  valid for the facade). Each subcommand opens the system bus, makes one method
  call, prints the reply as JSON, exits. `losos-ctl install` remains a **local**
  subcommand (ISO installer runs as root; no bus needed).

- The `rebuild-done` subcommand is **deleted**.

### Transport binding

Pure-Haskell `dbus` package (in nixpkgs `haskellPackages`) implements both the
daemon's server side and the facade's client side. No FFI, no `busctl` scraping.

### Authorization

A D-Bus system policy ships with the daemon package
(`services.dbus.packages = [ losos-dbus-policy ]`): group **`losos`** (fixed
gid, declared in `options.nix`/`configuration.nix`) may
`send_destination=org.nixos.losos1`; everyone else denied. The `nextcloud` user
(host or in-container — same uid/gid via plain nspawn, no `privateUsers`) joins
`losos`. The sudo rule, `security.sudo.enable`, and PHP's `sudo_path` are
**deleted**.

### Rebuild supervision (facade monitors systemd)

`spawnRebuild` (an effect on the existing `Losos` class) is reimplemented in the
daemon's IO interpreter as:

```
systemd-run --unit=losos-rebuild-<job> --collect --same-dir --wait=no \
  --property=StandardOutput=append:/var/lib/losos/rebuild.log \
  --property=StandardError=inherit \
  nixos-rebuild switch --flake /etc/nixos#install
```

The daemon **subscribes to systemd's `JobRemoved` D-Bus signal** (it is already
a bus client); when the `losos-rebuild-<job>` unit's job is removed it reads the
unit's `ExecMainStatus` (`org.freedesktop.systemd1.Unit` property or
`systemctl show`) and records done/failed in `state.json` — daemon is the sole
writer. While a rebuild is `building`, `Status` additionally reads the current
tail of `rebuild.log` so the UI shows live nixos-rebuild output lines
("facade monitors systemd out") instead of a frozen "rebuild started".

Effect-class compatibility: `cmdRebuildDone` and the `rebuild-done` CLI are
removed; `Losos` gains nothing new (the unit-watching is daemon-internal IO,
not an effect method). All existing `TestM` tests keep passing; the deleted
`rebuild-done` tests are removed and replaced by tests of the unit-completion
pure function (`exitCode -> RebuildState transition`).

### state.json

Unchanged format; still owned atomically (temp + rename) by the daemon alone.

## Part B — standalone admin endpoint

### Shape

- **`admin-ui/`** (new, in-repo): a dependency-free static SPA
  (HTML + vanilla JS + CSS, no build step). Built as flake package
  `losos-admin-ui` by copying the files into `$out`.
- **`lososd` HTTP API**: when `losos.admin.enable` (default true), lososd also
  listens on `127.0.0.1:${losos.admin.apiPort}` (default `8082`),
  serving JSON endpoints that map 1:1 onto the facade contract:

  - `GET  /api/state`, `GET /api/settings`, `GET /api/status`
  - `POST /api/change`    `{"mode":"local|mesh"}`
  - `POST /api/apply`     body = overrides.nix body (text/plain)
  - `POST /api/factory-reset`

  Auth: **Bearer token** — `losos.admin.tokenFile` (default
  `/var/secrets/losos-admin-token`, persisted via `/var`; created randomly at
  install/first boot by the daemon if absent). The SPA prompts once, stores the
  token in `sessionStorage`. Requests without it get 401.

- **Nginx vhost** on port `${losos.admin.port}` (default `8081`), opened in the
  firewall, serving the static `losos-admin-ui` and proxying `/api/` to
  `127.0.0.1:8082`. This is "its separate Nginx endpoint":
  `http://<hostName>.local:8081/`.

### Implementation of the HTTP side

`warp`/`wai` + `aeson` — both in nixpkgs. The API handlers call the same
`Losos`-class commands as the D-Bus methods; the command layer stays
transport-free. The daemon therefore owns two listeners (D-Bus + loopback
HTTP) over one command core.

### nextcloud-app retirement

The `losos` Nextcloud app is superseded: OS settings move to the new endpoint.
`nextcloud-app/`, the `losos-app` flake package, and its `extraApps` entry are
removed; the PHP test suite goes with it (its contract lives on as the facade
JSON responses + `schema.json`, which gains D-Bus/HTTP sections). The toggle
(`change`) and all settings remain available at the admin endpoint.

## Part C — rootless Podman → systemd-nspawn (NixOS Containers)

`modules/containers.nix` is rewritten; `modules/podman.nix`-era plumbing
(containers user, subuid, linger, user podman socket, `virtualisation.podman`)
is deleted.

### Nginx as the single front door

Every user-facing service answers **only** on host loopback; Nginx owns the
public ports. In the container deployment (the default) this means:

| public endpoint                    | backend                                   |
|------------------------------------|-------------------------------------------|
| `<hostName>.local:80` →            | nspawn nextcloud (`127.0.0.1:apachePort`) |
| `:8888` (Forgejo) →                | nspawn forgejo (`127.0.0.1:3000`)         |
| `:8081` (admin endpoint) →         | static SPA + `127.0.0.1:apiPort` (`/api`) |

Plus, per user request ("move the two services behind the Nginx proxy"), the
**Tahoe-LAFS web UI** loses its direct `:3456` firewall exposure: the tahoe
node binds `127.0.0.1:3456` and Nginx serves it on `:3456` (vhost with
`listen`), so the firewall opens Nginx ports only and no service binds a
non-loopback address except Nginx. The native-mode Nextcloud (`:80` vhost) and
native Forgejo (`:8888`) keep their current direct exposure only when those
legacy modes are selected; the documented/default deployment is fully
Nginx-fronted.

### `containers.nextcloud` (when `losos.nextcloud.mode == "container"`)

Runs the **native** Nextcloud stack inside the container: `services.nextcloud`
(nextcloud34), postgres (`database.createLocally`), redis
(`configureRedis`) — the same config as host-native mode. To prevent drift, the
shared settings live in a `let`-bound attrset in `services.nix`
(`nextcloudCommon`) consumed by both `services.nextcloud` (host mode) and
`containers.nextcloud.config`.

- `autoStart = true`, `privateNetwork = true` (`hostAddress = 10.231.1.1`,
  `localAddress = 10.231.1.2`).
- `bindMounts`: `/var/lib/nextcloud` (host path, persisted via impermanence) →
  same path in-container; `/run/dbus/system_bus_socket` read-write so the
  in-container admin tooling could reach the daemon (kept for future use; the
  web admin no longer lives in Nextcloud, so nothing in the container needs it
  initially — the bind mount is omitted until a consumer exists. Decision:
  **omit**).
- `forwardPorts`: container port 80 → host `127.0.0.1:${losos.aio.apachePort}`
  (option kept, see rename below) so the existing Nginx vhost
  (`mattbox.local` → loopback apachePort) is untouched.
- GPU (`losos.gpu.enable`): `bindMounts` `/dev/dri` +
  `systemd.services."container@nextcloud".serviceConfig.DeviceAllow =
  [ "char-drm rw" ] DevicePolicy = "auto"` — replaces AIO's
  `--device/--group-add` propagation uncertainty.

### `containers.forgejo` (when `losos.forgejo.mode == "container"`)

Runs `services.forgejo` inside a second container (own subnet octet
`10.231.2.0/24`), `forgejo-data` becomes a plain persisted host directory
`/var/lib/forgejo` bind-mounted in, forward port 3000 → host
`127.0.0.1:3000`; the existing Nginx vhost on :8888 is untouched.

### Mode rename

`losos.nextcloud.mode` enum `"native"|"aio"` → `"native"|"container"`
(symmetry with forgejo; "aio" no longer means anything). Ripple: `options.nix`,
`defaultSettings`/`parseSettings` in `Lib.hs` (rename `setNextcloudMode`
default + accept `"container"`), `defaultOverridesNix`, `schema.json`
`settingsResponse` enum, committed `modules/overrides.nix`. The settings JSON
key `nextcloudMode` keeps its name (wire stability); only its enum changes.
`losos.aio.*` options are renamed to `losos.containerPorts.apache` /
`losos.containerPorts.interface` is dropped (AIO interface is gone — first-run
Nextcloud setup is the standard web installer on the container). Specifically:
drop `aio.interfacePort`, `aio.datadir`; rename `aio.apachePort` →
`nextcloud.containerPort`? No — keep it simple: **`losos.nextcloud.apachePort`**
(host loopback port Nginx proxies to in container mode). Schema/Settings keys
`aioApachePort` → `apachePort`, `aioInterfacePort` dropped (breaking change to
`settingsResponse`, acceptable now that the PHP app is retired and the SPA is
new).

### What disappears

docker.io/codeberg.org runtime pulls (closure is fully flake-pinned), the
`containers` user + uid option, linger tmpfiles, user-level podman units,
the `losos.gpu` group plumbing for podman (kept for /dev/dri host perms),
`losos.containers.*` options.

## Option map (losos.*)

| option                          | change                                  |
|---------------------------------|------------------------------------------|
| `nextcloud.mode`                | enum → `"native" | "container"`          |
| `aio.apachePort`                | renamed → `nextcloud.apachePort`         |
| `aio.interfacePort`, `aio.datadir`, `containers.user`, `containers.uid` | deleted |
| `admin.enable` (true), `admin.port` (8081), `admin.apiPort` (8082), `admin.tokenFile` | new |
| `backend.package`               | now the daemon+facade package; semantics: daemon enabled system-wide |
| `backend.dbussGroup`/`losos` gid | new: fixed gid for the `losos` group     |
| `gpu.enable`                    | kept; semantics now nspawn device passthrough |

## Testing

- Haskell: existing `TestM` suite keeps passing; deleted `rebuild-done` cases
  removed; new pure unit for unit-exit → RebuildState mapping; D-Bus
  marshalling round-trip tests using the `dbus` client's in-process loopback
  is not feasible — instead validate against `schema.json` examples and add a
  NixOS VM test.
- NixOS VM test (new, `tests/admin-vm.nix`, run via `nix build .#checks…` or
  `nixosTest`): boots the install config with the daemon, asserts:
  `losos-ctl state --json` as root returns the settings JSON; the admin
  endpoint answers 401 without a token and 200 with it; `containers.nextcloud`
  reaches `systemctl is-active container@nextcloud`.
- No PHP suite anymore.

## Migration / rollout

Single rebuild: new modules gate on the new defaults
(`nextcloud.mode = "container"` is the new default, matching the old `"aio"`
deployment intent). Existing boxes with an `overrides.nix` containing
`"aio"` are handled by `parseSettings` accepting `"aio"` as a legacy alias
mapping to `"container"` (read-side only; writes always emit `"container"`).
CLAUDE.md sections on the sudoers bridge / podman overlay are rewritten in the
same change.
