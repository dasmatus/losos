[English](Home) · **Slovenčina** · [Deutsch](Home-de)

# losos

losos je NixOS appliance pre mini-PC. Beží na ňom Nextcloud pre vaše vlastné
súbory a voľné miesto na disku a výkon CPU môže požičiavať do mesh siete
ďalších losos zariadení. Nemá SSH ani prihlasovací shell. Spravujete ho
z webovej stránky a aktualizuje sa sám.

Koreňový súborový systém je tmpfs, ktorý sa pri každom štarte vytvorí nanovo.
Prežijú iba adresáre uvedené v `modules/impermanence.nix`, a to na šifrovanom
oddiele `/persist`.

Príručka pre vlastníka je [handbook](https://losos.dasmat.us) (`handbook/`
v repozitári). Každé zariadenie ju navyše poskytuje na
`http://<address>/handbook/`, takže sa dá čítať aj iba z LAN. Stránky nižšie
sú pre ľudí, ktorí projekt vyvíjajú a prevádzkujú.

## Stránky

- [Inštalácia](Install-sk). Inštalačné ISO, Secure Boot, TPM, demo obraz,
  zväčšenie disku.
- [Správa](Administration-sk). Admin UI, nastavenia, aktualizácie a nočný
  reštart.
- [Mesh](Mesh-sk). Poskytovanie úložiska a výpočtového výkonu iným
  zariadeniam.
- [Master proxy](Master-Proxy-sk). Prístup k zariadeniu z internetu bez
  otvorenia portu.
- [Federácia edge](Edge-Federation-sk). Vlastný edge v LAN, preposielaný cez oficiálne.
- [Hardening](Hardening-sk). Predvolený základ zabezpečenia a voliteľné
  prepínače.
- [TPM a odomykanie disku](TPM-sk). Čo čip robí a čo stráca zariadenie,
  ktoré ho nemá.
- [Bezpečnostný model](Security-Model-sk). Čo je a čo nie je chránené.
- [Architektúra](Architecture-sk). Ako do seba jednotlivé časti zapadajú.
- [Vývoj](Development-sk). Vývojový shell, testy, lock súbory.
- [CI a vydania](CI-and-Releases-sk). CI joby, inštalačné médiá vydaní,
  binárna cache.

## Porovnanie s alternatívami

- **NAS (Synology, QNAP).** Rovnaký nápad, krabička s webovým rozhraním.
  Rozdiel je v tom, že koreň sa tu pri každom štarte zahodí, takže zmeny
  útočníka vydržia len do najbližšieho reštartu, ktorý príde najneskôr
  o 00:07 každú noc. Všetko, čo na zariadení beží, si navyše môžete prečítať
  a znova zostaviť.
- **VPS.** Zariadenie je u vás doma a jeho disk je šifrovaný. Kľúč je
  zapečatený do TPM čipu zariadenia, alebo na počítači bez neho uložený na
  bootovacom oddiele. Voliteľný [master proxy](Master-Proxy-sk) mu dá
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
