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
| Forgejo (optional)  | `<host>.local/forgejo/`   |
| Handbuch            | `<host>.local/handbook/`  |
| [Lab](/in-depth/lab.md)       | `<host>.local/lab/`       |

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
[Widget-Baukasten](widget-builder.md).

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

LosOS Cloud und LosOS Git melden Menschen genauso an. nginx fragt auf der Anmeldeseite jeder App
LosOS Login und gibt die Person in drei Headern weiter, `X-Losos-Cloud-User`,
`-Name` und `-Email`, die es bei jeder Anfrage an eine der Apps setzt, damit
kein Client eigene schicken kann. Der Eigentümer, in der Gruppe
`losos-owner`, kommt in beiden als Konto `notshared` an; alle anderen unter
ihrem Anmeldenamen. LosOS Cloud liest die Header selbst
(`backend/src/cloud/auth.rs`), Forgejo mit seiner Anmeldung über den Reverse
Proxy, und beide legen beim ersten Mal ein Konto an; danach hält lososd
Namen und Adresse in beiden Apps mit der Personendatei gleich. Wen LosOS
Login nicht kennt, den schickt nginx dorthin, aus dem LAN wie über den Edge;
keine der beiden Apps zeigt eine eigene Anmeldeseite. Über den Edge liefert
der Tunnel von Loopback mit `X-Forwarded-Proto: https`, was nginx nur von
Loopback glaubt, und lososd nimmt jeden öffentlichen Namen, unter dem die
Box antwortet, in die Cookie-Domains von LosOS Login auf. Solange LosOS
Login startet, antworten die Apps mit 503 und einer Seite, die es von selbst
erneut versucht. Beide Apps lauschen auf Loopback, und die Firewall
weist eine Verbindung zu ihren Ports von jedem Prozess auf der Box ab außer
root, nginx und der App selbst, sodass nichts auf der Box ihnen eine Person
nennen kann. Die Namen `notshared`, `shared` und `losos` sind für die
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
gehört. Das Konto wird bei der ersten Synchronisierung mit dem
Admin-Passwort angelegt und bei jeder Passwortänderung und jedes Mal, wenn
ein Rechner für LosOS Git eingerichtet wird, damit abgeglichen. Ein
Reconciler in lososd läuft alle 30 s:

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
Token ausstellt (`nix/forgejo-bootstrap.nix`).

### Föderation {#federation}

Die Föderation läuft nur, solange die Box ihre Festplatte teilt.
`losos.forgejo.federation.enable` ist per UND mit `losos.sharingMyStorage`
verknüpft, der Einstellung, die den geteilten Datenpool freischaltet
(`lososInternal.federation` in `modules/options.nix`). Mit ausgeschaltetem
Teilen spricht LosOS Git mit keinem anderen Server, egal was die eigene
Option sagt. Der Bereich Apps graut den Schalter mit dem Grund aus, und der
Bereich Erweitert zeigt die Zeile als nicht verfügbar an und verweist auf
sie. LosOS Cloud föderiert nicht: Eine Datei verlässt die Box über einen
Freigabelink oder das Konto einer Person, nie als Freigabe an einen anderen
Server.

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
`https://<host>.local/forgejo/`, sodass das nur andere Boxen im selben LAN
können. `nix/tests/forgejo-federation.nix` startet das echte Forgejo und fragt
es ab, mit eingeschaltetem und dann ausgeschaltetem Teilen.

### Box-weite App-Einstellungen {#box-wide-app-settings}

Der Bereich Apps trägt die Einstellungen, die für alle in LosOS Cloud und
LosOS Git gelten, also die, die jede App sonst auf ihren eigenen
Verwaltungsseiten führt. Jedes Feld ist eine Option, `losos.cloud.trashDays`
oder eine unter `losos.forgejo.site.*`, und geht damit wie jede
andere Einstellung durch Anwenden, die Prüfung, den Verlauf und das
Konfigurations-Repository. Der Bereich Erweitert zeigt ihre Werte und
verweist auf den Abschnitt der App im Bereich Apps (`/apps#cloud`,
`/apps#git`), statt einen zweiten Editor zu zeichnen (`ON_PANE` in
`admin-ui/app/src/lib/option-value.ts`). Der Föderationsschalter von
oben steht dort auch, am Anfang des Abschnitts LosOS Git.

| Option | Was sie in der App setzt |
| --- | --- |
| `losos.cloud.trashDays` | wie viele Tage eine gelöschte Datei im Papierkorb bleibt (1 bis 365, Standard 30) |
| `losos.forgejo.site.requireSignIn` | `[service] REQUIRE_SIGNIN_VIEW` |
| `losos.forgejo.site.keepEmailPrivate` | `[service] DEFAULT_KEEP_EMAIL_PRIVATE` |
| `losos.forgejo.site.defaultPrivate` | `[repository] DEFAULT_PRIVATE` |
| `losos.forgejo.site.pushCreate` | `[repository] ENABLE_PUSH_CREATE_USER` |
| `losos.forgejo.site.landingPage` | `[server] LANDING_PAGE` |

`modules/cloud.nix` gibt `trashDays` als `--trash-days` an den Dienst, und
dessen stündliches Aufräumen löscht endgültig, was länger im Papierkorb
einer Person liegt.

Die fünf von Forgejo sind `app.ini`-Schlüssel, die Forgejos
Website-Administration ohne Möglichkeit zur Änderung zeigt. Beide Modi lesen
eine Attributmenge, `lososInternal.forgejoSite`.

:::note[Was in den Apps bleibt]
Alles, was ein Konto für sich selbst einstellt, bleibt in der App. In LosOS
Git bleibt das Konto des Besitzers Forgejos Administrator für das, was LosOS
noch nicht abdeckt: Forgejos in der Datenbank gespeicherte Einstellungen und
die Kontenliste. LosOS Cloud hat überhaupt keinen Administrator in sich:
Wer es nutzen darf, entscheidet LosOS Login, und alles Box-weite ist eine
Einstellung im Bereich Apps.
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

- `/var/lib/losos-cloud` und `/var/lib/forgejo`, wobei die Datenbank von
  LosOS Cloud zuerst mit dem eigenen Backup von SQLite kopiert wird, damit
  ein Schreibvorgang während der Kopie sie nicht zerreißen kann;
- einen `pg_dump` der Datenbank `forgejo`;
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

Der Bucket behält die letzten sieben. lososd führt jedes Backup und jede
Wiederherstellung als transiente Unit `losos-backup-<job>` aus, die systemd
außerhalb der `ProtectHome=true`-Sandbox von lososd startet. Die Schlüssel
des Buckets sind im Speicher für Zugangsdaten versiegelt und erreichen die
Unit über `LoadCredentialEncrypted=`, nie über eine Befehlszeile oder eine
einfache Datei.

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
  `losos.configRepo.upstream.branch` (`main`) unter
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
