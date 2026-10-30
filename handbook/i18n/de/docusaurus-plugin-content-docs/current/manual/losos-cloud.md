---
title: LosOS Cloud
sidebar_position: 4.4
slug: /manual/losos-cloud
---

# LosOS Cloud

![Die Dateien in LosOS Cloud: der Ordner Documents, links die Orte.](@site/docs/img/cloud-files-de.png)

LosOS Cloud ist der eigene Dateidienst der Box: deine Dateien im Browser,
ein Netzwerkordner für den Dateimanager, ein Server, mit dem sich die
Nextcloud-Apps für Computer und Telefon synchronisieren, und deine Kalender
und Kontakte für jede Kalender-App.

Öffne ihn unter `https://<name>.local/cloud/`. `<name>` ist der Name der
Box, wie er auf ihrem Bildschirm steht. Du meldest dich mit LosOS Login an,
und jeder, der sich anmelden kann, hat eigene Dateien. Wer die Box
eingerichtet hat, bekommt die des Besitzers.

## Im Browser \{#in-the-browser}

LosOS Cloud sitzt im selben Rahmen wie die anderen Seiten der Box: dieselbe
Leiste oben und dieselbe Seitenleiste, seine Orte unter **Cloud**.

- **Dateien** zeigt deine Ordner. Zieh Dateien oder ganze Ordner auf die
  Seite, um sie hochzuladen, oder nimm **Hochladen**. Eine Datei, die auf
  anderem Weg dazukommt, über die Netzwerkfreigabe oder aus der
  Desktop-App, erscheint nach ein paar Sekunden von selbst im offenen
  Ordner. Markierte Dateien
  kannst du herunterladen, umbenennen, verschieben, kopieren oder löschen,
  oder ihre früheren Versionen ansehen.
- **Zuletzt** zeigt, was sich zuletzt geändert hat, **Favoriten**, was du
  mit einem Stern markiert hast.
