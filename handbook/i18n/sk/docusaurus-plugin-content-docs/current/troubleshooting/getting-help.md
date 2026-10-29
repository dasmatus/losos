---
title: Kde hľadať pomoc
sidebar_position: 13
slug: /troubleshooting/getting-help
---

# Kde hľadať pomoc

Problémy a otázky posielajte do
[systému na hlásenie chýb](https://github.com/dasmatus/losos/issues)
projektu. Zariadenie nemá shell, takže to, čo zhromaždíte z LAN, je všetko,
o čo vás správca projektu môže požiadať. Zhromaždite to skôr, ako napíšete.

## Nechajte ho napísať zariadenie \{#let-the-box-write-it}

Pád, služba, ktorá skončí chybou, alebo program zastavený pre nedostatok
pamäte už je v **Nastavenia → Problémy** s hlásením napísaným zo záznamu
zariadenia. Prečítajte si ho tam a odošlite v dvoch krokoch:
[Hlásenia problémov](../manual/problem-reports.md) hovorí ako. Neúspešná
inštalácia ponúkne to isté na obrazovke inštalátora.

Pri všetkom ostatnom napíšte issue sami.

## Čo uviesť \{#what-to-include}

![Panel O zariadení, ktorého riadky patria do hlásenia: meno, adresa, režim úložiska a stav mesh.](@site/docs/img/settings-about-sk.png)

1. **Čo ste urobili a čo ste videli**, s presným textom každej hlášky a so
   snímkou obrazovky, ak nejaká bola.
2. **Text bannera**: adresu a meno `.local`.
3. **Panel O zariadení**: meno, adresu, na ktorej ste sa k zariadeniu
   dostali, režim úložiska, stav mesh.
4. **Odpoveď z** `http://<address>/api/health` a
   `http://<address>/setup/state.json`. Druhá neobsahuje žiadne tajomstvo,
   iba meno, adresu a odtlačok certifikátu.
5. **Záznamy z widgetu Zmeny**, ak ide o použitie zmien alebo aktualizáciu:
   názov úlohy a text jej výsledku.
6. **Ako bolo zariadenie nainštalované**: TPM alebo súbor s kľúčom, UEFI
   alebo BIOS, z ktorého vydania, na akom hardvéri alebo v akom softvéri pre
   virtuálne stroje. Prvé dve veci vypísala inštalácia.
7. **Kedy.** Na dennej dobe záleží kvôli reštartu o 00:07 a aktualizácii
   o 03:00.

## Čo neuvádzať \{#what-not-to-include}

Vynechajte náhradný správcovský kľúč, heslo vlastníka a obsah vašich
súborov. Nikto ich nepotrebuje a systém na hlásenie chýb je verejný.

## Ak je to naliehavé \{#if-it-is-urgent}

Reštart opraví všetko, čo nie je na šifrovanom zväzku, či už príde o 00:07,
alebo preto, že vytiahnete napájanie. **Reset** vráti nastavenia späť. Obe
veci môžete urobiť ešte dnes večer. Odpoveď správcu projektu môže trvať
dlhšie.
