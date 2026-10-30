---
title: Administration
sidebar_position: 2
mdx:
  format: md
---

# Administration

## Admin-Oberfläche {#admin-ui}

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
| LosOS Cloud         | `<host>.local/cloud/`     |
| LosOS Git           | `<host>.local/git/`       |
| Handbuch            | `<host>.local/handbook/`  |
| [Diagramme](/in-depth/diagrams.md) | `<host>.local/diagrams/` |

Die IP-Adresse der Box (die aus dem tty1-Banner) und ihr bloßer Name
funktionieren überall anstelle von `<host>.local`, auch bei LosOS Cloud. So
kommst du aus einer libvirt-VM hinein, die keinen mDNS-Namen erhält. LosOS
Cloud führt keine Liste vertrauenswürdiger Namen und antwortet deshalb auf
der Adresse, über die eine Anfrage eingegangen ist.

`losos.tls.enable` (standardmäßig an) fügt HTTPS auf :443 hinzu, mit einem
Zertifikat, das die Box selbst erzeugt und das zwei Jahre gültig ist.
Einfaches HTTP auf :80 bleibt offen. Browser vertrauen dem Zertifikat erst,
wenn du es installierst. Der Einrichtungsassistent bietet einen
Einzeiler-Installer für macOS, Linux und Windows (`/setup/trust.sh`,
`/setup/trust.ps1`, beide nur im LAN) und den einfachen Download
(`/setup/losos-ca.crt`). Siehe [Installation im Detail](/in-depth/install.md).

## Aussehen {#look}

**Einstellungen, Aussehen** ändert das Aussehen der Admin-Seiten, für jeden Browser,
der die Box öffnet. Nichts davon ist eine Einstellung im Sinne von Nix. Es
ist ein Dokument, das lososd unter `/var/lib/losos/look.json` führt, daher
wirkt eine Änderung sofort beim Speichern, ohne Rebuild.

### Hintergrund {#background}

Wähle eines der drei mitgelieferten Bilder oder lade ein eigenes
hoch (PNG, JPEG, WebP, GIF oder SVG, bis 8 MiB). Der Regler **Schleier**
legt die Seitenfarbe über das Bild, von 20 % bis 90 %, damit Text im hellen
wie im dunklen Theme lesbar bleibt. *Schlicht* entfernt das Bild wieder.

:::note[Warum das Bild kein Token braucht]
lososd liefert ein hochgeladenes Bild unter `/api/look/background` ohne
Token aus, weil ein CSS-`background-image` keinen senden kann. Die Route
liegt hinter demselben Nur-LAN-Schutz wie der Rest der Admin-Seiten, und ein
Hintergrundbild ist kein Geheimnis.
:::

### Selbst geschriebene Widgets {#widgets-written-by-hand}

Die Galerie der Übersicht (**Widget hinzufügen**) bietet zwei Wege zu
einem eigenen Widget. *Eins bauen* stellt ein Widget ohne jeden Code aus den
Messwerten der Box zusammen. *Eins schreiben* nimmt eigenes HTML, eigenen
Stil und eigenes Skript an, die auf der Box gespeichert und in diesem
Bereich aufgelistet werden, wo du sie bearbeiten und löschen kannst.

Ein Widget besteht aus ein paar Dateien. Es beginnt in `index.html`, das die
übrigen beim Namen einbindet, etwa `<link rel="stylesheet" href="style.css">`,
`<script src="app.js"></script>` oder `<img src="dot.svg">`, und
`fetch("data.json")` liest eine eigene Datei. Ein neues Widget beginnt mit
`index.html`, `style.css` und `app.js`; **Datei hinzufügen** legt eine
weitere an (`.html`, `.css`, `.js`, `.json`, `.svg`, `.txt` oder `.md`).
Skripte sind klassische Skripte: Mehrere lädst du mit mehreren
`<script src>`-Tags in Reihenfolge, nicht mit `import` untereinander.

