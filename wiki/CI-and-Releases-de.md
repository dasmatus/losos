[English](CI-and-Releases) · [Slovenčina](CI-and-Releases-sk) · **Deutsch**

# CI und Releases

CI ist `.github/workflows/ci.yml`. Es führt dieselben Befehle aus wie die
devenv-Skripte, direkt ausgeschrieben, weil Jobs über `devenv shell` zu
langsam liefen. Halte beide von Hand im Gleichschritt.

`.github/actions/setup-nix` installiert Nix und schreibt die Substituter nach
`/etc/nix/nix.conf`, bevor der Daemon startet. Nix ignoriert `NIX_CONFIG` von
nicht vertrauenswürdigen Clients, und ein Build, der den Cache verfehlt,
kompiliert aus dem Quellcode, ohne es zu melden.

## Jobs

- clippy und rustfmt (rustfmt wird nur bei Pull Requests geprüft)
- Rust-Tests für beide Crates
- Auswertung des Flakes
- Paket-Builds
- das Installer-ISO, bei jedem Push gebaut und danach von
  `tests/iso-boot.py` unter OVMF und SeaBIOS gebootet. Ein Boot gilt als
  bestanden, wenn der Installer eine DNS-Anfrage nach `github.com` sendet.
  Bei einer Zeitüberschreitung lädt der Job einen Screenshot hoch.
- das Nextcloud-Image, nur wöchentlich und auf Anfrage
- `losos-registrar` für den Entwicklerrechner, ein statischer musl-Build, bei
  jedem Push auf main und bei jedem Tag nach GHCR gepusht und vom Proxy
  ausgeliefert (siehe unten)

## Releases

Setze zuerst `flake/version.nix` auf main auf den neuen Tag, pushe
dann einen `v*`-Tag, oder lege in der Weboberfläche einen
Release-Entwurf mit einem neuen `v*`-Tag an. Die Bootmenüs, die Bootbilder,
os-release und die Konsolenbanner zeigen den Wert dieser Datei, und eine
installierte Box baut aus dem committeten Baum. Der Release-Job:

1. schreibt den Tag in `flake/version.nix` (mit einer Warnung, falls der
   committete Wert ein anderer war) und baut dann das Installer-ISO und das
   Demo-QCOW2,
2. signiert den UEFI-Loader des ISOs an Ort und Stelle mit dem Secret
   `SECURE_BOOT_DB_KEY` (`losos-sign-iso`) und signiert `SHA256SUMS` mit
   demselben Schlüssel. Fehlt das Secret oder schlägt das Signieren fehl,
   wird das Release trotzdem veröffentlicht: Das ISO ist der unsignierte
   Build, der Lauf trägt eine Warnung, und die Release Notes sagen
   *Not signed* (nicht signiert) und warum,
3. hängt das ISO, seine `.sha256`, `SHA256SUMS`, `SHA256SUMS.sig` und das
   Zertifikat (`.pem` und `.cer`) an das GitHub-Release an,
4. pusht beide Medien nach GHCR, da das QCOW2 über dem Limit von GitHub von
   2 GiB pro Asset liegt.

Der `iso`-Job auf main signiert genauso, bevor er bootet. OVMF mit den
Schlüsseln von Microsoft muss das Medium ablehnen, und derselbe OVMF-Speicher
mit dem committeten Zertifikat neben denen von Microsoft muss es booten
(`tests/iso-boot.py --firmware
uefi-sb-ms|uefi-sb`). Ein Pull Request aus einem Fork hat keinen Schlüssel,
bootet also unsigniert und überspringt den Teil mit eingespieltem
Zertifikat, und ebenso ein Lauf, bei dem das Signieren fehlschlug. Der
Schlüssel wird einmalig auf dem Rechner des Eigentümers mit
`provisioning/secure-boot/keygen.sh` erzeugt.

Deine Release Notes bleiben erhalten. Der Download-Abschnitt steht zwischen
zwei Marker-Kommentaren, und ein erneuter Lauf ersetzt nur diesen Abschnitt.

### Medien für einen Tag, der keine hat

