---
title: Do hĺbky
sidebar_position: 0
sidebar_label: Prehľad
slug: /in-depth
mdx:
  format: md
---

# Do hĺbky

Zvyšok tejto príručky je pre vlastníka zariadenia. Táto kapitola je pre ľudí,
ktorí LosOS vyvíjajú, prevádzkujú edge alebo pripravujú vydanie: ako do seba
časti zapadajú, prečo je každá tam, kde je, a ako ich meniť.

LosOS je NixOS appliance pre mini-PC. Beží na ňom Nextcloud pre vaše vlastné
súbory a voľné miesto na disku a výkon CPU môže požičiavať do mesh siete
ďalších LosOS zariadení. Nemá SSH ani prihlasovací shell. Spravujete ho
z webovej stránky a aktualizuje sa sám.

Koreňový súborový systém je tmpfs, ktorý sa pri každom štarte vytvorí nanovo.
Prežijú iba adresáre uvedené v `modules/impermanence.nix`, a to na šifrovanom
oddiele `/persist`.

## Stránky

- [Inštalácia podrobne](/in-depth/install.md). Inštalačné ISO, Secure Boot, TPM, demo obraz,
  zväčšenie disku.
- [Správa](/in-depth/administration.md). Admin UI, nastavenia, aktualizácie a nočný
  reštart.
- [Mesh](/in-depth/mesh.md). Poskytovanie úložiska a výpočtového výkonu iným
  zariadeniam.
- [Trh](/in-depth/market.md). Predaj disku a CPU, ktoré zariadenie už poskytuje
  (plánovaný).
- [Virtuálne stroje](/in-depth/virtual-machines.md). Prenájom strojov v mesh
  a hosťovanie strojov iných zariadení.
- [Master proxy](/in-depth/master-proxy.md). Prístup k zariadeniu z internetu bez
  otvorenia portu.
- [Federácia edge](/in-depth/edge-federation.md). Vlastný edge v LAN, preposielaný cez oficiálne.
- [Lab](/in-depth/lab.md). Vizualizér nasadenia na `/lab/`, jeho simulovaní a
  skutoční hostia.
- [Hardening](/in-depth/hardening.md). Predvolený základ zabezpečenia a voliteľné
  prepínače.
- [TPM a kľúč disku](/reference/tpm.md). Čo čip robí a čo stráca zariadenie,
  ktoré ho nemá.
- [Bezpečnostný model](/reference/security-model.md). Čo je a čo nie je chránené.
- [Architektúra](/in-depth/architecture.md). Ako do seba jednotlivé časti zapadajú.

## Porovnanie s alternatívami

- **NAS (Synology, QNAP).** Rovnaký nápad, krabička s webovým rozhraním.
  Rozdiel je v tom, že koreň sa tu pri každom štarte zahodí, takže zmeny
  útočníka vydržia len do najbližšieho reštartu, ktorý príde najneskôr
  o 00:07 každú noc. Všetko, čo na zariadení beží, si navyše môžete prečítať
  a znova zostaviť.
- **VPS.** Zariadenie je u vás doma a jeho disk je šifrovaný. Kľúč je
  zapečatený do TPM čipu zariadenia, alebo na počítači bez neho uložený na
  bootovacom oddiele. Voliteľný [master proxy](/in-depth/master-proxy.md) mu dá
  verejnú adresu bez prichádzajúceho portu.
- **Čistý NixOS.** Toto všetko by ste si mohli napísať sami. Tu je to už
  napísané a izolácia používateľov, šifrovaná perzistencia, zväčšovanie
  disku, hardening a obmedzenia mesh siete majú každé svoj VM test.
- **Iné self-hosting riešenia.** Žiadne z nich nepožičiava kapacitu do mesh
  siete iba vtedy, keď je otvorené časové okno vlastníka *a zároveň* je
  zariadenie nečinné.

## Čo to nie je

- Nie je to univerzálny server. Shell zámerne chýba.
- Nie je to záloha. Kópie `/persist` uchovávajte inde.
- Nie je to hotové.
