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
