---
title: Vzhľad a widgety
sidebar_position: 5
slug: /manual/look-and-widgets
---

# Vzhľad a widgety

## Vaša nástenka \{#your-board}

Nástenka na Prehľade pojme zopár widgetov. **Pridať widget** otvorí galériu
s tromi [typmi widgetov](../types/widget-types). Každú dlaždicu môžete
posunúť dopredu alebo dozadu, prípadne ju odstrániť. Widget, ktorý zlyhá,
ukáže chybu vo svojej dlaždici spolu s tlačidlom **Skúsiť znova**. Nástenka
patrí prehliadaču, v ktorom ste ju usporiadali. Iný počítač vidí vlastnú
nástenku. Pridanie vstavaného widgetu alebo widgetu zostaveného z hodnôt
zariadenia na zariadení nič nemení a nič nespúšťa.

![Prehľad s pozadím Príliv a ručne písaným widgetom Greeting na vrchu nástenky.](@site/docs/img/overview-tide-sk.png)

## Nastavenia → Vzhľad \{#settings--look}

Tento panel má každý prihlásený. Pozadie na ňom je len vaše: každý na
zariadení si vyberá vlastné a vaše vás sprevádza v každom prehliadači, kde
sa prihlásite, vrátane LosOS Cloud a LosOS Git. Zmena platí hneď, ako ju
uložíte, bez nového zostavenia.

![Nastavenia, Vzhľad: pozadia Bez obrázka, Príliv, Mriežka, Súmrak a váš vlastný obrázok, posuvník závoja a ručne písané widgety uložené na tomto zariadení.](@site/docs/img/settings-look-sk.png)

- **Vaše pozadie**: jeden z troch dodaných obrázkov (Príliv, Mriežka,
  Súmrak), vlastný nahraný obrázok (PNG, JPEG, WebP alebo GIF, do 8 MiB)
  alebo Bez obrázka.
- **Závoj**: kladie farbu stránky cez obrázok, od 20 % do 90 %, aby text
  zostal čitateľný vo svetlej aj tmavej téme.
- **Widgety napísané ručne**: ručne písané widgety uložené na tomto
  zariadení, ktoré vidia ľudia, čo smú meniť vzhľad zariadenia. Každý môžete
  upraviť alebo odstrániť.

## Písanie widgetu \{#writing-a-widget}

**Napísať**, v galérii alebo na paneli Vzhľad, otvorí editor so súbormi
widgetu, živým náhľadom a kartou Ako to funguje. Náhľad je skutočný
sandboxovaný rámec. Widget dostáva hodnoty a farby zariadenia cez objekt
`losos` opísaný na stránke [Typy widgetov](../types/widget-types#written-by-hand)
a nič iné. Nemá admin kľúč, API ani prístup k stránke okolo seba. Môže
sťahovať z internetu.

Nový widget začína tromi súbormi: `index.html`, ktorý prepája `style.css`
a `app.js`. **Pridať súbor** pridá ďalší, napríklad `data.json`, ktorý
skript načíta cez `fetch`, alebo `dot.svg` pre `<img>`. Editor počas písania
dopĺňa: objekt `losos`, mená hodnôt, farebné premenné zariadenia po `var(--`
a mená vlastných súborov widgetu. Ctrl+Medzerník ponúkne návrhy kdekoľvek.

![Editor widgetov: súbory a ich kód vľavo, živý náhľad v skutočnom sandboxovanom rámci vpravo a karta Ako to funguje.](@site/docs/img/widget-write-one-sk.png)

:::warning[Vložený kód si najprv prečítajte]
Ručne písaný widget beží v každom prehliadači, ktorý zariadenie otvorí.
Všetko, čo vložíte z internetu, si pred uložením prečítajte, ako hovorí aj
poznámka nad editorom.
:::
