---
title: Maschinen
sidebar_position: 4.9
slug: /manual/machines
---

# Maschinen

**Maschinen** unter Mesh in der Seitenleiste (unter `/machines`) ist der Ort,
an dem eine Box virtuelle Maschinen im Mesh mietet und Maschinen für andere
Boxen betreibt. Eine Maschine wird **pro Replikat** verkauft: Jedes Replikat
ist eine Kopie der Maschine mit eigener Festplatte und läuft einen Monat auf
der Box, die es betreibt.

Bezahlt wird über den [Marktplatz](../in-depth/market). Jeder Verkauf wird
zur Hälfte zwischen der Besitzerin oder dem Besitzer der betreibenden Box und
LosOS geteilt. Der Hypervisor ist KubeVirt im Mesh-Cluster des Edge; der
Registrar des Edge verkauft die Replikate und legt die Maschinen an.

![Der Bereich Mesh auf einer Box ohne Edge; Maschinen liegt unter Mesh.](@site/docs/img/mesh-none-de.png)

## Wer es nutzen kann

Die Box bietet Maschinen, zum Mieten wie zum Betreiben, nur an, solange sie
ihre Festplatte mit dem Mesh teilt. Eine Box, die nicht teilt, sieht einen
grauen Satz und einen Link zum Bereich Marktplatz, wo der Schalter fürs Teilen
der Festplatte liegt. Schalte das Teilen dort ein, wende die Änderung an, und
die Seite Maschinen zeigt das Formular.

## Mieten

Der Katalog listet die Größen, die ein Edge verkauft. Wähle eine aus, sag, wie
viele Replikate du willst, und bezahle. Das Edge legt die Maschinen an; jedes
Replikat bekommt eine eigene Festplatte, importiert aus dem Image, das der
Eintrag nennt. Die Seite zeigt die Maschinen, die du mietest, ihren Zustand
und wann der Zeitraum endet.

## Betreiben

Eine Box, die teilt, kann auch Maschinen betreiben. Die Seite zeigt, was du
betreibst und was du verdienst. Das Betreiben nutzt dasselbe Marktplatz-Konto
wie der Verkauf von Speicher oder Rechenleistung.

Die Seite des Edge steht unter [Virtuelle Maschinen](../in-depth/virtual-machines):
wie der Registrar Replikate verkauft, wie Festplatten importiert werden und aus
welchen Gründen die Seite sagen kann, dass keine Maschinen verfügbar sind.
