---
title: Lab
sidebar_position: 7
mdx:
  format: md
---

# Lab

LosOS Lab zeichnet ein LosOS-Setup als Netzwerkdiagramm und zeigt den
Verkehr, der hindurchläuft. Jede Box liefert es unter `/lab/` aus, in der
Seitenleiste der Administration unter **Labor**. Wie die übrigen
Administrationsseiten antwortet es nur im lokalen Netz.

Es öffnet mit **This box** (diese Box). Das Lab liest die Einstellungen der
Box und die Edges, die sie gefunden hat, über dieselben drei
Administrationsrouten wie die anderen Seiten, und zeichnet die Box daraus:
ihren Router, die erreichbaren Edges, die offizielle Edge, sofern eine
konfiguriert ist, und einen Laptop. Router, Kabel und Laptop sind
angenommen, weil die Box sie nicht sehen kann. Das Lab schreibt nie auf die
Box. Ist niemand angemeldet, sagt es das und öffnet die eingebauten Setups.

## Was es zeigt

- **Logisch und Physisch.** Die logische
  Ansicht zeigt Subnetze, die Edge-Pfade und das Mesh. Die physische
  Ansicht zeigt Racks, Schreibtische und die Kabel dazwischen.
- **Modi Echtzeit und Simulation.** Echtzeit spielt den Verkehr ab, wie er
  geschieht. Es hat eine Uhr, die du beschleunigen kannst (1×, 10×, 60×,
  10 min/s, 1 h/s), und eine Schaltfläche, die zum nächsten Timer springt:
  dem Neustart um 00:07, dem Upgrade um 03:00, der Garbage Collection um
  04:30 sowie Beginn und Ende des Rechenfensters. Simulation hält jedes
  Paket an, bis du Abspielen oder Schritt drückst, und listet jeden Hop mit seinem
  Protokoll auf.
- **Eigene Setups.** Beginne mit Empty canvas (leere Fläche) oder einem
  beliebigen eingebauten Setup, zieh Geräte aus der Leiste und zieh
  Kupfer, Glasfaser oder WLAN zwischen ihren Ports. **Speichern** lädt das
  Setup als `.llf`-Datei herunter (LosOS Lab file, darin JSON), und
  **Öffnen** lädt eine wieder. Das zuletzt geänderte Setup wird außerdem im
  Browser gespeichert und in der Auswahl als "Letzter Aufbau" aufgeführt. Eine
  geöffnete Datei wird mit denselben Werkzeugen wie die Leiste neu
  aufgebaut; alles darin, was die Leiste nicht erzeugen könnte, bleibt
  also weg, und das Lab sagt, wie viele Teile es verworfen hat.
- **Eingebaute Setups.** Zwei Standorte hinter einer offiziellen Edge, eine
  einzelne Box zu Hause, eine Firma mit eigenem Gateway, ein LAN ohne
  Internet und drei Lehrbuchformen: Stern, Bus und vermaschtes Netz. Sie
  sind nur Ausgangspunkte, gebaut mit derselben Leiste.
- **Ein Laptop mit LosOS Desktop.** Er hat den Anmeldebildschirm, die
  Übersicht und den Browser, den du auf jede Box und jede Edge im Diagramm
  richten kannst.
- **Netzwerkgeräte** (Router, Switches, WLAN-Access-Points und ein
  Koax-Bus) laufen mit einem eigenen kleinen System namens Netzgeräte
  Betriebssystem. Seine Konsole kennt `show interfaces`, `show ip route`,
  `show arp` und `show dhcp`.

Das Diagramm folgt denselben Regeln wie die echte Software. Beispiele: Eine
Box ohne erreichbare Edge startet keinen Tunnel; ein Wechsel zum Mesh wird
mit `edgeRequired` abgelehnt; der Markt öffnet nur neben einer offiziellen
Edge (einer, deren Zertifikat der LosOS-Root-Schlüssel signiert hat); ein
LAN-Gateway mit offener Registrierung nimmt unbekannte Boxen bei der ersten
Nutzung auf; und die Administrationsrouten lehnen Loopback ab.

## Die zwei Kopien

