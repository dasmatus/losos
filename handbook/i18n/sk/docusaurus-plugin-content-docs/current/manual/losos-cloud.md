---
title: LosOS Cloud
sidebar_position: 4.4
slug: /manual/losos-cloud
---

# LosOS Cloud

LosOS Cloud je vlastná súborová služba zariadenia: vaše súbory v
prehliadači, sieťový priečinok pre správcu súborov, server, s ktorým sa
synchronizujú aplikácie Nextcloud pre počítač a telefón, a vaše kalendáre a
kontakty pre akúkoľvek kalendárovú aplikáciu.

Otvorte ho na `https://<name>.local/cloud/`. `<name>` je meno zariadenia,
také ako na jeho obrazovke. Prihlásite sa cez LosOS Login a každý, kto sa
môže prihlásiť, má vlastné súbory. Kto zariadenie nastavil, dostane súbory
vlastníka.

## V prehliadači \{#in-the-browser}

- **Súbory** zobrazujú vaše priečinky. Súbory alebo celé priečinky
  nahráte tak, že ich pustíte na stránku, alebo cez **Nahrať**. Zaškrtnuté
  súbory môžete stiahnuť, premenovať, presunúť, skopírovať či vymazať
  alebo si pozrieť ich staršie verzie.
- **Nedávne** ukazujú, čo sa naposledy zmenilo, **Obľúbené** to, čo ste
  označili hviezdičkou.
- **Fotky** ukazujú všetky obrázky, najnovší mesiac navrchu. Kliknutím
  obrázok zväčšíte a šípkami prechádzate ďalšie.
- **Kôš** drží vymazané súbory 30 dní alebo tak dlho, ako určí panel
  **Aplikácie**. Súbor z neho obnovíte alebo ho vymažete natrvalo.
- **Zariadenia** zobrazujú počítače a telefóny, ktoré sa synchronizujú, a
  vedia ktorékoľvek odhlásiť.
- Vyhľadávanie hore nájde súbory podľa mena.
- Textové súbory sa otvárajú v editore, do ktorého môže písať viac ľudí
  naraz, a **Zdieľané s vami** drží to, čo s vami zdieľali ľudia na iných
  zariadeniach. Pozrite
  [Zdieľanie a spoločné úpravy](sharing-and-editing-together).
- **Poznámky** čítajú priečinok poznámok Markdown tak ako Obsidian, s
  odkazmi, štítkami a grafom, a môžu ho zverejniť ako znalostnú bázu.
  Pozrite [Poznámky a znalostné bázy](notes-and-knowledge-bases).

## Aplikácie pre počítač a telefón \{#desktop-and-phone-apps}

Klient Nextcloud pre počítač a aplikácie Nextcloud pre Android a iOS s
LosOS Cloud fungujú. Zadajte im adresu zariadenia bez cesty:

```text
https://<name>.local
```

Aplikácia otvorí LosOS Login v prehliadači. Prihláste sa, na ďalšej
stránke stlačte **Povoliť** a aplikácia dostane vlastné heslo. Potom sa
objaví v **Zariadeniach**, kde ju odhlásite bez toho, aby ste menili heslo
do LosOS Login.

## Kalendáre a kontakty \{#calendars-and-contacts}

Kalendárové a kontaktové aplikácie (Thunderbird, Kalendár v GNOME, DAVx⁵ v
Androide, účty v iPhone) nájdu vaše kalendáre a adresáre z adresy
zariadenia:

```text
https://<name>.local
```

Prihláste sa menom do LosOS Login a heslom aplikácie z prihlásenia
aplikácie pre počítač alebo telefón, alebo v sieti zariadenia heslom do
LosOS Login. Kalendárová aplikácia, ktorá chce celú adresu, dostane
`https://<name>.local/remote.php/dav/`.

## V správcovi súborov \{#in-the-file-manager}

Tie isté súbory sú zdieľaním WebDAV na tejto adrese:

```text
https://<name>.local/remote.php/webdav/
```

V sieti zariadenia ho GNOME Files a Dolphin ukážu v sieti ako **LosOS
Cloud on `<name>`**. Windows a macOS ho pridajú podľa adresy, ako v
[Súboroch vo vašom počítači](files-on-your-computer). Najprv nainštalujte certifikát zariadenia, ako
v [prvom spustení, kroku 1](../start/first-run#step-1-trust-this-box).

V sieti zariadenia zdieľanie prijme vaše meno a heslo do LosOS Login. Cez
internet prijme len vlastné heslo aplikácie, preto tam použite aplikáciu
pre počítač alebo telefón.

## Zariadenie, na ktorom bežal Nextcloud \{#a-box-that-ran-nextcloud}

Pred LosOS Cloud zariadenie držalo vaše súbory v Nextcloude. Pri prvom
spustení bez neho všetko presunie samo, skôr než sa LosOS Cloud otvorí:
súbory každého účtu s ich košom, kalendáre, kontakty a úlohy, tabule z Deck
a heslá, ktoré už mali aplikácie pre počítač a telefón, takže sa
synchronizujú ďalej bez nového prihlásenia. Aplikácie nastavené so starou
adresou končiacou na `/nextcloud` fungujú tiež.

Súbory sa presúvajú, nekopírujú, takže presun nepotrebuje miesto navyše. To,
čo Nextcloud držal popri nich, zostane, kde bolo; vymaže to iba
[vymazanie zariadenia](backup-and-erase) spolu so všetkým ostatným. Ak sa presun nepodarí, LosOS Cloud sa neotvorí nad prázdnym
priečinkom: zariadenie to skúsi znova pri ďalšom spustení.

## Nastavenia \{#settings}

Časť LosOS Cloud na paneli **Aplikácie** určuje, ako dlho kôš drží súbory
(`losos.cloud.trashDays`, 1 až 365 dní). `losos.cloud.enable` na paneli
**Rozšírené** LosOS Cloud vypne.
