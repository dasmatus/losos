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
| Nextcloud            | `<host>.local/nextcloud`  |
| Forgejo (voliteľné)  | `<host>.local/forgejo/`   |
| Príručka             | `<host>.local/handbook/`  |
| [Diagramy](/in-depth/diagrams.md) | `<host>.local/diagrams/` |

IP adresa zariadenia (tá z bannera na tty1) a jeho holé meno fungujú všade
namiesto `<host>.local`, Nextcloud nevynímajúc. Tak sa dostanete dnu z
libvirt VM, ktorá mDNS meno nedostane. nginx oznamuje podu Nextcloudu, na
ktorú adresu prišla každá požiadavka, a pod dôveruje presne tej jednej,
nikdy nie zástupnému znaku (`modules/workloads.nix`). `localhost` a
`127.0.0.1` dôveruje Nextcloud sám od seba.

`losos.tls.enable` (predvolene zapnuté) pridáva HTTPS na :443 s certifikátom,
ktorý si zariadenie vygeneruje samo, s platnosťou dva roky. Obyčajné HTTP na
:80 zostáva otvorené. Prehliadače certifikátu nedôverujú, kým ho
nenainštalujete. Sprievodca prvým spustením ponúka jednoriadkový inštalátor
pre macOS, Linux a Windows (`/setup/trust.sh`, `/setup/trust.ps1`, oba len z
LAN) a obyčajné stiahnutie (`/setup/losos-ca.crt`). Pozri [Inštalácia podrobne](/in-depth/install.md).

## Vzhľad {#look}

**Nastavenia, Vzhľad** mení vzhľad administrátorských stránok pre každý
prehliadač, ktorý zariadenie otvorí. Nič z toho nie je nastavenie v zmysle
Nixu. Je to dokument, ktorý lososd drží v `/var/lib/losos/look.json`, takže
zmena platí v okamihu uloženia, bez prestavby.

### Pozadie {#background}

Vyberte jeden z troch dodaných obrázkov alebo nahrajte vlastný (PNG, JPEG,
WebP, GIF alebo SVG, do 8 MiB). Posuvník **Závoj** kladie farbu
stránky cez obrázok, od 20 % do 90 %, aby text zostal čitateľný vo svetlej
aj tmavej téme. *Bez obrázka* obrázok opäť odstráni.

:::note[Prečo obrázok nepotrebuje token]
Nahraný obrázok servíruje lososd na `/api/look/background` bez tokenu, lebo
CSS `background-image` ho poslať nevie. Cesta je za tou istou ochranou „len
LAN“ ako zvyšok administrátorských stránok a tapeta nie je tajomstvo.
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
a tvorba nič nestojí; pozri [Tvorca widgetov](widget-builder.md).

## Admin token {#admin-token}

`lososd` pri prvom spustení zapíše náhodný token so 64 hexadecimálnymi
znakmi do `/var/secrets/losos-admin-token` (mód 0600). Token môže zmeniť
akékoľvek nastavenie, takže je to root.

Na novom, neprivlastnenom zariadení prvý volajúci z LAN, ktorý pošle
`POST /api/setup/claim`, nastaví heslo vlastníka, stane sa prvým človekom,
ktorý sa môže prihlásiť (správcom), a v odpovedi dostane token.
Táto cesta nevyžaduje token, ale prestane prijímať nároky, len čo má
zariadenie vlastníka. Zariadenie nastavujte iba v dôveryhodnej LAN, pretože
prvý volajúci sa stane jeho vlastníkom. Token sa vydá iba raz a cez UI sa už
znovu získať nedá.

Nárok poslaný predtým, než Nextcloud dokončí svoje prvé spustenie, odpovie
`503` s `{"error": …, "ready": false, "waitingFor": …}` a nič nezmení.
Zariadenie zostáva privlastniteľné. `GET /api/setup/claim` nesie tie isté
polia `ready` a `waitingFor`. Sprievodca prvým spustením ho periodicky
dotazuje a pustí vlastníka ďalej sám, len čo sa prepnú. Na čerstvom zariadení
to trvá niekoľko minút, pretože pod Nextcloudu spustí
`occ maintenance:install` skôr, než čokoľvek servíruje.

