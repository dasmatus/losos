---
title: Správa
sidebar_position: 2
mdx:
  format: md
---

# Správa

## Administrátorské rozhranie {#admin-ui}

Z LAN otvorte `http://<ip>/` (adresu z bannera na tty1 zariadenia) alebo
`https://<host>.local/`. Administrátorské rozhranie je zablokované pre všetko
mimo LAN vrátane prevádzky cez [master proxy](/in-depth/master-proxy.md).
`<host>.local` je mDNS meno. Počítač, ktorý ho nevie preložiť, typicky
hostiteľ VM za NAT, použije IP adresu a každá cesta nižšie na nej odpovedá
rovnako.

Služby na tom istom hostiteľovi:

| Služba               | URL                       |
| -------------------- | ------------------------- |
| Admin UI             | `<host>.local/`           |
| LosOS Cloud          | `<host>.local/cloud/`     |
| LosOS Git            | `<host>.local/git/`       |
| Príručka             | `<host>.local/handbook/`  |
| [Diagramy](/in-depth/diagrams.md) | `<host>.local/diagrams/` |

IP adresa zariadenia (tá z bannera na tty1) a jeho holé meno fungujú všade
namiesto `<host>.local`, LosOS Cloud nevynímajúc. Tak sa dostanete dnu z
libvirt VM, ktorá mDNS meno nedostane. LosOS Cloud nevedie zoznam dôveryhodných
mien, takže odpovie na tej adrese, na ktorú požiadavka prišla.

`losos.tls.enable` (predvolene zapnuté) pridáva HTTPS na :443 s certifikátom,
ktorý si zariadenie vygeneruje samo, s platnosťou dva roky. Obyčajné HTTP na
:80 zostáva otvorené. Prehliadače certifikátu nedôverujú, kým ho
nenainštalujete. Sprievodca prvým spustením ponúka jednoriadkový inštalátor
pre macOS, Linux a Windows (`/setup/trust.sh`, `/setup/trust.ps1`, oba len z
LAN) a obyčajné stiahnutie (`/setup/losos-ca.crt`). Pozri [Inštalácia podrobne](/in-depth/install.md).

## Vzhľad {#look}

**Nastavenia, Vzhľad** mení vzhľad administrátorských stránok. Každý
prihlásený ho otvorí kvôli vlastnému pozadiu; widgety na ňom sú pre ľudí v
sekcii vzhľadu. Nič z toho nie je nastavenie v zmysle Nixu. Widgety drží
lososd v `/var/lib/losos/look.json` a pozadie každého človeka v
`/var/lib/losos/looks/<meno>.json`, takže zmena platí v okamihu uloženia,
bez prestavby.

### Pozadie {#background}

Každý si vyberá vlastné a sprevádza ho v každom prehliadači, kde sa
prihlási; voľba nikoho iného jeho stránku nezmení. Vyberte jeden z troch
dodaných obrázkov alebo nahrajte vlastný (PNG, JPEG, WebP alebo GIF, do
8 MiB). Posuvník **Závoj** kladie farbu stránky cez obrázok, od 20 % do
90 %, aby text zostal čitateľný vo svetlej aj tmavej téme. *Bez obrázka*
obrázok opäť odstráni. LosOS Cloud a LosOS Git ho ukazujú za svojimi
stránkami tiež.

Náhradný kľúč nie je nikto, preto vidí a nastavuje vlastné pozadie
zariadenia. Na zariadení, ktoré malo pozadie ešte predtým, ako sa pozadia
stali osobnými, je to pozadie zariadenia a vlastník dostane jeho kópiu pri
prvom prihlásení. Odstránením človeka sa odstráni aj jeho pozadie.

:::note[Kto môže obrázok čítať]
Nahraný obrázok človeka servíruje lososd na `/api/look/background/<meno>`
len tomuto človeku, čítaný s jeho súborom cookie LosOS Login. Vlastný
obrázok zariadenia je na `/api/look/background` bez tokenu, lebo karta s
náhradným kľúčom drží kľúč v stránke a CSS `background-image` ho poslať
nevie. Obe cesty sú za tou istou ochranou „len LAN“ ako zvyšok
administrátorských stránok.
:::

