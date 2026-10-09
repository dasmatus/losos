---
title: Tvorca widgetov
sidebar_position: 4.7
mdx:
  format: md
---

# Tvorca widgetov

Opíšete widget bežnými slovami a AI agent ho napíše. Editor widgetov
(**Pridať widget**, potom **Napísať**) má vedľa karty **Písať** kartu
**Vytvoriť s AI**. Súbory widgetu sa objavia v editore, kde si ich môžete
prečítať, vyskúšať v náhľade a upraviť. Nič sa neuloží, kým nestlačíte
Uložiť, a widget od AI beží v rovnakom sandboxovanom rámci ako ručne písaný
(pozri [Správa](administration.md#ručne-písané-widgety)).

Karta funguje, keď edge zariadenia tvorcu ponúka. Inak to oznámi.

## Koľko to stojí

Karta ukazuje cenu za milión tokenov, ktoré AI prečíta a napíše, a najvyššiu
cenu jednej tvorby. Tvorba sa platí z predplateného zostatku. Dobijete ho
jedným z balíčkov, ktoré karta ponúka, na platobnej stránke Stripe, a
zostatok sa zmení, keď Stripe platbu potvrdí.

Tvorba sa účtuje podľa toho, čo spotrebovala, aj keď AI skončí bez
použiteľného widgetu. Tvorba, ktorú sa nepodarí dokončiť, sa neúčtuje. Keď
tvorba dosiahne limit výdavkov, karta upozorní, že widget môže byť
nedokončený.

## Podmienky spoločnosti Anthropic

Opis a súbory poslané na úpravu idú spoločnosti Anthropic a vzťahujú sa na ne
jej [Zásady používania](https://www.anthropic.com/legal/aup) a
[Obchodné podmienky](https://www.anthropic.com/legal/commercial-terms). Karta to uvádza pod tlačidlom Vytvoriť.

## Dobré vedieť

- Tvorba trvá minútu či dve a pokračuje aj po zatvorení okna.
- Zariadenie spúšťa jednu tvorbu naraz.
- Ak chcete widget upraviť, nechajte ho v editore a zaškrtnite políčko, ktoré
  žiada úpravu. AI dostane všetky súbory widgetu a vráti tie, ktoré ponechá.
- Widget od AI môže čítať hodnoty zariadenia ako každý widget. Zariadenie
  meniť nemôže.
