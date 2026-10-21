---
title: Dlaždica aplikácie vráti 404
sidebar_position: 8
slug: /troubleshooting/app-tile-404
---

# Dlaždica aplikácie vráti 404

<div className="losos-symptom">

**Čo vidíte:** kliknutie na **Súbory**, **Fotky** alebo **Kód** na
Prehľade, alebo stará záložka, otvorí stránku „Not Found“.

</div>

![Panel Aplikácie s LosOS Cloud a LosOS Git; dlaždice aplikácií na Prehľade otvárajú tie isté aplikácie.](@site/docs/img/apps-sk.png)

## „Not Found“ z dlaždice aplikácie \{#not-found-from-an-app-tile}

- **Aplikácia je vypnutá.** **Súbory** a **Fotky** otvárajú LosOS Cloud,
  ktorý zapína a vypína `losos.cloud.enable` na paneli **Rozšírené**.
  **Kód** otvára LosOS Git, ktorý zapína a vypína panel **Aplikácie**.
- **Zariadenie sa práve spustilo.** LosOS Cloud sa otvorí pár sekúnd po
  tom, čo zariadenie nabehne. Pri prvom spustení po aktualizácii z
  Nextcloudu najprv presunie vaše súbory, čo trvá dlhšie
  ([Zariadenie, na ktorom bežal Nextcloud](../manual/losos-cloud#a-box-that-ran-nextcloud)).
  Počkajte a stránku načítajte znova.

## „Not Found“ zo starej záložky \{#not-found-from-an-old-bookmark}

Záložky na vlastné stránky Nextcloudu, napríklad
`/nextcloud/index.php/apps/calendar/`, už nikam nevedú: zariadenie
Nextcloud nemá. Otvorte namiesto nich `https://<name>.local/cloud/`.
Aplikácií pre počítač a telefón sa to netýka, LosOS Cloud im stále odpovedá
na adrese `/nextcloud`, s ktorou boli nastavené.

## Odkazy v LosOS Git ukazujú nesprávne meno \{#losos-git-links-show-the-wrong-name}

LosOS Git vypisuje URL na klonovanie s menom `.local` zariadenia, aj keď ste
sa k nemu dostali cez adresu. Stránky fungujú v oboch prípadoch. Pri
`git clone` dajte namiesto mena adresu, alebo pridajte meno do súboru hosts,
ako opisuje stránka [Meno sa nedá preložiť](name-does-not-resolve).
