---
title: Režimy aplikácií
sidebar_position: 5
---

# Režimy aplikácií

LosOS Git beží v jednom z dvoch režimov. V oboch prípadoch vidíte tú istú
aplikáciu na tej istej adrese a panel Aplikácie ukazuje, v ktorom režime je.
LosOS Cloud režimy nemá: je to vlastná služba zariadenia, priamo na ňom.

| Režim           | Čo kde beží                                                                                               | Predvolený |
| --------------- | --------------------------------------------------------------------------------------------------------- | ---------- |
| **Záťaž**       | Aplikácia beží v kontajneri vo vlastnom malom Kubernetes klastri zariadenia (k3s), z obrazu zostaveného spolu so systémom. | áno        |
| **Natívny**     | Aplikácia beží priamo na zariadení ako systémová služba.                                                  | nie        |

## Prečo je predvolený režim záťaže \{#why-workload-is-the-default}

- Obraz aplikácie sa zostavuje a testuje spolu so systémom, takže aktualizácia
  prinesie overenú dvojicu.
- Zariadenie už prevádzkuje klaster pre mesh a jeho vlastné aplikácie
  používajú ten istý mechanizmus. Ich klaster nemá sieťový plugin. Pody
  zdieľajú sieť zariadenia, a preto odpovedajú na loopbacku.
- Každá aplikácia vidí iba svoj vlastný dátový adresár a nič iné.

## Prečo existuje natívny režim \{#why-native-exists}

Natívny režim je starší tvar. Zostáva, pretože obidva režimy čerpajú z tej
istej konfigurácie Forgejo, takže sa nemôžu rozísť. Režim sa nastavuje
v Nixe, nie na admin stránkach, a jeho prepnutie vyžaduje nové zostavenie
systému.

## Dva klastre, zámerne \{#two-clusters-on-purpose}

Vlastné aplikácie zariadenia bežia v jeho vlastnom klastri. [Mesh](official-edge)
je iný klaster a prevádzkuje ho edge. LosOS aplikácie zariadenia do klastra
mesh nikdy neumiestni. Člen klastra nedokáže naštartovať, kým je jeho server
nedostupný, a zariadenie sa každú noc o 00:07 reštartuje. Výpadok edge cez
polnoc by tak odpojil vaše vlastné súbory.

![Vnútri zariadenia: vlastný klaster zariadenia prevádzkuje LosOS Git a aplikácie inštalované ako shared, agent mesh patrí do iného klastra, ktorého serverom je edge.](@site/docs/img/box-architecture-sk.svg)

## Kde sú aplikácie \{#where-the-apps-are}

![Panel Aplikácie: LosOS Git so svojím režimom, jedna nainštalovaná aplikácia a pod nimi Nájsť ďalšie.](@site/docs/img/apps-sk.png)

| Aplikácia    | Adresa              | Poznámka                                                                                  |
| ------------ | ------------------- | ----------------------------------------------------------------------------------------- |
| LosOS Cloud  | `/cloud/`           | Aplikácie pre počítač a telefón dostanú adresu zariadenia bez cesty.                      |
| LosOS Git    | `/forgejo/`         | Adresy na klonovanie používajú názov zariadenia. Ak ho nechcete, vypnite ho na paneli Aplikácie. |
| Admin stránky | `/`                | Iba LAN.                                                                                  |
| Príručka     | `/handbook/`        | Iba LAN.                                                                                  |

