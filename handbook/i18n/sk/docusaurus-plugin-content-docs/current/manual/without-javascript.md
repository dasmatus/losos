---
title: Bez JavaScriptu
sidebar_position: 1.5
slug: /manual/without-javascript
---

# Bez JavaScriptu

Správcovské stránky fungujú aj v prehliadači s vypnutým JavaScriptom.
Otvorte zariadenie na obvyklej adrese a prehliadač vás pošle na tú istú
stránku pod `/basic/`: z `https://<adresa>/settings/network` sa stane
`https://<adresa>/basic/settings/network`. Zariadenie tieto stránky kreslí
samo a posiela ich ako hotové HTML, s tým istým bočným panelom, hornou
lištou, farbami a písmom ako obvyklé stránky.

Hodí sa to, keď prehliadač blokuje skripty, napríklad cez NoScript alebo
firemné pravidlá, alebo keď sa obvyklé stránky nenačítajú.

![Prehľad s vypnutým JavaScriptom: bočný panel, meno zariadenia a dlaždice, tak ako ich kreslí obvyklá stránka.](@site/docs/img/without-javascript-sk.png)

## Prihlásenie \{#signing-in}

Prihláste sa menom a heslom, ako v LosOS Login. Ak váš účet pýta kód z
overovacej aplikácie, pýta si ho ďalšia stránka. Prístupový kľúč (passkey)
potrebuje JavaScript, takže účet, ktorý sa prihlasuje ním, sa tu neprihlási.
Použite namiesto toho náhradný správcovský kľúč. Prehliadač si ho drží, kým
ho nezatvoríte.

Tieto stránky idú iba cez https, ako LosOS Login. Adresa `http://` sa
presmeruje na `https://`.

## Čo funguje \{#what-works}

- **Prehľad.** Meno zariadenia, či odpovedá, dlaždice a ako je plný disk.
- **Úložisko.** Koľko miesta je obsadeného a voľného a použitie rezervy,
  ktoré sa najprv opýta.
- **Sieť.** Meno zariadenia, dostupnosť zvonka domácej siete a vaše vlastné
  domény: pridajte doménu, pozrite si jej dva záznamy a odstráňte ju.
- **Hardvér** a **Zabezpečenie.** Každý prepínač na týchto paneloch.
- **Hľadanie.** Nájde stránku zariadenia podľa názvu.
- **Vzhľad.** Vaše vlastné pozadie, bez obrázka alebo s obrázkom, ktorý
  zariadenie dodáva, a závoj nad ním. Vlastný obrázok sa nahráva so
  zapnutým JavaScriptom.
- **Téma, jazyk a bočný panel.** Svetlý, tmavý alebo podľa prehliadača a
  jazyk, vo vašej ponuke vpravo hore, a bočný panel zbalený na ikony.
  Prehliadač si to pamätá.

Zmena, ktorú urobíte, sa ukáže v lište Použiť na spodku panela, ktorá zmeny
počíta. **Použiť** zariadenie prestavia ako na obvyklých stránkach a stránka
sa každých pár sekúnd sama načíta znova, kým prestavba neskončí, a potom
povie, či vyšla. **Zahodiť** vráti panel späť.

Stránky, ktoré tu ešte nie sú, napríklad Aplikácie, Mesh či LosOS Cloud,
povedia, že potrebujú JavaScript, a nič nemenia.

## Čo nefunguje \{#what-does-not-work}

- Nástenka s widgetmi na Prehľade. Jej widgety žijú v prehliadači a
  potrebujú skript.
- Čokoľvek, čo sa mení pred očami, okrem prestavby. Načítajte stránku znova,
  aby ste videli, čo sa zmenilo.

## Prečo stránka vyzerá rovnako \{#why-the-page-looks-the-same}

Stránky bez JavaScriptu používajú štýly a texty obvyklých stránok. Ponuky,
zbalenie bočného panela a dialóg, ktorý sa pýta pred použitím rezervy, sa
otvárajú a zatvárajú len cez CSS. Oznámenia sa vysunú a samy odídu, tiež cez
CSS, a ostanú stáť, ak váš systém žiada menej pohybu.

## Táto príručka \{#this-handbook}

Príručka sa dá čítať aj bez JavaScriptu, na boxe aj na webe. Na telefóne
tlačidlo ponuky otvorí kapitoly ako panel. Kapitola v bočnom paneli otvorí
svoju stránku a **Na tejto stránke**, ponuka jazykov a voľba Svetlý, Tmavý
alebo podľa prehliadača fungujú. Zvolený vzhľad platí len pre stránku, na
ktorej ste ho zvolili. Vyhľadávanie potrebuje JavaScript, takže pole
vyhľadávania namiesto toho otvorí zoznam všetkých stránok, ktorý prehľadá
funkcia Hľadať vo vašom prehliadači.
