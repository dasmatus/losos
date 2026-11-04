---
title: Stroje
sidebar_position: 4.9
slug: /manual/machines
---

# Stroje

**Stroje** pod Mesh v bočnom paneli (na `/machines`) je miesto, kde box
prenajíma virtuálne stroje v sieti mesh a hosťuje stroje pre iné boxy. Stroj
sa predáva po **replikách**: každá replika je jedna kópia stroja s vlastným
diskom, ktorá mesiac beží na boxe, ktorý ju hosťuje.

Platí sa cez [Trh](../in-depth/market). Každý predaj sa delí na polovicu
medzi vlastníka hosťujúceho boxu a LosOS. Hypervízor je KubeVirt v klastri
mesh na edge; registrar edge predáva repliky a vytvára stroje.

![Panel Mesh na boxe bez edge; Stroje sú pod Mesh.](@site/docs/img/mesh-none-sk.png)

## Kto to môže používať

Box ponúka stroje, na prenájom aj na hosťovanie, len vtedy, keď zdieľa svoj
disk so sieťou mesh. Box, ktorý nezdieľa, vidí jednu sivú vetu a odkaz na
panel Trh, kde je prepínač zdieľania disku. Zapnite tam zdieľanie, použite
zmenu a stránka Stroje ponúkne formulár.

## Prenájom

Katalóg ukazuje veľkosti, ktoré edge predáva. Vyberte jednu, povedzte, koľko
replík chcete, a zaplaťte. Edge vytvorí stroje; každá replika dostane vlastný
disk importovaný z obrazu, ktorý ponuka uvádza. Stránka ukazuje stroje, ktoré
prenajímate, ich stav a kedy obdobie končí.

## Hosťovanie

Box, ktorý zdieľa, môže aj hosťovať. Stránka ukazuje, čo hosťujete a koľko
zarábate. Hosťovanie používa ten istý účet na Trhu ako predaj úložiska alebo
výpočtového výkonu.

Stranu edge nájdete vo [Virtuálnych strojoch](../in-depth/virtual-machines):
ako registrar predáva repliky, ako sa importujú disky a z akých dôvodov
stránka povie, že stroje nie sú dostupné.
