---
title: LosOS značka
sidebar_position: 4.7
slug: /manual/losos-tag
---

# LosOS značka

LosOS značka je asistent, ktorého v LosOS Chate označíte. Napíšte
`@LosOS` do správy — alebo prijmite návrh, ktorý skladateľ ukáže, keď
otázky v miestnosti zostali nezodpovedané — a box odpovie vo vlákne
tým, čo našlo jeho vlastné vyhľadávanie: vaše súbory v LosOS Cloude,
kód v LosOS Gite, vaša pošta, miestnosti chatu. Nič neopúšťa box;
odpoveď beží na vlastnom modeli boxu.

Značka býva v **Nastavenia › LosOS značka**, kde určujete štyri veci.

## Úrovne a oblasti

*Úroveň* je slovo tohto boxu pre to, aké súkromné je jedno dátum, od
najmenej súkromných (predvolene `everyday`; správca pridáva ďalšie —
`personal`, `secret`, akékoľvek vlastné slová boxu). *Oblasť* označuje
jednu časť dát boxu jednou úrovňou: priečinok vo vašich súboroch,
repozitár, miestnosť, poštový priečinok, jednotka žurnálu.

Pravidlo je jeden riadok: **značka číta oblasť len keď jej oprávnenie
dosahuje úroveň oblasti.** Čím súkromnejšia oblasť, tým vyššie
oprávnenie asistent potrebuje. Vaše vlastné oprávnenie (to, čo značka a
spoločník vo vašich AI nástrojoch číta za vás) si nastavíte na rovnakej
stránke.

## Asistenti

Každý asistent je skutočný účastník chatu — `@LosOS` je ten vstavaný —
s vlastnou personou, oprávnením a tvorcom. Označte ho v správe a
odpovie; označte dvoch a odpovedia obaja, a odpoveď, ktorá označí
iného asistenta, odovzdá ťah ďalej, najviac tri odovzdania, takže dvaja
asistenti môžu niesť vlákno medzi sebou bez nekonečnej slučky.
Asistent číta len do vlastného oprávnenia, nech jeho persona hovorí
čokoľvek: úrovne sú pravidlo boxu, nie nálada modelu.

## Zručnosti

Zručnosť je pokyn, ktorý si dopyt môže vziať — vstavané prichádzajú s
boxom (*Nájdi, kde som to napísal*, *Zhrň toto vlákno*, *Čo mi ušlo*,
*Naplánuj to*) a vlastné si môžete napísať. Zručnosť nikdy nezvyšuje
oprávnenie a nikdy nedarí nástroj; hovorí len ako postupovať pri
odpovedi.

## Čo značka nikdy nerobí

Bez ohľadu na oprávnenie značka nikdy nevolá `sudo` ani shell, nikdy
nekupuje, nepredáva, neprenajíma ani nepožičiava úložisko či výpočty a
nikdy sa nedotýka ničoho na disku v adresároch vlastníka — jeho súbory
sa čítajú a píšu cez vlastnú aplikáciu LosOS Cloudu, nie cez asistenta.
Webové vyhľadávanie, keď ho box prevádzkuje (vlastné SearXNG, nikdy
otvorený internet), berie len predmet: žiadne mená, súbory ani pošta z
boxu sa nikdy nedostanú do dopytu.

## Väčšie modely, cez mesh

Model značky je jeden zjednotený model boxu (predvolene Bonsai 27B —
1-bitový záznam Qwen3.6-27B: 4,4 GB, nástroje, premýšľanie, videnie).
Box, ktorý ho neudrží, sa pýta cez sieť: v meshi edge prevádzkuje hlavu
pre každú skupinu modelov a boxy jej požičiavajú pamäť
([mesh](../in-depth/mesh), [AI modely](../in-depth/ai-models)), takže
4 GiB box sa môže opýtať Qwen 3.8 Max alebo DeepSeek R1 — hlava
odpovedá, požičiavatelia držia váhy. Front dopytov pozastaví to, na čo
nikto nečaká, kým má box málo pamäte, a obnoví ho sám; nič sa
nezahodí.
