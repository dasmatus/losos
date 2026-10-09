---
title: Administration
sidebar_position: 2
mdx:
  format: md
---

# Administration

## Admin-Oberfläche

Öffne aus dem LAN `http://<ip>/` (die Adresse im tty1-Banner der Box)
oder `https://<host>.local/`. Die Admin-Oberfläche ist für alles außerhalb
des LAN gesperrt, einschließlich Verkehr über den [Master-Proxy](/in-depth/master-proxy.md).
`<host>.local` ist ein mDNS-Name. Ein Rechner, der ihn nicht auflöst,
typischerweise der Host einer VM hinter NAT, verwendet die IP-Adresse, und
jede der folgenden Routen antwortet dort genauso.

Dienste auf demselben Host:

| Dienst              | URL                       |
| ------------------- | ------------------------- |
| Admin-Oberfläche    | `<host>.local/`           |
| Nextcloud           | `<host>.local/nextcloud`  |
| Forgejo (optional)  | `<host>.local/forgejo/`   |
| Handbuch            | `<host>.local/handbook/`  |
| [Lab](/in-depth/lab.md)       | `<host>.local/lab/`       |

Die IP-Adresse der Box (die aus dem tty1-Banner) und ihr bloßer Name
funktionieren überall anstelle von `<host>.local`, auch bei Nextcloud. So
kommst du aus einer libvirt-VM hinein, die keinen mDNS-Namen erhält. nginx
teilt dem Nextcloud-Pod mit, über welche Adresse jede Anfrage eingegangen
ist, und der Pod vertraut genau dieser einen, nie einem Platzhalter
(`modules/workloads.nix`). `localhost` und `127.0.0.1` vertraut Nextcloud
von sich aus.

`losos.tls.enable` (standardmäßig an) fügt HTTPS auf :443 hinzu, mit einem
Zertifikat, das die Box selbst erzeugt und das zwei Jahre gültig ist.
Einfaches HTTP auf :80 bleibt offen. Browser vertrauen dem Zertifikat erst,
wenn du es installierst. Der Einrichtungsassistent bietet einen
Einzeiler-Installer für macOS, Linux und Windows (`/setup/trust.sh`,
`/setup/trust.ps1`, beide nur im LAN) und den einfachen Download
(`/setup/losos-ca.crt`). Siehe [Installation im Detail](/in-depth/install.md).

## Aussehen

**Einstellungen, Aussehen** ändert das Aussehen der Admin-Seiten, für jeden Browser,
der die Box öffnet. Nichts davon ist eine Einstellung im Sinne von Nix. Es
ist ein Dokument, das lososd unter `/var/lib/losos/look.json` führt, daher
wirkt eine Änderung sofort beim Speichern, ohne Rebuild.

### Hintergrund

Wähle eines der drei mitgelieferten Bilder oder lade ein eigenes
hoch (PNG, JPEG, WebP, GIF oder SVG, bis 8 MiB). Der Regler **Schleier**
legt die Seitenfarbe über das Bild, von 20 % bis 90 %, damit Text im hellen
wie im dunklen Theme lesbar bleibt. *Schlicht* entfernt das Bild wieder.

lososd liefert ein hochgeladenes Bild unter `/api/look/background` ohne
Token aus, weil ein CSS-`background-image` keinen senden kann. Die Route
liegt hinter demselben Nur-LAN-Schutz wie der Rest der Admin-Seiten, und ein
Hintergrundbild ist kein Geheimnis.

### Selbst geschriebene Widgets

Die Galerie der Übersicht (**Widget hinzufügen**) bietet zwei Wege zu
einem eigenen Widget. *Eins bauen* stellt ein Widget ohne jeden Code aus den
Messwerten der Box zusammen. *Eins schreiben* nimmt eigenes HTML, eigenen
Stil und eigenes Skript an, die auf der Box gespeichert und in diesem
Bereich aufgelistet werden, wo du sie bearbeiten und löschen kannst.

Ein selbst geschriebenes Widget läuft in einem Sandbox-Frame, einem eigenen
Origin ohne Zugriff auf die Admin-Seiten, das Admin-Token oder die API. Mit
der Box spricht es über ein kleines `losos`-Objekt, das der Frame
bereitstellt:

