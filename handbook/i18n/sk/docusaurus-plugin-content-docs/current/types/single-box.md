---
title: Samostatné zariadenie
sidebar_position: 1
---

# Samostatné zariadenie

Toto je najjednoduchší LosOS a po prvom spustení tu začína každé zariadenie.
Je to jeden počítač v jednej lokálnej sieti. Dostanú sa k nemu počítače
a telefóny v tej sieti a nič iné.

## Čo dostanete \{#what-you-get}

- LosOS cloud a LosOS Git na adrese zariadenia, pre každého v sieti, kto má
  účet.
- Admin stránky, odomknuté heslom vlastníka.
- Nočnú samoopravu a samoaktualizáciu, šifrovaný disk a základný hardening.
- Príručku, ktorú práve čítate, na `/handbook/`.

## Čo nedostanete \{#what-you-do-not-get}

- **Žiadny prístup zvonku.** Zariadenie nič nezverejňuje a neotvára žiadny
  port na vašom routeri. Ak sa chcete k súborom dostať z kaviarne,
  potrebujete [edge LosOS](official-edge) alebo vlastnú VPN do domácej siete.
- **Žiadny mesh.** Zariadenie nemá komu požičať disk ani CPU a prepínače na
  paneli Mesh zostávajú vypnuté. Bez dostupného edge zariadenie odmietne
  zdieľanie úložiska, ktoré závisí od siete, takže prepínač omylom nezapnete.
- **Žiadny trh.**

![Panel Mesh na zariadení bez edge: Žiadny okrajový proxy server sa nenašiel, s tým, čo zariadenie skúsilo, a prepínače mesh sivé s textom Vyžaduje dostupný okrajový proxy server.](@site/docs/img/mesh-none-sk.png)

## Kedy je to správny typ \{#when-this-is-the-right-type}

Pre väčšinu domácností. Samostatné zariadenie má najmenej ciest dovnútra.
Jediná je prehliadač vo vašej LAN. Aktualizácie prichádzajú aj tak, pretože
si ich zariadenie sťahuje samo a cesta aktualizácií žiadny edge nepotrebuje.

## Dôležité nastavenia \{#settings-that-matter}

| Panel        | Nastavenie                  | Predvolené                                       |
| ------------ | --------------------------- | ------------------------------------------------ |
| Sieť         | Názov                       | `losos`, kým ho nenastavíte v sprievodcovi        |
| Sieť         | Šifrovať pripojenie         | zapnuté; certifikát je vlastný certifikát zariadenia |
| Sieť         | Dostupné aj mimo domova     | vypnuté, a práve to z neho robí samostatné zariadenie |
| Aplikácie    | LosOS Git                   | zapnuté                                           |
| Hardvér      | Grafika                     | vypnuté, pokiaľ zariadenie nemá GPU, ktoré chcete dať k dispozícii podom |
| Zabezpečenie | štyri ďalšie ochrany        | vypnuté; každá niečo stojí                        |

:::tip[Prechod na ďalší typ]
Keď zapnete **Dostupné aj mimo domova**, kým je dostupný edge, zariadenie sa
stane ďalším typom. Nič sa nepreinštaluje.
:::
