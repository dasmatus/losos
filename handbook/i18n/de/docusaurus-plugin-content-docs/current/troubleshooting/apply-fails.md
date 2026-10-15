---
title: Anwenden schlägt fehl
sidebar_position: 9
slug: /troubleshooting/apply-fails
---

# Anwenden schlägt fehl

<div className="losos-symptom">

**Was du siehst:** Nach **Anwenden** sagt die Benachrichtigung „Das ließ
sich nicht starten“ oder „Die Änderungen konnten nicht angewendet werden“,
oder das Widget **Änderungen** auf der Übersicht führt den Job als
fehlgeschlagen. Die Box läuft mit ihren alten Einstellungen weiter.

</div>

![Die Übersicht nach einem fehlgeschlagenen Anwenden: Die Kachel Einstellungen trägt einen roten Punkt und Eine Änderung ist fehlgeschlagen.](@site/docs/img/overview-apply-failed.png)

## Was ein fehlgeschlagenes Anwenden bedeutet \{#what-a-failed-apply-means}

Nichts hat sich geändert. Ein Rebuild wird entweder fertig, und die Box
wechselt darauf, oder er schlägt fehl, und das bisherige System läuft
weiter. Ein Rebuild berührt deine Dateien nie.

## Ursachen, die wahrscheinlichste zuerst \{#causes-most-likely-first}

1. **Eine Box von vor dem 7. Oktober 2026.** Jedes Anwenden auf einer
   installierten Box antwortete „Das ließ sich nicht starten“, weil der
   Rebuild-Befehl nicht im Pfad des Dienstes lag. Pull Request #71 hat das
   behoben, und das Update um 03:00 bringt die Korrektur mit. Das
   nächtliche Update nimmt einen anderen Weg und hatte den Fehler nie. Bis
   die Korrektur ankommt, kannst du in den Bereichen keine Einstellungen
   ändern.
2. **Eine Box, die schon einen Rebuild macht.** Die Benachrichtigung sagt
   das. Warte, bis der laufende Job fertig ist, was das Widget Änderungen
   zeigt, und wende dann noch einmal an.
3. **Die Festplatte ist voll.** Ein Rebuild braucht Platz. Siehe
   [Kein Platz mehr](out-of-room).
4. **Eine Einstellung, die die Box nicht übernehmen kann.** Ein Name mit
   einem Zeichen, das die Prüfungen nicht abgefangen haben, ein Port, der
   schon belegt ist. **Verwerfen**, eine Sache nach der anderen ändern,
   wieder anwenden und schauen, welche fehlschlägt.
5. **Die Upgrade-Quelle ist nicht erreichbar.** Wenn die Upgrade-Quelle der
   Box auf GitHub zeigt und das Internet ausgefallen ist, schlägt ein
   Rebuild fehl, der etwas herunterladen muss. Warte, oder wende an, sobald
   das Internet wieder da ist. Die eigene Beschreibung der Box braucht nie
   das Internet.

## Was tun, wenn es immer wieder fehlschlägt \{#what-to-do-when-it-keeps-failing}

- Warte auf die nächsten 03:00 Uhr. Der nächtliche Rebuild führt denselben
  Job mit derselben Einstellungsdatei aus. Schlägt auch er fehl, lässt er
  die Box, wie sie ist.
- **Einstellungen → Zurücksetzen** setzt jede Einstellung zurück und baut
  aus den Standardwerten neu. Die Standardwerte sind die eine
  Beschreibung, die erwiesenermaßen auf jeder Box funktioniert. Deine
  Dateien bleiben.
- [Hol dir Hilfe](getting-help) mit dem Text, den das Widget Änderungen für
  den fehlgeschlagenen Job zeigt.