| Aufruf                         | Was er liefert                                                       |
| ------------------------------ | -------------------------------------------------------------------- |
| `losos.metric(name)`           | Ein Promise auf einen Messwert der Box, mit denselben Namen, die die eingebauten Widgets der Galerie verwenden (`storage.bytes`, `box.settings`, …). |
| `losos.theme`, `losos.lang`    | `light`/`dark` und die Sprache der Admin-Seite.                      |
| `losos.palette`                | Die Farben der Box, auch als CSS-Variablen gesetzt (`var(--accent)` funktioniert). |
| `losos.onTheme(fn)`            | Wird aufgerufen, wann immer der Eigentümer das Theme wechselt.        |
| `losos.resize()`               | Bittet die Übersicht, die Kachel nach einer Änderung, die sie nicht sieht, neu zu vermessen. |

Der Tab **So funktioniert es** im Editor wiederholt dies mit einem Beispiel. Seine Vorschau
ist der echte Frame, was sie zeigt, zeigt also auch die Kachel. Ein Widget
kann Daten aus dem Internet abrufen (etwa eine Wetterkachel), aber nicht von
der Box. Die Grenze liegt bei 24 Widgets zu je 64 KiB.

## Admin-Token

`lososd` schreibt beim ersten Start ein zufälliges Token aus 64
Hexadezimalzeichen nach `/var/secrets/losos-admin-token` (Modus 0600). Das
Token kann jede Einstellung ändern, es ist also root.

Auf einer neuen, noch nicht beanspruchten Box setzt der erste Aufrufer aus
dem LAN, der `POST /api/setup/claim` sendet, das Eigentümer-Passwort und
erhält das Token in der Antwort. Diese Route verlangt kein Token, nimmt aber
keine Ansprüche mehr an, sobald die Box einen Eigentümer hat. Richte die
Box nur in einem vertrauenswürdigen LAN ein, denn der erste Aufrufer wird ihr
Eigentümer. Das Token wird nur einmal herausgegeben und kann über die
Oberfläche nicht erneut abgerufen werden.

Ein Anspruch, der gesendet wird, bevor Nextcloud seinen ersten Start
abgeschlossen hat, wird mit `503` und
`{"error": …, "ready": false, "waitingFor": …}` beantwortet und ändert
nichts. Die Box bleibt beanspruchbar. `GET /api/setup/claim` trägt dieselben
Felder `ready` und `waitingFor`. Der Einrichtungsassistent fragt sie
regelmäßig ab und lässt den Eigentümer von selbst weiter, sobald sie
umschlagen. Auf einer frischen Box dauert das einige Minuten, weil der
Nextcloud-Pod `occ maintenance:install` ausführt, bevor er irgendetwas
ausliefert.

`ready` schlägt um, wenn diese Installation fertig ist, also bevor LosOS
cloud einem Browser antwortet. Der Pod aktiviert dann seine 26 Apps und
startet erst danach Apache. Nativ auf einem 4-Kern-Host gemessen
(2026-10-07) dauert die Installation etwa 11 s, das Aktivieren der Apps etwa
13 s, und die erste Seite antwortet 27 s nach dem Start des Pods. Der
Anmeldeschritt des Assistenten zeigt für diese Lücke "Deine Dateien starten
noch". Ein Neustart wiederholt dieselben
Schritte gegen eine bereits installierte Instanz in etwa 2 s. In einer VM
ohne KVM (QEMUs TCG-Emulation) dauern dieselben Schritte 10 bis 30 Minuten,
gib einer Demo-VM also Hardwarevirtualisierung.

Dieser Aufrufer ist der Einrichtungsassistent. Jeder spätere Besuch der
Admin-Seiten fragt nach dem **Passwort**, das der Assistent gesetzt hat,
demselben, mit dem du dich bei LosOS cloud anmeldest. `POST /api/sign-in`
fragt Nextcloud, ob es stimmt, und gibt dem Browser-Tab bei einem Ja das
Token. Die Box hält keine zweite Kopie des Passworts, eine Änderung in LosOS
cloud ändert es also auch für die Admin-Seiten.

Das Token selbst wird nach dem Setzen des Passworts trotzdem einmal als
**Ersatz-Admin-Schlüssel** angezeigt, mit den Schaltflächen Kopieren und
Drucken. Es deckt den einen Fall ab, den das Passwort nicht abdecken
kann: LosOS cloud läuft nicht, denn LosOS cloud prüft das Passwort. Der
Entsperrdialog hat einen Link, um stattdessen den Schlüssel einzugeben.

