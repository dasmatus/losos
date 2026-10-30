---
title: The edge as an exit
sidebar_position: 6.2
mdx:
  format: md
---

# The edge as an exit

An edge can also be a way out to the internet for the computers of the
people it serves, the way a VPN is. LosOS Desktop uses it: its network
machine picks an edge the same way a box does ([Edge
federation](edge-federation.md)) and sends the computer's traffic through
it, so sites see the edge's address and not the person's home line.

It is off until the operator turns it on, and then only for the tenants
the operator names:

```nix
losos.edge.exit.enable = true;
losos.edge.tenants.alice.exit = true;
```

## How a computer joins

A tenant enrols a WireGuard key with `POST /exit`, sending its id, its
token and the key. The edge answers with:

| Field | What it is |
| --- | --- |
| `address` | the computer's one address on the edge's `losos-exit` interface, kept for as long as the id stays |
| `server_public_key`, `endpoint` | the edge's WireGuard key and where to dial it (`losos.edge.exit.endpoint`, by default the public domain on UDP 51820) |
| `allowed_ips` | `0.0.0.0/0` and `::/0`: everything goes into the tunnel |
| `onion`, `onion_port` | the edge's own onion address for the SOCKS door below, when it has one |
| `socks_username`, `socks_password` | the credentials for that door; the password is new on every enrolment |

`POST /exit/leave` takes the computer off again. A tenant without `exit`
gets 403, a key already enrolled for another id 400, and a full subnet 503.

The registrar keeps the enrolled computers in `/var/lib/losos-exit/peers.json`.
`losos-exit-sync` puts them on the interface whenever that file changes and
takes off the ones that left. The edge's private WireGuard key stays in
`/var/lib/losos-exit-key`, which only root can read.

## What it lets out

Only the internet. The edge drops anything a computer sends from the exit
to private, loopback, carrier-grade NAT, link-local, documentation,
multicast or reserved addresses, and anything to another computer on the
same exit. Without that, the edge's own network, a cloud's metadata
service and every other tenant would be one hop away. IPv6 is not handed
out and nothing of it is forwarded.

## Through Tor first

With `losos.edge.exit.onion.enable`, the same enrolment also opens a SOCKS5
server, `exit-socks`, behind an onion service of the edge's own. It is for
a computer whose traffic already goes through Tor (LosOS Desktop's schizo
mode): Tor carries the connection to the edge, and the edge makes it. The
site sees the edge, and the edge sees only Tor.

The server asks for the username and password from the enrolment and
compares the password against a SHA-256 digest; the file holds nothing
else. It connects only to public addresses, and refuses a name that
resolves to any address that is not one. It runs as its own user, reads
the peers file and nothing else, and systemd refuses it the private ranges
a second time.

:::warning[The edge sees where you go]
An exit hides a person's home address from the sites they visit, not
from the edge. The edge's operator can see where the traffic goes, as
any VPN provider can. Use an edge you trust, or Tor first.
:::
