---
title: Problemberichte
sidebar_position: 6.5
slug: /manual/problem-reports
---

# Problemberichte

Eine Box hat keine Shell und oft niemanden, der zusieht, deshalb führt sie
selbst eine Liste dessen, was schiefging. Ist etwas auf dieser Liste neu
für das Projekt, schreibt sie einen Bericht, den du ganz lesen und als
GitHub-Issue an die Entwickler schicken kannst. Nichts verlässt die Box,
bevor du es abschickst.

## Was die Box bemerkt \{#what-the-box-notices}

Einmal pro Minute liest die Box ihr Protokoll und sucht drei Arten von
Problemen:

- ein Dienst, der **mit einem Fehler endete**, etwa ein fehlgeschlagener
  Start oder eine Zeitüberschreitung;
- ein Programm, das **abgestürzt** ist, beendet durch ein Signal wie
  `SIGSEGV` oder `SIGABRT`;
- ein Programm, das der Kernel **beendet hat, weil der Box der Speicher
  ausging**.

Dasselbe Problem noch einmal ist ein Eintrag mit einer Anzahl, kein neuer.
**Einstellungen → Probleme** listet sie, das neueste zuerst.

Die Box schreibt keine Core-Dumps. Der Core-Dump eines laufenden Dienstes
enthielte dessen Geheimnisse, deshalb wird der Bericht aus dem Protokoll
geschrieben.

## Probleme, die das Handbuch erklärt \{#problems-the-handbook-explains}

Manche Fehler haben hier schon eine Seite, etwa eine Festplatte ohne
[Platz](../troubleshooting/out-of-room.md) oder ein
[Übernehmen, das nichts herunterladen kann](../troubleshooting/apply-fails.md).
Bei ihnen nennt der Eintrag die Seite, verlinkt sie und bietet keinen
Bericht an: Die Seite sagt, was zu tun ist, und die Entwickler wissen es
schon.

## Einen Bericht lesen \{#reading-a-report}

**Bericht lesen** öffnet den ganzen Text, genau so, wie GitHub ihn
bekäme:

- was passiert ist, wann und wie oft;
- der Zustand des Dienstes;
- das Protokoll dieses Dienstes um die Zeit des Fehlers, oder die Zeilen
  des Kernels bei einem Absturz oder einem Speichermangel;
- die Maschine: Release, Firmware, Prozessor, Speicher und Kernel.

Bevor du ihn siehst, entfernt die Box, was dich erkennbar machen könnte:
IP- und MAC-Adressen, E-Mail-Adressen, lange Schlüssel und Tokens, Tor- und
I2P-Namen, den eigenen Namen der Box und die Namen der Leute, die sie
nutzen. Lies ihn trotzdem. Ein Dateiname oder eine Meldung im Protokoll
kann mehr verraten, als du öffentlich haben willst.

## Einen senden \{#sending-one}

**Auf GitHub senden** fragt GitHub nach einem Code. Öffne
`github.com/login/device` auf einem beliebigen Handy oder Computer, melde
dich an und gib den Code ein, den der Bereich zeigt. Der Bericht geht dann
als Issue unter deinem GitHub-Konto hinaus, damit die Entwickler dich
dazu fragen können. Die Box behält die Anmeldung nur, bis der Bericht
gesendet ist.

Vorher kannst du eine Notiz hinzufügen, etwa was du gerade gemacht hast.
Sie steht über dem Bericht.

Hat jemand dasselbe Problem schon gemeldet, wird dein Bericht seinem Issue
als Kommentar hinzugefügt, statt ein zweites zu öffnen. Der Titel jedes
Berichts endet mit einem kurzen Fingerabdruck wie `[8c1f04e2]`, und der
macht zwei Berichte zu demselben.

**Abbrechen** beendet das Warten auf den Code jederzeit. Nichts wird
gesendet.

**Ausblenden** nimmt einen Eintrag von der Liste. Er kommt zurück, wenn
das Problem wieder auftritt.

## Aus dem Installer \{#from-the-installer}

Das Installationsmedium meldet auf dieselbe Weise, auf seinem eigenen
Bildschirm:

- Wenn eine Installation an etwas **scheitert**, das dieses Handbuch nicht
  erklärt, bietet der Bildschirm einen Bericht an. Erklärt das Handbuch
  es, nennt der Bildschirm stattdessen die Seite.
- Während die Installation läuft, beobachtet der Installer die
  **Auslastung von Speicher und Prozessor**. Das Medium läuft ganz aus dem
  Speicher. Bleibt der Speicher ein paar Sekunden über 90 % oder der
  Prozessor eine Minute über 95 %, sagt er das am Ende, auch wenn die
  Installation fertig wurde, und bietet einen Bericht an.

Der ganze Bericht steht auf Bildschirm 7: Halte **Alt** und drücke **F7**,
um ihn zu lesen, und **Alt+F1** bringt dich zurück. Mit **R** sendest du
ihn. Der Bildschirm zeigt dann die Adresse und den Code für ein anderes
Gerät und am Ende die Adresse des Issues. Jede andere Taste geht weiter,
ohne zu senden.

## Ohne JavaScript \{#without-javascript}

Die [Seiten ohne JavaScript](without-javascript.md) haben denselben Bereich
**Probleme**. Während sie auf den Code wartet, lädt sich die Seite alle
paar Sekunden selbst neu.