Ein neues Passwort muss mindestens 12 Zeichen lang sein, mit einem
Kleinbuchstaben, einem Großbuchstaben, einer Ziffer und einem Symbol wie `-`
oder `!`. Der Assistent zeigt während der Eingabe die vier Regeln mit je
einem Haken. Ein Passwort, das vor dieser Regel gesetzt wurde, meldet sich
weiterhin an.

Das Token lässt sich auf der Box nicht rotieren, da sie keine Shell hat. Wer
das Passwort *und* den Ersatzschlüssel verliert, muss vom
Installationsmedium neu installieren, was die Festplatte löscht.

## Einstellungen {#settings}

Einstellungen sind Nix. Die Einstellungsseite erzeugt `modules/overrides.nix`
und führt `nixos-rebuild switch` aus. Daher:

- Eine Änderung braucht einen Rebuild, keinen Neustart.
- Die Box lässt sich aus diesem Repository plus dieser einen Datei neu
  bauen.
- `apply` ersetzt `overrides.nix` vollständig. Eine Einstellung, die die
  Oberfläche nicht schreibt, wird beim nächsten Speichern zurückgesetzt.

### Erweitert

**Einstellungen, Erweitert** listet jede `losos.*`-Option, die die Box deklariert,
gelesen aus den Modulen, die sie ausführt. `flake/options-doc.nix` erzeugt
das Dokument beim Build, und lososd liefert es unter `GET /api/options` aus,
mit dem aktuellen `overrides.nix` eingefügt. Jede Zeile zeigt die
Beschreibung der Option, ihren Standardwert, den Wert, mit dem die Box
läuft, und einen Editor für ihren Typ: einen Schalter, eine Zahl mit ihren
Grenzen, eine Auswahl, ein Textfeld oder ein Element pro Zeile für eine
Liste.

- Eine Option, die schon ein anderer Bereich bearbeitet (der Name, die
  Betriebsmodi der Apps, die Hardening-Schalter), wird mit ihrem Wert und
  einem Link zu diesem Bereich angezeigt, damit es einen einzigen Ort gibt,
  an dem sie geändert wird.
- Die drei, die der Installer schreibt (`targetDrives`, `tpm.enable`,
  `bios`), und die Pakete, die der Build auswählt, sind schreibgeschützt.
  Eine Zeile für eine davon in `overrides.nix` ließe den nächsten Rebuild
  scheitern.
- Mit **Vorsicht** markierte Optionen (`admin.*`, `cluster.*`,
  `hostName`, `storage.*`, `tls.*`, `upgradeFlakeUri`, …) fragen vor der
  ersten Änderung einmal nach. Ein falscher Wert hinterlässt eine Box ohne
  Shell, von der aus man ihn beheben könnte.
- **Standard verwenden** entfernt die Zeile, statt den Standardwert zu schreiben,
  sodass einem Standardwert gefolgt wird, der sich mit einem Update ändert.
- Eine Zeile in `overrides.nix`, für die die Box keine Option hat (ein
  Tippfehler, eine Einstellung aus einer anderen Version), wird in einer
  eigenen Gruppe angezeigt und blockiert Anwenden, bis sie entfernt ist. lososd
  lehnt einen solchen Body ebenfalls ab (`backend/src/options.rs`,
  `check_body`). Es prüft jede Zeile eines Apply gegen das Dokument:
  deklariert, nicht schreibgeschützt, ein Wert der richtigen Art, kein `${`.

Alles, was hier geschrieben wird, durchläuft dasselbe Apply und denselben
Rebuild wie die anderen Bereiche. `losos.edge.*` ist ausgelassen, weil die
Appliance es nie liest.

### Historie und LosOS Git

Jedes Apply, jede Speicheränderung und jedes Zurücksetzen auf
Werkseinstellungen ist ein Commit in `/etc/nixos`, dem Repository, das
`losos-ctl install` angelegt hat. Der Commit ist nach den Einstellungen
benannt, die er geändert hat, `Change hostName, cluster.enable`, mit dem
Vorher und Nachher jeder Einstellung im Body. **Einstellungen, Historie** listet
die letzten vierzig.

