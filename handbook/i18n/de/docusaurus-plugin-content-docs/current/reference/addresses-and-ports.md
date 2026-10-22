---
title: Adressen und Ports
sidebar_position: 1
---

# Adressen und Ports

## Auf der Box \{#on-the-box}

![Im Inneren einer Box: nginx hält die Ports 80 und 443 und leitet nach Pfad an die Admin-Seiten, die Steuer-API, LosOS Cloud, LosOS Git und dieses Handbuch weiter.](@site/docs/img/box-architecture-de.svg)

| Pfad                          | Was                                                 | Wer es öffnen darf                  |
| ----------------------------- | --------------------------------------------------- | ----------------------------------- |
| `/`                           | Die Admin-Seiten                                    | nur das lokale Netzwerk             |
| `/handbook/`                  | Dieses Handbuch                                     | nur das lokale Netzwerk             |
| `/api/…`                      | Die Steuer-API der Box (JSON, braucht den Admin-Schlüssel) | nur das lokale Netzwerk      |
| `/api/health`                 | `{"ok":true}`, wenn der Steuerdienst läuft          | nur das lokale Netzwerk, ohne Schlüssel |
| `/setup/state.json`           | Name, Adresse, HTTPS, Fingerabdruck des Zertifikats | nur das lokale Netzwerk             |
| `/setup/losos-ca.crt`         | Das Zertifikat der Box                              | nur das lokale Netzwerk             |
| `/setup/trust.sh`, `/setup/trust.ps1` | Die einzeiligen Installer für das Zertifikat | nur das lokale Netzwerk           |
| `/cloud/`                     | LosOS Cloud                                         | jeder, auch über eine Edge          |
| `/remote.php/`, `/status.php`, `/ocs/`, `/index.php/` | Was die Nextcloud-Apps, Kalender-Apps und Dateimanager aufrufen | jeder, auch über eine Edge |
| `/nextcloud/`                 | Dasselbe, unter der Adresse, die das Nextcloud der Box hatte | jeder, auch über eine Edge |
| `/forgejo/`                   | LosOS Git                                           | jeder, auch über eine Edge          |
| `/.well-known/nodeinfo`, `/.well-known/webfinger` | Die Föderations-Erkennung von LosOS Git, solange die Föderation an ist und die Festplatte geteilt wird | jeder, auch über eine Edge |

:::note[Was das lokale Netzwerk heißt]
„Nur das lokale Netzwerk“ heißt: eine Quelladresse in einem privaten Bereich
(`10.…`, `172.16.…` bis `172.31.…`, `192.168.…`), die weder die eigene
Adresse der Box noch im Pod-Bereich des Mesh ist. Alles andere, auch Verkehr,
der durch den Tunnel einer Edge ankommt, bekommt **403**.
:::

## Ports \{#ports}

| Port    | Protokoll | Was                                                    |
| ------- | --------- | ------------------------------------------------------ |
| 80      | HTTP      | Alles oben Genannte                                    |
| 443     | HTTPS     | Dasselbe, mit dem eigenen Zertifikat der Box (`losos.tls.enable`, standardmäßig an) |
| 5353    | mDNS      | Ankündigung von `<name>.local` und der Box als `_losos._tcp`-Dienst |
| 8082    | HTTP      | Die Steuer-API, nur auf dem Loopback der Box; nginx leitet `/api/` dorthin weiter |

Sonst ist nichts offen. Die Box erreicht das Mesh und die Edge nach außen.
Sie öffnet den Tunnel und tritt dem Cluster selbst bei, sodass du zu Hause
keinen eingehenden Port öffnest.

## Namen \{#names}

| Name                        | Was                                                      |
| --------------------------- | -------------------------------------------------------- |
| `<name>.local`              | Die Box, in ihrem eigenen Netzwerk, über mDNS            |
| `<name>.<edge domain>`      | Die Box über ihre Edge, für LosOS Cloud und LosOS Git    |
| `losos-edge.dasmat.us`      | Die Steuerebene der LosOS-Edge und die Seite „Find my box“ |
| `proxy.losos.dasmat.us`     | Der Binär-Cache, aus dem sich die Box aktualisiert       |
| `losos-cache-proxy.vercel.app` | Derselbe Cache unter Vercels eigenem Namen            |
| `losos.dasmat.us/proxy`     | Eine Kopie dieses Caches, die verwendet wird, wenn der erste nicht antwortet |
| `losos.dasmat.us`           | Dieses Handbuch im Web                                   |
