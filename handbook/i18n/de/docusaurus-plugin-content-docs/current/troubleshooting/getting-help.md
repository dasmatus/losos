---
title: Hilfe bekommen
sidebar_position: 13
slug: /troubleshooting/getting-help
---

# Hilfe bekommen

Schick Probleme und Fragen an den
[Issue-Tracker](https://github.com/dasmatus/losos/issues) des Projekts. Die
Box hat keine Shell, deshalb ist das, was du im LAN sammeln kannst, alles,
worum ein Maintainer bitten kann. Sammle es, bevor du schreibst.

## Lass die Box ihn schreiben \{#let-the-box-write-it}

Ein Absturz, ein Dienst, der mit einem Fehler endet, oder ein Programm, das
wegen Speichermangels beendet wurde, steht schon unter **Einstellungen →
Probleme**, mit einem Bericht aus dem Protokoll der Box. Lies ihn dort und
schick ihn in zwei Schritten: [Problemberichte](../manual/problem-reports.md)
sagt wie. Eine gescheiterte Installation bietet dasselbe auf dem Bildschirm
des Installers an.

Alles andere schreibst du selbst als Issue.

## Was dazugehört \{#what-to-include}

![Der Bereich Über, dessen Zeilen in einen Bericht gehören: Name, Adresse, Speichermodus und Zustand des Mesh.](@site/docs/img/settings-about-de.png)

1. **Was du getan und was du gesehen hast**, mit dem genauen Text jeder
   Meldung und einem Screenshot, falls es einen gab.
2. **Der Text des Banners**: die Adresse und der `.local`-Name.
3. **Der Bereich Über**: Name, Adresse, unter der die Box erreicht wurde,
   Speichermodus, Zustand des Mesh.
4. **Die Antwort von** `http://<address>/api/health` und
   `http://<address>/setup/state.json`. Die zweite enthält kein Geheimnis,
   nur einen Namen, eine Adresse und einen Zertifikats-Fingerabdruck.
5. **Die Einträge des Widgets Änderungen**, wenn ein Anwenden oder ein
   Update beteiligt ist: der Name des Jobs und sein Ergebnistext.
6. **Wie die Box installiert wurde**: TPM oder Schlüsseldatei, UEFI oder
   BIOS, aus welchem Release, auf welcher Hardware oder mit welcher
   VM-Software. Die ersten beiden hat die Installation ausgegeben.
7. **Wann.** Die Uhrzeit zählt wegen des Neustarts um 00:07 und des
   Updates um 03:00.

## Was nicht dazugehört \{#what-not-to-include}

Lass den Ersatz-Admin-Schlüssel, das Passwort des Besitzers und den Inhalt
deiner Dateien weg. Niemand braucht sie, und der Issue-Tracker ist
öffentlich.

## Wenn es eilt \{#if-it-is-urgent}

Ein Neustart behebt alles, was nicht auf dem verschlüsselten Volume liegt,
ob er um 00:07 kommt oder weil du den Stecker ziehst. Ein **Zurücksetzen**
stellt die Einstellungen zurück. Beides kannst du heute Nacht erledigen.
Die Antwort eines Maintainers kann länger dauern.
