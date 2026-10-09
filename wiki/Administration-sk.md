[English](Administration) · **Slovenčina** · [Deutsch](Administration-de)

# Správa

## Administrátorské rozhranie

Z LAN otvorte `http://<ip>/` (adresu z bannera na tty1 zariadenia) alebo
`https://<host>.local/`. Administrátorské rozhranie je zablokované pre všetko
mimo LAN vrátane prevádzky cez [master proxy](Master-Proxy-sk).
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
| [Lab](Lab-sk)        | `<host>.local/lab/`       |

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
LAN) a obyčajné stiahnutie (`/setup/losos-ca.crt`). Pozri [Inštalácia](Install-sk).

## Vzhľad

**Nastavenia, Vzhľad** mení vzhľad administrátorských stránok pre každý
prehliadač, ktorý zariadenie otvorí. Nič z toho nie je nastavenie v zmysle
Nixu. Je to dokument, ktorý lososd drží v `/var/lib/losos/look.json`, takže
zmena platí v okamihu uloženia, bez prestavby.

### Pozadie

Vyberte jeden z troch dodaných obrázkov alebo nahrajte vlastný (PNG, JPEG,
WebP, GIF alebo SVG, do 8 MiB). Posuvník **Závoj** kladie farbu
stránky cez obrázok, od 20 % do 90 %, aby text zostal čitateľný vo svetlej
aj tmavej téme. *Bez obrázka* obrázok opäť odstráni.

Nahraný obrázok servíruje lososd na `/api/look/background` bez tokenu, lebo
CSS `background-image` ho poslať nevie. Cesta je za tou istou ochranou „len
LAN“ ako zvyšok administrátorských stránok a tapeta nie je tajomstvo.

### Ručne písané widgety

Galéria nástenky na Prehľade (**Pridať widget**) má dva spôsoby, ako si urobiť
vlastný. *Zostaviť* poskladá widget z hodnôt zariadenia bez akéhokoľvek
kódu. *Napísať* prijme vlastné HTML, štýl a skript, ktoré sa držia na
zariadení a sú vypísané na tomto paneli, kde ich môžete upraviť a zmazať.

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

Karta **Ako to funguje** v editore to zopakuje s príkladom. Jej náhľad je skutočný
rámec, takže to, čo ukazuje, ukáže aj dlaždica. Widget môže sťahovať z
internetu (napríklad dlaždica s počasím), ale nie zo zariadenia. Limit je
24 widgetov po 64 KiB.

Editor má aj kartu **Vytvoriť s Claude**, kde Claude napíše widget podľa
opisu a vloží ho do poľa so zdrojom, aby ste si ho pred uložením prečítali.
Platí sa za každú tvorbu z predplateného zostatku a potrebuje edge, ktorý ju
ponúka; pozri [Tvorca widgetov](Widget-Builder-sk).

## Admin token

`lososd` pri prvom spustení zapíše náhodný token so 64 hexadecimálnymi
znakmi do `/var/secrets/losos-admin-token` (mód 0600). Token môže zmeniť
akékoľvek nastavenie, takže je to root.

Na novom, neprivlastnenom zariadení prvý volajúci z LAN, ktorý pošle
`POST /api/setup/claim`, nastaví heslo vlastníka a v odpovedi dostane token.
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

Tým volajúcim je sprievodca prvým spustením. Každá ďalšia návšteva
administrátorských stránok si pýta **heslo**, ktoré sprievodca nastavil, to
isté, ktorým sa prihlasujete do LosOS cloud. `POST /api/sign-in` sa opýta
Nextcloudu, či je správne, a pri kladnej odpovedi dá karte prehliadača
token. Zariadenie si nedrží žiadnu druhú kópiu hesla, takže jeho zmena v
LosOS cloud ho zmení aj pre administrátorské stránky.

Samotný token sa po nastavení hesla stále raz zobrazí ako **náhradný
správcovský kľúč**, s tlačidlami Kopírovať a Tlačiť. Pokrýva jediný
prípad, ktorý heslo pokryť nevie: keď LosOS cloud nebeží, pretože práve
LosOS cloud heslo overuje. Dialóg odomknutia má odkaz na jeho zadanie
namiesto hesla.

Nové heslo musí mať aspoň 12 znakov s malým písmenom, veľkým písmenom,
číslicou a symbolom, napríklad `-` alebo `!`. Sprievodca počas písania
ukazuje štyri pravidlá, každé s fajkou. Heslo nastavené pred zavedením tohto
pravidla sa stále prihlási.

Token sa na zariadení nedá vymeniť, pretože nemá shell. Strata hesla *aj*
náhradného kľúča znamená preinštalovanie z inštalačného média, ktoré vymaže
disk.

## Nastavenia

Nastavenia sú Nix. Stránka nastavení vygeneruje `modules/overrides.nix` a
spustí `nixos-rebuild switch`. Teda:

- Zmena si vyžaduje prestavbu, nie reštart.
- Zariadenie sa dá prestavať z tohto repozitára a toho jedného súboru.
- `apply` nahradí `overrides.nix` celý. Nastavenie, ktoré UI nezapíše, sa
  pri ďalšom uložení vráti na predvolené.

