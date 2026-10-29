---
title: Dateien auf deinem Computer
sidebar_position: 4.5
slug: /manual/files-on-your-computer
---

# Dateien auf deinem Computer

![Die Seite Geräte in LosOS Cloud: die Serveradresse, die WebDAV-Adresse für einen Dateimanager und die angemeldeten Computer.](@site/docs/img/cloud-devices-de.png)

Deine Dateien in LosOS Cloud sind auch ein Netzwerkordner. Jeder Computer im
Netz der Box öffnet ihn im Dateimanager, zieht Dateien hinein und heraus und
öffnet sie an Ort und Stelle, ohne etwas zu installieren. Es gibt zwei Wege
hinein:

- **Die Netzwerkfreigabe.** Im Netz der Box zeigen Windows, macOS und Linux
  die Box von selbst unter **Netzwerk**, mit deinen Dateien in einem Ordner
  mit deinem Namen.
- **Die WebDAV-Adresse.** Sie funktioniert im Netz der Box und über das
  Internet und ist der Weg hinein, solange die Netzwerkfreigabe aus ist.

`<name>` unten ist der Name der Box, wie er auf ihrem Bildschirm steht.
Beide enthalten dieselben Dateien wie Dateien in LosOS Cloud.

## Die Netzwerkfreigabe \{#the-network-share}

Dein Ordner in der Freigabe ist:

```text
\\<name>\<dein Name>
```

Melde dich mit dem Namen und Passwort an, die du bei LosOS Login verwendest.
Jede Person sieht nur ihren eigenen Ordner; wer die Box eingerichtet hat,
bekommt die Dateien des Eigentümers. Die Adresse steht auch auf deiner Seite
**Dein Konto**.

Die Freigabe antwortet nur Computern im eigenen Netz der Box, nie über das
Internet. Eine Administratorin oder ein Administrator schaltet sie unter
**Einstellungen** > **Netzwerk** > **Windows, Mac und Linux** ein und aus.
Sie ist von Anfang an an.

### Windows \{#windows}

1. Öffne den Explorer und klicke auf **Netzwerk**. Meldet eine Leiste, dass
   die Netzwerkerkennung aus ist, klicke darauf und schalte sie für ein
   privates Netzwerk ein.
2. Doppelklicke auf `<name>` und dann auf den Ordner mit deinem Namen.
3. Gib deinen LosOS-Namen und dein Passwort ein, setze das Häkchen bei
   **Anmeldedaten speichern** und klicke auf **OK**.

Um dem Ordner einen Laufwerksbuchstaben zu geben, klicke ihn mit der
rechten Maustaste an und wähle **Netzlaufwerk verbinden**.

### macOS \{#macos}

1. Klicke in der Seitenleiste des Finders unter **Orte** auf **Netzwerk**
   und dann auf `<name>`.
2. Klicke auf **Verbinden als**, wähle **Registrierter Benutzer**, gib
   deinen LosOS-Namen und dein Passwort ein und setze das Häkchen bei
   **Kennwort im Schlüsselbund sichern**.

**Gehe zu** > **Mit Server verbinden** mit `smb://<name>.local/<dein Name>`
öffnet denselben Ordner.

### Linux \{#linux}

- **GNOME Dateien:** Öffne **Andere Orte**. Klicke unter **Netzwerke** auf
  `<name>`, dann auf den Ordner mit deinem Namen, und gib Namen und Passwort
  ein.
- **Dolphin:** Öffne in der Ortsleiste **Netzwerk** und dann `<name>`.

Die Adresse `smb://<name>.local/<dein Name>` funktioniert in beiden
Adresszeilen.

### Wenn die Freigabe dein Passwort ablehnt \{#if-your-password-is-refused}

Windows, und mit ihm die Netzwerkfreigabe, prüft ein Passwort auf seine
eigene Art, darum muss die Box deins einmal sehen, bevor die Freigabe es
annimmt. Sie sieht es, wenn du die Box einrichtest, wenn dein Konto angelegt
wird, wenn du dein Passwort änderst und wenn du dich an der WebDAV-Adresse
anmeldest. Ist nichts davon passiert, seit die Freigabe an ist, öffne in
LosOS die Seite **Dein Konto**, such **Deine Dateien unter Windows, Mac und
Linux**, gib dein Passwort unter **Windows dein Passwort erlauben** ein und
klicke auf **Erlauben**.

### Was die Freigabe anders macht \{#what-the-share-does-differently}

Dateien, die du in der Freigabe änderst, ändern sich sofort auch in LosOS
Cloud, im Browser und in den Apps für Computer und Handy. Eine Datei, die du
dort löschst, ist endgültig weg: Sie landet nicht im Papierkorb von LosOS
Cloud. Und eine Datei, die du dort überschreibst, behält keine ältere
Version.

## Die WebDAV-Adresse \{#the-webdav-address}

```text
https://<name>.local/remote.php/webdav/
```

### Bevor du anfängst \{#before-you-start}

Installiere das Zertifikat der Box auf dem Computer, wie in
[Erster Start, Schritt 1](../start/first-run#step-1-trust-this-box)
beschrieben. Ohne es meldet der Dateimanager, dass er die Box nicht prüfen
kann, und fragt, ob er weitermachen soll. Windows verbindet sich gar nicht.

### Welcher Name und welches Passwort \{#which-name-and-password}

Im Netz der Box gibst du den Anmeldenamen und das Passwort ein, die du bei
LosOS Login verwendest. Wer die Box eingerichtet hat, bekommt die Dateien
des Eigentümers. Alle anderen bekommen ihre eigenen, und beim ersten Öffnen
der Freigabe legt LosOS Cloud ihr Konto an.

Über das Internet antwortet LosOS Login nicht, deshalb fragt die Freigabe
dort nach einem App-Passwort: dem, das eine App für Computer oder Telefon
bei der Anmeldung bekommen hat, oder dem, das `losos cloud setup` erstellt
hat (unten). Ein App-Passwort funktioniert auch im Netz der Box, und
**Geräte** in LosOS Cloud meldet es einzeln ab.

### Linux \{#webdav-linux}

Solange die Netzwerkfreigabe aus ist, zeigen GNOME Dateien und KDE Dolphin
an ihrer Stelle **LosOS Cloud on `<name>`** unter **Netzwerk**. Um die
Adresse selbst einzugeben, schreibe `davs://<name>.local/remote.php/webdav/`
in das Feld **Mit Server verbinden** von GNOME Dateien oder
`webdavs://<name>.local/remote.php/webdav/` in die Adresszeile von
Dolphin.

Lässt du das Passwort speichern, öffnet sich der Ordner beim nächsten Mal
ohne Nachfrage. GNOME Dateien kann ihn auch als Lesezeichen in die
Seitenleiste legen.

### Windows \{#webdav-windows}

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
solange sein Limit nicht erhöht ist. Im Netz der Box nimm stattdessen die
Netzwerkfreigabe, anderswo die Desktop-App unten.
:::

### macOS \{#webdav-macos}

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
