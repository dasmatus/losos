---
title: Wo die Apps laufen
sidebar_position: 5
---

# Wo die Apps laufen

LosOS Cloud und LosOS Git sind Dienste der Box selbst, wie die Admin-Seiten.
Jeder läuft als eigener Systemdienst mit eigenem Datenverzeichnis, und
keiner braucht einen Cluster. Du findest sie auf jeder Box unter denselben
Adressen.

Apps, die du im Bereich Apps installierst, laufen an einem von zwei Orten,
je nachdem, als wer sie laufen:

| Läuft als       | Wo sie läuft                                                                                     |
| --------------- | ------------------------------------------------------------------------------------------------ |
| **notshared**   | Rootless Podman, als du. Ein Cluster ist nicht nötig.                                            |
| **shared**      | Der eigene kleine Kubernetes-Cluster der Box (k3s), der nur läuft, solange die Box ihre Festplatte mit einem Mesh teilt. |

## Zwei Cluster, mit Absicht \{#two-clusters-on-purpose}

Die geteilten Apps der Box leben in ihrem eigenen Cluster. Das
[Mesh](official-edge) ist ein anderer Cluster, und die Edge betreibt ihn.
LosOS legt die Apps der Box nie in den Cluster des Mesh. Ein Mitglied eines
Clusters kann nicht starten, solange sein Server nicht erreichbar ist, und
die Box startet jede Nacht um 00:07 neu. Ein Ausfall der Edge über
Mitternacht würde diese Apps also vom Netz nehmen.

![Das Innere einer Box: Der eigene Cluster der Box betreibt die als shared installierten Apps, der Mesh-Agent gehört zu einem anderen Cluster, dessen Server die Edge ist.](@site/docs/img/box-architecture-de.svg)

## Wo die Apps sind \{#where-the-apps-are}

![Der Bereich Apps: LosOS Cloud, LosOS Git, eine installierte App und darunter Mehr finden.](@site/docs/img/apps-de.png)

| App          | Adresse             | Hinweis                                                                                   |
| ------------ | ------------------- | ----------------------------------------------------------------------------------------- |
| LosOS Cloud  | `/cloud/`           | Die Apps für Computer und Telefon bekommen die Adresse der Box ohne Pfad.                 |
| LosOS Git    | `/git/`             | Schalte es im Bereich Apps ab, wenn du es nicht willst.                                   |
| Admin-Seiten | `/`                 | Nur im LAN.                                                                               |
| Handbuch     | `/handbook/`        | Nur im LAN.                                                                               |