Mit `losos.configRepo.enable` (Standard) und eingeschaltetem LosOS Git führt
lososd die Konfiguration außerdem in einem privaten Repository auf LosOS
Git, `<owner>/losos-config`, das dem eigenen Konto des Admins (`notshared`)
gehört. Das Konto wird bei der ersten Synchronisierung mit dem
Admin-Passwort angelegt und bei jeder Passwortänderung und Anmeldung damit
abgeglichen. Ein Reconciler in lososd läuft alle 30 s:

- er committet alles Uncommittete und pusht den Branch der Box, wenn LosOS
  Git zurückliegt;
- wenn LosOS Git voraus ist, weil du das Repository geklont,
  `modules/overrides.nix` bearbeitet und gepusht hast, spult er per
  Fast-Forward darauf vor, prüft das neue `overrides.nix` Zeile für Zeile so,
  wie Apply geprüft wird, und startet einen Rebuild. Ein abgelehnter Push
  wird unter Historie gemeldet, und die Box bleibt auf ihrem eigenen Commit;
- wenn beide auseinandergelaufen sind (eine umgeschriebene Historie), tut er
  nichts und sagt das. Bereinige es von einem Klon aus;
- während ein Rebuild läuft, wartet der Push auf den nächsten Tick.

**Jetzt abgleichen** im Bereich Historie führt sofort einen Tick aus. `losos-ctl
config` gibt dasselbe Dokument aus, und `losos-ctl config --sync` führt
einen Tick aus. Geheimnisse gelangen nie in das Repository. `overrides.nix`
enthält nur Optionswerte, und das Token, mit dem sich lososd bei LosOS Git
authentifiziert, liegt in `/var/lib/forgejo/.losos-token`, außerhalb von
`/etc/nixos`. Dahinter steht kein Forgejo-Actions-Runner. Die Box fragt
regelmäßig ab.

Die Klon-Adresse wird im Bereich angezeigt (`http://<box>/forgejo/<owner>/
losos-config.git`). Das Repository ist privat und braucht daher das
Admin-Passwort. Der Daemon spricht über Loopback mit Forgejo, mit einem
Bot-Konto `losos`, das das Start-Skript von Forgejo anlegt und dem es ein
Token ausstellt (`flake/forgejo-bootstrap.nix`).

### Föderation

Die Föderation läuft nur, solange die Box ihre Festplatte teilt. Beide
Föderationsoptionen unten sind per UND mit `losos.sharingMyStorage`
verknüpft, der Einstellung, die den geteilten Datenpool freischaltet
(`lososInternal.federation` in `modules/options.nix`). Mit ausgeschaltetem
Teilen spricht weder LosOS Git noch LosOS cloud mit anderen Servern, egal
was die eigene Option sagt, und der Bereich Erweitert zeigt beide Zeilen als
nicht verfügbar mit dem Grund an.

`losos.forgejo.federation.enable` (standardmäßig an) schaltet die
ActivityPub-Seite von Forgejo in beiden Modi ein: `[federation] ENABLED` in
app.ini, mit `SHARE_USER_STATISTICS = false`, damit nodeinfo keine Konto-
oder Aktivitätssummen veröffentlicht. Das mitgelieferte Forgejo (16.x)
föderiert Sterne (Settings → Federation eines Repositorys listet die Server,
deren Sterne zählen), erlaubt, Konten von anderen Servern aus zu folgen, und
liefert nodeinfo sowie einen ActivityPub-Actor pro Konto und Repository
unter `/api/v1/activitypub/` aus; jede eingehende Anfrage muss eine gültige
HTTP-Signatur tragen. Im Container-Modus fügt der Front-vhost Routen mit
exakter Übereinstimmung für `/.well-known/nodeinfo` und
`/.well-known/webfinger` hinzu, die zwei Adressen, über die andere Server
eine Box entdecken und die Forgejo an der Wurzel des Hosts statt unter
`ROOT_URL` ausliefert; sie sind wie `/forgejo/` nicht durch den
Nur-LAN-Schutz abgesichert. Die Reichweite folgt `ROOT_URL`: über die Edge
ist es `https://<proxy hostname>/forgejo/`, und jeder Server kann mit der
Box föderieren; auf einer reinen LAN-Box ist es
`http://<host>.local/forgejo/`, sodass das nur andere Boxen im selben LAN
können. `tests/forgejo-federation.nix` startet das echte Forgejo und fragt
es ab, mit eingeschaltetem und dann ausgeschaltetem Teilen.

