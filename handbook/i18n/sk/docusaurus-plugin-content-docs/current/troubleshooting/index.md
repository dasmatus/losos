---
title: Keď niečo nefunguje
sidebar_position: 0
sidebar_label: Začnite tu
slug: /troubleshooting
---

# Keď niečo nefunguje

Táto kapitola počíta s najhorším prípadom, s ktorým sa ešte dá pracovať.
**Ste v tej istej lokálnej sieti ako zariadenie a nič iné nemáte.** Žiadny
internet, žiadny edge, žiadny shell. Každá stránka je usporiadaná podľa
toho, čo vidíte, a každá začína krokmi, ktoré fungujú len zo samotnej LAN.

:::note[Keď chýba iba svet zvonku]
Ak sa vlastné stránky zariadenia otvárajú a chýba iba svet zvonku, chyba nie
je v zariadení. Stránka [Funguje iba lokálna sieť](/troubleshooting/only-the-lan-works.md)
vymenúva príčiny, napríklad linku poskytovateľa, pripojenie routera do
internetu alebo edge. Uvádza aj to, čo stále funguje, čo počká a prečo
potom nemusíte nič znova nastavovať.
:::

## Prvých päť minút \{#the-first-five-minutes}

Postupujte v tomto poradí. Väčšina problémov sa skončí pri jednom z krokov.

1. **Pozrite sa na obrazovku zariadenia.** Banner s `LosOS <version> is ready`
   a adresou `http://` znamená, že zariadenie naštartovalo a má sieť. Pod
   adresou je zoznam častí LosOS, ktoré zariadenie prevádzkuje, s fajkou pri
   každej, ktorá beží. Časť, ktorá nebeží, má krížik a nadpis sa zmení na
   varovanie. `LosOS <version> cannot start its admin pages` znamená, že
   adresa otvorí stránku, ktorá sa nenačíta, kým sa nespustia. Žiadny
   banner, textová výzva na prihlásenie alebo výzva na heslo disku znamenajú
   iný problém. Pozrite [Zaseknuté pri výzve na heslo disku](/troubleshooting/stuck-at-passphrase.md),
   pri čiernej obrazovke [Zariadenie je nedostupné](/troubleshooting/cannot-reach-the-box.md).
2. **Použite IP adresu, nie meno.** Napíšte `http://192.168.…` z bannera do
   prehliadača v tej istej sieti. Polovica hlásení „zariadenie nebeží“
   prichádza z počítača, ktorý nevie preložiť `.local`. Pozrite
   [Meno sa nedá preložiť](/troubleshooting/name-does-not-resolve.md).
3. **Skúste obyčajné `http://`.** Adresa `https://` s varovaním o certifikáte
   nie je výpadok. Pozrite [Varovanie o certifikáte](/troubleshooting/certificate-warning.md).
4. **Opýtajte sa zariadenia, či žije.** Otvorte `http://<address>/api/health`.
   Odpoveď `{"ok":true}` znamená, že riadiaci démon beží. Na
   `http://<address>/setup/state.json` nájdete meno a adresu zariadenia tak,
   ako ich vidí ono samo.
5. **Počkajte na najbližší reštart alebo ho vyvolajte.** Zariadenie sa samo
   reštartuje o 00:07 a pri tom si systémový disk zostaví nanovo. Vytiahnutie
   napájania a jeho opätovné zapojenie urobí to isté hneď. Reštart sa
   nedotkne vašich súborov, tie sú na šifrovanom zväzku.

## Čo vám zariadenie povie z LAN \{#what-the-box-can-tell-you-from-the-lan}

![Prehľad, keď zariadenie neodpovedá na kontrolu stavu: stav pod menom sa zmení z Odpovedá na neodpovedá.](@site/docs/img/overview-not-answering-sk.png)

| Kde                                     | Čo                                                                                           |
| --------------------------------------- | -------------------------------------------------------------------------------------------- |
| Obrazovka (tty1)                        | IP adresa, meno `.local` a ktoré časti LosOS bežia. Prekreslí sa, keď sa zmenia.             |
| `http://<address>/api/health`           | Či riadiaci démon odpovedá.                                                                   |
| `http://<address>/setup/state.json`     | Meno, adresa, či je zapnuté HTTPS, odtlačok certifikátu.                                      |
| Stránka Prehľad                         | Odpovedá alebo nie, doba behu, aplikácie bežia alebo nie, widget **Zmeny** s výsledkom každej prestavby. |
| Panel O zariadení                       | Meno, adresa, režim úložiska, stav mesh, dva časovače.                                        |
| Panel Úložisko                          | Ako plný je disk.                                                                             |
| Panel Mesh                              | Či sa našiel edge a aký.                                                                      |

## Čo vám nepovie \{#what-it-cannot-tell-you}

Nie je tu prehliadač logov ani terminál, pretože nie je shell. Ak nič z
tohto problém nevysvetlí, platí jedno z dvoch. Buď ho nočný reštart opraví
sám, alebo treba preinštalovať. Stránka [Kde hľadať pomoc](/troubleshooting/getting-help.md)
hovorí, čo zhromaždiť, skôr ako sa opýtate.

## Stránky \{#the-pages}

- [Funguje iba lokálna sieť](/troubleshooting/only-the-lan-works.md)
- [Zariadenie je nedostupné](/troubleshooting/cannot-reach-the-box.md)
- [Meno sa nedá preložiť](/troubleshooting/name-does-not-resolve.md)
- [Varovanie o certifikáte](/troubleshooting/certificate-warning.md)
- [Zabudnuté heslo](/troubleshooting/forgot-password.md)
- [Zaseknuté pri výzve na heslo disku](/troubleshooting/stuck-at-passphrase.md)
- [Sprievodca stále čaká](/troubleshooting/wizard-waits.md)
- [Dlaždica aplikácie vráti 404](/troubleshooting/app-tile-404.md)
- [Použitie zmien zlyhá](/troubleshooting/apply-fails.md)
- [Nie je miesto](/troubleshooting/out-of-room.md)
- [Nenašiel sa žiadny edge](/troubleshooting/edge-not-found.md)
- [Zdieľanie je odmietnuté](/troubleshooting/sharing-refused.md)
- [Kde hľadať pomoc](/troubleshooting/getting-help.md)
