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
