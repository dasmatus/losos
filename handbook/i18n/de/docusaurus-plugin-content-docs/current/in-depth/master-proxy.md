---
title: Master-Proxy
sidebar_position: 5
mdx:
  format: md
---

# Master-Proxy

Optional. Er macht die Box aus dem Internet erreichbar, ohne dass du zu Hause
einen Port öffnen musst.

![Internet → Traefik (VPS, :443) → Tunnel-Server ⇐ QUIC-Tunnel ⇐ Tunnel-Client (Box) → nginx](/img/diagrams/master-proxy-de.svg)

## Edge (VPS) {#edge-vps}

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

`losos-registrar` hält die Datei des Tunnel-Servers mit den registrierten
Appliances im Gleichschritt, und Traefik findet sie über den Registrar.

## Wie Traefik die Boxen findet {#how-traefik-finds-the-boxes}

Traefik entdeckt die Boxen über seinen HTTP-Provider. Alle zwei Sekunden
fragt er den Registrar auf einem nur über Loopback erreichbaren Port
(`losos.edge.traefikProviderPort`, standardmäßig 8090) nach den aktuellen
Routen: eine pro Name einer laufenden Box, eigener Domain und
veröffentlichter virtueller Maschine, jede zum Tunnelende dieser Box. Eine
Box, die sich registriert, ist innerhalb von Sekunden erreichbar, und eine,
deren Heartbeats ausbleiben, verliert ihre Route, sobald der Registrar sie
entfernt. Der Registrar schreibt nichts in das Verzeichnis von Traefik, dort
liegen nur die Routen, die NixOS verwaltet.

Hält der Registrar an oder startet er neu, behält Traefik die zuletzt
erhaltenen Routen, und die Boxen bleiben erreichbar. Beim Update einer Edge,
auf der noch die Dateien `losos.yml` und `losos-vms.yml` eines älteren
Registrars liegen, werden diese beim Umschalten entfernt.

## HTTP/3 {#http3}

Der öffentliche Einstiegspunkt beantwortet HTTP/3 über QUIC auf UDP-Port 443
und daneben HTTP/1.1 und HTTP/2 auf TCP-Port 443. Traefik kündigt HTTP/3 im
Header `Alt-Svc` an, sodass ein Browser nach der ersten Anfrage zu QUIC
wechselt. Ein Netz, das UDP sperrt, funktioniert weiter über TCP. Öffne UDP
443 in jeder Firewall vor der Edge; die Firewall der Edge selbst öffnet ihn.

## Wenn eine Box nicht erreichbar ist {#when-a-box-cant-be-reached}

Erreicht Traefik eine Box nicht (sie ist ausgeschaltet, startet neu, ist
offline oder ihr Tunnel ist unterbrochen), bekommt der Besucher statt eines
nackten „Bad Gateway“ eine Seite der Edge, die das erklärt, auf Englisch,
Slowakisch oder Deutsch je nach Sprache des Browsers. Der Statuscode bleibt
502, oder 504, wenn die Box nicht rechtzeitig geantwortet hat.

## Appliance {#appliance}

Setze `losos.proxy.enable = true` und die Optionen `losos.proxy.*`
(`registrarUrl`, `edgeTunnelEndpoint`, `hostname`, `applianceId`,
`tokenFile`).

## Der Tunnel {#the-tunnel}

Die Box hält eine QUIC-Verbindung zur Edge, auf UDP-Port 2333
(`losos.edge.tunnelBindPort`). Jede Verbindung eines Besuchers läuft darin
als eigener Stream, sodass ein langsamer Download keine anderen Anfragen
aufhält, eine neue Anfrage keinen neuen Handshake braucht und die Verbindung
einen Adresswechsel der Box übersteht. Die Box wählt sich nur nach außen
ein, sie funktioniert also hinter NAT und Carrier-Grade-NAT ohne
Portweiterleitung. Ein Keepalive alle 10 Sekunden hält den NAT-Eintrag offen.

