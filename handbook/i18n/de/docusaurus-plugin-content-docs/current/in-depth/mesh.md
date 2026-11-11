---
title: Mesh
sidebar_position: 3
mdx:
  format: md
---

# Mesh

![Der Bereich Mesh auf einer Box ohne erreichbaren Edge-Proxy.](@site/docs/img/mesh-none-de.png)

Eine Box kann freien Speicherplatz und CPU an andere LosOS-Boxen verleihen.
Der Beitritt ist standardmäßig aus. Der Bereich Mesh hat zwei Schalter: dem
Mesh beitreten und Rechenleistung teilen. Der dritte Schalter, das Teilen der
Festplatte dieser Box, liegt im Bereich [Markt](/in-depth/market.md), weil Speicher zu
verleihen und dafür bezahlt zu werden eine einzige Entscheidung ist. Der
Schalter funktioniert, ob die Edge dieser Box den Marktplatz anbietet oder
nicht.

## Zwei Cluster {#two-clusters}

Jede Box betreibt zwei Kubernetes-Instanzen:

|                     | Lokal (k3s)                 | Mesh (rke2-Agent)                |
| ------------------- | --------------------------- | -------------------------------- |
| Führt aus           | als shared installierte Apps | Longhorn-Speicher, geteilte Rechenleistung |
| Server              | diese Box                   | der Edge-VPS                     |
| Braucht das Netzwerk | nein                       | ja                               |

:::warning[Halte die beiden Cluster getrennt]
Ein Kubernetes-Agent kann nicht starten, solange sein Server nicht erreichbar
ist, und die Box startet jede Nacht neu. Liefen deine eigenen Dienste im
Mesh-Cluster, würde ein Edge-Ausfall über Mitternacht sie offline nehmen.
Halte sie getrennt.
:::

## Rechenleistung teilen {#sharing-compute}

Die Box nimmt Mesh-Arbeit nur an, wenn beides zutrifft:

1. Die aktuelle Uhrzeit liegt innerhalb des Fensters, das du festgelegt
   hast (`losos.cluster.computeWindow`).
2. Die Box ist im Leerlauf (`losos.cluster.idleLoadThreshold`).

Leerlauf kann das Fenster früher schließen. Außerhalb des Fensters kann er es
nicht öffnen.

:::note[Unbekanntes gilt als beschäftigt]
Alles Unbekannte gilt als beschäftigt: keine Leerlaufmeldung, eine veraltete
Meldung oder eine Edge, die gerade neu gestartet ist. Die Edge wertet das
Fenster in der Zeitzone der Appliance aus.
:::

## Die Edge finden {#finding-the-edge}

Das Mesh sind andere Boxen hinter einer Edge. Eine Box ohne erreichbare Edge
kann nichts teilen, egal was ihre Schalter sagen, deshalb sucht `lososd` alle
20 Sekunden nach einer, an zwei Stellen:

- **im lokalen Netzwerk**, per DNS-SD. Eine Edge, die mit
  `losos.edge.lan.advertise = true` konfiguriert ist, veröffentlicht
  `_losos-edge._tcp` über mDNS mit einem `url=`-Eintrag, der ihre
  Registrar-API nennt, und die Box findet sie über systemd-resolved.
- **unter der konfigurierten Adresse**, `losos.proxy.registrarUrl`, die auf
  einer Box mit Internet die öffentliche Edge ist. Die Box prüft sie, ob der
  Master-Proxy eingeschaltet ist oder nicht.

