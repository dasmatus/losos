---
title: Kde bežia aplikácie
sidebar_position: 5
---

# Kde bežia aplikácie

LosOS Cloud a LosOS Git sú služby samotného zariadenia, tak ako admin
stránky. Každá beží ako vlastná systémová služba s vlastným dátovým
adresárom a ani jedna nepotrebuje klaster. Na každom zariadení ich nájdete
na tých istých adresách.

Aplikácie nainštalované z panela Aplikácie bežia na jednom z dvoch miest
podľa toho, pod kým bežia:

| Beží ako        | Kde beží                                                                                         |
| --------------- | ------------------------------------------------------------------------------------------------ |
| **notshared**   | Rootless Podman, pod vami. Klaster netreba.                                                      |
| **shared**      | Vlastný malý Kubernetes klaster zariadenia (k3s), ktorý beží iba vtedy, keď zariadenie zdieľa disk s mesh. |

## Dva klastre, zámerne \{#two-clusters-on-purpose}

Zdieľané aplikácie zariadenia bežia v jeho vlastnom klastri.
[Mesh](official-edge) je iný klaster a prevádzkuje ho edge. LosOS aplikácie
zariadenia do klastra mesh nikdy neumiestni. Člen klastra nedokáže
naštartovať, kým je jeho server nedostupný, a zariadenie sa každú noc
o 00:07 reštartuje. Výpadok edge cez polnoc by tak tieto aplikácie odpojil.

![Vnútri zariadenia: vlastný klaster zariadenia prevádzkuje aplikácie inštalované ako shared, agent mesh patrí do iného klastra, ktorého serverom je edge.](@site/docs/img/box-architecture-sk.svg)

## Kde sú aplikácie \{#where-the-apps-are}

![Panel Aplikácie: LosOS Cloud, LosOS Git, jedna nainštalovaná aplikácia a pod nimi Nájsť ďalšie.](@site/docs/img/apps-sk.png)

| Aplikácia    | Adresa              | Poznámka                                                                                  |
| ------------ | ------------------- | ----------------------------------------------------------------------------------------- |
| LosOS Cloud  | `/cloud/`           | Aplikácie pre počítač a telefón dostanú adresu zariadenia bez cesty.                      |
| LosOS Git    | `/git/`             | Ak ho nechcete, vypnite ho na paneli Aplikácie.                                           |
| Admin stránky | `/`                | Iba LAN.                                                                                  |
| Príručka     | `/handbook/`        | Iba LAN.                                                                                  |