`ready` sa prepne, keď sa táto inštalácia dokončí, čo je skôr, než LosOS
cloud odpovie prehliadaču. Pod potom zapne svojich 26 aplikácií a až potom
spustí Apache. Natívne merané na 4-jadrovom hostiteľovi (2026-10-07) trvá
inštalácia asi 11 s, zapnutie aplikácií asi 13 s a prvá stránka odpovie 27 s
po štarte podu. Krok prihlásenia v sprievodcovi počas tejto medzery ukazuje
„Vaše súbory sa ešte spúšťajú“. Reštart
zopakuje tie isté kroky nad už nainštalovanou inštanciou asi za 2 s. Vo VM
bez KVM (emulácia TCG v QEMU) trvajú tie isté kroky 10 až 30 minút, preto
dajte demo VM hardvérovú virtualizáciu.

Tým volajúcim je sprievodca prvým spustením. Po ňom sa ľudia prihlasujú v
LosOS Login, prihlasovacej stránke zariadenia na
`https://<zariadenie>/auth/`, každý vlastným prihlasovacím menom a heslom.
LosOS Login je open-source prihlasovací server, ktorý beží na zariadení.
lososd vedie súbor ľudí, ktorý LosOS Login číta, s každým heslom ako hashom
argon2id, a LosOS Login jeho zmenu prevezme hneď. nginx sa LosOS Login pýta
na každú požiadavku na `/api/` a meno človeka odovzdá lososd, ktorý otvorí
časti, ktoré povoľujú jeho skupiny. Prvý človek je vždy správca. Vlastné
heslo si človek zmení cez `POST /api/set-password` a vlastníkovi to zmení aj
heslo do LosOS cloud. Všetkých ostatných pridáva, mení a odstraňuje správca
na paneli Používatelia cez `GET` a `POST /api/people` a
`POST /api/people/{access,password,remove}`. Panel je tam iba vtedy, keď
zariadenie prihlasuje ľudí samo (`standalone` v `GET /api/me`). lososd
odmietne siahnuť na vlastníka, na vlastný účet volajúceho aj na posledného
správcu. LosOS Login si človeka vyhľadá znova pri každej požiadavke, takže
odstránenie alebo zmenená skupina sa do aplikácií dostane tiež pri ďalšom
kliknutí. Prihlásenia sú vo vlastnom Redise LosOS Login, na sokete, ktorý
otvorí iba on, takže reštart LosOS Login ani nočný reštart nikoho
neodhlási.

