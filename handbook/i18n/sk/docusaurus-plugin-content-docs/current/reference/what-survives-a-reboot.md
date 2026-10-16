---
title: Čo prežije reštart
sidebar_position: 2
---

# Čo prežije reštart

Zariadenie pri každom štarte zostaví svoj systémový disk z ničoho. Ponechá
si iba tieto adresáre na šifrovanom zväzku a cez bind mount ich vráti na
ich miesto:

| Ponechané                  | Obsahuje                                                              |
| -------------------------- | --------------------------------------------------------------------- |
| `/home/notshared`          | Dáta LosOS cloud: vaše súbory, fotky, databázu                        |
| `/home/shared`             | Miesto požičané mesh sieti, šifrované zvlášť a nečitateľné, kým je zdieľanie vypnuté |
| `/var`                     | Dáta LosOS Git, stav a tajomstvá zariadenia, stav oboch clustrov, logy |
| `/nix`                     | Samotný systém, každá verzia ponechaná pre bootovacie menu            |
| `/etc/nixos`               | Vlastný opis zariadenia: jeho disky, režim firmvéru, režim odomykania, vaše nastavenia |
| `/etc/keys`                | Záložná kópia kľúča disku                                             |
| `/etc/ssh`                 | Kľúče hostiteľa, ktoré zariadenie identifikujú. SSH server nebeží.    |
| `/etc/rancher`             | Heslo uzla pre členstvo v mesh sieti                                  |
| `/etc/machine-id`          | Identita zariadenia                                                   |

![Systémový disk je v RAM a pri každom štarte sa zostaví nanovo; deväť adresárov sa cez bind mount vráti zo šifrovaného zväzku /persist a všetko ostatné zmizne pri reštarte o 00:07.](@site/docs/img/what-survives-sk.svg)

Všetko ostatné, aj čokoľvek, čo útočník alebo chyba zapísali kdekoľvek inde,
o 00:07 zmizne.

## Dve veci, ktoré sú dokumenty, nie nastavenia \{#two-things-that-are-documents-not-settings}

Zariadenie uchováva **Vzhľad**, teda pozadie, závoj a ručne písané widgety,
pod `/var`. Rozloženie nástenky si pamätá váš prehliadač. Obe prežijú
reštart, ale ani jedno nie je súčasťou opisu zariadenia a obe môžete zmeniť
bez prestavby.

## Koľko miesta zaberajú záznamy \{#how-much-room-the-logs-take}

Záznamy pod `/var` majú strop, takže disk zaplniť nemôžu:

- Systémový žurnál zaberá najviac 512 MB a nič staršie ako mesiac.
  Veľkosť nastavuje `losos.logs.journalMaxUse` na paneli Rozšírené.
- LosOS cloud, LosOS Git a každá aplikácia si nechávajú najviac tri súbory
  záznamov po 10 MB na kontajner.
- Prístupový záznam webového servera začína nový súbor každý týždeň, alebo
  skôr pri 20 MB, a nechá si osem starých.
- Auditný záznam stránok správy a záznam prestavieb, ktoré spúšťajú,
  začínajú nový súbor pri 10 MB a nechajú si tri staré.
- Záznamy zostavení z aktualizácií sa po 30 dňoch mažú.

## Čo urobí preinštalovanie \{#what-a-reinstall-does}

Vymaže to všetko. Pozrite si [Reset a preinštalovanie](../manual/reset-and-reinstall).