Der Editor ergänzt beim Tippen: das `losos`-Objekt und seine Mitglieder, die
Namen der Messwerte in `losos.metric("`, die Farbvariablen nach `var(--` und
die Dateinamen des Widgets überall, wo eine Datei genannt wird. Strg+Leertaste
zeigt überall Vorschläge.

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
| `losos.files`                  | Die Namen der Dateien des Widgets.                                   |
| `losos.file(name)`             | Eine Datei des Widgets als Text.                                     |
| `losos.asset(name)`            | Eine Datei des Widgets als `data:`-URL, für ein Bild, das ein Skript setzt. |

Der Tab **So funktioniert es** im Editor wiederholt dies mit einem Beispiel. Seine Vorschau
ist der echte Frame, was sie zeigt, zeigt also auch die Kachel. Ein Widget
kann Daten aus dem Internet abrufen (etwa eine Wetterkachel), aber nicht von
der Box. Die Grenze liegt bei 24 Widgets mit je höchstens 12 Dateien und
zusammen 128 KiB.

Der Editor hat auch einen Tab **Mit KI bauen**, in dem ein Sprachmodell auf
der Box das Widget nach einer Beschreibung schreibt und seine Dateien in den
Editor legt, damit du sie vor dem Speichern liest. Es läuft auf der Box, also
bleibt, was du eintippst, dort, und ein Bau kostet nichts; siehe
[Mit KI bauen](widget-builder.md).

## Admin-Token {#admin-token}

Die Box erzeugt beim ersten Start ein zufälliges Token aus 64
Hexadezimalzeichen und bewahrt es versiegelt in ihrem Speicher für
Zugangsdaten auf, als `admin-token` (siehe
[Zugangsdaten](../reference/credentials.md)); `lososd` liest es aus seinem
eigenen Verzeichnis für Zugangsdaten. Das Token kann jede Einstellung ändern,
es ist also root.

Auf einer neuen, noch nicht beanspruchten Box setzt der erste Aufrufer aus
dem LAN, der `POST /api/setup/claim` sendet, das Eigentümer-Passwort, wird
die erste Person, die sich anmelden kann (ein Administrator), und erhält das
Token in der Antwort. Diese Route verlangt kein Token, nimmt aber
keine Ansprüche mehr an, sobald die Box einen Eigentümer hat. Richte die
Box nur in einem vertrauenswürdigen LAN ein, denn der erste Aufrufer wird ihr
Eigentümer. Das Token wird nur einmal herausgegeben und kann über die
Oberfläche nicht erneut abgerufen werden.

Vor dem ersten Passwort muss nichts starten, deshalb antwortet
`GET /api/setup/claim` auf einer unbeanspruchten Box mit `ready: true`, und
der Einrichtungsassistent geht sofort weiter. Eine Anfrage, die einen Dienst
braucht, der noch bei seinem ersten Start ist (etwa LosOS Git), wird mit
`503`, `{"error": …, "ready": false, "waitingFor": …}` und einem
`Retry-After` beantwortet und ändert nichts.

Dieser Aufrufer ist der Einrichtungsassistent. Danach melden sich Personen
bei LosOS Login an, der Anmeldeseite der Box unter `https://<box>/auth/`,
jede mit einem eigenen Anmeldenamen und Passwort. LosOS Login ist ein
Open-Source-Anmeldeserver, der auf der Box läuft. lososd führt die
Personendatei, die LosOS Login liest, mit jedem Passwort als argon2id-Hash,
und LosOS Login übernimmt eine Änderung daran sofort. nginx fragt LosOS
Login bei jeder Anfrage an `/api/` und gibt den Namen der Person an lososd
weiter, das die Bereiche öffnet, die ihre Gruppen erlauben. Die erste Person
ist immer Administrator. Ihr eigenes Passwort ändert eine Person mit `POST
/api/set-password`, und beim Eigentümer ändert das auch das Passwort für
LosOS Cloud. Alle anderen fügt ein Administrator im Bereich Benutzer hinzu,
ändert und entfernt sie, über `GET` und `POST /api/people` und
`POST /api/people/{access,password,remove}`. Den Bereich gibt es nur, solange
die Box Personen selbst anmeldet (`standalone` in `GET /api/me`). lososd
weigert sich, den Eigentümer, das eigene Konto des Aufrufers oder den letzten
Administrator anzufassen. LosOS Login schlägt jede Person bei jeder Anfrage
neu nach, daher erreicht eine Entfernung oder eine geänderte Gruppe auch die
Apps beim nächsten Klick.
Die Anmeldungen liegen in einem eigenen Redis von LosOS Login, auf einem
Socket, den nur es öffnet, daher meldet weder ein Neustart von LosOS Login
noch der nächtliche Neustart jemanden ab.