LosOS cloud a LosOS Git prihlasujú ľudí rovnako, keď bežia ako pody, čo je
predvolené. nginx sa na prihlasovacej stránke každej aplikácie spýta LosOS
Login a pošle človeka ďalej v troch hlavičkách, `X-Losos-Cloud-User`,
`-Name` a `-Email`, ktoré nastaví pri každej požiadavke na niektorú z
aplikácií, takže žiadny klient nepošle vlastné. Vlastník, v skupine
`losos-owner`, príde do oboch ako účet `notshared`; ktokoľvek iný pod svojím
prihlasovacím menom. Nextcloud číta hlavičky aplikáciou user_saml v režime
premenných prostredia, Forgejo svojím prihlásením cez reverzný proxy, a
každá pri prvej návšteve vytvorí účet; lososd potom drží meno a adresu v
oboch aplikáciách v súlade so súborom ľudí. Človeka, ktorého LosOS Login
nepozná, pošle nginx tam, z LAN aj cez okrajový server; žiadna aplikácia
neukazuje vlastnú prihlasovaciu stránku. Cez okrajový server tunel doručuje
z loopbacku s `X-Forwarded-Proto: https`, čomu nginx verí iba z loopbacku, a
lososd pridá každé verejné meno, na ktorom zariadenie odpovedá, medzi domény
cookies LosOS Login. Kým sa LosOS Login spúšťa, aplikácie odpovedajú 503 so
stránkou, ktorá to skúša znova sama. Obe aplikácie
počúvajú na loopbacku a firewall odmietne spojenie na ktorýkoľvek z ich
portov od akéhokoľvek procesu na zariadení okrem roota, nginx a samotnej
aplikácie, takže nič na zariadení im nemôže pomenovať človeka. Mená
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
LosOS cloud ani LosOS Git a lososd mu odpovedá iba na jeho účet (`locked` v
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

- Voľba, ktorú už upravuje jeden z ostatných panelov (meno, režimy behu
  aplikácií, prepínače hardeningu), sa zobrazí so svojou hodnotou a odkazom
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
pri prvej synchronizácii s admin heslom a pri každej zmene hesla a pri
každom nastavení počítača pre LosOS Git sa s ním zosúladí. Reconciler v
lososd beží každých 30 s:

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
hodnoty volieb a token, ktorým sa lososd autentifikuje voči LosOS Git, žije v
`/var/lib/forgejo/.losos-token`, mimo `/etc/nixos`. Nestojí za tým žiadny
runner Forgejo Actions. Zariadenie sa periodicky dotazuje.

Adresa na klonovanie je zobrazená na paneli (`http://<box>/forgejo/<owner>/
losos-config.git`). Repozitár je súkromný, takže potrebuje admin heslo.
Démon komunikuje s Forgejom cez loopback s bot účtom `losos`, ktorý vytvorí
štartovací skript Forgeja a vydá mu token (`nix/forgejo-bootstrap.nix`).

### Federácia {#federation}

Federácia beží iba vtedy, keď zariadenie zdieľa svoj disk. Obe voľby
federácie nižšie sú spojené logickým AND s `losos.sharingMyStorage`,
nastavením, ktoré odomyká zdieľaný dátový pool (`lososInternal.federation` v
`modules/options.nix`). S vypnutým zdieľaním nekomunikuje ani LosOS Git, ani
LosOS cloud s inými servermi, nech hovorí ich vlastná voľba čokoľvek. Panel
Aplikácie oba prepínače zosivie aj s dôvodom a panel Rozšírené zobrazí oba
riadky ako nedostupné a odkáže na ne.

`losos.forgejo.federation.enable` (predvolene zapnuté) zapína ActivityPub
stránku Forgeja v oboch režimoch: `[federation] ENABLED` v app.ini, so
`SHARE_USER_STATISTICS = false`, aby nodeinfo nezverejňovalo súčty účtov ani
aktivity. Dodávaný Forgejo (16.x) federuje hviezdičky (Settings → Federation
repozitára vypisuje servery, ktorých hviezdičky sa rátajú), umožňuje sledovať
účty z iných serverov a servíruje nodeinfo a jedného ActivityPub aktéra na
každý účet a repozitár pod `/api/v1/activitypub/`; každá prichádzajúca
požiadavka musí niesť platný HTTP podpis. V režime kontajnera front vhost
pridá cesty s presnou zhodou pre `/.well-known/nodeinfo` a
`/.well-known/webfinger`, dve adresy, podľa ktorých iné servery zariadenie
objavia, a ktoré Forgejo servíruje v koreni hostiteľa, nie pod `ROOT_URL`;
nie sú chránené ochranou „len LAN“, rovnako ako `/forgejo/`. Dosah sa riadi
`ROOT_URL`: cez edge je to `https://<proxy hostname>/forgejo/` a federovať
so zariadením môže akýkoľvek server; na zariadení len v LAN je to
`https://<host>.local/forgejo/`, takže môžu iba iné zariadenia v tej istej
LAN. `nix/tests/forgejo-federation.nix` spustí skutočný Forgejo a opýta sa ho, so
zapnutým a potom vypnutým zdieľaním.

`losos.nextcloud.federation.enable` (predvolene zapnuté) je strana LosOS
cloud: Federated Cloud Sharing (zdieľanie s účtami `user@host` na iných
serveroch Nextcloud), federácia kalendárov a aplikácia trusted-servers. Keď
je vypnutá, stanú sa dve veci. Vlastné prepínače Nextcloudu sa nastavia pri
každom štarte: `outgoing_server2server_share_enabled` a
`incoming_server2server_share_enabled` aplikácie `files_sharing` (a dve
skupinové varianty) na `no`, `enableCalendarFederation` aplikácie `dav` na
false a aplikácia `federation` sa vypne. Entrypoint podu to urobí zo súboru
`federation`, ktorý hostiteľ pripojí; natívny režim to pridá do
`nextcloud-setup`. A vhost odpovie 404 na adresy, ktoré volajú iné servery:
`ocm-provider`, `ocs-provider`, `ocm/`, `.well-known/ocm`,
`ocs/v[12].php/cloud/shares` a cesty aplikácií `federation` a
`federatedfilesharing`. Polovica vo vhoste je potrebná, pretože OCM discovery
hlási, že je zapnuté, nech hovoria nastavenia čokoľvek, a
`cloud_federation_api` a `federatedfilesharing` sú vždy zapnuté aplikácie,
ktoré sa vypnúť nedajú. `nix/tests/front-vhost.nix` kontroluje cesty oboch
služieb so zapnutým aj vypnutým zdieľaním; `nix/tests/invariants.nix` kontroluje
bránu pri evaluácii.

Zapnutá federácia dá LosOS cloud aj `allow_local_remote_servers = true`
a `sharing.federation.allowSelfSignedCertificates = true`
(`modules/nextcloud-stack.nix`, `federation.system`; natívne v
`override.config.php` modulu, v pode v `losos.config.php`). `RemoteHostValidator`
v Nextcloud inak odmietne každý server pod `.local`, každé meno bez bodky a
každú súkromnú adresu, teda každé iné zariadenie v LAN, takže zdieľanie pre
`notshared@inezariadenie.local/nextcloud` zlyhalo skôr, než požiadavka
opustila zariadenie. Adresu bez protokolu Nextcloud skúša iba cez https
a každé zariadenie má vlastný certifikát (`modules/tls.nix`), preto druhý
kľúč vypne kontrolu certifikátov pri volaniach federácie. Cena: čokoľvek,
čo účet dokáže Nextcloud prikázať stiahnuť, môže tiež mieriť do LAN, a
volania federácie sú otvorené každému, kto sedí medzi dvoma zariadeniami.
Zariadenie bez https má adresu `notshared@http://inezariadenie.local/nextcloud`.
`nix/tests/cloud-apps.nix` spustí dve zariadenia v natívnom režime so
zapnutým zdieľaním a vlastnými certifikátmi pre https a pošle súbor od
jedného vlastníka druhému: zdieľanie sa na druhom zariadení ponúkne, prijme
a prečíta späť cez WebDAV.

### Nastavenia aplikácií pre celé zariadenie {#box-wide-app-settings}

Panel Aplikácie nesie nastavenia, ktoré platia pre všetkých v LosOS cloud a
LosOS Git, teda tie, ktoré si inak každá aplikácia drží na vlastných
stránkach správy. Každé pole je voľba pod `losos.nextcloud.site.*` alebo
`losos.forgejo.site.*`, takže ide cez Použiť, bránu, Históriu a repozitár
konfigurácie ako každé iné nastavenie. Panel Rozšírené ukazuje ich hodnoty a
odkazuje na sekciu aplikácie na paneli Aplikácie (`/apps#cloud`,
`/apps#git`), namiesto toho aby kreslil druhý editor (`ON_PANE` v
`admin-ui/app/src/lib/option-value.ts`). Kreslia sa tam aj oba prepínače
federácie vyššie, na začiatku každej sekcie.

| Voľba | Čo nastaví v aplikácii |
| --- | --- |
| `losos.nextcloud.site.defaultLanguage` | `default_language` |
| `losos.nextcloud.site.phoneRegion` | `default_phone_region` |
| `losos.nextcloud.site.trashDays` | `trashbin_retention_obligation`, `"D, D"` |
| `losos.nextcloud.site.versionDays` | `versions_retention_obligation`, `"D, D"` |
| `losos.nextcloud.site.publicLinks` | `core` `shareapi_allow_links` |
| `losos.nextcloud.site.linkPassword` | `core` `shareapi_enforce_links_password` |
| `losos.nextcloud.site.linkExpiryDays` | `core` `shareapi_default_expire_date`, `shareapi_enforce_expire_date`, `shareapi_expire_after_n_days` |
| `losos.nextcloud.site.defaultQuotaGB` | `files` `default_quota` |
| `losos.forgejo.site.requireSignIn` | `[service] REQUIRE_SIGNIN_VIEW` |
| `losos.forgejo.site.keepEmailPrivate` | `[service] DEFAULT_KEEP_EMAIL_PRIVATE` |
| `losos.forgejo.site.defaultPrivate` | `[repository] DEFAULT_PRIVATE` |
| `losos.forgejo.site.pushCreate` | `[repository] ENABLE_PUSH_CREATE_USER` |
| `losos.forgejo.site.landingPage` | `[server] LANDING_PAGE` |

Voľba Nextcloudu s hodnotou null nechá to nastavenie na Nextcloude. Prvé
štyri sú kľúče `config.php`: natívny režim ich dá do `override.config.php`
modulu, pod do `losos.config.php`, a oba sa čítajú po `config.php`, takže
hodnota, ktorú tam zapíše Nextcloud, nikdy nevyhrá. Ďalšie štyri sú
konfigurácia aplikácií v databáze. `modules/nextcloud-stack.nix` (`site`) ich
vypíše ako riadky `app key value` a `occ config:app:set` ich zapíše pri
každom štarte: entrypoint podu zo súboru `site`, ktorý pripojí hostiteľ,
natívny režim na konci `nextcloud-setup`. Stránky správy Nextcloudu ukladajú
tieto kľúče cez
`ocs/v[12].php/apps/provisioning_api/api/v1/config/apps/<app>/<key>` a
predný vhost odpovie 403 pre každý kľúč, ktorý nastavuje LosOS, vrátane
prepínačov federácie, takže zmena urobená tam sa nemôže rozísť so
zariadením. Stránka správy hodnotu stále ukazuje. `nix/tests/front-vhost.nix`
kontroluje 403 a to, že ostatné kľúče sa do Nextcloudu stále dostanú.

Päť nastavení Forgejo sú kľúče `app.ini`, ktoré Správa stránky vo Forgejo
ukazuje bez možnosti zmeny. Oba režimy čítajú jednu množinu atribútov,
`lososInternal.forgejoSite`.

:::note[Čo zostáva v aplikáciách]
Všetko, čo si nastaví účet sám pre seba, zostáva v aplikácii. V LosOS Git
zostáva účet vlastníka správcom Forgejo pre to, čo LosOS zatiaľ nepokrýva:
nastavenia Forgejo uložené v databáze a zoznam účtov. V LosOS cloud nie je
správcom žiadny účet (ďalšia časť), takže doručovanie pošty, úlohy na
pozadí a vynútenie dvojfaktorového overenia majú hodnoty, ktoré nastaví
zariadenie.
:::

### Aplikácie v LosOS cloud {#losos-clouds-apps}

Sekcia LosOS cloud na paneli Aplikácie vypisuje každú aplikáciu, ktorú
Nextcloud má, každú s prepínačom, odstraňuje aplikácie pridané z obchodu a
hľadá v obchode. lososd ju obsluhuje v sekcii `apps`:

| Cesta | Čo robí |
| --- | --- |
| `GET /api/cloud/apps` | `{available, store, apps}`, alebo `{available: false, reason}`, kým LosOS cloud neodpovedá |
| `GET /api/cloud/apps/search?q=` | najlepšie zhody obchodu pre túto verziu Nextcloud |
| `POST /api/cloud/apps/{install,enable,disable,remove}` | `{id}`; odpovie celým zoznamom po zmene |

lososd zmenu skontroluje skôr, než ju spustí (`backend/src/cloud_apps.rs`).
Odmietne aplikácie, ktoré LosOS nastavuje sám (`user_saml` a
`provisioning_api` pre prihlásenie, `federation` pre zdieľanie, `theming`
pre vzhľad, `firstrunwizard` pre sprievodcu nastavením), aplikácie, ktoré
Nextcloud drží zapnuté (`alwaysEnabled` v `core/shipped.json`), a
odstránenie čohokoľvek mimo `store-apps`, lebo pribalená aplikácia by sa
pri ďalšom štarte vrátila. Odmietnutie je 400, Nextcloud, ktorý nebeží, 409
a neúspešný beh `occ` 502 s jeho správou.

Prácu robí `losos-nextcloud-apps` (`modules/nextcloud-stack.nix`,
`appsTool`), v pode (`crictl exec`) alebo natívne na ceste jednotky lososd.
`list` spojí `occ app:list` so súborom `appinfo/info.xml` každej aplikácie
(`modules/nextcloud-apps-list.php`); slovesá spúšťajú `occ app:install`,
`app:enable`, `app:disable` a `app:remove --keep-data`. Vypnutie aplikácie
zapíše jej id do `/var/lib/nextcloud/.losos-apps-off` a každý štart to
rešpektuje: pod tieto id vynechá zo svojho riadku `app:enable` a natívny
režim ich po `extraAppsEnable` modulu znova vypne. Zapnutie alebo opätovné
pridanie id zo zoznamu zoberie. Hľadanie v obchode stiahne index obchodu
pre túto verziu Nextcloud raz za šesť hodín
(`apps.nextcloud.com/api/v1/platform/<version>/apps.json`, asi 17 MB) a
ponechá najnovšie stabilné vydanie každej aplikácie.

Účet vlastníka (`notshared`) sa pri každom štarte vyberie zo skupiny
`admin` v Nextcloud (`demoteOwner`, keď ho inštalácia urobila prvým
správcom), takže prihlásený vlastník nemôže vnútri LosOS cloud zmeniť, čo
nastavuje LosOS. Všetko, čo lososd od Nextcloud ako vlastník žiada (heslá
aplikácií, WebDAV, `cloud/user`), funguje pre bežný účet. Vlastné stránky
Aplikácie a Používatelia v Nextcloud sú preto nedostupné; ľudia dostanú
účty z LosOS Login pri prvej návšteve. `nix/tests/cloud-apps.nix` kontroluje
skupinu, zoznam, to, že vypnutá aplikácia je vypnutá aj po ďalšom behu
`nextcloud-setup`, a to, že odstrániť sa dá iba aplikácia z obchodu.

## Záloha, obnova a vymazanie {#backup-restore-and-erase}

**Nastavenia, Záloha** posiela dáta zariadenia do S3-kompatibilného bucketu,
ktorý si vlastník prenajíma (Amazon S3 vrátane Glacier, Backblaze B2, Wasabi,
Cloudflare R2, MinIO).
[restic](https://restic.net) robí kopírovanie a šifruje na zariadení skôr,
než čokoľvek odíde, takže bucket drží šifrovaný text. Heslo repozitára je
kód na obnovu zariadenia (`/var/secrets/losos-recovery-code`), ktorý panel na
požiadanie ukáže; bez neho nikto zálohu neotvorí.

Záloha obsahuje:

- `/var/lib/nextcloud` a `/var/lib/forgejo`, bez náhľadov Nextcloudu;
- `pg_dump` databáz `nextcloud` a `forgejo`;
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

Bucket drží posledných sedem. lososd spúšťa každú zálohu a obnovu ako
prechodnú jednotku `losos-backup-<job>`, spustenú systemd mimo sandboxu
`ProtectHome=true` lososd. Kľúče bucketu sa k nej dostanú ako
`EnvironmentFile=` v `/var/secrets`, nikdy nie na príkazovom riadku.

**Obnova** si vyžaduje kód na obnovu zariadenia, ktoré zálohu vytvorilo.
Vráti sa najnovší snapshot: aplikácie sa zastavia, súbory sa synchronizujú
späť, databázy sa načítajú cez `pg_restore --clean`, aplikácie sa spustia a
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
| `notshared` | Nextcloud           |
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
