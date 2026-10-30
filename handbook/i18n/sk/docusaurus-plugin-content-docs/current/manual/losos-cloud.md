---
title: LosOS Cloud
sidebar_position: 4.4
slug: /manual/losos-cloud
---

# LosOS Cloud

![Súbory v LosOS Cloud: priečinok Documents, vľavo miesta.](@site/docs/img/cloud-files-sk.png)

LosOS Cloud je vlastná súborová služba zariadenia: vaše súbory v
prehliadači, sieťový priečinok pre správcu súborov, server, s ktorým sa
synchronizujú aplikácie Nextcloud pre počítač a telefón, a vaše kalendáre a
kontakty pre akúkoľvek kalendárovú aplikáciu.

Otvorte ho na `https://<name>.local/cloud/`. `<name>` je meno zariadenia,
také ako na jeho obrazovke. Prihlásite sa cez LosOS Login a každý, kto sa
môže prihlásiť, má vlastné súbory. Kto zariadenie nastavil, dostane súbory
vlastníka.

## V prehliadači \{#in-the-browser}

LosOS Cloud je v tom istom rámci ako ostatné stránky zariadenia: rovnaký
pruh hore aj rovnaký bočný panel, jeho miesta sú pod položkou **Cloud**.

- **Súbory** zobrazujú vaše priečinky. Súbory alebo celé priečinky
  nahráte tak, že ich pustíte na stránku, alebo cez **Nahrať**. Súbor
  pridaný inak, cez sieťové zdieľanie alebo z aplikácie v počítači, sa v
  otvorenom priečinku ukáže sám do niekoľkých sekúnd. Zaškrtnuté
  súbory môžete stiahnuť, premenovať, presunúť, skopírovať či vymazať
  alebo si pozrieť ich staršie verzie.
- **Nedávne** ukazujú, čo sa naposledy zmenilo, **Obľúbené** to, čo ste
  označili hviezdičkou.
- **Fotky** ukazujú všetky obrázky podľa toho, kedy boli odfotené, a ľudí
  na nich. Pozrite [Fotky](#photos).
- **Kôš** drží vymazané súbory 30 dní alebo tak dlho, ako určí panel
  **Aplikácie**. Súbor z neho obnovíte alebo ho vymažete natrvalo. Čo
  vymažete cez [sieťové zdieľanie](files-on-your-computer#the-network-share),
  doň nepríde.
- **Zariadenia** zobrazujú počítače a telefóny, ktoré sa synchronizujú, a
  vedia ktorékoľvek odhlásiť.
- <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>P</kbd>, dvojbodka napísaná mimo
  textového poľa alebo lupa hore v bočnom paneli otvorí vyhľadávanie, ktoré
  nájde súbory podľa mena aj podľa textu v nich, na každej stránke
  zariadenia aj v otvorenom dokumente.
- Textové súbory sa otvárajú v editore, do ktorého môže písať viac ľudí
  naraz, a **Zdieľané s vami** drží to, čo s vami zdieľali ľudia na iných
  zariadeniach. Pozrite
  [Zdieľanie a spoločné úpravy](sharing-and-editing-together).
- **Poznámky** čítajú priečinok poznámok Markdown tak ako Obsidian, s
  odkazmi, štítkami a grafom, a môžu ho zverejniť ako znalostnú bázu.
  Pozrite [Poznámky a znalostné bázy](notes-and-knowledge-bases).
- Kliknite pravým tlačidlom na súbor, priečinok alebo miesto okolo nich a
  uvidíte, čo s nimi môžete urobiť; pozrite
  [ponuku pravého tlačidla](admin-pages#the-right-click-menu).
- Ponuka pravého tlačidla na súbore ho podá aj ďalším aplikáciám: **Poslať
  e-mailom** a **Poslať v chate**, kým má zariadenie Mail a Chat, a
  **Opýtať sa AI na toto** pri textovom súbore na zariadení s jazykovým
  modelom. AI beží na zariadení a číta len súbor, ktorý ste vybrali.

## Fotky \{#photos}

**Fotky** ukazujú každý obrázok z ľubovoľného priečinka, najnovšie navrchu,
podľa mesiaca, v ktorom ho fotoaparát odfotil. **Zoskupiť podľa** ich
namiesto toho zoradí podľa osoby, fotoaparátu alebo miesta. Kliknutím
obrázok zväčšíte, šípkami prechádzate ďalšie a **Podrobnosti** (alebo
<kbd>i</kbd>) ukážu, čo fotoaparát zapísal: kedy, aký fotoaparát
a objektív, expozíciu, veľkosť, kde bol obrázok odfotený, kto je na ňom
a každý ďalší údaj, ktorý fotoaparát uložil.

![Fotky v LosOS Cloud: všetky obrázky podľa mesiacov.](@site/docs/img/cloud-photos-sk.png)

Zariadenie nájde na vašich obrázkoch tváre a tie, ktoré sa podobajú, spojí
do jednej osoby. **Ľudia** nad fotkami ukazujú každého, kto je na viac ako
jednom obrázku; kliknutím na tvár uvidíte všetky fotky tejto osoby a môžete
jej dať meno. Ak dvom osobám dáte rovnaké meno, stanú sa jednou.

Všetko sa deje na zariadení, na pozadí a s najnižšou prioritou, pár minút
po tom, ako obrázok príde: dva malé modely hľadajú a porovnávajú tváre na
procesore zariadenia a nič o obrázku, tvári ani mieste sa nikam neposiela.
Tváre, mená a miesta vašich fotiek vidíte len vy; ostatní ľudia na
zariadení z nich nevidia nič.

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

![Stránka, ktorú otvorí aplikácia v počítači pri prihlásení: Pripojiť aplikáciu, s tlačidlami Povoliť a Teraz nie.](@site/docs/img/cloud-connect-sk.png)

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

V prehliadači sú tie isté kalendáre v [Kalendári](calendar) a adresáre
v [Kontaktoch](contacts).

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
**Rozšírené** LosOS Cloud vypne a `losos.cloud.photos.faces` zastaví
hľadanie tvárí vo Fotkách: ďalší prechod vymaže všetky doteraz nájdené
tváre a mená, údaje fotoaparátu zostanú.
