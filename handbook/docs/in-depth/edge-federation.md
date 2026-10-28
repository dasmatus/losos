---
title: Edge federation
sidebar_position: 6
mdx:
  format: md
---

# Edge federation

Anyone may run an edge beside their boxes ([Master proxy, Edge on the same
LAN](master-proxy.md#edge-on-the-same-lan)). This page is how such a **local
edge** plugs into the **official edges** so that a box behind it is reachable
from anywhere, how a box chooses which edge to use, and how a local edge is
delivered without a second installer ISO.

## The shape

Hub and spoke. An official edge is a **hub**. A user-hosted edge is a
**spoke**. It keeps everything it already has (the registrar, the tunnel
endpoint, the LAN advert, optionally the mesh control plane) and adds one
**uplink** to a hub. Two sites, each behind its own spoke, reach each other
through the hub; a box never talks to another site's spoke directly.

![Site A: a box dials spoke A over QUIC and announces itself; spoke A dials the hub on the internet over a QUIC uplink and relays the box with POST /relay; site B mirrors it. Each spoke runs Traefik on the LAN; the hub runs Traefik with public TLS, routing Host(<box>.<zone>).](/img/diagrams/edge-federation-en.svg)

Three planes, and only two of them federate:

| Plane   | What                                             | Federated?                                                                 |
| ------- | ------------------------------------------------ | -------------------------------------------------------------------------- |
| control | registrar API: register, heartbeat, join, market | the spoke relays its boxes to the hub (`POST /relay`); the market does not |
| data    | QUIC tunnels + Traefik                           | one more hop: hub → spoke → box                                            |
| mesh    | rke2 server + Longhorn                           | **no**: each edge is its own cluster, see "What stays local"               |

## Three tiers

Every link an edge or a box holds has a tier, and the box, the gateway and
the hub all report it:

| Tier      | Link                                                   | Cluster                     |
| --------- | ------------------------------------------------------ | --------------------------- |
| primary   | edge to edge: a gateway's uplink to an official edge   | the **main cluster**        |
| secondary | box to a local gateway of its own cluster              | a **secondary cluster**     |
| tertiary  | box straight to an official edge                       | none: the fallback          |

A box never holds a primary link itself. It sees its own link as
`path.tier` in `GET /api/edge` and on the Mesh page, the gateway shows its
hub and clusters on its manage page, and the hub keeps the gateways of the
main cluster, every one on a primary link, in `main-cluster.json` beside its
registry (a gateway's manage page lists its own site's under **Main
cluster**).

### Secondary clusters

One gateway on a network can fail. A **secondary cluster** is one or more
gateways on the same network that serve the same boxes, so a box keeps its
reach when one of them goes away. It is runtime state, made on the gateways'
manage pages: **Start a cluster** on the first gateway, which then shows the
cluster's name, its secret and its address; **Join a cluster** on each other
gateway, with those three. The page calls `PUT /manage/api/gateway/cluster`
(a name alone starts a cluster; a name, the secret and another member's
address join it) and **Leave** calls `DELETE` on the same path.

The cluster file (`losos.edge.secondary.file`, by default
`/var/lib/losos-edge/cluster.json`, mode 0600) holds the name, this
gateway's member name, the shared secret and the gateways it was joined
through. The registrar re-reads it on every pass, so starting, joining and
leaving need no restart. The members sync every
`losos.edge.secondary.interval` over `POST /cluster/secondary/sync`, each
message signed with an HMAC of the secret. Every member learns the boxes the
others enrolled, so a box's token works on all of them, and the time each
member last heard each box. Only the member that heard a box most recently
relays it to the hub, so the hub's route follows the box without flapping.
`GET /manage/api/gateway` lists the members that answer.

`GET /cluster/secondary` is what a box reads from every edge it finds: the
edge's role, the tier of the links it serves, its cluster and member name,
the members that answer, and on an official edge its official cluster and
peers. A gateway without a cluster file counts as a cluster of its own,
named by its host.

### Official clusters and load balancing

The official edges behind one load balancer form an **official cluster**.
Each names it and lists the official edges boxes may use:

```nix
losos.edge.official = {
  cluster = "eu";
  peers = [
    { cluster = "eu"; url = "https://eu-1.losos.cfd"; tunnel = "eu-1.losos.cfd:2333"; }
    { cluster = "eu"; url = "https://eu-2.losos.cfd"; tunnel = "eu-2.losos.cfd:2333"; }
  ];
};
```

A box adds the listed peers that answer to the edges it found. On a
tertiary link it stays on the official edge it uses while that edge
answers. Otherwise it takes the one that a rendezvous hash of its appliance
id weighs heaviest, which spreads boxes evenly across the cluster and moves
only the boxes of an edge that left.

### Bans

A ban names single edges, whole official clusters, or both:

```json
{ "edges": ["https://eu-1.losos.cfd"], "clusters": ["us"] }
```

A box reads it from `losos.proxy.bansFile` on every scan, and an edge from
`losos.edge.bansFile` on every uplink pass. A missing or unreadable file
bans nothing. A banned edge is never chosen. A box on one moves on its own
to another edge of the same tier, and `GET /api/edge` names the edge it
left as `leftBanned`. A gateway whose hub, or the hub's official cluster, is
banned rewrites its uplink to an official peer outside the ban and drops the
old hub's pinned key.

### Custom domains hold a box

A box with a live custom domain stays on the edge that serves the domain.
It keeps dialling that edge while it is gone and fails over to no other
gateway or tier, because a move would break the domain. `GET /api/edge`
shows the edge as `held`, and moving the box to another cluster
(`POST /api/edge/cluster`) answers 409 with `heldByDomain`. A ban is the one
exception: a ban on the held edge, or on its official cluster, moves the box
like any other, and the hold follows it to the new edge.

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
under the zone** (`mattbox.acme.losos.cfd`). It keeps the relayed set equal
to the last list it was sent. A box the spoke stops listing is gone on the
next reconcile, and a spoke that goes silent takes all of its boxes with it
after the heartbeat TTL, like any tenant. Relayed entries live in the hub's
registry as `<spoke>.<box>` with `via: <spoke>`. Each gets a port from the
same tunnel range, a Traefik router `Host(<hostname>)` and a tunnel service
whose token is the **spoke's** token. The spoke is the authenticated party,
and the hub never holds a box's token. Hostname authority stays where it
was. The hub operator chose the zone and the spoke chooses labels inside it.
The hub refuses anything else per box, and the spoke logs it.

:::note[Why boxes do not register with the hub]
Why not let a spoke's boxes register with the hub directly? Because then the
hub operator lists every box in the world, which is the thing a local
gateway exists to avoid. One tenant row per site is the whole cost of a
site on the official edge.
:::

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
    tunnelEndpoint = "edge.losos.cfd:2333";        # the hub's tunnel port
    id = "acme";                                   # the tenant row above
    tokenFile = "/var/secrets/losos-uplink-token";
  };
};
```

The registrar's reconciler, after it has decided its own Traefik routes and
written its tunnel file, relays its **live** tenants (registered, whitelisted
or enrolled, heartbeating) to the hub and renders
`/etc/losos-tunnel/uplink.json`: a tunnel *client* file with one service per
relayed box, `local = 127.0.0.1:<that box's port on this spoke>`, token = the
uplink token. The unit `losos-tunnel-uplink` runs the same client a box runs
(started by a path unit the moment the file exists), and reconnects when the
file changes. The hub's tunnel certificate is pinned on first contact (`GET
/tunnel-pin`, written to `uplink.pinFile` by the registrar itself), as a box
pins its edge's. So a request for
`https://mattbox.acme.losos.cfd/` terminates TLS on the hub, enters the
hub's tunnel service `acme.mattbox`, crosses the uplink to the spoke's local
port for `mattbox`, crosses the box's tunnel, and lands on the box's Nginx.
Two tunnels, one Host header, no change on the box.