| | Auf der Box (`/lab/`) | Gehostet |
| --- | --- | --- |
| Gebaut von | `nix build .#losos-admin-ui` (das Lab ist seine zweite Seite) | `admin-ui/lab/engine/build.sh` in `lab.yml` |
| Konsolen | simuliert, oder echte Gäste unter libvirt, wenn die Box den Helfer ausführt | echte x86_64-Gäste unter libvirt auf deinem Rechner, sonst qemu-wasm |
| Braucht | nichts außer der Box | einen Cross-Origin-isolierten Host (COOP + COEP) |

Das Lab ist Teil der Administrationsoberfläche: `admin-ui/app/lab/index.html`
ist eine zweite Vite-Seite neben den Administrationsseiten, gebaut aus
denselben Komponenten, derselben Palette, denselben Icons und Sprachen, und
wird unter `/lab/` ausgeliefert. Alles, was nicht Zeichnen ist, läuft in
WebAssembly: Modell, Simulator, Uhr, Konsolen und Seiten sind ein Rust-Crate,
`admin-ui/lab/core` (`losos-lab-core`), das nix vor dem eigentlichen Build
der Administrationsoberfläche für wasm32 baut. Die Seite läuft unter einer
eigenen Content Security Policy: der der Administrationsseite plus
`'wasm-unsafe-eval'`, damit der Browser den Kern kompilieren darf.

