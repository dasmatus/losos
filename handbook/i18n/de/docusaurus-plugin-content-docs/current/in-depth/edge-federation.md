---
title: Edge-Föderation
sidebar_position: 6
mdx:
  format: md
---

# Edge-Föderation

Jeder darf neben seinen Boxen eine Edge betreiben ([Master-Proxy, Edge im
selben LAN](/in-depth/master-proxy.md#edge-on-the-same-lan)). Diese Seite beschreibt,
wie sich eine solche **lokale Edge** an die **offiziellen Edges** anschließt,
damit eine Box dahinter von überall erreichbar ist, wie eine Box wählt,
welche Edge sie nutzt, und wie eine lokale Edge ohne ein zweites
Installations-ISO ausgeliefert wird.

## Die Struktur {#the-shape}

Hub and Spoke. Eine offizielle Edge ist ein **Hub**. Eine vom Benutzer
betriebene Edge ist ein **Spoke**: Sie behält alles, was sie heute hat (den
Registrar, den Tunnel-Endpunkt, die Ankündigung im LAN, optional die
Control Plane des Mesh) und bekommt einen **Uplink** zu einem Hub dazu. Zwei
Standorte, jeder hinter seinem eigenen Spoke, erreichen einander über den
Hub; eine Box spricht nie direkt mit dem Spoke eines anderen Standorts.

![Standort A: Eine Box verbindet sich per rathole mit Spoke A und meldet sich dort an; Spoke A verbindet sich per rathole-Uplink mit dem Hub im Internet und meldet die Box mit POST /relay weiter; Standort B ist spiegelbildlich. Jeder Spoke betreibt Traefik im LAN; der Hub betreibt Traefik mit öffentlichem TLS und routet nach Host(<box>.<zone>).](/img/diagrams/edge-federation-de.svg)

Drei Ebenen, von denen nur zwei föderiert werden:

| Ebene   | Was                                              | Föderiert?                                                                 |
| ------- | ------------------------------------------------ | -------------------------------------------------------------------------- |
| control | Registrar-API: register, heartbeat, join, market | der Spoke leitet seine Boxen an den Hub weiter (`POST /relay`); der Market nicht |
| data    | rathole-Tunnel + Traefik                         | ein Hop mehr: Hub → Spoke → Box                                            |
| mesh    | rke2-Server + Longhorn                           | **nein**: jede Edge ist ihr eigener Cluster, siehe „Was lokal bleibt“      |

## Drei Stufen {#three-tiers}

Jede Verbindung, die eine Edge oder eine Box hält, hat eine Stufe, und Box,
Gateway und Hub melden sie alle:

| Stufe     | Verbindung                                                  | Cluster                      |
| --------- | ----------------------------------------------------------- | ---------------------------- |
| primär    | Edge zu Edge: der Uplink eines Gateways zu einer offiziellen Edge | der **Hauptcluster**   |
| sekundär  | Box zu einem lokalen Gateway ihres eigenen Clusters         | ein **sekundärer Cluster**   |
| tertiär   | Box direkt zu einer offiziellen Edge                        | keiner: der Rückfall         |

Eine Box hält selbst nie eine primäre Verbindung. Ihre eigene sieht sie als
`path.tier` in `GET /api/edge` und auf der Mesh-Seite, das Gateway zeigt
seine Verbindungen in `losos-edge status`, und der Hub führt die Gateways
des Hauptclusters, jedes auf einer primären Verbindung, in
`main-cluster.json` neben seiner Registry (`losos-edge main` auf einem
Gateway gibt die des eigenen Standorts aus).

### Sekundäre Cluster {#secondary-clusters}

Ein Gateway in einem Netzwerk kann ausfallen. Ein **sekundärer Cluster**
besteht aus einem oder mehreren Gateways im selben Netzwerk, die dieselben
Boxen bedienen, damit eine Box erreichbar bleibt, wenn eines davon
wegfällt. Er ist Laufzeitzustand und wird auf den Gateways angelegt:

```sh
# auf dem ersten Gateway
losos-edge cluster create office
# es gibt die Zeile aus, die du auf jedem weiteren Gateway ausführst:
losos-edge cluster join office <secret> http://gwa.local:8443
```

Die Cluster-Datei (`losos.edge.secondary.file`, standardmäßig
`/var/lib/losos-edge/cluster.json`, Modus 0600) enthält den Namen, den
Mitgliedsnamen dieses Gateways, das gemeinsame Geheimnis und die Gateways,
über die es beigetreten ist. Der Registrar liest sie bei jedem Durchlauf
neu, deshalb brauchen `create`, `join` und `losos-edge cluster leave`
keinen Neustart. Die Mitglieder gleichen sich alle
`losos.edge.secondary.interval` über `POST /cluster/secondary/sync` ab,
jede Nachricht mit einem HMAC des Geheimnisses signiert: Jedes Mitglied
erfährt die Boxen, die die anderen eingeschrieben haben, sodass der Token
einer Box auf allen gilt, und wann welches Mitglied welche Box zuletzt
gehört hat. Nur das Mitglied, das eine Box zuletzt gehört hat, leitet sie
an den Hub weiter, sodass die Route des Hubs der Box folgt, ohne zu
springen. `losos-edge cluster show` listet die Mitglieder, die antworten.

`GET /cluster/secondary` liest eine Box von jeder Edge, die sie findet: die
Rolle der Edge, die Stufe der Verbindungen, die sie bedient, ihren Cluster
und Mitgliedsnamen, die Mitglieder, die antworten, und bei einer
offiziellen Edge ihren offiziellen Cluster und ihre Partner. Ein Gateway
ohne Cluster-Datei zählt als eigener Cluster, benannt nach seinem Host.

### Offizielle Cluster und Lastverteilung {#official-clusters-and-load-balancing}

Die offiziellen Edges hinter einem Load Balancer bilden einen
**offiziellen Cluster**. Jede benennt ihn und listet die offiziellen
Edges, die Boxen nutzen dürfen:

```nix
losos.edge.official = {
  cluster = "eu";
  peers = [
    { cluster = "eu"; url = "https://eu-1.losos.cfd"; tunnel = "eu-1.losos.cfd:2333"; }
    { cluster = "eu"; url = "https://eu-2.losos.cfd"; tunnel = "eu-2.losos.cfd:2333"; }
  ];
};
```

Eine Box fügt die gelisteten Partner, die antworten, zu den Edges hinzu,
die sie gefunden hat. Auf einer tertiären Verbindung bleibt sie bei der
offiziellen Edge, die sie nutzt, solange diese antwortet. Sonst nimmt sie
die, der ein Rendezvous-Hash ihrer Appliance-ID das größte Gewicht gibt.
So verteilen sich die Boxen gleichmäßig über den Cluster, und es wechseln
nur die Boxen einer Edge, die weggefallen ist.

### Sperren {#bans}

Eine Sperre nennt einzelne Edges, ganze offizielle Cluster oder beides:

```json
{ "edges": ["https://eu-1.losos.cfd"], "clusters": ["us"] }
```

Eine Box liest sie bei jedem Scan aus `losos.proxy.bansFile`, eine Edge bei
jedem Uplink-Durchlauf aus `losos.edge.bansFile`. Eine fehlende oder
unlesbare Datei sperrt nichts. Eine gesperrte Edge wird nie gewählt. Eine
Box auf einer gesperrten Edge wechselt von selbst zu einer anderen Edge
derselben Stufe, und `GET /api/edge` nennt die verlassene Edge als
`leftBanned`. Ein Gateway, dessen Hub oder dessen offizieller Cluster
gesperrt ist, schreibt seinen Uplink auf einen offiziellen Partner
außerhalb der Sperre um und verwirft den gepinnten Key des alten Hubs.

### Eigene Domains halten eine Box {#custom-domains-hold-a-box}

Eine Box mit einer aktiven eigenen Domain bleibt bei der Edge, die die
Domain bedient. Sie wählt diese Edge weiter an, auch wenn sie weg ist, und
wechselt zu keinem anderen Gateway und keiner anderen Stufe, weil ein
Wechsel die Domain kaputt machen würde. `GET /api/edge` zeigt die Edge als
`held`, und ein Wechsel der Box in einen anderen Cluster
(`POST /api/edge/cluster`) antwortet 409 mit `heldByDomain`. Die einzige
Ausnahme ist eine Sperre: Eine Sperre der gehaltenen Edge oder ihres
offiziellen Clusters verschiebt die Box wie jede andere, und das Halten
zieht mit auf die neue Edge.

## Hub-Seite: ein Tenant, der weiterleiten darf {#hub-side-a-tenant-that-may-relay}

Ein Spoke ist ein gewöhnlicher Tenant des Hubs mit einem zusätzlichen
Attribut, der Zone, unter der er weiterleiten darf:

```nix
# on the hub (an official edge)
losos.edge.tenants.acme = {
  hostname = "acme.losos.cfd";           # the spoke's own name, as any tenant
  tokenFile = "/var/secrets/tenant-acme";
  relayZone = "acme.losos.cfd";          # it may relay <label>.acme.losos.cfd
};
```

Der Spoke sendet in regelmäßigen Abständen `POST /relay {appliance_id:
"acme", token, tenants: [{id, hostname}, …]}`, authentifiziert genau wie
`/register` (gleicher Token, gleiche Prüfung in konstanter Zeit, gleiche
Obergrenze für den Body). Der Hub nimmt eine weitergeleitete Box nur an, wenn
ihre ID ein DNS-Label ist und ihr Hostname **genau ein Label unterhalb der
Zone** liegt (`mattbox.acme.losos.cfd`), und hält die Menge der
weitergeleiteten Boxen gleich der zuletzt gesendeten Liste: Eine Box, die
der Spoke nicht mehr aufführt, ist beim nächsten Reconcile verschwunden, und
ein Spoke, der verstummt, nimmt nach Ablauf der Heartbeat-TTL alle seine
Boxen mit, wie jeder Tenant. Weitergeleitete Einträge stehen in der Registry
des Hubs als `<spoke>.<box>` mit `via: <spoke>`, bekommen einen Port aus
demselben rathole-Bereich, einen Traefik-Router `Host(<hostname>)` und einen
rathole-Dienst, dessen Token der Token des **Spokes** ist: Der Spoke ist die
authentifizierte Partei, der Hub hält nie den Token einer Box. Die Hoheit
über Hostnamen bleibt, wo sie war: Der Betreiber des Hubs hat die Zone
gewählt; der Spoke wählt die Labels darin; alles andere wird pro Box
abgelehnt, und der Spoke protokolliert es.

:::note[Warum Boxen sich nicht beim Hub registrieren]
Warum registrieren sich die Boxen eines Spokes nicht direkt beim Hub? Weil
der Betreiber des Hubs dann jede Box der Welt führen würde, und genau das
soll ein lokales Gateway vermeiden. Eine Tenant-Zeile pro Standort sind die
gesamten Kosten eines Standorts auf der offiziellen Edge.
:::

## Spoke-Seite: der Uplink {#spoke-side-the-uplink}

```nix
# on the local edge
losos.edge = {
  enable = true;
  lan.advertise = true;                     # boxes find it by DNS-SD
  lan.openEnrolment = true;                 # boxes on the LAN enrol themselves
  uplink = {
    enable = true;
    registrarUrl = "https://register.losos.cfd";   # the hub's API
    ratholeEndpoint = "edge.losos.cfd:2333";       # the hub's tunnel port
    id = "acme";                                   # the tenant row above
    tokenFile = "/var/secrets/losos-uplink-token";
    # bootstrapTokenFile is optional: every relayed service carries the
    # uplink token, so rathole's default_token is never consulted.
  };
};
```

Nachdem der Reconciler des Registrars seine eigenen Traefik- und
rathole-Dateien geschrieben hat, leitet er seine **aktiven** Tenants
(registriert, auf der Whitelist oder eingeschrieben, mit Heartbeat) an den
Hub weiter und erzeugt `/etc/rathole/uplink.toml`: eine rathole-*Client*-
Konfiguration mit einem Dienst pro weitergeleiteter Box, `local_addr =
127.0.0.1:<Port dieser Box auf diesem Spoke>`, Token = der Uplink-Token. Eine
zweite rathole-Unit, `losos-rathole-uplink`, führt sie aus (gestartet von
einer Path-Unit, sobald die Datei existiert); rathole lädt die Datei im
laufenden Betrieb neu, wie die des Servers. Der Noise-Public-Key des Hubs
wird beim ersten Kontakt gepinnt (`GET /noise-public-key`, vom Registrar
selbst nach `uplink.noisePublicKeyFile` geschrieben), so wie eine Box den
ihrer Edge pinnt. Eine Anfrage an `https://mattbox.acme.losos.cfd/`
terminiert also TLS auf dem Hub, betritt den rathole-Dienst `acme.mattbox`
des Hubs, durchquert den Uplink zum lokalen Port des Spokes für `mattbox`,
durchquert den Tunnel der Box und landet im Nginx der Box. Zwei Tunnel, ein
Host-Header, keine Änderung an der Box.

**Offene Einschreibung** (Open Enrolment) macht eine Standard-Box mit einem
Gateway nutzbar, für das niemand Tokens bereitgestellt hat: Mit
`lan.openEnrolment` wird eine unbekannte Appliance-ID, die `/register`
aufruft, per Trust-on-first-use eingeschrieben. Ihr Token und ihr Hostname
werden unter `/var/lib/losos-registrar/enrolled/` abgelegt, und jede spätere
Anfrage für diese ID muss damit übereinstimmen. Sie gewährt nur die
Mitgliedschaft im Proxy (nie `cluster` oder `market`), wird auf einer Edge
abgelehnt, die sich nicht in einem LAN ankündigt (ein VPS darf sie nie
setzen), und die Ankündigung sagt es (`enrol=open`). `losos-registrar enrol
list|forget --dir /var/lib/losos-registrar/enrolled` (auf dem Gateway
`losos-edge boxes` und `losos-edge forget <id>`) zeigt eingeschriebene Boxen
an und entfernt sie; eine Box, die noch Heartbeats sendet, schreibt sich mit
dem Token, den sie hält, neu ein, daher ist das Vergessen für eine Box
gedacht, die gegangen ist. Der Hub behält die geschlossene Einschreibung.

## Die Box: welche Edge, und wenn keine {#the-box-which-edge-and-when-none}

`lososd` findet bereits jede erreichbare Edge (im LAN per DNS-SD, die
konfigurierte offizielle per URL) und weigert sich, das Teilen
einzuschalten, wenn keine da ist. Es wählt einen Pfad nach dieser Regel:

1. **gesperrte Edges fallen zuerst heraus** ([Sperren](#bans));
2. **die Edge einer eigenen Domain**, solange die Box eine hat und die Edge
   nicht gesperrt ist ([Eigene Domains halten eine Box](#custom-domains-hold-a-box));
3. **ein Gateway ihres eigenen sekundären Clusters** (sekundär): das
   genutzte, solange es antwortet, sonst das erste nach Mitgliedsnamen.
   Eine Box, die noch in keinem Cluster ist, lässt sich im erreichbaren
   Cluster mit den meisten Gateways nieder (bei Gleichstand nach Namen) und
   behält ihn, sodass sie nur dann den Cluster wechselt, wenn ihr Besitzer
   es auf der Mesh-Seite sagt (`POST /api/edge/cluster`). Ein Gateway, das
   keinen Cluster nennt, wird genutzt, aber nie festgehalten;
4. **eine offizielle Edge** (tertiär), per
   [Lastverteilung](#official-clusters-and-load-balancing), oder die
   konfigurierte Edge (`losos.proxy.registrarUrl` und
   `losos.proxy.edgeRatholeEndpoint`), wenn keine als offiziell
   nachgewiesen ist;
5. **keine**: jede Funktion, die von einer Edge abhängt, ist aus: Die
   Tunnel- und Announce-Units sind gestoppt, die Freigabesperre lehnt ab,
   das Market-Relay lehnt ab.

`GET /api/edge` liefert die Wahl als `path` (`{name, url, rathole, source,
tier}`, mit `cluster` und `member` auf einer sekundären Verbindung und
`officialCluster` auf einer tertiären, oder `null`), neben dem
`rathole`-Endpunkt jeder Edge, den erreichbaren Clustern und dem eigenen
`cluster` der Box. Mit eingeschaltetem
`losos.proxy.enable` schreibt `lososd` den Pfad nach
`/run/losos/edge-path.env` (`LOSOS_EDGE_PATH_URL`, `_RATHOLE`, `_SOURCE`,
`_TIER`, `_CLUSTER`, `_OFFICIAL_CLUSTER`, `_NOISE_PUB`) und steuert danach
die beiden Tunnel-Units: Ein Wechsel des Pfads startet `losos-rathole-client`
und `losos-registrar-announce` neu, ein Wechsel in einen anderen Cluster
zusätzlich `losos-mesh-join` und `rke2-agent`,
ohne Pfad wird `/run/losos/edge-none` geschrieben und beide werden gestoppt
(beide Units tragen `ConditionPathExists=!/run/losos/edge-none`, sodass
nichts sie neu startet, bis wieder eine Edge da ist). Beide Dateien liegen
auf `/run`: Bis zum ersten Scan nach einem Neustart wählen die Units die
konfigurierte Edge an, wie sie es immer getan haben. Das eigene
`ExecStartPre` des Clients pinnt den Noise-Key jeder Edge in einer eigenen
Datei: den der konfigurierten Edge in `losos.proxy.noisePublicKeyFile`, den
jeder anderen Edge (eines LAN-Gateways oder eines offiziellen Partners)
unter `/var/secrets/losos-edge-pins/<host>_<port>.pub`. Eine zweite Edge
erbt deshalb nie den Key der ersten. Der Token der Box ist
auf jedem Weg derselbe. Der Hub hat ihn in seiner Whitelist, ein Gateway
erfährt ihn beim ersten Kontakt, und `lososd` erzeugt ihn (und den
Bootstrap-Token) beim ersten Start, wenn die Dateien fehlen. Ihr öffentlicher
Name ist ebenfalls derselbe: `losos.proxy.hostname`
des Besitzers muss ein Label unterhalb der Zone des Standorts sein
(`mattbox.acme.losos.cfd`), damit das Relay ihn annimmt, und das ist auch
der Name, auf den die Apps der Box antworten. Eine Box, die auf die offizielle Edge
zurückfällt, behält über ihre direkte Tenant-Zeile denselben Namen. Das
Market-Relay braucht keinen zusätzlichen Schalter: Es hängt davon ab, dass
eine *offizielle* Edge antwortet, und ist keine erreichbar, meldet es
bereits `available: false`.

`nix/tests/edge-federation.nix` betreibt die drei Maschinen (Hub, Gateway, Box)
und geht genau das durch: LAN-Pfad, Einschreibung, Uplink, Rückfall, aus,
wieder da. `nix/tests/edge-clusters.nix` betreibt zwei offizielle Edges
eines offiziellen Clusters, zwei Gateways eines sekundären Clusters und
eine Box: den Abgleich im Cluster, die primären Verbindungen, den Wechsel
zwischen den Gateways, den tertiären Rückfall, eine Sperre für die Box und
für ein Gateway und das Halten durch eine eigene Domain.

## Was lokal bleibt {#what-stays-local}

- **Das Mesh.** Longhorn-Replikation und Mesh-Rechenleistung laufen in einem
  rke2-Cluster, und dieser Cluster ist die Edge, der die Box beigetreten
  ist. Ein Spoke mit `losos.edge.cluster.enable` bündelt die Boxen seines
  eigenen Standorts; der Hub verbindet die Cluster zweier Standorte nicht,
  und nichts hier leitet 9345/6443 über den Uplink weiter.
  Standortübergreifendes Bündeln ist ein späterer Schritt mit eigenem
  Entwurf.
- **Der Market.** Handel lehnt die Box ab, solange keine *offizielle* Edge
  erreichbar ist ([Offizielle Edges](/in-depth/master-proxy.md#official-edges)); ein
  Spoke ist nie offiziell, und eine weitergeleitete Box ist kein Tenant des
  Hubs, daher kann kein `/market/*`-Aufruf in ihrem Namen erfolgen. Das
  Relay transportiert Erreichbarkeit per HTTP, sonst nichts.
- **Die Identität.** Ein Spoke leitet `/identity` nicht weiter; eine Box,
  die den Market will, erreicht die offizielle Edge selbst, über ihren
  eigenen Internetzugang.

## Eigene Domains hinter einer lokalen Edge {#custom-domains-behind-a-local-edge}

[Eigene Domains](/in-depth/master-proxy.md#custom-domains) funktionieren auch für eine
Box hinter einer lokalen Edge, unter einer Regel. Nur die offizielle Edge
routet eine Domain. Die lokale Edge bedient nie eine Zone, prüft nie einen
Eintrag und fragt Let's Encrypt nie nach etwas. Sie trägt den Verkehr der
offiziellen Edge zur Box über denselben weitergeleiteten Dienst, der bereits
den eigenen Hostnamen der Box trägt.

Der schwierige Teil ist Vertrauen. Eine lokale Edge ist der Rechner von
irgendjemandem, und ihre `/relay`-Liste sagt „meine Box `mattbox`“. Würde
die offizielle Edge das glauben und die Domains von `mattbox` dorthin
routen, könnte jeder mit einem Gateway seine Box nach der eines anderen
benennen, dessen Domain übernehmen und ein gültiges Zertifikat dafür
bekommen. Also muss die Box sagen, hinter welcher lokalen Edge sie steht,
und die lokale Edge kann das nicht für die Box sagen.

Die Box tut das mit einem **Relay-Pass**:

1. Die Box fragt die offizielle Edge nach ihrer Domain-Übersicht, wie sie es
   ohnehin alle zwei Minuten tut, über ihren eigenen Internetzugang und mit
   ihrem eigenen Token. Die Antwort trägt jetzt `relay_pass`,
   `v1.<hour>.<64 hex>`. Der Hex-Teil ist ein HMAC-SHA256 über die Box-ID und
   die Stunde, unter `/var/lib/losos-registrar/relay-pass.key`. Dieser
   Schlüssel verlässt die offizielle Edge nie, und der Registrar erzeugt ihn
   beim ersten Start.
2. lososd schreibt den Pass nach `/run/losos/relay-pass` (nur root) und
   entfernt ihn aus dem, was die Admin-Seite bekommt.
3. `losos-registrar announce` liest die Datei bei jedem Register und
   Heartbeat und sendet den Pass an die Edge, die die Box gerade nutzt. Eine
   lokale Edge hält ihn im Speicher, und nur, wenn er die richtige Form hat.
4. Der Uplink der lokalen Edge sendet den Pass jeder Box mit der Box in
   `POST /relay`. Die offizielle Edge prüft ihn. Ein gültiger Pass bindet die
   Box an diese lokale Edge.

Ein Pass gilt in der Stunde, in der er ausgestellt wurde, und in den zwei
folgenden. Eine lokale Edge, die die Box verlassen hat, kann das zuletzt
Gesehene höchstens so lange wiederholen, und nie über einen neueren Pass
einer anderen lokalen Edge hinweg, denn eine Bindung wechselt nur zu einem
Pass, der mindestens ebenso neu ist. Die lokale Edge kann ihre eigenen Boxen
auch ohne Pass weiterleiten. Sie bekommen dann nur keine Domains.

Zusätzlich zur Bindung muss die Box **im Mesh** sein. Sie muss dem Cluster
dieser offiziellen Edge mit `/cluster/join` beigetreten sein, was ihr
Rechenfenster unter ihrer eigenen ID festhält. Eine Box, die nur einen
Tunnel hat, bekommt ihren Hostnamen und nichts weiter. Und ist die Box
zusätzlich direkt bei der offiziellen Edge registriert, gewinnt der direkte
Pfad, und die lokale Edge wird nicht genutzt.

### Die Routentabelle {#the-route-table}

Der Registrar führt die Tabelle selbst, genauso wie seine Registry. Es muss
keine Datenbank betrieben werden. Jeder Durchlauf des Reconcilers ermittelt,
welche Routen existieren sollen, und baut daraus die Traefik-Router. Ändert
sich die Tabelle, schreibt der Registrar
`/var/lib/losos-registrar/relay-routes.json` neben `registry.json` neu und
protokolliert jede Route, die er hinzufügt oder entfernt:

```json
{
  "bindings": {
    "mattbox": { "spoke": "acme", "epoch": 493281 }
  },
  "routes": [
    { "domain": "cloud.example.org", "tenant": "mattbox", "spoke": "acme", "service": "acme.mattbox" }
  ]
}
```

`bindings` hält fest, für welche lokale Edge sich jede Box verbürgt hat, und
mit einem Pass aus welcher Stunde. Diesen Teil liest ein Neustart zurück,
sodass die Routen beim ersten Durchlauf des Registrars wieder da sind,
statt darauf zu warten, dass jede lokale Edge `/relay` erneut aufruft. Eine
Bindung, deren Pass abgelaufen ist, wird ignoriert. `routes` ist die
Tabelle, die diese Bindungen ergeben haben, geschrieben, damit du sie lesen
kannst. Der Registrar plant sie aus den Bindungen neu und lädt sie nie. Der
Name der Box in der Zone der Edge, `<label>.<zone>`, geht denselben Weg wie
ihre Domains, sodass auch das CNAME-Ziel sie erreicht.

:::note[Die Kopie auf dem Gateway ist nur zum Lesen]
Die lokale Edge bekommt ihre eigenen Zeilen in der Antwort auf `/relay`
zurück. Das Gateway schreibt sie nach
`/var/lib/losos-registrar/hub-routes.json` und protokolliert jede Änderung.
Diese Datei sagt dir nur, was die offizielle Edge zu dir routet. Nichts
auf der lokalen Edge routet danach.
:::

### Einschalten {#turning-it-on}

Auf der offiziellen Edge:

```nix
losos.edge.dns = {
  enable = true;                 # the zone and custom domains
  relayRoutes.enable = true;     # and the route table
};
```

Sonst läuft nichts dafür. Die Tabellendatei und der Pass-Schlüssel liegen
beide im Statusverzeichnis des Registrars. Auf der lokalen Edge und der Box
ändert sich nichts. Beide übernehmen den Pass ab einem Build mit dieser
Änderung.

`edge/registrar/tests/domains.rs` testet den ganzen Pfad gegen echte
Registrare. Eine Box hinter einem Gateway bekommt ihre Domain, eine Box
außerhalb des Mesh bekommt nichts, und drei gefälschte Pässe bekommen
nichts. `nix/tests/edge-dns.nix` bootet die NixOS-Verdrahtung. Es prüft die
Schlüsseldatei, `/relay` mit einem Pass, dass die Bindung in
`relay-routes.json` landet und dass sie nach einem Neustart des Registrars
noch da ist.

## Auslieferung: das Gateway, ohne ISO {#delivery-the-gateway-without-an-iso}

Zwei Formen, eine Konfiguration:

1. **Ein VM-Image**, `nix build .#losos-disk-edge-qcow2` (`nix run
   .#losos-disk-edge-qcow2-run` bootet es in QEMU), an jedes getaggte
   Release als `losos-edge-gateway-<tag>.qcow2` angehängt. Es ist
   `nixosConfigurations.edge-gateway`: das Edge-Modul mit
   `losos.edge.gateway.enable` (`modules/edge-gateway.nix`), das
   `lan.advertise` und `lan.openEnrolment` setzt, den Uplink zur Laufzeit
   aus `/var/lib/losos-edge/uplink.json` liest, sodass ein Image jedem
   Standort dient, beim ersten Booten seinen eigenen Bootstrap-Token
   erzeugt, root eine Konsole gibt (erstes Passwort `losos`, Änderung beim
   ersten Login erzwungen; sshd installiert, aber gestoppt bis
   `losos-edge ssh on`) und seine Adresse auf tty1 ausgibt. Boote es
   auf Proxmox, libvirt oder VirtualBox mit einer Netzwerkkarte im LAN; vom
   Standard-ISO installierte Boxen finden es innerhalb einer Minute. Dann,
   auf seiner Konsole:

   ```sh
   losos-edge status
   losos-edge uplink set --id acme --token-file /root/acme.token \
     --registrar https://register.losos.cfd --rathole edge.losos.cfd:2333
   losos-edge boxes
   ```

   mit der Tenant-Zeile, die dir der Betreiber des Hubs gegeben hat.
   `losos-edge uplink clear` beendet das Weiterleiten.
2. **Eine bestehende NixOS-Maschine**: Importiere `nixosModules.edge`
   und setze entweder `losos.edge.gateway.enable = true` (die Form des
   Images) oder schreib die Optionen oben von Hand, wie es die
   Zwei-VM-Demo und `nix/tests/edge-lan.nix` tun.

Die Hub-Seite für die offiziellen Edges ist eine Tenant-Zeile pro Standort.
Der Vercel-Host hat dieselbe `/relay`-Route und kann daher für eine Demo
einen Spoke *annehmen*, aber da dort kein rathole läuft, braucht der
Datenpfad wie bisher den VPS.