Ein Release, dessen Tag-Lauf fehlschlug, kann seine Medien nachträglich
bekommen. Öffne Actions, wähle den Workflow `ci`, dann *Run
workflow* auf `main`, und fülle `release_tags` aus: entweder `missing`,
für jedes veröffentlichte Release ohne Installer-ISO, oder die Tags selbst,
durch Leerzeichen getrennt (`v0.1.7
v0.1.8`). Jeder Tag wird aus seinem eigenen Baum gebaut, genauso wie bei
einem Tag-Push, jeweils zwei gleichzeitig, und seine Medien werden an das
bestehende Release angehängt. Titel und Notes des Releases bleiben, wie sie
sind. Tags aus der Zeit vor dem Edge-Gateway-Image (v0.1.6 und älter) können
die Medien nicht bauen und werden mit einem Hinweis übersprungen.

## Binär-Cache

CI veröffentlicht signierte Nix-Store-Pfade als OCI-Artefakte unter
`ghcr.io/dasmatus/losos/nix-cache`. Ein eigenes Deployment des
[LosOS-Desktop-Proxys](https://github.com/dasmatus/losos-desktop/tree/main/proxy)
liefert sie aus, mit `GHCR_REPOSITORY=dasmatus/losos`.

Einrichtung:

1. `nix key generate-secret --key-name losos-1`
2. Speichere den geheimen Schlüssel als Repository-Secret
   `NIX_CACHE_SIGNING_KEY`.
3. Speichere den öffentlichen Schlüssel (`nix key convert-secret-to-public`)
   als Repository-Variable `NIX_CACHE_PUBLIC_KEY`.
4. Speichere die HTTPS-URL des Proxys als Repository-Variable
   `LOSOS_PROXY_URL`. Das ist die Domain, die am Vercel-Projekt
   `losos-cache-proxy` hängt, `https://proxy.losos.dasmat.us`, und nichts
   anderes. Der frühere Name, `losos-proxy.dasmat.us`, leitet jetzt per 307
   dorthin um. Ein Hostname, der lediglich auf Vercel auflöst, antwortet mit
   `DEPLOYMENT_NOT_FOUND`; nix gibt dann in jedem Job
   `warning: '<url>' does not appear to be a binary cache` aus, baut aus dem
   Quellcode, und der Lauf bleibt grün. `setup-nix` prüft jetzt
   `<url>/nix-cache-info` und annotiert den Lauf, wenn das passiert.
5. Mach das Paket `losos/nix-cache` öffentlich, und `losos/images`
   (siehe unten), sobald der erste Push auf main es angelegt hat. GitHub legt
   Pakete privat an, und der Proxy antwortet dann auf alles darin mit
   `502 token: 403`.
6. Halte `losos.cache.substituters` und `losos.cache.trustedPublicKeys`
   in `modules/options.nix` auf derselben URL und demselben Schlüssel. Ihre
   Standardwerte sind `https://proxy.losos.dasmat.us` und der öffentliche
   Schlüssel `losos-1`, sodass eine unveränderte Appliance und das
   Installationsmedium bereits aus dem Cache ziehen, und
   `tests/invariants.nix` schlägt fehl, wenn einer der beiden Standardwerte
   wegfällt. Den Signierschlüssel zu wechseln heißt, Variable und
   Standardwert gemeinsam zu ändern.

Jeder Job meldet, was er aus dem Cache bekommen hat. `setup-nix` legt einen
`nix`-Shim in den `PATH`, der pro Aufruf die eigenen Zeilen von nix
`copying path '…' from '<url>'` und `building '…'` zählt. Er schreibt eine
Tabelle **Nix binary cache use** in die Step Summary des Jobs, eine Zeile pro
nix-Befehl, der etwas kopiert oder gebaut hat, mit Pfaden aus dem LosOS-Cache,
aus `cache.nixos.org`, von anderswo und auf dem Runner gebaut. Außerdem gibt
er direkt nach dem Befehl eine Zeile `LosOS cache: …` im Job-Log aus. Eine
Zeile mit 0 in der LosOS-Spalte und einem Wert ungleich null bei "built here"
bedeutet, dass die eigenen Pfade dieses Projekts kompiliert wurden, obwohl
ein früherer Lauf sie hätte pushen sollen. Ein Job, der nur auswertet, fügt
keine Zeile hinzu. Der Shim ändert weder die Ausgabe von nix noch seinen
Exit-Status.

Prüfe `<proxy-url>/nix-cache-info` und `nix copy --from <proxy-url>
<store-path>`, bevor du dich darauf verlässt. Ein nicht erreichbarer Cache
ist nicht fatal. Die Appliance wartet 5 s und weicht auf `cache.nixos.org`
und eigenes Bauen aus. Lege den geheimen Schlüssel niemals in den Flake
oder auf eine Appliance.

### Die Kopie auf GitHub Pages

Die GitHub-Pages-Seite, die das Handbuch ausliefert, liefert unter
`https://losos.dasmat.us/proxy` auch eine Kopie des Caches aus, für den
Fall, dass der Proxy auf Vercel nicht antwortet. Pages führt keinen Code
aus, also ist die Kopie ein einfacher Datei-Cache. Bei jedem Push auf main
schreibt der Job `publish-cache` die Pfade, die er veröffentlicht, mit `nix
copy --to file://`, signiert sie mit demselben Schlüssel `losos-1` und lässt
jeden Pfad weg, den cache.nixos.org schon ausliefert. Der Workflow
`handbook` läuft erneut, wenn `ci` auf main fertig ist, und veröffentlicht
die neueste Kopie unter `/proxy`.

Sie hat drei Grenzen:

- Sie enthält nur den neuesten Build von main. Eine Appliance auf einer
  älteren Revision findet ihre Pfade nur auf dem Proxy.
- Eine Pages-Seite darf höchstens 1 GB groß sein, also bleibt die Kopie
  unter 800 MiB an NARs und lässt die größten Pfade zuerst weg. Die Step
  Summary des Jobs nennt, was fehlt. Das Nextcloud-Image, das wöchentlich
  gebaut wird, ist nie darin.
- Ihre `nix-cache-info` sagt `Priority: 45`, nach dem Proxy (30) und
  cache.nixos.org (40), also fragt nix sie erst nach beiden.

`losos.cache.substituters` nennt standardmäßig beide. Die Pages-Quelle des
Repositorys muss "GitHub Actions" sein (Settings, Pages), wie für das
Handbuch.

## Das Werkzeug für den Entwicklerrechner

Die Schlüsselzeremonie ([Master-Proxy, offizielle Edges](Master-Proxy-de#offizielle-edges))
führt `losos-registrar provision` auf dem eigenen Rechner des Operators aus,
der keinen Nix-Store hat. `.#losos-registrar-static` ist dasselbe Crate,
statisch gegen musl gelinkt. Der Job `registrar-and-ui` baut es, und der Job
`publish-tool` pusht es als OCI-Artefakt
`ghcr.io/dasmatus/losos/images:<channel>-x86_64` nach GHCR, eine Schicht pro
Datei, `losos-registrar` und `SHA256SUMS`. Genau in dieser Form liefert die
Route `/updates/<channel>/<arch>/<file>` des Proxys sie aus. Sie wählt die
Schicht, deren Titel der Dateiname ist, und leitet auf den Speicher von GHCR
um, außer bei `SHA256SUMS`, die sie direkt ausliefert. Das sysupdate von
LosOS Desktop liest dieselbe Route. Der Kanal ist `main` für einen Push auf
main, der Tag für ein Release (das auch `stable` weiterschiebt) und ein
bereinigter Branch-Name für einen manuellen Lauf anderswo. Pull Requests
veröffentlichen nie.

```sh
curl -fsSLO "https://proxy.losos.dasmat.us/updates/main/x86_64/{losos-registrar,SHA256SUMS}" && sha256sum -c --ignore-missing SHA256SUMS && chmod +x losos-registrar
```

Antwortet der Proxy nicht, liegen dieselben zwei Dateien des neuesten
main-Builds unter `https://losos.dasmat.us/proxy/updates/main/x86_64/`.

Die Step Summary des Jobs enthält die Artefakt-Referenz, die URL und die
Prüfsumme. `oras pull ghcr.io/dasmatus/losos/images:main-x86_64` holt
dieselben Dateien ohne den Proxy.
