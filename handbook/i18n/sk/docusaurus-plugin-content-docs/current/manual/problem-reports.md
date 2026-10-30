---
title: Hlásenia problémov
sidebar_position: 6.5
slug: /manual/problem-reports
---

# Hlásenia problémov

Zariadenie nemá shell a často ho nikto nesleduje, preto si samo vedie
zoznam toho, čo sa pokazilo. Keď je niečo v tom zozname pre projekt nové,
napíše hlásenie, ktoré si môžete celé prečítať a poslať vývojárom ako
issue na GitHube. Nič neopustí zariadenie, kým to neodošlete.

## Čo si zariadenie všíma \{#what-the-box-notices}

Raz za minútu zariadenie prečíta svoj záznam a hľadá tri druhy problémov:

- služba, ktorá **skončila chybou**, napríklad neúspešné spustenie alebo
  vypršaný čas;
- program, ktorý **spadol**, ukončený signálom ako `SIGSEGV` alebo
  `SIGABRT`;
- program, ktorý jadro **zastavilo, lebo zariadeniu došla pamäť**.

Ten istý problém znova je jedna položka s počtom, nie nová.
**Nastavenia → Problémy** ich zobrazuje od najnovšieho.

Zariadenie nezapisuje výpisy pamäte (core dump). Výpis bežiacej služby by
obsahoval jej tajomstvá, preto sa hlásenie píše zo záznamu.

## Problémy, ktoré príručka vysvetľuje \{#problems-the-handbook-explains}

Niektoré zlyhania už tu majú svoju stránku, napríklad disk, na ktorom
[nie je miesto](../troubleshooting/out-of-room.md), alebo
[Použitie, ktoré nemôže nič stiahnuť](../troubleshooting/apply-fails.md).
Pri nich položka povie, ktorá stránka to je, odkáže na ňu a hlásenie
neponúkne: stránka povie, čo robiť, a vývojári to už vedia.

## Čítanie hlásenia \{#reading-a-report}

**Prečítať hlásenie** otvorí celý text presne tak, ako by ho dostal GitHub:

- čo sa stalo, kedy a koľkokrát;
- stav služby;
- záznam tej služby okolo času zlyhania, alebo riadky jadra pri páde či
  zastavení pre pamäť;
- stroj: vydanie, firmvér, procesor, pamäť a jadro.

Skôr ako ho uvidíte, zariadenie z neho odstráni to, čo by vás mohlo
identifikovať: IP a MAC adresy, e-mailové adresy, dlhé kľúče a tokeny,
mená Tor a I2P, vlastný názov zariadenia a mená ľudí, ktorí ho používajú.
Aj tak si ho prečítajte. Názov súboru alebo správa v zázname môže
prezradiť viac, než chcete mať verejne.

## Odoslanie \{#sending-one}

**Odoslať na GitHub** požiada GitHub o kód. Na ľubovoľnom telefóne alebo
počítači otvorte `github.com/login/device`, prihláste sa a zadajte kód,
ktorý zobrazí panel. Hlásenie potom odíde ako issue pod vaším účtom na
GitHube, takže sa vás vývojári môžu na niečo opýtať. Zariadenie si
prihlásenie ponechá len do odoslania hlásenia.

Najprv môžete pridať poznámku, napríklad čo ste robili, keď sa to stalo.
Pridá sa nad hlásenie.

Ak ten istý problém už niekto nahlásil, vaše hlásenie sa pridá k jeho
issue ako komentár a druhé sa neotvorí. Názov každého hlásenia končí
krátkym odtlačkom, napríklad `[8c1f04e2]`, a podľa neho sa dve hlásenia
považujú za rovnaké.

**Zastaviť** kedykoľvek ukončí čakanie na kód. Nič sa neodošle.

**Skryť** odstráni položku zo zoznamu. Vráti sa, ak sa problém zopakuje.

## Z inštalátora \{#from-the-installer}

Inštalačné médium hlási rovnako, na vlastnej obrazovke:

- Keď inštalácia **zlyhá** na niečom, čo táto príručka nevysvetľuje,
  obrazovka ponúkne hlásenie. Keď to príručka vysvetľuje, obrazovka namiesto
  toho uvedie stránku.
- Počas inštalácie inštalátor sleduje **využitie pamäte a procesora**.
  Médium beží celé z pamäte, takže ak pamäť zostane nad 90 % niekoľko
  sekúnd alebo procesor nad 95 % minútu, na konci to oznámi, aj keď sa
  inštalácia dokončila, a ponúkne hlásenie.

Celé hlásenie je na obrazovke 7: podržte **Alt** a stlačte **F7**, aby ste
si ho prečítali, a **Alt+F1** vás vráti späť. Stlačením **R** ho odošlete.
Obrazovka potom ukáže adresu a kód, ktorý zadáte na inom zariadení, a po
podaní adresu issue. Akýkoľvek iný kláves pokračuje bez odoslania.

## Bez JavaScriptu \{#without-javascript}

[Stránky bez JavaScriptu](without-javascript.md) majú ten istý panel
**Problémy**. Kým čaká na kód, stránka sa každých pár sekúnd sama obnoví.
