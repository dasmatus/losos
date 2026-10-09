**English** · [Slovenčina](Edge-Federation-sk) · [Deutsch](Edge-Federation-de)

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

![Site A: a box dials spoke A over rathole and announces itself; spoke A dials the hub on the internet over a rathole uplink and relays the box with POST /relay; site B mirrors it. Each spoke runs Traefik on the LAN; the hub runs Traefik with public TLS, routing Host(<box>.<zone>).](images/edge-federation-en.svg)

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
  lan.advertise = true;                     # boxes find it by DNS-SD
  lan.openEnrolment = true;                 # boxes on the LAN enrol themselves
  uplink = {
    enable = true;
    registrarUrl = "https://register.losos.cfd";   # the hub's API
    ratholeEndpoint = "edge.losos.cfd:2333";       # the hub's tunnel port
    id = "acme";                                   # the tenant row above
    tokenFile = "/var/secrets/losos-uplink-token";
    # bootstrapTokenFile is optional: every relayed service carries the
    # uplink token, so rathole's default_token is never consulted.
  };
};
```

The registrar's reconciler, after it has written its own Traefik and rathole
files, relays its **live** tenants (registered, whitelisted or enrolled,
heartbeating) to the hub and renders `/etc/rathole/uplink.toml`: a rathole
*client* config with one service per relayed box, `local_addr =
127.0.0.1:<that box's port on this spoke>`, token = the uplink token. A
second rathole unit, `losos-rathole-uplink`, runs it (started by a path unit
the moment the file exists); rathole hot-reloads the file as it does the
server's. The hub's Noise public key is pinned on first contact (`GET
/noise-public-key`, written to `uplink.noisePublicKeyFile` by the registrar
itself), as a box pins its edge's. So a request for
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
must never set it), and the advert says so (`enrol=open`). `losos-registrar
enrol list|forget --dir /var/lib/losos-registrar/enrolled` (on the gateway,
`losos-edge boxes` and `losos-edge forget <id>`) shows and drops enrolled
boxes; a box that is still heartbeating re-enrols with the token it holds,
so forgetting is for a box that left. The hub keeps closed enrolment.

## The box: which edge, and when none

`lososd` already finds every edge in reach (LAN by DNS-SD, the configured
official one by URL) and refuses to turn sharing on when there is none. It
picks one path by this rule:

1. **a local edge first** — the first LAN advert whose `/health` answers; the
   advert now also carries `rathole=<host:port>` so the box knows where its
   tunnel goes;
2. **the official edge second** — `losos.proxy.registrarUrl` and
   `losos.proxy.edgeRatholeEndpoint`, when the LAN has none;
3. **none** — every edge-dependent feature is off: the tunnel and announce
   units are stopped, the sharing gate refuses, the market relay refuses.

`GET /api/edge` carries the choice as `path` (`{name, url, rathole,
source}` or `null`), beside every edge's `rathole` endpoint. With
`losos.proxy.enable` on, `lososd` writes the path to
`/run/losos/edge-path.env` (`LOSOS_EDGE_PATH_URL`, `_RATHOLE`, `_SOURCE`,
`_NOISE_PUB`) and drives the two tunnel units from it: a change of path
restarts `losos-rathole-client` and `losos-registrar-announce`, no path
writes `/run/losos/edge-none` and stops them (both units carry
`ConditionPathExists=!/run/losos/edge-none`, so nothing restarts them until
an edge is back). Both files are on `/run`: until the first scan after a
boot the units dial the configured edge, as they always did. Each edge's
Noise key is pinned in its own file — the configured edge's at
`losos.proxy.noisePublicKeyFile`, a LAN edge's under
`/var/secrets/losos-edge-pins/<host>_<port>.pub` — by the client's own
`ExecStartPre`, so a second gateway never inherits the first one's key.
The box's token is the same on every road — the hub has it in its
whitelist, a gateway learns it on first contact, and `lososd` mints it (and
the bootstrap token) on first start when the files are missing — and its
public name is the same too: the owner's `losos.proxy.hostname` must be one
label under the site's zone (`mattbox.acme.losos.cfd`) for the relay to
accept it, which is also the name Nextcloud trusts. A box that falls back to
the official edge keeps the same name through its direct tenant row. The
market relay needs no extra switch: it is gated on an *official* edge
answering, and with none in reach it already says `available: false`.

`tests/edge-federation.nix` runs the three machines (hub, gateway, box) and
walks exactly this: LAN path, enrolment, uplink, fall back, off, back.

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

## Custom domains behind a local edge

