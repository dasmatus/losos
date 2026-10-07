---
title: Cenová ponuka
sidebar_position: 8
---

# Cenová ponuka hardvérového a softvérového vybavenia

Bod 3 zadania: *spracujte cenovú ponuku hardvérového a softvérového
vybavenia potrebného na realizáciu projektu.* Táto kapitola hovorí, z čoho
sa ponuka skladá a prečo; konkrétne položky s cenami a zdrojmi sú
v samostatnom dokumente **Cenová ponuka LosOS** a tabuľke
*cenova-ponuka.xlsx*, ktoré sú súčasťou materiálov k obhajobe (autor sa
rozhodol udržiavať ich mimo repozitára kódu, lebo ceny sa menia a repozitár
nesie len kód, Markdown a obrázky). Pri odovzdaní sa dokument s cenami
prikladá za touto kapitolou; jeho výsledok je zhrnutý na konci kapitoly.

## Z čoho sa realizácia skladá

LosOS má dve polohy a ponuka ich oddeľuje:

1. **Jeden box doma alebo v kancelárii.** Potrebuje hardvér boxu, USB kľúč
   na inštaláciu a sieť, ktorá už existuje. Softvér je celý slobodný, teda
   bez licenčných poplatkov.
2. **Firemné nasadenie s vlastným edge** (niekoľko boxov v jednej sieti,
   verejná adresa a mesh medzi nimi). K tomu pribúda edge server, doména
   a prípadne hosting; a prácu inštalácie a zaškolenia, ktorú autor plánuje
   ponúkať po maturite.

Tretia poloha, **trh**, nie je položka ponuky, ale obchodný model: majiteľ
boxu predáva kapacitu, ktorú už zdieľa, a platforma si necháva 4 %.

## Hardvér boxu

Požiadavky vyplývajú z kódu, nie z marketingu: 64-bitový procesor x86-64,
aspoň 4 GiB pamäte (8 GiB pre pohodlný beh Nextcloudu a Forgejo spolu
s Kubernetes), disk 40 GiB alebo väčší (reálne podľa toho, koľko dát má
box držať a zdieľať), káblová sieť, a pre predvolený spôsob odomykania
disku čip TPM 2.0 (firmvérový Intel PTT alebo AMD fTPM stačí; bez neho box
beží v režime kľúčového súboru). Grafický čip nie je potrebný.

Ponuka preto pracuje s dvoma úrovňami:

| Úroveň                    | Čo to je                                                                                | Pre koho                              |
| ------------------------- | --------------------------------------------------------------------------------------- | ------------------------------------- |
| **Úsporná**               | repasované kancelárske mini-PC (ThinkCentre Tiny, EliteDesk Mini, OptiPlex Micro) z druhej ruky, 8 GiB RAM, SSD 256 GiB a viac | domácnosť, ukážka, prvý box             |
| **Priestrannejšia**       | nové mini-PC s Intel N100/N305 alebo Ryzen, 16 GiB RAM, NVMe 1 TB a viac, 2,5 GbE      | firma, box so zdieľaním do meshu        |

K obom patrí USB kľúč 2 GiB a viac (jednorazovo) a sieťový kábel. Druhý
disk nie je potrebný: inštalátor spája všetky pevné disky do jedného
zväzku, a redundanciu rieši mesh, nie RAID.

## Hardvér edge

Edge je modul NixOS a jeho nároky sú malé: registrátor v Ruste, Traefik,
rathole a voliteľne rke2 server s Longhornom. Pre firemné nasadenie v
jednej sieti by prichádzalo do úvahy aj zariadenie triedy Raspberry Pi
(ARM64), flake však dnes zostavuje len pre `x86_64-linux`, takže je to
položka na overenie, nie hotová možnosť; istá voľba je malý x86-64 stroj
v sieti alebo, pre verejnú adresu z internetu, malý VPS. Ponuka uvádza obe možnosti vedľa seba.

## Softvér