### Rozšírené

**Nastavenia, Rozšírené** vypisuje každú voľbu `losos.*`, ktorú zariadenie
deklaruje, načítanú z modulov, ktoré beží. `flake/options-doc.nix` vygeneruje
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

### História a LosOS Git

Každé Použiť, zmena úložiska a obnovenie továrenských nastavení je jeden
commit v `/etc/nixos`, repozitári, ktorý vytvoril `losos-ctl install`. Commit
je pomenovaný podľa nastavení, ktoré zmenil, `Change hostName, cluster.enable`,
s hodnotou pred a po každého z nich v tele. **Nastavenia,
História** vypisuje posledných štyridsať.

S `losos.configRepo.enable` (predvolené) a zapnutým LosOS Git drží lososd
konfiguráciu aj v súkromnom repozitári na LosOS Git, `<owner>/losos-config`,
ktorý vlastní administrátorov vlastný účet (`notshared`). Účet sa vytvorí pri
prvej synchronizácii s admin heslom a pri každej zmene hesla a prihlásení sa
s ním zosúladí. Reconciler v lososd beží každých 30 s:

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
štartovací skript Forgeja a vydá mu token (`flake/forgejo-bootstrap.nix`).

### Federácia

Federácia beží iba vtedy, keď zariadenie zdieľa svoj disk. Obe voľby
federácie nižšie sú spojené logickým AND s `losos.sharingMyStorage`,
nastavením, ktoré odomyká zdieľaný dátový pool (`lososInternal.federation` v
`modules/options.nix`). S vypnutým zdieľaním nekomunikuje ani LosOS Git, ani
LosOS cloud s inými servermi, nech hovorí ich vlastná voľba čokoľvek, a
panel Rozšírené zobrazí oba riadky ako nedostupné aj s dôvodom.

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
`http://<host>.local/forgejo/`, takže môžu iba iné zariadenia v tej istej
LAN. `tests/forgejo-federation.nix` spustí skutočný Forgejo a opýta sa ho, so
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
ktoré sa vypnúť nedajú. `tests/front-vhost.nix` kontroluje cesty oboch
služieb so zapnutým aj vypnutým zdieľaním; `tests/invariants.nix` kontroluje
bránu pri evaluácii.

## Záloha, obnova a vymazanie

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
- `modules/overrides.nix`, vzhľad domovskej stránky a token proxy.

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
3. odchod z edge: vlastné domény zariadenia sa odstránia, jeho aktívne
   ponuky na trhu sa uzavrú a zariadenie sa vyradí z registra
   (`POST /deregister`). Každý krok sa skúsi, aj keď predchádzajúci zlyhal,
   a správa vymenuje tie, ktoré edge nepotvrdil;
4. predvolené nastavenia sa commitnú a prestavia, potom lososd nechá
   značku na `/persist` a reštartuje;
5. `losos-factory-wipe.service` beží skoro pri ďalšom štarte, pred
   `sysinit.target` a pred akoukoľvek službou, ktorá vlastní dáta, a vymaže
   dáta aplikácií, databázy, stav oboch inštancií Kubernetes, tajomstvá,
   journal a metadáta fscrypt. Vyprázdni oba dátové domovy. Značka ide
   posledná, takže prerušené mazanie sa spustí znova.

Zrušiť sa dá iba v krokoch 1 a 2. `/nix`, `/etc/nixos` a kľúčový súbor
disku zostanú, takže LosOS je stále nainštalovaný a zariadenie otvorí
sprievodcu nastavením. `/var/lib/losos-erase/report.json` uchováva, čoho sa
vymazanie vzdalo mimo zariadenia, ako počty, a Nastavenia, Reset to ukazuje.
`tests/erase.nix` prebehne celý cyklus voči bucketu MinIO v jednej VM.

## Používatelia

Dvaja dátoví používatelia, obaja bez hesiel a shellov:

| Používateľ  | Vlastní             |
| ----------- | ------------------- |
| `notshared` | Nextcloud           |
| `shared`    | Úložisko mesh       |

Každý má vlastnú primárnu skupinu a domov s módom 700, takže ani jeden
nemôže čítať súbory toho druhého. `tests/impermanence.nix` to kontroluje.

## Aktualizácie a reštarty

- **03:00.** `system.autoUpgrade` prestavia systém z `losos.upgradeFlakeUri`.
  Predvolené `git+file:///etc/nixos#install` nesťahuje nové balíky. Na
  aktualizácie nastavte URI `github:`. Fragment `#install` ponechajte; bez
  neho prestavba zlyhá. Vzdialené URI stále používa zoznam diskov, režim
  firmvéru, režim odomykania a nastavenia tohto zariadenia, pretože ich
  prestavba číta z `/etc/nixos` na zariadení, nie z publikovaného
  repozitára.
- **00:07.** `midnight-reboot.timer` reštartuje bezpodmienečne
  (`Persistent=true`, takže zariadenie, ktoré bolo vypnuté, to dobehne).
  Keďže sa koreň pri štarte zostavuje nanovo, reštart je spôsob, akým sa
  zariadenie samo opravuje.
