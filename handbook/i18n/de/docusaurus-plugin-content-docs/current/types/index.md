---
title: Arten der Einrichtung
sidebar_position: 0
sidebar_label: Überblick
slug: /types
---

# Arten der Einrichtung

„Welches LosOS habe ich?“ hat fünf Antworten zugleich, weil sich eine Box
entlang fünf voneinander unabhängiger Linien unterscheidet. Diese Seite fasst
jede Linie zusammen und verweist auf ihre eigene Seite.

![Die drei Arten der Bereitstellung nebeneinander: eine Box allein in einem Heimnetz; eine Box, die einen Tunnel zur offiziellen LosOS-Edge öffnet; mehrere Boxen in einem Büronetz, die über die eigene Edge der Firma einen gemeinsamen Pool teilen.](@site/docs/img/setup-types-de.svg)

## 1. Wie die Box bereitgestellt wird \{#1-how-the-box-is-deployed}

| Art                                    | Für wen                                         | Fernzugriff   | Mesh                | Markt  |
| -------------------------------------- | ----------------------------------------------- | ------------- | ------------------- | ------ |
| [Eine Box für sich allein](/types/single-box.md)         | Ein Haus, eine Wohnung, ein Schreibtisch        | nein          | nein                | nein   |
| [Eine Box mit der LosOS-Edge](/types/official-edge.md) | Besitzer, die eine öffentliche Adresse und das Mesh wollen | ja           | ja, mit anderen     | ja, sobald er öffnet |
| [Eine Firma mit eigener Edge](/types/company-edge.md) | Eine Firma, die mehrere Boxen in einem Netz betreibt | ja, über ihre eigene Edge | ja, unter ihren eigenen Boxen | nein |

Jede Box beginnt als erste Art. Zu den anderen beiden kommst du, indem du
Einstellungen änderst, nicht durch eine Neuinstallation.

## 2. Wie die Festplatte entsperrt wird und wie der Rechner startet \{#2-how-the-disk-is-unlocked-and-how-the-machine-boots}

[Installationsvarianten](/types/install-variants.md) behandelt beides. Der
Installer wählt die Entsperrung per TPM oder per Schlüsseldatei anhand der
Hardware, die er vorfindet, und übernimmt **UEFI** oder **BIOS** aus dem
Firmware-Menü. Es gibt drei Installationsmedien: das kleine Installer-ISO,
das ISO mit dem vollständigen System darauf und das Demo-Festplatten-Image für
virtuelle Maschinen.

## 3. Wie die Apps laufen \{#3-how-the-apps-run}

[App-Modi](/types/app-modes.md) erklärt die beiden Modi. Standardmäßig laufen
LosOS Cloud und LosOS Git als Workloads im eigenen kleinen Kubernetes-Cluster
der Box. Sie können auch nativ auf dem Host laufen. Du siehst in beiden Fällen
dieselben Apps.

## 4. Wofür die Festplatte da ist \{#4-what-the-disk-is-for}

[Speichermodi](/types/storage-modes.md) behandelt die Festplatte und die CPU.
Eine Box behält ihre Festplatte für sich oder teilt freien Platz mit dem Mesh.
Außerdem kann sie freie CPU-Leistung innerhalb eines täglichen Zeitfensters
verleihen, und nur, solange sie untätig ist.

## 5. Was auf dem Board ist \{#5-what-is-on-the-board}

[Widget-Arten](/types/widget-types.md) listet die drei Arten auf dem Board der
Übersicht auf: eingebaute Widgets, Widgets, die ohne Code aus den Messwerten
der Box gebaut werden, und von Hand in HTML und JavaScript geschriebene
Widgets, die in einer Sandbox laufen.

## Und die drei Namen \{#and-the-three-names}

Egal welche Art, dir begegnen drei Namen. LosOS Cloud enthält Dateien, Fotos
und Kalender. LosOS Git enthält Repositorys. Mit den Admin-Seiten steuerst du
die Box selbst. Das [Glossar](/reference/glossary.md) erklärt den Rest.
