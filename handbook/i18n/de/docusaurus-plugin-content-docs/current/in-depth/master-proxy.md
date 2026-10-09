---
title: Master-Proxy
sidebar_position: 5
mdx:
  format: md
---

# Master-Proxy

Optional. Er macht die Box aus dem Internet erreichbar, ohne dass du zu Hause
einen Port öffnen musst.

![Internet → Traefik (VPS, :443) → rathole-Server ⇐ Tunnel ⇐ rathole-Client (Box) → nginx](/img/diagrams/master-proxy-de.svg)

## Edge (VPS)

Importiere den Flake-Output `nixosModules.edge` in die Konfiguration des
VPS und setze `losos.edge.*`:

- `losos.edge.publicDomain` ist die Domain, unter der die Appliances
  ausgeliefert werden.
- `losos.edge.tenants` ist die Allowlist der Appliances, jede mit einem
  Hostnamen und einer Token-Datei. Der Registrar legt Routen und Zertifikate
  nur für die hier aufgeführten IDs an.
- `losos.edge.cluster.enable` betreibt zusätzlich die Control Plane des
  [Mesh](/in-depth/mesh.md), einen rke2-Server mit Longhorn. Der Proxy funktioniert
  auch ohne sie.

`losos-registrar` hält die dynamische Konfiguration von Traefik und die
Serverkonfiguration von rathole mit den registrierten Appliances im
Gleichschritt.

## Appliance

Setze `losos.proxy.enable = true` und die Optionen `losos.proxy.*`
(`registrarUrl`, `edgeRatholeEndpoint`, `hostname`, `applianceId`,
`tokenFile`).

## Tunnel-Verschlüsselung

rathole nutzt seinen Noise-Transport. Die Edge erzeugt beim ersten Booten ein
Schlüsselpaar. Jede Appliance holt sich beim ersten Start den öffentlichen
Schlüssel vom Registrar und legt ihn unter `losos.proxy.noisePublicKeyFile`
ab. Willst du den Schlüssel auf einem separaten Weg festlegen, leg die
Datei vorher dort ab; die Appliance überschreibt sie nie. Wird eine der beiden
Schlüsseloptionen auf `null` gesetzt, fällt die Verbindung auf reines TCP
zurück. Tu das nicht.

## Die Admin-Oberfläche bleibt im LAN

Der Tunnelverkehr erreicht nginx von `127.0.0.1`, daher verweigern die
Admin-Routen Loopback. Füge ihnen kein `allow 127.0.0.1` hinzu, sonst
wird die Admin-Oberfläche aus dem Internet erreichbar.

## Edge im selben LAN