**Open enrolment** lets a stock box use a gateway nobody provisioned tokens
for. With `lan.openEnrolment`, the gateway enrols an unknown appliance id
that `/register`s, trust-on-first-use. It keeps the token and hostname
under `/var/lib/losos-registrar/enrolled/`, and every later request
for that id must match. It grants proxy membership only (never `cluster` or
`market`), it is refused on an edge that does not advertise on a LAN (a VPS
must never set it), and the advert says so (`enrol=open`). `losos-registrar
enrol list|forget --dir /var/lib/losos-registrar/enrolled` (on a gateway,
**Enrolled on first contact** on its manage page) shows and drops enrolled
boxes; a box that is still heartbeating re-enrols with the token it holds,
so forgetting is for a box that left. The hub keeps closed enrolment.

## The box: which edge, and when none

`lososd` already finds every edge in reach (LAN by DNS-SD, the configured
official one by URL) and refuses to turn sharing on when there is none. It
picks one path by this rule:

1. **banned edges are dropped** first ([Bans](#bans));
2. **a custom domain's edge**, while the box has one and the edge is not
   banned ([Custom domains hold a box](#custom-domains-hold-a-box));
3. **a gateway of its own secondary cluster** (secondary): the one in use
   while it answers, else the first by member name. A box in no cluster yet
   settles in the cluster in reach with the most gateways (ties by name) and
   keeps it, so it moves between clusters only when its owner says so on
   the Mesh page (`POST /api/edge/cluster`). A gateway that names no cluster
   is used but never written down;
4. **an official edge** (tertiary), by [load balancing](#official-clusters-and-load-balancing),
   or the configured edge (`losos.proxy.registrarUrl` and
   `losos.proxy.edgeTunnelEndpoint`) when none is proved official;
5. **none**. Every edge-dependent feature is off. lososd stops the tunnel
   and announce units, and the sharing gate and the market relay refuse.

`GET /api/edge` carries the choice as `path` (`{name, url, tunnel,
source, tier}`, with `cluster` and `member` on a secondary link and
`officialCluster` on a tertiary one, or `null`), beside every edge's
`tunnel` endpoint, the clusters in reach and the box's own `cluster`. With
`losos.proxy.enable` on, `lososd` writes the path to
`/run/losos/edge-path.env` (`LOSOS_EDGE_PATH_URL`, `_TUNNEL`, `_SOURCE`,
`_TIER`, `_CLUSTER`, `_OFFICIAL_CLUSTER`, `_TUNNEL_PIN`) and drives the two
tunnel units from it. A change of path restarts `losos-tunnel-client` and
`losos-registrar-announce`, and a move to another cluster also restarts
`losos-mesh-join` and `rke2-agent`. With no path, lososd writes
`/run/losos/edge-none` and stops them. Both units carry
`ConditionPathExists=!/run/losos/edge-none`, so nothing restarts them until
an edge is back. Both files are on `/run`, so until the first scan after a
boot the units dial the configured edge. The client's own
`ExecStartPre` pins each edge's tunnel certificate in a file of its own:
the configured edge's at `losos.proxy.tunnelPinFile`, any other edge's (a
LAN gateway, or an official peer) under
`/var/secrets/losos-edge-pins/<host>_<port>.pin`. A second edge therefore
never inherits the first one's certificate. The box's token is the same on
every road. The hub has it in its whitelist, a gateway learns it on first
contact, and `lososd` mints it on first start when the file is missing. Its
public name is the same too. The owner's `losos.proxy.hostname` must be one
label under the site's zone (`mattbox.acme.losos.cfd`) for the relay to
accept it, and that is also the name the box's apps answer to. A box that
falls back to the official edge keeps the same name through its direct
tenant row. The market relay needs no extra switch. It already waits for an
*official* edge to answer, and with none in reach it says
`available: false`.

`nix/tests/edge-federation.nix` runs the three machines (hub, gateway, box) and
walks exactly this: LAN path, enrolment, uplink, fall back, off, back.
`nix/tests/edge-clusters.nix` runs two official edges of one official
cluster, two gateways of one secondary cluster and a box: the cluster sync,
the primary links, failover between the gateways, the tertiary fallback, a
ban on an edge for the box and for a gateway, and the custom domain's hold.

## What stays local

- **The mesh.** Longhorn replication and mesh compute run inside one rke2
  cluster, and that cluster is the edge the box joined. A spoke with
  `losos.edge.cluster.enable` pools its own site's boxes; the hub does not
  bridge two sites' clusters, and nothing here forwards 9345/6443 over the
  uplink. Cross-site pooling is a follow-up with its own design.
- **The market.** The box refuses trading unless an *official* edge is
  in reach ([Official edges](master-proxy.md#official-edges)); a spoke is
  official only if it passes the gateway checks itself, and a relayed box is not a hub tenant, so no `/market/*` call
  can be made on its behalf. The relay carries HTTP reachability, nothing
  else.
- **The passport.** A spoke does not forward the hub's `/passport`; a box
  checks every edge it reaches for itself, and one that wants the market
  through the hub reaches the hub itself, over its own internet.

## Custom domains behind a local edge

[Custom domains](master-proxy.md#custom-domains) work for a box behind a local
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

:::note[The gateway's copy is for reading]
The local edge gets its own rows back in the `/relay` answer. The gateway
writes them to `/var/lib/losos-registrar/hub-routes.json` and logs each
change. That file only tells you what the official edge routes your way.
Nothing on the local edge routes by it.
:::

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

`edge/registrar/tests/domains.rs` runs the whole path against real
registrars. A box behind a gateway gets its domain, a box outside the mesh
gets nothing, and three forged passes get nothing. `nix/tests/edge-dns.nix`
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
   `/var/lib/losos-edge/uplink.json` so one image serves every site, and
   makes its own tunnel certificate on first boot. Like a box it has no
   shell: no console login, no SSH, no root password. Its screen shows the
   manage page's address and the key that signs in there. Boot it on
   Proxmox, libvirt or VirtualBox with a NIC on the LAN; boxes installed from
   the stock ISO find it within a minute. Then open the manage page and,
   under **Hub**, enter the tenant row the hub operator gave you: the tenant
   id, the registrar URL (for example `https://register.losos.cfd`), the
   tunnel endpoint (`edge.losos.cfd:2333`) and the token. **Stop relaying**
   clears it. The page calls `PUT` and `DELETE /manage/api/gateway/uplink`.
2. **An existing NixOS machine**: import `nixosModules.edge` and either set
   `losos.edge.gateway.enable = true` (the image's shape) or write the
   options above by hand, as the two-VM demo and `nix/tests/edge-lan.nix` do.

The hub side for the official edges is one tenant row per site. The Vercel
host carries the same `/relay` route and so can *accept* a spoke for a demo,
but with no tunnel on it the data path, as before, needs the VPS.
