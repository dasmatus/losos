---
title: Dateien auf deinem Computer
sidebar_position: 4.5
slug: /manual/files-on-your-computer
---

# Dateien auf deinem Computer

Deine Dateien in LosOS Cloud sind auch ein Netzwerkordner. Jeder Computer im
Netz der Box öffnet ihn im Dateimanager, zieht Dateien hinein und heraus und
öffnet sie an Ort und Stelle, ohne etwas zu installieren. Der Ordner ist die
WebDAV-Freigabe von LosOS Cloud unter dieser Adresse:

```text
https://<name>.local/remote.php/webdav/
```

`<name>` ist der Name der Box, wie er auf ihrem Bildschirm steht. Die
Freigabe enthält dieselben Dateien wie Dateien in LosOS Cloud.

## Bevor du anfängst \{#before-you-start}

Installiere das Zertifikat der Box auf dem Computer, wie in
[Erster Start, Schritt 1](../start/first-run#step-1-trust-this-box)
beschrieben. Ohne es meldet der Dateimanager, dass er die Box nicht prüfen
kann, und fragt, ob er weitermachen soll. Windows verbindet sich gar nicht.

## Welcher Name und welches Passwort \{#which-name-and-password}

Im Netz der Box gibst du den Anmeldenamen und das Passwort ein, die du bei
LosOS Login verwendest. Wer die Box eingerichtet hat, bekommt die Dateien
des Eigentümers. Alle anderen bekommen ihre eigenen, und beim ersten Öffnen
der Freigabe legt LosOS Cloud ihr Konto an.

Über das Internet antwortet LosOS Login nicht, deshalb fragt die Freigabe
dort nach einem App-Passwort: dem, das eine App für Computer oder Telefon
bei der Anmeldung bekommen hat, oder dem, das `losos cloud setup` erstellt
hat (unten). Ein App-Passwort funktioniert auch im Netz der Box, und
**Geräte** in LosOS Cloud meldet es einzeln ab.

## Linux \{#linux}

GNOME Dateien und KDE Dolphin finden die Box von selbst.

- **GNOME Dateien:** Öffne **Andere Orte**. Klicke unter **Netzwerke** auf
  **LosOS Cloud on `<name>`** und gib Namen und Passwort ein.
- **Dolphin:** Öffne in der Ortsleiste **Netzwerk** und dann **LosOS Cloud
  on `<name>`**.

Um die Adresse selbst einzugeben, schreibe
`davs://<name>.local/remote.php/webdav/` in das Feld **Mit Server
verbinden** von GNOME Dateien oder
`webdavs://<name>.local/remote.php/webdav/` in die Adresszeile von
Dolphin.

Lässt du das Passwort speichern, öffnet sich der Ordner beim nächsten Mal
ohne Nachfrage. GNOME Dateien kann ihn auch als Lesezeichen in die
Seitenleiste legen.

## Windows \{#windows}

Der Datei-Explorer zeigt die Freigabe nicht unter **Netzwerk**. Füge sie
über die Adresse hinzu:

1. Öffne **Dieser PC** und dann **Netzlaufwerk verbinden** (unter Windows 11
   im Menü **...**).
2. Wähle einen Laufwerksbuchstaben. Gib unter **Ordner**
   `https://<name>.local/remote.php/webdav/` ein.
3. Setze die Häkchen bei **Verbindung mit anderen Anmeldeinformationen
   herstellen** und **Verbindung bei Anmeldung wiederherstellen**, dann
   **Fertig stellen**.
4. Gib Namen und Passwort ein und setze das Häkchen bei **Anmeldedaten
   speichern**.

Die Freigabe erscheint dann unter **Dieser PC** mit eigenem
Laufwerksbuchstaben.

:::note[Große Dateien unter Windows]
Der eigene WebDAV-Client von Windows lehnt Dateien über etwa 50 MB ab,
solange sein Limit nicht erhöht ist. Für große Dateien oder ganze Ordner
nimm die Desktop-App unten.
:::

## macOS \{#macos}

Auch der Finder zeigt die Freigabe nicht unter **Netzwerk**. Füge sie über
die Adresse hinzu:

1. Wähle im Finder **Gehe zu** > **Mit Server verbinden** (Befehl-K).
2. Gib `https://<name>.local/remote.php/webdav/` ein und klicke
   auf **Verbinden**.
3. Wähle **Registrierter Benutzer**, gib Namen und Passwort ein und setze
   das Häkchen bei **Kennwort im Schlüsselbund sichern**.

Klickst du vor **Verbinden** auf **+**, bleibt die Adresse in der Liste der
bevorzugten Server.

## Die Desktop- und Handy-Apps \{#the-desktop-and-phone-apps}

Die Freigabe ist dafür da, Dateien dort zu öffnen, wo sie liegen. Um eine
Kopie eines Ordners synchron zu halten oder Fotos vom Handy hochzuladen,
nimm den Nextcloud-Desktop-Client und die Nextcloud-Apps fürs Handy, die mit
LosOS Cloud synchronisieren. Gib ihnen die Adresse `https://<name>.local`. Zum Anmelden öffnen sie LosOS Login im
Browser und behalten danach ein eigenes App-Passwort.

Auf einem Linux-Computer mit dem Terminal-Client `losos` richtet
`losos cloud setup` den Dateimanager für dich ein; siehe
[Der Terminal-Client](terminal).