LosOS Cloud und LosOS Git melden Menschen genauso an. nginx fragt LosOS
Login und gibt die Person in drei Headern weiter, `X-Losos-Cloud-User`,
`-Name` und `-Email`, die es bei jeder Anfrage an eine der Apps setzt, damit
kein Client eigene schicken kann. Der Eigentümer, in der Gruppe
`losos-owner`, kommt in beiden als Konto `notshared` an; alle anderen unter
ihrem Anmeldenamen. LosOS Cloud (`backend/src/cloud/auth.rs`) und LosOS Git
(`backend/src/forge/auth.rs`) lesen die Header selbst, und beide legen beim
ersten Mal ein Konto an; danach hält lososd Namen und Adresse in beiden Apps
mit der Personendatei gleich. Eine Anfrage an LosOS Git, die ein eigenes
Token in `Authorization` mitbringt (git, die API), fragt nginx nicht nach.
Wen LosOS
Login nicht kennt, den schickt nginx dorthin, aus dem LAN wie über den Edge;
keine der beiden Apps zeigt eine eigene Anmeldeseite. Über den Edge liefert
der Tunnel von Loopback mit `X-Forwarded-Proto: https`, was nginx nur von
Loopback glaubt, und lososd nimmt jeden öffentlichen Namen, unter dem die
Box antwortet, in die Cookie-Domains von LosOS Login auf. Solange LosOS
Login startet, antworten die Apps mit 503 und einer Seite, die es von selbst
erneut versucht. Beide Apps lesen die Header nur auf ihren Unix-Sockets,
`/run/losos-cloud/http.sock` und `/run/losos-git/http.sock`, die nur root
und nginx öffnen dürfen; der Loopback-Port von LosOS Git, `127.0.0.1:3000`,
nimmt Tokens und nie die Header. So kann nichts auf der Box einer der beiden Apps eine Person
nennen. Die Namen `notshared`, `shared` und `losos` sind für die
eigenen Konten der Box reserviert.

Jede Person mit der Gruppe `losos-account` (Eigenes Konto im Bereich
Benutzer) pflegt ihren eigenen Namen, ihre E-Mail-Adresse und ihr Passwort
über `GET` und `POST /api/account`. Authenticator-App und Passkeys hält
LosOS Login in seiner eigenen Datenbank; sie hinzuzufügen oder zu entfernen
verlangt einen Einmalcode, den es sonst per Mail schicken würde, also
schreibt sein Notifier den Code in eine Datei, und `POST /api/account/code`
gibt ihn hinter dem Passwort der Person heraus. Ein Administrator setzt mit
`POST /api/people/rules` zwei Regeln: Zwei-Faktor-Anmeldung für alle und
Passwörter, die nach einer Zahl von Tagen ablaufen. lososd markiert die
Personendatei mit zwei Gruppen: `losos-two-factor` für alle mit einem
zweiten Faktor, nach dem LosOS Login bei jeder Anmeldung fragt, und
`losos-locked` für alle, die eine Regel erfasst. LosOS Login verweigert
einer gesperrten Person LosOS Cloud und LosOS Git, und lososd antwortet ihr
nur zu ihrem Konto (`locked` in `GET /api/me`). lososd lehnt die
Zwei-Faktor-Regel ab, solange der Administrator, der sie setzt, keinen
zweiten Faktor hat, und der Ersatzschlüssel wird nie gesperrt.
`POST /api/people/two-factor/reset` entfernt die zweiten Faktoren einer
Person.