Verwirft ein Netz UDP, gibt die Box QUIC fünf Sekunden und wählt dann
denselben Port über TCP: TLS 1.3 mit demselben Zertifikat, darin
HTTP/2-Streams. Alle fünf Minuten versucht sie QUIC erneut und wechselt
zurück, sobald es durchkommt. Die Firewall der Edge öffnet den Port für
beides.

## Tunnel-Verschlüsselung {#tunnel-encryption}

Der Tunnel ist auf beiden Transporten TLS 1.3. Die Edge erzeugt beim ersten
Start ein selbstsigniertes Zertifikat und behält es in
`/var/lib/losos-tunnel`. Jede Appliance holt sich vor ihrer ersten
Verbindung den SHA-256 des Zertifikats vom Registrar (`GET /tunnel-pin`) und
legt ihn unter `losos.proxy.tunnelPinFile` ab; ab dann akzeptiert der Tunnel
dieses Zertifikat und kein anderes. Willst du es auf einem separaten Weg
festlegen, leg die Datei vorher dort ab; die Appliance überschreibt sie nie.
Jeden Dienst, den die Box anbietet, beansprucht sie mit ihrem Token, das in
konstanter Zeit verglichen wird.

## Die Admin-Oberfläche bleibt im LAN {#admin-ui-stays-lan-only}

Der Tunnelverkehr erreicht nginx von `127.0.0.1`, daher verweigern die
Admin-Routen Loopback. Füge ihnen kein `allow 127.0.0.1` hinzu, sonst
wird die Admin-Oberfläche aus dem Internet erreichbar.

## Edge im selben LAN {#edge-on-the-same-lan}