### Ručne písané widgety {#widgets-written-by-hand}

Galéria nástenky na Prehľade (**Pridať widget**) má dva spôsoby, ako si urobiť
vlastný. *Zostaviť* poskladá widget z hodnôt zariadenia bez akéhokoľvek
kódu. *Napísať* prijme vlastné HTML, štýl a skript, ktoré sa držia na
zariadení a sú vypísané na tomto paneli, kde ich môžete upraviť a zmazať.

Widget tvorí niekoľko súborov. Začína sa v `index.html`, ktorý ostatné
prepája menom, napríklad `<link rel="stylesheet" href="style.css">`,
`<script src="app.js"></script>` alebo `<img src="dot.svg">`, a
`fetch("data.json")` prečíta jeden z vlastných súborov. Nový widget začína
súbormi `index.html`, `style.css` a `app.js`; **Pridať súbor** pridá ďalší
(`.html`, `.css`, `.js`, `.json`, `.svg`, `.txt` alebo `.md`). Skripty sú
klasické: viac ich načítate viacerými značkami `<script src>` v poradí, nie
cez `import` medzi nimi.

Editor počas písania dopĺňa: objekt `losos` a jeho členy, mená hodnôt vnútri
`losos.metric("`, farebné premenné po `var(--` a mená súborov widgetu všade,
kde sa súbor menuje. Ctrl+Medzerník ponúkne návrhy kdekoľvek.

Ručne písaný widget beží v sandboxovanom rámci, vo vlastnom origine bez
prístupu k administrátorským stránkam, admin tokenu či API. So zariadením
komunikuje cez malý objekt `losos`, ktorý rámec poskytuje:

| Volanie                        | Čo poskytuje                                                         |
| ------------------------------ | -------------------------------------------------------------------- |
| `losos.metric(name)`           | Promise jednej z hodnôt zariadenia, s tými istými menami, aké používajú vstavané widgety galérie (`storage.bytes`, `box.settings`, …). |
| `losos.theme`, `losos.lang`    | `light`/`dark` a jazyk, v ktorom je administrátorská stránka.        |
| `losos.palette`                | Farby zariadenia, nastavené aj ako CSS premenné (`var(--accent)` funguje). |
| `losos.onTheme(fn)`            | Volá sa vždy, keď vlastník prepne tému.                               |
| `losos.resize()`               | Požiada nástenku, aby dlaždicu znovu premerala po zmene, ktorú nevidí. |
| `losos.files`                  | Mená súborov widgetu.                                                 |
| `losos.file(name)`             | Jeden zo súborov widgetu ako text.                                    |
| `losos.asset(name)`            | Jeden zo súborov widgetu ako `data:` URL, pre obrázok nastavený zo skriptu. |

Karta **Ako to funguje** v editore to zopakuje s príkladom. Jej náhľad je skutočný
rámec, takže to, čo ukazuje, ukáže aj dlaždica. Widget môže sťahovať z
internetu (napríklad dlaždica s počasím), ale nie zo zariadenia. Limit je
24 widgetov, každý najviac 12 súborov a spolu 128 KiB.

Editor má aj kartu **Vytvoriť s AI**, kde jazykový model na zariadení
napíše widget podľa opisu a vloží jeho súbory do editora, aby ste si ich pred
uložením prečítali. Beží na zariadení, takže to, čo napíšete, zostane na ňom
a tvorba nič nestojí; pozri [Vytvoriť s AI](widget-builder.md).

## Admin token {#admin-token}

Zariadenie pri prvom štarte vytvorí náhodný token so 64 hexadecimálnymi
znakmi a uchová ho zapečatený vo svojom úložisku prihlasovacích údajov ako
`admin-token` (pozrite [Prihlasovacie údaje](../reference/credentials.md));
`lososd` ho číta zo svojho vlastného adresára prihlasovacích údajov. Token
môže zmeniť akékoľvek nastavenie, takže je to root.

