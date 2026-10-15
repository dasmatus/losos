---
title: Einstellungen und Anwenden
sidebar_position: 3
---

# Einstellungen und Anwenden

Eine LosOS-Box hat keine Einstellungen im üblichen Sinn. Sie hat eine
Beschreibung ihrer selbst, in Nix geschrieben, und die Einstellungsbereiche
bearbeiten einen kleinen Teil davon. Wendest du eine Änderung an, baut sich
die Box aus der neuen Beschreibung neu auf. Deshalb gilt:

- Eine Änderung braucht einen Neuaufbau von ein paar Minuten, keinen
  Neustart. Die Box bleibt dabei erreichbar, und eine Benachrichtigung sagt,
  wann sie fertig ist.
- Eine Änderung wird entweder ganz übernommen oder gar nicht. Ein
  gescheiterter Neuaufbau lässt die Box mit ihren vorherigen Einstellungen
  weiterlaufen.
- Jede Box lässt sich aus dem LosOS-Quellcode und einer Datei mit ihren
  Entscheidungen neu bauen, `modules/overrides.nix`. Das nächste Speichern
  setzt jede Einstellung zurück, die die Bereiche nicht schreiben.

## Die Leiste zum Anwenden \{#the-apply-bar}

Bearbeite irgendein Feld, und unten erscheint eine Leiste: *N Änderungen
noch nicht angewendet*, mit **Verwerfen** und **Anwenden**. Die Seite
markiert ein Feld, das seine Prüfung nicht besteht, etwa einen Namen mit
Leerzeichen oder einen Port außerhalb des erlaubten Bereichs, und Anwenden
wartet, bis du es korrigierst. Nichts erreicht die Box, bevor du Anwenden
drückst.

![Ein Name mit einem Leerzeichen: Das Feld ist markiert, und Anwenden wartet, bis es korrigiert ist.](@site/docs/img/settings-apply-invalid.png)

![Der Bereich Netzwerk mit einem neu eingetippten Namen: Die Leiste sagt, dass eine Änderung noch nicht angewendet ist.](@site/docs/img/settings-apply-bar.png)

## Was Anwenden tut \{#what-apply-does}

1. Die Box schreibt deine Entscheidungen in ihre Overrides-Datei.
2. Sie startet einen Neuaufbau als Hintergrundjob (`losos-rebuild-<job>`).
   Das Widget **Änderungen** auf der Übersicht listet diese Jobs und ob
   jeder übernommen wurde.
3. Die Box wechselt auf das neue System. Manche Änderungen starten den
   Steuerdienst der Box mittendrin neu. Der Dienst nimmt den Job von selbst
   wieder auf.
4. Eine Benachrichtigung sagt *Änderungen angewendet* oder *Die Änderungen
   konnten nicht angewendet werden*, mit dem Grund, den die Box genannt hat.

## Zwei Einstellungen, die die Adresse ändern \{#two-settings-that-change-the-address}

![Der Bereich Historie: Jede angewendete Änderung, jeder Moduswechsel und jedes Zurücksetzen ist ein Commit in der eigenen Beschreibung der Box, die neuesten zuerst.](@site/docs/img/settings-history.png)

- **Name**: Die Box hört auf `<name>.local`, und die Apps verteilen Links
  auf diesen Namen. Nach einer Umbenennung erreichst du die Box unter dem
  neuen Namen oder unter ihrer IP-Adresse, die sich nicht ändert.
- **Verbindung verschlüsseln**: Ist das aus, spricht die Box nur einfaches
  HTTP, und der Schritt mit dem Zertifikat und die Passkeys fallen mit weg.

## Einstellungen, die nicht Nix sind \{#settings-that-are-not-nix}

Zwei Dinge auf den Admin-Seiten wirken sofort, ohne Neuaufbau, weil sie
Dokumente sind, die die Box aufbewahrt, und nicht Teile ihrer Beschreibung:

- **Aussehen**, also das Hintergrundbild, der Schleier und die von Hand
  geschriebenen Widgets. Der Steuerdienst der Box bewahrt es auf, sodass
  jeder Browser dasselbe sieht.
- **Dein Board** mit Widgets. Der Browser, in dem du es angeordnet hast,
  bewahrt es auf.

## Die vollständige Liste \{#the-full-list}

Die [Tabelle von Einstellungen zu Optionen](../reference/settings-to-options)
ordnet jedem Feld der Bereiche die Nix-Option dahinter zu, für den Tag, an
dem du etwas ändern willst, das die Bereiche nicht anbieten.

![Der Bereich Erweitert: jede losos-Option mit ihrem aktuellen Wert und einem passenden Editor, und Links zu den Bereichen, denen die gängigen gehören.](@site/docs/img/settings-advanced.png)
