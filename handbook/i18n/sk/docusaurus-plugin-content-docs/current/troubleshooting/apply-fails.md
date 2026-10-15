---
title: Použitie zmien zlyhá
sidebar_position: 9
slug: /troubleshooting/apply-fails
---

# Použitie zmien zlyhá

<div className="losos-symptom">

**Čo vidíte:** po stlačení **Použiť** oznámenie hlási „Nepodarilo sa to
spustiť“ alebo „Zmeny sa nepodarilo použiť“, prípadne widget **Zmeny** na
Prehľade uvádza úlohu ako zlyhanú. Zariadenie beží ďalej so starými
nastaveniami.

</div>

![Prehľad po zlyhanom použití zmien: dlaždica Nastavenia má červenú bodku a nápis Zmena zlyhala.](@site/docs/img/overview-apply-failed.png)

## Čo znamená zlyhané použitie zmien \{#what-a-failed-apply-means}

Nič sa nezmenilo. Prestavba sa buď dokončí a zariadenie sa na ňu prepne,
alebo zlyhá a ďalej beží predchádzajúci systém. Prestavba sa nikdy nedotkne
vašich súborov.

## Príčiny, od najpravdepodobnejšej \{#causes-most-likely-first}

1. **Zariadenie spred 7. októbra 2026.** Každé **Použiť** na nainštalovanom
   zariadení odpovedalo „Nepodarilo sa to spustiť“, pretože príkaz na
   prestavbu nebol v ceste (PATH) démona. Opravil to pull request #71 a
   oprava príde s aktualizáciou o 03:00. Nočná aktualizácia používa inú
   cestu a touto chybou nikdy netrpela. Kým oprava nepríde, nastavenia na
   paneloch meniť nemôžete.
2. **Zariadenie sa už prestavuje.** Oznámenie to povie. Počkajte, kým sa
   bežiaca úloha dokončí (ukáže to widget Zmeny), a použite zmeny znova.
3. **Disk je plný.** Prestavba potrebuje miesto. Pozrite [Nie je miesto](out-of-room).
4. **Nastavenie, ktoré zariadenie nevie prijať.** Meno so znakom, ktorý
   kontroly nezachytili, alebo obsadený port. Dajte **Zahodiť**, meňte jednu
   vec naraz, použite zmeny znova a sledujte, ktorá zlyhá.
5. **Zdroj aktualizácií je nedostupný.** Ak zdroj aktualizácií zariadenia
   ukazuje na GitHub a internet nefunguje, prestavba, ktorá potrebuje niečo
   stiahnuť, zlyhá. Počkajte alebo zmeny použite, keď sa internet vráti.
   Vlastný popis zariadenia internet nikdy nepotrebuje.

## Čo robiť, keď to stále zlyháva \{#what-to-do-when-it-keeps-failing}

- Počkajte na najbližšiu 03:00. Nočná prestavba spustí tú istú úlohu s tým
  istým súborom nastavení. Ak zlyhá aj ona, nechá zariadenie tak, ako je.
- **Nastavenia → Reset** vráti všetky nastavenia späť a zariadenie prestavia
  z predvolených hodnôt. Predvolené hodnoty sú jediný popis, o ktorom vieme,
  že funguje na každom zariadení. Vaše súbory zostanú.
- [Požiadajte o pomoc](getting-help) a priložte text z widgetu Zmeny pri
  zlyhanej úlohe.