Die gehostete Kopie ist dieselbe Seite, gebaut mit `VITE_LAB_HOSTED=1`, plus
[qemu-wasm](https://github.com/ktock/qemu-wasm): QEMU nach WebAssembly
kompiliert, sodass jede Konsole ein echter Linux-Gast im Browser-Tab ist.
Boxen und Edges booten ein busybox als Ersatz für LosOS, und die
Netzwerkgeräte booten Netzgeräte Betriebssystem, das nix aus
`admin-ui/lab/engine/gear/netzgeraete.nix` baut. Das Lab verbindet die
Netzwerkkarten der Gäste mit dem Diagramm, sodass DHCP, ARP, Ping und HTTP
zwischen Gästen über die Kabel laufen, die du gezogen hast. Ein
Router-Gast vergibt Leases mit seinem eigenen `udhcpd`. Ein Tab führt
höchstens drei Gäste gleichzeitig aus. Mit einem vierten gingen den Gästen
die Kerne aus und sie blieben hängen, deshalb weigert sich das Lab, einen
weiteren zu booten. qemu-wasm führt die Threads von QEMU als Web Worker mit
gemeinsamem Speicher aus, und Browser erlauben das nur auf einer Seite, die
mit `Cross-Origin-Opener-Policy: same-origin` und
`Cross-Origin-Embedder-Policy: require-corp` ausgeliefert wird.
`vercel.json` setzt beide Header auf Vercel, und `serve.json` tut dasselbe
für `serve` auf deinem eigenen Rechner:

```sh
admin-ui/lab/engine/build.sh /tmp/lab     # docker, nix, node
npx --yes serve@14.2.4 -l 8080 /tmp/lab   # then open http://localhost:8080/lab/
```

`lab.yml` baut die gehostete Kopie für jeden Pull Request, der
`admin-ui/lab/` oder die Seite des Labs in `admin-ui/app/` berührt, und lädt
sie als Artefakt `losos-lab-hosted` hoch. Bei einem Push auf `main`
deployt es die Kopie außerdem auf Vercel, sofern das Repository die Secrets
`VERCEL_TOKEN`, `VERCEL_ORG_ID` und `VERCEL_PROJECT_ID` hat.

## Die GPU-Zeichenfläche

Das Diagramm hat zwei Zeichenflächen. Die Seite beginnt immer mit der
SVG-Fläche, sodass das Setup auf dem Bildschirm steht, sobald der Kern
geladen ist. Dann fragt sie den Browser, was er bietet. Mit WebGPU lädt sie
den WebGPU-Build von `admin-ui/lab/render` (`losos-lab-render`). Ohne WebGPU,
aber mit WebGL2 lädt sie den WebGL2-Build. Ohne beides bleibt sie bei SVG.

Jeder Build ist Bevy 0.19.1, zusammen mit dem Kern des Labs nach
WebAssembly kompiliert. Die Seite überträgt das Setup in den eigenen Kern
des Moduls, indem sie jede Änderung wiederholt, die sie am ersten
vorgenommen hat, und macht nur weiter, wenn beide danach dasselbe Setup
beschreiben. Die Bevy-Fläche nimmt dann den Platz der SVG-Fläche ein und
behält Auswahl, Werkzeug und Kamera.

Die Szene ist 3D. Jeder Raum, jede WLAN-Reichweite, jedes Kabel und jedes
Gerät ist ein eigenes flaches Mesh auf einer Grundebene, und eine
orthografische Kamera blickt senkrecht von oben darauf, sodass beide
Ansichten so aussehen wie in SVG. Namen und Adressen sind Text im
Bildschirmraum über der Szene. Klicks sind Raycasts von der Kamera in die
Szene.

Die Seite kehrt zu SVG zurück und sagt das einmal, wenn das Modul nicht
lädt, wenn innerhalb von 20 Sekunden kein Frame ankommt oder wenn das
GPU-Gerät später verloren geht. Ein verlorenes WebGPU-Gerät merkt sich der
Browser, und der nächste Besuch beginnt mit WebGL2.

Jeder Build umfasst etwa 16 MB WebAssembly, mit brotli 3,4 MB für WebGPU und
3,6 MB für WebGL2. Der Browser lädt nur den, den er verwendet, und zwar
nachdem die SVG-Fläche gezeichnet ist. Die Content Security Policy der
Seite braucht keine Änderung, weil `'wasm-unsafe-eval'` dem Browser bereits
erlaubt, den Kern zu kompilieren.

Um eine Zeichenfläche zu wählen, häng `?canvas=svg`, `?canvas=webgpu`
oder `?canvas=webgl2` an die Adresse an. Der Local-Storage-Schlüssel
`losos-lab-canvas` nimmt dieselben Werte an und behält die Wahl in diesem
Browser. Das Engine-Abzeichen in der oberen Leiste nennt die Zeichenfläche
in seinem Tooltip.

## Gäste unter libvirt

qemu-wasm ist langsam: Es emuliert jede Instruktion ohne KVM, jeder Gast
kostet den Tab etwa 450 MB, und ein Tab führt drei aus. Hat der Rechner, auf
dem das Lab geöffnet ist, libvirt (das, was virt-manager steuert), kann das
Lab seine Gäste stattdessen dort ausführen, unter KVM, wenn die CPU es hat.
Sie booten in wenigen Sekunden, und bis zu acht laufen gleichzeitig.

Ein Gast kann auf drei Arten laufen, in der Reihenfolge der Präferenz:

1. **Der Helfer steuert libvirt über `virsh`.** Der Helfer ist ein
   Unterbefehl des Edge-Registrars, `losos-registrar lab`, und läuft auf
   demselben Rechner wie libvirt. Er braucht dort libvirt, QEMU und den
   Befehl `virsh` sowie die Gast-Images in einem Ordner, den der Helfer
   lesen kann.
2. **Die Seite steuert libvirt selbst, über das Relay des Helfers.** Der
   Helfer bietet außerdem einen WebSocket an, den er Byte für Byte auf den
   eigenen Socket von libvirt kopiert. Die Lab-Seite bringt den
   WebAssembly-Client für libvirt mit (`admin-ui/lab/virt-rpc`, das Paket
   `losos-lab-virt`) und spricht darüber das Protokoll von libvirt; dieser
   Weg braucht also den Helfer und einen laufenden libvirt-Daemon, aber
   weder `virsh` noch eine libvirt-Client-Bibliothek auf dem Rechner. Die
   Seite nutzt ihn, wenn dem Helfer `virsh` fehlt, und für jeden Gast, den
   der erste Weg nicht starten konnte.
3. **qemu-wasm im Tab.** Gar nichts auf dem Rechner, nur die Engine des
   gehosteten Labs. Das ist der langsame Weg und der Fallback für jeden
   Gast, den die ersten beiden nicht starten können.

### Der Helfer und `virsh`

Der Helfer startet jeden Gast als transiente Domain namens `losos-lab-...`
mit `virsh create`. Transient bedeutet, dass libvirt sie nie speichert,
sodass nichts den Helfer überdauert: Ctrl-C oder SIGTERM zerstört jeden Gast,
den er gestartet hat, ein Gast, den eine Minute lang keine Lab-Seite
beobachtet hat, wird ebenfalls zerstört, und beim Start entfernt er jede
`losos-lab-`-Domain, die ein abgeschossener Helfer hinterlassen hat. Ein
Gast hängt an keinem libvirt-Netzwerk und keiner Bridge. Seine serielle
Konsole und seine Netzwerkkarte sind mit dem Helfer auf 127.0.0.1 verbunden,
und der Helfer reicht beide über WebSockets an die Seite weiter. Die Seite
ist weiterhin der Switch, sodass ein libvirt-Gast und ein qemu-wasm-Gast
sich ein Kabel teilen können und DHCP, ARP und Ping zwischen ihnen wie
bisher funktionieren.

So nutzt du ihn neben virt-manager auf deinem eigenen PC:

1. Installiere libvirt und QEMU (auf den meisten Distributionen die
   Pakete, die virt-manager bereits mitgebracht hat) und das Binary
   `losos-registrar` (`nix build .#losos-registrar`, oder das statische, das
   das Runbook in `provisioning/edge-identity/README.md` herunterlädt).
2. Leg die Gast-Images in einen Ordner namens `guest`: `bzImage`,
   `rootfs.bin` und, für Router, Switches und Access Points, `gear.bin`.
   `admin-ui/lab/engine/build.sh` erzeugt alle drei, und das gehostete Lab
   liefert sie unter `/lab/guest/` aus, sodass du sie von dort
   herunterladen kannst.
3. Starte den Helfer im Ordner oberhalb von `guest`:

   ```sh
   losos-registrar lab --origin https://your-lab.example.org
   ```

   Er lauscht auf `127.0.0.1:8095` und verwendet `qemu:///session`, das kein
   root braucht. Gib `--connect qemu:///system` an, damit die Gäste in
   virt-manager neben deinen anderen VMs erscheinen (dein Benutzer muss in der
   Gruppe `libvirt` sein, und der eigene qemu-Benutzer von libvirt muss den
   Image-Ordner lesen können). `--origin` nennt die Adresse der Lab-Seite,
   die du öffnest; eine Kopie, die wie oben unter `localhost:8080`
   ausgeliefert wird, ist auch ohne erlaubt. Die übrigen Flags sind
   `--images DIR`, `--max-guests N` (8), `--memory MiB` (96),
   `--virt-type auto|kvm|qemu`, `--idle 60s`, `--virsh PATH` und
   `--token-file FILE`. Ohne Token-Datei weigert sich der Helfer, auf etwas
   anderem als Loopback zu lauschen, und beantwortet nur Anfragen, die an
   `127.0.0.1` oder `localhost` adressiert sind.
4. Öffne das Lab. Eine Kopie auf `localhost` sucht den Helfer von
   selbst. Eine gehostete Kopie sucht erst, wenn du sie mit `?libvirt` am
   Ende der Adresse öffnest, weil Chrome jeden Besucher einer öffentlichen
   Seite in dem Moment um Zugriff aufs lokale Netz bittet, in dem sie
   `127.0.0.1` berührt. Das Lab merkt sich die Wahl; `?libvirt=0` vergisst
   sie.

Das Abzeichen in der oberen Leiste zeigt dann "KVM via libvirt" (oder "QEMU
via libvirt (no KVM)" auf einem Rechner ohne KVM), und jede Konsole sagt,
was ihren Gast ausführt: libvirt oder "QEMU in this tab". Läuft der Helfer
nicht oder lehnt libvirt einen Gast ab, bootet dieser Gast wie bisher unter
qemu-wasm, und das Lab sagt das einmal. Die eigene Kopie der Box hat kein
qemu-wasm, dort bleibt es also bei der simulierten Konsole.

### Der WebAssembly-Client und das Relay

Eine Webseite kann den Socket von libvirt nicht öffnen: Browser bieten
einer normalen Seite weder rohes TCP noch Unix-Sockets, und libvirt hat
keinen eigenen WebSocket-Listener. Deshalb übernimmt der Helfer das Relay.
Die Seite fordert mit `POST /lab/v1/virt-ticket` ein Ticket an und öffnet
dann den WebSocket `/lab/v1/virt` mit den Subprotokollen `losos-lab` und
`ticket.<ticket>`. Das Ticket gilt einmal, für 30 Sekunden. Der Helfer
verbindet sich mit dem Socket seiner `--connect`-URI und kopiert Bytes in
beide Richtungen, ohne sie zu lesen. Für `qemu:///session` ist dieser Socket
`$XDG_RUNTIME_DIR/libvirt/virtqemud-sock`, oder `libvirt-sock` daneben für
ein monolithisches libvirtd. Für `qemu:///system` ist es
`/run/libvirt/virtqemud-sock`, danach `/run/libvirt/libvirt-sock`. Ein
`?socket=PATH` in der URI benennt den Socket direkt, wie bei `virsh`. Der
Helfer hält höchstens `--max-guests` weitergeleitete Sockets gleichzeitig
offen.

`GET /lab/v1/hello` meldet diesen Weg gesondert, als
`virt: {available, socket}`. `available` ist true, wenn der Socket gerade
jetzt eine Verbindung annimmt, und kann daher true sein, während `virsh`
fehlt. Der Helfer startet keinen Session-Daemon, wie `virsh` es tut; bei
`qemu:///session` muss der Daemon also bereits laufen oder von seinem
systemd-Socket gestartet werden.

Die Lab-Seite nutzt diesen Weg von selbst (`engine/virt-rpc.ts`). Für jeden
Gast fordert sie ein Ticket an, öffnet eine weitergeleitete Verbindung,
legt die Domain pausiert an, öffnet ihre Konsole und setzt sie dann fort,
sodass die Konsole das erste Byte sieht, das der Gast ausgibt. Die Domain
ist dieselbe, die der Helfer für `virsh` geschrieben hätte, nur dass der
serielle Port ein Pseudoterminal ist, das der Client als Konsole der Domain
liest. Das Modul des Clients, etwa 220 KB, wird erst geladen, wenn der erste
Gast auf diesem Weg startet. Der Konsolenkopf zeigt "KVM via libvirt from
WebAssembly" oder "QEMU via libvirt from WebAssembly (no KVM)". Ohne KVM
wird die erste Domain abgelehnt, und die Seite fragt erneut nach einfachem
QEMU.

Zu jedem Ticket gehört auch eine Netzwerkkarte: Der Helfer bindet dieselbe
Art UDP-Tunnel, die er seinen eigenen Gästen gibt, und antwortet mit dessen
zwei Ports, einem Namen für die Domain (`losos-lab-virt-...`) und einem
zweiten Ticket. Die Seite schreibt den Tunnel in die Domain und öffnet die
Karte unter `guests/<key>/nic/0`, sodass diese Gäste wie die anderen an den
Kabeln des Labs hängen und ein Gast des einen Wegs einen des anderen
anpingen kann. Die Karte zählt gegen `--max-guests`; ein voller Helfer gibt
weiterhin das Relay-Ticket aus, aber keine Karte, und der Gast bootet dann
ohne Netzwerk, was sein Konsolenkopf anzeigt.

Der Client legt seine Gäste mit dem autodestroy-Flag von libvirt an, sodass
sie enden, wenn die Verbindung endet: wenn der Tab geschlossen wird und wenn
der Helfer stoppt, weil der Helfer bei Ctrl-C oder SIGTERM jeden
weitergeleiteten Socket schließt. Schaltet die Seite einen Gast aus,
zerstört sie die Domain selbst und gibt die Karte mit `DELETE guests/<key>`
zurück. Eine Karte, die keine Seite beobachtet, wird nach `--idle` beendet,
wie ein Gast des Helfers selbst, und weil das Lab seine Domain so benennt
wie der Helfer, erreichen das Aufräumen und die Leerlaufprüfung des Helfers
auch diese Domain. `admin-ui/lab/virt-rpc/README.md` zeigt den Client für
sich, über seine Demo-Seite.

### Sicherheit

Wer einen weitergeleiteten Socket hält, hat libvirt mit den Rechten des
Helfers. libvirt erkennt die Gegenseite eines Sockets an ihrer User-ID, sieht
also den Helfer und nie die Seite, und auch polkit fragt nach dem Helfer.
Mit `qemu:///system` ist das so gut wie root auf dem Rechner, denn eine Seite
kann eine Domain anlegen, die jede beliebige Datei oder Festplatte des Hosts
einbindet, und das Relay kann eine solche Domain nicht ablehnen, ohne das
Protokoll von libvirt zu parsen. Bevorzuge auf deinem eigenen PC
`qemu:///session`: Dann sind im schlimmsten Fall die Dateien deines eigenen
Benutzers betroffen. Die Gäste des Labs brauchen nichts von der
System-Instanz.

Der Helfer lässt den Relay-Socket deshalb nur mit einem Ticket zu, und ein
Ticket kostet dasselbe wie das Starten eines Gastes: das Bearer-Token, wenn
der Helfer `--token-file` hat, und sonst eine an `127.0.0.1` oder
`localhost` adressierte Anfrage von einer erlaubten Seite. Die
`--origin`-Liste ist hier am wichtigsten. Ein WebSocket unterliegt nicht
CORS, sodass jede im selben Browser geöffnete Website versuchen kann,
`ws://127.0.0.1:8095` zu erreichen, und der Helfer lehnt jede Anfrage ab,
deren Origin weder auf der Liste steht noch die eigene Adresse des Helfers
ist.

### Auf einer Box

`losos.lab.libvirt.enable` (standardmäßig aus) schaltet libvirtd ein und
führt denselben Helfer als Dienst aus, mit `qemu:///system`. lososd leitet
die Anfragen des Labs unter `/api/lab/` mit dem Admin-Schlüssel an ihn
weiter, einschließlich `POST /api/lab/virt-ticket`. Die Konsolen und
Netzwerkkarten der Gäste (`/api/lab/ws/`) und das libvirt-Relay
(`/api/lab/virt`) gehen über nginx direkt zum Helfer, nur aus dem lokalen
Netz, und jedes braucht ein Ticket, das nur dieser Schlüssel bekommen kann.
Das Relay gibt libvirt als Benutzer des Helfers heraus, der in der Gruppe
`libvirtd` ist; es ist auf der Box also root-äquivalent. Der
Admin-Schlüssel kann ohnehin schon das ganze System neu bauen, er gibt also
nichts preis, was der Schlüssel nicht schon hatte. Leg die drei
Images in `/var/lib/losos-lab/images` ab (`losos.lab.libvirt.images`).

## Was nicht echt ist

Die Gäste sind Platzhalter: busybox und ein 4-MiB-Image, nicht NixOS. Die
Edges, der Tunnel, das Mesh und der Markt sind im Rust-Kern nach den obigen
Regeln modelliert; sie sind kein laufender Code. Der Desktop des Laptops ist
ein Bild von LosOS Desktop, nicht der Desktop selbst.

## Änderungen vornehmen

- Die Seite und ihre Teile liegen in `admin-ui/app/src/lab/`.
  `npm run lab:core` baut den Kern nach `src/lab/core-pkg/` und
  `npm run lab:virt` den libvirt-Client nach `src/lab/virt-pkg/` (beide in
  gitignore; sie brauchen cargo mit dem Target `wasm32-unknown-unknown` und
  `wasm-bindgen` 0.2.127), und danach liefert `npm run dev` die Seite unter
  `/lab/` aus. `npm run lab:render` baut die zwei Module der GPU-Fläche auf
  dieselbe Weise nach `src/lab/render-pkg/` und führt `wasm-opt` darauf
  aus, wenn `WASM_OPT` es benennt. Ohne sie baut die Seite nicht, und ihr
  Build dauert etwa 13 Minuten. `tests/lab.browser.mjs` prüft sie unter der
  eigenen Policy des Labs, den Relay-Weg gegen einen gefälschten
  libvirt-Daemon.
- Der Vertrag des Kerns ist `admin-ui/lab/core/README.md`. Seine Tests
  vergleichen jedes eingebaute Setup mit dem Verhalten des
  JavaScript-Labs, das er ersetzt hat.
- Gäste: `admin-ui/app/src/lab/engine/` versucht libvirt über das `virsh`
  des Helfers, dann libvirt über den eigenen Client der Seite und das Relay
  des Helfers, dann qemu-wasm, in dieser Reihenfolge.