Ein Kandidat zählt erst, wenn sein `/health` antwortet. Mehrere können
gleichzeitig erreichbar sein, etwa die Edge einer Firma im LAN und die
öffentliche über das Internet. `GET /api/edge` listet sie alle auf, LAN
zuerst, und der Bereich Mesh zeigt eine Zeile pro Edge, oder "Kein Edge-Proxy
gefunden" samt dem, was versucht wurde. Teilen ist
erlaubt, solange irgendeine davon antwortet. Die eigene Festplatte einer Box
zu vergrößern braucht überhaupt keine Edge. Speicher über Boxen hinweg zu
bündeln schon, weil die Control Plane des Mesh auf der Edge läuft. In einem
LAN ohne Internet ist diese Edge ein ständig eingeschalteter PC mit dem
Edge-Modul, siehe [Master-Proxy, Edge im selben
LAN](/in-depth/master-proxy.md#edge-on-the-same-lan).

Solange nichts antwortet, weigert sich die Box, netzwerkabhängiges Teilen
einzuschalten. Der Wechsel in den Mesh-Modus und jedes Apply, das
`losos.sharingMyStorage` oder `losos.cluster.enable` einschaltet, werden mit
409 und dem Grund beantwortet, und die Schalter sind mit demselben Satz
ausgegraut. Teilen auszuschalten ist immer erlaubt, und alles andere lässt
sich auch während eines Ausfalls ändern. Deine eigenen Dateien und Apps
hängen nie von der Edge ab.

Eine Box, die ihren Speicher teilt und dann zehn Minuten lang keine Edge
findet, schaltet das Teilen selbst aus, so wie es ihr Schalter täte, und der
Bereich Mesh zeigt **Teilen pausiert**. Sobald eine Edge eine Minute lang
antwortet, schaltet die Box das Teilen wieder ein. Sie nimmt nur ihre eigene
Pause zurück: Schaltest du das Teilen in der Zwischenzeit ein oder aus,
bleibt deine Wahl. `losos.proxy.pauseSharingAfter` legt die Wartezeit in
Sekunden fest, `0` schaltet die Pause ab.

Eine Edge zu finden und ihr Geld anzuvertrauen sind zwei verschiedene
Fragen. Jede Edge, die antwortet, öffnet das Teilen. Handel auf dem Market
ist nur über eine **offizielle** Edge erlaubt, ein Gateway, das die Prüfung
der Box besteht: den Pass, den sein Betreiber signiert hat, ein TPM, das
zeigt, dass ein veröffentlichtes LosOS-Gateway-Image unverändert gestartet
ist, und die Gebührenadressen von LosOS. Die Box prüft das bei jedem Scan.
Die Zeile jeder Edge trägt neben ihrem Namen ein Zeichen, einen Haken für ein
geprüftes Gateway und eine Warnung für jede andere Edge. Der Tooltip der Warnung listet auf,
was diese Edge für diese Box nicht tun kann: auf dem Market kaufen sowie den
freien Speicher und die Rechenleistung dieser Box verkaufen. Die eigene Edge
einer Firma trägt die Warnung, und das ist kein Fehler. Siehe [Master-Proxy,
offizielle Edges](/in-depth/master-proxy.md#official-edges).

:::note[Auf virtuellen Maschinen ausprobieren]
`nix/demo/edge-lan/run.sh` bootet zwei Boxen und eine Edge in einem virtuellen
Netzwerk und geht genau das in der Reihenfolge der Prüfliste durch
(`nix/demo/edge-lan/CHECKLIST.md`). Zuerst ganz ohne Edge: nichts gefunden,
Teilen abgelehnt, lokale Nutzung intakt, ein Neustart ändert nichts. Dann mit
eingeschalteter Edge: auf beiden Boxen innerhalb eines Scans gefunden,
erlaubt, Edge weg, abgelehnt, wieder da. `nix/demo/edge-lan/record.sh` zeichnet
denselben Durchlauf aus der Admin-Oberfläche der Boxen auf.
:::

## Speicher teilen {#sharing-storage}

Beigesteuerter Speicher liegt im Home-Verzeichnis des Benutzers `shared`
unter einer fscrypt-Richtlinie. Der Schlüssel ist im TPM versiegelt. Wenn das
Teilen aus ist, ist der Schlüssel nicht geladen und das Verzeichnis
unlesbar, selbst für root auf der laufenden Box. LUKS schützt die Box, wenn
sie aus ist; fscrypt schützt dieses Verzeichnis, wenn sie läuft.

Beigesteuerte Dateien sind pro Maschine in eigenen Namespaces abgelegt.

Die Edge betreibt Longhorn im Mesh-Cluster, installiert aus einem
festgelegten Chart (`losos.edge.cluster.longhornChart`). Die Datenträger
liegen nur auf den Boxen: Eine Box, die dem Mesh beitritt, bekommt das Label
`node.longhorn.io/create-default-disk=true`, und die Edge, die es nicht hat,
speichert keine Replikate. Speicherbestellungen aus dem Markt beziehen ihre
Volumes aus Longhorns Storage-Class `longhorn`. Der fscrypt-Schlüssel schützt
die Platte des Verleihers, liegt aber auf dessen Box, also kann ein Verleiher
lesen, was in seinem Anteil landet. Backups eines Käufers auf so einem Volume
werden deshalb zuerst auf der Box des Käufers verschlüsselt, mit restic und
dem Wiederherstellungscode des Käufers ([Marktplatz](/in-depth/market.md)). Longhorn auf der Edge sucht
nicht nach Updates, sendet keine Nutzungsdaten, und seine Weboberfläche wird
nicht bereitgestellt.

## Verkaufen {#selling}

Speicher und Rechenleistung lassen sich über den optionalen Marktplatz auch
an andere Boxen verkaufen, in der Seitenleiste unter Mesh, Markt. Käufer
zahlen in USDC, Bitcoin oder Monero, oder per Karte, wo die Edge Karten
annimmt. Eine Box zeigt den Marktplatz nur, wenn eine offizielle LosOS-Edge
erreichbar ist. Siehe [Marktplatz](/in-depth/market.md).

## Persistenz {#persistence}

Der Mesh-Agent schreibt beim ersten Beitritt `/etc/rancher/node/password`.
Dieser Pfad muss in der Persistenzliste bleiben, sonst lehnt der Server die
Box nach dem nächsten Neustart ab.
