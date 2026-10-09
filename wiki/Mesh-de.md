[English](Mesh) · [Slovenčina](Mesh-sk) · **Deutsch**

# Mesh

Eine Box kann freien Speicherplatz und CPU an andere losos-Boxen verleihen.
Der Beitritt ist standardmäßig aus. Der Bereich Mesh hat zwei Schalter: dem
Mesh beitreten und Rechenleistung teilen. Der dritte Schalter, das Teilen der
Festplatte dieser Box, liegt im Bereich [Market](Market-de), weil Speicher zu
verleihen und dafür bezahlt zu werden eine einzige Entscheidung ist. Solange
der Market nur geplant ist, ist dieser Schalter mit ihm unerreichbar.

## Zwei Cluster

Jede Box betreibt zwei Kubernetes-Instanzen:

|                     | Lokal (k3s)                 | Mesh (rke2-Agent)                |
| ------------------- | --------------------------- | -------------------------------- |
| Führt aus           | Nextcloud und Forgejo dieser Box | Longhorn-Speicher, geteilte Rechenleistung |
| Server              | diese Box                   | der Edge-VPS                     |
| Braucht das Netzwerk | nein                       | ja                               |

Ein Kubernetes-Agent kann nicht starten, solange sein Server nicht erreichbar
ist, und die Box startet jede Nacht neu. Liefen Ihre eigenen Dienste im
Mesh-Cluster, würde ein Edge-Ausfall über Mitternacht sie offline nehmen.
Halten Sie sie getrennt.

## Rechenleistung teilen

Die Box nimmt Mesh-Arbeit nur an, wenn beides zutrifft:

1. Die aktuelle Uhrzeit liegt innerhalb des Fensters, das Sie festgelegt
   haben (`losos.cluster.computeWindow`).
2. Die Box ist im Leerlauf (`losos.cluster.idleLoadThreshold`).

Leerlauf kann das Fenster früher schließen. Außerhalb des Fensters kann er es
nicht öffnen.

Alles Unbekannte gilt als beschäftigt: keine Leerlaufmeldung, eine veraltete
Meldung oder eine Edge, die gerade neu gestartet ist. Die Edge wertet das
Fenster in der Zeitzone der Appliance aus.

## Die Edge finden

Das Mesh sind andere Boxen hinter einer Edge. Eine Box ohne erreichbare Edge
kann nichts teilen, egal was ihre Schalter sagen, deshalb sucht `lososd` alle
20 Sekunden nach einer, an zwei Stellen:

- **im lokalen Netzwerk**, per DNS-SD. Eine Edge, die mit
  `losos.edge.lan.advertise = true` konfiguriert ist, veröffentlicht
  `_losos-edge._tcp` über mDNS mit einem `url=`-Eintrag, der ihre
  Registrar-API nennt, und das Avahi der Box findet sie.
- **unter der konfigurierten Adresse**, `losos.proxy.registrarUrl`, die auf
  einer Box mit Internet die öffentliche Edge ist. Die Box prüft sie, ob der
  Master-Proxy eingeschaltet ist oder nicht.

Ein Kandidat zählt erst, wenn sein `/health` antwortet. Mehrere können
gleichzeitig erreichbar sein, etwa die Edge einer Firma im LAN und die
öffentliche über das Internet. `GET /api/edge` listet sie alle auf, LAN
zuerst, und der Bereich Mesh zeigt eine Zeile pro Edge, oder "No edge proxy
found" (kein Edge-Proxy gefunden) samt dem, was versucht wurde. Teilen ist
erlaubt, solange irgendeine davon antwortet. Die eigene Festplatte einer Box
zu vergrößern braucht überhaupt keine Edge. Speicher über Boxen hinweg zu
bündeln schon, weil die Control Plane des Mesh auf der Edge läuft. In einem
LAN ohne Internet ist diese Edge ein ständig eingeschalteter PC mit dem
Edge-Modul, siehe [Master-Proxy, Edge im selben
LAN](Master-Proxy-de#edge-im-selben-lan).

Solange nichts antwortet, weigert sich die Box, netzwerkabhängiges Teilen
einzuschalten. Der Wechsel in den Mesh-Modus und jedes Apply, das
`losos.sharingMyStorage` oder `losos.cluster.enable` einschaltet, werden mit
409 und dem Grund beantwortet, und die Schalter sind mit demselben Satz
ausgegraut. Bereits eingeschaltete Einstellungen bleiben unberührt, sodass
eine Box, deren Edge verschwunden ist, ihre Konfiguration behält und alles
andere weiterhin ändern kann. Teilen auszuschalten ist immer erlaubt. Ihre
eigenen Dateien und Apps hängen nie von der Edge ab.

Eine Edge zu finden und ihr Geld anzuvertrauen sind zwei verschiedene
Fragen. Jede Edge, die antwortet, öffnet das Teilen. Handel auf dem Market
ist nur über eine **offizielle** Edge erlaubt, eine, die ein vom
LosOS-Root-Schlüssel signiertes Zertifikat vorlegt und damit auf eine frische
Nonce der Box antwortet. Die Box prüft das bei jedem Scan. Die Zeile jeder
Edge trägt neben ihrem Namen ein Zeichen, einen Haken für eine offizielle
Edge und eine Warnung für jede andere. Der Tooltip der Warnung listet auf,
was diese Edge für diese Box nicht tun kann: auf dem Market kaufen sowie den
freien Speicher und die Rechenleistung dieser Box verkaufen. Die eigene Edge
einer Firma trägt die Warnung, und das ist kein Fehler. Siehe [Master-Proxy,
offizielle Edges](Master-Proxy-de#offizielle-edges).

`demo/edge-lan/run.sh` bootet zwei Boxen und eine Edge in einem virtuellen
Netzwerk und geht genau das in der Reihenfolge der Prüfliste durch
(`demo/edge-lan/CHECKLIST.md`). Zuerst ganz ohne Edge: nichts gefunden,
Teilen abgelehnt, lokale Nutzung intakt, ein Neustart ändert nichts. Dann mit
eingeschalteter Edge: auf beiden Boxen innerhalb eines Scans gefunden,
erlaubt, Edge weg, abgelehnt, wieder da. `demo/edge-lan/record.sh` zeichnet
denselben Durchlauf aus der Admin-Oberfläche der Boxen auf.

## Speicher teilen

Beigesteuerter Speicher liegt im Home-Verzeichnis des Benutzers `shared`
unter einer fscrypt-Richtlinie. Der Schlüssel ist im TPM versiegelt. Wenn das
Teilen aus ist, ist der Schlüssel nicht geladen und das Verzeichnis
unlesbar, selbst für root auf der laufenden Box. LUKS schützt die Box, wenn
sie aus ist; fscrypt schützt dieses Verzeichnis, wenn sie läuft.

Beigesteuerte Dateien sind pro Maschine in eigenen Namespaces abgelegt.

## Verkaufen

Speicher und Rechenleistung werden sich über den optionalen
Stripe-Connect-Market auch an andere Boxen verkaufen lassen. Er ist gebaut, aber noch nicht
geöffnet, und die Admin-Oberfläche zeigt seinen Tab ausgegraut als
"soon(TM)". Siehe [Market](Market-de).

## Persistenz

Der Mesh-Agent schreibt beim ersten Beitritt `/etc/rancher/node/password`.
Dieser Pfad muss in der Persistenzliste bleiben, sonst lehnt der Server die
Box nach dem nächsten Neustart ab.
