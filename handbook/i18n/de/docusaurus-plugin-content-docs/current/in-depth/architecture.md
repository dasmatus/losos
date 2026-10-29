---
title: Architektur
sidebar_position: 9
mdx:
  format: md
---

# Architektur

![Die Übersicht, die Admin-Seite, die nginx ausliefert und für die lososd antwortet.](@site/docs/img/overview-de.png)

## Systeme im Flake {#systems-in-the-flake}

| Ausgabe                       | Was                                              |
| ----------------------------- | ------------------------------------------------ |
| `nixosConfigurations.iso`     | Live-Installer-ISO (`options`, `disko`, `installer`) |
| `nixosConfigurations.install` | Die installierte Appliance (alle Module)         |
| `nixosModules.edge`           | Die VPS-Seite des [Master-Proxys](/in-depth/master-proxy.md) und des Mesh |

Alle Projektoptionen liegen unter `losos.*` in `modules/options.nix`, mit
Standardwerten in `defaults.nix`. Module lesen `config.losos.*`.

## Impermanence {#impermanence}

`/` ist ein tmpfs. `/persist` ist LUKS-verschlüsseltes ext4. `impermanence`
bindet diese Pfade per Bind-Mount aus `/persist` ein: `/nix`, `/var`,
`/etc/ssh`, `/etc/keys`, `/etc/nixos`, `/etc/rancher`, die beiden
Daten-Home-Verzeichnisse und `machine-id`.

:::warning[Nicht gelisteter Zustand geht beim Neustart verloren]
**Alles, was nicht auf dieser Liste steht, geht beim Neustart verloren.**
Neuen Zustand trägst du in `modules/impermanence.nix` ein.
:::

## Steuerungsebene {#control-plane}

| Komponente     | Rolle |
| -------------- | ---- |
| `lososd`       | Systemd-Daemon als root (Rust). Einziger Schreiber von `/var/lib/losos/state.json`. D-Bus-Dienst `org.losos1` und eine per Token authentifizierte JSON-API auf `127.0.0.1:8082`. |
| `losos-ctl`    | CLI, die jeden Unterbefehl über D-Bus an `lososd` weiterreicht. `losos-ctl install` läuft eigenständig auf der ISO. |
| `losos-admin-ui` | SPA aus React + Vite + Tailwind, ausgeliefert von nginx, das `/api/*` an `lososd` weiterleitet. |

Rebuilds laufen als transiente systemd-Units (`losos-rebuild-<job>`).
`nixos-rebuild switch` startet `lososd` neu, daher hängt sich der Daemon beim
Start wieder an jeden Rebuild, der noch als `building` vermerkt ist.

