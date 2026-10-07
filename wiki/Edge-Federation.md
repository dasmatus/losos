# Edge federation

Anyone may run an edge beside their boxes ([Master proxy — Edge on the same
LAN](Master-Proxy#edge-on-the-same-lan)). This page is how such a **local
edge** plugs into the **official edges** so that a box behind it is reachable
from anywhere, how a box chooses which edge to use, and how a local edge is
delivered without a second installer ISO.

## The shape

Hub and spoke. An official edge is a **hub**. A user-hosted edge is a
**spoke**: it keeps everything it has today (the registrar, the tunnel
endpoint, the LAN advert, optionally the mesh control plane) and adds one
**uplink** to a hub. Two sites, each behind its own spoke, reach each other
through the hub; a box never talks to another site's spoke directly.

```
site A                         internet                        site B
box ──rathole──▶ spoke A ──rathole uplink──▶ hub ◀──rathole uplink── spoke B ◀──rathole── box
     announce        │        POST /relay       │       POST /relay        │     announce
                     ▼                          ▼                          ▼
              Traefik (LAN)       Traefik (public TLS, Host(<box>.<zone>))   Traefik (LAN)
```

Three planes, and only two of them federate:

| Plane   | What                                             | Federated?                                                                 |
| ------- | ------------------------------------------------ | -------------------------------------------------------------------------- |
| control | registrar API: register, heartbeat, join, market | the spoke relays its boxes to the hub (`POST /relay`); the market does not |
| data    | rathole tunnels + Traefik                        | one more hop: hub → spoke → box                                            |
| mesh    | rke2 server + Longhorn                           | **no**: each edge is its own cluster, see "What stays local"               |

## Hub side: a tenant that may relay

A spoke is an ordinary tenant of the hub with one extra attribute, the zone
it may relay under:

```nix
# on the hub (an official edge)
losos.edge.tenants.acme = {
  hostname = "acme.losos.cfd";           # the spoke's own name, as any tenant
  tokenFile = "/var/secrets/tenant-acme";
  relayZone = "acme.losos.cfd";          # it may relay <label>.acme.losos.cfd
};
```

The spoke sends `POST /relay {appliance_id: "acme", token, tenants: [{id,
hostname}, …]}` on a cadence, authenticated exactly like `/register` (same
token, same constant-time check, same body cap). The hub accepts a relayed
box only if its id is a DNS label and its hostname is **exactly one label
under the zone** (`mattbox.acme.losos.cfd`), and keeps the relayed set equal
to the last list it was sent: a box the spoke stops listing is gone on the
next reconcile, and a spoke that goes silent takes all of its boxes with it
after the heartbeat TTL, like any tenant. Relayed entries live in the hub's
registry as `<spoke>.<box>` with `via: <spoke>`, get a port from the same
rathole range, a Traefik router `Host(<hostname>)` and a rathole service
whose token is the **spoke's** token: the spoke is the authenticated party,
the hub never holds a box's token. Hostname authority stays where it was: the
hub operator chose the zone; the spoke chooses labels inside it; anything
else is refused per box and the spoke logs it.

Why not let a spoke's boxes register with the hub directly? Because then the
hub operator lists every box in the world, which is the thing a local
gateway exists to avoid. One tenant row per site is the whole cost of a
site on the official edge.

## Spoke side: the uplink

```nix
# on the local edge
losos.edge = {
  enable = true;
  lan.advertise = true;                     # boxes find it (PR #75)
  lan.openEnrolment = true;                 # boxes on the LAN enrol themselves
  uplink = {
    enable = true;
    registrarUrl = "https://register.losos.cfd";   # the hub's API
    ratholeEndpoint = "edge.losos.cfd:2333";       # the hub's tunnel port
    id = "acme";                                   # the tenant row above
    tokenFile = "/var/secrets/losos-uplink-token";
    bootstrapTokenFile = "/var/secrets/losos-uplink-bootstrap";
  };
};
```

The registrar's reconciler, after it has written its own Traefik and rathole
files, relays its **live** tenants (registered, whitelisted or enrolled,
heartbeating) to the hub and renders `/etc/rathole/uplink.toml`: a rathole
*client* config with one service per relayed box, `local_addr =
127.0.0.1:<that box's port on this spoke>`, token = the uplink token. A
second rathole unit, `losos-rathole-uplink`, runs it; rathole hot-reloads the
file as it does the server's. The hub's Noise public key is pinned on first
contact (`GET /noise-public-key`), as a box pins its edge's. So a request for
`https://mattbox.acme.losos.cfd/` terminates TLS on the hub, enters the
hub's rathole service `acme.mattbox`, crosses the uplink to the spoke's local
port for `mattbox`, crosses the box's tunnel, and lands on the box's Nginx.
Two tunnels, one Host header, no change on the box.

**Open enrolment** is what makes a stock box usable with a gateway nobody
provisioned tokens for: with `lan.openEnrolment`, an unknown appliance id
that `/register`s is enrolled trust-on-first-use — its token and hostname
are kept under `/var/lib/losos-registrar/enrolled/`, and every later request
for that id must match. It grants proxy membership only (never `cluster` or
`market`), it is refused on an edge that does not advertise on a LAN (a VPS
must never set it), and a squatted id is the LAN owner's problem to forget
(`losos-edge forget <id>`). The hub keeps closed enrolment.

## The box: which edge, and when none

`lososd` already finds every edge in reach (LAN by DNS-SD, the configured
official one by URL) and refuses to turn sharing on when there is none. The
path rule, from Matus (2026-10-07):

1. **a local edge first** — the first LAN advert whose `/health` answers; the
   advert now also carries `rathole=<host:port>` so the box knows where its
   tunnel goes;
2. **the official edge second** — `losos.proxy.registrarUrl` and
   `losos.proxy.edgeRatholeEndpoint`, when the LAN has none;
3. **none** — every edge-dependent feature is off: the tunnel and announce
   units are stopped, the sharing gate refuses, the market relay refuses.

`GET /api/edge` carries the choice as `path` (`{name, url, rathole,
source}` or `null`). `lososd` writes it to `/run/losos/edge-path.env` and
drives the two tunnel units from it: a change of path restarts them, a path
of none stops them; with the proxy switched off it only records. The box's
token is the same on every road — the hub has it in its whitelist, a gateway
learns it on first contact — and its public name is the same too: the
owner's `losos.proxy.hostname` must be one label under the site's zone
(`mattbox.acme.losos.cfd`) for the relay to accept it, which is also the
name Nextcloud trusts. A box that falls back to the official edge keeps the
same name through its direct tenant row.

## What stays local

- **The mesh.** Longhorn replication and mesh compute run inside one rke2
  cluster, and that cluster is the edge the box joined. A spoke with
  `losos.edge.cluster.enable` pools its own site's boxes; the hub does not
  bridge two sites' clusters, and nothing here forwards 9345/6443 over the
  uplink. Cross-site pooling is a follow-up with its own design.
- **The market.** Trading is refused by the box unless an *official* edge is
  in reach ([Official edges](Master-Proxy#official-edges)); a spoke is never
  official, and a relayed box is not a hub tenant, so no `/market/*` call
  can be made on its behalf. The relay carries HTTP reachability, nothing
  else.
- **The identity.** A spoke does not forward `/identity`; a box that wants
  the market reaches the official edge itself, over its own internet.

## Delivery: the gateway, without an ISO

Two forms, one configuration:

1. **A VM image**, `nix build .#losos-edge-qcow2`, attached to every tagged
   release. It is the edge module as `nixosConfigurations.edge-gateway`:
   `lan.advertise` and `lan.openEnrolment` on, the uplink read at runtime
   from `/var/lib/losos-edge/uplink.json` so one image serves every site,
   a root console (password `losos`, change forced at first login, SSH off
   until `losos-edge ssh on`), and a tty1 banner with its address. Boot it
   on Proxmox, libvirt or VirtualBox on the LAN; boxes installed from the
   stock ISO find it within a minute. Then `losos-edge uplink set --id acme
   --token-file …` with the tenant row the hub operator gave you.
2. **An existing NixOS machine**: import `nixosModules.edge` and write the
   options above, as the two-VM demo and `tests/edge-lan.nix` do.

The hub side for the official edges is one tenant row per site. The Vercel
host carries the same `/relay` route and so can *accept* a spoke for a demo,
but with no rathole on it the data path, as before, needs the VPS.
