---
title: Zariadenie je nedostupné
sidebar_position: 2
slug: /troubleshooting/cannot-reach-the-box
---

# Zariadenie je nedostupné

<div className="losos-symptom">

**Čo vidíte:** prehliadač hlási, že stránka nie je dostupná alebo že
vypršal časový limit pripojenia, alebo sa stránka donekonečna načítava.
Na zariadení neodpovedá nič.

</div>

## Z LAN \{#from-the-lan}

![Banner, ktorý zariadenie ukazuje pod lososom, keď naštartovalo a má sieť, s adresou, ktorú treba napísať.](@site/docs/img/tty1-banner.png)

1. **Prečítajte si obrazovku.** Banner ukazuje adresu, ktorú má zariadenie
   *teraz*. Router po reštarte mu mohol prideliť novú a banner sa prekreslí
   sám. Napíšte túto adresu ako `http://`, nie meno a nie `https://`.
2. **Tá istá sieť?** Notebook na hosťovskej Wi-Fi alebo vo VPN je v inej
   sieti. Zariadenie aj počítač potrebujú adresu z toho istého rozsahu,
   napríklad oba `192.168.1.x`. Správcovské stránky navyše odmietnu každú
   adresu, ktorá nie je súkromná. Požiadavka, ktorá príde cez tunel alebo
   z internetu, dostane zámerne **403**.
3. **Banner hlási „No network address yet.“** Zapojte ethernetový kábel
   alebo ho presuňte do iného portu na routeri. Zariadenie nemá nastavenie
   Wi-Fi. Obrazovka sa aktualizuje sama, keď kábel nadviaže spojenie.
4. **Žiadny banner, čierna obrazovka.** Po zapnutí počkajte dve minúty. Ak
   obrazovka zostane čierna, skontrolujte kontrolku napájania a vstup
   monitora. Textová výzva na prihlásenie namiesto bannera je jediný stav,
   ktorý opraví nočný reštart. Počkajte do 00:07 alebo vytiahnite napájanie.
5. **Nefunguje iba jedna stránka.** `http://<address>/api/health` odpovedá,
   ale LosOS Cloud nie. Buď sa aplikácia ešte spúšťa, čo po každom reštarte
   trvá pár minút, alebo pozrite [Dlaždica aplikácie vráti 404](app-tile-404).

## Mimo LAN \{#from-outside-the-lan}

Samostatné zariadenie zvonku dostupné nie je a nie je to chyba. Zariadenie
s edge je na svojom verejnom mene dostupné iba pre LosOS Cloud a LosOS Git.
Správcovské stránky zariadenie nikdy nezverejňuje. Ak verejné meno prestane
odpovedať, tunel nefunguje. V LAN panel Mesh ukáže, či zariadenie edge
nachádza, a prípad, keď ho nenachádza, rieši stránka
[Nenašiel sa žiadny edge](edge-not-found). Keď zariadenie edge nájde,
pripojí sa znova samo.

## Stále nič \{#still-nothing}

:::tip[Načítajte stránku mimo cache]
Ak banner ukazuje adresu a `http://<address>/api/health` odpovedá, ale
správcovské stránky sa nenačítajú, prehliadač môže mať v cache stránku
spred aktualizácie. Načítajte ju znova cez Ctrl+Shift+R, čím cache
obídete, alebo otvorte súkromné okno.
:::

Ak sa stránka načíta, ale každý panel čaká alebo ukazuje chybu, riadiaca
služba možno nebeží. `https://<address>/basic/settings/logs` sa otvorí aj
bez nej a ukáže jej riadky, s náhradným kľúčom správcu, ak neodpovedá ani
LosOS Login. Pozrite [Záznamy](../manual/logs).

Ak banner ukazuje adresu a na nej neodpovedá vôbec nič, nefunguje webový
server zariadenia, nginx, a opraví ho nočný reštart. Ak nefunguje ani
nasledujúce ráno, [požiadajte o pomoc](getting-help) a priložte text
z bannera.
