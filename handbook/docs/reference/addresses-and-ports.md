---
title: Addresses and ports
sidebar_position: 1
---

# Addresses and ports

## On the box

![Inside a box: nginx holds ports 80 and 443 and routes by path to the admin pages, the control API, LosOS cloud, LosOS Git and this handbook.](../img/box-architecture.svg)

| Path                          | What                                               | Who may open it                     |
| ----------------------------- | -------------------------------------------------- | ----------------------------------- |
| `/`                           | The admin pages                                     | the local network only              |
| `/handbook/`                  | This handbook                                       | the local network only              |
| `/api/…`                      | The box's control API (JSON, needs the admin key)   | the local network only              |
| `/api/health`                 | `{"ok":true}` when the control daemon is up         | the local network only, no key      |
| `/setup/state.json`           | Name, address, HTTPS, certificate fingerprint       | the local network only              |
| `/setup/losos-ca.crt`         | The box's certificate                               | the local network only              |
| `/setup/trust.sh`, `/setup/trust.ps1` | The one-line certificate installers          | the local network only              |
| `/nextcloud`                  | LosOS cloud                                         | anyone, and through an edge         |
| `/forgejo/`                   | LosOS Git                                           | anyone, and through an edge         |
| `/.well-known/nodeinfo`, `/.well-known/webfinger` | LosOS Git's federation discovery, while federation is on and the disk is shared | anyone, and through an edge |
| `/nextcloud/ocm-provider/`, `/nextcloud/ocm/…` and the other federation addresses under `/nextcloud` | LosOS cloud's federation, while federation is on and the disk is shared; **404** otherwise | anyone, and through an edge |

"The local network only" means a source address in a private range
(`10.…`, `172.16.…` to `172.31.…`, `192.168.…`) that is not the box's own
address and not the mesh's pod range. Anything else, including traffic
arriving through an edge's tunnel, gets **403**.

## Ports

| Port    | Protocol | What                                                   |
| ------- | -------- | ------------------------------------------------------ |
| 80      | HTTP     | Everything above                                        |
| 443     | HTTPS    | The same, with the box's own certificate (`losos.tls.enable`, on by default) |
| 5353    | mDNS     | Announcing `<name>.local`                               |
| 8082    | HTTP     | The control API, on the box's loopback only; nginx proxies `/api/` to it |

Nothing else is open. The box reaches the mesh and the edge outwards. It
opens the tunnel and joins the cluster itself, so you open no inbound port
at home.

## Names

| Name                        | What                                                     |
| --------------------------- | -------------------------------------------------------- |
| `<name>.local`              | The box, on its own network, over mDNS                   |
| `<name>.<edge domain>`      | The box through its edge, for LosOS cloud and LosOS Git  |
| `losos-edge.dasmat.us`      | The LosOS edge's control plane and the "find my box" page |
| `proxy.losos.dasmat.us`     | The binary cache the box updates from                    |
| `losos-cache-proxy.vercel.app` | The same cache under Vercel's own name                 |
| `losos.dasmat.us/proxy`     | A copy of that cache, used when the first does not answer |
| `losos.dasmat.us`           | This handbook on the web                                 |