Na novom, neprivlastnenom zariadení prvý volajúci z LAN, ktorý pošle
`POST /api/setup/claim`, nastaví heslo vlastníka, stane sa prvým človekom,
ktorý sa môže prihlásiť (správcom), a v odpovedi dostane token.
Táto cesta nevyžaduje token, ale prestane prijímať nároky, len čo má
zariadenie vlastníka. Zariadenie nastavujte iba v dôveryhodnej LAN, pretože
prvý volajúci sa stane jeho vlastníkom. Token sa vydá iba raz a cez UI sa už
znovu získať nedá.

Pred prvým heslom sa nemusí nič spustiť, takže `GET /api/setup/claim` na
neprivlastnenom zariadení odpovie `ready: true` a sprievodca prvým spustením
pokračuje hneď. Požiadavka, ktorá potrebuje službu ešte na jej prvom štarte
(napríklad LosOS Git), odpovie `503` s `{"error": …, "ready": false,
"waitingFor": …}` a `Retry-After` a nič nezmení.

Tým volajúcim je sprievodca prvým spustením. Po ňom sa ľudia prihlasujú v
LosOS Login, prihlasovacej stránke zariadenia na
`https://<zariadenie>/auth/`, každý vlastným prihlasovacím menom a heslom.
LosOS Login je open-source prihlasovací server, ktorý beží na zariadení.
lososd vedie súbor ľudí, ktorý LosOS Login číta, s každým heslom ako hashom
argon2id, a LosOS Login jeho zmenu prevezme hneď. nginx sa LosOS Login pýta
na každú požiadavku na `/api/` a meno človeka odovzdá lososd, ktorý otvorí
časti, ktoré povoľujú jeho skupiny. Prvý človek je vždy správca. Vlastné
heslo si človek zmení cez `POST /api/set-password` a vlastníkovi to zmení aj
heslo do LosOS Cloud. Všetkých ostatných pridáva, mení a odstraňuje správca
na paneli Používatelia cez `GET` a `POST /api/people` a
`POST /api/people/{access,password,remove}`. Panel je tam iba vtedy, keď
zariadenie prihlasuje ľudí samo (`standalone` v `GET /api/me`). lososd
odmietne siahnuť na vlastníka, na vlastný účet volajúceho aj na posledného
správcu. LosOS Login si človeka vyhľadá znova pri každej požiadavke, takže
odstránenie alebo zmenená skupina sa do aplikácií dostane tiež pri ďalšom
kliknutí. Prihlásenia sú vo vlastnom Redise LosOS Login, na sokete, ktorý
otvorí iba on, takže reštart LosOS Login ani nočný reštart nikoho
neodhlási.

LosOS Cloud a LosOS Git prihlasujú ľudí rovnako. nginx sa spýta LosOS Login
a pošle človeka ďalej v troch hlavičkách, `X-Losos-Cloud-User`, `-Name` a
`-Email`, ktoré nastaví pri každej požiadavke na niektorú z aplikácií, takže
žiadny klient nepošle vlastné. Vlastník, v skupine `losos-owner`, príde do
oboch ako účet `notshared`; ktokoľvek iný pod svojím prihlasovacím menom.
LosOS Cloud (`backend/src/cloud/auth.rs`) aj LosOS Git
(`backend/src/forge/auth.rs`) čítajú hlavičky samy a každá pri prvej
návšteve vytvorí účet; lososd potom drží meno a adresu v oboch aplikáciách
v súlade so súborom ľudí. Na požiadavku do LosOS Git, ktorá prinesie vlastný
token v `Authorization` (git, API), sa nginx nepýta. Človeka, ktorého LosOS Login
nepozná, pošle nginx tam, z LAN aj cez okrajový server; žiadna aplikácia
neukazuje vlastnú prihlasovaciu stránku. Cez okrajový server tunel doručuje
z loopbacku s `X-Forwarded-Proto: https`, čomu nginx verí iba z loopbacku, a
lososd pridá každé verejné meno, na ktorom zariadenie odpovedá, medzi domény
cookies LosOS Login. Kým sa LosOS Login spúšťa, aplikácie odpovedajú 503 so
stránkou, ktorá to skúša znova sama. Obe aplikácie čítajú hlavičky iba na
svojich unixových socketoch, `/run/losos-cloud/http.sock` a
`/run/losos-git/http.sock`, ktoré smie otvoriť iba root a nginx; port LosOS
Git na loopbacku, `127.0.0.1:3000`, berie tokeny a hlavičky nikdy. Takže nič
na zariadení nemôže ani jednej aplikácii pomenovať človeka. Mená
`notshared`, `shared` a `losos` sú vyhradené pre vlastné účty zariadenia.

