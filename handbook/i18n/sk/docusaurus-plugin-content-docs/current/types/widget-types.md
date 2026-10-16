---
title: Typy widgetov
sidebar_position: 7
---

# Typy widgetov

Stránka Prehľad má **nástenku** widgetov, malých dlaždíc, z ktorých každá
ukazuje jednu vec o zariadení. **Pridať widget** otvorí galériu, ktorá má tri
druhy.

| Druh                     | Kde je uložený     | Môže spúšťať kód?                | Ako získa údaje zariadenia                           |
| ------------------------ | ------------------ | -------------------------------- | ---------------------------------------------------- |
| **Vstavaný**             | v systéme          | áno, skompilovaný do admin stránky | priamo                                             |
| **Zostavený z údajov**   | v tomto prehliadači | nie                             | jeden údaj, niekoľko výrazov v malom jazyku          |
| **Napísaný ručne**       | na zariadení       | áno, v sandboxe                  | cez objekt `losos`, pomocou správ                    |

## Vstavané \{#built-in}

So zariadením prichádza päť widgetov. **Odpovedá** vytieňuje dni roka podľa
toho, akú časť každého dňa zariadenie odpovedalo. **Miesto** ukazuje použitý
disk a rezervu. **Zdieľanie** porovnáva požičané hodiny s ponechanými.
**Aplikácie** ukazuje, ktoré aplikácie práve odpovedali. **Zmeny** vypisuje
použité zmeny nastavení, od najnovšej. Každý sa vykresľuje v jednom zo
štyroch tvarov: teplotná mapa, číslo, pruh alebo zoznam. Vstavané widgety sú
súčasťou vlastného kódu admin stránky a prichádzajú so systémovými
aktualizáciami.

![Galéria widgetov: vstavané widgety, Zostaviť a Napísať.](@site/docs/img/widget-gallery-sk.png)

## Zostavené z údajov \{#built-from-readings}

**Zostaviť** vytvorí widget bez kódu. Vyberiete údaj, ktorý zariadenie
zverejňuje, napríklad miesto na disku, okno mesh alebo nastavenia, a tvar.
Potom napíšete, čo sa má zobraziť, krátkymi výrazmi ako `m.free / m.total`.
**Zostaviť** widget pred uložením skontroluje a ukáže jeho náhľad. Widget
je uložený v prehliadači, v ktorom ste ho vytvorili. Na zariadení nič
nemení a nedostane sa do siete, na stránku ani k ničomu inému než k svojmu
jedinému údaju. To ho zároveň obmedzuje: nedokáže sa vetviť podľa viacerých
údajov, opakovať v cykle ani si medzi spusteniami nič pamätať.

![Zostaviť: údaj, tvar, výraz na zobrazenie a náhľad dlaždice.](@site/docs/img/widget-build-one-sk.png)

## Napísané ručne \{#written-by-hand}

**Napísať** prijme vlastné HTML, štýly a skript v niekoľkých súboroch:
`index.html` a štýly, skripty, obrázky a dáta, na ktoré odkazuje menom.
Zariadenie widget uloží, takže ho vidí každý prehliadač, ktorý zariadenie
otvorí. Je uvedený v **Nastavenia → Vzhľad**, kde ho môžete upraviť
a odstrániť.

![Napísať: zdrojový kód ručne napísaného widgetu vedľa jeho živého náhľadu v sandboxovanom rámci.](@site/docs/img/widget-write-one-sk.png)

Ručne napísaný widget beží v sandboxovanom rámci. Rámec má vlastný origin,
bez prístupu k admin stránkam, admin kľúču a API zariadenia. Widget sa so
zariadením dorozumieva cez malý objekt `losos`, ktorý mu rámec poskytuje:

| Volanie                     | Čo poskytuje                                                                  |
| --------------------------- | ----------------------------------------------------------------------------- |
| `losos.metric(name)`        | promise s jedným z údajov zariadenia, s rovnakými názvami, aké používajú vstavané widgety |
| `losos.theme`, `losos.lang` | `light`/`dark` a jazyk, v ktorom je admin stránka                             |
| `losos.palette`             | farby zariadenia, nastavené aj ako premenné CSS (`var(--accent)` funguje)     |
| `losos.onTheme(fn)`         | zavolá sa vždy, keď vlastník prepne tému                                      |
| `losos.resize()`            | požiada nástenku, aby dlaždicu znovu premerala                                |
| `losos.files`               | názvy súborov widgetu                                                         |
| `losos.file(name)`          | jeden zo súborov widgetu ako text                                             |
| `losos.asset(name)`         | jeden zo súborov widgetu ako URL `data:`, pre obrázok nastavený zo skriptu    |

Karta **Ako to funguje** v editore to zopakuje s príkladom. Jej náhľad je
skutočný rámec, takže dlaždica ukazuje presne to, čo náhľad.

:::warning[Vložený kód si pred uložením prečítajte]
Ručne napísaný widget smie sťahovať dáta z internetu, napríklad pre dlaždicu
s počasím, ale nesmie sa dostať k API zariadenia. Beží v každom prehliadači,
ktorý zariadenie otvorí, preto si všetko, čo vložíte z internetu, pred
uložením prečítajte.
:::

Limit je 24 widgetov, každý najviac 12 súborov a spolu 128 KiB.


Samotnú nástenku, pozadia a závoj opisuje [Vzhľad a widgety](../manual/look-and-widgets).
