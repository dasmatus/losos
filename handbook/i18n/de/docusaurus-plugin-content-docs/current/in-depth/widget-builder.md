---
title: Mit KI bauen
sidebar_position: 4.7
mdx:
  format: md
---

# Mit KI bauen

Du beschreibst ein Widget oder eine App in eigenen Worten, und ein
Sprachmodell auf der Box schreibt es. Zwei Stellen haben einen Tab **Mit KI
bauen**:

- der Widget-Editor (**Widget hinzufügen**, dann **Eins schreiben**), neben
  **Schreiben**;
- **Ihre Apps** auf der Seite Apps, unter **App erstellen**.

Was das Modell schreibt, landet im Editor und in der Vorschau, wo du es lesen,
benutzen und ändern kannst. Nichts wird gespeichert, bevor du auf Speichern
drückst. Ein Widget des Modells läuft im selben abgeschotteten Rahmen wie ein
selbst geschriebenes (siehe
[Administration](administration.md#widgets-written-by-hand)), und eine App
läuft in einem eigenen abgeschotteten Rahmen in deinem Browser, nie auf der
Box.

## Wie ein Bau abläuft {#how-a-build-goes}

Ein Bau ist eine kurze Schleife, die die Box ausführt und der Tab Schritt für
Schritt zeigt:

1. **Schreiben.** Das Modell schreibt das Widget oder die App.
2. **Prüfen.** Die Box liest den Code auf die Fehler, die ein kleines Modell
   am häufigsten macht: ein Name, der benutzt, aber nirgends angelegt wird,
   ein Name, der vor der Zeile benutzt wird, die ihn anlegt, ein Skript, das
   ein Element sucht, das es im Widget nicht gibt, Code, der sich nicht
   übersetzen lässt, oder ein Beispiel aus seinen Anweisungen, das statt des
   Gewünschten zurückkommt.
3. **Ausprobieren** (nur Apps). Die Vorschau in deinem Browser bedient die
   App wie ein Mensch: Sie füllt jedes Feld aus, drückt die erste
   Schaltfläche und sieht nach, was die App behalten hat. Hat sie nichts
   behalten und gibt es eine Auswahlliste, wählt sie einen anderen Eintrag
   und drückt noch einmal. Sie notiert auch alles, was die App beim Start
   oder beim Drücken wirft.
4. **Korrigieren.** Hat das Prüfen oder das Ausprobieren etwas gefunden,
   sagt die Box dem Modell in Worten, was, und das Modell schreibt alles
   noch einmal. Eine App bekommt bis zu drei Korrekturen, ein Widget eine.

Jeder Schritt zeigt, was er gefunden hat. Du kannst einen Bau bei jedem
Schritt anhalten. Am Ende landet die beste Fassung im Editor: eine, die sich
übersetzen lässt und die wenigsten Probleme übrig hat. Bleiben welche, listet
der Tab sie auf, damit du weißt, worauf du in der Vorschau achten musst.

Das Ausprobieren braucht die offene Seite. Schließt du das Fenster, läuft der
Bau weiter, und die Schritte sagen, dass die App nicht ausprobiert wurde.

## Es läuft auf der Box {#it-runs-on-the-box}

Das Modell ist Qwen2.5-Coder 1.5B Instruct vom Qwen-Team bei Alibaba Cloud,
bereitgestellt von [Ollama](https://ollama.com) auf der Box selbst. Was du
eintippst, und das Widget oder die App, die du ändern lässt, verlassen die
Box nie, und ein Bau kostet nichts.

Die Box lädt das Modell, etwa 1 GB, einmal herunter, wenn sie nach der
Installation zum ersten Mal online ist. Bis dahin sagt der Tab, dass das
Modell noch geladen wird. Das Modell bleibt auf der verschlüsselten Platte,
ein Neustart lädt es also nicht noch einmal.

### Warum ein so kleines Modell {#why-a-model-this-small}

Das Modell ist für die kleinste Box ausgelegt, auf der LosOS läuft: 4 GiB
Speicher, zwei Prozessorkerne, keine Grafikkarte, und daneben laufen LosOS
cloud und LosOS Git. Ein größeres Modell passte dort nicht hinein oder
bräuchte fast eine Stunde pro App.

Ein kleines Modell macht mehr Fehler, und dafür ist die Schleife da. Das
meiste, was es falsch macht, fängt das Prüfen oder das Ausprobieren ab, und
die nächste Runde korrigiert es.

Auf einer Box mit 8 GiB Speicher oder mehr übrig schreibt ein größeres
Qwen2.5-Coder schon beim ersten Mal bessere Apps. Setze in **Einstellungen,
Erweitert** `losos.builder.model` auf `qwen2.5-coder:7b`. Es braucht beim
Schreiben etwa 5 GiB und dauert ein Mehrfaches.

## Was dich erwartet {#what-to-expect}

Ein Bau läuft auf dem Prozessor der Box. Auf einem Mini-PC mit zwei Kernen
dauert eine App DURATION_APP_DE, ein Widget DURATION_WIDGET_DE, und jede
Korrektur etwa noch einmal so lange. Der Tab zeigt, wie viele Zeilen das
Modell schon geschrieben hat.

:::tip[Bitte um eine Sache]
Das Modell ist klein. Am besten gelingt ihm eine klare Sache: eine
Einkaufsliste, Notizen, ein Gewohnheits-Tracker, eine Uhr, die Platte als
Balken. Sag in ein, zwei Sätzen, was hinein soll. Probiere das Ergebnis dann
in der Vorschau aus, bevor du speicherst. Stimmt etwas nicht, baue neu, bitte
um eine Änderung oder ändere den Code von Hand.
:::

## Speicher {#memory}

Während es schreibt, braucht das Modell etwa 1,5 GiB Speicher zusätzlich zu
dem, was die Box ohnehin nutzt. Fünf Minuten nach dem letzten Bau gibt es ihn
wieder frei. Fehlt der Box Speicher, kann ein Bau scheitern, und der Tab sagt
das.

:::note[Das Bauen mit KI abschalten]
**Einstellungen, Erweitert** hat `losos.builder.enable`. Schaltest du es ab,
lädt und startet die Box das Modell nicht, und der Tab sagt, dass das Bauen
mit KI aus ist.
:::

## Bedingungen von Qwen {#qwens-terms}

Qwen2.5-Coder 1.5B Instruct steht unter der
[Apache License 2.0](https://huggingface.co/Qwen/Qwen2.5-Coder-1.5B-Instruct/blob/main/LICENSE).
Der Tab nennt das Modell unter der Schaltfläche Bauen und verlinkt seine
Lizenz und seine Seite.

## Gut zu wissen {#good-to-know}

- Ein Bau läuft weiter, wenn du das Fenster schließt, nur ohne Ausprobieren.
- Eine Box führt einen Bau zur Zeit aus.
- Um ein Widget oder eine App zu ändern, lass es im Editor und setze den
  Haken, der eine Änderung verlangt. Das Modell bekommt, was dort ist, und
  schickt alles geändert zurück.
- Das Widget des Modells kann die Messwerte der Box lesen wie jedes Widget.
  Eine App speichert ihre Daten über die Seite auf der Box. Die Box ändern
  kann keins von beiden.
