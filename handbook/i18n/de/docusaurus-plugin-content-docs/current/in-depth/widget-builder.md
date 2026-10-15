---
title: Widget-Baukasten
sidebar_position: 4.7
mdx:
  format: md
---

# Widget-Baukasten

Du beschreibst ein Widget in eigenen Worten, und ein Sprachmodell auf der Box
schreibt es. Der Widget-Editor (**Widget hinzufügen**, dann **Eins
schreiben**) hat neben **Schreiben** einen Tab **Mit KI bauen**. Die Dateien
des Widgets landen im Editor, wo du sie lesen, in der Vorschau ausprobieren
und ändern kannst. Nichts wird gespeichert, bevor du auf Speichern drückst,
und ein Widget des Modells läuft im selben abgeschotteten Rahmen wie ein
selbst geschriebenes (siehe
[Administration](administration.md#widgets-written-by-hand)).

## Es läuft auf der Box {#it-runs-on-the-box}

Das Modell ist Llama 3.2 von Meta mit 3 Milliarden Parametern, bereitgestellt
von [Ollama](https://ollama.com) auf der Box selbst. Was du eintippst, und das
Widget, das du ändern lässt, verlassen die Box nie, und ein Bau kostet nichts.

Die Box lädt das Modell, etwa 2 GB, einmal herunter, wenn sie nach der
Installation zum ersten Mal online ist. Bis dahin sagt der Tab, dass das
Modell noch geladen wird. Das Modell bleibt auf der verschlüsselten Platte,
ein Neustart lädt es also nicht noch einmal.

## Was dich erwartet {#what-to-expect}

Ein Bau läuft auf dem Prozessor der Box. Auf einem Mini-PC dauert er einige
Minuten, und der Tab zeigt, wie viele Zeilen das Modell schon geschrieben
hat. Du kannst ihn anhalten.

Bevor das Widget im Editor landet, prüft die Box es auf die Fehler, die ein
kleines Modell am häufigsten macht, etwa ein Skript, das ein Element sucht,
das es im Widget nicht gibt. Findet sie einen, bekommt das Modell noch eine
Runde mit dem benannten Fehler, und der Tab sagt das. Das bessere der zwei
Widgets landet im Editor.

Das Modell ist klein. Am besten gelingt ihm eine klare Sache pro Widget: eine
Uhr, ein Countdown, die Platte als Balken, die Liste der Apps, das Wetter für
eine Stadt. Bitte um eine Sache in ein, zwei Sätzen. Sieh dir das Ergebnis in
der Vorschau an, bevor du speicherst; manchmal zeigt ein Widget nichts oder
den falschen Messwert, und dann hilft ein neuer Bau oder eine kleine Änderung
von Hand.

## Speicher {#memory}

Während es schreibt, braucht das Modell etwa 4 GiB Speicher zusätzlich zu
dem, was die Box ohnehin nutzt. Fünf Minuten nach dem letzten Bau gibt es ihn
wieder frei. Mit 8 GiB Speicher oder mehr ist Platz für beides. Auf einer Box
mit weniger kann ein Bau aus Speichermangel scheitern, und der Tab sagt das.

**Einstellungen, Erweitert** hat `losos.builder.enable`. Schaltest du es ab,
lädt und startet die Box das Modell nicht, und der Tab sagt, dass der
Baukasten aus ist.

## Bedingungen von Llama {#llamas-terms}

Llama 3.2 steht unter der
[Llama 3.2 Community License](https://www.llama.com/llama3_2/license/) und
ihrer [Acceptable Use Policy](https://www.llama.com/llama3_2/use-policy/). Der
Tab sagt „Built with Llama“ und verlinkt beide unter der Schaltfläche Bauen.

## Gut zu wissen {#good-to-know}

- Ein Bau läuft weiter, wenn du das Fenster schließt.
- Eine Box führt einen Bau zur Zeit aus.
- Um ein Widget zu ändern, lass es im Editor und setze den Haken, der eine
  Änderung verlangt. Das Modell bekommt alle Dateien des Widgets und schickt
  die zurück, die es behält.
- Das Widget des Modells kann die Messwerte der Box lesen wie jedes Widget.
  Die Box ändern kann es nicht.
