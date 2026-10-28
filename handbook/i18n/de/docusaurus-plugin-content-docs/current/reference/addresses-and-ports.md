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
| `/git/`                       | LosOS Git                                           | jeder, auch über eine Edge          |

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
| 443 (UDP) | HTTP/3  | Dasselbe über QUIC, Browsern per `Alt-Svc`-Header angeboten; HTTPS über TCP bleibt der Rückfall |
| 5353    | mDNS      | Ankündigung von `<name>.local` und der Box als `_losos._tcp`-Dienst |
| 445     | SMB       | Die Dateien jeder Person in LosOS Cloud als Windows-Netzwerkfreigabe (`losos.cloud.windows.enable`, standardmäßig an); nur das lokale Netz |
| 3702 (UDP), 5357 | WS-Discovery | Die Box unter Netzwerk im Windows-Explorer zeigen, solange die Freigabe an ist |
| 5355    | LLMNR     | Windows antworten, wenn es die Box beim bloßen Namen sucht, solange die Freigabe an ist |
| 8082    | HTTP      | Die Steuer-API, nur auf dem Loopback der Box; nginx leitet `/api/` dorthin weiter |
| 8480, 8443 | HTTP, HTTPS | Was Tor und I2P für die eigenen Adressen der Box zustellen (`losos.hidden.tor.enable`, `losos.hidden.i2p.enable`), nur auf dem Loopback der Box. Siehe [Tor und I2P](../manual/tor-and-i2p.md) |
| 8790    | HTTP      | Die lokale Kopie der Binär-Caches, auf dem Loopback; im lokalen Netz nur mit `losos.cache.local.shareOnLan` |

Solange die Freigabe an ist, ist der mDNS-Eintrag für LosOS Cloud die
SMB-Freigabe (`_smb._tcp`), sodass Finder und Linux-Dateimanager die Box
einmal zeigen.

Sonst ist nichts offen. Die Box erreicht das Mesh und die Edge nach außen.
Sie öffnet den Tunnel und tritt dem Cluster selbst bei, sodass du zu Hause
keinen eingehenden Port öffnest.

## Auf einer Edge \{#on-an-edge}

| Port         | Protokoll | Was                                                    |
| ------------ | --------- | ------------------------------------------------------ |
| 80, 443      | HTTP(S)   | Traefik: der öffentliche Name jeder Box                 |
| 2333 (UDP)   | QUIC      | Der Tunnel, den Boxen anwählen (`losos.edge.tunnelBindPort`) |
| 2333 (TCP)   | TLS       | Derselbe Tunnel für Boxen in einem Netz, das UDP verwirft |
| 7883 (UDP)   | QUIC      | Das Relay für Anrufe (`losos.edge.calls.relay.port`)    |

## Namen \{#names}

| Name                        | Was                                                      |
| --------------------------- | -------------------------------------------------------- |
| `<name>.local`              | Die Box, in ihrem eigenen Netzwerk, über mDNS            |
| `<name>.<edge domain>`      | Die Box über ihre Edge, für LosOS Cloud und LosOS Git    |
| `losos-edge.dasmat.us`      | Die Steuerebene der LosOS-Edge und die Seite „Find my box“ |
| `proxy.losos.dasmat.us`     | Der Binär-Cache, aus dem sich die Box aktualisiert       |
| `127.0.0.1:8787`            | Lokaler GHCR-Nix-Cache auf Gerät und Installationsmedium |
| `127.0.0.1:8790`            | Die lokale Kopie all dieser Caches auf der Platte der Box |
| `losos.dasmat.us/proxy`     | Eine Kopie dieses Caches, die verwendet wird, wenn der erste nicht antwortet |
| `losos.dasmat.us`           | Dieses Handbuch im Web                                   |
