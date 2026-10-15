---
title: Typy nasadenia
sidebar_position: 0
sidebar_label: Prehľad
slug: /types
---

# Typy nasadenia

Otázka „Aký LosOS mám?“ má naraz päť odpovedí, pretože zariadenia sa líšia
v piatich na sebe nezávislých smeroch. Táto stránka každý z nich zhrnie
a odkáže na jeho vlastnú stránku.

![Tri typy nasadenia vedľa seba: samostatné zariadenie v domácej sieti; zariadenie, ktoré otvára tunel k oficiálnemu edge LosOS; niekoľko zariadení vo firemnej sieti, ktoré zdieľajú jeden spoločný priestor cez vlastný edge firmy.](@site/docs/img/setup-types-sk.svg)

## 1. Ako je zariadenie nasadené \{#1-how-the-box-is-deployed}

| Typ                                    | Pre koho je                                     | Vzdialený prístup | Mesh                | Trh    |
| -------------------------------------- | ----------------------------------------------- | ----------------- | ------------------- | ------ |
| [Samostatné zariadenie](/types/single-box.md)         | Domácnosť, byt, jeden pracovný stôl | nie               | nie                 | nie    |
| [Zariadenie s edge LosOS](/types/official-edge.md) | Vlastníci, ktorí chcú verejnú adresu a mesh | áno         | áno, s ostatnými    | áno, keď sa otvorí |
| [Firma s vlastným edge](/types/company-edge.md) | Firma, ktorá prevádzkuje niekoľko zariadení v jednej sieti | áno, cez vlastný edge | áno, medzi vlastnými zariadeniami | nie |

Každé zariadenie začína ako prvý typ. K ďalším dvom sa dostanete zmenou
nastavení, nie preinštalovaním.

## 2. Ako sa odomyká disk a ako počítač štartuje \{#2-how-the-disk-is-unlocked-and-how-the-machine-boots}

Obe veci opisuje stránka [Varianty inštalácie](/types/install-variants.md).
Inštalátor zvolí odomykanie cez TPM alebo cez súbor s kľúčom podľa hardvéru,
ktorý nájde, a **UEFI** alebo **BIOS** prevezme z ponuky firmvéru. Inštalačné
médiá sú tri: malé inštalačné ISO, ISO s celým systémom a demo obraz disku pre
virtuálne stroje.

## 3. Ako bežia aplikácie \{#3-how-the-apps-run}

Dva režimy vysvetľuje stránka [Režimy aplikácií](/types/app-modes.md).
Predvolene bežia LosOS cloud a LosOS Git ako záťaže vo vlastnom malom
Kubernetes klastri zariadenia. Môžu bežať aj natívne priamo na hostiteľovi.
V oboch prípadoch vidíte tie isté aplikácie.

## 4. Na čo slúži disk \{#4-what-the-disk-is-for}

Disk a CPU opisuje stránka [Režimy úložiska](/types/storage-modes.md).
Zariadenie si disk nechá pre seba, alebo voľné miesto zdieľa s mesh. Môže
tiež požičiavať voľný výkon CPU v dennom časovom okne, a to iba vtedy, keď
je nečinné.

## 5. Čo je na nástenke \{#5-what-is-on-the-board}

Stránka [Typy widgetov](/types/widget-types.md) uvádza tri druhy widgetov na
nástenke Prehľadu: vstavané widgety, widgety zostavené z údajov zariadenia
bez kódu a widgety napísané ručne v HTML a JavaScripte, ktoré bežia
v sandboxe.

## A tri názvy \{#and-the-three-names}

Nech je typ akýkoľvek, stretnete sa s tromi názvami. LosOS cloud uchováva
súbory, fotky a kalendáre. LosOS Git uchováva repozitáre. Admin stránky
spravujú samotné zariadenie. Ostatné nájdete v [slovníku pojmov](/reference/glossary.md).
