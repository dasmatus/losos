---
title: Die Edge als Ausgang
sidebar_position: 6.2
mdx:
  format: md
---

# Die Edge als Ausgang

Eine Edge kann für die Computer der Menschen, denen er dient, auch ein Weg
ins Internet sein, so wie ein VPN. LosOS Desktop nutzt das: Seine
Netzwerkmaschine wählt eine Edge so, wie eine Box es tut
([Edge-Föderation](/in-depth/edge-federation.md)), und schickt den Verkehr
des Computers hindurch. Seiten sehen dann die Adresse der Edge, nicht den
Anschluss zu Hause.

Sie ist aus, bis der Betreiber sie einschaltet, und dann nur für die
Mieter, die der Betreiber nennt:

```nix
losos.edge.exit.enable = true;
losos.edge.tenants.alice.exit = true;
```

## Wie ein Computer beitritt

Ein Mieter meldet mit `POST /exit` einen WireGuard-Schlüssel an und schickt
seine Id, sein Token und den Schlüssel. Die Edge antwortet mit:

| Feld | Was es ist |
| --- | --- |
| `address` | die eine Adresse des Computers auf der Schnittstelle `losos-exit`, solange die Id bleibt |
| `server_public_key`, `endpoint` | der WireGuard-Schlüssel der Edge und wohin man wählt (`losos.edge.exit.endpoint`, sonst die öffentliche Domain auf UDP 51820) |
| `allowed_ips` | `0.0.0.0/0` und `::/0`: alles geht in den Tunnel |
| `onion`, `onion_port` | die eigene Onion-Adresse der Edge für die SOCKS-Tür unten, wenn sie eine hat |
| `socks_username`, `socks_password` | die Zugangsdaten für diese Tür; das Passwort ist bei jeder Anmeldung neu |

`POST /exit/leave` meldet den Computer wieder ab. Ein Mieter ohne `exit`
bekommt 403, ein Schlüssel, der schon für eine andere Id angemeldet ist,
400, und ein volles Subnetz 503.

Der Registrar hält die angemeldeten Computer in
`/var/lib/losos-exit/peers.json`. `losos-exit-sync` setzt sie bei jeder
Änderung der Datei auf die Schnittstelle und nimmt die gegangenen herunter.
Der private WireGuard-Schlüssel der Edge bleibt in `/var/lib/losos-exit-key`,
das nur root lesen kann.

## Was sie hinauslässt

Nur das Internet. Die Edge verwirft alles, was ein Computer vom Ausgang an
private, Loopback-, Carrier-Grade-NAT-, Link-Local-, Dokumentations-,
Multicast- oder reservierte Adressen schickt, und alles an einen anderen
Computer am selben Ausgang. Ohne das wären das eigene Netz der Edge, der
Metadatendienst einer Cloud und jeder andere Mieter einen Sprung entfernt.
IPv6 wird nicht vergeben und nichts davon weitergeleitet.

## Erst durch Tor

Mit `losos.edge.exit.onion.enable` öffnet dieselbe Anmeldung auch einen
SOCKS5-Server, `exit-socks`, hinter einem eigenen Onion-Dienst der Edge. Er
ist für einen Computer, dessen Verkehr schon durch Tor geht (der
Schizo-Modus von LosOS Desktop): Tor trägt die Verbindung zur Edge, und die
Edge baut sie auf. Die Seite sieht die Edge, und die Edge sieht nur Tor.

Der Server verlangt Namen und Passwort aus der Anmeldung und vergleicht
das Passwort mit einem SHA-256-Hash; mehr steht nicht in der Datei. Er
verbindet nur zu öffentlichen Adressen und lehnt einen Namen ab, der auf
irgendeine nicht öffentliche Adresse auflöst. Er läuft als eigener
Benutzer, liest nur die Mieterdatei, und systemd verweigert ihm die
privaten Bereiche ein zweites Mal.

:::warning[Die Edge sieht, wohin Sie gehen]
Ein Ausgang verbirgt die Adresse zu Hause vor den Seiten, die man besucht,
nicht vor der Edge. Der Betreiber der Edge sieht, wohin der Verkehr geht,
wie jeder VPN-Anbieter. Nutzen Sie eine Edge, der Sie vertrauen, oder
erst Tor.
:::