Den Aufbau der Crates beschreibt
[`backend/README.md`](https://github.com/dasmatus/losos/blob/main/backend/README.md),
das Wire-Format [`backend/schema.json`](https://github.com/dasmatus/losos/blob/main/backend/schema.json).

## Workloads {#workloads}

LosOS Cloud (`modules/cloud.nix`, `backend/src/cloud/`) und LosOS Git
(`modules/git.nix`, `backend/src/forge/`) sind eigene Dienste auf dem Host
und lauschen je auf einem Unix-Socket, den nur nginx öffnen darf. Der eigene
k3s-Cluster der Box betreibt nur die als `shared` installierten Apps,
solange die Box ihre Festplatte mit einem Mesh teilt; seine Pods nutzen
`hostNetwork`. nginx belegt die Webports der Box und routet nach Pfad.
Traefik belegt die Ports der installierten Apps im lokalen Netz
(`losos.apps.ports`) und liest die Route jeder App aus den Labels an ihren
Containern.

## Aufbau des Repositorys {#repository-layout}

```
admin-ui/app/            the admin SPA
admin-ui/tui/            losos, the terminal client (Rust)
admin-ui/lab/            LosOS Diagrams' core and canvas (Rust, WebAssembly)
admin-ui/themes/         LosOS Cloud and LosOS Git: themes on the SPA's tokens.css, logos, names
admin-ui/design-system/  dev-only tokens and React wrapper; not shipped
backend/                 lososd + losos-ctl (Rust)
edge/registrar/          the edge registrar (Rust)
edge/vercel/             the registrar as one Vercel Function, for demos
edge/contracts/          the payment and escrow contracts (Solidity)
modules/                 NixOS modules; options in options.nix
nix/                     packages, images and build helpers
nix/tests/               NixOS VM tests
docs/                    security model, developer notes, design specs and plans
handbook/                this handbook (Docusaurus)
```

:::note[Die komprimierte Fassung dieser Seite]
`.claude/CLAUDE.md` im Repository ist die komprimierte Fassung dieser Seite und
listet die Fallstricke, die lautlos zubeißen.
:::

## Im Detail {#in-depth}

Ein Absatz pro Mechanismus: warum jedes Teil dort sitzt, wo es sitzt, und was
eine gut gemeinte Änderung dort kaputt macht.

**Zwei NixOS-Systeme teilen sich einen Modulsatz** (`flake.nix`).

- `iso` ist eine minimale Live-ISO. Sie enthält das `disko`-Layout, damit
  sie die Zielplatte formatieren und Verschlüsselungsschlüssel einschreiben
  kann, und importiert nur `options.nix`, `disko.nix` und `installer.nix`.
- `install` ist das auf der Platte installierte System. Es importiert alles:
  `options`, `configuration`, `impermanence`, `disko`, `boot`, `services`,
  `cloud`, `containers`, `daemon`, `overrides`, `updates`, `defaults`. Der Flake übergibt `specialArgs.self = self`, damit
  `defaults.nix` an `self.packages.${system}.{losos-ctl,losos-admin-ui}`
  kommt und Steuerungsebene und Admin-UI einbinden kann.

**Zustandslos durch Impermanence** (`impermanence.nix`, `disko.nix`, `boot.nix`).
Das Root-Dateisystem ist ein tmpfs. `/persist` ist LUKS-verschlüsseltes ext4
mit dem Feature `encrypt`, weil fscrypt es braucht und btrfs es nicht bieten
kann. Nur die Verzeichnisse in
`environment.persistence."/persist".directories` überleben einen Neustart:
`/nix`, `/var`, `/etc/ssh`, `/etc/keys`, `/etc/nixos`, `/etc/rancher`, die
beiden Daten-Home-Verzeichnisse und `machine-id`. **Alles Neue, das einen
Neustart überleben muss, gehört auf diese Liste**, sonst verschwindet es beim
nächsten Start ohne Fehlermeldung. `/persist` ist `neededForBoot`, damit die
Bind-Mounts von Impermanence aufgelöst sind, bevor das Sysroot befüllt wird.

Der Installer formatiert das Volume immer unbeaufsichtigt, mit einer
zufälligen Schlüsseldatei, die er unter `/etc/keys/persist-keyfile` erzeugt.
Entsperrt wird standardmäßig per TPM2 (`losos.tpm.enable = true`). Direkt
nach `disko` führt der Installer
`systemd-cryptenroll --tpm2-device=auto --tpm2-pcrs= --unlock-key-file=…` aus,
sodass die initrd kein Geheimnis enthält und die Schlüsseldatei nur innerhalb
von `/persist` als Wiederherstellungs-Slot bleibt, mit dem sich
`losos-ctl grow` authentifiziert. Eine PCR-Bindung gibt es absichtlich nicht.
Die Box aktualisiert sich unbeaufsichtigt und hat keine Shell, von der aus
man eine Aussperrung beheben könnte.

Sieht das Installationsmedium kein `/dev/tpmrm0`, oder mit `losos-ctl install
--no-tpm`, läuft die Box im Schlüsseldatei-Modus. `install-target.nix` setzt
`tpm.enable = false`, und die Schlüsseldatei kommt als
`/crypto_keyfile.bin` in die initrd, auf der unverschlüsselten ESP. Die
beiden crypttab-Formen schließen einander aus: systemd-cryptsetup, das eine
Schlüsseldatei *und* `tpm2-device=` bekommt, liest die Datei als versiegelten
Blob. `nix/tests/invariants.nix` legt den Standardwert fest und prüft, dass der
TPM-Pfad kein initrd-Geheimnis deklariert. `nix/tests/tpm.nix` bootet ihn unter
swtpm von Anfang bis Ende.

Eine entfernte `github:`-Upgrade-URI wertet einen Baum ohne
`install-target.nix` aus. Deshalb bevorzugt `flake.nix` die lebenden
`/etc/nixos/modules/{install-target,overrides}.nix`, wann immer es sie lesen
kann, und beide Rebuild-Pfade (`updates.nix`, `supervisor.rs`) übergeben
dafür `--impure`. Siehe den Absatz zum automatischen Upgrade weiter unten.

**Zwei isolierte Datendomänen, keine Shell** (`configuration.nix`). `notshared`
(uid 1000) ist das Konto des Eigentümers in LosOS Cloud. `shared` (uid 1001) besitzt den Speicher, der
dem Mesh zur Verfügung gestellt wird. Beide Home-Verzeichnisse haben Modus
`700`, jedes mit eigener primärer Gruppe, sodass keiner der beiden Benutzer
die Dateien des anderen lesen kann. `isNormalUser` ohne explizite `group`
steckt beide in `users`, und ein Home mit `750` gibt dieser Gruppe dann r-x.
Das hat die Isolation unbemerkt ausgehebelt, bis `nix/tests/impermanence.nix` es
aufgedeckt hat. Keiner der Benutzer hat ein Passwort, und
`services.openssh.enable = false`. Die einzigen Konfigurationsänderungen, die
auf der laufenden Box erreichbar sind, gehen über die Admin-UI: der
Speicherschalter Local oder Mesh, **join the compute mesh** (dem
Rechen-Mesh beitreten) und **share my compute when I sleep** (meine
Rechenleistung teilen, während ich schlafe).

**Der Daemon lososd, die Fassade losos-ctl und der Admin-Endpunkt**
(`modules/daemon.nix`, `backend/`, `admin-ui/`). Die privilegierte Logik
steckt in `lososd`, einem systemd-Daemon als root in Rust (`backend/`, ein
Crate mit zwei Binaries; zbus für D-Bus, actix-web für HTTP, clap für die
CLI). Der Bus-Listener und der Rebuild-Supervisor laufen auf einer
tokio-Runtime. Der HTTP-Server läuft in einem eigenen Thread unter einem
actix-`System`. Die beiden Runtimes sind absichtlich getrennt.

lososd besitzt `/var/lib/losos/state.json` als einziger Schreiber, mit
atomaren Schreibvorgängen über temporäre Datei und Umbenennen. Auf dem
System-D-Bus exportiert er eine Methode pro Unterbefehl (Bus `org.losos1`,
Pfad `/org/losos1`, Interface `org.losos.Control1`) und stellt eine per
Bearer authentifizierte JSON-HTTP-API auf
`127.0.0.1:${losos.admin.apiPort}` (Standard 8082) bereit. Rebuilds laufen
als transiente `systemd-run`-Units (`losos-rebuild-<job>`). Ein
Überwachungs-Thread fragt die Unit ab und vermerkt Erfolg oder Fehlschlag.
Nach einem Neustart des Daemons hängt er sich wieder an; der kommt vor, weil
`nixos-rebuild switch` lososd mitten im Rebuild selbst neu startet.

Die Arbeit von lososd wandert in Plugins (`backend/plugins/`). Ein Plugin
ist eine WebAssembly-Komponente, die die API-Routen aus ihrem Manifest
beantwortet und von Wasmtime in lososd ausgeführt wird. Sie hat kein WASI:
Dateien, Programme, systemd-Units, HTTP, Sperren und Timer sind Aufrufe an
den Host, und lososd prüft jeden gegen das Manifest des Plugins, sodass ein
Plugin nur die Pfade, Programme und Endpunkte erreicht, die es deklariert.
Anmeldung, die Bereiche in `backend/sections.json` und D-Bus bleiben in
lososd. Jeder Aufruf läuft in einer frischen Instanz mit Speichergrenze und
Frist, sodass ein Plugin, das hängt oder abstürzt, eine 500 kostet und sonst
nichts. lososd lädt Plugins nur aus `losos.backend.plugins`, einem
Verzeichnis im Nix-Store (`.#losos-plugins`), und startet nicht, wenn das
Verzeichnis oder eine Datei darin außerhalb des Stores liegt, beschreibbar
ist, nicht root gehört oder auf einem beschreibbaren Mount liegt. Bisher
sind das Aussehen (Hintergrundbild und Widgets) und die angehefteten
Einträge der Seitenleiste Plugins; `nix/tests/plugins.nix` bootet sie.

`losos-ctl` ist eine schlanke Fassaden-CLI. Sie behält die alten
Unterbefehle und die JSON-Ausgabe bei (`state`, `change --mode`, `status`,
`settings`, `apply` (stdin), `factory-reset`; `--json` bewirkt nichts) und
reicht jeden Aufruf über D-Bus an lososd weiter. `losos-ctl install` bleibt
lokal, weil der ISO-Installer als root läuft und keinen Bus braucht.

Die Admin-UI (`admin-ui/app/`, paketiert als `losos-admin-ui` mit
Bun) ist eine SPA aus React 19, Vite und Tailwind v4 auf echten
Pfaden (`/`, `/apps`, `/storage`, `/mesh`, `/settings/<pane>`). Ihr gebautes
`dist/` **ist** das Document Root des vorderen nginx-vhosts, ausgeliefert mit
`try_files $uri $uri/ /index.html`. Ohne diese Zeile ist jeder Deep Link und
jedes Neuladen ein 404. Der vhost leitet `/api/*` an die Loopback-API von
lososd weiter. Die SPA hat ein Paar reiner JS-Seiten ersetzt, `dashboard/`
und `settings/`. Jeder Verweis darauf, oder auf `/ds/` oder `/common.js`,
ist veraltet.

Die Bausteine der SPA (`admin-ui/app/src/components/ui/`) sind
shadcn/ui-Komponenten auf der Palette der Box. `components.json` richtet die
shadcn-CLI auf `src/styles/index.css`, das die Farbnamen von shadcn auf
`tokens.css` abbildet. `accent` und `muted` sind absichtlich nicht
abgebildet, weil die App beide Namen schon für etwas anderes verwendet.

Aus welcher Variante jede Komponente stammt, entscheidet das `style-src
'self'` der Admin-Seite. In Chromium geprüft: Es erlaubt React-`style`-Props
und jeden anderen Schreibzugriff über CSSOM, verweigert aber
`setAttribute("style")` und jedes `<style>`-Element, das eine Bibliothek zur
Laufzeit erzeugt. sonner wird ohne Styles gerendert, sofern sein
mitgeliefertes `styles.css` nicht ins Bundle importiert wird. Die
Scroll-Sperre von Radix (react-remove-scroll) tut gar nichts, daher ist
nichts Modales aus Radix. Die Sidebar mit Sheet, Tooltip und Faltbereichen
stammt aus der *base*-Registry von shadcn auf Base UI, deren Scroll-Sperre
CSSOM nutzt. Der Switch ist die React-Aria-Variante von shadcn
(`react-aria-components`), die `SwitchRow` als shadcn-Field „Switch with a
description“ in Einstellungszeilen setzt. Nur der Bestätigungs-Toast ist aus
Radix, und er braucht keine Scroll-Sperre. Dialog bleibt beim nativen
`<dialog>`. `tests/app.browser.mjs` liefert den echten CSP-Header aus und
schlägt bei jeder neuen Verletzung fehl, sodass ein Bibliothekswechsel, der
Styles einschleust, einen Check rot werden lässt.

Die Befehlsschicht ist gegen einen Effekt-Trait `Losos` geschrieben, mit
einer echten Implementierung (`io_backend`) und einer im Speicher (`fake`),
sodass die Zustandsmaschine ohne Dateisystem per Unit-Test geprüft wird. Der
Installer wiederholt das Muster mit `Install` und `plan_install`. Sein Plan
ist Daten, also prüft ein Test die Reihenfolge der zerstörerischen Schritte,
ohne irgendetwas zu formatieren. Der Wire-Vertrag ist `backend/schema.json`.
Setze `losos.backend.package = null`, um ohne den Daemon zu laufen.

**Jede Option in einem Bereich, und die Konfiguration in LosOS Git**
(`nix/options-doc.nix`, `modules/config-repo.nix`,
`backend/src/options.rs`, `backend/src/config_repo.rs`,
`admin-ui/app/src/screens/settings/pane-advanced.tsx`, `pane-history.tsx`).
Beim Build werden die `losos.*`-Deklarationen nach
`/etc/losos/options.json` durchlaufen: Name, Editor-Art, Standardwert,
Beschreibung, der laufende Wert sowie die Gefahren- und Nur-Lesen-Flags.
lososd fügt bei `GET /api/options` das aktuelle `overrides.nix` hinzu, und
der Bereich Erweitert zeichnet pro Zeile einen typisierten Editor.

`classify` in der Nix-Datei ist das Tor. Eine Deklaration mit einem Typ, den
es nicht kennt, ist ein `throw`, sodass der Eval-Job rot wird, statt dass
eine leere Zeile im Bereich landet. `tests/advanced.browser.mjs` rendert das
echte Dokument (`nix/tests/admin-ui.nix` übergibt den Check `losos-options-doc`)
und schlägt bei jeder Zeile fehl, die der Bereich nicht zeichnen kann.
`POST /api/apply` prüft jede Zeile gegen das Dokument (`check_body`). Es
lehnt nicht deklarierte Schlüssel, schreibgeschützte (vom Installer
festgelegte Werte, Pakete), falsch typisierte Werte und jedes `${` ab und
nennt den abgelehnten Schlüssel.

Jedes Apply, jeder Moduswechsel und jedes Zurücksetzen ist ein Commit in
`/etc/nixos`, dem Repository, das der Installer angelegt hat. Ein
Abgleich-Thread in lososd fragt alle 30 s ab. Er legt das LosOS-Git-Konto
des Admins an (`notshared`, ein Administrator) sowie ein privates
Repository `losos-config`, mit dem Admin-Token, das LosOS Git beim ersten
Start schreibt (`/var/lib/losos-git/admin-token`). Er pusht, wenn LosOS Git
zurückliegt. Hat ein Klon vorausgepusht, macht er einen Fast-Forward, lässt
das neue `overrides.nix` durch dasselbe Tor und baut neu. Eine divergierte
Historie meldet er und rührt sie nie an. Dahinter steht absichtlich **kein
CI-Runner**.

Der Bereich Erweitert verlinkt die fünfzehn Einstellungen, die den anderen
Bereichen gehören, statt sie ein zweites Mal zu bearbeiten. `OWNED` in
`admin-ui/app/src/lib/option-value.ts` und `OWNED_NAMES` in `lib/api.ts`
sind diese Liste, und `buildOverridesNix` filtert zusätzliche Schlüssel
dagegen, damit die Datei nie einen Schlüssel doppelt enthält. Zwei
Eigenschaften des Parsers sind hier wichtig. `parse_line` in `overrides.rs`
schneidet einen Wert beim ersten `#` *außerhalb* von Anführungszeichen ab;
früher schnitt es auch innerhalb, was das Standard-`upgradeFlakeUri`
verstümmelte. Und eine Liste ist eine Zeile `[ "a" "b" ]`, nur Strings.

**Workloads und die einzige Eingangstür** (`modules/containers.nix`,
`modules/cluster.nix`, `modules/cloud.nix`, `modules/git.nix`).
systemd-nspawn ist weg, und mit ihm die App-Modi: LosOS Cloud und LosOS Git
laufen auf dem Host, jeder unter einem eigenen Benutzer, und der eigene
lokale k3s-Cluster der Box betreibt nur die als `shared` installierten Apps.
nginx belegt die Webports 80 und 443. Er routet nach
Pfad auf dem mDNS-Namen der Appliance (`<hostName>.local/cloud/` und die
Adressen, die die Nextcloud-Apps aufrufen, `/git/`) und liefert die
Admin-SPA auf demselben vhost aus. Apps, die ein Besitzer als `notshared`
installiert, und die eigenen Apps der Box laufen dagegen auf Podman ohne
root mit Podman Compose (`modules/apps.nix`, [App-Format](app-format.md)),
sodass eine Box, die nichts teilt, für sie keinen Cluster braucht. Jede
installierte App öffnet Traefik (`losos-traefik`, unter einem eigenen
Benutzer) auf einem Port aus `losos.apps.ports` im lokalen Netz. Die Routen
liest Traefik aus den Labels, die die Box an die Container der App setzt,
über eine Podman-API von `notshared`, die nur Traefiks Gruppe öffnen darf.
Eine `shared`-App bekommt stattdessen eine Routendatei. Traefiks API und
seine Prometheus-Metriken antworten auf `127.0.0.1:${losos.apps.traefikPort}`
(8089), wo lososds Statistik-Plugin sie einmal pro Minute für
`GET /api/statistics` liest.

**LosOS Git** (`modules/git.nix`, `backend/src/forge/`, das Binary
`losos-git` aus dem Crate von lososd). Der Git-Server der Box, unter einem
eigenen Benutzer. Das Smart HTTP von git (`git-upload-pack` und
`git-receive-pack --stateless-rpc`, Protokoll v2 eingeschlossen) streamt in
beide Richtungen, Konten, Tokens und Repository-Einträge sind eine
SQLite-Datei, und die Repositorys liegen bare unter
`/var/lib/losos-git/repositories/<owner>/<name>.git`. Es lauscht zweimal:
auf `/run/losos-git/http.sock`, den nur nginx öffnen darf und der einzige
Ort ist, an dem es der Person glaubt, die LosOS Login in den
`X-Losos-Cloud-*`-Headern nennt, und auf `127.0.0.1:3000` nur für Tokens,
wo lososd das Konfigurations-Repository führt und die Tokens der Rechner
erstellt. Ein Token wird als sein SHA-256 gespeichert und trägt Scopes.
Issues, Pull Requests, Kommentare, Labels und Anhänge liegen in derselben
Datei und werden unter `/api/v1/repos/…` gelesen.

Eine Box, auf der Forgejo lief, nimmt dessen Einträge mit. Bevor LosOS Git
zum ersten Mal startet, verschiebt `losos-git-forgejo-import` die
Repositorys ohne ihre Hooks, exportiert die Datenbank von Forgejo (aus
Postgres, auf einer nativen Box aus SQLite), behält einen vollständigen
Dump davon, und `losos-git import-forgejo` übernimmt Konten,
Mitarbeitende, Zugriffstokens (die alten Werte funktionieren weiter),
Labels, Issues, Pull Requests, Kommentare, Zuständige und Anhänge. Aus
einer Organisation wird ein Konto mit demselben Namen. Was in LosOS Git
keinen Platz hat (Meilensteine, Releases, Zeitleisteneinträge wie „hat das
geschlossen“), wird im Bericht gezählt und genannt, nie stillschweigend
verworfen. Nichts von Forgejo wird gelöscht, und ein fehlgeschlagener Lauf
ändert nichts und wird beim nächsten Start erneut versucht.

`admin-ui/design-system/` (`tokens.css` und `losos.css`, dazu ein
Wrapper-Paket `react/` für claude.ai/design) ist nur für den
Entwicklerrechner gedacht und wird überhaupt nicht mehr ausgeliefert. Die
ausgelieferte SPA bringt ihre eigene Palette in
`admin-ui/app/src/styles/tokens.css` mit, die `index.css` als Theme-Schlüssel
von Tailwind v4 neu veröffentlicht. Das Designsystem bleibt außerhalb der
Nix-Closure, weil die Quell-*Wurzel* von `losos-admin-ui` `admin-ui/app` ist.
Das ist eine stärkere Garantie als der Namensfilter `design-system`, den sie
ersetzt hat und den eine Umbenennung ausgehebelt hätte.

**Ein Erscheinungsbild für Admin-Seiten, LosOS Cloud und LosOS Git**
(`admin-ui/themes/`). LosOS Cloud und LosOS Git sind Seiten aus dem eigenen
Build der Admin-Oberfläche (`admin-ui/app/cloud/`, `admin-ui/app/git/`) und
nutzen deshalb dieselben Komponenten und dieselbe `tokens.css`. `tokens.css`
ist absichtlich reines CSS: Das Theme von LosOS Login liefert es Byte für
Byte als `losos-tokens.css` aus und bildet seine eigenen Variablen darauf ab,
sodass eine dort geänderte Farbe alle mitzieht.

Besitzer sehen die beiden Apps als **LosOS Cloud** und **LosOS Git**.
Beide bekommen Logos, die aus `admin-ui/themes/brand/`
gezeichnet sind. `marks.py` verpackt `salmon.png`, einen lebenden
Silberlachs, ausgeschnitten aus einem Foto von NOAA Fisheries, in die SVGs
und kopiert ihn in die SPA und das Handbuch. Der Build rendert daraus jedes
PNG und die `.ico`. Am 31. Oktober, nach dem eigenen Datum des Betrachters,
zeigen die Admin-Seiten und das Handbuch stattdessen `plate.png`, einen
Teller Lachs aus Matus' eigenem Foto. `brand/CREDITS.md` nennt die Herkunft
beider Bilder. Der alte Pixel-Art-Fisch wurde wegen urheberrechtlicher
Bedenken entfernt und kommt nicht zurück. Das restliche
Upstream-Branding regelt die Konfiguration (keine Skeleton-Dateien, keine
Hilfe- oder Registrierungslinks, kein „Powered by“).

`admin-ui/themes/default.nix` sind reine Daten, die jeder Aufrufer
importiert. Die
Workload-Pods nutzen `hostNetwork`, da der lokale Cluster kein CNI hat, und
antworten daher auf Loopback. Was das kostet, erklärt die Stolperfalle zum
`lanOnly`-Wächter in `.claude/CLAUDE.md`.

**Der Vercel-Demo-Host** (`edge/vercel/`). Ein drittes Rust-Crate außerhalb
des Flakes, das den Router des Registrars als eine Vercel Function ausführt.
Die Registry ist eine `jsonb`-Zeile in einem Neon-Postgres (tokio-postgres
über rustls), mit `DATABASE_URL` aus der Marketplace-Integration von Vercel,
vor jedem Request geladen und danach per Upsert gespeichert. Es fügt zwei
reine Demo-Routen (`/status`, `/status/traefik`) und eine statische Seite
hinzu. Es existiert für Präsentationen und soll danach gelöscht werden. In
`edge/registrar` brauchte es nur zwei Änderungen. `server::build` liefert
Router und Abgleicher ohne Listener und Timer, und `serve` ruft es auf.
`Registry::{export,import}` ist ein Snapshot mit Altersangaben, damit die
Registry zwischen Prozessen wandern kann, und `import` wendet die
Heartbeat-TTL an, weil dazwischen kein Abgleicher lief.

Tunnel, Traefik, Mesh und Stripe-Gate können dort nicht laufen, daher
antworten auf diesem Host `/cluster/join` und `/market/*` mit 503 und
`/tunnel-pin` mit 404. Die `crates`-Liste in `devenv.nix` und CI linten
und testen es wie die anderen beiden Crates. `nix build` sieht es nie. Sein
`tests/postgres.rs` läuft nur, wenn `LOSOS_TEST_DATABASE_URL` gesetzt ist,
was der Test-Job in CI mit einem Postgres-Dienst bereitstellt. Der Rest
seiner Testsuite läuft auf dem In-Memory-Speicher.

Seine zweite statische Seite, `public/find.html`, ist der Ablauf „find my
box“ (meine Box finden). Die Berechtigung Local Network Access in Chrome
erlaubt ihr, `/setup/state.json` einer Box vom öffentlichen Origin aus zu
lesen. `modules/setup.nix` erlaubt genau die Origins in
`losos.setup.finderOrigins` (eine nginx-Map, geprüft von `nix/tests/setup.nix`),
nie `*`, denn das Dokument ist ein LAN-Inventar.

**Der Options-Namensraum `losos.*`** (`options.nix`). Alle
projektspezifischen Einstellungen liegen unter `options.losos`, mit
Standardwerten in `defaults.nix`: `targetDrive`, `tpm.enable`,
`sharingMyStorage`, `git.enable`, `cloud.*`, `cluster.*`
(Mesh-Beitritt, Rechenfenster), `shared.fscrypt.*`, `upgradeFlakeUri`,
`backend.package`, `admin.{enable,port,apiPort,tokenFile,ui}`, `proxy.*`
(die Appliance-Seite des Master-Proxys) und `edge.*` (die VPS-Seite, genutzt
über die Flake-Ausgabe `nixosModules.edge`). **Module lesen nie ein nacktes
`config.X`; sie lesen `config.losos.X`.** Nur hier werden neue Optionen
hinzugefügt. Älterer Code hat Optionen fälschlich innerhalb von
`config`-Blöcken deklariert.

**Härtung** (`hardening.nix`, nur `install`). `losos.hardening.*` ist aus
KSPP-Bausteinen aufgebaut, statt ein Upstream-Profil zu kapseln, denn es ist
keines mehr zum Kapseln übrig. `profiles/hardened.nix` wurde in 26.05
entfernt, und `linux_hardened` ist jetzt ein `throw`. `hardening.enable`
(Standard true) ist die Schicht, die nichts kostet: Kernel-Parameter,
sysctls, eine Modul-Blacklist, `boot.tmp.useTmpfs`, dbus-broker und
systemd-Sandboxing für `nginx` und `lososd`. mDNS ist systemd-resolved in
der Sandbox seiner eigenen Upstream-Unit (`modules/mdns.nix`). Die Blacklist
blockiert auch ein explizites `modprobe`. Ein einfaches
`boot.blacklistedKernelModules` tut das **nicht**; es verhindert nur das
automatische Laden über Aliase. Vier Opt-in-Flags tragen das, was etwas
kaputt machen kann: `apparmor`, `malloc`, `nosmt`, `usbguard`. Das System
`iso` importiert das Modul nicht, weil das Live-Medium squashfs braucht.
`nix/tests/hardening.nix` prüft beide Hälften, auch dass nichts die
Kernel-Funktionen blockiert, die k3s, rke2, containerd und Longhorn brauchen.

**Online-Vergrößerung** (`grow.rs`, `losos.storage.fillPercent`). Das
Logical Volume endet absichtlich vor dem Ende der Volume Group, damit
`/persist` vergrößert werden kann, ohne die Box zu öffnen. `/persist` *ist*
`/nix`, über Impermanence. `losos-ctl grow` führt lvextend aus, dann
`cryptsetup resize`, dann `resize2fs`, in dieser Reihenfolge, im laufenden
Betrieb. Die Reihenfolge ist das ganze Korrektheitsargument, und ein Fehler
darin scheitert lautlos. Deshalb prüft `grow.rs` sie gegen den *Plan*, und
`nix/tests/resize.nix` prüft sie von Anfang bis Ende.

**BIOS und das tty1-Banner** (`losos.bios`, `modules/console.nix`). Das
Firmware-Menü der Installer-ISO schreibt `losos.bios` in
`install-target.nix`. BIOS und UEFI lassen sich explizit wählen. Die
Autoerkennung wählt BIOS, wenn `/sys/firmware/efi` fehlt. `--bios` und
`--uefi` wählen ohne Menü. Das Menü gibt es nur auf der Installer-ISO, nicht
im installierten System, und es nimmt nach 30 s ohne Antwort die
Autoerkennung, weil das Medium unbeaufsichtigt installieren soll und
`nix/tests/iso-boot.py` nie tippt. Auf dem Medium ist der Installer ein Dienst
auf tty1 statt einer Login-Shell (`installer.nix`). Wo Plymouth zeichnet,
läuft er unter dem Startbildschirm. Menü, Schritte und das Ergebnis kommen
auf das Panel (`backend/src/install_screen.rs`), und die Tasten kommen von
`plymouth watch-keystroke`, weil Plymouth beim Zeichnen die Tastatur hält.
Ein Startbildschirm im Textmodus wird beendet, und der Installer läuft als
Text. Nach der Installation verschwindet der Startbildschirm und gibt die
Ausgabe auf tty1 und eine Root-Shell frei. `nix/tests/installer-screen.nix`
bootet beide Varianten. BIOS stellt `boot.nix` auf GRUB um und fügt
der ersten Platte in `disko.nix` eine EF02-Partition hinzu. Die ESP bleibt
in beiden Fällen `/boot`. `console.nix` ersetzt getty auf tty1 durch einen
Banner-Dienst, der die IPv4-Adresse im LAN und `<hostName>.local` anzeigt.
`splash.nix` (`losos.splash.enable`, standardmäßig an) ist Plymouth mit
einem LosOS-Theme (`modules/splash/losos.script`). Ein Start beginnt mit
der Animation des Logos: Ein Umriss zieht sich von den Bildschirmrändern
auf eine Leiste unter dem Namen zusammen, der Lachs schwimmt waagerecht
herein und dreht sich neben „LosOS“ nach oben, und die Schraffur der Leiste
füllt sich dann mit dem Fortschritt des Starts. Sobald der Banner-Dienst
dieselben Zeilen schickt, gleitet das Logo nach oben und darunter öffnet
sich ein Feld. Am Ende des
Starts beendet nichts Plymouth, das Feld bleibt also stehen. Darunter
zeichnet der Banner weiter auf tty1; das ist zu sehen, wo Plymouth in den
ersten Sekunden keine Anzeige gefunden hat und im Textmodus läuft. Unter
BIOS behält der Kernel den Grafikmodus von GRUB (`gfxpayloadBios =
"keep"`), damit Plymouth schon in Stage 1 einen Framebuffer hat.
`startup-sound.nix` (`losos.startupSound.enable`, standardmäßig an) spielt
die letzten sieben Sekunden der Musik aus dem Launch-Film, sobald der
Banner-Dienst läuft, auf jedem Ausgabegerät, das ALSA findet.
`modules/startup-sound/render.py` synthetisiert ihn beim Bauen, das
Repository enthält also keine Aufnahme. Der nächtliche Neustart und der
Neustart nach einem Update bleiben still: beim Herunterfahren für einen
Neustart bleibt eine Markierung in `/var/lib/losos-startup-sound` zurück, und
der nächste Start entfernt sie, statt zu spielen.

**Protokollgrenzen** (`logs.nix`, auf der Box und auf einem Edge). `/var`
liegt auf demselben Volume wie `/nix`, ein Protokoll ohne Ende endet also als
nächtlicher Neubau ohne Platz zum Bauen. Das Journal ist auf
`losos.logs.journalMaxUse` (512 MB) und einen Monat begrenzt. Das Kubelet
jedes Clusters behält drei Dateien zu 10 MiB pro Container
(`containerLogMaxSize`, `containerLogMaxFiles`). Das deckt LosOS Cloud,
LosOS Git und die Apps ab, weil sie auf stdout protokollieren. logrotate
rotiert `audit.log` und `rebuild.log` von lososd bei 10 MB und ergänzt die
nginx-Regel, die nixpkgs mitbringt, um eine Grenze von 20 MB. Eine
tmpfiles-Regel löscht Nix-Build-Protokolle, die älter als 30 Tage sind und
die die Garbage Collection liegen lässt. rke2 und libvirt rotieren ihre
Protokolldateien selbst, und auf einem Edge schreibt `bitcoind` nur ins
Journal. `nix/tests/invariants.nix` prüft die Journalgrenze, beide
logrotate-Regeln und die Grenze der Pod-Protokolle in beiden Clustern.

**Automatisches Upgrade und der nächtliche Neustart** (`updates.nix`).
`system.autoUpgrade` baut um 03:00 aus `losos.upgradeFlakeUri` neu,
standardmäßig `git+file:///etc/nixos#install`. Vor dem Bau führt
`nixos-upgrade.service` `losos-ctl config --update` aus, und lososd bringt
`/etc/nixos` auf den neuesten Commit von `losos.configRepo.upstream.branch`
unter `losos.configRepo.upstream.url` (standardmäßig `main` des
GitHub-Repositorys). Der Fetch ist flach, und das Ergebnis ist ein Commit
auf dem eigenen Zweig der Box, kein Merge: sein Baum ist der veröffentlichte
Baum, in den die Box ihre `modules/overrides.nix` und
`modules/install-target.nix` zurücklegt. Apply und der nächtliche Rebuild
bauen danach denselben Baum, was eine `github:`-URI hier nicht leisten
würde: ein Apply baute das ältere `/etc/nixos` und machte das Update der
Nacht rückgängig. lososd überspringt den Schritt und meldet `kept`, wenn
seit dem letzten Update eine andere Datei geändert wurde, sodass eine von
Hand nach LosOS Git gepushte Änderung nie überschrieben wird.
`nix/tests/invariants.nix` pinnt den Schritt.

:::warning[Beide Rebuild-Pfade impure lassen]
**Beide Rebuild-Pfade sind absichtlich `--impure`**:
`system.autoUpgrade.flags` und `rebuild_command` von lososd. Der
veröffentlichte Baum enthält weder `modules/install-target.nix` dieser Box
noch ihr `modules/overrides.nix`, daher importiert `flake.nix` die lebenden
Kopien aus `/etc/nixos`, wenn es sie lesen kann, und das kann nur eine
unreine Auswertung. Lass das Flag weg, und ein `github:`-Upgrade stellt
eine Box mit Schlüsseldatei auf die TPM-Form um, vergisst ihre Platten und
ihren Firmware-Modus und setzt jede Einstellung zurück. Bei reiner
Auswertung ist `builtins.pathExists` auf einem absoluten Pfad `false`, kein
Fehler. So bleiben CI, `nix flake check` und der Installer rein und sehen
die Datei aus dem Baum oder die Standardwerte. `nix/tests/invariants.nix` legt
das Flag fest.
:::

`nix.gc` läuft um 04:30 mit `--delete-older-than 14d`, und `boot.nix`
begrenzt beide Bootloader auf fünf Generationen. `/nix` *ist* `/persist`,
die ESP hat 500 MiB, und `linuxPackages_latest` legt dort in den meisten
Nächten einen neuen Kernel ab. Ohne beide Grenzen füllt sich die Box selbst,
und der Switch um 03:00 scheitert beim Bootloader-Schritt.
`nix/tests/invariants.nix` prüft beides schon bei der Auswertung.

**Das Fragment `#install` ist tragend.** Ohne es sucht `nixos-rebuild`
nach `nixosConfigurations.$(hostname)`. Dieser Flake exportiert nur `iso`
und `install`, also brach jeder nächtliche Lauf mit „flake does not provide
attribute“ ab, auf einer Box ohne Shell, von der aus es jemand hätte bemerken
können. Der Daemon hatte es die ganze Zeit richtig (`io_backend.rs`,
`/etc/nixos#install`). Nur der Standardwert auf NixOS-Seite war falsch.

Ein separater `midnight-reboot.timer` startet um 00:07 bedingungslos neu,
mit `Persistent=true`, damit eine Box, die ausgeschaltet war, das nachholt.
