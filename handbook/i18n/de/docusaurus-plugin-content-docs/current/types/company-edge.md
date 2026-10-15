---
title: Eine Firma mit eigener Edge
sidebar_position: 3
---

# Eine Firma mit eigener Edge

Eine Firma betreibt mehrere Boxen in einem Netz und einen eigenen
Edge-Server, entweder in diesem Netz oder auf einem gemieteten Server. Alles
bleibt in der Firma. Die Boxen finden die Edge im LAN, teilen Festplatte und
CPU untereinander und antworten über die Edge der Firma unter deren Domain.
In einem Netz ganz ohne Internet ist das der Weg, Speicher zu bündeln. Ein
ständig laufender Rechner im LAN betreibt die Edge, und die Boxen finden sie
von selbst.

## Was genauso ist wie bei der LosOS-Edge \{#what-is-the-same-as-the-losos-edge}

- Fernzugriff unter der Domain der Edge, ohne dass auf den Boxen Ports
  geöffnet werden.
- Das Mesh, mit einem gemeinsamen, replizierten Festplatten-Pool und freier
  Rechenleistung über die Boxen der Firma hinweg, eingeplant innerhalb der
  eigenen Stunden jeder Box.
- Erkennung im LAN. Eine Box im selben Netz findet die Edge innerhalb eines
  20-Sekunden-Suchlaufs, ohne dass ihr jemand die Adresse nennt, und der
  Bereich Mesh zeigt sie als **In diesem Netzwerk**.
- Die Admin-Seiten jeder Box bleiben nur im LAN erreichbar.

![Der Bereich Mesh in einem Firmennetz: die Büro-Edge, gefunden In diesem Netzwerk, mit dem Warnzeichen, weil sie nicht offiziell ist.](@site/docs/img/mesh-company.png)

## Was anders ist \{#what-is-different}

- **Kein Markt.** Die Edge einer Firma kann die LosOS-Signatur nicht
  vorweisen, deshalb behandelt jede Box sie als inoffiziell;
  [Offiziell oder nicht](official-edge#official-or-not) erklärt die Prüfung.
  Ihre Zeile im Bereich Mesh trägt das Warnzeichen, und der Bereich Markt
  sagt: „Der Edge-Proxy, den diese Box gefunden hat, wird nicht von LosOS
  betrieben. Speicher darüber zu teilen funktioniert; Handel braucht einen
  erreichbaren offiziellen LosOS-Edge.“ Die eigenen Boxen der Firma teilen
  Speicher und Rechenleistung untereinander kostenlos, und genau darum geht
  es.
- **Die Firma hält die Schlüssel.** Das Schlüsselpaar des Tunnels, das
  Cluster-Token und die Zulassungsliste der Boxen liegen auf der Edge der
  Firma. Nichts von den Daten oder dem Verkehr der Firma berührt das
  LosOS-Projekt.
- **Updates kommen weiterhin vom Projekt.** Eine Firma kann ihre Boxen
  stattdessen auf einen Fork zeigen lassen, weil die Quelle der Updates eine
  Einstellung ist.

## Einrichtung \{#setting-it-up}

Die Edge ist ein NixOS-Modul, das mit LosOS ausgeliefert wird,
`nixosModules.edge`. Du installierst den Edge-Server so, wie eine Box
konfiguriert wird, mit einer kurzen Nix-Datei. Sie nennt die öffentliche
Domain, die Boxen, die sich verbinden dürfen, ob die Control Plane des Mesh
laufen soll und ob sich die Edge im LAN ankündigen soll. Die Anleitung für
Betreiber steht in [Master-Proxy](/in-depth/master-proxy.md) und
[Mesh](/in-depth/mesh.md). Das Repository enthält unter `demo/edge-lan/`
außerdem eine Vorführung genau dieser Einrichtung: zwei Boxen und eine Edge
in einem virtuellen Netz. `run.sh` startet sie, und `CHECKLIST.md` gibt die
Reihenfolge vor, die sie durchläuft.

![Der Bildschirm eines Edge-Gateway-Rechners aus der Vorführung: Er kündigt sich im Netz an, und die Boxen in diesem Netz finden ihn von selbst.](@site/docs/img/edge-gateway-tty1.png)

Kurz gesagt:

1. Installiere NixOS auf dem Edge-Server und importiere das Modul.
2. Setze `losos.edge.publicDomain`, trage jede Box mit einer Token-Datei
   unter `losos.edge.tenants` ein und schalte für das Mesh
   `losos.edge.cluster.enable` ein.
3. Für eine Edge im eigenen Netz der Firma setzt du `losos.edge.lan.advertise`
   auf `true`. Die Edge kündigt sich dann per mDNS an, standardmäßig als
   `http://<edge>.local:8443`, und öffnet ihre API für das LAN. Jede Box in
   diesem Netz findet sie ohne Konfiguration.
4. Gib auf jeder Box der Box einen eigenen Namen und schalte **Auch von
   außerhalb erreichbar** ein, beides unter Netzwerk, und schalte dann
   **Dem Mesh beitreten** ein. Die Box meldet sich mit ihrem Tunnelschlüssel
   bei der Edge an, und jede Box startet mit demselben Namen, den die Edge
   nur einmal annimmt. Steht die Edge *nicht* im selben Netz, trag zuerst
   ihre Adresse unter Einstellungen → Erweitert in
   `losos.proxy.registrarUrl` ein. mDNS-Ankündigungen gehen nicht über
   Router hinweg.

## Für wen das ist \{#who-this-is-for}

Für kleine Firmen, die die Einrichtung „eine Box pro Team, ein Pool für
alle“ wollen, ohne dass etwas das Gebäude verlässt. Sie brauchen jemanden,
der einen NixOS-Server betreiben kann. In dieser Form soll LosOS verkauft
werden.
