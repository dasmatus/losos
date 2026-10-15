---
title: Aktualizácie a reštarty
sidebar_position: 6
---

# Aktualizácie a reštarty

Na každom zariadení bežia dva časovače. Panel O zariadení ukazuje oba.

![Noc na zariadení od 22:00 do 06:00: reštart o 00:07, aktualizácia o 03:00, upratovanie o 04:30.](@site/docs/img/nightly-cycle-sk.svg)

| Čas       | Čo                                                                                                                                       |
| --------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| **00:07** | **Reštart.** Bez podmienok. Systémový disk začína prázdny a zariadenie ho znova zostaví zo svojho opisu, takže týmto reštartom sa zariadenie samo opravuje. Zariadenie, ktoré bolo o 00:07 vypnuté, sa reštartuje hneď, ako sa zapne. |
| **03:00** | **Aktualizácia.** Zariadenie stiahne z GitHubu najnovšiu verziu LosOS, ponechá si vlastné nastavenia a zostaví sa nanovo na jej základe. |
| **04:30** | **Upratovanie.** Zariadenie zmaže verzie systému staršie ako 14 dní a bootovacia ponuka si ponechá posledných päť. Tak sa disk ani bootovací oddiel nezaplnia. |

## Čo prežije \{#what-survives}

Iba to, čo je na krátkom zozname: vaše súbory a dáta LosOS Git, vlastné
úložisko systému, nastavenia, ktoré ste zvolili, kľúče a identita
zariadenia, členstvo v mesh. Všetko ostatné o 00:07 zmizne. Zoznam je na
[referenčnej stránke](../reference/what-survives-a-reboot).

![Panel O zariadení, ktorý ukazuje časy reštartu a aktualizácie a kedy každý naposledy prebehol.](@site/docs/img/settings-about.png)

## Počas práce \{#while-it-works}

Zariadenie zostáva dostupné, kým uplatňuje aktualizáciu alebo zmenu.
Aplikácie odpovedajú, až kým sa zariadenie neprepne na nový systém, čo trvá
sekundy. Ak aktualizácia zlyhá, ďalej beží predchádzajúci systém a
zariadenie to skúsi znova nasledujúcu noc.

## Odkiaľ prichádzajú nové verzie \{#where-new-versions-come-from}

Zariadenie drží svoj opis vo vlastnom repozitári, v tom, ktorý LosOS Git
ukazuje na paneli História. Pred zostavením o 03:00 stiahne najnovší commit
vetvy `main` z `https://github.com/dasmatus/losos.git` a urobí z neho ďalšiu
verziu zariadenia ako jeden commit s názvom `Update to dasmatus/losos main`
a krátkym identifikátorom commitu. Dva súbory zostanú tak, ako ich má
zariadenie: `modules/overrides.nix`, ktorý drží vaše nastavenia, a
`modules/install-target.nix`, ktorý drží zoznam diskov, režim firmvéru a
spôsob odomykania. Zostavenie potom zostaví tento commit a rovnako každé
Použiť po ňom.

Ak ste zmenili akýkoľvek iný súbor v repozitári zariadenia, napríklad
pushnutím commitu do LosOS Git z klonu, zariadenie si vašu zmenu ponechá a
neaktualizuje sa. Panel História to povie. Ak chcete projekt znova
nasledovať, vráťte tie súbory do pôvodného stavu.

Riadia to tri nastavenia Nixu a panel Rozšírené ich ukazuje:

| Voľba                                 | Predvolená hodnota                      |
| ------------------------------------- | --------------------------------------- |
| `losos.configRepo.upstream.enable`    | zapnuté                                 |
| `losos.configRepo.upstream.url`       | `https://github.com/dasmatus/losos.git` |
| `losos.configRepo.upstream.branch`    | `main`                                  |

Firma môže adresu nasmerovať na vlastný fork. Repozitár musí byť čitateľný
bez hesla. Keď prvé nastavenie vypnete, zariadenie sa zostavuje iba z
verzie, ktorú má.

Zariadenie, ktoré o 03:00 nemá internet, zostaví verziu, ktorú má, a skúsi
to znova nasledujúcu noc.

## Keď je zariadenie vypnuté \{#when-the-box-is-off}

Nič sa nestane a nič sa nestratí. Zapnite ho a naštartuje, reštartuje sa,
ak zmeškalo polnoc, a aktualizuje sa o najbližšej 03:00.
