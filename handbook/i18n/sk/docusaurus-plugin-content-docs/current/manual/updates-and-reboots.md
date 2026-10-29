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

![Panel História: vetva, z ktorej zariadenie berie nové verzie, a kedy ju naposledy skontrolovalo, potom zmeny.](@site/docs/img/settings-history-sk.png)

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

:::note[Vlastné zmeny zastavia aktualizácie]
Ak ste zmenili akýkoľvek iný súbor v repozitári zariadenia, napríklad
pushnutím commitu do LosOS Git z klonu, zariadenie si vašu zmenu ponechá a
neaktualizuje sa. Panel História to povie. Ak chcete projekt znova
nasledovať, vráťte tie súbory do pôvodného stavu.
:::

Riadia to tri nastavenia Nixu a panel Rozšírené ich ukazuje:

| Voľba                                 | Predvolená hodnota                      |
| ------------------------------------- | --------------------------------------- |
| `losos.configRepo.upstream.enable`    | zapnuté                                 |
| `losos.configRepo.upstream.url`       | `https://github.com/dasmatus/losos.git` |
| `losos.configRepo.upstream.branch`    | `main`                                  |

Firma môže adresu nasmerovať na vlastný fork. Repozitár musí byť čitateľný
bez hesla. Keď prvé nastavenie vypnete, zariadenie sa zostavuje iba z
verzie, ktorú má.

:::note[Bez internetu o 03:00]
Zariadenie, ktoré o 03:00 nemá internet, zostaví verziu, ktorú má, a skúsi
to znova nasledujúcu noc.
:::

## Stiahnuté zostáva na zariadení \{#downloads-stay-on-the-box}

Všetko, čo zariadenie pri zostavení stiahne, ide cez lokálnu kópiu na
zariadení, uloženú na šifrovanom disku v `/var/cache/losos-nix-cache`.
Upratovanie o 04:30 zmaže nástroje, ktoré stiahlo posledné zostavenie, a
ďalšie zostavenie ich nájde v kópii namiesto toho, aby ich sťahovalo znova.
Kópia prežije aj reštart o 00:07, takže zariadenie bez internetu o 03:00 sa
vie zostaviť z toho, čo už raz stiahlo.

Nič v kópii nie je dôveryhodné len preto, že tam je. Každý softvér v nej
nesie podpis cache, z ktorej pochádza, a zariadenie tento podpis overí pri
každej inštalácii.

| Nastavenie                      | Predvolené | Čo                                                   |
| ------------------------------- | ---------- | ---------------------------------------------------- |
| `losos.cache.local.enable`      | zapnuté    | Kópiu uchovávať                                      |
| `losos.cache.local.maxSize`     | `8g`       | Najviac miesta; ako prvé idú najdlhšie nepoužité     |
| `losos.cache.local.minFree`     | `4g`       | Voľné miesto, ktoré vždy nechá pre vaše súbory       |
| `losos.cache.local.shareOnLan`  | vypnuté    | Kópiu môžu používať iné zariadenia vo vašej sieti    |
| `losos.cache.local.peers`       | žiadne     | Kópie iných zariadení, ktoré sa spýta ako prvé       |

Ak máte doma viac zariadení, zapnite `shareOnLan` na jednom a na ostatných
ho uveďte v `peers` ako `http://<name>.local:8790`. Každú aktualizáciu potom
stiahnu z internetu spolu iba raz. Pustí len adresy z vašej vlastnej siete.

## Keď je zariadenie vypnuté \{#when-the-box-is-off}

Nič sa nestane a nič sa nestratí. Zapnite ho a naštartuje, reštartuje sa,
ak zmeškalo polnoc, a aktualizuje sa o najbližšej 03:00.
