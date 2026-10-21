---
title: Office-Dokumente
sidebar_position: 4.6
slug: /manual/office
---

# Office-Dokumente

Office öffnet Textdokumente, Tabellen und Präsentationen im Browser und
speichert sie zurück auf die Box. Mehrere Personen können gleichzeitig in
einem Dokument arbeiten, und jede sieht, wo die anderen gerade schreiben.
Der Editor ist Collabora Online und läuft auf der Box selbst. Die Dokumente
verlassen sie nie.

Diese Typen öffnet und speichert es:

| Art | Typen |
|---|---|
| Text | `.odt`, `.docx`, `.doc`, `.rtf`, `.txt` |
| Tabellen | `.ods`, `.xlsx`, `.xls`, `.csv` |
| Präsentationen | `.odp`, `.pptx`, `.ppt` |
| Zeichnungen | `.odg` |

PDF-Dateien, Visio-Zeichnungen und Apples Pages-, Numbers- und
Keynote-Dateien zeigt es an, ohne sie zu ändern. Ältere Microsoft-Formate
öffnet und speichert es, aber die OpenDocument- und die neueren
Microsoft-Formate erhalten am meisten.

## Einschalten \{#turning-it-on}

Auf einer neuen Box ist Office aus, weil es Speicher braucht, auch wenn
niemand ein Dokument offen hat. Schalte es unter **Einstellungen →
Erweitert** mit `losos.office.enable` ein und drück dann **Anwenden**.
Rechne mit etwa 550 MB ohne offenes Dokument. Jedes offene Dokument kommt
dazu: ein kurzes mit einigen Dutzend MB, ein großes mit mehr. Die Box lässt
es nie mehr als 2 GB nehmen.

## Ein Dokument öffnen \{#opening-a-document}

Klick auf den Namen eines Dokuments, und es öffnet sich bildschirmfüllend,
unter einer Leiste mit dem Weg zurück und dem Namen des Dokuments. Ein
Dokument, das du nur lesen darfst, öffnet sich mit dem Hinweis **Nur
lesen**, und der Editor bietet keinen Weg, es zu ändern. Der Editor
funktioniert unter der https-Adresse der Box, derselben, die LosOS Login
nutzt; eine über http geöffnete Seite wechselt zuerst selbst dorthin.

Der Editor speichert während der Arbeit und wenn du gehst. **Datei →
Speichern unter** legt eine Kopie neben dem Original ab, in einem der Typen
oben oder als PDF.

## Gemeinsam arbeiten \{#working-together}

Wenn eine zweite Person dasselbe Dokument öffnet, seid ihr beide in
derselben Kopie. Jede sieht den Cursor und den Namen der anderen, und jede
Änderung erscheint auf dem anderen Bildschirm, sobald sie gemacht ist.
Niemand wartet, bis jemand fertig ist, und ein Speichern behält die
Änderungen aller.

## Was privat bleibt \{#what-stays-private}

Der Editor läuft auf der Box und liest das Dokument direkt von ihrer
Platte. Nichts geht an Collabora oder an sonst jemanden: keine
Update-Prüfungen, keine Rechtschreib- oder Übersetzungsdienste, keine
Schriften aus dem Internet.

Wenn du ein Dokument öffnest, bekommt dein Browser einen Schlüssel für
genau dieses Dokument, der nach zehn Stunden abläuft. Etwas anderes nimmt
der Editor nicht an, also kommen nur Leute in das Dokument, die es selbst
öffnen dürfen.

## Ein Ordner mit Dokumenten \{#a-folder-of-documents}

`losos.office.folder` nennt einen Ordner auf der Box, dessen Dokumente
Office selbst bearbeitet, ohne LosOS cloud. `https://<box>/office/` listet
dann die Dokumente darin und in den Ordnern darin auf und öffnet jedes im
Editor. Das ist für eine Box gedacht, die einen Ordner mit Dokumenten für
die Leute in ihrem Netz hält. Sonst lass ihn leer.
