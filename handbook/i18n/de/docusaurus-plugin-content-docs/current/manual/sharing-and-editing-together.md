---
title: Teilen und gemeinsam bearbeiten
sidebar_position: 4.45
slug: /manual/sharing-and-editing-together
---

# Teilen und gemeinsam bearbeiten

In [LosOS Cloud](losos-cloud) öffnen sich Textdateien in einem Editor, in den
mehrere Leute gleichzeitig schreiben können. Jeder sieht oben die Gesichter
der anderen, ihre Cursor in ihrer eigenen Farbe mit dem Namen daneben und
unter dem Text, wer zuletzt etwas geändert hat. So öffnet sich reiner Text:
`.txt`, `.csv`, `.json`, `.yaml` und ähnliche. Eine Markdown-Datei öffnet
sich in [Notizen](notes-and-knowledge-bases), und dort bringt dich
**Bearbeiten** hierher.

Auf einer Box geht das für jeden, der die Datei öffnen kann. Sobald die Box
in einem Mesh ist, kannst du eine Datei oder einen Ordner auch mit jemandem
auf einer anderen Box teilen und über beide Boxen hinweg gemeinsam darin
schreiben.

## Eine Datei zum Bearbeiten öffnen \{#open-a-file-to-edit-it}

Klick in **Dateien** auf eine Textdatei. Der Editor zeigt:

- die Leute, die gerade in der Datei sind, als runde Gesichter;
- **Live**, solange dein Browser mit der Box verbunden ist;
- wer die Datei zuletzt geändert hat, und wann.

Die Datei auf der Box wird kurz gespeichert, nachdem jemand aufhört zu
schreiben, damit auch die Apps für Computer und Telefon die Änderung sehen.
Jedes Speichern ist eine Version, zu der du zurückkehren kannst. Ändert eine
App die Datei, während Leute sie offen haben, erscheint die Änderung in
ihren Editoren.

## Mit jemandem auf einer anderen Box teilen \{#share-with-someone-on-another-box}

Jeder hat eine Adresse der Form `name@box`, zum Beispiel
`eva@kitchenbox.local`. Deine steht unten im Fenster zum Teilen.

1. Markiere in **Dateien** die Datei oder den Ordner und drück **Teilen**.
2. Gib die Adresse der anderen Person ein.
3. Wähl **Kann bearbeiten** oder **Kann ansehen** und drück **Teilen**.

Die Liste im selben Fenster zeigt jede Person, mit der du geteilt hast, und
wie das Angebot steht: wartend, angenommen oder abgelehnt. Dort änderst du,
was sie tun darf, oder beendest das Teilen mit dem Kreuz. Danach ist die
Datei auch von ihrer Box verschwunden.

## Was du bekommst \{#what-you-receive}

Was mit dir geteilt wurde, steht unter **Mit dir geteilt**. Ein neues
Angebot wartet, bis du es **Annehmen** oder **Ablehnen** wählst. Danach:

- eine Textdatei öffnet sich im Editor, zusammen mit ihrem Besitzer und
  allen anderen, mit denen sie geteilt ist;
- ein Ordner öffnet sich als Liste, seine Dateien öffnest oder lädst du von
  dort;
- jede andere Datei wird heruntergeladen.

Die Datei bleibt auf der Box ihres Besitzers. Deine Box behält eine Kopie
einer Textdatei, die du bearbeitest, sodass du weiterschreiben kannst, wenn
die andere Box nicht erreichbar ist. Der Editor sagt das dann, und **Mit
dir geteilt** zeigt, dass deine Änderungen warten. Sobald sich die Boxen
wieder erreichen, kommen die Änderungen beider Seiten zusammen, ohne dass
eine Zeile verloren geht.

## Wie Boxen einander finden \{#how-boxes-find-each-other}

Eine Box antwortet anderen Boxen unter `https://<name>.local/mesh/v1/link`
oder unter dem öffentlichen Namen, den ihr der Edge gibt. Jede Box hat einen
eigenen Schlüssel. Wenn sich zwei Boxen das erste Mal melden, merkt sich
jede den Schlüssel der anderen und weist danach eine Box ab, die unter
demselben Namen mit einem anderen kommt. An dieser Adresse meldet sich
niemand an; wer spricht, entscheiden die Schlüssel.

## Einstellungen \{#settings}

Sie stehen im Bereich **Erweitert**:

- `losos.cloud.mesh.enable` schaltet das Teilen mit anderen Boxen ein und
  aus. Es ist an, wenn die Box ihren Speicher oder ihre Rechenleistung mit
  einem Mesh teilt. Gemeinsames Bearbeiten auf einer Box geht so oder so.
- `losos.cloud.mesh.name` ist der Name, unter dem andere Boxen diese
  erreichen, der Teil nach dem `@` in den Adressen der Leute. Im Netz der
  Box ist es `<name>.local`, über einen Edge der Name, den der Edge der Box
  gibt. Andere Boxen merken sich den Schlüssel dieser Box unter diesem
  Namen, ein neuer Name heißt also neu teilen.
- `losos.cloud.mesh.selfSigned` lässt die Box das selbst signierte
  Zertifikat einer anderen Box annehmen. Es ist an, solange die Box nicht
  über einen Edge erreicht wird.