Každý človek so skupinou `losos-account` (Vlastný účet na paneli
Používatelia) si spravuje vlastné meno, e-mailovú adresu a heslo cez `GET` a
`POST /api/account`. Overovaciu aplikáciu a prístupové kľúče drží LosOS Login
vo vlastnej databáze; ich pridanie alebo odstránenie si pýta jednorazový
kód, ktorý by inak poslal e-mailom, preto ho jeho notifikátor zapíše do
súboru a `POST /api/account/code` ho vráti za heslom daného človeka. Správca
nastavuje dve pravidlá cez `POST /api/people/rules`: dvojstupňové
prihlásenie pre všetkých a heslá, ktoré po určitom počte dní vypršia. lososd
označí súbor ľudí dvoma skupinami: `losos-two-factor` pre každého s druhým
faktorom, ktorý si LosOS Login pýta pri každom prihlásení, a `losos-locked`
pre každého, koho zachytí pravidlo. LosOS Login takého človeka nepustí do
LosOS Cloud ani LosOS Git a lososd mu odpovedá iba na jeho účet (`locked` v
`GET /api/me`). lososd odmietne pravidlo dvojstupňového prihlásenia, kým ho
nastavujúci správca nemá druhý faktor, a náhradný kľúč nie je zadržaný
nikdy. `POST /api/people/two-factor/reset` odstráni druhé faktory človeka.

Samotný token sa po nastavení hesla stále raz zobrazí ako **náhradný
správcovský kľúč**, s tlačidlami Kopírovať a Tlačiť. Požiadavka, ktorá ho
nesie, obíde LosOS Login a lososd kľúč overí sám, takže kľúč otvorí
administrátorské stránky aj vtedy, keď LosOS Login nebeží. Prihlasovací
dialóg má odkaz na jeho zadanie. Zariadenie nastavené predtým, než mali
ľudia prihlasovacie mená, si kľúč raz vypýta spolu s prihlasovacím menom a
heslom, aby vytvorilo svojho prvého správcu.

Nové heslo musí mať aspoň 12 znakov s malým písmenom, veľkým písmenom,
číslicou a symbolom, napríklad `-` alebo `!`. Sprievodca počas písania
ukazuje štyri pravidlá, každé s fajkou.

:::warning[Stratené heslo aj náhradný kľúč]
Token sa na zariadení nedá vymeniť, pretože nemá shell. Strata hesla *aj*
náhradného kľúča znamená preinštalovanie z inštalačného média, ktoré vymaže
disk.
:::

## Nastavenia {#settings}

Nastavenia sú Nix. Stránka nastavení vygeneruje `modules/overrides.nix` a
spustí `nixos-rebuild switch`. Teda:

- Zmena si vyžaduje prestavbu, nie reštart.
- Zariadenie sa dá prestavať z tohto repozitára a toho jedného súboru.
- `apply` nahradí `overrides.nix` celý. Nastavenie, ktoré UI nezapíše, sa
  pri ďalšom uložení vráti na predvolené.

### Rozšírené {#advanced}

