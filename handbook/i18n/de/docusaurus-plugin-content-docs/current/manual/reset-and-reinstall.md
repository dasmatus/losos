---
title: Zurücksetzen und Neuinstallation
sidebar_position: 9
---

# Zurücksetzen und Neuinstallation

Zurücksetzen und Neuinstallation sind zwei verschiedene Dinge, und du
erledigst sie an verschiedenen Stellen.

## Zurücksetzen: alle Einstellungen zurück \{#reset-put-every-setting-back}

**Einstellungen → Zurücksetzen → Zurücksetzen…** setzt jede Einstellung auf
den Zustand zurück, in dem die Box kam: den Namen, das Netzwerk, das Mesh,
die Apps. Deine Dateien, Fotos und Repositorys rührt das nicht an. Die Box
baut sich neu auf und kommt unter ihrem ursprünglichen Namen zurück, also
ändert sich auch der Name, unter dem du sie erreichst, wieder zurück. Die
IP-Adresse bleibt gleich. Der Dialog listet, was geht und was bleibt, und du
musst ein zweites Mal drücken.

![Der Dialog zum Zurücksetzen: Er sagt genau, was zurückgesetzt wird und was bleibt, mit Einstellungen behalten an erster Stelle.](@site/docs/img/settings-reset-dialog-de.png)

![Einstellungen, Zurücksetzen: Alle Einstellungen zurücksetzen, mit dem, was geht und was bleibt.](@site/docs/img/settings-reset-de.png)

Ein Zurücksetzen ändert weder das Passwort des Besitzers noch den
Ersatzschlüssel und öffnet das Zeitfenster zum Beanspruchen der Box nicht
wieder.

Um auch die Daten zu löschen, verwende **Löschen…** darunter auf demselben
Bildschirm. [Sicherung und Löschen](./backup-and-erase) erklärt das, und
auch, wie eine Sicherung danach alles zurückholt.

## Neuinstallation: alles löschen und neu anfangen \{#reinstall-wipe-and-start-over}

Startest du den Installations-Stick erneut, installiert er von Grund auf
neu. Dabei **löscht er jede Festplatte**, samt deinen Dateien, dem Passwort,
dem Ersatzschlüssel, der Identität der Box im Mesh und ihren Schlüsseln. Eine
Option „meine Daten behalten“ gibt es bewusst nicht. Die Box ist eine Kopie,
und deine andere Kopie ist der Ort, an dem deine Daten sicher sind.

Nach einer Neuinstallation ist die Box eine neue Box. Führe den Assistenten
erneut aus, drucke einen neuen Ersatzschlüssel und vertraue dem neuen
Zertifikat. Mit einer Edge lässt du die Box unter ihrer neuen Identität
erneut aufnehmen.

## Was eine Neuinstallation nicht braucht \{#what-a-reinstall-does-not-need}

Einen Rechner, der vom Installations-Stick startet. Mehr nicht. Das Einzige,
was du beibehalten musst, sind die Firmware-Einstellungen der Box: Secure
Boot aus für die installierte Box oder das LosOS-Zertifikat für den Stick
eingetragen. [Secure Boot](../start/secure-boot) erklärt beides.
