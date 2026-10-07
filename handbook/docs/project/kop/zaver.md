---
title: Záver
sidebar_position: 10
---

# Záver

## Čo bolo dosiahnuté

Zadanie žiadalo preskúmať a opísať funkcionality a možnosti operačného
systému, vybrať jadro, systém nainštalovať, nakonfigurovať a overiť,
spracovať cenovú ponuku a vypracovať technickú dokumentáciu. Všetky štyri
body sú splnené a každý má v tomto dokumente kapitolu.

Hotové a otestované je: bezstavové zariadenie na NixOS so šifrovaným
trvalým zväzkom, odomykanie z TPM aj z kľúčového súboru, inštalácia UEFI aj
BIOS bez obsluhy, administračné stránky s jedným heslom a náhradným
kľúčom, LosOS cloud a LosOS Git ako záťaž v lokálnom clustri s jednotným
vzhľadom, nočná samooprava a samoaktualizácia so stropom generácií a GC,
rast disku za behu, hardening postavený z primitív KSPP s testom
chránených výnimiek, edge s tunelom, meshom a oknom výpočtov, trh od konca
po koniec proti náhradám Stripe, riadiaca rovina edge pre prezentácie na
Vercel, príručka majiteľa na každom boxe a táto dokumentácia.

Projekt má približne 45 000 riadkov vlastného kódu v Nixe, Ruste
a TypeScripte, vyše 200 jednotkových testov, sedemnásť testov vo
virtuálnych strojoch, invarianty vyhodnocované pri zostavení, testy
v prehliadači a CI, ktoré inštalačné médium nielen zostaví, ale aj
nabootuje.

## Čo je plánované

- **Otvorenie trhu** po založení podnikania, ktoré bude platformou Stripe;
  dovtedy ostáva karta Trh sivá. Prvý beh proti testovaciemu režimu Stripe.
- **Objavovanie edge a oficiálna identita edge** (pripravované v PR #75):
  box hľadá edge v LAN aj na nastavenej adrese, odmieta zdieľanie bez
  dosiahnuteľného edge a obchoduje len s edge podpísaným koreňovým kľúčom
  projektu.
- **Firemné nasadenie** ako produkt: reprodukovateľný skript pre edge
  a boxy v jednej sieti, návod pre operátora firmy.
- **Preklady príručky** do slovenčiny a nemčiny.
- **Oddelená CA s obmedzením mena** namiesto certifikátu boxu, ktorý je
  zároveň koreňovou autoritou.
- **Rozšírenie VM testov** o boot nainštalovaného systému až po prihlásenie
  do Nextcloudu (dnes to overuje ukážka, nie test).

## Poučenia

- **Testy majú chrániť rozhodnutia, nie len správanie.** Najcennejšie
  testy projektu sú tie, ktoré zlyhajú, keď niekto „opraví“ niečo, čo je
  vypnuté zámerne.
- **Lintre a kontroly do CI od prvého dňa.** Kód v Ruste driftoval bez
  rustfmt a clippy, piny nixpkgs sa rozišli bez kontroly; obe veci stáli
  viac času dodatočne než by stáli na začiatku.
- **Chyby, ktoré vidno len na skutočnom štarte.** Dve najvážnejšie chyby
  projektu (kľúčový súbor s NUL bajtmi, chýbajúce virtio ovládače v initrd)
  prešli všetkými testami, lebo žiadny test nebootoval nainštalovaný systém
  až do konca. Ukážka vo virtuálnom stroji nie je len prezentácia; je to
  test.
- **Nezávislá recenzia vlastnej práce** našla za jedno popoludnie sedem
  vecí, ktoré by komisia našla tiež. Oplatí sa ju urobiť skôr, než ju urobí
  niekto iný.
- **Infraštruktúra CI má limity, ktoré formujú architektúru.** Desaťminútový
  strop bývalého hostiteľa CI viedol k rozdeleniu úloh a k importu
  artefaktov medzi nimi; štruktúra ostala, aj keď strop padol, lebo dôvody
  ju prežili.

Jednou vetou: LosOS robí z vyradeného mini-PC vlastný cloud, ktorý sa sám
spravuje, a z jeho voľných zdrojov susedskú sieť, z ktorej sa raz dá aj
zarábať.
