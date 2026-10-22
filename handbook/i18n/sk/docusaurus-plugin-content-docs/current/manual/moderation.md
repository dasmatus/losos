---
title: Roly a moderovanie
sidebar_position: 2.5
slug: /manual/moderation
---

# Roly a moderovanie

Každý, kto sa prihlasuje na zariadenie, má rolu. Rola určuje, čo môže meniť
a či môže zasiahnuť, keď niekto nahlási správu, obrázok alebo súbor. Roly a
nahlásenia fungujú iba na zariadení, ktoré prihlasuje ľudí samo cez LosOS
Login.

## Roly \{#the-roles}

| Rola | Čo môže |
|---|---|
| **Vlastník** | Človek, ktorý zariadenie nastavil. Správca, ktorého nikto iný nemôže odstrániť, zosadiť ani zadržať. |
| **Správca** | Všetko vrátane toho, kto sa môže prihlásiť, ich rolí a panela Moderovanie. |
| **Moderátor** | Vybavuje nahlásenia a stíši alebo pozastaví členov a hostí. Mení časti, ktoré dostal, ako člen. |
| **Člen** | Vidí a mení iba zapnuté časti, napríklad Úložisko a Záloha. |
| **Hosť** | Vlastný účet, pošta a profil. Nemení žiadnu časť zariadenia a nič na ňom nespúšťa. |

Člen alebo moderátor môže dostať aj **Vidí všetky časti**: pozrie si každý
panel a mení stále iba svoje časti. Kto iba číta, panel Moderovanie nevidí.

Pri obrázku človeka je malá značka jeho roly a pri mene štítok: koruna pre
vlastníka, štít pre správcu a vlajka pre moderátora. Členovia značku nemajú.

Rolu vyberá správca v **Nastavenia > Používatelia**, pri pridávaní človeka
alebo tlačidlom **Prístup** v jeho riadku. Tabuľka **Čo môže každá rola** na
tom istom paneli ukazuje, kam ktorá z piatich rolí v jednotlivých
aplikáciách siaha.

![Tabuľka na paneli Používatelia: čo môže každá rola.](@site/docs/img/settings-roles-sk.png)

### Vlastné roly \{#roles-of-your-own}

Vo **Vlastné roly** správca pomenuje skupinu častí, napríklad „Pomocníci so
zálohou“ s Úložiskom a Zálohou, a dá ju viacerým ľuďom naraz. Keď zmeníte
časti roly, každý, kto ju má, ich dostane pri ďalšom kliknutí. Keď rolu
odstránite, ľuďom, ktorí ju mali, zostane iba vlastný účet, kým im správca
nedá inú. Zariadenie si ponechá najviac 16 vlastných rolí.

## Nahlásenie \{#reporting-something}

Každý prihlásený, aj hosť, môže v aplikácii nahlásiť správu, obrázok, profil
alebo súbor. Nahlásenie pomenuje vec a koho sa týka. Nikdy neskopíruje správu
ani obrázok, takže nič na paneli Moderovanie ich neotvorí. Každý môže mať
naraz otvorených 20 nahlásení.

Vyberte dôvod, ktorý sedí: spam, urážky alebo obťažovanie, sexuálny obsah,
ubližovanie dieťaťu alebo niečo iné. Krátka poznámka moderátorovi pomôže,
okrem nahlásenia o dieťati, ktoré si poznámku nenechá vôbec.

## Panel Moderovanie \{#the-moderation-pane}

Moderátori a správcovia otvárajú **Nastavenia > Moderovanie**. Má tri časti.

![Panel Moderovanie: najprv otvorené nahlásenia, potom ľudia, ktorých môže moderátor zadržať, a nakoniec záznam.](@site/docs/img/settings-moderation-sk.png)

**Nahlásenia.** Otvorené nahlásenia sú prvé, nahlásenia o dieťati pred
ostatnými, najnovšie navrchu. Každé nahlásenie má tlačidlá podľa toho, čo sa
v jeho aplikácii dá urobiť:

- **Zadržať správu** presunie nahlásenú poštu z doručenej do priečinka,
  ktorý žiadna stránka neukazuje. Zariadenie ju odmietne otvoriť.
  **Uvoľniť** ju vráti.
- **Odstrániť ho** stiahne nahlásenú profilovú fotku alebo banner.
- **Vybavené** uzavrie nahlásenie, ktoré ste vyriešili inak, napríklad
  rozhovorom s daným človekom.
- **Bez problému** uzavrie nahlásenie, ktoré nič nepotrebuje.

**Ľudia.** Moderátori môžu zasiahnuť pri členoch a hosťoch. Správcovia pri
každom okrem vlastníka. Nikto nemôže zasiahnuť sám pri sebe.

- **Stíšiť** mu zastaví posielanie pošty a zmeny profilu. Naďalej číta
  všetko, čo čítal predtým.
- **Pozastaviť** zastaví LosOS Login, aby ho pustil dnu, od jeho ďalšieho
  kliknutia. Jeho súbory, pošta a účty na zariadení zostanú.

Oboje sa pýta, na ako dlho: hodinu, deň, týždeň, 30 dní alebo kým sa
nezruší. Časové obmedzenie skončí samo. Dôvod, ktorý napíšete, ide do
záznamu pre ostatných moderátorov. **Zrušiť stíšenie** a **Zrušiť
pozastavenie** ho ukončia skôr.

**Čo urobili moderátori.** Každé nahlásenie, zmena roly, stíšenie,
pozastavenie a odstránenie sa zapíše do záznamu, aj s tým, kto to urobil a
kedy. Keď obmedzenie skončí samo, riadok zapíše zariadenie. Záznam sa zo
správcovských stránok nedá upraviť.

## Nahlásenie o dieťati \{#a-report-about-a-child}

Neotvárajte to, neukladajte a nikomu to neposielajte, ani polícii. Pozrieť
si to alebo to poslať ďalej môže byť trestný čin, aj pre moderátora.

1. Pri nahlásení stlačte **Skryť bez otvorenia**. Kde to aplikácia z
   frontu skryť nevie, tlačidlo je **Nahlásené polícii**.
2. Povedzte polícii alebo linke, ktorá takéto hlásenia prijíma, kde to je:
   zariadenie, účet a čas nahlásenia. Vec si vyzdvihnú sami.
   - V núdzi volajte 112.
   - [INHOPE](https://www.inhope.org/EN) uvádza linku pre každú krajinu.
   - Slovensko: [Stopline.sk](https://www.stopline.sk/).
   - Nemecko: [internet-beschwerdestelle.de](https://www.internet-beschwerdestelle.de/).
   - Spojené štáty: [NCMEC CyberTipline](https://report.cybertip.org/).
   - Spojené kráľovstvo: [Internet Watch Foundation](https://report.iwf.org.uk/).

Nahlásenie o dieťati sa nedá uzavrieť ako **Bez problému** a zadržanú poštu
z neho nemožno z panela uvoľniť.
