[English](Architecture) · [Slovenčina](Architecture-sk) · **Deutsch**

# Architektur

## Systeme im Flake

| Ausgabe                       | Was                                              |
| ----------------------------- | ------------------------------------------------ |
| `nixosConfigurations.iso`     | Live-Installer-ISO (`options`, `disko`, `installer`) |
| `nixosConfigurations.install` | Die installierte Appliance (alle Module)         |
| `nixosModules.edge`           | Die VPS-Seite des [Master-Proxys](Master-Proxy-de) und des Mesh |

Alle Projektoptionen liegen unter `losos.*` in `modules/options.nix`, mit
Standardwerten in `defaults.nix`. Module lesen `config.losos.*`.

## Impermanence

`/` ist ein tmpfs. `/persist` ist LUKS-verschlüsseltes ext4. `impermanence`
bindet diese Pfade per Bind-Mount aus `/persist` ein: `/nix`, `/var`,
`/etc/ssh`, `/etc/keys`, `/etc/nixos`, `/etc/rancher`, die beiden
Daten-Home-Verzeichnisse und `machine-id`.

**Alles, was nicht auf dieser Liste steht, geht beim Neustart verloren.**
Neuen Zustand trägst du in `modules/impermanence.nix` ein.

## Steuerungsebene

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

## Workloads

Nextcloud und Forgejo laufen im eigenen k3s-Cluster der Box
(`losos.<svc>.mode = "container"`) oder nativ auf dem Host. Die Pods nutzen
`hostNetwork`. nginx ist der einzige Dienst auf öffentlichen Ports und
routet nach Pfad.

## Aufbau des Repositorys

```
modules/                 NixOS modules; options in options.nix
backend/                 lososd + losos-ctl (Rust)
backend-registrar/       master-proxy edge registrar (Rust)
admin-ui/app/            the admin SPA
admin-ui/themes/         LosOS cloud and LosOS Git: themes on the SPA's tokens.css, logos, names
admin-ui/design-system/  dev-only tokens and React wrapper; not shipped
tests/                   NixOS VM tests
docs/                    security model, design specs and plans
wiki/                    source of this wiki
```

