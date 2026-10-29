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
- `losos.edge.tang.enable` (standardmäßig an) lässt Boxen, die mit
  `--tang-edge` installiert wurden, ihre Festplatte über die Edge öffnen,
  mit einem Tang-Schlüsselpaar pro Box unter `/var/lib/losos-registrar/tang`
  und einer Seite `/tang/lost`, auf der ein Besitzer eine gestohlene Box
  meldet. Die Seite der Box beschreibt [TPM und der
  Festplattenschlüssel](/reference/tpm.md).

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

Eine Box, die TLS selbst beendet (siehe
[Die Edge kann den Verkehr nicht lesen](#the-edge-cannot-read-the-traffic)),
läuft nur über HTTP/2 und HTTP/1.1 auf TCP. HTTP/3 bleibt auf den eigenen
Namen der Edge.

## Wenn eine Box nicht erreichbar ist {#when-a-box-cant-be-reached}

Erreicht Traefik eine Box nicht (sie ist ausgeschaltet, startet neu, ist
offline oder ihr Tunnel ist unterbrochen), bekommt der Besucher statt eines
nackten „Bad Gateway“ eine Seite der Edge, die das erklärt, auf Englisch,
Slowakisch oder Deutsch je nach Sprache des Browsers. Der Statuscode bleibt
502, oder 504, wenn die Box nicht rechtzeitig geantwortet hat.

Die Seite setzt voraus, dass die Edge TLS für den Namen beendet; sie
erscheint also nur bei Namen, für die die Edge das noch tut. Sobald eine Box
TLS selbst beendet, bekommt der Besucher einer ausgeschalteten Box
stattdessen den Verbindungsfehler seines Browsers.

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

## Die Edge kann den Verkehr nicht lesen {#the-edge-cannot-read-the-traffic}

Die Box holt sich für jeden ihrer öffentlichen Namen ein eigenes Zertifikat
von Let's Encrypt und beendet TLS selbst. Die Edge leitet jede Verbindung
nach dem Namen im TLS-Hello weiter und reicht die verschlüsselten Bytes an
die Box durch, ohne sie zu öffnen. Wer die Edge betreibt, oder die Maschine,
auf der sie läuft, kann Anmeldungen, Dateien und Seiten also nicht lesen.
Das ist standardmäßig an (`losos.proxy.tlsPassthrough.enable`).

1. Die Edge leitet `http://<Name>/.well-known/acme-challenge/` auf Port 80
   an jede Box weiter, damit die Box die HTTP-01-Challenge von Let's Encrypt
   durch den Tunnel beantworten kann. Die Box fragt mit `lego` an
   (`losos-public-tls`, jede Stunde und immer, wenn die Edge einen neuen
   Namen nennt) und hält die Zertifikate in
   `/var/lib/losos-public-tls/live/<Name>/`.
2. Der Heartbeat der Box nennt die Namen, für die sie ein gültiges
   Zertifikat hat. Bis das erste da ist, beendet die Edge TLS wie bisher;
   der Wechsel kostet also keine Ausfallzeit.
3. Für diese Namen gibt die Edge ihren eigenen TLS-Router auf und behält
   einen TCP-Router mit `HostSNI` und Passthrough. Ein eigenes Zertifikat
   fordert sie dafür nicht an.
4. Auf der Box liest nginx die ersten Bytes jeder Tunnelverbindung: TLS geht
   an das eigene Zertifikat der Box, einfaches HTTP (die Challenges, eine
   ältere Edge) wie bisher an Port 80. Die Admin-Oberfläche bleibt auf
   beiden Wegen im LAN.

Hat eine Box einmal Passthrough verlangt, gibt ein abgelaufenes Zertifikat
TLS nicht an die Edge zurück. Besucher sehen dann eine Zertifikatswarnung,
bis die Box es erneuert.

Jede Stunde öffnet die Box außerdem jeden ihrer Namen von außen, über das
öffentliche DNS und die Edge, und vergleicht das Zertifikat, das sie
bekommt, mit ihrem eigenen. Die Seite Mesh zeigt das Ergebnis: **Bis zur Box
verschlüsselt**, wenn sie übereinstimmen, **Lesbar**, solange die Edge TLS
beendet, und **Abgefangen**, wenn ein Zertifikat antwortet, das die Box
nicht erstellt hat. Das Letzte heißt, dass etwas zwischen den Besuchern und
der Box ihr TLS beendet, etwa eine Edge, die sich für den Namen ein eigenes
Zertifikat besorgt hat. So ein Zertifikat erscheint auch in den
öffentlichen Certificate-Transparency-Logs.

Was die Edge weiterhin sieht: welchen Namen ein Besucher anfragt, wann und
wie viele Daten fließen. Aus dem Tunnel kennt sie auch die Adresse der Box
zu Hause. Passthrough kostet auf den Namen der Box drei Dinge: die Seite für
eine nicht erreichbare Box, HTTP/3 und Encrypted Client Hello, weil jedes
davon voraussetzt, dass die Edge die Schlüssel hält. Die eigenen Namen der
Edge behalten alle drei.

Eigene Domains, die eine Edge an eine Box leitet, sind genauso abgedeckt:
Die Antwort auf den Heartbeat nennt der Box jeden Namen, der zu ihr führt,
und die Box fordert für jeden ein Zertifikat an. Bei der
[Föderation](edge-federation.md) reicht der Spoke die Liste der Box weiter,
und der Hub leitet `<spoke>.<box>` nach dem Namen an den Spoke, der sie an
die Box weitergibt.

:::note[Wie das getestet wird]
`nix/tests/edge-passthrough.nix` (`losos-edge-passthrough`) startet einen
Test-ACME-Server, eine Edge und eine Box. Solange die Box kein Zertifikat
hat, zeigt ein Mitschnitt auf der Seite der Edge im Tunnel ein Passwort, das
ein Besucher sendet. Nachdem die Box ihr Zertifikat über die Edge bekommen
hat, prüft der Besucher es gegen die Wurzel des ACME-Servers, Traefik hat
für den Namen nur eine Passthrough-Route, und dasselbe Passwort steht
nirgends im Mitschnitt. Zeigt der Name auf eine andere Maschine, springt
die eigene Prüfung der Box auf **Abgefangen**.
:::

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

Jede Edge kann gefunden werden und Speicher weiterleiten. Nur eine Edge, die
die Gateway-Prüfung besteht, darf den P2P-Handel mit Speicher und
Rechenleistung abwickeln (den [Marktplatz](/in-depth/market.md)). Eine solche
Edge, ein offenes Gateway, darf jeder betreiben, solange sie nachweist, was
auf ihr läuft. Die Box prüft das selbst, bei jedem Scan, und eine Edge, die
nicht besteht, bekommt alles außer dem Marktplatz.

Was eine Box von einer Edge verlangt:

- **Einen Pass.** Der Betreiber der Edge signiert mit seinem
  OpenPGP-Schlüssel eine kurze Erklärung: die URL der Edge, den eigenen
  Ed25519-Schlüssel der Edge (auf der Edge erzeugt, die private Hälfte
  verlässt sie nie), wohin sein Anteil an der Gebühr in jedem Netz geht, und
  ein Ablaufdatum. Die Box prüft die Signatur und den Schlüssel
  (Selbstsignaturen, nicht widerrufen, nicht abgelaufen) und zeigt die User-ID
  des Schlüssels als Betreiber der Edge.
- **Die Gebühren.** Die Einstellungen neben dem Pass teilen jeden Verkauf
  2 % für LosOS und 2 % für den Betreiber, und die Adressen von LosOS sind
  genau die, die die Box mitbringt (`losos.proxy.gateways.payouts`).
- **Ein TPM.** Der Endorsement-Schlüssel im TPM der Edge trägt das
  Zertifikat seines Herstellers, und die Box prüft es gegen die Hersteller,
  die sie mitbringt (`losos.proxy.gateways.tpmRoots`, standardmäßig das
  TrustedTpm-Bündel von Microsoft). Die Box erzeugt ein Credential, das nur
  dieses TPM öffnen kann.
- **Ein Quote.** Das TPM signiert PCR 4, 11 und 15 zusammen mit der frischen
  Nonce der Box und dem Schlüssel der Edge. PCR 4 und 11 müssen eines der
  veröffentlichten Gateway-Images zeigen (`losos.proxy.gateways.images`), und
  PCR 15 muss den Hash genau der Einstellungen enthalten, die die Edge
  geschickt hat. Die Edge misst ihre Einstellungen beim Start, bevor der
  Registrar läuft, und PCR 15 lässt sich nicht zurücksetzen, also zeigt das
  Quote die Einstellungen, mit denen dieser Start begann.

Zwei Runden tragen das: `GET /passport?nonce=<hex>` und
`POST /passport/attest`. Die Hälfte der Box ist
`backend/access/src/gateway/check.rs`, die der Edge
`edge/registrar/src/gateway.rs`.

Das Ergebnis ist `official` pro Edge in `GET /api/edge`, mit dem Zustand des
Gateways daneben, und ein Zeichen neben dem Namen jeder Edge im
Mesh-Bereich: ein Häkchen bei einem geprüften Gateway, mit Betreiber und
Image im Tooltip; eine Warnung bei jeder anderen Edge, mit einem Tooltip, der
auflistet, was diese Edge für die Box nicht kann; ein Stoppzeichen bei einer
Edge, die die Box bei einer Manipulation ertappt hat. Eine Box kann mehrere
Edges in Reichweite haben. Teilen funktioniert über jede davon, Handel nur
über eine geprüfte. Solange keine in Reichweite ist, antwortet das
Marktplatz-Relay mit `{available: false, reason: "noOfficialEdge"}` und lehnt
jede Aktion mit 409 `officialEdgeRequired` ab. Nichts verlässt die Box.

Eine Ablehnung gibt es in zwei Arten. Was die Box noch nicht kennt, etwa ein
Image, für das sie keine Messwerte hat, einen TPM-Hersteller, für den sie
keine Wurzel hat, oder eine Gebührenadresse, die von ihren abweicht, wird
angezeigt und beim nächsten Scan erneut geprüft. Ein Beweis für Manipulation,
etwa eine gefälschte Attestierung, Einstellungen, die sich nach dem Start der
Edge geändert haben, oder Firmware, die ein Programm außerhalb des
veröffentlichten Images gestartet hat, schaltet die Edge auf dieser Box für
immer ab, nach ihrer URL, ihrem TPM und dem Schlüssel ihres Betreibers, in
`/var/lib/losos/gateways.json`. Die Box nutzt diese Edge dann für nichts
mehr, auch nicht zum Teilen. Einem geprüften Gateway glaubt die Box zehn
Minuten, dann fragt sie erneut.

Der Betreiber einer Edge übernimmt sie mit `POST /passport/settings`,
signiert mit seinem OpenPGP-Schlüssel (`losos-registrar passport statement`
gibt den Text zum Signieren aus, `passport settings` baut die Datei). Die
Edge schreibt die Datei und startet neu, um sie zu messen. Bis dahin
beantwortet sie `/passport` mit 404 und ist nicht geprüft. Eine Edge ohne
`losos.edge.passport.enable` bietet gar keinen Pass an; so sieht die eigene
Edge einer Firma meist aus, auch die, die `nix/demo/edge-lan/` startet.

Zwei Listen sind in diesem Baum leer: `nix/gateway/images.nix`, bis ein
Gateway-Image als Unified Kernel Image startet, und `nix/gateway/payouts.nix`,
bis die Adressen von LosOS dort stehen. Solange eine davon leer ist, besteht
keine Edge, und der Marktplatz ist auf jeder Box aus. Der Standard schlägt
geschlossen fehl.

Eine Firma, die ihre eigene Edge betreibt, bekommt also eine funktionierende
On-Premises-Installation und kann darüber auch handeln, sobald die Edge das
veröffentlichte Image unter ihrem Pass betreibt und den Anteil von LosOS
zahlt. Wer als Fremder das Image, die Einstellungen oder die
Gebührenadressen ändert, bekommt eine Edge, über die keine Box handelt.

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

Eine Box hält eine offizielle Edge daran fest. Eine Edge, die jede Prüfung
als Gateway besteht, bleibt nur unter einer `https://`-URL vertrauenswürdig,
deren Name einen HTTPS-Eintrag hat, der sich per DNSSEC prüfen lässt und einen
ECH-Schlüssel trägt. lososd fragt bei jeder Suche `delv`, das selbst vom
Schlüssel der DNS-Wurzel aus prüft, und führt eine Edge, die durchfällt, als
abgelehnt (`domainNotHttps`, `domainUnsigned` oder `domainNoEch`), bis sie
besteht. `losos-ctl edge-domain <url>` führt dieselbe Prüfung von Hand aus.
Die URL ist die, die der Pass der Edge nennt, also gehört dieser Name in die
Zone: Eine Edge, deren Zone ihre ganze `publicDomain` ist, hat dort auch
`register.<publicDomain>`. `losos.proxy.gateways.domain` schaltet die Prüfung
ab oder richtet sie auf einen anderen Resolver und Vertrauensanker.

:::note[Wie das getestet wird]
`nix/tests/edge-dns.nix` (`losos-edge-dns`) liest den Schlüssel aus dem
signierten HTTPS-Eintrag, prüft ihn mit `delv` und öffnet TLS zum Namen einer
Box mit dem ECH-Client von Go. Traefik nimmt ECH an, das mitgeschnittene
Hello trägt nur den öffentlichen Namen, und einen Schlüssel, den Traefik
nicht hat, lehnt es ab. `losos-ctl edge-domain` lässt den Namen
einer Box durch, findet keinen Schlüssel unter einem Namen, dessen Fehlen
die Zone belegt, prüft mit einem falschen Vertrauensanker nichts und lehnt
reines HTTP ab.
:::

Eine Box, die TLS selbst beendet, bekommt an ihren Namen keinen
HTTPS-Eintrag, denn nur wer den ECH-Schlüssel hält, kann das Hello
entschlüsseln, und die Edge reicht die Verbindungen dieser Box ungeöffnet
durch. Ihr Name steht dann im TLS-Hello im Klartext, wie vor ECH.

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
DNS-over-HTTPS-Server durch. `admin-ui/app/tests/ens.unit.mjs` spielt über
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