Eine Edge muss kein VPS sein. Mit `losos.edge.lan.advertise = true` kündigt
sie sich zusätzlich per mDNS als `_losos-edge._tcp` an, mit der URL des
Registrars in einem `url=`-Eintrag (`losos.edge.lan.url`, Standard
`http://<edge>.local:8443`). Sie bindet die Registrar-API außerhalb von
Loopback und öffnet ihren Port. Eine Box im selben Netz findet sie dann ohne
jede Konfiguration und darf über sie Speicher teilen, siehe
[Mesh](/in-depth/mesh.md#die-edge-finden). So sieht ein On-Premises-Betrieb aus: ein
ständig laufender Rechner im Firmennetz, auf dem `nixosModules.edge` läuft,
und daneben Boxen, die vom Standard-ISO installiert wurden.

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

Die Boxen brauchen nichts. Das ISO installiert die veröffentlichte
`install`-Konfiguration, der Mesh-Bereich zeigt "Edge-Proxy gefunden: … In diesem
Netzwerk", sobald die Edge antwortet,
und das Teilen lässt sich einschalten.

So bündelt auch ein LAN ohne Internet seinen Speicher. Eine Box vergrößert
ihre eigene Festplatte ganz ohne Edge (`losos-ctl grow`, Modus Local). Das
Bündeln über mehrere Boxen hinweg ist jedoch das Mesh, und die Control Plane
des Mesh (rke2-Server, Longhorn, der Registrar) läuft auf dem Edge-Rechner.
Eine Edge auf einem PC im LAN ist also das, was einen Offline-Standort zum
Laufen bringt. Das WAN kann abgezogen werden, denn die Boxen finden sie
ausschließlich per mDNS. Derzeit gibt es zwei Einschränkungen:

- Die Registrierungsadresse des Tunnels (`losos.proxy.registrarUrl`) ist
  eine Build-Option, die standardmäßig noch auf die öffentliche Edge zeigt.
  Ein Standort, an dem der Tunnel auf der eigenen Edge enden soll, setzt sie
  in seinem eigenen Flake.
- Die Ankündigung läuft über mDNS, daher müssen Edge und Boxen eine
  Broadcast-Domäne teilen (ein VLAN).

`demo/edge-lan/run.sh` baut diese ganze Anordnung auf einem Rechner als VMs
an einem virtuellen Switch auf: die Edge, die zugleich der Router des LAN
ist, und zwei vom ISO installierte Boxen. Es arbeitet die
Verifikations-Checkliste ab (`demo/edge-lan/CHECKLIST.md`). Ohne Edge
verweigern beide Boxen. Die Edge erscheint, beide finden sie und erlauben das
Teilen. Die Edge verschwindet, sie verweigern wieder, und sie kommt zurück.
Die zugehörige README erklärt, welche Teile an einem echten Standort wofür
stehen und was die Schritte für gebündelten Speicher noch benötigen.

## Mehrere Edges: Föderation

Eine selbst betriebene Edge kann ihre Boxen über eine offizielle Edge
weiterleiten, sodass zwei Standorte, jeder hinter seiner eigenen lokalen
Edge, einander über das Internet erreichen, und eine Box bevorzugt ihre
lokale Edge, weicht auf die offizielle aus und schaltet ihre von der Edge
abhängigen Funktionen ab, wenn keine von beiden antwortet. Das Design, das
Tenant-Attribut auf Hub-Seite (`relayZone`), `losos.edge.uplink.*` auf
Spoke-Seite, die offene LAN-Registrierung und das Gateway-VM-Image stehen
unter [Edge-Föderation](/in-depth/edge-federation.md).

## Offizielle Edges

Jede Edge kann gefunden werden und Speicher weiterleiten. Nur Edges, die
LosOS betreibt, dürfen den P2P-Handel mit Speicher und Rechenleistung
abwickeln (den [Marktplatz](/in-depth/market.md)). Die Box prüft das selbst, bei jedem
Scan, und die eigene Edge einer Firma bekommt alles außer dem Marktplatz.

Die Prüfung ist eine Ed25519-Identität:

- **Der LosOS-Root-Schlüssel.** Ein Schlüsselpaar. Die öffentliche Hälfte
  ist eine Datei, die jede Box mitbringt, `keys/official-edge-root.pub` in
  diesem Repository (`losos.proxy.officialRootKeyFile`). Der Projektinhaber
  verwahrt die private Hälfte offline. Sie gelangt nie in das Repository
  oder auf eine Box und signiert Edge-Zertifikate und sonst nichts. Bis
  jemand den öffentlichen Schlüssel in diese Datei schreibt, ist keine Edge
  offiziell und der Marktplatz ist auf jeder aus dem Baum gebauten Box aus.
  Der Standard schlägt geschlossen fehl.
- **Ein Edge-Zertifikat.** Jede offizielle Edge hat ein eigenes
  Schlüsselpaar und ein kleines JSON-Zertifikat
  `{name, url, public_key, not_after, signature}`, signiert vom Root. Der
  Registrar beantwortet `GET /identity?nonce=<hex>` mit dem Zertifikat und
  einer Signatur über die Nonce mit dem Schlüssel der Edge.
- **Die Prüfung auf der Box** (`backend/src/edge.rs`) läuft für jede Edge,
  deren `/health` geantwortet hat. Der Root hat das Zertifikat signiert, das
  Zertifikat nennt die URL, mit der die Box spricht, es ist nicht abgelaufen,
  und der Schlüssel der Edge hat die Nonce signiert, die sich die Box gerade
  ausgedacht hat. Alle vier bestehen, oder die Edge ist nicht offiziell. Das
  Ergebnis ist `official` pro Edge in `GET /api/edge` und ein Zeichen neben
  dem Namen jeder Edge im Mesh-Bereich: ein Häkchen bei einer offiziellen
  Edge, eine Warnung bei jeder anderen, mit einem Tooltip, der auflistet, was
  diese Edge für die Box nicht leisten kann. Eine Box kann mehrere Edges in
  Reichweite haben. Teilen funktioniert über jede von ihnen, Handel nur über
  eine offizielle. Solange keine offizielle Edge in Reichweite ist,
  antwortet das Marktplatz-Relay mit `{available: false, reason:
  "noOfficialEdge"}` und lehnt jede Aktion mit 409
  `officialEdgeRequired` ab. Nichts verlässt die Box.

Die Edge erzeugt ihren Identitätsschlüssel beim ersten Start
(`losos.edge.identity.keyFile`, Standard
`/var/lib/losos-registrar/identity.key`), und die private Hälfte verlässt die
Edge nie. Für eine offizielle Edge signiert LosOS mit dem Root ein Zertifikat
und installiert es dort; die Edge schreibt es neben den Schlüssel
(`losos.edge.identity.certFile`). Bis ein Zertifikat eintrifft, beantwortet
die Edge `/identity` mit 404 und ist nicht offiziell. So sieht die eigene Edge
einer Firma aus, einschließlich der, die `demo/edge-lan/` bootet. Eine Edge,
bei der beide Optionen auf `null` stehen, stellt überhaupt keine Identität
bereit.

Was das schützt: Eine Firma, die ihre eigene Edge betreibt, erhält einen
funktionierenden On-Premises-Betrieb, kann darüber aber keine Geschäfte
abrechnen. Ein Fremder, der eine Edge aufsetzt, kann Boxen ebenfalls nicht
dazu bringen, darüber zu handeln, denn nur der private Schlüssel des Roots
macht eine Edge offiziell. Was es nicht schützt: Das Marktplatz-Protokoll ist
nicht geheim, und der private Schlüssel des Roots ist das gesamte
Geheimnis. Geht er verloren, muss ein Release ausgeliefert werden, das den
Vertrauensanker jeder Box durch einen neuen Schlüssel ersetzt.

## DNS-Zone und eigene Domains

Eine offizielle Edge kann eine eigene DNS-Zone bereitstellen und Domains
routen, die Box-Inhaber bereits besitzen. Das ist aus, solange
`losos.edge.dns.enable` nicht gesetzt ist.

```nix
losos.edge.dns = {
  enable = true;
  zone = "boxes.losos.dasmat.us";   # default: "boxes.${publicDomain}"
  ipv4 = [ "203.0.113.7" ];         # the edge's public addresses; one family at least
  ipv6 = [ "2001:db8::7" ];
  # nameservers defaults to [ "ns1.<zone>" ], served with glue from ipv4/ipv6.
};
```

Der Registrar rendert die Zonendatei bei jedem Abgleich, und Knot liefert sie
auf Port 53 aus, den das Modul öffnet. Eine Path-Unit lädt Knot neu, wenn sich
die Datei ändert. Die Zone enthält SOA, NS und Glue, jeden Tenant-Hostnamen,
der in die Zone fällt, und einen Namen pro Box, deren Stripe-Konto bestätigt
ist: 16 Hex-Zeichen eines SHA-256 der Box-UUID. Jeder Name löst auf die Edge
auf, denn dort sind Traefik und der Tunnel. Die eigene Adresse einer Box
erscheint nie im öffentlichen DNS, ebenso wenig eine Stripe-Konto-ID oder eine
UUID. Die Seriennummer ist die Unix-Zeit der Änderung, oder eins mehr als die
vorherige Seriennummer, falls diese größer ist.

Der Operator delegiert die Zone einmalig beim Registrar der übergeordneten
Domain:

```
boxes.losos.dasmat.us.      NS  ns1.boxes.losos.dasmat.us.
ns1.boxes.losos.dasmat.us.  A   203.0.113.7
ns1.boxes.losos.dasmat.us.  AAAA 2001:db8::7
```

Nichts wird vergeben, bevor die Edge ein nicht abgelaufenes
Identitätszertifikat besitzt ([Offizielle Edges](#offizielle-edges)). Bis dahin
besteht die Zone nur aus SOA, NS und Glue, und `/domains/*` antwortet mit 503.

### Eigene Domains

Eine Box fügt eine Domain über `POST /domains/add` hinzu (authentifiziert mit
ihrem Tunnel-Token, von lososd aus Einstellungen → Netzwerk weitergereicht). Die
Edge akzeptiert sie nur, wenn das verbundene Stripe-Konto der Box bereit ist
und die Box-UUID trägt, dieselbe Prüfung, die der Marktplatz vor einer
Auszahlung vornimmt. Dann veröffentlicht der Inhaber zwei Einträge:

- `_losos-challenge.<domain> TXT "losos-domain-v1=<32 hex>"`, wobei der
  Hex-Wert ein SHA-256 über die Domain, die Box-UUID und die Stripe-Konto-ID
  ist. Er belegt die Kontrolle über die Domain und bindet sie an diese Box
  und dieses Konto. Ein von einem früheren Inhaber zurückgelassener Eintrag
  belegt nichts.
- `<domain> CNAME <label>.<zone>`, oder A/AAAA-Einträge mit den Adressen der
  Edge an einem Zonen-Apex.

Der Registrar fragt beide über DNS-over-HTTPS ab (`losos.edge.dns.checkUrl`,
standardmäßig die JSON-API von Cloudflare), höchstens 16 pro Durchlauf, alle
30 s, solange ein Anspruch wartet, und stündlich, sobald er live ist. Erst wenn
beide Einträge stimmen, erhält die Domain einen Traefik-Router mit Let's
Encrypt, geroutet durch den Tunnel der Box. Dass auch auf den CNAME gewartet
wird, hält Traefik davon ab, bei Let's Encrypt einen Namen anzufragen, der die
Edge nicht erreicht. Eine live geschaltete Domain verfällt nach drei
fehlgeschlagenen Nachprüfungen in Folge, oder sofort, wenn das Stripe-Konto
nicht mehr bereit ist. Eine Box darf fünf Domains halten, eine Domain gehört
zu einer Box, und ein unbestätigter Anspruch wird nach sieben Tagen verworfen.
Die Ansprüche liegen in `/var/lib/losos-registrar/domains.json`.

Auf der Box hält lososd die live geschalteten Domains in
`/var/lib/losos-public-names/domains.json`. Die Datei wird alle zwei Minuten
aktualisiert und immer dann, wenn der Bereich Netzwerk danach fragt. Der
Nextcloud-Pod bindet dieses Verzeichnis schreibgeschützt ein, fügt die Namen
pro Anfrage zu `trusted_domains` hinzu und verwendet die Domain einer Anfrage
für `overwritehost`, wenn sie eine davon ist. Das tut nur der Container-Modus.
LosOS Git behält seine `ROOT_URL` mit dem Edge-Namen.

Auch eine Box hinter einer lokalen Edge bekommt ihre Domains geroutet, über
diese lokale Edge, sobald sie sich mit einem Relay-Pass für die lokale Edge
verbürgt hat und dem Mesh dieser Edge beigetreten ist. Der Registrar hält
diese Routen selbst, in `relay-routes.json` neben seiner Registry.
[Edge-Föderation](/in-depth/edge-federation.md#eigene-domains-hinter-einer-lokalen-edge)
enthält die Details und `losos.edge.dns.relayRoutes`.

`tests/edge-dns.nix` (`losos-edge-dns`) bootet eine Edge und einen Client und
fragt Knot über UDP und TCP ab. `backend-registrar/tests/domains.rs` spielt
den Anspruchsablauf gegen einen echten Registrar und einen nachgebildeten
DNS-over-HTTPS-Server durch.

## Demo-Betrieb auf Vercel

`edge-vercel/` betreibt die API des Registrars, denselben Router und dieselbe
Token-Prüfung, als Vercel Function unter `https://losos-edge.dasmat.us`, dem
Standardwert von `losos.proxy.registrarUrl`. Die Registry liegt in einer Neon
Postgres, und eine Statusseite unter `/` zeigt die registrierten Boxen und
die Traefik-Konfiguration, die die Edge für sie schreiben würde. Es ist nur
die Control Plane. Der rathole-Tunnel, das TLS von Traefik, das Mesh und der
Marktplatz können nicht auf einem Serverless-Host laufen, daher braucht der
Datenpfad, also das Erreichen einer Box über ihren öffentlichen Hostnamen,
weiterhin den VPS. Die Seite listet außerdem IDs und Hostnamen für jeden auf,
der die URL kennt. Sie ist ein Demonstrations-Host, keine Edge. Die Schritte
für das Deployment und die Umgebungsvariablen stehen in
[edge-vercel/README.md](https://github.com/dasmatus/losos/blob/main/edge-vercel/README.md).
