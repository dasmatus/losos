---
title: Aussehen und Widgets
sidebar_position: 5
slug: /manual/look-and-widgets
---

# Aussehen und Widgets

## Dein Board \{#your-board}

Das Board der Übersicht fasst bis zu einer Handvoll Widgets. **Widget hinzufügen**
öffnet die Galerie mit den drei [Widget-Typen](../types/widget-types). Du
kannst jede Kachel nach vorn oder nach hinten schieben oder sie entfernen.
Ein Widget, das scheitert, zeigt den Fehler in seiner Kachel, mit **Erneut
versuchen**. Das Board gehört zu dem Browser, in dem du es angeordnet hast.
Ein anderer Rechner sieht sein eigenes Board. Ein eingebautes Widget oder
eines aus Messwerten hinzuzufügen ändert nichts auf der Box und startet
nichts.

![Die Übersicht mit dem Hintergrund Gezeiten und einem von Hand geschriebenen Widget Begrüßung oben auf dem Board.](@site/docs/img/overview-tide-de.png)

## Einstellungen → Aussehen \{#settings--look}

Diesen Bereich haben alle, die angemeldet sind. Der Hintergrund darauf
gehört nur dir: Alle auf der Box wählen ihren eigenen, und deiner begleitet
dich in jeden Browser, in dem du dich anmeldest, LosOS Cloud und LosOS Git
eingeschlossen. Eine Änderung wirkt, sobald du sie speicherst, ohne
Neuaufbau.

![Einstellungen, Aussehen: die Hintergründe Schlicht, Gezeiten, Raster, Abenddämmerung und dein eigenes Bild, der Schieberegler für den Schleier und die von Hand geschriebenen Widgets auf dieser Box.](@site/docs/img/settings-look-de.png)

- **Dein Hintergrund**: eines von drei mitgelieferten Bildern (Gezeiten,
  Raster, Abenddämmerung), ein eigenes hochgeladenes Bild (PNG, JPEG, WebP
  oder GIF, bis 8 MiB) oder Schlicht.
- **Schleier**: legt die Seitenfarbe über das Bild, von 20 % bis 90 %, damit
  Text im hellen und im dunklen Design lesbar bleibt.
- **Von Hand geschriebene Widgets**: die von Hand geschriebenen Widgets, die
  auf dieser Box gespeichert sind, für die Leute, die das Aussehen der Box
  ändern dürfen. Du kannst jedes bearbeiten oder löschen.

## Ein Widget schreiben \{#writing-a-widget}

**Eins schreiben**, in der Galerie oder im Bereich Aussehen, öffnet einen
Editor mit den Dateien des Widgets, einer Live-Vorschau und einem Tab mit
Hilfe. Die Vorschau ist der echte Rahmen in der Sandbox. Das Widget bekommt
die Messwerte und Farben der Box über das Objekt `losos`, das auf der Seite
[Widget-Typen](../types/widget-types#written-by-hand) beschrieben ist, und
sonst nichts. Es hat keinen Admin-Schlüssel, keine API und keinen Zugriff
auf die Seite um es herum. Es darf Daten aus dem Internet laden.

Ein neues Widget beginnt mit drei Dateien: `index.html`, das `style.css` und
`app.js` einbindet. **Datei hinzufügen** fügt eine weitere hinzu, etwa
`data.json`, die das Skript per `fetch` lädt, oder `dot.svg` für ein
`<img>`. Der Editor macht beim Tippen Vorschläge: das Objekt `losos`, die
Namen der Messwerte, die Farbvariablen der Box nach `var(--` und die eigenen
Dateinamen des Widgets. Strg+Leertaste fragt überall nach Vorschlägen.

![Der Widget-Editor: links die Dateien und ihr Code, rechts die Live-Vorschau im echten Sandbox-Rahmen, und der Tab mit Hilfe.](@site/docs/img/widget-write-one-de.png)

:::warning[Lies eingefügten Code zuerst]
Ein von Hand geschriebenes Widget läuft in jedem Browser, der die Box
öffnet. Lies alles, was du aus dem Internet einfügst, bevor du es behältst,
wie es auch der Hinweis über dem Editor sagt.
:::