Das Token selbst wird nach dem Setzen des Passworts trotzdem einmal als
**Ersatz-Admin-Schlüssel** angezeigt, mit den Schaltflächen Kopieren und
Drucken. Eine Anfrage, die ihn trägt, umgeht LosOS Login, und lososd prüft
den Schlüssel selbst, also öffnet der Schlüssel die Admin-Seiten auch dann,
wenn LosOS Login nicht läuft. Der Anmeldedialog hat einen Link, um
stattdessen den Schlüssel einzugeben. Eine Box, die eingerichtet wurde,
bevor Personen Anmeldenamen hatten, fragt einmal nach dem Schlüssel,
zusammen mit einem Anmeldenamen und einem Passwort, um ihren ersten
Administrator anzulegen.

Ein neues Passwort muss mindestens 12 Zeichen lang sein, mit einem
Kleinbuchstaben, einem Großbuchstaben, einer Ziffer und einem Symbol wie `-`
oder `!`. Der Assistent zeigt während der Eingabe die vier Regeln mit je
einem Haken.

:::warning[Passwort und Ersatzschlüssel verloren]
Das Token lässt sich auf der Box nicht rotieren, da sie keine Shell hat. Wer
das Passwort *und* den Ersatzschlüssel verliert, muss vom
Installationsmedium neu installieren, was die Festplatte löscht.
:::

## Einstellungen {#settings}

Einstellungen sind Nix. Die Einstellungsseite erzeugt `modules/overrides.nix`
und führt `nixos-rebuild switch` aus. Daher:

- Eine Änderung braucht einen Rebuild, keinen Neustart.
- Die Box lässt sich aus diesem Repository plus dieser einen Datei neu
  bauen.
- `apply` ersetzt `overrides.nix` vollständig. Eine Einstellung, die die
  Oberfläche nicht schreibt, wird beim nächsten Speichern zurückgesetzt.

### Erweitert {#advanced}

**Einstellungen, Erweitert** listet jede `losos.*`-Option, die die Box deklariert,
gelesen aus den Modulen, die sie ausführt. `nix/options-doc.nix` erzeugt
das Dokument beim Build, und lososd liefert es unter `GET /api/options` aus,
mit dem aktuellen `overrides.nix` eingefügt. Jede Zeile zeigt die
Beschreibung der Option, ihren Standardwert, den Wert, mit dem die Box
läuft, und einen Editor für ihren Typ: einen Schalter, eine Zahl mit ihren
Grenzen, eine Auswahl, ein Textfeld oder ein Element pro Zeile für eine
Liste.

- Eine Option, die schon ein anderer Bereich bearbeitet (der Name, die
  Mesh-Einstellungen, die Hardening-Schalter), wird mit ihrem Wert und
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

### Historie und LosOS Git {#history-and-losos-git}

Jedes Apply, jede Speicheränderung und jedes Zurücksetzen auf
Werkseinstellungen ist ein Commit in `/etc/nixos`, dem Repository, das
`losos-ctl install` angelegt hat. Der Commit ist nach den Einstellungen
benannt, die er geändert hat, `Change hostName, cluster.enable`, mit dem
Vorher und Nachher jeder Einstellung im Body. **Einstellungen, Historie** listet
die letzten vierzig.

Mit `losos.configRepo.enable` (Standard) und eingeschaltetem LosOS Git führt
lososd die Konfiguration außerdem in einem privaten Repository auf LosOS
Git, `<owner>/losos-config`, das dem eigenen Konto des Admins (`notshared`)
gehört. Das Konto wird bei der ersten Synchronisierung als Administrator
von LosOS Git angelegt. Ein Reconciler in lososd läuft alle 30 s:

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
authentifiziert, ist das Admin-Token, das LosOS Git beim ersten Start
schreibt, `/var/lib/losos-git/admin-token`, außerhalb von `/etc/nixos`.
Dahinter steht kein CI-Runner. Die Box fragt regelmäßig ab.

Die Klon-Adresse wird im Bereich angezeigt (`http://<box>/git/<owner>/
losos-config.git`). Das Repository ist privat, ein Rechner braucht daher
ein eigenes Token (`losos git setup`, [Der Terminal-Client](../manual/terminal)).
lososd spricht mit LosOS Git über dessen Loopback-Port mit dem Admin-Token
und handelt mit `X-Losos-As` als Eigentümer, wo ein Aufruf dessen ist.

