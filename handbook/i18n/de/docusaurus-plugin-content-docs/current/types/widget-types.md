---
title: Widget-Arten
sidebar_position: 7
---

# Widget-Arten

Die Übersicht hat ein **Board** mit Widgets, kleinen Kacheln, die jeweils
eine Sache über die Box zeigen. **Widget hinzufügen** öffnet die Galerie, und
die kennt drei Arten.

| Art                      | Wo es liegt        | Kann es Code ausführen?          | Wie es an die Messwerte der Box kommt                |
| ------------------------ | ------------------ | -------------------------------- | ---------------------------------------------------- |
| **Eingebaut**            | im System          | ja, in die Admin-Seite einkompiliert | direkt                                           |
| **Aus Messwerten gebaut** | in diesem Browser | nein                             | ein Messwert, ein paar Ausdrücke in einer kleinen Sprache |
| **Von Hand geschrieben** | auf der Box        | ja, in einer Sandbox             | über das Objekt `losos`, per Nachrichten             |

## Eingebaut \{#built-in}

Fünf liegen der Box bei. **Antwortet** färbt ein Jahr in Tagen danach, wie
viel jedes Tages die Box geantwortet hat. **Platz** zeigt die belegte
Festplatte und die Reserve. **Teilen** stellt verliehene Stunden den
behaltenen gegenüber. **Apps** zeigt, welche Apps gerade eben geantwortet
haben. **Änderungen** listet die angewendeten Änderungen an Einstellungen,
die neueste zuerst. Jedes zeichnet sich in einer von vier Formen:
Heatmap, Zahl, Balken oder Liste. Die eingebauten Widgets sind Teil des
eigenen Codes der Admin-Seite und kommen mit System-Updates.

![Die Widget-Galerie: die eingebauten Widgets, Eins bauen und Eins schreiben.](@site/docs/img/widget-gallery.png)

## Aus Messwerten gebaut \{#built-from-readings}

**Eins bauen** macht ein Widget ohne Code. Du wählst einen Messwert, den die
Box bereitstellt, etwa den Platz auf der Festplatte, das Zeitfenster des
Mesh oder die Einstellungen, und eine Form. Dann schreibst du mit kurzen
Ausdrücken wie `m.free / m.total`, was angezeigt werden soll. **Eins bauen**
prüft das Widget und zeigt eine Vorschau, bevor du es behältst. Das Widget
lebt in dem Browser, in dem du es gebaut hast. Es ändert nichts auf der Box
und erreicht weder das Netz noch die Seite noch irgendetwas außer seinem
einen Messwert. Das begrenzt es auch: Es kann nicht zwischen Messwerten
verzweigen, keine Schleifen laufen und sich zwischen zwei Durchläufen nichts
merken.

![Eins bauen: ein Messwert, eine Form, der anzuzeigende Ausdruck und eine Vorschau der Kachel.](@site/docs/img/widget-build-one.png)

## Von Hand geschrieben \{#written-by-hand}

**Eins schreiben** nimmt eigenes HTML, eigenen Stil und eigenes Skript an,
in ein paar Dateien: `index.html` und die Stylesheets, Skripte, Bilder und
Daten, die es beim Namen einbindet. Die Box speichert das Widget, sodass
jeder Browser, der die Box öffnet, es sieht. Es steht unter
**Einstellungen → Aussehen**, wo du es bearbeiten und löschen kannst.

![Eins schreiben: der Quelltext eines von Hand geschriebenen Widgets neben seiner Live-Vorschau im Sandbox-Frame.](@site/docs/img/widget-write-one.png)

Ein von Hand geschriebenes Widget läuft in einem Sandbox-Frame. Der Frame ist
ein eigener Origin, ohne Zugriff auf die Admin-Seiten, den Admin-Schlüssel
oder die API der Box. Mit der Box spricht das Widget über ein kleines
`losos`-Objekt, das der Frame bereitstellt:

| Aufruf                      | Was er liefert                                                                |
| --------------------------- | ----------------------------------------------------------------------------- |
| `losos.metric(name)`        | ein Promise auf einen Messwert der Box, mit denselben Namen, die die eingebauten Widgets verwenden |
| `losos.theme`, `losos.lang` | `light`/`dark` und die Sprache der Admin-Seite                                 |
| `losos.palette`             | die Farben der Box, auch als CSS-Variablen gesetzt (`var(--accent)` funktioniert) |
| `losos.onTheme(fn)`         | wird aufgerufen, wann immer der Besitzer das Theme wechselt                    |
| `losos.resize()`            | bittet das Board, die Kachel neu zu vermessen                                  |
| `losos.files`               | die Namen der Dateien des Widgets                                              |
| `losos.file(name)`          | eine Datei des Widgets, als Text                                               |
| `losos.asset(name)`         | eine Datei des Widgets als `data:`-URL, für ein Bild, das ein Skript setzt     |

Der Tab **So funktioniert es** im Editor wiederholt das mit einem Beispiel.
Seine Vorschau ist der echte Frame, die Kachel zeigt also genau, was die
Vorschau zeigt. Ein von Hand geschriebenes Widget darf Daten aus dem
Internet abrufen, etwa für eine Wetterkachel, aber nicht die API der Box
erreichen. Es läuft in jedem Browser, der die Box öffnet, also lies alles,
was du aus dem Internet einfügst, bevor du es behältst. Die Grenze liegt bei
24 Widgets mit je höchstens 12 Dateien und zusammen 128 KiB.


Unter [Aussehen und Widgets](../manual/look-and-widgets) findest du das Board
selbst, Hintergründe und den Schleier.
