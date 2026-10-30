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

![Panel Úložisko: disk zariadenia a miesto odložené na neskôr, s tlačidlom Použiť rezervu.](@site/docs/img/storage-sk.png)

- **použité miesto**, ktoré zaberá LosOS Cloud, LosOS Git a systém;
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

## Zdieľané iba s touto sieťou \{#shared-with-this-network-only}

Medzi ponechaním pre seba a zdieľaním so sieťou mesh je tretia možnosť pre
zariadenie vo firemnej sieti alebo v domácnosti s vlastnou bránou: zdieľať
s lokálnymi bránami tejto siete a s ničím ďalším.

- Dáta sú v treťom účte `kindashared`, vo vlastnom adresári šifrovanom
  vlastným kľúčom. Otvorí sa iba vtedy, keď je toto zdieľanie zapnuté.
  `notshared`, `shared` a `kindashared` sa navzájom čítať nemôžu.
- Potrebuje dostupnú **lokálnu bránu**: edge, ktorý zariadenie dosiahne vo
  vlastnej sieti a ktorého spojenie je sekundárne (pozri
  [federáciu edge](../in-depth/edge-federation)). Samotný oficiálny edge
  nestačí a zariadenie prepínač odmietne.
- Žiadny trh. Nič, čo má `kindashared`, sa neponúka, nepredáva ani neplatí
  a zariadenie, ktoré zdieľa len takto, nemôže dať na trh miesto ani čas.
  Nakupovať na trhu sa dá ďalej.
- Keď sa desať minút neozve žiadna lokálna brána, zariadenie ho samo vypne
  a znova zapne minútu po tom, ako sa niektorá opäť ozve.
- Kým je zapnuté a zariadenie nie je v sieti mesh, chat, pošta, hovory
  a Mesh CI bežia ako `kindashared` a ich dáta sú v jeho adresári. Aj
  aplikácie možno inštalovať ako `kindashared`. Zariadenie, ktoré sa neskôr
  pripojí k sieti mesh, ich spúšťa ako `shared` a začína tam odznova; čo
  má `kindashared`, zostáva v jeho vlastnom adresári.
- Prepínač **Zdieľať s bránami tejto siete** je na paneli **Mesh** spolu
  s dostupnými bránami a účtom, pod ktorým beží zdieľaná strana.

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