`CLAUDE.md` ist die komprimierte Fassung dieser Seite und der Stolperfallen
unter [Entwicklung](Development-de#fallstricke-die-lautlos-zubeißen).

## Im Detail

Ein Absatz pro Mechanismus: warum jedes Teil dort sitzt, wo es sitzt, und was
eine gut gemeinte Änderung dort kaputt macht.

**Zwei NixOS-Systeme teilen sich einen Modulsatz** (`flake.nix`).

- `iso` ist eine minimale Live-ISO. Sie enthält das `disko`-Layout, damit
  sie die Zielplatte formatieren und Verschlüsselungsschlüssel einschreiben
  kann, und importiert nur `options.nix`, `disko.nix` und `installer.nix`.
- `install` ist das auf der Platte installierte System. Es importiert alles:
  `options`, `configuration`, `impermanence`, `disko`, `boot`, `services`,
  `nextcloud-common`, `containers`, `daemon`, `overrides`, `updates`,
  `defaults`. Der Flake übergibt `specialArgs.self = self`, damit
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
man eine Aussperrung beheben könnte; `keyring.nix` trifft dieselbe
Entscheidung.

Sieht das Installationsmedium kein `/dev/tpmrm0`, oder mit `losos-ctl install
--no-tpm`, läuft die Box im Schlüsseldatei-Modus. `install-target.nix` setzt
`tpm.enable = false`, und die Schlüsseldatei kommt als
`/crypto_keyfile.bin` in die initrd, auf der unverschlüsselten ESP. Die
beiden crypttab-Formen schließen einander aus: systemd-cryptsetup, das eine
Schlüsseldatei *und* `tpm2-device=` bekommt, liest die Datei als versiegelten
Blob. `tests/invariants.nix` legt den Standardwert fest und prüft, dass der
TPM-Pfad kein initrd-Geheimnis deklariert. `tests/tpm.nix` bootet ihn unter
swtpm von Anfang bis Ende.

Eine entfernte `github:`-Upgrade-URI wertet einen Baum ohne
`install-target.nix` aus. Deshalb bevorzugt `flake.nix` die lebenden
`/etc/nixos/modules/{install-target,overrides}.nix`, wann immer es sie lesen
kann, und beide Rebuild-Pfade (`updates.nix`, `supervisor.rs`) übergeben
dafür `--impure`. Siehe den Absatz zum automatischen Upgrade weiter unten.

**Zwei isolierte Datendomänen, keine Shell** (`configuration.nix`). `notshared`
(uid 1000) besitzt Nextcloud. `shared` (uid 1001) besitzt den Speicher, der
dem Mesh zur Verfügung gestellt wird. Beide Home-Verzeichnisse haben Modus
`700`, jedes mit eigener primärer Gruppe, sodass keiner der beiden Benutzer
die Dateien des anderen lesen kann. `isNormalUser` ohne explizite `group`
steckt beide in `users`, und ein Home mit `750` gibt dieser Gruppe dann r-x.
Das hat die Isolation unbemerkt ausgehebelt, bis `tests/impermanence.nix` es
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

`losos-ctl` ist eine schlanke Fassaden-CLI. Sie behält die alten
Unterbefehle und die JSON-Ausgabe bei (`state`, `change --mode`, `status`,
`settings`, `apply` (stdin), `factory-reset`; `--json` bewirkt nichts) und
reicht jeden Aufruf über D-Bus an lososd weiter. `losos-ctl install` bleibt
lokal, weil der ISO-Installer als root läuft und keinen Bus braucht.

Die Admin-UI (`admin-ui/app/`, paketiert als `losos-admin-ui` mit
`buildNpmPackage`) ist eine SPA aus React 19, Vite und Tailwind v4 auf echten
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
(`flake/options-doc.nix`, `modules/config-repo.nix`,
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
echte Dokument (`tests/admin-ui.nix` übergibt den Check `losos-options-doc`)
und schlägt bei jeder Zeile fehl, die der Bereich nicht zeichnen kann.
`POST /api/apply` prüft jede Zeile gegen das Dokument (`check_body`). Es
lehnt nicht deklarierte Schlüssel, schreibgeschützte (vom Installer
festgelegte Werte, Pakete), falsch typisierte Werte und jedes `${` ab und
nennt den abgelehnten Schlüssel.

Jedes Apply, jeder Moduswechsel und jedes Zurücksetzen ist ein Commit in
`/etc/nixos`, dem Repository, das der Installer angelegt hat. Ein
Abgleich-Thread in lososd fragt alle 30 s ab. Er legt das Forgejo-Konto des
Admins an (`notshared`, mit dem Passwort des Besitzers, das bei Anmeldung
und Passwortänderung mitgezogen wird) sowie ein privates Repository
`losos-config`. Das geschieht über ein Bot-Konto, dessen Token das
Forgejo-Startskript erzeugt (`flake/forgejo-bootstrap.nix`, Token unter
`/var/lib/forgejo/.losos-token`). Er pusht, wenn Forgejo zurückliegt. Hat
ein Klon vorausgepusht, macht er einen Fast-Forward, lässt das neue
`overrides.nix` durch dasselbe Tor und baut neu. Eine divergierte Historie
meldet er und rührt sie nie an. Dahinter steht absichtlich **kein Forgejo
Actions Runner**.

Der Bereich Erweitert verlinkt die sechzehn Einstellungen, die den anderen
Bereichen gehören, statt sie ein zweites Mal zu bearbeiten. `OWNED` in
`admin-ui/app/src/lib/option-value.ts` und `OWNED_NAMES` in `lib/api.ts`
sind diese Liste, und `buildOverridesNix` filtert zusätzliche Schlüssel
dagegen, damit die Datei nie einen Schlüssel doppelt enthält. Zwei
Eigenschaften des Parsers sind hier wichtig. `parse_line` in `overrides.rs`
schneidet einen Wert beim ersten `#` *außerhalb* von Anführungszeichen ab;
früher schnitt es auch innerhalb, was das Standard-`upgradeFlakeUri`
verstümmelte. Und eine Liste ist eine Zeile `[ "a" "b" ]`, nur Strings.

**Workloads und die einzige Eingangstür** (`modules/containers.nix`,
`modules/workloads.nix`, `modules/cluster.nix`,
`modules/nextcloud-common.nix`). Rootless Podman ist weg, und
systemd-nspawn auch. Bei `losos.<svc>.mode == "container"` laufen Nextcloud
und Forgejo als Kubernetes-Workloads im eigenen lokalen k3s-Cluster der Box.
Die gemeinsame Nextcloud-Konfiguration liegt weiterhin in
`nextcloud-common.nix`, sodass nativer und Workload-Modus nicht
auseinanderlaufen können. nginx ist der einzige Prozess, der öffentliche
Ports belegt. Er routet nach Pfad auf dem mDNS-Namen der Appliance
(`<hostName>.local:80/nextcloud`, `:80/forgejo`) und liefert die Admin-SPA
auf demselben `:80`-vhost aus.

`admin-ui/design-system/` (`tokens.css` und `losos.css`, dazu ein
Wrapper-Paket `react/` für claude.ai/design) ist nur für den
Entwicklerrechner gedacht und wird überhaupt nicht mehr ausgeliefert. Die
ausgelieferte SPA bringt ihre eigene Palette in
`admin-ui/app/src/styles/tokens.css` mit, die `index.css` als Theme-Schlüssel
von Tailwind v4 neu veröffentlicht. Das Designsystem bleibt außerhalb der
Nix-Closure, weil die Quell-*Wurzel* von `losos-admin-ui` `admin-ui/app` ist.
Das ist eine stärkere Garantie als der Namensfilter `design-system`, den sie
ersetzt hat und den eine Umbenennung ausgehebelt hätte.

**Ein Erscheinungsbild für Startseite, Nextcloud und Forgejo** (`admin-ui/themes/`).
`tokens.css` ist absichtlich reines CSS. Die Themes beider Apps liefern es
Byte für Byte als `losos-tokens.css` aus und bilden ihre eigenen Variablen
darauf ab, sodass eine dort geänderte Farbe alle drei mitzieht.

Nextcloud bekommt einen *Theme-Ordner*: `nextcloud-stack.nix` kopiert
`themes/losos/` ins Paket, und die Konfiguration setzt `theme = "losos"`.
Nextcloud löst Themes gegen sein echtes Server-Root auf und findet daher nie
einen Symlink neben dem Paket. Forgejo bekommt
`theme-losos-{auto,light,dark}.css` in `$FORGEJO_CUSTOM/public/assets/css/`,
nativ per tmpfiles verlinkt und vom Entrypoint des Images kopiert, mit
`DEFAULT_THEME = losos-auto`. Forgejo prägt das Konten bei ihrer Erstellung
auf, sodass ein bestehendes Konto sein altes Theme behält, bis sein Besitzer
umschaltet.

Besitzer sehen die beiden Apps als **LosOS cloud** und **LosOS Git**.
`defaults.php` im Theme-Ordner benennt Nextcloud, und `APP_NAME` von Forgejo
benennt Forgejo. Beide bekommen Logos, die aus `admin-ui/themes/brand/`
gezeichnet sind. `marks.py` verpackt `salmon.png`, einen lebenden
Silberlachs, ausgeschnitten aus einem Foto von NOAA Fisheries, in die SVGs
und kopiert ihn in die SPA und das Handbuch. Der Build rendert daraus jedes
PNG und die `.ico`. Am 31. Oktober, nach dem eigenen Datum des Betrachters,
zeigen die Admin-Seiten und das Handbuch stattdessen `plate.png`, einen
Teller Lachs aus Matus' eigenem Foto. `brand/CREDITS.md` nennt die Herkunft
beider Bilder. Der alte Pixel-Art-Fisch wurde wegen urheberrechtlicher
Bedenken entfernt und kommt nicht zurück. Das restliche
Upstream-Branding regelt die Konfiguration (keine Skeleton-Dateien, keine
Hilfe- oder Registrierungslinks, kein „Powered by“) sowie zwei überschriebene
Forgejo-Templates.

Zwei Details übersieht man leicht. Der Header-Filter von Nextcloud
invertiert jedes Logo, das er für sein eigenes weißes hält, deshalb setzt
`server.css` `--image-logoheader-custom`. Und die Theming-App liest einige
Dateien aus `core/img/` über den absoluten Pfad, deshalb überschreibt
`nextcloud-stack.nix` diese im Paket. Darum setzt das Image
`integrity.check.disabled`, wie es `services.nextcloud` bereits tut.
`admin-ui/themes/default.nix` sind reine Daten, die alle drei Aufrufer
importieren, aus demselben Grund wie bei `nextcloud-stack.nix`. Die
Workload-Pods nutzen `hostNetwork`, da der lokale Cluster kein CNI hat, und
antworten daher auf Loopback. Was das kostet, erklärt die
[Stolperfalle](Development-de#fallstricke-die-lautlos-zubeißen) zum
`lanOnly`-Wächter.

**Der Vercel-Demo-Host** (`edge-vercel/`). Ein drittes Rust-Crate außerhalb
des Flakes, das den Router des Registrars als eine Vercel Function ausführt.
Die Registry ist eine `jsonb`-Zeile in einem Neon-Postgres (tokio-postgres
über rustls), mit `DATABASE_URL` aus der Marketplace-Integration von Vercel,
vor jedem Request geladen und danach per Upsert gespeichert. Es fügt zwei
reine Demo-Routen (`/status`, `/status/traefik`) und eine statische Seite
hinzu. Es existiert für Präsentationen und soll danach gelöscht werden. In
`backend-registrar` brauchte es nur zwei Änderungen. `server::build` liefert
Router und Abgleicher ohne Listener und Timer, und `serve` ruft es auf.
`Registry::{export,import}` ist ein Snapshot mit Altersangaben, damit die
Registry zwischen Prozessen wandern kann, und `import` wendet die
Heartbeat-TTL an, weil dazwischen kein Abgleicher lief.

Tunnel, Traefik, Mesh und Stripe-Gate können dort nicht laufen, daher
antworten auf diesem Host `/cluster/join` und `/market/*` mit 503 und
`/noise-public-key` mit 404. Die `crates`-Liste in `devenv.nix` und CI linten
und testen es wie die anderen beiden Crates. `nix build` sieht es nie. Sein
`tests/postgres.rs` läuft nur, wenn `LOSOS_TEST_DATABASE_URL` gesetzt ist,
was der Test-Job in CI mit einem Postgres-Dienst bereitstellt. Der Rest
seiner Testsuite läuft auf dem In-Memory-Speicher.

Seine zweite statische Seite, `public/find.html`, ist der Ablauf „find my
box“ (meine Box finden). Die Berechtigung Local Network Access in Chrome
erlaubt ihr, `/setup/state.json` einer Box vom öffentlichen Origin aus zu
lesen. `modules/setup.nix` erlaubt genau die Origins in
`losos.setup.finderOrigins` (eine nginx-Map, geprüft von `tests/setup.nix`),
nie `*`, denn das Dokument ist ein LAN-Inventar.

**Der Options-Namensraum `losos.*`** (`options.nix`). Alle
projektspezifischen Einstellungen liegen unter `options.losos`, mit
Standardwerten in `defaults.nix`: `targetDrive`, `tpm.enable`,
`sharingMyStorage`, `forgejo.enable`, `nextcloud.*`, `cluster.*`
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
systemd-Sandboxing für `nginx`, `avahi-daemon` und `lososd`. Die Blacklist
blockiert auch ein explizites `modprobe`. Ein einfaches
`boot.blacklistedKernelModules` tut das **nicht**; es verhindert nur das
automatische Laden über Aliase. Vier Opt-in-Flags tragen das, was etwas
kaputt machen kann: `apparmor`, `malloc`, `nosmt`, `usbguard`. Das System
`iso` importiert das Modul nicht, weil das Live-Medium squashfs braucht.
`tests/hardening.nix` prüft beide Hälften, auch dass nichts die
Kernel-Funktionen blockiert, die k3s, rke2, containerd und Longhorn brauchen.

**Online-Vergrößerung** (`grow.rs`, `losos.storage.fillPercent`). Das
Logical Volume endet absichtlich vor dem Ende der Volume Group, damit
`/persist` vergrößert werden kann, ohne die Box zu öffnen. `/persist` *ist*
`/nix`, über Impermanence. `losos-ctl grow` führt lvextend aus, dann
`cryptsetup resize`, dann `resize2fs`, in dieser Reihenfolge, im laufenden
Betrieb. Die Reihenfolge ist das ganze Korrektheitsargument, und ein Fehler
darin scheitert lautlos. Deshalb prüft `grow.rs` sie gegen den *Plan*, und
`tests/resize.nix` prüft sie von Anfang bis Ende.

**BIOS und das tty1-Banner** (`losos.bios`, `modules/console.nix`). Das
Firmware-Menü der Installer-ISO schreibt `losos.bios` in
`install-target.nix`. BIOS und UEFI lassen sich explizit wählen. Die
Autoerkennung wählt BIOS, wenn `/sys/firmware/efi` fehlt. `--bios` und
`--uefi` wählen ohne Menü. Das Menü gibt es nur auf der Installer-ISO, nicht
im installierten System, und es nimmt nach 30 s ohne Antwort die
Autoerkennung, weil das Medium unbeaufsichtigt installieren soll und
`tests/iso-boot.py` nie tippt. BIOS stellt `boot.nix` auf GRUB um und fügt
der ersten Platte in `disko.nix` eine EF02-Partition hinzu. Die ESP bleibt
in beiden Fällen `/boot`. `console.nix` ersetzt getty auf tty1 durch einen
Banner-Dienst, der die IPv4-Adresse im LAN und `<hostName>.local` anzeigt.

**Automatisches Upgrade und der nächtliche Neustart** (`updates.nix`).
`system.autoUpgrade` baut um 03:00 aus `losos.upgradeFlakeUri` neu. Der
Standard, `git+file:///etc/nixos#install`, bringt das System nur konsistent
voran und zieht kein neues nixpkgs. Setze eine `github:`-URI, um
wirklich zu aktualisieren.

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
die Datei aus dem Baum oder die Standardwerte. `tests/invariants.nix` legt
das Flag fest.

`nix.gc` läuft um 04:30 mit `--delete-older-than 14d`, und `boot.nix`
begrenzt beide Bootloader auf fünf Generationen. `/nix` *ist* `/persist`,
die ESP hat 500 MiB, und `linuxPackages_latest` legt dort in den meisten
Nächten einen neuen Kernel ab. Ohne beide Grenzen füllt sich die Box selbst,
und der Switch um 03:00 scheitert beim Bootloader-Schritt.
`tests/invariants.nix` prüft beides schon bei der Auswertung.

**Das Fragment `#install` ist tragend.** Ohne es sucht `nixos-rebuild`
nach `nixosConfigurations.$(hostname)`. Dieser Flake exportiert nur `iso`
und `install`, also brach jeder nächtliche Lauf mit „flake does not provide
attribute“ ab, auf einer Box ohne Shell, von der aus es jemand hätte bemerken
können. Der Daemon hatte es die ganze Zeit richtig (`io_backend.rs`,
`/etc/nixos#install`). Nur der Standardwert auf NixOS-Seite war falsch.

Ein separater `midnight-reboot.timer` startet um 00:07 bedingungslos neu,
mit `Persistent=true`, damit eine Box, die ausgeschaltet war, das nachholt.
