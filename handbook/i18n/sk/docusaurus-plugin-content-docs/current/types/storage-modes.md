---
title: Režimy úložiska
sidebar_position: 6
---

# Režimy úložiska

Karta úložiska na stránke Prehľad ukazuje jeden z dvoch stavov a panel
O zariadení ukazuje to isté ako odznak: **len pre seba**, vykreslené plnou
výplňou, alebo **zdieľané so sieťou mesh**, vykreslené šrafovaním. Všade,
kde sa dve veci líšia textúrou a nie farbou, admin stránky používajú tú istú
dvojicu: plná výplň pre toto zariadenie, šrafovanie pre mesh.

## Len pre seba (predvolené) \{#kept-to-itself-default}

Všetko, čo máte na zariadení, zostáva na zariadení. Zariadenie nič
nekopíruje nikam inam. Disk má dve časti:

![Panel Úložisko na zariadení, ktoré si disk nechává pre seba: použité miesto, voľné miesto a rezerva, s tlačidlom Použiť rezervu.](@site/docs/img/storage-sk.png)

- **použité miesto**, ktoré zaberá LosOS cloud, LosOS Git a systém;
- **rezerva**, časť, ktorú inštalátor odložil bokom. Panel Úložisko ju môže
  kedykoľvek zabrať, pozrite si [zväčšenie disku](../manual/disk-growth).

## Zdieľané so sieťou mesh \{#shared-with-the-mesh}

Zariadenie požičiava mesh miesto, ktoré nevyužíva. Toto miesto sa pridá do
replikovaného spoločného priestoru, ktorý cez zariadenia jedného edge
prevádzkuje Longhorn. Zariadenie si za to môže samo zabrať miesto
v spoločnom priestore. Platia tri veci:

![Mesh: tri zariadenia, každé si nechá súbory svojho vlastníka pre seba a miesto, ktoré nevyužíva, požičia do jedného replikovaného spoločného priestoru; dole okno od 23:00 do 07:00, keď sa požičiava voľný čas, iba v nečinnosti.](@site/docs/img/mesh-sk.svg)

![Panel Úložisko na zariadení, ktoré zdieľa: miesto požičané mesh je vykreslené šrafovane vedľa vlastného miesta zariadenia.](@site/docs/img/storage-shared-sk.png)

- Zdieľané dáta sú na zariadení v samostatnom účte `shared`, v adresári
  šifrovanom kľúčom zapečateným v TPM. Keď je zdieľanie vypnuté, zariadenie
  kľúč nenačíta a adresár je nečitateľný aj pre samotné zariadenie. Vaše
  vlastné súbory pod `notshared` a zdieľané dáta sa navzájom čítať nemôžu.
- Zdieľanie potrebuje dostupný edge. Bez neho zariadenie prepínač odmietne.
- Prepínač je na paneli **Trh**, pretože požičať disk a dostať za to
  zaplatené je jedno rozhodnutie. Funguje bez ohľadu na to, či edge tomuto
  zariadeniu trh ponúka.

## Požičiavanie voľného času (výpočtový výkon) \{#lending-spare-time-compute}

Toto je od disku oddelené a nachádza sa na paneli Mesh. Zariadenie spúšťa
prácu iných ľudí iba vtedy, keď platí oboje:

![Panel Mesh s pripojeným zariadením: Požičiavať toto zariadenie, kým spím, je zapnuté a hodiny sú nastavené od 23:00 do 07:00.](@site/docs/img/mesh-two-edges-sk.png)

1. čas je vnútri okna, ktoré ste nastavili, so začiatkom a koncom vo vašom
   časovom pásme;
2. zariadenie je nečinné.

Vyťaženosť môže okno zavrieť skôr, mimo nastavených hodín ho však nič
neotvorí. Čokoľvek neznáme sa počíta ako vyťaženosť. Edge posudzuje okno
v časovom pásme vášho zariadenia, takže okno od 23:00 do 07:00 zostane od
23:00 do 07:00 aj pri prechode na letný a zimný čas.