`losos.nextcloud.federation.enable` (standardmäßig an) ist die Seite von
LosOS cloud: Federated Cloud Sharing (Teilen mit `user@host`-Konten auf
anderen Nextcloud-Servern), Kalenderföderation und die App trusted-servers.
Ist sie aus, passieren zwei Dinge. Die eigenen Schalter von Nextcloud werden
bei jedem Start gesetzt: `outgoing_server2server_share_enabled` und
`incoming_server2server_share_enabled` von `files_sharing` (und die zwei
Gruppenvarianten) auf `no`, `enableCalendarFederation` von `dav` auf false,
und die App `federation` wird deaktiviert. Der Entrypoint des Pods tut das
anhand einer `federation`-Datei, die der Host einhängt; der native Modus
hängt es an `nextcloud-setup` an. Und der vhost antwortet mit 404 auf die
Adressen, die andere Server aufrufen: `ocm-provider`, `ocs-provider`,
`ocm/`, `.well-known/ocm`, `ocs/v[12].php/cloud/shares` und die App-Routen
von `federation` und `federatedfilesharing`. Die vhost-Hälfte ist nötig,
weil die OCM-Discovery sich als aktiviert meldet, egal was die
Einstellungen sagen, und `cloud_federation_api` und `federatedfilesharing`
immer aktivierte Apps sind, die sich nicht abschalten lassen.
`tests/front-vhost.nix` prüft die Routen beider Dienste mit ein- und
ausgeschaltetem Teilen; `tests/invariants.nix` prüft das Gate zur
Evaluierungszeit.

## Sicherung, Wiederherstellung und Löschen

