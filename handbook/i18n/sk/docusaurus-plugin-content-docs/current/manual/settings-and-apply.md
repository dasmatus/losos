---
title: Nastavenia a Použiť
sidebar_position: 3
---

# Nastavenia a Použiť

Zariadenie LosOS nemá nastavenia v bežnom zmysle. Má opis samého seba
napísaný v Nixe a panely nastavení upravujú jeho malú časť. Použitie zmeny
zostaví zariadenie nanovo podľa nového opisu. Preto:

- Zmena si vyžaduje zostavenie, ktoré trvá niekoľko minút, nie reštart.
  Zariadenie počas práce zostáva dostupné a upozornenie povie, kedy je
  hotovo.
- Zmena sa buď prejaví celá, alebo vôbec. Keď zostavenie zlyhá, zariadenie
  beží ďalej s predchádzajúcimi nastaveniami.
- Každé zariadenie sa dá zostaviť zo zdrojového kódu LosOS a jedného súboru
  s jeho voľbami, `modules/overrides.nix`. Najbližšie uloženie vráti každé
  nastavenie, ktoré panely nezapisujú.

## Lišta Použiť \{#the-apply-bar}

Upravte ktorékoľvek pole a dole sa objaví lišta: *N zmien ešte nie je
použitých*, s tlačidlami **Zahodiť** a **Použiť**. Stránka označí pole,
ktoré neprejde kontrolou, napríklad názov s medzerou alebo port mimo
rozsahu, a Použiť počká, kým ho neopravíte. Kým nestlačíte Použiť,
k zariadeniu sa nič nedostane.

![Názov s medzerou: pole je označené a Použiť čaká, kým sa neopraví.](@site/docs/img/settings-apply-invalid.png)

![Panel Sieť s napísaným novým názvom: lišta hovorí, že jedna zmena ešte nie je použitá.](@site/docs/img/settings-apply-bar.png)

## Čo urobí Použiť \{#what-apply-does}

1. Zariadenie zapíše vaše voľby do svojho súboru prepísaní (overrides).
2. Spustí zostavenie ako úlohu na pozadí (`losos-rebuild-<job>`). Widget
   **Zmeny** na Prehľade tieto úlohy vypisuje a ukazuje, či sa každá
   podarila.
3. Zariadenie sa prepne na nový systém. Niektoré zmeny uprostred reštartujú
   riadiaceho démona zariadenia. Démon si úlohu prevezme späť sám.
4. Upozornenie povie *Zmeny sú použité* alebo *Zmeny sa nepodarilo použiť*
   s dôvodom, ktorý zariadenie uviedlo.

## Dve nastavenia, ktoré menia adresu \{#two-settings-that-change-the-address}

![Panel História: každá použitá zmena, zmena režimu a reset je commit vo vlastnom opise zariadenia, od najnovšieho.](@site/docs/img/settings-history.png)

- **Názov**: zariadenie odpovedá na `<name>.local` a aplikácie rozdávajú
  odkazy s týmto menom. Po premenovaní sa k zariadeniu dostanete na novom
  mene alebo na jeho IP adrese, ktorá sa nemení.
- **Šifrovať pripojenie**: keď ho vypnete, zariadenie hovorí iba obyčajným
  HTTP a krok s certifikátom aj prístupové kľúče (passkeys) zmiznú s ním.

## Nastavenia, ktoré nie sú Nix \{#settings-that-are-not-nix}

Dve veci na správcovských stránkach platia hneď, bez zostavenia, pretože sú
to dokumenty, ktoré zariadenie uchováva, a nie časti jeho opisu:

- **Vzhľad**, teda obrázok pozadia, závoj a ručne písané widgety. Drží ho
  riadiaci démon zariadenia, takže každý prehliadač vidí to isté.
- **Vaša nástenka** widgetov. Drží ju prehliadač, v ktorom ste ju
  usporiadali.

## Úplný zoznam \{#the-full-list}

[Tabuľka nastavení a volieb](../reference/settings-to-options) priraďuje
každé pole panelov k voľbe Nixu, ktorá je za ním, pre deň, keď budete chcieť
zmeniť niečo, čo panely neponúkajú.

![Panel Rozšírené: každá voľba losos s aktuálnou hodnotou a editorom podľa typu a odkazy na panely, ktoré vlastnia tie bežné.](@site/docs/img/settings-advanced.png)
