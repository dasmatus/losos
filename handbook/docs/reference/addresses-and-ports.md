---
title: Addresses and ports
sidebar_position: 1
---

# Addresses and ports

## On the box

![Inside a box: nginx holds ports 80 and 443 and routes by path to the admin pages, the control API, LosOS Cloud, LosOS Git and this handbook.](../img/box-architecture.svg)

| Path                          | What                                               | Who may open it                     |
| ----------------------------- | -------------------------------------------------- | ----------------------------------- |
| `/`                           | The admin pages                                     | the local network only              |
| `/handbook/`                  | This handbook                                       | the local network only              |
| `/api/…`                      | The box's control API (JSON, needs the admin key)   | the local network only              |
| `/api/health`                 | `{"ok":true}` when the control daemon is up         | the local network only, no key      |
| `/setup/state.json`           | Name, address, HTTPS, certificate fingerprint       | the local network only              |
| `/setup/losos-ca.crt`         | The box's certificate                               | the local network only              |
| `/setup/trust.sh`, `/setup/trust.ps1` | The one-line certificate installers          | the local network only              |
| `/cloud/`                     | LosOS Cloud                                         | anyone, and through an edge         |
| `/remote.php/`, `/status.php`, `/ocs/`, `/index.php/` | What the Nextcloud apps, calendar apps and file managers call | anyone, and through an edge |
| `/nextcloud/`                 | The same, at the address a box's Nextcloud used     | anyone, and through an edge         |
| `/git/`                       | LosOS Git                                           | anyone, and through an edge         |

:::note[What the local network means]
"The local network only" means a source address in a private range
(`10.…`, `172.16.…` to `172.31.…`, `192.168.…`) that is not the box's own
address and not the mesh's pod range. Anything else, including traffic
arriving through an edge's tunnel, gets **403**.
:::

## Ports

| Port    | Protocol | What                                                   |
| ------- | -------- | ------------------------------------------------------ |
| 80      | HTTP     | Everything above                                        |
| 443     | HTTPS    | The same, with the box's own certificate (`losos.tls.enable`, on by default) |
| 443 (UDP) | HTTP/3 | The same over QUIC, offered to browsers by an `Alt-Svc` header; HTTPS on TCP stays the fallback |
| 5353    | mDNS     | Announcing `<name>.local`, and the box as a `_losos._tcp` service |
| 445     | SMB      | Each person's LosOS Cloud files as a Windows network share (`losos.cloud.windows.enable`, on by default); the local network only |
| 3702 (UDP), 5357 | WS-Discovery | Listing the box under Network in Windows' File Explorer, while the share is on |
| 5355    | LLMNR    | Answering Windows when it asks for the box by its bare name, while the share is on |
| 8082    | HTTP     | The control API, on the box's loopback only; nginx proxies `/api/` to it |
| 8480, 8443 | HTTP, HTTPS | What Tor and I2P deliver for the box's own addresses (`losos.hidden.tor.enable`, `losos.hidden.i2p.enable`), on the box's loopback only. See [Tor and I2P](../manual/tor-and-i2p.md) |
| 8790    | HTTP     | The local copy of the binary caches, on loopback; on the local network only with `losos.cache.local.shareOnLan` |

While the share is on, the mDNS entry for LosOS Cloud is the SMB share
(`_smb._tcp`), so Finder and Linux file managers list the box once.

Nothing else is open. The box reaches the mesh and the edge outwards. It
opens the tunnel and joins the cluster itself, so you open no inbound port
at home.

## On an edge

| Port         | Protocol | What                                                    |
| ------------ | -------- | ------------------------------------------------------- |
| 80, 443      | HTTP(S)  | Traefik: every box's public name                         |
| 2333 (UDP)   | QUIC     | The tunnel boxes dial (`losos.edge.tunnelBindPort`)      |
| 2333 (TCP)   | TLS      | The same tunnel, for boxes on a network that drops UDP   |
| 7883 (UDP)   | QUIC     | The calls relay (`losos.edge.calls.relay.port`)          |

## Names

| Name                        | What                                                     |
| --------------------------- | -------------------------------------------------------- |
| `<name>.local`              | The box, on its own network, over mDNS                   |
| `<name>.<edge domain>`      | The box through its edge, for LosOS Cloud and LosOS Git  |
| `losos-edge.dasmat.us`      | The LosOS edge's control plane and the "find my box" page |
| `proxy.losos.dasmat.us`     | The binary cache the box updates from                    |
| `127.0.0.1:8787`            | Local GHCR-backed Nix cache, on the box and installer |
| `127.0.0.1:8790`            | The box's local copy of all of these, kept on its disk |
| `losos.dasmat.us/proxy`     | A copy of that cache, used when the first does not answer |
| `losos.dasmat.us`           | This handbook on the web                                 |
