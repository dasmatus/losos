# Master proxy: replace Cloudflare reliance with Traefik + Rathole + a Rust registrar

Date: 2026-08-17
Status: Design (pending implementation)

## Goal

Replace losos's reliance on Cloudflare Tunnel (`losos.cfd.*`) with a self-hosted
edge proxy so the behind-NAT appliance is reachable on the public internet
without opening any inbound port on the box and without a third-party tunnel
provider.

Three components:
1. **Traefik** — the master (public edge) proxy, TLS termination.
2. **Rathole** — a Rust reverse tunnel; the appliance dials out to the edge
   (no inbound public port), multiplexing reverse traffic over one TCP
   connection per appliance.
3. **A stub Rust HTTP server** (`losos-registrar`) — a registration +
   reconciliation daemon that *constantly updates* Traefik (and rathole)
   dynamic config from a tenant registry.

The on-box Nginx stays as the **slave** proxy (existing `default_server` front
door on `:80`); Traefik on the edge is the **master**.

## Decisions (approved)

- **Topology**: a remote VPS runs the edge. A **new NixOS system**
  `nixosConfigurations.edge` is added to this flake. The appliance runs only
  a rathole client + a registration client. (Edge: master. Appliance: slave
  Nginx + outbound clients.)
- **Registration scope**: **multi-appliance**. The registrar holds a tenant
  registry keyed by appliance id; appliances heartbeat; stale routes are
  pruned. The reconciler continuously re-derives config from the registry.
- **TLS**: **on-demand Let's Encrypt** via per-router `tls.certResolver: le`
  + `tls.domains: [<hostname>]` written by the registrar. The registrar is
  the gatekeeper: it only ever writes routers for authenticated appliances,
  so only those hostnames ever get certs.
- **Wiring**: **Approach A** — the Rust reconciler writes BOTH Traefik
  dynamic config and rathole `server.toml`, and SIGHUPs rathole on change
  (rathole supports SIGHUP hot-reload). One L4 tunnel per appliance (rathole
  is TCP-only; it cannot demux by HTTP Host, so per-appliance tunnels are
  unavoidable).

## Architecture

### Two NixOS systems in the flake

`flake.nix` gains `nixosConfigurations.edge` alongside `install`. The two
share only `modules/options.nix` (the `losos.*` namespace). The edge imports
a **disjoint** module set — no disko, no impermanence, no containers, no
Nextcloud/Tahoe:

- `install` (appliance, existing) — adds `losos.proxy.*` options, a rathole
  client service, a `losos-registrar announce` service. Nginx front door
  unchanged.
- `edge` (new) — imports `options.nix` + new `modules/edge-*.nix` modules:
  Traefik, rathole server, `losos-registrar serve`. A normal stateful VPS
  (not tmpfs-root); `/var/lib/losos-registrar` and `/var/lib/traefik` live on
  durable disk.

### Components

| System | Component | Role |
|---|---|---|
| edge | Traefik (master) | Public `:443` TLS, `:80`→HTTPS. File provider watches `/etc/traefik/dynamic/`. One **static** router `register.<domain>` → loopback registrar; all appliance routers are **dynamic**, written by the registrar. |
| edge | rathole server | Listens `:2333` for appliance clients. Hot-reloads `/etc/rathole/server.toml` on SIGHUP. One `[server.services.<id>]` per live appliance bound to a loopback port. |
| edge | `losos-registrar serve` | Loopback HTTP API + reconciler thread. Sole writer of `/etc/traefik/dynamic/losos.yml` and `/etc/rathole/server.toml`. Owns `/var/lib/losos-registrar/registry.json`. |
| appliance | Nginx (slave) | Existing `default_server` vhost on `:80`. Path routing unchanged. Sees the public `Host` (rathole is transparent TCP). |
| appliance | rathole client | Dials `edge:2333`; one service `<appliance_id>` → `127.0.0.1:80` (Nginx). No inbound port. |
| appliance | `losos-registrar announce` | Reads id/token/hostname; POSTs `/register` on boot, `/heartbeat` every N s. Same Rust binary, `announce` subcommand. |

### Data flow

**Public request** (browser → appliance):
1. `https://mattbox.losos.cfd/nextcloud` → edge Traefik `:443`.
2. Traefik terminates TLS (cert obtained lazily via the router's
   `certResolver` on first hit).
3. Router `Host(\`mattbox.losos.cfd\`)` → service →
   `http://127.0.0.1:<rathole_port>`.
4. Rathole server forwards over the tunnel to the appliance rathole client,
   which dials `127.0.0.1:80` (Nginx slave).
5. Nginx does the existing path routing → Nextcloud/Forgejo/Tahoe/admin.

