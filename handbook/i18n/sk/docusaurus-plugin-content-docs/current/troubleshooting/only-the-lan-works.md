---
title: Funguje iba lokálna sieť
sidebar_position: 1
slug: /troubleshooting/only-the-lan-works
---

# Funguje iba lokálna sieť

<div className="losos-symptom">

**Čo vidíte:** vlastné stránky zariadenia sa otvárajú na jeho adrese
`http://192.168.…` alebo na mene `.local` a všetko v nich funguje.
Vzdialený prístup zvonku nefunguje, panel Mesh nenachádza žiadny edge,
alebo sa z internetu nenačíta nič na žiadnom počítači doma.

</div>

Zariadenie je postavené presne na tento prípad. **Všetko, čo žije na
zariadení, v lokálnej sieti funguje ďalej**, nech svet zvonku odrezalo
čokoľvek. Nič sa nestratí ani nepozastaví a keď sa svet zvonku vráti, nič
znova nenastavujete.

## Čo to môže spôsobiť \{#what-can-cause-it}

Ktorákoľvek z týchto príčin nechá zariadenie dostupné z LAN a odinakiaľ
nie. Kroky nižšie sú pre všetky rovnaké. Príčina rozhoduje iba o tom, kto to
opraví.

| Príčina                                 | Ako ju spoznáte                                                                                         |
| --------------------------------------- | ------------------------------------------------------------------------------------------------------- |
| Linka poskytovateľa nefunguje           | Žiadna webová stránka sa neotvorí na žiadnom zariadení doma a ukazuje to aj kontrolka stavu na routeri. Môžete len čakať. |
| Router nemá pripojenie do internetu     | Router stratil spojenie pre odpojený kábel, prebiehajúci reštart alebo zmenené nastavenie. LAN funguje ďalej, pretože funguje switch. |
| Edge nefunguje alebo je nedostupný      | Na vašom notebooku internet funguje, ale panel Mesh nenachádza žiadny edge a verejné meno neodpovedá. Pozrite [Nenašiel sa žiadny edge](/troubleshooting/edge-not-found.md). |
| DNS poskytovateľa je pokazené           | Stránky podľa mena zlyhávajú, zariadenie cez IP funguje. Prepnite router alebo počítač na iný resolver.   |
| Zariadenie nikdy nebolo pripojené k edge | Nie je to výpadok. [Samostatné zariadenie](../types/single-box) zámerne nemá vzdialený prístup ani mesh. |

## Čo funguje ďalej \{#what-still-works}

![Zariadenie, ktorému nefunguje internet, ale jeho firemný edge v LAN stále odpovedá: panel Mesh ho uvádza ako V tejto sieti.](@site/docs/img/mesh-company-sk.png)

| Čo                                  | Prečo to nepotrebuje internet                                                                                    |
| ----------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| Prihlásenie na správcovské stránky  | Zariadenie overí heslo voči LosOS cloud priamo na sebe, cez vlastný loopback. Funguje aj vytlačený náhradný kľúč. |
| LosOS cloud a LosOS Git             | Oba bežia na zariadení, takže súbory, kalendáre, kontakty, repozitáre aj mobilné aplikácie na tej istej Wi-Fi zostávajú lokálne. |
| Vaše vlastné súbory                 | Sú na šifrovanom disku zariadenia, v každom režime úložiska.                                                     |
| Lokálne zdieľanie medzi ľuďmi doma  | O zdieľaniach, odkazoch a oprávneniach rozhoduje zariadenie vnútri LosOS cloud.                                  |
| Celé admin UI                       | Použiť, Nastavenia, panel Vzhľad, panel Úložisko aj reštarty komunikujú so zariadením cez LAN.                  |
| Vstavané a ručne písané widgety     | Vykresľujú sa na zariadení, z jeho vlastných hodnôt. Ručne písaný widget, ktorý sa *rozhodne* niečo stiahnuť z internetu, nechá tú časť prázdnu, kým sa svet zvonku nevráti, a zvyšok vykreslí. |
| Táto príručka                       | Zariadenie poskytuje vlastnú kópiu na `/handbook/` aj s vyhľadávaním. Ak ste túto stránku otvorili z Nastavenia → O zariadení, čítate práve túto kópiu. |
| Nočný reštart a aktualizácia o 03:00 s predvoleným zdrojom | Predvolený zdroj je vlastná kópia popisu zariadenia na jeho disku. Prestavba sa spustí aj dokončí offline. |
| Firemný edge v tej istej LAN        | [Firemné nasadenie](../types/company-edge) má svoj edge, mesh aj zdieľanie v lokálnej sieti. Poskytovateľ internetu v tej ceste nie je. |

## Čo počká na svet zvonku \{#what-waits-for-the-outside}

