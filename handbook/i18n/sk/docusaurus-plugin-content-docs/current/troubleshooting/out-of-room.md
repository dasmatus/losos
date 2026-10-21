---
title: Nie je miesto
sidebar_position: 10
slug: /troubleshooting/out-of-room
---

# Nie je miesto

<div className="losos-symptom">

**Čo vidíte:** Prehľad hlási **Takmer plné** alebo **Nie je miesto**;
nahrávanie do LosOS Cloud zlyháva; použitie zmien alebo nočná aktualizácia
zlyhá.

</div>

![Prehľad na zariadení, ktoré je takmer plné.](@site/docs/img/overview-almost-full-sk.png)

## Prečo na tom záleží viac ako na notebooku \{#why-it-matters-more-than-on-a-laptop}

Vlastný softvér zariadenia je na tom istom zväzku ako vaše súbory. Každá
aktualizácia pridá jednu verziu a upratovanie o 04:30 odstráni verzie
staršie ako 14 dní. Plný disk preto zablokuje práve to jediné, čo zariadenie
opravuje, takže konajte skôr, než sa objaví **Nie je miesto**.

## Čo robiť, v tomto poradí \{#what-to-do-in-order}

![Použitie rezervy, to prvé, čo treba skúsiť: dialóg uvádza, koľko miesta pridá.](@site/docs/img/storage-use-reserve-sk.png)

1. **Zaberte rezervu.** Panel Úložisko → **Použiť rezervu**. Inštalátor na
   to odložil 10 % disku. Zabratie trvá minútu a nepotrebuje reštart.
   Pozrite [Zväčšenie disku](../manual/disk-growth).
2. **Vyprázdnite kôš v LosOS Cloud.** Odstránené súbory zostávajú
   v „Vymazané súbory“ 30 dní a stále zaberajú miesto. Náhľady aplikácie
   Fotky a staré verzie súborov (karta „Verzie“ v bočnom paneli súboru) sa
   tiež nasčítajú.
3. **Počkajte jednu noc.** Upratovanie o 04:30 zmaže staré verzie systému.
   Ak bolo zariadenie za posledné dva týždne nainštalované alebo veľa
   aktualizované, môže ísť o niekoľko gigabajtov.
4. **Presuňte dáta preč** desktopovým klientom alebo stiahnutím z LosOS
   Cloud a potom ich tam zmažte.
5. **Zdieľajte menej.** Zariadenie, ktoré požičiava miesto do mesh (režim
   úložiska nastavený na zdieľaný), drží kópie pre iné zariadenia. Vypnutie
   zdieľania toto miesto vráti, len čo spoločný priestor znova rozloží svoje
   repliky.
6. **Pridajte disk** alebo preinštalujte na väčší. Pozrite [Zväčšenie disku](../manual/disk-growth).

## Panel Úložisko neuvádza veľkosť \{#the-storage-pane-does-not-report-a-size}

Hláška „Zariadenie nenahlásilo, aký plný je jeho disk.“ je hneď po reštarte
minútu normálna. Ak zostane, riadiaci démon neodpovedá.
`http://<address>/api/health` by mal vrátiť `{"ok":true}`. Ak nevráti,
opraví to nočný reštart.