[Custom domains](Master-Proxy#custom-domains) work for a box behind a local
edge too, under one rule. Only the official edge routes a domain. The local
edge never serves a zone, never checks a record and never
asks Let's Encrypt for anything. It carries the official edge's traffic to
the box on the same relayed service that already carries the box's own
hostname.

The hard part is trust. A local edge is somebody's machine, and its `/relay`
list says "my box `mattbox`". If the official edge believed that and routed
`mattbox`'s domains there, anyone running a gateway could name their box
after someone else's, take their domain and get a valid certificate for it.
So the box has to say which local edge it is behind, and the local edge
cannot say it for the box.

The box does that with a **relay pass**:

1. The box asks the official edge for its domains view, as it already does
   every two minutes, over its own internet and with its own token. The
   answer now carries `relay_pass`, `v1.<hour>.<64 hex>`. The hex is an
   HMAC-SHA256 over the box id and the hour, under
   `/var/lib/losos-registrar/relay-pass.key`. That key never leaves the
   official edge, and the registrar makes it on first start.
2. lososd writes the pass to `/run/losos/relay-pass` (root only) and drops
   it from what the admin page gets.
3. `losos-registrar announce` reads the file on every register and
   heartbeat and sends the pass to whichever edge the box uses. A local
   edge keeps it in memory, only if it has the right shape.
4. The local edge's uplink sends each box's pass with the box in
   `POST /relay`. The official edge checks it. A good pass binds the box to
   that local edge.

A pass is good for the hour it was issued in and the two after. A local
edge the box has left can replay what it last saw for at most that long,
and never over a newer pass from another local edge, because a binding only
moves to a pass at least as new. The local edge can relay its own boxes
without one. They just get no domains.

On top of the binding, the box has to be **in the mesh**. It must have
joined this official edge's cluster with `/cluster/join`, which records its
compute window under its own id. A box with only a tunnel gets its hostname
and nothing more. And if the box is also registered with the official edge
directly, the direct path wins and the local edge is not used.

### The route table

The registrar keeps the table itself, the same way it keeps its registry.
There is no database to run. Each reconciler pass works out which routes
should exist and builds the Traefik routers from that. When the table
changes, the registrar rewrites `/var/lib/losos-registrar/relay-routes.json`
beside `registry.json` and logs each route it adds or removes:

```json
{
  "bindings": {
    "mattbox": { "spoke": "acme", "epoch": 493281 }
  },
  "routes": [
    { "domain": "cloud.example.org", "tenant": "mattbox", "spoke": "acme", "service": "acme.mattbox" }
  ]
}
```

`bindings` records which local edge each box vouched for, and with a pass
from which hour. That part is what a restart reads back, so the routes come
back on the registrar's first pass instead of waiting for every local edge
to call `/relay` again. A binding whose pass has expired is ignored. `routes`
is the table those bindings produced, written for you to read. The registrar
plans it again from the bindings and never loads it. The box's name in the
edge's zone, `<label>.<zone>`, goes the same way as its domains, so the
CNAME target reaches it too.

The local edge gets its own rows back in the `/relay` answer. The gateway
writes them to `/var/lib/losos-registrar/hub-routes.json` and logs each
change. That file only tells you what the official edge routes your way.
Nothing on the local edge routes by it.

### Turning it on

On the official edge:

```nix
losos.edge.dns = {
  enable = true;                 # the zone and custom domains
  relayRoutes.enable = true;     # and the route table
};
```

Nothing else runs for it. The table file and the pass key both live in the
registrar's state directory. Nothing changes on the local edge or the box.
Both pick the pass up from a build with this change.

`backend-registrar/tests/domains.rs` runs the whole path against real
registrars. A box behind a gateway gets its domain, a box outside the mesh
gets nothing, and three forged passes get nothing. `tests/edge-dns.nix`
boots the NixOS wiring. It checks the key file, `/relay` with a pass, the
binding landing in `relay-routes.json`, and the binding still being there
after the registrar restarts.

## Delivery: the gateway, without an ISO

Two forms, one configuration:

1. **A VM image**, `nix build .#losos-disk-edge-qcow2` (`nix run
   .#losos-disk-edge-qcow2-run` boots it in QEMU), attached to every tagged
   release as `losos-edge-gateway-<tag>.qcow2`. It is
   `nixosConfigurations.edge-gateway`: the edge module with
   `losos.edge.gateway.enable` (`modules/edge-gateway.nix`), which sets
   `lan.advertise` and `lan.openEnrolment`, reads the uplink at runtime from
   `/var/lib/losos-edge/uplink.json` so one image serves every site, mints
   its own bootstrap token on first boot, gives root a console (first
   password `losos`, change forced at first login; sshd installed but
   stopped until `losos-edge ssh on`), and prints its address on tty1. Boot
   it on Proxmox, libvirt or VirtualBox with a NIC on the LAN; boxes
   installed from the stock ISO find it within a minute. Then, on its
   console:

   ```sh
   losos-edge status
   losos-edge uplink set --id acme --token-file /root/acme.token \
     --registrar https://register.losos.cfd --rathole edge.losos.cfd:2333
   losos-edge boxes
   ```

   with the tenant row the hub operator gave you. `losos-edge uplink
   clear` stops relaying.
2. **An existing NixOS machine**: import `nixosModules.edge` and either set
   `losos.edge.gateway.enable = true` (the image's shape) or write the
   options above by hand, as the two-VM demo and `tests/edge-lan.nix` do.

The hub side for the official edges is one tenant row per site. The Vercel
host carries the same `/relay` route and so can *accept* a spoke for a demo,
but with no rathole on it the data path, as before, needs the VPS.