- **Fotos** zeigt alle Bilder nach dem Aufnahmedatum und die Personen
  darauf. Siehe [Fotos](#photos).
- **Papierkorb** behält gelöschte Dateien 30 Tage oder so lange, wie es
  der Bereich **Apps** sagt. Stell eine wieder her oder lösche sie
  endgültig. Was du über die
  [Netzwerkfreigabe](files-on-your-computer#the-network-share) löschst,
  landet nicht darin.
- **Geräte** listet die Computer und Telefone, die synchronisieren, und
  meldet eins ab.
- <kbd>Strg</kbd>+<kbd>Umschalt</kbd>+<kbd>P</kbd>, ein Doppelpunkt
  außerhalb eines Textfelds oder die Lupe oben in der Seitenleiste öffnet
  die Suche. Sie findet Dateien nach Namen und nach dem Text darin, auf
  jeder Seite der Box und auch in einem offenen Dokument.
- Textdateien öffnen sich in einem Editor, in den mehrere Leute
  gleichzeitig schreiben, und **Mit Ihnen geteilt** enthält, was Leute auf
  anderen Boxen mit dir geteilt haben. Siehe
  [Teilen und gemeinsam bearbeiten](sharing-and-editing-together).
- **Notizen** liest einen Ordner mit Markdown-Notizen wie Obsidian, mit
  Links, Tags und einem Graphen, und kann ihn als Wissensbasis
  veröffentlichen. Siehe [Notizen und Wissensbasen](notes-and-knowledge-bases).
- Ein Rechtsklick auf eine Datei, einen Ordner oder die Fläche darum zeigt,
  was du dort tun kannst; siehe
  [das Rechtsklick-Menü](admin-pages#the-right-click-menu).
- Das Rechtsklick-Menü einer Datei gibt sie auch an die anderen Apps
  weiter: **Per Mail senden** und **Im Chat senden**, solange die Box Mail
  und Chat hat, und **Die KI dazu fragen** bei einer Textdatei auf einer Box
  mit Sprachmodell. Die KI läuft auf der Box und liest nur die gewählte
  Datei.

## Fotos \{#photos}

**Fotos** zeigt jedes Bild aus jedem Ordner, das neueste zuerst, nach dem
Monat, in dem die Kamera es aufgenommen hat. **Gruppieren nach** ordnet sie
stattdessen nach Person, Kamera oder Ort. Klick auf ein Bild, um es groß zu
sehen, blättere mit den Pfeiltasten weiter und drück **Details** (oder
<kbd>i</kbd>) für das, was die Kamera gespeichert hat: wann, welche Kamera
und welches Objektiv, die Belichtung, die Größe, wo es aufgenommen wurde,
wer darauf ist und jede weitere Angabe der Kamera.

![Fotos in LosOS Cloud: alle Bilder nach Monat.](@site/docs/img/cloud-photos-de.png)

Die Box findet die Gesichter auf deinen Bildern und fasst die, die sich
ähneln, zu einer Person zusammen. **Personen** über den Fotos zeigt alle,
die auf mehr als einem Bild vorkommen; klick auf ein Gesicht, um alle Fotos
dieser Person zu sehen und ihr einen Namen zu geben. Gibst du zwei Personen
denselben Namen, werden sie zu einer.

Das alles geschieht auf der Box, im Hintergrund und mit der niedrigsten
Priorität, ein paar Minuten nachdem ein Bild angekommen ist: Zwei kleine
Modelle finden und vergleichen Gesichter auf dem Prozessor der Box, und
nichts über ein Bild, ein Gesicht oder einen Ort wird irgendwohin
geschickt. Nur du siehst die Gesichter, Namen und Orte deiner Fotos; andere
Personen auf der Box sehen nichts davon.

## Apps für Computer und Telefon \{#desktop-and-phone-apps}

Der Nextcloud-Desktop-Client und die Nextcloud-Apps für Android und iOS
funktionieren mit LosOS Cloud. Gib ihnen die Adresse der Box ohne Pfad:

```text
https://<name>.local
```

Die App öffnet LosOS Login im Browser. Melde dich an, drück auf der
folgenden Seite **Erlauben**, und die App bekommt ein eigenes Passwort.
Danach steht sie unter **Geräte**, wo du sie abmelden kannst, ohne dein
Passwort für LosOS Login zu ändern.

![Die Seite, die eine Desktop-App zum Anmelden öffnet: App verbinden, mit Erlauben und Jetzt nicht.](@site/docs/img/cloud-connect-de.png)

## Kalender und Kontakte \{#calendars-and-contacts}

Kalender- und Kontakt-Apps (Thunderbird, GNOME Kalender, DAVx⁵ unter
Android, die Konten des iPhones) finden deine Kalender und Adressbücher über
die Adresse der Box:

```text
https://<name>.local
```

Melde dich mit deinem Namen für LosOS Login und einem App-Passwort aus der
Anmeldung der App für Computer oder Telefon an, oder im Netz der Box mit
deinem Passwort für LosOS Login. Eine Kalender-App, die die ganze Adresse
will, bekommt `https://<name>.local/remote.php/dav/`.

Im Browser findest du dieselben Kalender im [Kalender](calendar) und die
Adressbücher in [Kontakte](contacts).

## Im Dateimanager \{#in-the-file-manager}

Dieselben Dateien sind eine WebDAV-Freigabe unter dieser Adresse:

```text
https://<name>.local/remote.php/webdav/
```

Im Netz der Box zeigen GNOME Files und Dolphin sie im Netzwerk als **LosOS
Cloud on `<name>`**. Windows und macOS fügen sie über die Adresse hinzu,
wie in [Dateien auf deinem Computer](files-on-your-computer). Installiere vorher das Zertifikat der
Box, wie im [ersten Start, Schritt 1](../start/first-run#step-1-trust-this-box).

Im Netz der Box nimmt die Freigabe deinen Namen und dein Passwort für LosOS
Login. Über das Internet nimmt sie nur das eigene Passwort einer App, nimm
dort also die App für Computer oder Telefon.

## Eine Box, auf der Nextcloud lief \{#a-box-that-ran-nextcloud}

Vor LosOS Cloud hat eine Box deine Dateien in Nextcloud gehalten. Beim
ersten Start ohne Nextcloud zieht die Box alles von selbst um, bevor LosOS
Cloud öffnet: die Dateien jedes Kontos mit ihrem Papierkorb, Kalender,
Kontakte und Aufgaben, die Boards aus Deck und die Passwörter, die die Apps
für Computer und Telefon schon hatten, sodass sie ohne neue Anmeldung
weiter synchronisieren. Apps, die mit der alten Adresse auf `/nextcloud`
eingerichtet sind, funktionieren auch weiter.

Die Dateien werden verschoben, nicht kopiert, also braucht der Umzug keinen
zusätzlichen Platz. Was Nextcloud daneben gehalten hat, bleibt, wo es war;
nur das [Löschen der Box](backup-and-erase) entfernt es mit allem anderen. Schlägt der Umzug fehl, öffnet LosOS Cloud nicht
auf einem leeren Ordner: Die Box versucht es beim nächsten Start noch
einmal.

## Einstellungen \{#settings}

Der Abschnitt LosOS Cloud im Bereich **Apps** legt fest, wie lange der
Papierkorb Dateien behält (`losos.cloud.trashDays`, 1 bis 365 Tage).
`losos.cloud.enable` im Bereich **Erweitert** schaltet LosOS Cloud ab, und
`losos.cloud.photos.faces` lässt Fotos keine Gesichter mehr suchen: Der
nächste Durchlauf löscht alle bisher gefundenen Gesichter und Namen, die
Angaben der Kamera bleiben.
