---
title: Tor a I2P
sidebar_position: 4.52
slug: /manual/tor-and-i2p
---

# Tor a I2P

Zariadenie môže mať adresu v sieti Tor, meno končiace na `.onion`, a adresu
v sieti I2P, meno končiace na `.b32.i2p`. Ktokoľvek s Tor Browserom alebo s
prehliadačom nastaveným pre I2P na nej odkiaľkoľvek otvorí LosOS Login,
LosOS Cloud a LosOS Git. Zariadenie na to nepotrebuje edge ani doménu. Na
routeri sa nič neotvára, pretože Tor aj I2P sa zo zariadenia pripájajú
smerom von.

Stránky správy na týchto adresách nikdy neodpovedajú. Zostávajú len v
sieti zariadenia, rovnako ako pri každej inej adrese.

## Zapnutie

Správca zapne každú sieť v časti **Nastavenia** > **Sieť** > **Tor a I2P**
a zmeny použije. Obe sú predvolene vypnuté.

Keď sa zariadenie prestavia, Tor vytvorí adresu za pár sekúnd a I2P asi za
minútu. Dovtedy je v riadku **Vytvára sa…**. Potom sa adresy objavia na tej
istej stránke, každá s tlačidlom, ktoré ju skopíruje.

![Nastavenia, Sieť: Tor a I2P s oboma zapnutými prepínačmi, vlastné adresy zariadenia .onion a .b32.i2p a dve adresy od jeho edge, každá s tlačidlom na kopírovanie.](@site/docs/img/tor-and-i2p-sk.png)

Zariadenie si adresy necháva. Keď sieť vypnete a znova zapnete, vráti sa tá
istá adresa, a aktualizácie ani reštarty ich nemenia.

## Otvorenie zariadenia na jeho adrese

- **Tor.** Vložte adresu do Tor Browsera.
- **I2P.** Spustite na počítači router I2P, napríklad i2pd alebo router
  Java I2P, a nastavte prehliadaču jeho proxy.

Adresu začnite s `https://`. LosOS Login prihlasuje len cez https.
Certifikát pre svoje adresy si zariadenie vytvorí samo a podpíše ho vlastným
certifikátom, takže zariadenie, ktoré mu dôveruje od prvého nastavenia,
dôveruje aj týmto adresám. Iné zariadenia raz ukážu varovanie, ako pri
väčšine stránok .onion. Funguje aj `http://`, ale prihlásiť sa cez neho
nedá.

Na týchto adresách zariadenie odpovedá na to isté, na čo odpovedá jeho
verejné meno cez edge: LosOS Login na `/auth/`, LosOS Cloud na `/cloud/` a
LosOS Git na `/git/`. Stránky správy, `/api/` a prvé nastavenie odpovedia
`403`, nech prehliadač pošle akékoľvek meno.

## Adresy od vášho edge

Edge môže dať každému zariadeniu, ktorému slúži, vlastnú adresu v sieti Tor
aj I2P. Tieto adresy vznikajú a zostávajú na edge, takže samotné zariadenie
pre ne Tor ani I2P nespúšťa. K zariadeniu vedú cez jeho tunel, tak ako jeho
verejné meno, a zariadenie ich ukáže v časti **Nastavenia** > **Sieť** ako
**Tor cez váš edge** a **I2P cez váš edge**. Zapína ich prevádzkovateľ edge
pre celý edge. Na zariadení to nastaviť nejde.

Edge na týchto adresách odpovedá cez https s certifikátom, ktorý si vytvorí
sám, takže prehliadač pri prvom otvorení ukáže varovanie.

Rozdiel je v tom, kto prenáša prevádzku. Vlastné adresy zariadenia vedú z
Toru alebo I2P priamo do zariadenia. Adresy od edge idú cez edge, ktorý
požiadavky vidí tak ako pri verejnom mene zariadenia.

## Čo na zariadení beží

So zapnutým Torom zariadenie spúšťa Tor, ktorý zverejňuje jeho službu
.onion. So zapnutým I2P spúšťa i2pd s jedným tunelom pre svoju adresu. Ani
jeden neponúka proxy programom na zariadení a ani jeden nepočúva v sieti
zariadenia. i2pd okrem toho prenáša trochu prevádzky pre iných používateľov
I2P, najviac 32 KB/s, ako každý router I2P.
