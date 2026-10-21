---
title: Slovník pojmov
sidebar_position: 5
---

# Slovník pojmov

![Kam ktoré slovo patrí: zariadenia, edge, oficiálny edge LosOS, firemný edge a spoločný priestor mesh siete.](@site/docs/img/setup-types-sk.svg)

**Admin stránky.** Vlastná webová stránka zariadenia na `/`, dostupná iba
z LAN, kde zariadenie nastavujete a staráte sa oň. Je to jednostránková
aplikácia, ktorú zariadenie poskytuje samo.

**Použiť.** Stlačenie tlačidla, ktoré z upravených nastavení urobí
prestavbu zariadenia.

**Nástenka.** Mriežka widgetov na stránke Prehľad, uložená vo vašom
prehliadači.

**Zariadenie.** Počítač, na ktorom beží LosOS. Admin stránky hovoria
„toto zariadenie“.

**Edge.** Server na internete alebo vo firemnej sieti, ku ktorému majú
zariadenia otvorený tunel. Dáva im verejnú adresu, mesh a trh. **Edge LosOS**
patrí projektu. **Firemný edge** patrí komukoľvek inému.

**Find my box.** Stránka na edge LosOS, ktorá z prehliadača Chrome nájde
zariadenie vo vašej LAN.

**Režim s keyfile.** Režim odomykania disku pre počítače bez TPM. Kľúč leží
na bootovacom oddiele.

**LosOS cloud.** Nextcloud vo farbách zariadenia: súbory, fotky, kalendár,
kontakty, poznámky, úlohy.

**LosOS Git.** Vlastný Git server zariadenia: repozitáre.

**Trh.** Platba za zdieľaný disk a CPU a ich nákup, v USDC, Bitcoine alebo
Monere, alebo kartou tam, kde edge karty prijíma. Funguje iba na edge LosOS.

**Mesh.** Cluster zariadení za jedným edge. Spájajú voľný disk, ktorý mesh
replikuje, a voľné CPU, ktoré každé zariadenie požičiava vo svojich
hodinách, keď je nečinné.

**Overrides.** Jediný súbor s nastaveniami, ktoré ste si zvolili,
`modules/overrides.nix`. Zariadenie je zdrojový kód LosOS plus tento súbor.

**Prestavba.** Zariadenie zostaví novú verziu celého svojho systému zo
svojho opisu a prepne sa na ňu. Zmeny nastavení aj aktualizácia o 03:00
sú prestavby.

**Rezerva.** 10 % disku, ktoré inštalátor nechal nevyužité. Môžete si ich
zabrať z panela Úložisko.

**Náhradný správcovský kľúč.** Kľúč so 64 znakmi, ktorý sprievodca ukáže
raz, na odomknutie admin stránok, keď LosOS cloud nemôže overiť heslo.

**TPM.** Bezpečnostný čip vo firmvéri počítača, ktorý v predvolenom režime
odomykania drží kľúč disku.

**Závoj.** Posuvník na paneli Vzhľad, ktorý kladie farbu stránky na
obrázok pozadia.

**Widget.** Dlaždica na nástenke. Vstavaný, zostavený z údajov zariadenia
alebo napísaný ručne.

**Sprievodca.** Tri kroky, ktoré admin stránky ukážu na novom zariadení:
dôverovať certifikátu, nastaviť heslo, prihlásiť sa.

**Okno.** Denné hodiny, medzi ktorými zariadenie požičiava voľné CPU.
