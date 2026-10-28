---
title: Einstellungen und Optionen
sidebar_position: 3
---

# Einstellungen und Optionen

Jedes Feld in den Einstellungsbereichen schreibt eine Option in die
Nix-Beschreibung der Box (`losos.*`). Nimm diese Tabelle, wenn du etwas
willst, das die Bereiche nicht anbieten. Ein Maintainer setzt diese Optionen
in `modules/overrides.nix`, und
[Administration](/in-depth/administration.md#settings) beschreibt sie
vollständig.

## In den Bereichen \{#on-the-panes}

| Bereich   | Feld                        | Option                                              | Standard                |
| --------- | --------------------------- | --------------------------------------------------- | ----------------------- |
| Netzwerk  | Name                        | `losos.hostName`                                    | `losos`                 |
| Netzwerk  | Verbindung verschlüsseln    | `losos.tls.enable`                                  | an                      |
| Netzwerk  | Auch von außerhalb erreichbar | `losos.proxy.enable`                              | aus                     |
| Netzwerk  | Adresse der Edge            | `losos.proxy.registrarUrl`                          | die LosOS-Edge          |
| Apps      | LosOS Git                   | `losos.git.enable`                                  | an                      |
| Apps      | LosOS Cloud, Tage im Papierkorb | `losos.cloud.trashDays`                         | 30                      |
| Apps      | LosOS Git, Einstellungen für alle | `losos.git.site.*` | wie LosOS Git ausgeliefert wird |
| Speicher  | Reserve nutzen              | führt `losos-ctl grow` aus, und die Reserve ist `losos.storage.fillPercent` | 90 % |
| Mesh      | Dem Mesh beitreten          | `losos.cluster.enable`                              | aus                     |
| Mesh      | Meine freie Zeit teilen     | `losos.cluster.shareCompute`                        | aus                     |
| Mesh      | Stunden                     | `losos.cluster.computeWindow.{start,end}`           | nicht gesetzt           |
| Markt     | Meine Festplatte teilen     | `losos.sharingMyStorage`                            | aus                     |
| Hardware  | Grafik                      | `losos.gpu.enable`                                  | aus                     |
| Sicherheit | AppArmor                   | `losos.hardening.apparmor`                          | aus                     |
| Sicherheit | Gehärteter Speicherallokator | `losos.hardening.malloc`                          | aus                     |
| Sicherheit | Kein SMT                   | `losos.hardening.nosmt`                             | aus                     |
| Sicherheit | USBGuard                   | `losos.hardening.usbguard`                          | aus                     |

## Nicht in den Bereichen \{#not-on-the-panes}

| Was                                  | Option                             | Standard                                   |
| ------------------------------------ | ---------------------------------- | ------------------------------------------ |
| Woher die Updates kommen             | `losos.configRepo.upstream.url`, `.branch` | `https://github.com/dasmatus/losos.git`, `main` |
| Ob sich die Box überhaupt aktualisiert | `losos.configRepo.upstream.enable` | an                                       |
| Was der nächtliche Neubau baut       | `losos.upgradeFlakeUri`            | `git+file:///etc/nixos#install`, das eigene Repository der Box |
| Die Härtungsbasis                    | `losos.hardening.enable`           | an                                         |
| Der Entsperrmodus der Festplatte     | `losos.tpm.enable`                 | was der Installer vorgefunden hat          |
| UEFI oder BIOS                       | `losos.bios`                       | was der Installer vorgefunden hat          |
| Die Festplatten                      | `losos.targetDrives`               | was der Installer vorgefunden hat          |
| Gültigkeitsdauer des Zertifikats     | `losos.tls.validityDays`           | 730                                        |
| Leerlaufschwelle für das Verleihen von CPU-Leistung | `losos.cluster.idleLoadThreshold` | 0,25 pro Kern               |
| Der Binär-Cache                      | `losos.cache.substituters`         | `http://127.0.0.1:8787`, dann `https://losos.dasmat.us/proxy` |
| Größe der lokalen Cache-Kopie        | `losos.cache.local.maxSize`        | `8g`                                        |
| Wer `/setup/state.json` von einer Webseite aus lesen darf | `losos.setup.finderOrigins` | die Suchseite der LosOS-Edge |

![Der Bereich Erweitert, in dem die Optionen bearbeitet werden, die in keinem anderen Bereich stehen, jede vor Anwenden geprüft.](@site/docs/img/settings-advanced-de.png)

## Regeln, die die Bereiche durchsetzen \{#rules-the-panes-enforce}

- Ein Name besteht aus Buchstaben, Ziffern und Bindestrichen, beginnt und
  endet mit einem Buchstaben oder einer Ziffer und hat höchstens 63 Zeichen.
- Ein Port liegt zwischen 1024 und 65535.
- Die Stunden sind zwei Uhrzeiten im 24-Stunden-Format, `HH:MM`, in der
  eigenen Zeitzone der Box. Das Zeitfenster darf über Mitternacht gehen.
