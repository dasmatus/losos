---
title: LosOS Wiki
sidebar_position: 4.65
slug: /manual/wiki
---

# LosOS Wiki

LosOS Wiki hält Dokumentation so, wie es ein Team-Wiki tut: Bereiche mit
Seiten in einem Baum, im Browser geschrieben, und jeder Bereich zusätzlich
als eigene Website veröffentlicht. **Wiki** in der Seitenleiste öffnet es
unter `/wikis`.

Es ist auch das Verzeichnis der Wikis auf der Box. Eine Liste zeigt jedes
Wiki auf der Box, das du lesen darfst:

- **Bereiche**: die Wikis von Repositorys in LosOS Git, hier geschrieben.
- **Wissensdatenbanken**: Ordner mit Notizen, veröffentlicht aus den
  [Notizen in LosOS Cloud](notes-and-knowledge-bases). Sie werden in den
  Notizen geschrieben; das Verzeichnis verlinkt ihre Seiten.
- **Eingebaut**: dieses Handbuch.

Die Suche über der Liste durchsucht die Seiten aller Bereiche und
Wissensdatenbanken auf einmal und bietet dieselben Wörter der Suche des
Handbuchs an.

## Bereiche \{#spaces}

Ein Bereich ist das Wiki eines Repositorys in [LosOS Git](losos-git).
Seine Seiten sind Markdown-Dateien in einem eigenen Git-Repository neben
dem Code, `<Repository>.wiki.git`, im selben Aufbau wie bei Forgejo und
GitHub. Eine Box, die von Forgejo umgezogen ist, hat ihre Wikis schon dort,
und sie erscheinen ohne Import als Bereiche.

Wer was darf, richtet sich nach dem Repository:

| Das Repository ist | Wer den Bereich liest | Wer darin schreibt |
| --- | --- | --- |
| Öffentlich | Alle mit der Adresse | Wer hineinpushen darf |
| Für Personen auf der Box | Alle, die auf der Box angemeldet sind | Wer hineinpushen darf |
| Privat | Wem es gehört und wer mitarbeitet | Wer hineinpushen darf |

**Neuer Bereich** legt ein Repository mit diesem Namen und der gewählten
Sichtbarkeit an und schreibt seine erste Seite. Um einem bestehenden
Repository ein Wiki zu geben, öffne es in LosOS Git und wähle **Wiki** in
seinem Menü.

## Seiten \{#pages}

- Jede Seite ist eine Datei. „Getting started“ ist `Getting-started.md`.
- Eine Seite unter einer anderen ist eine Datei in einem Ordner: **Neue
  Seite unter dieser** auf `Guides` legt `Guides/<Titel>.md` an. Der Baum
  links folgt den Ordnern.
- `Home` öffnet sich zuerst. Ohne sie öffnet sich die erste Seite.
- `_Sidebar.md` und `_Footer.md` rahmen jede Seite neben ihnen, seitlich
  und unten. Ein Ordner kann eigene haben.
- `[[Seite]]` und `[[Seite|Text]]` verlinken eine Seite desselben Bereichs,
  ebenso ein einfacher Link auf `Seite` oder `Seite.md`. In einem Ordner
  beginnen Links bei diesem Ordner.
- Bilder, die im Bereich liegen (PNG, JPEG, GIF, WebP oder AVIF), erscheinen
  auf der Seite: `![Die Beete](beds.png)`. Bilder von anderen Websites
  bleiben Links.

**Bearbeiten** öffnet die Seite in einem Markdown-Editor mit Vorschau. Jedes
Speichern ist ein Commit im Verlauf des Bereichs, mit deinem Namen und einer
optionalen Notiz, was sich geändert hat; **Änderungen** unter der Seite
listet sie auf. Hat jemand anderes die Seite gespeichert, während du
geschrieben hast, wird das Speichern abgelehnt und dein Text bleibt im
Editor, sodass du die Seite neu öffnen und ihn wieder einfügen kannst.
**Seite löschen** entfernt die Datei ebenfalls mit einem Commit, der Text
bleibt also im Verlauf.

## Mit Git \{#with-git}

Jeder Bereich ist ein Git-Repository. Klone es mit der Adresse unter seinem
Seitenbaum:

```sh
git clone https://<box>/git/<Besitzer>/<Repository>.wiki.git
```

Zum Pushen braucht es dieselben Rechte und denselben Token wie für das
Repository selbst (siehe [LosOS Git](losos-git)). Der erste Push an die
Wiki-Adresse eines Repositorys legt den Bereich an. Im Browser geschriebene
und mit Git gepushte Seiten sind dieselben Dateien.

## Die veröffentlichte Seite \{#the-published-site}

Jeder Bereich ist auch eine Website unter
`https://<box>/wiki/<Besitzer>/<Repository>/`, wie die Seiten eines Projekts
auf GitHub Pages: der Seitenbaum, die Seite und eine Suche im Bereich, ohne
etwas ändern zu können. Sie ist für alle offen, die den Bereich lesen
dürfen; ein öffentlicher Bereich lässt sich also ohne Anmeldung lesen, auch
über eine Edge. Wer schreiben darf, sieht **Bearbeiten**, das die Seite in
LosOS Wiki öffnet. `https://<box>/wiki/` listet alle Bereiche, die du lesen
darfst.

Hält LosOS Git seine Seiten hinter LosOS Login
(`losos.git.site.requireSignIn`), liegen auch die veröffentlichten Seiten
dahinter.