**Registration** (appliance → edge):
1. Appliance boot: `announce` reads id+token+hostname, POSTs `/register` to
   `https://register.losos.cfd/register` (a **static** Traefik router →
   loopback registrar — cert exists before any appliance registers; avoids
   the register/route chicken-and-egg).
2. Registrar authenticates the per-appliance token against
   `losos.edge.tenants.<id>` (closed enrollment — only pre-provisioned ids
   may register), allocates a rathole port from the configured range,
   records `{id, hostname, port, last_seen}` in `registry.json`.
3. Reconciler regenerates `losos.yml` + `server.toml`, SIGHUPs rathole
   (only if content changed), lets Traefik auto-reload from the file dir.
4. Appliance rathole client (started in parallel, retrying forever) dials
   `edge:2333`, presents its token, rathole matches the just-provisioned
   service. Tunnel up.
5. Appliance `/heartbeat` every N s refreshes `last_seen`. Stale tenants
   (TTL exceeded) pruned on the next reconciler tick: router + service
   removed, rathole SIGHUPed, Traefik rewritten.

**Bootstrap ordering**: the registration API is fronted by Traefik at the
*static* hostname `register.<domain>`, so it has a cert before any appliance
has registered. Traefik's static config points `register.*` at
`127.0.0.1:<registrarApiPort>`; only the *dynamic* file-provider config is
what the registrar rewrites. Static and dynamic config are separate files
Traefik merges — the registrar never touches the file that feeds it.

## Option namespace

All under `options.losos` (the sanctioned namespace — modules never read bare
`config.X`).

### Appliance side — `losos.proxy.*`

| Option | Type | Default | Notes |
|---|---|---|---|
| `proxy.enable` | bool | `false` | Master switch on the appliance. |
| `proxy.edgeRatholeEndpoint` | str | `edge.losos.cfd:2333` | Where the rathole client dials. |
| `proxy.registrarUrl` | str | `https://register.losos.cfd` | Where `announce` POSTs. |
| `proxy.hostname` | str | `"<hostName>.losos.cfd"` | Public hostname registered. |
| `proxy.applianceId` | str | derived from hostname | Stable id; key into the registry. |
| `proxy.tokenFile` | path | `/var/secrets/losos-proxy-token` | Per-appliance secret, 0600, persisted via `/var`. Also the rathole service token. |
| `proxy.heartbeatInterval` | str | `30s` | `announce` heartbeat cadence. |
| `proxy.rathole.package` | package | `pkgs.rathole` | |
| `proxy.registrar.package` | package | `self.packages.${system}.losos-registrar` | |

### Edge side — `losos.edge.*`

| Option | Type | Default | Notes |
|---|---|---|---|
| `edge.enable` | bool | `false` | |
| `edge.publicDomain` | str | `losos.cfd` | Used for the static `register.<domain>` router + SNI suffix. |
| `edge.ratholeBindPort` | port | `2333` | Where appliance clients dial. |
| `edge.ratholePortRange` | str | `"50000-50100"` | Allocated per appliance. |
| `edge.heartbeatTtl` | str | `120s` | Tenants older than this are pruned. |
| `edge.reconcileInterval` | str | `15s` | Reconciler tick. |
| `edge.registrarApiPort` | port | `8443` | Loopback HTTP API. |
| `edge.acmeEmail` | str | (required) | Let's Encrypt account. |
| `edge.bootstrapTokenFile` | path | (required) | rathole `default_token`, 0600. |
| `edge.tenants.<id>.hostname` | str | (required) | Closed-enrollment whitelist. |
| `edge.tenants.<id>.tokenFile` | path | (required) | Matching appliance token, 0600. |

### Retiring `losos.cfd.*`

Remove `services.cloudflared` (containers.nix:44) and the `cfd.enable` /
`cfd.tunnels` options (options.nix:26, 225). Replaced by `proxy.*` / `edge.*`.
Nothing breaks: `cfd` was a never-wired stub (`tunnels = {}`) and off by
default. The Nginx `default_server` + `trusted_domains` assumption (a public
hostname arrives) is preserved — Traefik+rathole deliver it transparently.

## Secrets & persistence

- **Secrets never in the nix store.** `tokenFile`/`bootstrapTokenFile`/
  `tenants.<id>.tokenFile` are runtime paths (agenix or manually-placed 0600
  files under `/var/secrets/`). The flake stores only id→hostname+tokenFile
  *paths*. Mirrors the `cfd.tunnels.<name>.credentialsFile` convention.
- **Appliance persistence**: token file under `/var` (already persisted).
- **Edge persistence** (stateful VPS, not tmpfs): `/var/lib/losos-registrar`
  (registry.json) and `/var/lib/traefik` (acme.json) on durable disk.
- **Transport security**: rathole Noise protocol between client and server
  (built-in) so the VPS operator can't read appliance traffic — replaces
  Cloudflare Tunnel's origin encryption.

