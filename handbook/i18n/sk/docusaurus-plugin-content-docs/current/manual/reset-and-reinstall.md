---
title: Reset a preinštalovanie
sidebar_position: 9
---

# Reset a preinštalovanie

Reset a preinštalovanie sú dve rôzne veci a robia sa z rôznych miest.

## Reset: vrátiť všetky nastavenia \{#reset-put-every-setting-back}

**Nastavenia → Reset → Reset…** vráti všetky nastavenia do stavu, v akom
zariadenie prišlo: názov, sieť, mesh, aplikácie. Vašich súborov, fotiek ani
repozitárov sa nedotkne. Zariadenie sa znova zostaví a vráti sa pod
pôvodným názvom, takže aj meno, na ktorom ho nájdete, sa zmení späť. IP
adresa zostane rovnaká. Dialóg vypíše, čo zmizne a čo zostane, a stlačiť
musíte ešte raz.

![Dialóg resetu, ktorý vypisuje, čo sa vráti a čo zostane, s tlačidlom Ponechať moje nastavenia ako prvým.](@site/docs/img/settings-reset-dialog.png)

![Nastavenia, Reset: Vrátiť všetky nastavenia, s tým, čo zmizne a čo zostane.](@site/docs/img/settings-reset.png)

Reset nezmení heslo vlastníka ani náhradný kľúč a znova neotvorí okno na
prevzatie zariadenia.

Ak chcete zmazať aj dáta, použite **Vymazať…** pod ním na tej istej
obrazovke. [Záloha a vymazanie](./backup-and-erase) ho vysvetľuje spolu
s tým, ako záloha potom všetko vráti.

## Preinštalovanie: zmazať a začať odznova \{#reinstall-wipe-and-start-over}

Keď znova spustíte zariadenie z USB kľúča, nainštaluje sa od
začiatku. Inštalácia **zmaže každý disk** vrátane vašich súborov, hesla,
náhradného kľúča, identity zariadenia v mesh a jeho kľúčov. Možnosť
„ponechať moje dáta“ zámerne neexistuje. Zariadenie je jedna kópia a vaše
dáta sú v bezpečí tam, kde máte druhú.

Po preinštalovaní je zariadenie nové zariadenie. Znova prejdite
sprievodcom, vytlačte nový náhradný kľúč a dôverujte novému certifikátu. Ak
používate edge, nechajte zariadenie znova zaregistrovať pod jeho novou
identitou.

## Čo preinštalovanie nepotrebuje \{#what-a-reinstall-does-not-need}

Počítač, ktorý naštartuje z USB kľúča. To je všetko. Jediné, čo
treba zachovať, sú nastavenia firmvéru zariadenia: vypnutý Secure Boot pre
nainštalované zariadenie alebo zapísaný certifikát LosOS pre USB kľúč.
[Secure Boot](../start/secure-boot) vysvetľuje oboje.
