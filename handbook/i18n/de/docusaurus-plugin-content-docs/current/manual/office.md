---
title: Office-Dokumente
sidebar_position: 4.6
slug: /manual/office
---

# Office-Dokumente

LosOS Office öffnet Textdokumente, Tabellen und Präsentationen im Browser und
speichert sie zurück auf die Box. Mehrere Personen können gleichzeitig in
einem Dokument arbeiten, und jede sieht, wo die anderen gerade schreiben.
Der Editor ist Collabora Online und läuft auf der Box selbst. Die Dokumente
verlassen sie nie.

Diese Typen öffnet und speichert es:

| Art | Typen |
|---|---|
| Text | `.odt`, `.docx`, `.docm`, `.dotx`, `.doc`, `.rtf`, `.txt` |
| Tabellen | `.ods`, `.xlsx`, `.xlsm`, `.xlsb`, `.xls`, `.csv` |
| Präsentationen | `.odp`, `.pptx`, `.pptm`, `.ppsx`, `.ppt` |
| Zeichnungen | `.odg` |

PDF-Dateien, Visio-Zeichnungen und Apples Pages-, Numbers- und
Keynote-Dateien zeigt es an, ohne sie zu ändern. Ältere Microsoft-Formate
öffnet und speichert es, aber die OpenDocument- und die neueren
Microsoft-Formate erhalten am meisten.

Word-, Excel- und PowerPoint-Dateien aus Microsoft 365 bleiben, was sie
sind: Eine `.docx` wird als `.docx` mit ihren Formatvorlagen zurückgespeichert,
und eine Arbeitsmappe behält ihre Formeln. Makros in `.docm`-, `.xlsm`- und
`.pptm`-Dateien laufen auf der Box nie.

## Einschalten \{#turning-it-on}

Auf einer neuen Box ist LosOS Office aus, weil es Speicher braucht, auch wenn
niemand ein Dokument offen hat. Schalte es unter **Einstellungen →
Erweitert** mit `losos.office.enable` ein und drück dann **Anwenden**.
Rechne mit etwa 550 MB ohne offenes Dokument. Jedes offene Dokument kommt
dazu: ein kurzes mit einigen Dutzend MB, ein großes mit mehr. Die Box lässt
es nie mehr als 2 GB nehmen.

## Ein Dokument öffnen \{#opening-a-document}

Klick in LosOS Cloud auf den Namen eines Dokuments. Es öffnet sich in LosOS
Office über die ganze Breite der Seite, unter der oberen Leiste der Box,
unter `/cloud/office/` mit der Nummer der Datei. Die Adresse kannst du dir
also als Lesezeichen speichern oder jemandem schicken, der die Datei auch
öffnen darf.

Über dem Dokument liegen zwei Leisten, gezeichnet wie der Rest von LosOS.
Die erste hat den Weg zurück, den Namen des Dokuments und ob deine
Änderungen schon auf der Box gespeichert sind, die Leute, die mit dir im
Dokument sind, den Zoom, **Als PDF herunterladen** und **Speichern**. Die
zweite hat das, wonach du am häufigsten greifst: Rückgängig und
Wiederholen, Absatzvorlage und Schriftgröße, fett, kursiv, unterstrichen,
Listen, Ausrichtung, Links, Tabellen und Kommentare. Eine Tabelle bekommt
stattdessen Zahlenformate, Sortieren und neue Zeilen und Spalten, eine
Präsentation neue Folien und **Präsentieren**. Alles andere steht im
Rechtsklick-Menü des Dokuments.

Die Leisten folgen dem hellen oder dunklen Design der Box. Die Seiten eines
Dokuments bleiben in beiden weiß, so wie sie gedruckt werden. Ein Dokument,
das du nur lesen darfst, öffnet sich mit dem Hinweis **Nur lesen** und ohne
Formatleiste. Der Editor funktioniert unter der https-Adresse der Box,
derselben, die LosOS Login nutzt; eine über http geöffnete Seite wechselt
zuerst selbst dorthin.

Der Editor speichert während der Arbeit und wenn du gehst, und
**Speichern** schreibt deine Änderungen sofort. In LosOS Cloud behält jedes
Speichern den vorigen Inhalt als Version der Datei, und die Sync-Apps auf
deinen Computern holen sich den neuen.

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
LosOS Office selbst bearbeitet, statt der Dokumente in LosOS Cloud. In der Seitenleiste
erscheint dann nach **Apps** der Eintrag **Dokumente**, für alle, die sich
anmelden. Er listet die Dokumente im Ordner und in den Ordnern darin auf
und öffnet jedes im Editor. Wie die übrigen Verwaltungsseiten antwortet er
nur im eigenen Netz der Box. Das ist für eine Box gedacht, die einen Ordner mit Dokumenten für
die Leute in ihrem Netz hält. Sonst lass ihn leer.
