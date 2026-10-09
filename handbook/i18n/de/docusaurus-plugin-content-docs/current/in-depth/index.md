---
title: Im Detail
sidebar_position: 0
sidebar_label: Überblick
slug: /in-depth
mdx:
  format: md
---

# Im Detail

Der Rest dieses Handbuchs richtet sich an die Besitzer einer Box. Dieses
Kapitel ist für die Leute, die LosOS bauen, eine Edge betreiben oder ein
Release erstellen: wie die Teile zusammenspielen, warum jedes dort sitzt, wo
es sitzt, und wie man sie ändert.

LosOS ist eine NixOS-Appliance für einen Mini-PC. Sie betreibt Nextcloud für
deine eigenen Dateien und kann freien Speicherplatz und CPU-Leistung an ein
Mesh anderer LosOS-Boxen verleihen. Es gibt kein SSH und keine Login-Shell.
Du verwaltest die Box über eine Webseite, und sie aktualisiert sich selbst.

Das Root-Dateisystem ist ein tmpfs, das bei jedem Start neu aufgebaut wird.
Nur die in `modules/impermanence.nix` aufgeführten Verzeichnisse bleiben
erhalten, und zwar auf einer verschlüsselten Partition `/persist`.

## Seiten

- [Installation im Detail](/in-depth/install.md). Das Installer-ISO, Secure Boot, TPM, das
  Demo-Image, das Vergrößern der Festplatte.
- [Administration](/in-depth/administration.md). Die Admin-UI, Einstellungen, Updates
  und der nächtliche Neustart.
- [Mesh](/in-depth/mesh.md). Speicher und Rechenleistung für andere Boxen bereitstellen.
- [Marktplatz](/in-depth/market.md). Den Speicher und die CPU verkaufen, die eine
  Box schon teilt (geplant).
- [Master-Proxy](/in-depth/master-proxy.md). Die Box aus dem Internet erreichen, ohne
  einen Port zu öffnen.
- [Edge-Föderation](/in-depth/edge-federation.md). Deine eigene Edge im LAN, weitergeleitet über die offiziellen.
- [Lab](/in-depth/lab.md). Der Visualisierer unter `/lab/`, seine simulierten und
  echten Gäste.
- [Härtung](/in-depth/hardening.md). Die standardmäßige Härtungsbasis und die
  optionalen Schalter.
- [TPM und der Festplattenschlüssel](/reference/tpm.md). Was der Chip leistet und
  worauf eine Box ohne ihn verzichtet.
- [Sicherheitsmodell](/reference/security-model.md). Wogegen geschützt wird und wogegen
  nicht.
- [Architektur](/in-depth/architecture.md). Wie die Teile zusammenspielen.
- [Entwicklung](/in-depth/development.md). Die Dev-Shell, Tests, Lock-Dateien.
- [CI und Releases](/in-depth/ci-and-releases.md). CI-Jobs, Release-Medien, der
  Binary-Cache.

## Im Vergleich mit Alternativen

- **Ein NAS (Synology, QNAP).** Dieselbe Idee, eine Box mit Web-UI. Der
  Unterschied: Das Root-Dateisystem wird hier bei jedem Start verworfen,
  sodass die Änderungen eines Angreifers nur bis zum nächsten Neustart
  bestehen, und der kommt spätestens jede Nacht um 00:07. Außerdem kannst du
  alles, was auf der Box läuft, lesen und selbst neu bauen.
- **Ein VPS.** Die Box steht bei dir zu Hause, und ihre Festplatte ist
  verschlüsselt. Der Schlüssel ist im TPM-Chip der Box versiegelt oder liegt
  auf einem Rechner ohne Chip auf der Boot-Partition. Der optionale
  [Master-Proxy](/in-depth/master-proxy.md) gibt ihr eine öffentliche Adresse ohne
  eingehenden Port.
- **Reines NixOS.** Du könntest das alles selbst schreiben. Hier ist es
  bereits geschrieben, und Benutzertrennung, verschlüsselte Persistenz,
  Vergrößern der Festplatte, Härtung und die Bedingungen für das Mesh haben
  jeweils einen VM-Test.
- **Andere Self-Hosting-Lösungen.** Keine davon verleiht Kapazität an ein
  Mesh nur dann, wenn das Zeitfenster des Besitzers offen ist *und* die Box
  untätig ist.

## Was es nicht ist

- Kein Allzweck-Server. Eine Shell gibt es bewusst nicht.
- Kein Backup. Bewahre Kopien von `/persist` an anderer Stelle auf.
- Nicht fertig.
