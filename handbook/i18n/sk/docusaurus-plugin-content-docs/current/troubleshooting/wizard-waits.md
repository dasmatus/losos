---
title: Sprievodca stále čaká
sidebar_position: 7
slug: /troubleshooting/wizard-waits
---

# Sprievodca stále čaká

<div className="losos-symptom">

**Čo vidíte:** krok 2 prvého spustenia hlási „Čaká sa, kým sa zariadenie
dokončí spúšťať“ a zostane pri tom.

</div>

<img src={require("@site/docs/img/wizard-step2-waiting-sk.png").default} alt="Krok 2 sprievodcu čaká na LosOS cloud a uvádza, na čo čaká." width="520" />

## Prečo \{#why}

Heslo je uložené v LosOS cloud, takže sprievodca ho nemôže nastaviť, kým sa
LosOS cloud nenainštaluje. LosOS cloud to robí pri prvom štarte zariadenia.
Na mini-PC to trvá dve až päť minút; v emulovanom VM bez hardvérovej
akcelerácie to môže trvať pätnásť. Stránka to kontroluje každých pár sekúnd
a pokračuje sama.

## Čo robiť \{#what-to-do}

1. **Počkajte desať minút**, kým urobíte čokoľvek iné. Otvorte
   `http://<address>/api/setup/claim` na inej karte. Ak hlási
   `"ready": false`, jeho pole `waitingFor` uvádza, na čo zariadenie čaká.
2. **Po pätnástich minútach** vytiahnite napájanie, aby sa zariadenie
   reštartovalo, a sprievodcu otvorte znova. Prvé spustenie LosOS cloud sa
   dá po reštarte zopakovať.
3. **Zariadeniu, ktoré sa tam nedostane** ani po reštarte, len zriedka chýba
   sieť, pretože nič nesťahuje a všetko je na zariadení. Zvyčajne má plný
   disk alebo zlyhanú inštaláciu. Ak sa správcovské stránky otvárajú,
   skontrolujte panel Úložisko, inak preinštalujte.

## Známa chyba \{#a-known-bug}

Zostavenia spred 6. októbra 2026 mohli nechať panel na stave „spúšťa sa“ aj
potom, čo bol LosOS cloud pripravený. Opravil to pull request #62. Ak po
opätovnom načítaní stránka hneď pokračuje, narazili ste na ňu.
