[English](Edge-Federation) · [Slovenčina](Edge-Federation-sk) · **Deutsch**

# Edge-Föderation

Jeder darf neben seinen Boxen eine Edge betreiben ([Master-Proxy — Edge im
selben LAN](Master-Proxy-de#edge-on-the-same-lan)). Diese Seite beschreibt,
wie sich eine solche **lokale Edge** an die **offiziellen Edges** anschließt,
damit eine Box dahinter von überall erreichbar ist, wie eine Box wählt,
welche Edge sie nutzt, und wie eine lokale Edge ohne ein zweites
Installations-ISO ausgeliefert wird.

## Die Struktur

Hub and Spoke. Eine offizielle Edge ist ein **Hub**. Eine vom Benutzer
betriebene Edge ist ein **Spoke**: Sie behält alles, was sie heute hat (den
Registrar, den Tunnel-Endpunkt, die Ankündigung im LAN, optional die
Control Plane des Mesh) und bekommt einen **Uplink** zu einem Hub dazu. Zwei
Standorte, jeder hinter seinem eigenen Spoke, erreichen einander über den
Hub; eine Box spricht nie direkt mit dem Spoke eines anderen Standorts.

```
site A                         internet                        site B
box ──rathole──▶ spoke A ──rathole uplink──▶ hub ◀──rathole uplink── spoke B ◀──rathole── box
     announce        │        POST /relay       │       POST /relay        │     announce
                     ▼                          ▼                          ▼
              Traefik (LAN)       Traefik (public TLS, Host(<box>.<zone>))   Traefik (LAN)
```

Drei Ebenen, von denen nur zwei föderiert werden:

| Ebene   | Was                                              | Föderiert?                                                                 |
| ------- | ------------------------------------------------ | -------------------------------------------------------------------------- |
| control | Registrar-API: register, heartbeat, join, market | der Spoke leitet seine Boxen an den Hub weiter (`POST /relay`); der Market nicht |
| data    | rathole-Tunnel + Traefik                         | ein Hop mehr: Hub → Spoke → Box                                            |
| mesh    | rke2-Server + Longhorn                           | **nein**: jede Edge ist ihr eigener Cluster, siehe „Was lokal bleibt“      |

## Hub-Seite: ein Tenant, der weiterleiten darf

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

Warum registrieren sich die Boxen eines Spokes nicht direkt beim Hub? Weil
der Betreiber des Hubs dann jede Box der Welt führen würde, und genau das
soll ein lokales Gateway vermeiden. Eine Tenant-Zeile pro Standort sind die
gesamten Kosten eines Standorts auf der offiziellen Edge.

## Spoke-Seite: der Uplink

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
aufruft, per Trust-on-first-use eingeschrieben — ihr Token und ihr Hostname
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

## Die Box: welche Edge, und wenn keine

`lososd` findet bereits jede erreichbare Edge (im LAN per DNS-SD, die
konfigurierte offizielle per URL) und weigert sich, das Teilen
einzuschalten, wenn keine da ist. Es wählt einen Pfad nach dieser Regel:

1. **zuerst eine lokale Edge** — die erste Ankündigung im LAN, deren
   `/health` antwortet; die Ankündigung trägt jetzt auch
   `rathole=<host:port>`, damit die Box weiß, wohin ihr Tunnel führt;
2. **danach die offizielle Edge** — `losos.proxy.registrarUrl` und
   `losos.proxy.edgeRatholeEndpoint`, wenn das LAN keine hat;
3. **keine** — jede Funktion, die von einer Edge abhängt, ist aus: Die
   Tunnel- und Announce-Units sind gestoppt, die Freigabesperre lehnt ab,
   das Market-Relay lehnt ab.

`GET /api/edge` liefert die Wahl als `path` (`{name, url, rathole, source}`
oder `null`), neben dem `rathole`-Endpunkt jeder Edge. Mit eingeschaltetem
`losos.proxy.enable` schreibt `lososd` den Pfad nach
`/run/losos/edge-path.env` (`LOSOS_EDGE_PATH_URL`, `_RATHOLE`, `_SOURCE`,
`_NOISE_PUB`) und steuert danach die beiden Tunnel-Units: Ein Wechsel des
Pfads startet `losos-rathole-client` und `losos-registrar-announce` neu,
ohne Pfad wird `/run/losos/edge-none` geschrieben und beide werden gestoppt
(beide Units tragen `ConditionPathExists=!/run/losos/edge-none`, sodass
nichts sie neu startet, bis wieder eine Edge da ist). Beide Dateien liegen
auf `/run`: Bis zum ersten Scan nach einem Neustart wählen die Units die
konfigurierte Edge an, wie sie es immer getan haben. Der Noise-Key jeder
Edge wird in einer eigenen Datei gepinnt — der der konfigurierten Edge in
`losos.proxy.noisePublicKeyFile`, der einer LAN-Edge unter
`/var/secrets/losos-edge-pins/<host>_<port>.pub` — durch das eigene
`ExecStartPre` des Clients, sodass ein zweites Gateway nie den Key des
ersten erbt. Der Token der Box ist auf jedem Weg derselbe — der Hub hat ihn
in seiner Whitelist, ein Gateway erfährt ihn beim ersten Kontakt, und
`lososd` erzeugt ihn (und den Bootstrap-Token) beim ersten Start, wenn die
Dateien fehlen — und ihr öffentlicher Name ebenso: `losos.proxy.hostname`
des Besitzers muss ein Label unterhalb der Zone des Standorts sein
(`mattbox.acme.losos.cfd`), damit das Relay ihn annimmt, und das ist auch
der Name, dem Nextcloud vertraut. Eine Box, die auf die offizielle Edge
zurückfällt, behält über ihre direkte Tenant-Zeile denselben Namen. Das
Market-Relay braucht keinen zusätzlichen Schalter: Es hängt davon ab, dass
eine *offizielle* Edge antwortet, und ist keine erreichbar, meldet es
bereits `available: false`.

`tests/edge-federation.nix` betreibt die drei Maschinen (Hub, Gateway, Box)
und geht genau das durch: LAN-Pfad, Einschreibung, Uplink, Rückfall, aus,
wieder da.

## Was lokal bleibt

- **Das Mesh.** Longhorn-Replikation und Mesh-Rechenleistung laufen in einem
  rke2-Cluster, und dieser Cluster ist die Edge, der die Box beigetreten
  ist. Ein Spoke mit `losos.edge.cluster.enable` bündelt die Boxen seines
  eigenen Standorts; der Hub verbindet die Cluster zweier Standorte nicht,
  und nichts hier leitet 9345/6443 über den Uplink weiter.
  Standortübergreifendes Bündeln ist ein späterer Schritt mit eigenem
  Entwurf.
- **Der Market.** Handel lehnt die Box ab, solange keine *offizielle* Edge
  erreichbar ist ([Offizielle Edges](Master-Proxy-de#official-edges)); ein
  Spoke ist nie offiziell, und eine weitergeleitete Box ist kein Tenant des
  Hubs, daher kann kein `/market/*`-Aufruf in ihrem Namen erfolgen. Das
  Relay transportiert Erreichbarkeit per HTTP, sonst nichts.
- **Die Identität.** Ein Spoke leitet `/identity` nicht weiter; eine Box,
  die den Market will, erreicht die offizielle Edge selbst, über ihren
  eigenen Internetzugang.

## Eigene Domains hinter einer lokalen Edge

[Eigene Domains](Master-Proxy-de#custom-domains) funktionieren auch für eine
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

### Die Routentabelle

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
Tabelle, die diese Bindungen ergeben haben, geschrieben, damit Sie sie lesen
können. Der Registrar plant sie aus den Bindungen neu und lädt sie nie. Der
Name der Box in der Zone der Edge, `<label>.<zone>`, geht denselben Weg wie
ihre Domains, sodass auch das CNAME-Ziel sie erreicht.

Die lokale Edge bekommt ihre eigenen Zeilen in der Antwort auf `/relay`
zurück. Das Gateway schreibt sie nach
`/var/lib/losos-registrar/hub-routes.json` und protokolliert jede Änderung.
Diese Datei sagt Ihnen nur, was die offizielle Edge zu Ihnen routet. Nichts
auf der lokalen Edge routet danach.

### Einschalten

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

`backend-registrar/tests/domains.rs` testet den ganzen Pfad gegen echte
Registrare. Eine Box hinter einem Gateway bekommt ihre Domain, eine Box
außerhalb des Mesh bekommt nichts, und drei gefälschte Pässe bekommen
nichts. `tests/edge-dns.nix` bootet die NixOS-Verdrahtung. Es prüft die
Schlüsseldatei, `/relay` mit einem Pass, dass die Bindung in
`relay-routes.json` landet und dass sie nach einem Neustart des Registrars
noch da ist.

## Auslieferung: das Gateway, ohne ISO

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
   `losos-edge ssh on`) und seine Adresse auf tty1 ausgibt. Booten Sie es
   auf Proxmox, libvirt oder VirtualBox mit einer Netzwerkkarte im LAN; vom
   Standard-ISO installierte Boxen finden es innerhalb einer Minute. Dann,
   auf seiner Konsole:

   ```sh
   losos-edge status
   losos-edge uplink set --id acme --token-file /root/acme.token \
     --registrar https://register.losos.cfd --rathole edge.losos.cfd:2333
   losos-edge boxes
   ```

   mit der Tenant-Zeile, die Ihnen der Betreiber des Hubs gegeben hat.
   `losos-edge uplink clear` beendet das Weiterleiten.
2. **Eine bestehende NixOS-Maschine**: Importieren Sie `nixosModules.edge`
   und setzen Sie entweder `losos.edge.gateway.enable = true` (die Form des
   Images) oder schreiben Sie die Optionen oben von Hand, wie es die
   Zwei-VM-Demo und `tests/edge-lan.nix` tun.

Die Hub-Seite für die offiziellen Edges ist eine Tenant-Zeile pro Standort.
Der Vercel-Host hat dieselbe `/relay`-Route und kann daher für eine Demo
einen Spoke *annehmen*, aber da dort kein rathole läuft, braucht der
Datenpfad wie bisher den VPS.
