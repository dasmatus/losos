---
title: LosOS Tag
sidebar_position: 4.7
slug: /manual/losos-tag
---

# LosOS Tag

LosOS Tag ist der Assistent, den du im LosOS Chat ansprichst. Schreibe
`@LosOS` in eine Nachricht — oder nimm den Vorschlag an, den der
Composer zeigt, wenn Fragen im Raum unbeantwortet blieben — und die Box
antwortet im Thread mit dem, was ihre eigene Suche fand: deine Dateien
in LosOS Cloud, der Code in LosOS Git, deine Mail, die Räume des Chats.
Nichts verlässt die Box; die Antwort läuft auf dem eigenen Modell der
Box.

Der Tag lebt in **Einstellungen › LosOS Tag**, wo du vier Dinge
gestaltest.

## Stufen und Bereiche

Eine *Stufe* ist eines der Wörter dieser Box dafür, wie privat ein Stück
Daten ist, am wenigsten privat zuerst (`everyday` als Standard; ein
Administrator fügt weitere hinzu — `personal`, `secret`, welche Wörter
der Box auch immer). Ein *Bereich* bewertet ein Stück der Box-Daten mit
einer Stufe: ein Ordner in deinen Dateien, ein Repository, ein Raum, ein
Postfach, eine Journaleinheit.

Die Regel ist eine Zeile: **Eine Tag liest einen Bereich nur, wenn ihre
Freigabe die Stufe des Bereichs erreicht.** Je privater ein Bereich,
desto höher die Freigabe, die der Assistent braucht. Deine eigene
Freigabe (was der Tag und der Begleiter in deinen KI-Werkzeugen in
deinem Namen lesen) setzt du auf derselben Seite.

## Assistenten

Jeder Assistent ist ein echter Chat-Beteiligter — `@LosOS` ist der
eingebaute — mit eigener Persona, Freigabe und Urheber. Sprich ihn in
einer Nachricht an und er antwortet; sprich zwei an und sie antworten,
und eine Antwort, die einen anderen Assistenten anspricht, gibt den Zug
weiter, höchstens drei Übergaben, sodass zwei Assistenten einen Thread
unter sich tragen können, ohne endlos zu kreisen. Ein Assistent liest
nur bis zu seiner eigenen Freigabe, was seine Persona auch sagt: die
Stufen sind die Regel der Box, nicht die Laune des Modells.

## Fähigkeiten

Eine Fähigkeit ist eine Anweisung, die eine Anfrage mitnimmt — die
mitgelieferten kommen mit der Box (*Finde, wo ich es schrieb*, *Fasse
diesen Thread zusammen*, *Was habe ich verpasst*, *Plane es*), und
eigene kannst du schreiben. Eine Fähigkeit hebt nie eine Freigabe und
vergibt nie ein Werkzeug; sie sagt nur, wie die Antwort anzugehen ist.

## Was die Tag niemals tut

Bei aller Freigabe ruft eine Tag niemals `sudo` oder eine Shell auf,
kauft, verkauft, vermietet oder verleiht niemals Speicher oder Rechenzeit
und fasst niemals etwas auf der Festplatte in den Verzeichnissen des
Eigentümers an — dessen Dateien werden über die eigene Anwendung von
LosOS Cloud gelesen und geschrieben, nicht über einen Assistenten. Eine
Websuche, wenn die Box eine betreibt (ihr eigenes SearXNG, niemals das
offene Internet), nimmt nur das Thema: keine Namen, Dateien oder Mails
aus der Box gelangen je in eine Anfrage.

## Größere Modelle, über das Mesh

Das Modell der Tag ist das eine vereinheitlichte Modell der Box
(standardmäßig Bonsai 27B — eine 1-Bit-Aufzeichnung von Qwen3.6-27B:
4,4 GB, Werkzeuge, Denken, Sehen). Eine Box, die es nicht halten kann,
fragt stattdessen über das Netzwerk: In einem Mesh betreibt die Edge
einen Kopf pro Modellgruppe und die Boxen leihen ihr ihren Speicher
([das Mesh](../in-depth/mesh), [KI-Modelle](../in-depth/ai-models)),
sodass eine 4-GiB-Box Qwen 3.8 Max oder DeepSeek R1 fragen kann — der
Kopf antwortet, die Verleiher halten die Gewichte. Die Warteschlange
pausiert, worauf niemand wartet, solange die Box wenig Speicher hat, und
nimmt es von selbst wieder auf; nichts wird verworfen.
