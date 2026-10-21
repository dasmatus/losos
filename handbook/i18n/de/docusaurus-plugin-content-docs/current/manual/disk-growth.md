---
title: Festplatte vergrößern
sidebar_position: 7
---

# Festplatte vergrößern

Der Installer lässt absichtlich 10 % der Festplatte ungenutzt. Die Box hält
ihre eigene Software auf demselben verschlüsselten Volume wie deine Dateien,
und diese Software wächst mit jedem Update. Ein Volume, das nie wachsen
könnte, wäre eines Tages voll, und das auf einer Box, auf der niemand eine
Shell öffnen kann.

## Die Reserve freigeben \{#claiming-the-reserve}

**Bereich Speicher → Reserve nutzen.** Ein Dialog sagt, wie viel Platz das
hinzufügt. Die Box vergrößert das Volume, die Verschlüsselungsschicht und
das Dateisystem, in dieser Reihenfolge. Nichts hält an, und nichts wird
ausgehängt. Die Füllstandsanzeige des Bereichs zeigt die Reserve als
schraffiertes Stück, bis du sie freigibst.

![Der Dialog Reserve nutzen: wie viel Platz er hinzufügt, mit Abbrechen an erster Stelle.](@site/docs/img/storage-use-reserve-de.png)

![Die drei Schichten, der Reihe nach vergrößert: das logische Volume, dann die Verschlüsselungsschicht, dann das Dateisystem.](@site/docs/img/disk-growth-de.svg)

Ist die Reserve aufgebraucht, lautet die Schaltfläche **Reserve genutzt**
und bleibt ausgegraut, in jedem Browser und auch nach einem Neuladen: Die
Box meldet die Reserve selbst.

## Eine Festplatte hinzufügen \{#adding-a-disk}

Eine Box ist eine Volume Group über alle fest eingebauten Festplatten, die
der Installer gefunden hat. Um später eine Festplatte hinzuzufügen, nimmst
du sie in diese Gruppe auf und vergrößerst erneut. Dafür braucht es die
Kommandozeile der Box, und die gibt es nur auf dem Installationsmedium:

```sh
pvcreate /dev/sdX && vgextend persist-vg /dev/sdX && losos-ctl grow
```

:::danger[Eine Neuinstallation löscht alles]
Das ist eine Aufgabe für die Person, die die Box wartet, keine Schaltfläche
für Besitzer. Für die meisten ist eine vollständige Neuinstallation mit der
neuen Festplatte im Rechner einfacher. Sichere vorher, denn der Installer
löscht alles.
:::

## Wenn der Platz ausgeht \{#running-out-of-room}

Die Übersicht zeigt **Fast voll** und dann **Kein Platz mehr**. Eine volle
Festplatte hindert LosOS Cloud daran, Uploads anzunehmen. Schlimmer noch, sie
kann den nächtlichen Neuaufbau verhindern. [Kein Platz
mehr](../troubleshooting/out-of-room) beschreibt die Schritte.

![Die Übersicht auf einer fast vollen Box: Die Kachel Dateien und die Speicherkarte warnen.](@site/docs/img/overview-almost-full-de.png)