**Nastavenia, Rozšírené** vypisuje každú voľbu `losos.*`, ktorú zariadenie
deklaruje, načítanú z modulov, ktoré beží. `nix/options-doc.nix` vygeneruje
dokument pri zostavení a lososd ho servíruje na `GET /api/options` s
pripojeným aktuálnym `overrides.nix`. Každý riadok ukazuje popis voľby, jej
predvolenú hodnotu, hodnotu, s ktorou zariadenie beží, a editor pre jej typ:
prepínač, číslo s hranicami, výber, textové pole alebo jednu položku na
riadok pre zoznam.

- Voľba, ktorú už upravuje jeden z ostatných panelov (meno, nastavenia
  mesh, prepínače hardeningu), sa zobrazí so svojou hodnotou a odkazom
  na ten panel, aby bolo jedno miesto, kde ju zmeniť.
- Tri voľby, ktoré zapisuje inštalátor (`targetDrives`, `tpm.enable`,
  `bios`), a balíky, ktoré vyberá zostavenie, sú len na čítanie. Riadok pre
  niektorú z nich v `overrides.nix` by zhodil ďalšiu prestavbu.
- Voľby označené **Opatrne** (`admin.*`, `cluster.*`, `hostName`,
  `storage.*`, `tls.*`, `upgradeFlakeUri`, …) sa pred prvou zmenou raz
  opýtajú. Nesprávna hodnota zanechá zariadenie bez shellu, z ktorého by sa
  to dalo opraviť.
- **Použiť predvolené** riadok odstráni namiesto zapísania predvolenej hodnoty,
  takže predvolenú hodnotu, ktorá sa s aktualizáciou posunie, zariadenie
  nasleduje.
- Riadok v `overrides.nix`, pre ktorý zariadenie nemá voľbu (preklep,
  nastavenie z inej verzie), sa zobrazí vo vlastnej skupine a blokuje Použiť,
  kým sa neodstráni. Takéto telo odmietne aj lososd (`backend/src/options.rs`,
  `check_body`). Kontroluje každý riadok apply voči dokumentu: deklarovaný,
  nie len na čítanie, hodnota správneho druhu, žiadne `${`.

Všetko, čo sa tu zapíše, ide cez to isté Použiť a tú istú prestavbu ako
ostatné panely. `losos.edge.*` je vynechané, pretože ho zariadenie nikdy
nečíta.

### História a LosOS Git {#history-and-losos-git}

Každé Použiť, zmena úložiska a obnovenie továrenských nastavení je jeden
commit v `/etc/nixos`, repozitári, ktorý vytvoril `losos-ctl install`. Commit
je pomenovaný podľa nastavení, ktoré zmenil, `Change hostName, cluster.enable`,
s hodnotou pred a po každého z nich v tele. **Nastavenia,
História** vypisuje posledných štyridsať.

S `losos.configRepo.enable` (predvolené) a zapnutým LosOS Git drží lososd
konfiguráciu aj v súkromnom repozitári na LosOS Git, `<owner>/losos-config`,
ktorý vlastní administrátorov vlastný účet (`notshared`). Účet sa vytvorí
pri prvej synchronizácii ako správca LosOS Git. Reconciler v lososd beží
každých 30 s:

- commitne všetko necommitnuté a pushne vetvu zariadenia, keď LosOS Git
  zaostáva;
- keď je LosOS Git napred, lebo ste repozitár naklonovali, upravili
  `modules/overrides.nix` a pushli, posunie sa naň fast-forwardom, skontroluje
  nové `overrides.nix` riadok po riadku tak, ako sa kontroluje Použiť, a
  spustí prestavbu. Odmietnutý push sa ohlási v Histórii a zariadenie zostane
  na vlastnom commite;
- keď sa obe rozišli (prepísaná história), neurobí nič a povie to. Vyriešte
  to z klonu;
- kým beží prestavba, push čaká na ďalší tik.