| Čo                                          | Čo medzitým vidíte                                                                                                  |
| ------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| Vzdialený prístup cez edge LosOS            | Verejná adresa vášho zariadenia zvonku neodpovedá. Tunel sa pripojí znova sám, keď sa svet zvonku vráti.            |
| Samotný [edge LosOS](../types/official-edge): mesh, zdieľanie výpočtového výkonu, trh | Panel Mesh hlási, že sa nenašiel žiadny edge. Prepínač, ktorý edge potrebuje, zariadenie odmietne s hláškou, namiesto toho, aby ho nechalo zapnutý napoly. |
| Miesto **zdieľané s mesh**                  | Spoločný priestor je rozložený na zariadeniach edge, takže čokoľvek v ňom je nedostupné, kým je nedostupný edge. Vaše vlastné súbory v ňom nikdy nie sú. Zostávajú na zariadení, nedotknuté. |
| **Nájsť ďalšie** na stránke Aplikácie       | Zariadenie sťahuje katalóg z internetu, takže vyhľadávanie neukáže nič, kým sa internet nevráti. Už nainštalované aplikácie bežia ďalej. |
| Aktualizácia o 03:00 so zdrojom `github:`   | Nemôže nič stiahnuť a widget **Zmeny** zapíše nočnú prestavbu ako zlyhanú. Zariadenie beží ďalej na tom, čo má, a skúsi to znova nasledujúcu noc. |
| Binárna cache                               | Prestavba, ktorá potrebuje nový softvér, si ho namiesto toho zostaví na zariadení, pomaly, alebo v tú noc zlyhá. Najbližšia úspešná noc to dobehne. |
| Pustenie návštevníka cez verejný odkaz      | Verejné odkazy na zdieľanie vedú cez verejnú adresu zariadenia; znova fungujú, keď sa vráti svet zvonku.            |

## Ako rozlíšiť výpadok od chyby zariadenia \{#how-to-tell-an-outage-from-a-box-fault}

Zariadenie nemá kontrolku „internet funguje“. Má to, čo je vymenované
vyššie.

1. Otvorte adresu zariadenia. Ak Prehľad hlási **Odpovedá** a dlaždice
   aplikácií sa otvárajú, zariadenie aj LAN sú v poriadku. Ak sa neotvorí
   ani to, problém nie je u poskytovateľa internetu. Pozrite
   [Zariadenie je nedostupné](/troubleshooting/cannot-reach-the-box.md).
2. Otvorte panel **Mesh**. Zariadenie pripojené k edge LosOS hlási, že sa
   nenašiel žiadny edge, kým je svet zvonku nedostupný. Zariadenie
   s firemným edge v LAN ho stále ukazuje ako nájdený. Zariadenie, ktoré
   nikdy nebolo pripojené, neukazuje nič v žiadnom prípade, čo je pri
   [samostatnom zariadení](../types/single-box) normálne. Ak sa internet
   k edge dostane, ale zariadenie nie, problém je v edge, nie v linke.
   Pozrite [Nenašiel sa žiadny edge](/troubleshooting/edge-not-found.md).
3. Po 03:00 sa pozrite na widget **Zmeny** na Prehľade. Na zariadení so
   zdrojom aktualizácií `github:` je prestavba, ktorá zlyhala počas
   výpadku, dôsledkom výpadku, nie chybou zariadenia. Ukáže to záznam
   z nasledujúcej noci.
4. Otvorte ľubovoľnú webovú stránku z telefónu na tej istej Wi-Fi. Ak zlyhá
   aj to, problém je v linke alebo v routeri a zariadenie čaká spolu
   s vami. Ak to funguje, chýba pripojenie samotného zariadenia alebo edge.

## Keď sa svet zvonku vráti \{#when-the-outside-comes-back}

Netreba nič robiť. Tunel k edge sa znova pripojí, spoločný priestor sa
vráti, najbližší beh o 03:00 stiahne to, čo predchádzajúci nemohol, a panel
Mesh sa vráti do obvyklého stavu. Nič znova nezadávate a zariadenie
nereštartujete. Ak panel Mesh hodinu po návrate internetu stále neukazuje
žiadny edge, pozrite [Nenašiel sa žiadny edge](/troubleshooting/edge-not-found.md).

## Čomu sa počas výpadku vyhnúť \{#what-to-avoid-during-the-outage}

- **Nepreinštalujte ani neresetujte do továrenského stavu** preto, že
  prestal fungovať vzdialený prístup. Ani jedno svet zvonku nevráti a obe
  zahodia lokálny stav.
- **Nevypínajte a nezapínajte zdieľanie**, aby ste mesh prebudili.
  Zariadenie ho odmietne zapnúť, kým neodpovedá žiadny edge, a spoločný
  priestor sa vráti sám.
- **Nemeňte zdroj aktualizácií**, aby ste obišli zlyhanú noc. Predvolený
  zdroj funguje offline a zdroj `github:` počká.
