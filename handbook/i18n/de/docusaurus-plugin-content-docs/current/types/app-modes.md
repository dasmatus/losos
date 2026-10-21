---
title: App-Modi
sidebar_position: 5
---

# App-Modi

LosOS Cloud und LosOS Git laufen jeweils in einem von zwei Modi. Du siehst in
beiden Fällen dieselbe App unter derselben Adresse, und der Bereich Apps
zeigt, in welchem Modus jede läuft.

| Modus           | Was wo läuft                                                                                              | Standard |
| --------------- | --------------------------------------------------------------------------------------------------------- | -------- |
| **Workload**    | Die App läuft in einem Container im eigenen kleinen Kubernetes-Cluster der Box (k3s), aus einem Image, das mit dem System gebaut wird. | ja       |
| **Nativ**       | Die App läuft direkt auf der Box als Systemdienst.                                                         | nein     |

## Warum Workload der Standard ist \{#why-workload-is-the-default}

- Das Image der App wird zusammen mit dem System gebaut und getestet, ein
  Update bringt also ein Paar, das nachweislich zusammen funktioniert.
- Die Box betreibt für das Mesh ohnehin schon einen Cluster, und ihre eigenen
  Apps nutzen dieselbe Technik. Ihr Cluster hat kein Netzwerk-Plugin. Die
  Pods teilen sich das Netzwerk der Box, deshalb antworten sie auf Loopback.
- Jede App sieht ihr eigenes Datenverzeichnis und sonst nichts.

## Warum es Nativ gibt \{#why-native-exists}

Nativ ist die ältere Form. Sie bleibt, weil dieselbe Nextcloud-Konfiguration
beide Modi speist, sodass die beiden nicht auseinanderlaufen können. Den
Modus legst du in Nix fest, nicht auf den Admin-Seiten, und ein Wechsel
braucht einen Neubau.

## Zwei Cluster, mit Absicht \{#two-clusters-on-purpose}

Die eigenen Apps der Box leben in ihrem eigenen Cluster. Das
[Mesh](official-edge) ist ein anderer Cluster, und die Edge betreibt ihn.
LosOS legt die Apps der Box nie in den Cluster des Mesh. Ein Mitglied eines
Clusters kann nicht starten, solange sein Server nicht erreichbar ist, und
die Box startet jede Nacht um 00:07 neu. Ein Ausfall der Edge über
Mitternacht würde deine eigenen Dateien also vom Netz nehmen.

![Das Innere einer Box: Der eigene Cluster der Box betreibt LosOS Cloud und LosOS Git, der Mesh-Agent gehört zu einem anderen Cluster, dessen Server die Edge ist.](@site/docs/img/box-architecture-de.svg)

## Wo die Apps sind \{#where-the-apps-are}

![Der Bereich Apps: LosOS Cloud und LosOS Git, jede mit ihrem Modus, eine installierte App und darunter Mehr finden.](@site/docs/img/apps-de.png)

| App          | Adresse             | Hinweis                                                                                   |
| ------------ | ------------------- | ----------------------------------------------------------------------------------------- |
| LosOS Cloud  | `/nextcloud`        | Vertraut dem Namen der Box und der Adresse, auf der eine Anfrage ankam, nie einem Platzhalter. |
| LosOS Git    | `/forgejo/`         | Clone-URLs verwenden den Namen der Box; schalte es im Bereich Apps ab, wenn du es nicht willst. |
| Admin-Seiten | `/`                 | Nur im LAN.                                                                               |
| Handbuch     | `/handbook/`        | Nur im LAN.                                                                               |

:::note[App-Kacheln gehen über index.php]
Die App-Kacheln der Admin-Seiten öffnen die Apps von LosOS Cloud über
`index.php` (`/nextcloud/index.php/apps/<app>/`), worauf jede Konfiguration
antwortet.
:::
