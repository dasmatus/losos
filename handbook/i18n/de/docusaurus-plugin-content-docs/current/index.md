---
sidebar_class_name: hb-icon hb-icon-home
slug: /
title: LosOS-Handbuch
sidebar_position: 0
sidebar_label: Willkommen
---

# LosOS-Handbuch

LosOS ist eine kleine Box für dein Zuhause oder deine Firma. Sie verwahrt
deine Dateien, Fotos, deinen Kalender und deinen Code und kümmert sich um
sich selbst. Sie hat keinen Bildschirm zum Anmelden, keine Shell und kein
SSH. Du erreichst sie aus einem Webbrowser im selben Netzwerk, und einmal pro
Nacht baut sie sich neu und startet neu.

![Die Admin-Seiten einer Box namens mattbox: die App-Kacheln, die Speicherkarte und das Board mit den Widgets.](@site/docs/img/overview.png)

Dieses Handbuch ist die Bedienungsanleitung für die Person, bei der die Box
auf dem Schreibtisch steht. Wer LosOS baut, eine Edge betreibt oder ein
Release erstellt, liest das Kapitel [Im Detail](/in-depth/index.md).

## Wo dieses Handbuch zu finden ist \{#where-this-handbook-is}

- **Auf der Box selbst**, unter `http://<address>/handbook/`, wobei
  `<address>` das ist, was der eigene Bildschirm der Box anzeigt. Die Box hat
  das ganze Handbuch samt Suche dabei, du kannst es also auch lesen, wenn das
  Internet ausgefallen ist oder die Box nur aus deinem lokalen Netzwerk
  erreichbar ist. Der Bereich **Über** der Admin-Seiten verlinkt darauf.
- **Im Web**, unter [losos.dasmat.us](https://losos.dasmat.us). Der Text ist
  derselbe, nur mit einem neueren Datum.

## Hier anfangen \{#start-here}

- Neue Box in der Hand: [Was LosOS ist](/start/what-is-losos.md), danach
  [Installation](/start/install.md) und [Der erste Start](/start/first-run.md).
- Du überlegst, wie du sie einsetzt: [Arten der Einrichtung](/types/index.md)
  führt jede Form auf, die eine LosOS-Installation annehmen kann, von einer
  Box im Heimnetz bis zu einer Firma mit eigener Edge.
- Etwas stimmt nicht, und du hast nur das lokale Netzwerk:
  [Wenn etwas nicht funktioniert](/troubleshooting/index.md) ist danach
  geordnet, was du siehst, und jede Seite beginnt mit den Schritten, für die
  das LAN genügt.
- Du willst die Box aus einem Terminal auf einem anderen Rechner verwalten:
  [Der Terminal-Client](/manual/terminal.md). Lies das vor der Installation,
  denn der Schlüssel, den er braucht, kommt auf den Installationsstick.
- Die Box funktioniert, aber etwas außerhalb nicht, etwa der Provider, der
  Router oder die Edge: [Nur das lokale Netzwerk funktioniert](/troubleshooting/only-the-lan-works.md)
  sagt, was weiterläuft, was wartet, und dass du danach nichts neu machen
  musst.

## Wie die Adressen zu lesen sind \{#how-to-read-the-addresses}

In diesem Handbuch steht `<address>` für die IP-Adresse auf dem Bildschirm
der Box, etwa `192.168.1.23`, und `<name>` für den Namen, den du der Box bei
der Einrichtung gegeben hast. Die Box antwortet sowohl unter
`http://<address>` als auch unter `http://<name>.local`. Nur Rechner, die
mDNS-Namen auflösen, finden die zweite, also ist die Adresse immer die
sichere Wahl. Siehe [Die Box erreichen](/start/reaching-the-box.md).
