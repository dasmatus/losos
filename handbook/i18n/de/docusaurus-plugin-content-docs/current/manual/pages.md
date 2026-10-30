---
title: Seiten ändern
sidebar_position: 5.1
slug: /manual/pages
---

# Seiten ändern

Jede Seite der Verwaltung ist in MDX geschrieben: Markdown mit ein paar
Tags. Ein Administrator kann ändern, was eine Seite sagt und in welcher
Reihenfolge sie ihre Teile zeigt, eigene Schaltflächen hinzufügen und allen
Seiten ein Stylesheet geben. Eine Seite, die niemand geändert hat, sieht
genau so aus, wie LosOS sie ausliefert.

Bisher lassen sich die Übersicht und Speicher ändern. Die anderen
Bildschirme und die Apps folgen.

## Wer sie ändern kann \{#who-can-change-them}

Nur Administratoren. Den Abschnitt **Seiten** unter **Einstellungen →
Aussehen** sehen nur sie, und die Box lehnt eine Änderung von allen anderen
ab. Alle, die angemeldet sind, sehen die Seiten so, wie ein Administrator
sie hinterlassen hat, in jedem Browser, mit und ohne JavaScript.

## Eine Seite bearbeiten \{#editing-a-page}

**Einstellungen → Aussehen → Seiten** listet jede Seite, markiert mit **Wie
ausgeliefert** oder **Geändert**. Der Stift daneben öffnet die Seite mit dem
Editor daneben (auf dem Telefon darunter). Die Vorschau ist die Seite
selbst: was du tippst, erscheint sofort darauf, und gespeichert wird erst,
wenn du **Speichern** drückst.

Eine Seite besteht aus drei Arten von Dingen, in der Reihenfolge, in der du
sie schreibst:

- **Text**: Überschriften, Absätze, Listen, Links und der Rest von Markdown.
- **Die Blöcke der Seite**: die Teile, die LosOS zeichnet, etwa die Apps
  oder die Platte, jeder in einer eigenen Zeile, etwa `<Apps />`. Ein Block,
  den du weglässt, wird nicht gezeigt. Der Editor listet die Blöcke der
  Seite, die du bearbeitest, und was jeder zeichnet.
- **Komponenten**: die, die ein LosOS-Dokument verwenden kann (Callout,
  Columns, Steps, Badge und die anderen), und `Button`.

Die Übersicht, wie LosOS sie ausliefert, besteht nur aus ihren Blöcken:

```mdx
<BoxName />

<Applying />

<Apps />

<Glance />
```

Dieselbe Seite mit einer Begrüßung, einer Schaltfläche und den Apps ohne
den Speicherstreifen darunter:

```mdx
<BoxName />

## Willkommen auf der Büro-Box

Hier liegen Dateien und Code. Frag, bevor du eine App installierst.

<Button href="/apps" variant="secondary">Apps öffnen</Button>

<Apps hideStorage />
```

Der Editor zeigt, was an einer Seite falsch ist, mit Zeile und Spalte, und
**Speichern** wartet, bis nichts mehr falsch ist. Die Box prüft jedes
Speichern genauso, eine Seite, die sich nicht lesen lässt, wird also nie
gespeichert. Auf einer Seite gibt es kein Script: `import`, `export` und
`{Ausdrücke}` aus MDX werden abgelehnt, und gezeichnet werden nur die
genannten Tags.

## Schaltflächen \{#buttons}

`<Button href="…">Text</Button>` zeichnet eine der eigenen Schaltflächen der
Seite. Die Adresse ist ein Pfad auf dieser Box (`/apps`, `/settings/look`)
oder ein `https:`-, `http:`- oder `mailto:`-Link. `variant` ist `primary`
(Standard), `secondary`, `outline`, `ghost` oder `link`; `size` ist `sm`,
`md` (Standard) oder `lg`. Eine Schaltfläche ist ein Link: sie öffnet eine
Seite und ändert von sich aus nichts.

## Das Stylesheet \{#the-stylesheet}

Die Zeile **Stylesheet** öffnet einen CSS-Editor. Sobald du es speicherst,
verwendet jede Seite das Stylesheet, und es kommt nach dem von LosOS, eine
Regel von dir gewinnt also gegen eine gleich starke. Jede Seite benennt
sich am Wurzelelement, `html[data-page="overview"]` ist also die Übersicht
und `html[data-page="settings.storage"]` Speicher. Text, den du geschrieben
hast, steht in `.mdx-page-text`. Solange der Editor offen ist, zeigt jede
Seite, die du besuchst, das Stylesheet, das du gerade tippst.

## Verlauf und zurück \{#history-and-going-back}

Jedes Speichern, Zurücksetzen und jede Änderung am Stylesheet ist ein
Commit in deinem Namen, in einem eigenen Git-Repository auf der Box:
`/var/lib/losos/pages`, getrennt von der Konfiguration der Box. Der
**Verlauf** im Editor listet die Commits einer Seite. **Zurück zum
Standard** setzt eine Seite auf die Auslieferung von LosOS zurück, und
**Stylesheet entfernen** entfernt deins. Beides sind ebenfalls Commits, der
Verlauf bleibt also.

Wenn ein Update von LosOS den Standard einer Seite ändert, die du geändert
hast, sagt der Editor es. Deine Seite bleibt, wie du sie hinterlassen hast;
**Neuen Standard öffnen** zeigt den neuen, damit du übernehmen kannst, was
er hinzugefügt hat.
