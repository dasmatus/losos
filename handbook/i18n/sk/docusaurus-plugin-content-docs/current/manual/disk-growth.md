---
title: Zväčšenie disku
sidebar_position: 7
---

# Zväčšenie disku

Inštalátor zámerne nechá 10 % disku nevyužitých. Zariadenie drží svoj
vlastný softvér na tom istom šifrovanom zväzku ako vaše súbory a ten softvér
rastie s každou aktualizáciou. Zväzok, ktorý by sa nikdy nemohol zväčšiť, by
sa raz zaplnil, a to na zariadení, kde nikto nemôže otvoriť shell.

## Použitie rezervy \{#claiming-the-reserve}

**Panel Úložisko → Použiť rezervu.** Dialóg povie, koľko miesta tým
pribudne. Zariadenie zväčší zväzok, šifrovaciu vrstvu a súborový systém, v
tomto poradí. Nič sa nezastaví a nič sa neodpojí. Ukazovateľ kapacity na
paneli zobrazuje rezervu ako šrafovaný dielik, kým si ju nevezmete.

![Dialóg Použiť rezervu: koľko miesta pribudne, s tlačidlom Zrušiť ako prvým.](@site/docs/img/storage-use-reserve-sk.png)

![Tri vrstvy zväčšené v poradí: logický zväzok, potom šifrovacia vrstva, potom súborový systém.](@site/docs/img/disk-growth-sk.svg)

Keď je rezerva vyčerpaná, na tlačidle stojí **Rezerva využitá** a zostane
sivé v každom prehliadači aj po opätovnom načítaní: rezervu hlási samo
zariadenie.

## Pridanie disku \{#adding-a-disk}

Zariadenie je jedna skupina zväzkov cez všetky pevné disky, ktoré inštalátor
našiel. Ak chcete neskôr pridať disk, pridáte ho do tejto skupiny a zväzok
znova zväčšíte. Na to treba príkazový riadok zariadenia, ktorý existuje iba
na inštalačnom médiu:

```sh
pvcreate /dev/sdX && vgextend persist-vg /dev/sdX && losos-ctl grow
```

Je to práca pre toho, kto zariadenie spravuje, nie tlačidlo pre vlastníka.
Pre väčšinu ľudí je jednoduchšie celé zariadenie preinštalovať s novým
diskom na mieste. Najprv zálohujte, pretože inštalátor zmaže všetko.

## Keď dochádza miesto \{#running-out-of-room}

Prehľad ukáže **Takmer plné** a potom **Nie je miesto**. Plný disk zabráni
LosOS cloud prijímať nahrávané súbory. Horšie je, že môže zastaviť nočné
zostavenie. Postup nájdete v časti [Nie je miesto](../troubleshooting/out-of-room).

![Prehľad na takmer plnom zariadení: dlaždica Súbory a karta úložiska varujú.](@site/docs/img/overview-almost-full-sk.png)