**Einstellungen, Sicherung** sendet die Daten der Box in einen S3-kompatiblen
Bucket, den der Eigentümer mietet (Amazon S3, Glacier eingeschlossen,
Backblaze B2, Wasabi, Cloudflare R2, MinIO).
[restic](https://restic.net) übernimmt das Kopieren und verschlüsselt auf der
Box, bevor irgendetwas sie verlässt, der Bucket enthält also nur Chiffretext.
Das Repository-Passwort ist der Wiederherstellungscode der Box
(`/var/secrets/losos-recovery-code`), den der Bereich auf Anfrage anzeigt;
ohne ihn kann niemand ein Backup öffnen.

Ein Backup enthält:

- `/var/lib/nextcloud` und `/var/lib/forgejo`, ohne die Nextcloud-Vorschauen;
- einen `pg_dump` der Datenbanken `nextcloud` und `forgejo`;
- den geteilten Ordner. Bei eingeschaltetem Teilen ist er eine
  fscrypt-Policy, und Linux liefert für eine gesperrte fscrypt-Datei keinen
  Chiffretext, daher entsperrt das Skript die Policy für die Kopie und
  sperrt sie danach wieder. Im Bucket deckt ihn die Verschlüsselung von
  restic ab;
- `modules/overrides.nix`, das Aussehen der Startseite und das Proxy-Token.

Bei einer `amazonaws.com`-Adresse kann der Eigentümer eine Speicherklasse
wählen: Glacier Instant Retrieval, Glacier Flexible Retrieval oder Glacier
Deep Archive. restic legt nur die Datenpakete in diese Klasse und hält seine
eigenen Metadaten in S3 Standard, sodass das Prüfen eines Codes und das
Aufräumen sofort bleiben. Eine Wiederherstellung aus Flexible Retrieval oder
Deep Archive schaltet restics Funktion `s3-restore` ein, die AWS bittet, die
Pakete aufzutauen, und bis zu 48 Stunden wartet; das Aufräumen packt dort
nie um (`--max-repack-size 0`), da ein Umpacken ebenfalls ein Auftauen
bräuchte. Eine Lifecycle-Regel, die den Bucket nach Glacier verschiebt,
würde restics Metadaten mitnehmen, daher wird die Klasse stattdessen im
Bereich gewählt.

Der Bucket behält die letzten sieben. lososd führt jedes Backup und jede
Wiederherstellung als transiente Unit `losos-backup-<job>` aus, die systemd
außerhalb der `ProtectHome=true`-Sandbox von lososd startet. Die Schlüssel
des Buckets erreichen sie als `EnvironmentFile=` in `/var/secrets`, nie
über eine Befehlszeile.

**Wiederherstellen** verlangt den Wiederherstellungscode der Box, die das Backup
erstellt hat. Der neueste Snapshot kommt zurück: Die Apps halten an, die
Dateien werden zurücksynchronisiert, die Datenbanken mit
`pg_restore --clean` geladen, die Apps starten, und das wiederhergestellte
`overrides.nix` wird wie jedes Apply geprüft und neu gebaut. Der Code, der
das Backup geöffnet hat, wird zum Wiederherstellungscode dieser Box. Auf
einer gelöschten oder neu installierten Box führst du den Assistenten aus,
richtest denselben Bucket ein und stellst dann wieder her.

**Einstellungen, Zurücksetzen, Löschen** löscht die Daten ebenso wie die Einstellungen.
lososd steuert den Vorgang in Phasen, die in `state.json` festgehalten
werden, sodass er auch ohne offenen Tab weiterläuft:

1. ein optionales Backup. Schlägt es fehl, stoppt das Löschen, und nichts
   ändert sich;
2. ein Countdown von `losos.reset.graceMinutes` (standardmäßig 15). Abbrechen
   stoppt ihn, und weder auf der Box noch außerhalb hat sich bisher etwas
   geändert;
3. Verlassen der Edge: Die eigenen Domains der Box werden entfernt, ihre
   aktiven Angebote auf dem Marktplatz geschlossen, und sie wird aus der
   Registry ausgetragen (`POST /deregister`). Jeder Schritt wird versucht,
   auch wenn der vorherige fehlschlug, und der Bericht nennt die, die die
   Edge nicht bestätigt hat;
4. die Standardeinstellungen werden committet und neu gebaut, dann
   hinterlässt lososd eine Markierung auf `/persist` und startet neu;
5. `losos-factory-wipe.service` läuft früh im nächsten Bootvorgang, vor
   `sysinit.target` und vor jedem Dienst, dem die Daten gehören, und löscht
   die App-Daten, die Datenbanken, den Zustand beider Kubernetes-Instanzen,
   die Geheimnisse, das Journal und die fscrypt-Metadaten. Er leert beide
   Daten-Homes. Die Markierung wird zuletzt entfernt, sodass ein
   unterbrochenes Löschen erneut läuft.

Abbrechen ist nur in den Schritten 1 und 2 möglich. `/nix`, `/etc/nixos`
und die Schlüsseldatei der Festplatte bleiben, LosOS ist also weiterhin
installiert, und die Box öffnet den Einrichtungsassistenten.
`/var/lib/losos-erase/report.json` hält fest, was das Löschen außerhalb der
Box aufgegeben hat, als Zählwerte, und Einstellungen, Zurücksetzen zeigt es an.
`tests/erase.nix` durchläuft den ganzen Zyklus gegen einen MinIO-Bucket in
einer VM.

## Benutzer

Zwei Datenbenutzer, beide ohne Passwort und ohne Shell:

| Benutzer    | Besitzt             |
| ----------- | ------------------- |
| `notshared` | Nextcloud           |
| `shared`    | Mesh-Speicher       |

Jeder hat seine eigene primäre Gruppe und ein Home mit Modus 700, sodass
keiner die Dateien des anderen lesen kann. `tests/impermanence.nix` prüft
das.

## Updates und Neustarts

- **03:00.** `system.autoUpgrade` baut aus `losos.upgradeFlakeUri` neu. Der
  Standardwert `git+file:///etc/nixos#install` zieht keine neuen Pakete.
  Setze eine `github:`-URI, um Updates zu bekommen. Behalte das
  Fragment `#install`; ohne es schlägt der Rebuild fehl. Eine entfernte URI
  verwendet weiterhin die Laufwerksliste, den Firmware-Modus, den
  Entsperrmodus und die Einstellungen dieser Box, weil der Rebuild sie aus
  `/etc/nixos` auf der Box liest, nicht aus dem veröffentlichten Repository.
- **00:07.** `midnight-reboot.timer` startet bedingungslos neu
  (`Persistent=true`, sodass eine Box, die aus war, das nachholt). Da die
  Wurzel beim Booten neu aufgebaut wird, ist der Neustart der Weg, auf dem
  sich die Box selbst repariert.