Celý softvérový stack je slobodný softvér; licenčné náklady sú nulové.
Ponuka ich uvádza, aby bolo vidieť, čo projekt používa a za akých podmienok:

| Zložka                          | Licencia                 | Úloha v LosOS                                  |
| ------------------------------- | ------------------------ | ---------------------------------------------- |
| LosOS (tento projekt)           | AGPL-3.0-or-later        | moduly, démon, UI, inštalátor, edge             |
| NixOS / nixpkgs                 | MIT                      | základ systému, zostavenie, cache               |
| Linux                           | GPL-2.0                  | jadro                                           |
| Nextcloud                       | AGPL-3.0                 | LosOS cloud                                     |
| Forgejo                         | GPL-3.0-or-later         | LosOS Git                                       |
| k3s, rke2, containerd           | Apache-2.0               | beh aplikácií a mesh                            |
| Longhorn                        | Apache-2.0               | replikované úložisko meshu                      |
| Traefik, rathole                | MIT, Apache-2.0          | TLS a tunel na edge                             |
| disko, impermanence             | MIT                      | rozloženie diskov, bezstavový koreň             |
| systemd (cryptenroll), LUKS, fscrypt | LGPL/GPL            | šifrovanie a odomykanie                         |

Platené služby vstupujú až s firemným nasadením alebo trhom: doména
(autorova `dasmat.us` stojí približne 8 EUR ročne), hosting edge (VPS)
alebo demo hostiteľa riadiacej roviny (Vercel a Neon vo voľných
úrovniach), poplatky Stripe pri trhu (pre karty EÚ približne 1,5 % + 0,25
EUR za transakciu, čo pri 4 % platforme znamená bod zvratu okolo 10 EUR na
objednávku; pod ním platforma dopláca, čo recenzia v kapitole 7 odhalila a
ponuka s tým počíta minimom objednávky).

## Čo ponuka zámerne neobsahuje

- **Cenu práce autora na vývoji.** Projekt je maturitná práca; ponuka
  oceňuje, čo si zákazník kupuje, nie čo vznik systému stál.
- **Prémiový hardvér.** Systém beží pohodlne na ľubovoľnom stroji x86-64
  s dostatkom pamäte a disku; drahší box neprinesie nič, čo by softvér
  využil. Jediná tvrdá hranica je architektúra: ARM (vrátane Macov s čipom
  Apple M) podporovaná nie je.
- **Zálohovanie.** Box je jedna kópia; druhú si drží majiteľ, kde chce.

## Výsledok ponuky

Dokument s cenami (stav k 7. októbru 2026, slovenské maloobchodné ceny
s DPH 23 %, zdroj a dátum pri každej položke) počíta dve konfigurácie:

| Konfigurácia                                   | Z čoho sa skladá                                                           | Jednorazovo   | Ročne        |
| ---------------------------- | ---------------------------------------------------------- | ------------------ | ---------------- |
| **A: maturitná ukážka**                        | jeden box (Acer Veriton N150), doména (Namecheap, 7,79 € ročne), elektrina | 452,59 €      | 25,31 €      |
| A + voliteľný edge v cloude                    | k tomu Hetzner CX23 ako edge                                               | 452,59 €      | 113,72 €     |
| **B: nasadenie vo firme**                      | tri boxy, lokálny x86 edge, prepínač, doména, elektrina                    | 1 795,59 €    | 74,37 €      |

Softvér nestojí nič: každý komponent je slobodný (tabuľka licencií vyššie),
takže softvérová časť ponuky sú len prevádzkové služby (doména, prípadne
VPS). Predpoklady: elektrina 0,20 € za kWh, práca autora neocenená (pozri
predchádzajúcu časť), Raspberry Pi ako edge je uvedené len orientačne a označené *na
overenie*, lebo flake zatiaľ zostavuje len x86_64. Konfigurácia A je to,
čo sa predvádza na obhajobe (vo virtuálnom stroji, so schváleným nahradením
fyzického boxu); konfigurácia B je ponuka, s ktorou chce autor po maturite
osloviť firmy.
