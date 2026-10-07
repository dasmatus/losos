# Master proxy

Optional. Makes the box reachable from the internet without opening a port at
home.

```
internet → Traefik (VPS, :443) → rathole server ⇐ tunnel ⇐ rathole client (box) → nginx
```

## Edge (VPS)

Import the `nixosModules.edge` flake output into the VPS configuration and set
`losos.edge.*`:

- `losos.edge.publicDomain` — the domain appliances are served under.
- `losos.edge.tenants` — the allow-list of appliances, each with a hostname
  and a token file. The registrar only creates routes and certificates for
  ids listed here.
- `losos.edge.cluster.enable` — also run the [mesh](Mesh) control plane
  (rke2 server with Longhorn). The proxy works without it.

`losos-registrar` keeps Traefik's dynamic config and rathole's server config
in sync with the registered appliances.

## Appliance

Set `losos.proxy.enable = true` and the `losos.proxy.*` options
(`registrarUrl`, `edgeRatholeEndpoint`, `hostname`, `applianceId`,
`tokenFile`).

## Tunnel encryption

rathole uses its Noise transport. The edge generates a key pair on first
boot. Each appliance fetches the public key from the registrar on first start
and stores it at `losos.proxy.noisePublicKeyFile`. To pin the key out of band,
put the file there beforehand; it is never overwritten. Setting either key
option to `null` falls back to plain TCP. Don't.

## Admin UI stays LAN-only

Tunnel traffic reaches nginx from `127.0.0.1`. The admin routes therefore
deny loopback. Do not add `allow 127.0.0.1` to them, or the admin UI becomes
reachable from the internet.

## Edge on the same LAN

An edge does not have to be a VPS. With `losos.edge.lan.advertise = true` it
also announces itself over mDNS (`_losos-edge._tcp`, with the registrar URL
in a `url=` record, default `http://<edge>.local:8443`, `losos.edge.lan.url`),
binds the registrar API off-loopback and opens its port, so a box on the same
network finds it with no configuration and may share storage through it — see
[Mesh](Mesh#finding-the-edge). This is the shape of an **on-premises
deployment**: one always-on machine on the company network running
`nixosModules.edge`, and boxes installed from the stock ISO beside it.

```nix
# the edge machine's configuration
imports = [ losos.nixosModules.edge ];
losos.edge = {
  enable = true;
  acmeEmail = "ops@example.com";       # a public name gets a certificate as usual
  lan.advertise = true;                # announce on the LAN, bind the API off-loopback
  tenants.<id>.tokenFile = "/run/secrets/tenant-<id>";
};
```

The boxes need nothing: the ISO installs the published `install`
configuration, the Mesh pane shows "Edge proxy found: … On this network" once
the edge answers, and sharing can be switched on. Two limits today: the
tunnel's enrolment address (`losos.proxy.registrarUrl`) is a build-time
option that still defaults to the public edge, so a site that wants the
tunnel to terminate on its own edge sets it in its own flake; and the advert
is mDNS, so the edge and the boxes must share a broadcast domain (one VLAN).

`demo/edge-lan/run.sh` stands this whole arrangement up on one machine as two
VMs on a virtual switch — the edge (as the LAN's router too), a box installed
from the ISO — and walks through found → allowed → edge gone → refused →
back. Its README says which parts stand in for what on a real site.

## Demo deployment on Vercel

`edge-vercel/` runs the registrar's API — the same router and the same
token check — as a Vercel Function at `https://losos-edge.dasmat.us` (the
default `losos.proxy.registrarUrl`), with the registry in a Neon Postgres
and a status page at `/` that shows the registered boxes and the Traefik
configuration the edge would be writing for them. It is the control plane
only: the rathole tunnel, Traefik's TLS, the mesh and the market cannot run
on a serverless host, so the data path (reaching a box through its public
hostname) still needs the VPS. The page also lists ids and hostnames to anyone
with the URL; it is a demonstration host, not an edge. Deployment steps and
the environment variables are in
[edge-vercel/README.md](https://github.com/dasmatus/losos/blob/main/edge-vercel/README.md).