Eine Edge muss kein VPS sein. Mit `losos.edge.lan.advertise = true` kündigt
sie sich zusätzlich per mDNS als `_losos-edge._tcp` an, mit der URL des
Registrars in einem `url=`-Eintrag (`losos.edge.lan.url`, Standard
`http://<edge>.local:8443`). Sie bindet die Registrar-API außerhalb von
Loopback und öffnet ihren Port. Eine Box im selben Netz findet sie dann ohne
jede Konfiguration und darf über sie Speicher teilen, siehe
[Mesh](/in-depth/mesh.md#finding-the-edge). So sieht ein On-Premises-Betrieb aus: ein
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
ausschließlich per mDNS.

:::note[Derzeit zwei Einschränkungen]
- Die Registrierungsadresse des Tunnels (`losos.proxy.registrarUrl`) ist
  eine Build-Option, die standardmäßig noch auf die öffentliche Edge zeigt.
  Ein Standort, an dem der Tunnel auf der eigenen Edge enden soll, setzt sie
  in seinem eigenen Flake.
- Die Ankündigung läuft über mDNS, daher müssen Edge und Boxen eine
  Broadcast-Domäne teilen (ein VLAN).
:::

`nix/demo/edge-lan/run.sh` baut diese ganze Anordnung auf einem Rechner als VMs
an einem virtuellen Switch auf: die Edge, die zugleich der Router des LAN
ist, und zwei vom ISO installierte Boxen. Es arbeitet die
Verifikations-Checkliste ab (`nix/demo/edge-lan/CHECKLIST.md`). Ohne Edge
verweigern beide Boxen. Die Edge erscheint, beide finden sie und erlauben das
Teilen. Die Edge verschwindet, sie verweigern wieder, und sie kommt zurück.
Die zugehörige README erklärt, welche Teile an einem echten Standort wofür
stehen und was die Schritte für gebündelten Speicher noch benötigen.

## Mehrere Edges: Föderation {#several-edges-federation}

Eine selbst betriebene Edge kann ihre Boxen über eine offizielle Edge
weiterleiten, sodass zwei Standorte, jeder hinter seiner eigenen lokalen
Edge, einander über das Internet erreichen, und eine Box bevorzugt ihre
lokale Edge, weicht auf die offizielle aus und schaltet ihre von der Edge
abhängigen Funktionen ab, wenn keine von beiden antwortet. Das Design, das
Tenant-Attribut auf Hub-Seite (`relayZone`), `losos.edge.uplink.*` auf
Spoke-Seite, die offene LAN-Registrierung und das Gateway-VM-Image stehen
unter [Edge-Föderation](/in-depth/edge-federation.md).

## Offizielle Edges {#official-edges}

Jede Edge kann gefunden werden und Speicher weiterleiten. Nur Edges, die
LosOS betreibt, dürfen den P2P-Handel mit Speicher und Rechenleistung
abwickeln (den [Marktplatz](/in-depth/market.md)). Die Box prüft das selbst, bei jedem
Scan, und die eigene Edge einer Firma bekommt alles außer dem Marktplatz.

Die Prüfung ist eine Ed25519-Identität:

- **Der LosOS-Root-Schlüssel.** Ein Schlüsselpaar. Die öffentliche Hälfte
  ist eine Datei, die jede Box mitbringt, `nix/keys/official-edge-root.pub` in
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
einer Firma aus, einschließlich der, die `nix/demo/edge-lan/` bootet. Eine Edge,
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

## DNS-Zone und eigene Domains {#dns-zone-and-custom-domains}

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

Die Zone ist mit DNSSEC signiert ([weiter unten](#wallets-at-box-names)).
Veröffentliche ihren DS-Eintrag beim selben Registrar, damit Resolver den
Signaturen vertrauen: `keymgr <zone> ds` auf der Edge gibt ihn aus. Solange
der DS fehlt, behandeln Resolver die Zone als unsigniert, und ein Browser hat
auch keinen Grund, dem ECH-Schlüssel unten zu trauen.

:::note[Nur auf einem übernommenen Gateway]
Die Zone braucht `losos.edge.passport.enable`, und nichts wird vergeben,
bevor der Besitzer die Edge übernommen hat, mit Einstellungen, die sein
Schlüssel signiert hat und die bei diesem Start gemessen wurden. Bis dahin
besteht die Zone nur aus SOA, NS und Glue, und `/domains/*` antwortet mit 503.
:::

### Encrypted Client Hello {#encrypted-client-hello}

Jeder Name in der Zone trägt auch einen HTTPS-Eintrag mit dem Encrypted
Client Hello (ECH) Schlüssel der Edge:

```
mattbox IN HTTPS 1 . alpn="h3,h2" ech="AEz+DQBI…"
```

Ein Browser, der den Eintrag findet, verschlüsselt den Teil seines TLS-Hello,
der die Box nennt, und zeigt dem Netz nur den öffentlichen Namen der Edge,
`register.<publicDomain>`, wenn `losos.edge.dns.ech.publicName` keinen anderen
nennt. Wer die Leitung beobachtet, sieht, dass der Browser zur Edge ging,
nicht nach welcher Box er fragte. ECH ist mit der Zone eingeschaltet
(`losos.edge.dns.ech.enable`), und das Modul lehnt eine Zone ohne DNSSEC oder
ohne ECH ab: ein unsignierter Eintrag ließe sich auf dem Weg zum Browser
ändern.

Die Edge erzeugt den Schlüssel beim ersten Start (`losos-ech-key`,
`/var/lib/losos-ech/ech.pem`, lesbar für root und Traefik) und behält ihn,
bis sich der öffentliche Name ändert. `losos-registrar ech-key --key-file …
--public-name …` gibt den Wert aus, den die Einträge tragen. Traefik
entschlüsselt damit. Noch kein veröffentlichter Traefik kann das, deshalb
läuft auf einer Edge mit Zone ein Traefik aus seiner ECH-Änderung
(`nix/traefik-ech.nix`), der kein Dashboard ausliefert.

:::note[Wie das getestet wird]
`nix/tests/edge-dns.nix` (`losos-edge-dns`) liest den Schlüssel aus dem
signierten HTTPS-Eintrag, prüft ihn mit `delv` und öffnet TLS zum Namen einer
Box mit dem ECH-Client von Go. Traefik nimmt ECH an, das mitgeschnittene
Hello trägt nur den öffentlichen Namen, und einen Schlüssel, den Traefik
nicht hat, lehnt es ab.
:::

### Eigene Domains {#custom-domains}

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
aktualisiert und immer dann, wenn der Bereich Netzwerk danach fragt. LosOS
Login liest sie, um dem Cookie seiner Sitzung für jeden dieser Namen eine
Domain zu geben (`modules/identity.nix`). LosOS Cloud und LosOS Git antworten
auf jeden Namen, über den eine Anfrage kam, und brauchen daher keine Liste.

Auch eine Box hinter einer lokalen Edge bekommt ihre Domains geroutet, über
diese lokale Edge, sobald sie sich mit einem Relay-Pass für die lokale Edge
verbürgt hat und dem Mesh dieser Edge beigetreten ist. Der Registrar hält
diese Routen selbst, in `relay-routes.json` neben seiner Registry.
[Edge-Föderation](/in-depth/edge-federation.md#custom-domains-behind-a-local-edge)
enthält die Details und `losos.edge.dns.relayRoutes`.

### Wallets unter Box-Namen {#wallets-at-box-names}

Eine Box verknüpft ihre Wallet mit ihren Namen über `POST /domains/wallet`
(`{address, signature}`, von lososd aus Einstellungen → Netzwerk
weitergereicht). Die Signatur ist das `personal_sign` der Wallet über

```
LosOS box name v1
zone: <zone>
box: <box id>
address: <0x address, lowercase>
```

sodass sie auf einer anderen Edge oder für eine andere Box nichts verknüpft.
Der Registrar ermittelt den Signierer, speichert Adresse und Signatur in
`domains.json` und schreibt unter jeden Namen der Box in der Zone, ihren
Hostnamen und ihr Label, einen TXT-Eintrag:

```
mattbox IN TXT "ENS1 dnsname.ens.eth 0x…"
```

Das ist die gaslose DNS-Auflösung von ENS (ENSIP-17): Eine Wallet fragt ENS
auf Ethereum Mainnet, das DNSSEC-Gateway von ENS holt den TXT-Eintrag, ein
Contract prüft die Signaturen bis zur DNS-Wurzel, und `dnsname.ens.eth`, der
erste DNS-Resolver von ENS, liefert die Adresse. Er liefert `addr`
ABI-kodiert, so wie jede Wallet es dekodiert. Sein Nachfolger,
`2.dnsname.ens.eth`, liest Schlüssel je Chain (`a[e8453]=…`), liefert aber
nackte 20 Bytes, die weder die Standardsuche von viem noch ethers dekodieren,
sodass die Wallet eines Käufers nichts fände. Die Wallet der Box hat auf jedem
EVM-Netz dieselbe Adresse, also genügt die Antwort für Ethereum. `{}` trennt
sie.

Der Aufruf trägt das Token der Box, also kann nur die Box verknüpfen oder
trennen, und lososd tut das nur für einen Administrator (der Abschnitt
`network`). Die Signatur zeigt, dass die Wallet dem Namen dieser Box
zugestimmt hat. Sie hält die Edge, die die Zone signiert, nicht davon ab,
etwas anderes auszuliefern: Der Name ist so vertrauenswürdig wie die Edge.

ENS liest nur eine signierte Zone, also signiert Knot diese
(`losos.edge.dns.dnssec.enable`, standardmäßig an) mit seiner
Standardrichtlinie, ECDSA P-256 mit Schlüsseln in `/var/lib/knot`. Resolver
vertrauen den Signaturen, sobald die Elternzone den DS-Eintrag der Zone
veröffentlicht, den `keymgr <zone> ds` auf der Edge ausgibt. Veröffentliche
ihn neben den NS-Einträgen oben.

Dass Knot antwortet, heißt noch nicht, dass eine Wallet den Namen findet.
Solange eine Wallet verknüpft ist, fragt der Reconciler den
DNS-over-HTTPS-Resolver (`losos.edge.dns.checkUrl`) nach dem TXT-Eintrag bei
jedem Namen der Box, so wie das Gateway von ENS fragen würde, und meldet ihn
je Name in `wallet.published`: `found`, sobald der Eintrag validiert (das
AD-Bit des Resolvers), `unsigned`, solange die Elternzone keinen DS
veröffentlicht, `missing`, solange sie die Zone nicht delegiert, `other`,
solange ein Resolver noch einen älteren Eintrag hält. Ein noch nicht
gefundener Name wird jede halbe Minute neu gesucht, ein gefundener stündlich.
Der Bereich Netzwerk zeigt es neben jedem Namen.

Die Wallet der Box löst Namen über dasselbe Relay auf wie ihre Lesezugriffe
auf die Chain (`/api/market/rpc` mit `network: "ens"`). Die Suche nach einem
DNS-Namen hat einen Schritt außerhalb der Chain (EIP-3668): Der Resolver
bricht mit der URL des Gateways ab, die Seite schickt `ens_ccipRead` mit dem
Absender und den Calldata, und die Edge schickt sie an
`https://dnssec-oracle.ens.domains/`, das einzige Gateway, das sie fragt, und
gibt die Antwort zurück, die der Contract prüft. Die CSP der Admin-Seite lässt
sie weiterhin nur mit ihrer eigenen Box sprechen.

Die eigene Domain eines Besitzers bekommt denselben Eintrag im Bereich
angezeigt, um ihn direkt bei der Domain einzutragen. Solange eine Wallet
verknüpft ist, sucht jede Prüfung auch TXT bei der Domain und meldet
`wallet`: `found` (der Eintrag, wie die Edge ihn veröffentlicht, vom Resolver
per DNSSEC validiert),
`unsigned`, `cname` (ein CNAME trägt kein TXT, und ENS liest nur den exakten
Namen), `other` oder `missing`. Darüber, ob die Domain geroutet wird,
entscheidet das nie.

:::note[Wie das getestet wird]
`nix/tests/edge-dns.nix` (`losos-edge-dns`) bootet eine Edge und einen Client,
fragt Knot über UDP und TCP ab, verknüpft eine Wallet und validiert ihren
signierten Eintrag mit `delv` ab dem DS der Zone.
`nix/tests/box-wallet-name.nix` (`losos-box-wallet-name`) fügt eine Box mit
LosOS Login und einen Laptop hinzu: Der Besitzer verknüpft die Wallet in
Chromium im Bereich Netzwerk, über lososd, und der Eintrag besteht die
Validierung. Ein Ersatz für das öffentliche DNS auf der Edge antwortet dann
mit nichts, einem unsignierten und einem validierten Eintrag, während der
Test die Zone delegiert und ihren DS veröffentlicht, und der Bereich folgt
jedem Schritt. `edge/registrar/tests/domains.rs` spielt den Anspruchs- und den
Wallet-Ablauf gegen einen echten Registrar und einen nachgebildeten
DNS-over-HTTPS-Server durch. `admin-ui/app/tests/ens.node.mjs` spielt über
das Relay die Suche nach einem DNS-Namen ab, aufgezeichnet von Ethereum
Mainnet und dem Gateway von ENS.
:::

## Demo-Betrieb auf Vercel {#demo-deployment-on-vercel}

`edge/vercel/` betreibt die API des Registrars, denselben Router und dieselbe
Token-Prüfung, als Vercel Function unter `https://losos-edge.dasmat.us`, dem
Standardwert von `losos.proxy.registrarUrl`. Die Registry liegt in einer Neon
Postgres, und eine Statusseite unter `/` zeigt die registrierten Boxen und
die Traefik-Konfiguration, die die Edge für sie schreiben würde. Es ist nur
die Control Plane. Der Tunnel, das TLS von Traefik, das Mesh und der
Marktplatz können nicht auf einem Serverless-Host laufen, daher braucht der
Datenpfad, also das Erreichen einer Box über ihren öffentlichen Hostnamen,
weiterhin den VPS. Die Seite listet außerdem IDs und Hostnamen für jeden auf,
der die URL kennt. Sie ist ein Demonstrations-Host, keine Edge. Die Schritte
für das Deployment und die Umgebungsvariablen stehen in
[edge/vercel/README.md](https://github.com/dasmatus/losos/blob/main/edge/vercel/README.md).
