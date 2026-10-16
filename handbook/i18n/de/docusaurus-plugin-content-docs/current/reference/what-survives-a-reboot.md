---
title: Was einen Neustart übersteht
sidebar_position: 2
---

# Was einen Neustart übersteht

Die Box baut ihre Systemfestplatte bei jedem Start aus dem Nichts neu auf.
Sie behält nur diese Verzeichnisse, auf dem verschlüsselten Volume, und
hängt sie per Bind-Mount wieder an ihren Platz:

| Behalten                   | Enthält                                                               |
| -------------------------- | --------------------------------------------------------------------- |
| `/home/notshared`          | Die Daten von LosOS cloud: deine Dateien, Fotos, Datenbank            |
| `/home/shared`             | Den an das Mesh verliehenen Platz, separat verschlüsselt und unlesbar, solange das Teilen aus ist |
| `/var`                     | Die Daten von LosOS Git, Zustand und Geheimnisse der Box, den Zustand der beiden Cluster, Logs |
| `/nix`                     | Das System selbst, jede Version, die für das Bootmenü aufbewahrt wird |
| `/etc/nixos`               | Die eigene Beschreibung der Box: ihre Festplatten, Firmware-Modus, Entsperrmodus, deine Einstellungen |
| `/etc/keys`                | Die Wiederherstellungskopie des Festplattenschlüssels                 |
| `/etc/ssh`                 | Host-Schlüssel, die die Box identifizieren. Einen SSH-Server gibt es nicht. |
| `/etc/rancher`             | Das Node-Passwort der Mesh-Mitgliedschaft                             |
| `/etc/machine-id`          | Die Identität der Box                                                 |

![Die Systemfestplatte liegt im RAM und wird bei jedem Start neu aufgebaut; neun Verzeichnisse werden per Bind-Mount aus dem verschlüsselten Volume /persist zurückgeholt, und alles andere ist beim Neustart um 00:07 weg.](@site/docs/img/what-survives-de.svg)

Alles andere, auch alles, was ein Angreifer oder ein Fehler irgendwo anders
hingeschrieben hat, ist um 00:07 weg.

## Zwei Dinge, die Dokumente sind, keine Einstellungen \{#two-things-that-are-documents-not-settings}

Die Box bewahrt das **Aussehen**, also den Hintergrund, den Schleier und die
von Hand geschriebenen Widgets, unter `/var` auf. Dein Browser bewahrt die
Anordnung der Pinnwand auf. Beides übersteht einen Neustart, aber keins von
beiden gehört zur Beschreibung der Box, und du kannst beides ohne Neubau
ändern.

## Wie viel Platz die Protokolle brauchen \{#how-much-room-the-logs-take}

Die Protokolle unter `/var` sind gedeckelt und können die Platte nicht
füllen:

- Das Systemjournal belegt höchstens 512 MB und nichts, was älter als einen
  Monat ist. Die Größe stellt `losos.logs.journalMaxUse` im Bereich
  Erweitert ein.
- LosOS cloud, LosOS Git und jede App behalten höchstens drei
  Protokolldateien zu 10 MB pro Container.
- Das Zugriffsprotokoll des Webservers beginnt jede Woche eine neue Datei,
  bei 20 MB auch früher, und behält acht alte.
- Das Prüfprotokoll der Verwaltungsseiten und das Protokoll der Neubauten,
  die sie starten, beginnen bei 10 MB eine neue Datei und behalten drei alte.
- Build-Protokolle von Updates werden nach 30 Tagen gelöscht.

## Was eine Neuinstallation tut \{#what-a-reinstall-does}

Sie löscht alles davon. Siehe
[Zurücksetzen und neu installieren](../manual/reset-and-reinstall).
