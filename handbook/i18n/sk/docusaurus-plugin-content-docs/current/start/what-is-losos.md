---
title: Čo je LosOS
sidebar_position: 1
---

# Čo je LosOS

LosOS urobí z malého počítača, zvyčajne z vysluženého mini-PC, appliance.
Zapojíte ho do siete a do elektriny, raz ho nainštalujete z USB kľúča a potom
ho už používate iba cez webový prehliadač.

## Čo na ňom beží \{#what-runs-on-it}

| Čo vidíte             | Čo to je                                                                                        |
| --------------------- | ----------------------------------------------------------------------------------------------- |
| **LosOS Cloud**       | Vaše súbory, fotky, kalendár, kontakty, poznámky a úlohy. Postavené na Nextcloude. Na `/nextcloud`. |
| **LosOS Git**         | Vaše repozitáre kódu, predvolene zapnuté. Postavené na Forgejo. Na `/forgejo/`.                 |
| **Admin stránky**     | Tu zariadenie nastavujete a staráte sa oň. Na `/`, dostupné iba z vašej lokálnej siete.         |
| **Mesh**              | Voliteľné. Voľný disk a CPU požičané iným zariadeniam LosOS a požičané od nich.                  |
| **Vzdialený prístup** | Voliteľný. Verejná adresa zariadenia cez edge server, bez otvoreného portu doma.                  |

## Čím sa líši od NAS \{#what-makes-it-different-from-a-nas}

![Vnútri zariadenia: nginx sú jediné vstupné dvere; za nimi riadiaci démon, vlastný cluster zariadenia s LosOS Cloud a LosOS Git a agent mesh siete; pod nimi systémový disk v RAM a jeden šifrovaný zväzok.](@site/docs/img/box-architecture-sk.svg)

- **Zabudne všetko, čo nepotrebuje.** Zariadenie pri každom štarte zostaví
  svoj systémový disk úplne nanovo. Prežijú iba vaše dáta, nastavenia, ktoré
  ste si zvolili, a krátky zoznam systémových adresárov. Tie ležia na
  šifrovanom zväzku. Čokoľvek, čo útočník, chyba alebo omyl zmenili
  kdekoľvek inde, je do rána preč.
- **Každú noc sa samo reštartuje** o 00:07 a o 03:00 sa aktualizuje. Týmto
  reštartom sa opravuje. Aj preto nemá shell. Nič na zariadení ho
  nepotrebuje.
- **Šifruje svoj disk a kľúč drží vo vlastnom čipe.** Na počítači s TPM
  disk neprečíta nikto, kto ho vyberie a zapojí do iného počítača.
- **Vlastné prihlásenie.** Každý sa do admin stránok prihlasuje vlastným
  menom a heslom a heslo vlastníka otvára aj LosOS Cloud. Vytlačený náhradný
  kľúč vás pustí dnu v deň, keď LosOS Login, prihlasovacia stránka
  zariadenia, nebeží.
- **Môže požičať, čo práve nepoužíva.** Zariadenie môže zdieľať voľný disk
  a CPU s inými zariadeniami, iba v hodinách, ktoré si zvolíte, a iba keď je
  nečinné. [Trh](../types/official-edge#what-the-edge-does-for-a-box), ktorý
  za to vlastníkom platí, je hotový. Otvorí sa, keď bude existovať firma,
  ktorá zaň zodpovedá.

## Čo to nie je \{#what-it-is-not}

- Nie je to univerzálny server. Bez shellu nikto nedokáže zariadenie
  nastaviť do stavu, ktorý nikto nezapísal.
- Nie je to záloha. Zariadenie je jedna kópia. Všetko, o čo nesmiete prísť,
  majte ešte v ďalšej kópii. Jednou z možností sú desktopoví a mobilní
  klienti LosOS Cloud.
- Nie je to hotové. Trh, jeden spoločný priestor úložiska naprieč
  zariadeniami a podpísaný inštalátor ešte len prídu.