## Rust registrar (`backend-registrar/`)

One cargo workspace, one binary `losos-registrar` with two subcommands, built
via `rustPlatform.buildRustPackage` in `flake/packages.nix` (mirrors how
`losos-ctl` is built).

### `losos-registrar serve` (edge)
- hyper-based loopback HTTP server: `/register`, `/heartbeat`, `/deregister`,
  `/health`.
- A reconciler thread (ticks every `reconcileInterval`, also triggered on
  write) that regenerates `/etc/traefik/dynamic/losos.yml` and
  `/etc/rathole/server.toml` from the registry.
- **Pure core**: `desired_config(registry) -> {traefik_yaml, rathole_toml}`
  is a pure function, unit-tested with no filesystem (mirrors the repo's
  `Losos` effect class / `TestM` split).
- Atomic temp+rename for both files; SIGHUP rathole **only when content
  changed** (byte-compare) to avoid needless reloads.
- Sole writer of both config files.
- **Re-attach on startup**: reads `registry.json` and regenerates both files
  *before* opening the API — existing tenants' routes are restored even
  before they re-heartbeat. Mirrors lososd's rebuild re-attach.

### `losos-registrar announce` (appliance)
- Reads the token file, POSTs `/register`, loops `/heartbeat` with backoff.
- Stateless; re-registers on every start.

### Config the reconciler writes

`/etc/traefik/dynamic/losos.yml` (per live tenant):
```yaml
http:
  routers:
    <id>:
      rule: "Host(`mattbox.losos.cfd`)"
      service: <id>
      entryPoints: [websecure]
      tls:
        certResolver: le
        domains:
          - main: "mattbox.losos.cfd"
  services:
    <id>:
      loadBalancer:
        servers:
          - url: "http://127.0.0.1:<rathole_port>"
```

`/etc/rathole/server.toml` (per live tenant):
```toml
[server]
bind = ["0.0.0.0:2333"]
default_token = "<bootstrap>"

[server.services.<id>]
token = "<per-appliance token>"
bind = ["127.0.0.1:<rathole_port>"]
```

Appliance `/etc/rathole/client.toml` (generated from `losos.proxy.*`):
```toml
[client]
remote = ["edge.losos.cfd:2333"]
default_token = "<bootstrap>"

[client.services.<id>]
token = "<per-appliance token>"
local = ["127.0.0.1:80"]   # the slave Nginx
```

## Error handling

- **Appliance**: registration failures retry with backoff; rathole client
  retries dial forever (native); heartbeat failures keep retrying. Edge
  pruning only removes after TTL, so brief blips don't kill a route.
- **Edge**: failed config writes keep the old config, log, self-correct on
  the next tick. Reconciler is idempotent.
- **Edge restart**: registrar re-derives config from `registry.json` on
  startup (see above).
- **Appliance reboot**: `announce` re-registers; rathole reconnects.

## Gotchas

- **rathole SIGHUP must not drop active tunnels.** Advertised as a rathole
  feature; **verify at impl time**. If it does drop, fall back to Approach B
  (static pre-provisioned port pool) — the pure `desired_config` core is
  identical, only the writer differs.
- **Don't import appliance modules into the edge system.** `edge` gets only
  `options.nix` + `modules/edge*.nix`. No disko/impermanence/containers.
- **Static vs dynamic Traefik config are separate files.** The registrar
  rewrites only the dynamic file; the static `register.<domain>` router that
  feeds the registrar lives in the Traefik static config the flake declares.
- **Nginx `default_server` + `trusted_domains`** still expect a public
  hostname. Traefik forwards with the original Host; rathole is transparent
  TCP; Nginx sees the public hostname. No Nginx change needed.

## Testing

- **Rust unit tests** (`#[test]`): `desired_config` for add/heartbeat/prune;
  idempotent writes; re-attach from `registry.json`; port allocation within
  range + reuse.
- **NixOS VM test** (`tests/edge-vm.nix`, mirrors `tests/admin-vm.nix`):
  edge VM + appliance VM on a test net; appliance registers;
  `curl https://mattbox.losos.cfd/` through Traefik+rathole reaches the
  appliance Nginx; prune after TTL removes the route.
- **Build checks**: `nix build .#losos-registrar` and
  `.#nixosConfigurations.edge.config.system.build.toplevel`.

## Out of scope

- DNS for `*.losos.cfd` — assumed pointed at the edge VPS externally.
- Provisioning the edge VPS itself (the flake produces a config you `nixos-rebuild` onto a VPS; cloud-init / provider provisioning is out of scope).
- Multi-tenant *auth* beyond the closed-enrollment token whitelist (no
  per-tenant isolation beyond separate rathole tunnels; all appliances share
  one `losos.cfd` apex).