### Umzug von Forgejo {#moving-from-forgejo}

Eine Box, auf der Forgejo lief, bringt dessen Einträge selbst nach LosOS
Git, beim ersten Start nach dem Update und bevor LosOS Git antwortet:
Repositorys, Konten, Mitarbeitende, Zugriffstokens (die alten Werte
funktionieren weiter für git und die API), Labels, Issues, Pull Requests,
Kommentare, Zuständige und Anhänge. Aus einer Organisation wird ein Konto
mit demselben Namen.

Der Bericht ist `/var/lib/losos-git/forgejo/import.json`. Er zählt, was
Forgejo hatte und was übernommen wurde, Tabelle für Tabelle, und nennt,
was nicht übernommen wurde, etwa Meilensteine, Releases und
Zeitleisteneinträge. Daneben bleibt ein vollständiger Dump der Datenbank
von Forgejo (`forgejo.sql`, auf einer nativen Box `forgejo.db`), und der
Ordner von Forgejo sowie die Postgres-Daten bleiben, wo sie waren. Schlägt
der Umzug fehl, ändert sich nichts, und er wird beim nächsten Start erneut
versucht; LosOS Git bedient die Repositorys solange.

### Box-weite App-Einstellungen {#box-wide-app-settings}

Der Bereich Apps trägt die Einstellungen, die für alle in LosOS Cloud und
LosOS Git gelten, also die, die jede App sonst auf ihren eigenen
Verwaltungsseiten führt. Jedes Feld ist eine Option, `losos.cloud.trashDays`
oder eine unter `losos.git.site.*`, und geht damit wie jede
andere Einstellung durch Anwenden, die Prüfung, den Verlauf und das
Konfigurations-Repository. Der Bereich Erweitert zeigt ihre Werte und
verweist auf den Abschnitt der App im Bereich Apps (`/apps#cloud`,
`/apps#git`), statt einen zweiten Editor zu zeichnen (`ON_PANE` in
`admin-ui/app/src/lib/option-value.ts`).

| Option | Was sie in der App setzt |
| --- | --- |
| `losos.cloud.trashDays` | wie viele Tage eine gelöschte Datei im Papierkorb bleibt (1 bis 365, Standard 30) |
| `losos.git.site.requireSignIn` | `losos-git serve --require-sign-in` |
| `losos.git.site.defaultPrivate` | `--public-by-default` bei `"public"` |
| `losos.git.site.pushCreate` | `--push-create` |

`modules/cloud.nix` gibt `trashDays` als `--trash-days` an den Dienst, und
dessen stündliches Aufräumen löscht endgültig, was länger im Papierkorb
einer Person liegt.

Die drei von LosOS Git sind Schalter auf seiner Befehlszeile
(`modules/git.nix`), daher gibt es darin keine Seite, auf der man sie ändern
könnte.

:::note[Was in den Apps bleibt]
Alles, was ein Konto für sich selbst einstellt, bleibt in der App. In LosOS
Git ist das Konto des Besitzers der Administrator. LosOS Cloud hat
überhaupt keinen Administrator in sich: Wer es nutzen darf, entscheidet
LosOS Login, und alles Box-weite ist eine Einstellung im Bereich Apps.
:::

## Sicherung, Wiederherstellung und Löschen {#backup-restore-and-erase}