**Synchronizovať teraz** na paneli História spustí jeden tik okamžite. `losos-ctl
config` vypíše ten istý dokument a `losos-ctl config --sync` spustí tik.
Tajomstvá sa do repozitára nikdy nedostanú. `overrides.nix` nesie iba
hodnoty volieb a token, ktorým sa lososd autentifikuje voči LosOS Git, je
admin token, ktorý LosOS Git zapíše pri prvom štarte,
`/var/lib/losos-git/admin-token`, mimo `/etc/nixos`. Nestojí za tým žiadny
CI runner. Zariadenie sa periodicky dotazuje.

Adresa na klonovanie je zobrazená na paneli (`http://<box>/git/<owner>/
losos-config.git`). Repozitár je súkromný, takže počítač potrebuje vlastný
token (`losos git setup`, [Terminálový klient](../manual/terminal)). lososd
komunikuje s LosOS Git cez jeho port na loopbacku s admin tokenom a tam,
kde ide o volanie vlastníka, koná za neho s `X-Losos-As`.

### Prechod z Forgejo {#moving-from-forgejo}

Zariadenie, na ktorom bežal Forgejo, prenesie jeho záznamy do LosOS Git
samo, pri prvom štarte po aktualizácii a skôr, ako LosOS Git odpovie:
repozitáre, účty, spolupracovníkov, prístupové tokeny (staré hodnoty
fungujú ďalej pre git aj API), štítky, issues, pull requesty, komentáre,
priradených ľudí a prílohy. Z organizácie sa stane účet s rovnakým menom.

Správa je `/var/lib/losos-git/forgejo/import.json`. Počíta, čo mal Forgejo
a čo prešlo, tabuľku po tabuľke, a pomenuje, čo neprešlo, napríklad
míľniky, vydania a záznamy časovej osi. Vedľa nej zostáva úplný dump
databázy Forgejo (`forgejo.sql`, na natívnom zariadení `forgejo.db`) a
priečinok Forgejo aj dáta Postgresu zostávajú, kde boli. Ak sa prechod
nepodarí, nič sa nezmení a skúsi sa znova pri ďalšom štarte; LosOS Git
medzitým repozitáre obsluhuje.

### Nastavenia aplikácií pre celé zariadenie {#box-wide-app-settings}

Panel Aplikácie nesie nastavenia, ktoré platia pre všetkých v LosOS Cloud a
LosOS Git, teda tie, ktoré si inak každá aplikácia drží na vlastných
stránkach správy. Každé pole je voľba, `losos.cloud.trashDays` alebo jedna
pod `losos.git.site.*`, takže ide cez Použiť, bránu, Históriu a repozitár
konfigurácie ako každé iné nastavenie. Panel Rozšírené ukazuje ich hodnoty a
odkazuje na sekciu aplikácie na paneli Aplikácie (`/apps#cloud`,
`/apps#git`), namiesto toho aby kreslil druhý editor (`ON_PANE` v
`admin-ui/app/src/lib/option-value.ts`).

| Voľba | Čo nastaví v aplikácii |
| --- | --- |
| `losos.cloud.trashDays` | koľko dní zostane zmazaný súbor v koši (1 až 365, predvolene 30) |
| `losos.git.site.requireSignIn` | `losos-git serve --require-sign-in` |
| `losos.git.site.defaultPrivate` | `--public-by-default` pri `"public"` |
| `losos.git.site.pushCreate` | `--push-create` |

`modules/cloud.nix` odovzdá `trashDays` službe ako `--trash-days` a
hodinové upratovanie služby natrvalo zmaže všetko, čo leží v niečom koši
dlhšie.

Tri nastavenia LosOS Git sú prepínače na jeho príkazovom riadku
(`modules/git.nix`), takže vnútri neho nie je stránka, kde by sa dali
zmeniť.

:::note[Čo zostáva v aplikáciách]
Všetko, čo si nastaví účet sám pre seba, zostáva v aplikácii. V LosOS Git
je účet vlastníka správcom. LosOS Cloud v sebe nemá vôbec žiadneho správcu:
kto ho smie používať, rozhoduje LosOS Login, a všetko pre celé zariadenie je
nastavenie na paneli Aplikácie.
:::

