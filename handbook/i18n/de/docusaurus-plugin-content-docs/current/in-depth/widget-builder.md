---
title: Widget-Baukasten
sidebar_position: 4.7
mdx:
  format: md
---

# Widget-Baukasten

Du beschreibst ein Widget in eigenen Worten, und ein KI-Agent schreibt es.
Der Widget-Editor (**Widget hinzufügen**, dann **Eins schreiben**) hat neben
**Schreiben** einen Tab **Mit KI bauen**. Die Dateien des Widgets landen im
Editor, wo du sie lesen, in der Vorschau ausprobieren und ändern kannst.
Nichts wird gespeichert, bevor du auf Speichern drückst, und ein Widget der
KI läuft im selben abgeschotteten Rahmen wie ein selbst geschriebenes (siehe
[Administration](administration.md#selbst-geschriebene-widgets)).

Der Tab funktioniert, wenn der Edge der Box den Baukasten anbietet. Sonst
sagt er das.

## Was es kostet

Der Tab zeigt den Preis pro Million Tokens, die die KI liest und schreibt,
und was ein Bau höchstens kostet. Bezahlt wird aus einem vorausbezahlten
Guthaben. Du lädst es mit einem der Pakete auf, die der Tab anbietet, auf der
Zahlungsseite von Stripe, und das Guthaben ändert sich, sobald Stripe die
Zahlung bestätigt.

Ein Bau wird nach Verbrauch berechnet, auch wenn die KI ohne brauchbares
Widget aufhört. Ein Bau, der nicht abgeschlossen werden kann, kostet nichts.
Erreicht ein Bau seine Ausgabengrenze, sagt der Tab, dass das Widget
vielleicht unfertig ist.

## Bedingungen von Anthropic

Die Beschreibung und die zur Änderung geschickten Dateien gehen an Anthropic
und unterliegen dessen [Nutzungsrichtlinie](https://www.anthropic.com/legal/aup) und
[Geschäftsbedingungen](https://www.anthropic.com/legal/commercial-terms). Der Tab sagt das unter der Schaltfläche Bauen.

## Gut zu wissen

- Ein Bau dauert ein, zwei Minuten und läuft weiter, wenn du das Fenster
  schließt.
- Eine Box führt einen Bau zur Zeit aus.
- Um ein Widget zu ändern, lass es im Editor und setze den Haken, der eine
  Änderung verlangt. Die KI bekommt alle Dateien des Widgets und schickt die
  zurück, die es behält.
- Das Widget der KI kann die Messwerte der Box lesen wie jedes Widget. Die
  Box ändern kann es nicht.