**Einstellungen, Sicherung** sendet die Daten der Box in einen S3-kompatiblen
Bucket, den der Eigentümer mietet (Amazon S3, Glacier eingeschlossen,
Backblaze B2, Wasabi, Cloudflare R2, MinIO).
[restic](https://restic.net) übernimmt das Kopieren und verschlüsselt auf der
Box, bevor irgendetwas sie verlässt, der Bucket enthält also nur Chiffretext.
Das Repository-Passwort ist der Wiederherstellungscode der Box
(`/var/secrets/losos-recovery-code`), den der Bereich auf Anfrage anzeigt;
ohne ihn kann niemand ein Backup öffnen.

Ein Backup enthält:

- `/var/lib/losos-cloud` und `/var/lib/losos-git`, wobei die
  SQLite-Datenbanken von LosOS Cloud und LosOS Git zuerst mit dem eigenen
  Backup von SQLite kopiert werden, damit ein Schreibvorgang während der
  Kopie sie nicht zerreißen kann;
- den geteilten Ordner. Bei eingeschaltetem Teilen ist er eine
  fscrypt-Policy, und Linux liefert für eine gesperrte fscrypt-Datei keinen
  Chiffretext, daher entsperrt das Skript die Policy für die Kopie und
  sperrt sie danach wieder. Im Bucket deckt ihn die Verschlüsselung von
  restic ab;
- `modules/overrides.nix`, das Aussehen der Startseite, das Proxy-Token und
  die Personendatei, die LosOS Login liest.

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

Die Seite bietet auch jede Speicherbestellung an, die die Box auf dem
[Marktplatz](/in-depth/market.md) gekauft hat, sobald ihr Volume gebunden
ist. Backups gehen dann über die Edge, die sie verkauft hat, per restics
REST-Protokoll an `/market/storage/<order>/`, mit derselben Verschlüsselung
wie bei einem Bucket. Die Edge öffnet das Volume nur für einen Schlüssel,
den die Box aus ihrem Wiederherstellungscode und der Bestell-ID macht
(`POST /api/backup/target/bought`). Weder die Edge noch die Boxen mit den
Replikaten des Volumes können also ein Backup lesen, und eine neue Box mit
demselben Code kann daraus wiederherstellen. Die Box speichert für dieses
Ziel kein Geheimnis: Der Backup-Job macht den Schlüssel jedes Mal neu aus
dem Code.

Der Bucket behält die letzten sieben. lososd führt jedes Backup und jede
Wiederherstellung als transiente Unit `losos-backup-<job>` aus, die systemd
außerhalb der `ProtectHome=true`-Sandbox von lososd startet. Die Schlüssel
des Buckets sind im Speicher für Zugangsdaten versiegelt und erreichen die
Unit über `LoadCredentialEncrypted=`, nie über eine Befehlszeile oder eine
einfache Datei.

**Wiederherstellen** verlangt den Wiederherstellungscode der Box, die das Backup
erstellt hat. Der neueste Snapshot kommt zurück: Die Apps halten an, die
Dateien werden zurücksynchronisiert, die Datenbanken geladen (mit
`pg_restore --clean` auf einer Box, die noch PostgreSQL betreibt), die Apps starten, und das wiederhergestellte
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
3. Verlassen der Edge: Die eigenen Domains der Box werden entfernt, eine an
   ihre Namen gebundene Wallet wird gelöst, ihre aktiven Angebote auf dem
   Marktplatz geschlossen, und sie wird aus der
   Registry ausgetragen (`POST /deregister`). Jeder Schritt wird versucht,
   auch wenn der vorherige fehlschlug, und der Bericht nennt die, die die
   Edge nicht bestätigt hat;
4. die Standardeinstellungen werden committet und neu gebaut, dann
   hinterlässt lososd eine Markierung auf `/persist` und startet neu;
5. `losos-factory-wipe.service` läuft früh im nächsten Bootvorgang, vor
   `sysinit.target` und vor jedem Dienst, dem die Daten gehören, und löscht
   die App-Daten, die Datenbanken, den Zustand beider Kubernetes-Instanzen,
   die Personendatei und den Zustand von LosOS Login, die Geheimnisse, das
   Journal und die fscrypt-Metadaten. Er leert beide Daten-Homes. Die
   Markierung wird zuletzt entfernt, sodass ein
   unterbrochenes Löschen erneut läuft.

:::danger[Abbrechen nur in den ersten zwei Schritten]
Abbrechen ist nur in den Schritten 1 und 2 möglich. `/nix`, `/etc/nixos`
und die Schlüsseldatei der Festplatte bleiben, LosOS ist also weiterhin
installiert, und die Box öffnet den Einrichtungsassistenten.
`/var/lib/losos-erase/report.json` hält fest, was das Löschen außerhalb der
Box aufgegeben hat, als Zählwerte, und Einstellungen, Zurücksetzen zeigt es an.
`nix/tests/erase.nix` durchläuft den ganzen Zyklus gegen einen MinIO-Bucket in
einer VM.
:::

## Benutzer {#users}

Zwei Datenbenutzer, beide ohne Passwort und ohne Shell:

| Benutzer    | Besitzt             |
| ----------- | ------------------- |
| `notshared` | Die Apps des Eigentümers; seine Dateien in LosOS Cloud tragen diesen Namen |
| `shared`    | Mesh-Speicher       |

Jeder hat seine eigene primäre Gruppe und ein Home mit Modus 700, sodass
keiner die Dateien des anderen lesen kann. `nix/tests/impermanence.nix` prüft
das.

## Updates und Neustarts {#upgrades-and-reboots}

- **03:00.** lososd bringt `/etc/nixos` zuerst auf den neuesten Commit von
  `losos.configRepo.upstream.branch` (dem Release-Kanal, unten) unter
  `losos.configRepo.upstream.url` (dem GitHub-Repository) und behält dabei
  `modules/overrides.nix` und `modules/install-target.nix` der Box. Dann
  baut `system.autoUpgrade` aus `losos.upgradeFlakeUri` neu, also aus
  `git+file:///etc/nixos#install`. Behalte das Fragment `#install`; ohne es
  schlägt der Rebuild fehl. Wurde seit dem letzten Update eine andere Datei
  in `/etc/nixos` geändert, behält die Box sie und aktualisiert nicht; der
  Verlauf-Bereich sagt es. `losos-ctl config --update` führt den ersten
  Schritt von Hand aus.
- **00:07.** `midnight-reboot.timer` startet bedingungslos neu
  (`Persistent=true`, sodass eine Box, die aus war, das nachholt). Da die
  Wurzel beim Booten neu aufgebaut wird, ist der Neustart der Weg, auf dem
  sich die Box selbst repariert.

### Release-Kanäle {#release-channels}

Eine Box folgt einem von drei Branches, gewählt unter Einstellungen,
Verlauf, Release-Kanal (`losos.updates.channel`):

| Kanal        | Branch            | Rückt vor                  |
| ------------ | ----------------- | -------------------------- |
| Täglich      | `channel/daily`   | Jede Nacht um 00:17 UTC    |
| Wöchentlich  | `channel/weekly`  | Montags um 00:17 UTC       |
| Monatlich    | `channel/monthly` | Am 1. um 00:17 UTC         |

Jeder Schritt nimmt den neuesten Commit von `main`, dessen CI-Lauf bestanden
hat, den Upload in den Binär-Cache eingeschlossen. So baut eine Box nie eine
Version, die ihre Prüfungen nicht bestanden hat, und kompiliert nie, was CI
schon gebaut hat. Vor LosOS 1.0.0 folgt eine Box dem täglichen Kanal, danach
dem wöchentlichen. „Jede Änderung“ (`main`) folgt `main` selbst, geprüft
oder nicht, und ist zum Testen von LosOS.

Ein Pull Request mit dem Label `hotfix` erreicht jeden Kanal am selben Tag.
Sobald er gemergt ist, überträgt ein Bot ihn per Cherry-Pick auf jeden Kanal,
dem er fehlt, auf dem Ergebnis laufen CI und der Cache-Upload, und der Kanal
rückt vor, wenn beide bestehen. Boxen holen ihn beim nächsten Update um
03:00. Ein Hotfix trägt, was in diesem Repository liegt: den eigenen Code von
LosOS, seine Rust- und npm-Abhängigkeiten und seine Moduleinstellungen. Eine
Korrektur in NixOS selbst, etwa in nginx oder im Kernel, kommt wie jede
andere Änderung über den nixpkgs-Pin.

Der Wechsel auf einen langsameren Kanal bringt eine Box nie zurück. Jeder
Commit eines Kanals nennt den `main`-Commit, aus dem er geschnitten wurde
(eine Zeile `Channel-Base:`), und die Box behält ihre Version, bis der Kanal
sie überholt; der Verlauf-Bereich zeigt so lange „Wartet auf den Kanal“.
`.github/workflows/channels.yml` und `.github/scripts/channels.py` erledigen
die Schritte und die Hotfixes, und kein Kanal wird je per Force-Push
überschrieben.