## Záloha, obnova a vymazanie {#backup-restore-and-erase}

**Nastavenia, Záloha** posiela dáta zariadenia do S3-kompatibilného bucketu,
ktorý si vlastník prenajíma (Amazon S3 vrátane Glacier, Backblaze B2, Wasabi,
Cloudflare R2, MinIO).
[restic](https://restic.net) robí kopírovanie a šifruje na zariadení skôr,
než čokoľvek odíde, takže bucket drží šifrovaný text. Heslo repozitára je
kód na obnovu zariadenia (`/var/secrets/losos-recovery-code`), ktorý panel na
požiadanie ukáže; bez neho nikto zálohu neotvorí.

Záloha obsahuje:

- `/var/lib/losos-cloud` a `/var/lib/losos-git`, pričom databázy SQLite
  LosOS Cloud a LosOS Git najprv skopíruje vlastná záloha SQLite, aby ich
  zápis počas kopírovania nemohol roztrhnúť;
- zdieľaný priečinok. Keď je zdieľanie zapnuté, je to politika fscrypt a
  Linux pre zamknutý súbor fscrypt nedáva šifrovaný text, takže skript
  politiku na kopírovanie odomkne a potom ju znova zamkne. V buckete ho
  pokrýva šifrovanie resticu;
- `modules/overrides.nix`, vzhľad domovskej stránky, token proxy a súbor
  ľudí, ktorý číta LosOS Login.

Na adrese `amazonaws.com` si vlastník môže vybrať triedu úložiska: Glacier
Instant Retrieval, Glacier Flexible Retrieval alebo Glacier Deep Archive.
restic dá do tejto triedy iba dátové balíky a vlastné metadáta drží v S3
Standard, takže kontrola kódu a upratovanie zostávajú okamžité. Obnova z
Flexible Retrieval alebo Deep Archive zapne funkciu resticu `s3-restore`,
ktorá požiada AWS o rozmrazenie balíkov a čaká až 48 hodín; upratovanie tam
nikdy neprebaľuje (`--max-repack-size 0`), pretože prebalenie by tiež
vyžadovalo rozmrazenie. Pravidlo životného cyklu, ktoré presunie bucket do
Glacier, by so sebou vzalo aj metadáta resticu, preto sa trieda vyberá na
paneli.

Panel ponúka aj každú objednávku úložiska, ktorú zariadenie kúpilo na
[trhu](/in-depth/market.md), keď je jej zväzok naviazaný. Zálohy potom idú
cez edge, ktorý ho predal, protokolom REST nástroja restic na
`/market/storage/<order>/`, s rovnakým šifrovaním ako pri buckete. Edge
otvorí zväzok len kľúču, ktorý zariadenie vytvorí zo svojho kódu obnovy
a id objednávky (`POST /api/backup/target/bought`), takže zálohu neprečíta
edge ani zariadenia, ktoré držia repliky zväzku, a nové zariadenie s tým
istým kódom z nej dokáže obnoviť. Zariadenie pre tento cieľ nedrží žiadne
tajomstvo: úloha zálohy vytvorí kľúč z kódu zakaždým znova.

Bucket drží posledných sedem. lososd spúšťa každú zálohu a obnovu ako
prechodnú jednotku `losos-backup-<job>`, spustenú systemd mimo sandboxu
`ProtectHome=true` lososd. Kľúče bucketu sú zapečatené v úložisku
prihlasovacích údajov a k jednotke sa dostanú cez `LoadCredentialEncrypted=`,
nikdy nie na príkazovom riadku ani v obyčajnom súbore.

**Obnova** si vyžaduje kód na obnovu zariadenia, ktoré zálohu vytvorilo.
Vráti sa najnovší snapshot: aplikácie sa zastavia, súbory sa synchronizujú
späť, databázy sa načítajú (na zariadení, ktoré ešte používa PostgreSQL, cez
`pg_restore --clean`), aplikácie sa spustia a
obnovené `overrides.nix` sa skontroluje ako každé apply a prestavia sa. Kód,
ktorý zálohu otvoril, sa stane kódom na obnovu tohto zariadenia. Na
vymazanom alebo preinštalovanom zariadení spustite sprievodcu, nastavte ten
istý bucket a potom obnovte.

**Nastavenia, Reset, Vymazať** vymaže dáta aj nastavenia. lososd ho vedie vo
fázach uložených v `state.json`, takže pokračuje aj bez otvorenej karty:

1. voliteľná záloha. Ak zlyhá, vymazanie sa zastaví a nič sa nezmení;
2. odpočítavanie `losos.reset.graceMinutes` (predvolene 15). Zrušiť ho
   zastaví a nič na zariadení ani mimo neho sa ešte nezmenilo;
3. odchod z edge: vlastné domény zariadenia sa odstránia, peňaženka
   viazaná na jeho názvy sa odviaže, jeho aktívne ponuky na trhu sa uzavrú a zariadenie sa vyradí z registra
   (`POST /deregister`). Každý krok sa skúsi, aj keď predchádzajúci zlyhal,
   a správa vymenuje tie, ktoré edge nepotvrdil;
4. predvolené nastavenia sa commitnú a prestavia, potom lososd nechá
   značku na `/persist` a reštartuje;
5. `losos-factory-wipe.service` beží skoro pri ďalšom štarte, pred
   `sysinit.target` a pred akoukoľvek službou, ktorá vlastní dáta, a vymaže
   dáta aplikácií, databázy, stav oboch inštancií Kubernetes, súbor ľudí a
   stav LosOS Login, tajomstvá, journal a metadáta fscrypt. Vyprázdni oba
   dátové domovy. Značka ide posledná, takže prerušené mazanie sa spustí
   znova.

:::danger[Zrušiť sa dajú iba prvé dva kroky]
Zrušiť sa dá iba v krokoch 1 a 2. `/nix`, `/etc/nixos` a kľúčový súbor
disku zostanú, takže LosOS je stále nainštalovaný a zariadenie otvorí
sprievodcu nastavením. `/var/lib/losos-erase/report.json` uchováva, čoho sa
vymazanie vzdalo mimo zariadenia, ako počty, a Nastavenia, Reset to ukazuje.
`nix/tests/erase.nix` prebehne celý cyklus voči bucketu MinIO v jednej VM.
:::

## Používatelia {#users}

Dvaja dátoví používatelia, obaja bez hesiel a shellov:

| Používateľ  | Vlastní             |
| ----------- | ------------------- |
| `notshared` | Aplikácie vlastníka; jeho súbory v LosOS Cloud nesú toto meno |
| `shared`    | Úložisko mesh       |

Každý má vlastnú primárnu skupinu a domov s módom 700, takže ani jeden
nemôže čítať súbory toho druhého. `nix/tests/impermanence.nix` to kontroluje.

## Aktualizácie a reštarty {#upgrades-and-reboots}

- **03:00.** lososd najprv posunie `/etc/nixos` na najnovší commit vetvy
  `losos.configRepo.upstream.branch` (`main`) na
  `losos.configRepo.upstream.url` (repozitár na GitHube) a ponechá pritom
  `modules/overrides.nix` a `modules/install-target.nix` zariadenia. Potom
  `system.autoUpgrade` prestavia systém z `losos.upgradeFlakeUri`, teda z
  `git+file:///etc/nixos#install`. Fragment `#install` ponechajte; bez neho
  prestavba zlyhá. Ak sa od poslednej aktualizácie zmenil iný súbor v
  `/etc/nixos`, zariadenie ho ponechá a neaktualizuje; panel História to
  povie. `losos-ctl config --update` spustí prvý krok ručne.
- **00:07.** `midnight-reboot.timer` reštartuje bezpodmienečne
  (`Persistent=true`, takže zariadenie, ktoré bolo vypnuté, to dobehne).
  Keďže sa koreň pri štarte zostavuje nanovo, reštart je spôsob, akým sa
  zariadenie samo opravuje.
