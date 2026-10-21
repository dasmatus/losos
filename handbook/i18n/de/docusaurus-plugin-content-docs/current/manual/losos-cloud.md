---
title: LosOS Cloud
sidebar_position: 4.4
slug: /manual/losos-cloud
---

# LosOS Cloud

LosOS Cloud ist der eigene Dateidienst der Box: deine Dateien im Browser,
ein Netzwerkordner für den Dateimanager und ein Server, mit dem sich die
Nextcloud-Apps für Computer und Telefon synchronisieren. Er läuft auf der
Box neben dem Nextcloud-Dienst unter `/nextcloud/`, den er App für App
übernimmt; vorerst haben beide getrennte Dateien.

Öffne ihn unter `https://<name>.local/cloud/`. `<name>` ist der Name der
Box, wie er auf ihrem Bildschirm steht. Du meldest dich mit LosOS Login an,
und jeder, der sich anmelden kann, hat eigene Dateien. Wer die Box
eingerichtet hat, bekommt die des Besitzers.

## Im Browser \{#in-the-browser}

- **Dateien** zeigt deine Ordner. Zieh Dateien oder ganze Ordner auf die
  Seite, um sie hochzuladen, oder nimm **Hochladen**. Markierte Dateien
  kannst du herunterladen, umbenennen, verschieben, kopieren oder löschen,
  oder ihre früheren Versionen ansehen.
- **Zuletzt** zeigt, was sich zuletzt geändert hat, **Favoriten**, was du
  mit einem Stern markiert hast.
- **Fotos** zeigt alle Bilder, den neuesten Monat zuerst. Klick auf eins,
  um es groß zu sehen, und blättere mit den Pfeiltasten weiter.
- **Papierkorb** behält gelöschte Dateien 30 Tage. Stell eine wieder her
  oder lösche sie endgültig.
- **Geräte** listet die Computer und Telefone, die synchronisieren, und
  meldet eins ab.
- Das Suchfeld oben findet Dateien nach Namen.
- Ein Rechtsklick auf eine Datei, einen Ordner oder die Fläche darum zeigt,
  was du dort tun kannst; siehe
  [das Rechtsklick-Menü](admin-pages#the-right-click-menu).

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

## Im Dateimanager \{#in-the-file-manager}

Dieselben Dateien sind eine WebDAV-Freigabe unter dieser Adresse:

```text
https://<name>.local/remote.php/webdav/
```

Im Netz der Box zeigen GNOME Files und Dolphin sie im Netzwerk als **LosOS
Cloud on `<name>`**. Windows und macOS fügen sie über die Adresse hinzu,
wie in [Dateien auf deinem Computer](files-on-your-computer), mit dieser
Adresse statt der von LosOS Cloud. Installiere vorher das Zertifikat der
Box, wie im [ersten Start, Schritt 1](../start/first-run#step-1-trust-this-box).

Im Netz der Box nimmt die Freigabe deinen Namen und dein Passwort für LosOS
Login. Über das Internet nimmt sie nur das eigene Passwort einer App, nimm
dort also die App für Computer oder Telefon.

## Einstellungen \{#settings}

`losos.cloud.enable` schaltet LosOS Cloud ab, und `losos.cloud.trashDays`
legt fest, wie lange der Papierkorb Dateien behält (1 bis 365 Tage). Beides
steht im Bereich **Erweitert**.
