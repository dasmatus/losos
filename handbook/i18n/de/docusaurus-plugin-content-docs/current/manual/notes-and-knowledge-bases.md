---
title: Notizen und Wissensbasen
sidebar_position: 4.42
slug: /manual/notes-and-knowledge-bases
---

# Notizen und Wissensbasen

**Notizen** in [LosOS Cloud](losos-cloud) öffnet einen Ordner mit
Markdown-Dateien so, wie Obsidian einen Vault öffnet: die Notizen links, die
offene Notiz in der Mitte und daneben die Notizen, die auf sie verlinken.
Ein Obsidian-Vault, der in LosOS Cloud synchronisiert wird, öffnet sich so,
wie er ist, mit seinen Links, Einbettungen, Tags und Canvases. Solange die
Box teilt und in einem Mesh ist, kannst du einen Ordner mit Notizen auch als
Wissensbasis veröffentlichen: eine Seite, die Leute im Browser lesen.

## Deine Notizen öffnen \{#open-your-notes}

- **Notizen** in der Seitenleiste öffnet den Ordner `Notes` in deinen
  Dateien. Er wird beim ersten Öffnen angelegt.
- Klick in **Dateien** mit der rechten Maustaste auf einen Ordner und wähl
  **Als Notizen öffnen**.
- Klick in **Dateien** auf eine `.md`-Datei. Sie öffnet sich in ihrem Vault:
  dem nächsten Ordner darüber, in dem ein Ordner `.obsidian` liegt, so wie
  Obsidian einen Vault findet, sonst in dem Ordner, in dem die Datei liegt.

## Eine Notiz lesen \{#reading-a-note}

Eine Notiz wird aus ihrem Markdown gezeichnet. Beide Arten, die Leute
schreiben, werden verstanden:

- die von GitHub: Tabellen, Aufgabenlisten, ~~Durchstreichen~~, Fußnoten,
  Links, die mit `https://` beginnen, und Hinweise wie `> [!NOTE]`.
- die von Obsidian: `[[Links]]` auf andere Notizen, auch
  `[[Notiz|angezeigter Text]]`, `[[Notiz#Überschrift]]` und
  `[[Notiz#^Block]]`; `![[Einbettungen]]` einer Notiz, eines Teils davon
  oder eines Bildes; Callouts, auch einklappbare (`> [!tip]-`); `#Tags` und
  `#verschachtelte/Tags`; `==Hervorhebungen==`; `%%Kommentare%%`, die nicht
  angezeigt werden; Mathematik zwischen `$` und `$$`; und die Eigenschaften
  am Anfang einer Notiz.

Codeblöcke werden nach Sprache eingefärbt. Mermaid-Diagramme bleiben Code:
Um sie zu zeichnen, braucht es Styles, die die Seiten der Box nicht
erlauben.

Ein Link auf eine Notiz, die es noch nicht gibt, ist blasser. Klick darauf,
um die Notiz anzulegen. Hakst du eine Aufgabe ab, wird die Notiz mit dem
Haken gespeichert.

## Neben einer Notiz \{#beside-a-note}

- **Lokaler Graph**: die Notiz und die Notizen, auf die sie verlinkt oder
  die auf sie verlinken, ein bis drei Links entfernt. Klick auf einen Punkt,
  um diese Notiz zu öffnen.
- **Verlinkt hierher**: die Notizen, die auf diese verlinken.
- **Verlinkt auf** und **Noch nicht geschrieben**: wohin diese Notiz
  verlinkt.
- **Gliederung**: ihre Überschriften. Klick auf eine, um hinzuspringen.

## Bearbeiten, Mindmap und Graph \{#edit-mind-map-and-graph}

- **Bearbeiten** öffnet die Notiz in dem Editor, in den Leute gemeinsam
  schreiben (siehe
  [Teilen und gemeinsam bearbeiten](sharing-and-editing-together)), auf
  einem breiten Bildschirm mit der gezeichneten Notiz neben dem Text. Tipp
  `[[`, um eine Notiz zum Verlinken auszuwählen. Markier Wörter, im Editor
  oder beim Lesen, für
  [eine Leiste mit Aktionen](admin-pages#the-bar-over-a-selection) darüber.
- **Mindmap** zeichnet die Überschriften der Notiz und die Listen darunter
  als Äste. Klick auf den Ast einer Überschrift, um diesen Teil zu lesen.
- Der Knopf **Graph aller Notizen** über den Notizen zeigt jede Notiz im
  Ordner und jeden Link zwischen ihnen. Schalter fügen Tags, Anhänge, Links
  auf noch nicht geschriebene Notizen und Notizen hinzu, auf die nichts
  verlinkt. Das Feld filtert nach Namen.

Das Suchfeld über den Notizen findet Wörter in Namen und im Text. **Tags**
listet jeden Tag auf; klick auf einen, um seine Notizen zu sehen.

## Einen Ordner als Wissensbasis veröffentlichen \{#publish-a-folder-as-a-knowledge-base}

Wissensbasen brauchen eine Box, die teilt und in einem Mesh ist, wie Chat
und Mail. Ohne beides fehlt der Menüpunkt und die Seiten sind aus.

1. Klick in **Dateien** mit der rechten Maustaste auf den Ordner und wähl
   **Als Wissensbasis veröffentlichen**.
2. Gib ihr einen Titel und wähl, wer sie lesen kann:
   - **Alle mit dem Link** lesen ohne Anmeldung, überall, wo die Box
     erreichbar ist: in ihrem Netzwerk und über ihren Edge, wenn sie einen
     hat.
   - **Leute auf dieser Box** melden sich zuerst mit LosOS Login an.
3. Drück **Veröffentlichen**. Das Fenster zeigt die Adresse der Seite, zum
   Beispiel `https://<name>.local/kb/kitchen-garden/`.

Die Seite folgt dem Ordner: Eine Notiz, die du änderst oder hinzufügst,
erscheint dort sofort. Jede Notiz hat ihre eigene Adresse, etwa
`/kb/kitchen-garden/Tomatoes`. Leser bekommen dieselben Links,
Einbettungen, Suche, Graph und Mindmap, und nichts, womit sie die Notizen
ändern könnten.

Nicht auf der Seite:

- Notizen, deren Eigenschaften `publish: false` oder `private: true` sagen;
- Ordner, deren Name mit einem Punkt beginnt, darunter `.obsidian`;
- alles außerhalb des Ordners.

Um den Titel oder die Leser zu ändern, klick den Ordner wieder mit der
rechten Maustaste an und wähl **Wissensbasis**. **Nicht mehr
veröffentlichen** nimmt die Seite vom Netz; die Notizen bleiben in deinen
Dateien.

Hört die Box auf zu teilen oder verlässt sie das Mesh, antwortet keine
Seite mehr. Sie kommen so zurück, wie sie waren, sobald die Box wieder in
einem Mesh teilt.
