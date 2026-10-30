---
title: Architektúra
sidebar_position: 9
mdx:
  format: md
---

# Architektúra

![Prehľad, admin stránka, ktorú posiela nginx a za ktorú odpovedá lososd.](@site/docs/img/overview-sk.png)

## Systémy vo flaku {#systems-in-the-flake}

| Výstup                        | Čo to je                                         |
| ----------------------------- | ------------------------------------------------ |
| `nixosConfigurations.iso`     | Živé inštalačné ISO (`options`, `disko`, `installer`) |
| `nixosConfigurations.install` | Nainštalované zariadenie (všetky moduly)         |
| `nixosModules.edge`           | Strana VPS [master proxy](/in-depth/master-proxy.md) a mesh |

Všetky voľby projektu sú pod `losos.*` v `modules/options.nix`, s
predvolenými hodnotami v `defaults.nix`. Moduly čítajú `config.losos.*`.

## Impermanence {#impermanence}

`/` je tmpfs. `/persist` je ext4 šifrovaný cez LUKS. `impermanence` do
systému bind-mountuje z `/persist` tieto cesty: `/nix`, `/var`, `/etc/ssh`,
`/etc/keys`, `/etc/nixos`, `/etc/rancher`, dva dátové domovské adresáre a
`machine-id`.

:::warning[Nezapísaný stav sa pri reštarte stratí]
**Všetko, čo na tomto zozname nie je, sa reštartom stratí.** Nový stav
pridajte do `modules/impermanence.nix`.
:::

## Riadiaca vrstva {#control-plane}

| Komponent      | Úloha |
| -------------- | ---- |
| `lososd`       | Systemd démon bežiaci ako root (Rust). Jediný, kto zapisuje `/var/lib/losos/state.json`. D-Bus služba `org.losos1` a JSON API overované tokenom na `127.0.0.1:8082`. |
| `losos-ctl`    | CLI, ktoré každý podpríkaz posiela `lososd` cez D-Bus. `losos-ctl install` beží na ISO samostatne. |
| `losos-admin-ui` | SPA v React + Vite + Tailwind, servírovaná cez nginx, ktorý `/api/*` presmerúva na `lososd`. |

Prestavby (rebuildy) bežia ako dočasné systemd jednotky (`losos-rebuild-<job>`).
`nixos-rebuild switch` reštartuje `lososd`, preto sa démon pri štarte znova
pripojí ku každej prestavbe, ktorá je stále zaznamenaná ako `building`.

Rozloženie crate nájdete v
[`backend/README.md`](https://github.com/dasmatus/losos/blob/main/backend/README.md)
a formát komunikácie v [`backend/schema.json`](https://github.com/dasmatus/losos/blob/main/backend/schema.json).

## Pracovné záťaže {#workloads}

LosOS Cloud (`modules/cloud.nix`, `backend/src/cloud/`) a LosOS Git
(`modules/git.nix`, `backend/src/forge/`) sú samostatné služby na
hostiteľovi a každá počúva na Unix sockete, ktorý smie otvoriť iba nginx.
Vlastný k3s klaster zariadenia prevádzkuje iba aplikácie inštalované ako
`shared`, kým zariadenie zdieľa disk s mesh; jeho pody používajú
`hostNetwork`. nginx drží webové porty zariadenia a smeruje podľa cesty.
Traefik drží porty nainštalovaných aplikácií v lokálnej sieti
(`losos.apps.ports`) a cestu ku každej aplikácii číta zo štítkov na jej
kontajneroch.

## Rozloženie repozitára {#repository-layout}

```
admin-ui/app/            the admin SPA
admin-ui/tui/            losos, the terminal client (Rust)
admin-ui/lab/            LosOS Diagrams' core and canvas (Rust, WebAssembly)
admin-ui/themes/         LosOS Cloud and LosOS Git: themes on the SPA's tokens.css, logos, names
admin-ui/design-system/  dev-only tokens and React wrapper; not shipped
backend/                 lososd + losos-ctl (Rust)
edge/registrar/          the edge registrar (Rust)
edge/vercel/             the registrar as one Vercel Function, for demos
edge/contracts/          the payment and escrow contracts (Solidity)
modules/                 NixOS modules; options in options.nix
nix/                     packages, images and build helpers
nix/tests/               NixOS VM tests
docs/                    security model, developer notes, design specs and plans
handbook/                this handbook (Docusaurus)
```

:::note[Skomprimovaná podoba tejto stránky]
`.claude/CLAUDE.md` v repozitári je skomprimovaná podoba tejto stránky a vymenúva
záludnosti, ktoré hryznú potichu.
:::

## Do hĺbky {#in-depth}

Jeden odsek na každý mechanizmus: prečo je ktorý kus tam, kde je, a čo tam
pokazí dobre mienená zmena.

**Dva systémy NixOS zdieľajú jednu sadu modulov** (`flake.nix`).

- `iso` je minimálne živé ISO. Nesie rozloženie `disko`, aby mohlo
  naformátovať cieľový disk a zaregistrovať šifrovacie kľúče, a importuje
  iba `options.nix`, `disko.nix` a `installer.nix`.
- `install` je systém nainštalovaný na disku. Importuje všetko:
  `options`, `configuration`, `impermanence`, `disko`, `boot`, `services`,
  `cloud`, `containers`, `daemon`, `overrides`, `updates`, `defaults`. Flake odovzdáva `specialArgs.self = self`, aby `defaults.nix`
  dosiahol na `self.packages.${system}.{losos-ctl,losos-admin-ui}` a zapojil
  riadiacu vrstvu a administračné UI.

**Bezstavovosť cez impermanence** (`impermanence.nix`, `disko.nix`, `boot.nix`).
Koreň je tmpfs. `/persist` je ext4 šifrovaný cez LUKS s funkciou `encrypt`,
pretože ju potrebuje fscrypt a btrfs ju poskytnúť nevie. Reštart prežijú iba
adresáre v `environment.persistence."/persist".directories`: `/nix`, `/var`,
`/etc/ssh`, `/etc/keys`, `/etc/nixos`, `/etc/rancher`, dva dátové domovské
adresáre a `machine-id`. **Všetko nové, čo musí prežiť reštart, patrí na
tento zoznam**, inak to pri ďalšom štarte bez chyby zmizne. `/persist` je
`neededForBoot`, takže bind mounty impermanence sa vyriešia skôr, než sa
naplní sysroot.

Inštalátor zväzok vždy naformátuje bez obsluhy, z náhodného kľúčového súboru,
ktorý vygeneruje v `/etc/keys/persist-keyfile`. Odomykanie je predvolene cez
TPM2 (`losos.tpm.enable = true`). Hneď po `disko` inštalátor spustí
`systemd-cryptenroll --tpm2-device=auto --tpm2-pcrs= --unlock-key-file=…`, takže
initrd nenesie žiadne tajomstvo a kľúčový súbor zostáva iba vnútri `/persist`
ako záchranný slot, ktorým sa overuje `losos-ctl grow`. Väzba na PCR zámerne
neexistuje. Zariadenie sa aktualizuje samo bez obsluhy a nemá shell, z ktorého by sa
dalo zotaviť zo zablokovania.

Ak inštalačné médium nevidí `/dev/tpmrm0`, alebo s `losos-ctl install
--no-tpm`, je zariadenie v režime kľúčového súboru. `install-target.nix` nastaví
`tpm.enable = false` a kľúčový súbor ide do initrd ako
`/crypto_keyfile.bin` na nešifrovanom ESP. Dva tvary crypttab sa navzájom
vylučujú: systemd-cryptsetup, ktorý dostane kľúčový súbor *aj* `tpm2-device=`,
číta súbor ako zapečatený blob. `nix/tests/invariants.nix` pripína predvolenú
hodnotu a overuje, že cesta s TPM nedeklaruje žiadne tajomstvo v initrd.
`nix/tests/tpm.nix` ju celú nabootuje pod swtpm.

Vzdialené URI `github:` pre upgrade vyhodnocuje strom bez
`install-target.nix`. Preto `flake.nix` uprednostní živé
`/etc/nixos/modules/{install-target,overrides}.nix` vždy, keď ich dokáže
prečítať, a obe cesty prestavby (`updates.nix`, `supervisor.rs`) kvôli tomu
odovzdávajú `--impure`. Pozrite odsek o automatickom upgrade nižšie.

**Dve izolované dátové domény, žiadny shell** (`configuration.nix`). `notshared`
(uid 1000) je účet vlastníka v LosOS Cloud. `shared` (uid 1001) vlastní úložisko poskytnuté
do mesh. Oba domovské adresáre majú režim `700`, každý s vlastnou primárnou
skupinou, takže žiadny z používateľov nemôže čítať súbory toho druhého.
`isNormalUser` bez explicitnej `group` dá oboch do `users` a domovský adresár
s `750` potom tejto skupine povolí r-x. To potichu rušilo izoláciu, kým to
nezachytil `nix/tests/impermanence.nix`. Žiadny z používateľov nemá heslo a
`services.openssh.enable = false`. Jediné zmeny konfigurácie dostupné z
bežiaceho zariadenia idú cez administračné UI: prepínač úložiska Local alebo Mesh,
**join the compute mesh** (pripojiť sa k výpočtovému mesh) a **share my
compute when I sleep** (zdieľať môj výkon, keď spím).

**Démon lososd, fasáda losos-ctl a administračný endpoint**
(`modules/daemon.nix`, `backend/`, `admin-ui/`). Privilegovaná logika je
v `lososd`, systemd démonovi bežiacom ako root, napísanom v Ruste (`backend/`,
jeden crate s dvoma binárkami; zbus pre D-Bus, actix-web pre HTTP, clap pre
CLI). Poslucháč zbernice a supervízor prestavieb bežia na runtime tokio.
HTTP server beží vo vlastnom vlákne pod actix `System`. Oba runtimy sú
zámerne oddelené.

lososd vlastní `/var/lib/losos/state.json` ako jediný zapisovateľ, s
atomickými zápismi cez dočasný súbor a premenovanie. Na systémovej zbernici
D-Bus exportuje jednu metódu na každý podpríkaz (zbernica `org.losos1`, cesta
`/org/losos1`, rozhranie `org.losos.Control1`) a poskytuje JSON HTTP API
overované cez Bearer na `127.0.0.1:${losos.admin.apiPort}` (predvolene 8082).
Prestavby bežia ako dočasné jednotky `systemd-run` (`losos-rebuild-<job>`).
Sledovacie vlákno jednotku dopytuje a zaznamená hotovo alebo zlyhanie. Po
reštarte démona sa znova pripojí, a ten sa stáva, pretože `nixos-rebuild
switch` reštartuje samotný lososd uprostred prestavby.

Práca lososd sa presúva do pluginov (`backend/plugins/`). Plugin je
komponent WebAssembly, ktorý odpovedá na cesty API uvedené v jeho manifeste
a beží vo Wasmtime vnútri lososd. Nemá WASI: súbory, programy, jednotky
systemd, HTTP, zámky a časovače sú volania hostiteľa a lososd každé overí
voči manifestu pluginu, takže plugin dosiahne len cesty, programy a
endpointy, ktoré deklaruje. Prihlásenie, sekcie v `backend/sections.json` a
D-Bus zostávajú v lososd. Každé volanie beží v novej inštancii s limitom
pamäte a časovým limitom, takže plugin, ktorý sa zacyklí alebo spanikári,
stojí jednu odpoveď 500 a nič viac. lososd načítava pluginy len z
`losos.backend.plugins`, adresára v úložisku Nix (`.#losos-plugins`), a
odmietne sa spustiť, ak je adresár alebo ktorýkoľvek súbor v ňom mimo
úložiska, zapisovateľný, nepatrí rootovi alebo leží na pripojení na čítanie
aj zápis. Zatiaľ sú pluginmi vzhľad (obrázok pozadia a widgety) a
pripnuté položky bočného panela; `nix/tests/plugins.nix` ich bootuje.

`losos-ctl` je tenké fasádne CLI. Zachováva staré podpríkazy a JSON výstup
(`state`, `change --mode`, `status`, `settings`, `apply` (stdin),
`factory-reset`; `--json` nerobí nič) a každé volanie posúva lososd cez
D-Bus. `losos-ctl install` zostáva lokálny, pretože inštalátor na ISO beží
ako root a zbernicu nepotrebuje.

Administračné UI (`admin-ui/app/`, zabalené ako `losos-admin-ui` cez
Bun) je SPA v React 19, Vite a Tailwind v4 na skutočných
cestách (`/`, `/apps`, `/storage`, `/mesh`, `/settings/<pane>`). Jeho
zostavený `dist/` **je** koreňový adresár dokumentov predného nginx vhostu,
servírovaný s `try_files $uri $uri/ /index.html`. Bez tohto riadku je každý
hlboký odkaz a každé znovunačítanie 404. Vhost presmerúva `/api/*` na
loopback API lososd. SPA nahradila dvojicu stránok v čistom JS, `dashboard/`
a `settings/`. Akákoľvek zmienka o nich, alebo o `/ds/` či `/common.js`, je
zastaraná.

Stavebné prvky SPA (`admin-ui/app/src/components/ui/`) sú komponenty
shadcn/ui na palete zariadenia. `components.json` nasmeruje shadcn CLI na
`src/styles/index.css`, ktorý mapuje názvy farieb shadcn na `tokens.css`.
`accent` a `muted` sú zámerne nenamapované, pretože aplikácia oba názvy už
používa na niečo iné.

O tom, z ktorej varianty každý komponent pochádza, rozhoduje `style-src
'self'` administračnej stránky. Overené v Chromiu: povoľuje React props
`style` a každý iný zápis cez CSSOM, ale odmieta `setAttribute("style")` a
každý prvok `<style>`, ktorý knižnica vytvorí za behu. sonner sa vykreslí
bez štýlov, pokiaľ sa jeho dodávaný `styles.css` neimportuje do balíka.
Zámok posúvania z Radixu (react-remove-scroll) nerobí vôbec nič, takže nič
modálne nie je z Radixu. Sidebar a jeho sheet, tooltip a zbaľovacie časti
pochádzajú zo *základného* registra shadcn na Base UI, ktorého zámok
posúvania používa CSSOM. Switch je varianta shadcn pre React Aria
(`react-aria-components`), ktorú `SwitchRow` vkladá do riadkov nastavení ako
shadcn Field „Switch with a description“. Iba potvrdzovací Toast je z Radixu
a zámok posúvania nepotrebuje. Dialog zostáva na natívnom `<dialog>`.
`tests/app.browser.mjs` servíruje skutočnú hlavičku CSP a zlyhá pri každom
novom porušení, takže výmena knižnice, ktorá vkladá štýly, zmení kontrolu na
červenú.

Príkazová vrstva je napísaná proti efektovému traitu `Losos` so skutočnou
implementáciou (`io_backend`) a implementáciou v pamäti (`fake`), takže
stavový automat sa testuje jednotkovými testami bez súborového systému.
Inštalátor opakuje rovnaký vzor s `Install` a `plan_install`. Jeho plán sú
dáta, takže test overí poradie deštruktívnych krokov bez toho, aby čokoľvek
formátoval. Komunikačný kontrakt je `backend/schema.json`. Nastavte
`losos.backend.package = null`, ak chcete bežať bez démona.

**Každá voľba na jednom paneli a konfigurácia v LosOS Git**
(`nix/options-doc.nix`, `modules/config-repo.nix`,
`backend/src/options.rs`, `backend/src/config_repo.rs`,
`admin-ui/app/src/screens/settings/pane-advanced.tsx`, `pane-history.tsx`).
Pri zostavení sa deklarácie `losos.*` prejdú do
`/etc/losos/options.json`: názov, druh editora, predvolená hodnota, popis,
bežiaca hodnota a príznaky nebezpečnosti a iba na čítanie. lososd k tomu pri
`GET /api/options` pripojí aktuálny `overrides.nix` a panel Rozšírené vykreslí
pre každý riadok typovaný editor.

Bránou je `classify` v Nix súbore. Deklarácia s typom, ktorý nepozná, je
`throw`, takže úloha eval zčervenie namiesto toho, aby na panel dopadol
prázdny riadok. `tests/advanced.browser.mjs` vykreslí skutočný dokument
(`nix/tests/admin-ui.nix` odovzdáva kontrolu `losos-options-doc`) a zlyhá pri
každom riadku, ktorý panel nevie vykresliť. `POST /api/apply` overí každý
riadok proti dokumentu (`check_body`). Odmietne nedeklarované kľúče, kľúče
iba na čítanie (hodnoty pevne dané inštalátorom, balíky), hodnoty zlého typu
a akékoľvek `${` a pomenuje kľúč, ktorý odmietol.

Každé použitie zmien (apply), zmena režimu a reset je commit v `/etc/nixos`,
repozitári, ktorý vytvoril inštalátor. Zosúlaďovacie vlákno v lososd sa
dopytuje každých 30 s. Vytvorí administrátorov účet v LosOS Git
(`notshared`, správca) a súkromný repozitár `losos-config`, s admin
tokenom, ktorý LosOS Git zapíše pri prvom štarte
(`/var/lib/losos-git/admin-token`). Keď LosOS Git zaostáva, pushne. Keď
klon pushol dopredu, urobí fast-forward, nový `overrides.nix` prepustí
rovnakou bránou a spustí prestavbu. Rozdvojenú históriu nahlási a nikdy sa
jej nedotkne. Za tým všetkým zámerne **nie je žiadny CI runner**.

Panel Rozšírené odkazuje na pätnásť nastavení, ktoré vlastnia ostatné panely,
namiesto toho, aby ich upravoval druhýkrát. `OWNED` v
`admin-ui/app/src/lib/option-value.ts` a `OWNED_NAMES` v `lib/api.ts` sú
tento zoznam a `buildOverridesNix` podľa neho filtruje ďalšie kľúče, aby
súbor nikdy neobsahoval kľúč dvakrát. Dôležité sú tu dve vlastnosti parsera.
`parse_line` v `overrides.rs` odreže hodnotu pri prvom `#` *mimo* úvodzoviek;
kedysi ju odrezával aj v nich, čo pokazilo predvolené `upgradeFlakeUri`. A
zoznam je jeden riadok `[ "a" "b" ]`, iba reťazce.

**Pracovné záťaže a jediné predné dvere** (`modules/containers.nix`,
`modules/cluster.nix`, `modules/cloud.nix`, `modules/git.nix`).
systemd-nspawn je preč a s ním aj režimy aplikácií: LosOS Cloud a LosOS Git
bežia na hostiteľovi, každá pod vlastným používateľom, a vlastný lokálny k3s
klaster zariadenia prevádzkuje iba aplikácie inštalované ako `shared`. nginx
drží webové porty 80 a 443. Smeruje podľa cesty na mDNS mene
zariadenia (`<hostName>.local/cloud/` a adresy, ktoré volajú aplikácie
Nextcloud, `/git/`) a na tom istom vhoste servíruje administračnú SPA.
Aplikácie, ktoré vlastník nainštaluje ako
`notshared`, a vlastné aplikácie zariadenia naopak bežia na Podmane bez
roota cez Podman Compose (`modules/apps.nix`,
[Formát aplikácií](app-format.md)), takže zariadenie, ktoré nič nezdieľa,
pre ne nepotrebuje klaster. Každú
nainštalovanú aplikáciu otvorí na porte z `losos.apps.ports` v lokálnej sieti
Traefik (`losos-traefik`, pod vlastným používateľom). Cesty číta zo štítkov,
ktoré zariadenie dá na kontajnery aplikácie, cez Podman API používateľa
`notshared`, ktoré smie otvoriť iba skupina Traefiku. Aplikácia `shared`
namiesto toho dostane súbor s cestou. API Traefiku a jeho metriky
Prometheus odpovedajú na `127.0.0.1:${losos.apps.traefikPort}` (8089), kde
ich plugin štatistík v lososd raz za minútu číta pre `GET /api/statistics`.

**LosOS Git** (`modules/git.nix`, `backend/src/forge/`, binárka
`losos-git` z crate lososd). Git server zariadenia, pod vlastným
používateľom. Smart HTTP gitu (`git-upload-pack` a `git-receive-pack
--stateless-rpc`, vrátane protokolu v2) prúdi oboma smermi, účty, tokeny a
záznamy repozitárov sú jeden súbor SQLite a repozitáre sú holé pod
`/var/lib/losos-git/repositories/<owner>/<name>.git`. Počúva dvakrát: na
`/run/losos-git/http.sock`, ktorý smie otvoriť iba nginx a ktorý je jediné
miesto, kde verí človeku, ktorého LosOS Login pomenuje v hlavičkách
`X-Losos-Cloud-*`, a na `127.0.0.1:3000` iba pre tokeny, kde lososd drží
repozitár konfigurácie a vyrába tokeny počítačov. Token je uložený ako
jeho SHA-256 a nesie rozsahy. Issues, pull requesty, komentáre, štítky a
prílohy sú v tom istom súbore a číta ich `/api/v1/repos/…`.

Zariadenie, na ktorom bežal Forgejo, si jeho záznamy prinesie. Pred prvým
štartom LosOS Git presunie `losos-git-forgejo-import` repozitáre bez ich
hookov, vyexportuje databázu Forgejo (z Postgresu, na natívnom zariadení zo
SQLite), ponechá jej úplný dump a `losos-git import-forgejo` prevezme účty,
spolupracovníkov, prístupové tokeny (staré hodnoty fungujú ďalej), štítky,
issues, pull requesty, komentáre, priradených ľudí a prílohy. Z organizácie
sa stane účet s rovnakým menom. Čo v LosOS Git nemá miesto (míľniky,
vydania, záznamy časovej osi ako „zavrel to“), je spočítané a pomenované v
správe, nikdy potichu zahodené. Nič z Forgejo sa nemaže a neúspešný beh
nič nezmení a skúsi sa znova pri ďalšom štarte.

`admin-ui/design-system/` (`tokens.css` a `losos.css`, plus obalový balík
`react/` pre claude.ai/design) je iba pre vývojársky stroj a už sa vôbec
neservíruje. Dodávaná SPA nesie vlastnú paletu v
`admin-ui/app/src/styles/tokens.css`, ktorú `index.css` znova publikuje ako
kľúče témy Tailwind v4. Dizajnový systém zostáva mimo Nix closure, pretože
*koreň* zdrojov `losos-admin-ui` je `admin-ui/app`. To je silnejšia záruka
než filter podľa názvu `design-system`, ktorý nahradil a ktorý by
premenovanie pokazilo.

**Jeden vzhľad pre administračné stránky, LosOS Cloud a LosOS Git**
(`admin-ui/themes/`). LosOS Cloud a LosOS Git sú stránky vlastného
zostavenia administračného rozhrania (`admin-ui/app/cloud/`,
`admin-ui/app/git/`), takže používajú tie isté komponenty aj `tokens.css`.
`tokens.css` je zámerne čisté CSS: téma LosOS Login ho dodáva bajt po bajte
ako `losos-tokens.css` a mapuje naň vlastné premenné, takže farba zmenená
tam pohne všetkými.

Vlastníci vidia tieto dve aplikácie ako **LosOS Cloud** a **LosOS Git**.
Obe dostanú logá nakreslené z `admin-ui/themes/brand/`.
`marks.py` zabalí `salmon.png`, živého lososa kisuča vystrihnutého z
fotografie NOAA Fisheries, do SVG a skopíruje ho do SPA a príručky. Zostavenie
z nich vyrenderuje každé PNG aj `.ico`. 31. októbra, podľa vlastného dátumu
návštevníka, ukážu administračné stránky a príručka namiesto toho
`plate.png`, tanier lososa z Matusovej vlastnej fotografie. `brand/CREDITS.md`
uvádza, odkiaľ obe obrázky pochádzajú. Stará pixel-artová ryba bola
odstránená pre obavy z autorských práv a späť sa nevracia. Zvyšok
značky upstreamu sa rieši konfiguráciou (žiadne súbory skeleton, žiadne
odkazy na pomoc ani registráciu, žiadne „Powered by“).

`admin-ui/themes/default.nix` sú čisté dáta, ktoré importuje každý volajúci.
Pody záťaží používajú `hostNetwork`, keďže lokálny klaster nemá žiadne CNI,
takže odpovedajú na loopbacku. Čo to stojí, vysvetľuje záludnosť o strážcovi
`lanOnly` v `.claude/CLAUDE.md`.

**Demo hostiteľ na Verceli** (`edge/vercel/`). Tretí Rust crate, mimo
flaku, ktorý spúšťa router registrátora ako jednu Vercel Function. Register je
jeden riadok `jsonb` v Neon Postgres (tokio-postgres cez rustls), s
`DATABASE_URL` z marketplace integrácie Vercelu, načítaný pred každou
požiadavkou a po nej uložený cez upsert. Pridáva dve trasy len pre demo
(`/status`, `/status/traefik`) a statickú stránku. Existuje pre prezentácie
a je určený na zmazanie. V `edge/registrar` potreboval iba dve zmeny.
`server::build` vracia router a zosúlaďovač bez poslucháča a časovača a
`serve` ho volá. `Registry::{export,import}` je snímka s vekmi, aby register
mohol putovať medzi procesmi, a `import` uplatní TTL heartbeatu, pretože
medzitým nebežal žiadny zosúlaďovač.

Tunel, Traefik, mesh ani brána Stripe tam bežať nemôžu, takže na tomto
hostiteľovi `/cluster/join` a `/market/*` odpovedajú 503 a `/tunnel-pin`
404. Zoznam `crates` v `devenv.nix` a CI ho lintujú a testujú ako ostatné dva
crates. `nix build` ho nikdy nevidí. Jeho `tests/postgres.rs` beží iba vtedy,
keď je nastavené `LOSOS_TEST_DATABASE_URL`, čo testovacia úloha v CI
poskytuje so službou Postgres. Zvyšok jeho testovacej sady beží nad úložiskom
v pamäti.

Jeho druhá statická stránka, `public/find.html`, je postup „find my box“
(nájdi moje zariadenie). Povolenie Local Network Access v Chrome jej dovolí čítať
`/setup/state.json` zariadenia z verejného originu. `modules/setup.nix` povoľuje
presne originy v `losos.setup.finderOrigins` (nginx map, overený v
`nix/tests/setup.nix`), nikdy `*`, pretože dokument je inventár LAN.

**Menný priestor volieb `losos.*`** (`options.nix`). Všetky nastavenia
špecifické pre projekt sú pod `options.losos`, s predvolenými hodnotami v
`defaults.nix`: `targetDrive`, `tpm.enable`, `sharingMyStorage`,
`git.enable`, `cloud.*`, `cluster.*` (pripojenie k mesh, výpočtové
okno), `shared.fscrypt.*`, `upgradeFlakeUri`, `backend.package`,
`admin.{enable,port,apiPort,tokenFile,ui}`, `proxy.*` (strana zariadenia v
master proxy) a `edge.*` (strana VPS, používaná cez výstup flaku
`nixosModules.edge`). **Moduly nikdy nečítajú holé `config.X`; čítajú
`config.losos.X`.** Toto je jediné miesto, kam pridávať nové voľby. Starší
kód nesprávne deklaroval voľby vnútri blokov `config`.

**Hardening** (`hardening.nix`, iba `install`). `losos.hardening.*` je
postavené z primitív KSPP namiesto obalenia upstreamového profilu, pretože
žiadny na obalenie nezostal. `profiles/hardened.nix` bol odstránený v 26.05
a `linux_hardened` je teraz `throw`. `hardening.enable` (predvolene true) je
vrstva, ktorá nič nestojí: parametre jadra, sysctl, čierna listina modulov,
`boot.tmp.useTmpfs`, dbus-broker a systemd sandboxing pre `nginx`
a `lososd`. mDNS je systemd-resolved v sandboxe jeho vlastnej upstream
jednotky (`modules/mdns.nix`). Čierna listina blokuje aj explicitný `modprobe`.
Obyčajné `boot.blacklistedKernelModules` to **nerobí**; zastaví iba
automatické načítanie cez aliasy. Štyri voliteľné príznaky nesú to, čo môže
niečo pokaziť: `apparmor`, `malloc`, `nosmt`, `usbguard`. Systém `iso` modul
neimportuje, pretože živé médium potrebuje squashfs. `nix/tests/hardening.nix`
overuje obe polovice, vrátane toho, že nič neblokuje funkcie jadra, ktoré
potrebujú k3s, rke2, containerd a Longhorn.

**Online rozširovanie** (`grow.rs`, `losos.storage.fillPercent`). Logický
zväzok zámerne nezaberá celú skupinu zväzkov, aby sa `/persist` dal zväčšiť
bez otvárania zariadenia. `/persist` *je* `/nix`, cez impermanence.
`losos-ctl grow` spustí lvextend, potom `cryptsetup resize`, potom
`resize2fs`, v tomto poradí, za behu. Poradie je celý argument správnosti a
chyba v ňom zlyhá potichu. Preto ho `grow.rs` overuje voči *plánu* a
`nix/tests/resize.nix` ho kontroluje od začiatku do konca.

**BIOS a banner na tty1** (`losos.bios`, `modules/console.nix`). Menu
firmvéru na inštalačnom ISO zapíše `losos.bios` do
`install-target.nix`. BIOS a UEFI sa dajú zvoliť explicitne. Automatická
detekcia zvolí BIOS, keď chýba `/sys/firmware/efi`. `--bios` a `--uefi`
zvolia bez menu. Menu existuje iba na inštalačnom ISO, nie v nainštalovanom
systéme, a po 30 s bez odpovede zvolí automatickú detekciu, pretože médium
má inštalovať bez obsluhy a `nix/tests/iso-boot.py` nikdy nepíše. Na médiu je
inštalátor služba na tty1, nie prihlasovací shell (`installer.nix`). Kde
Plymouth kreslí, beží pod úvodnou obrazovkou. Menu, kroky a výsledok
inštalácie idú na panel (`backend/src/install_screen.rs`) a klávesy
prichádzajú z `plymouth watch-keystroke`, lebo Plymouth počas kreslenia drží
klávesnicu. Úvodná obrazovka v textovom režime sa ukončí a inštalátor beží
ako text. Po inštalácii obrazovka zmizne a odkryje výpis na tty1 a shell
roota. `nix/tests/installer-screen.nix` nabootuje obe varianty. BIOS prepne
`boot.nix` na GRUB a pridá oddiel EF02 na prvý disk v `disko.nix`. ESP
zostáva v oboch prípadoch `/boot`. `console.nix` nahradí getty na tty1
službou s bannerom, ktorá ukazuje IPv4 adresu v LAN a `<hostName>.local`.
`splash.nix` (`losos.splash.enable`, predvolene zapnuté) je Plymouth s
témou LosOS (`modules/splash/losos.script`). Štart sa otvorí animáciou
loga: z okrajov obrazovky sa stiahne obrys na pásik pod názvom, losos
priplává vodorovne a natočí sa nahor vedľa nápisu „LosOS“ a šrafovanie
pásika sa potom plní podľa postupu štartu. Keď mu služba s bannerom pošle
tie isté riadky, logo plynulo vyjde hore a pod ním sa otvorí panel. Na konci štartu Plymouth nič neukončí, takže panel
zostáva na obrazovke. Banner sa pod ním ďalej kreslí na tty1, a ten je
vidno tam, kde Plymouth v prvých sekundách nenašiel displej a beží v
textovom režime. Pri BIOS-e zostáva grafický režim GRUB-u jadru
(`gfxpayloadBios = "keep"`), aby mal Plymouth framebuffer už v stage 1.
`startup-sound.nix` (`losos.startupSound.enable`, predvolene zapnuté)
zahrá posledných sedem sekúnd hudby z úvodného filmu, keď sa spustí služba
s bannerom, na každom výstupnom zariadení, ktoré ALSA nájde.
`modules/startup-sound/render.py` ho syntetizuje pri zostavení, takže v
repozitári nie je žiadna nahrávka. Nočný reštart a reštart po aktualizácii
zostávajú tiché: pri vypínaní kvôli reštartu sa v
`/var/lib/losos-startup-sound` zanechá značka a ďalší štart ju namiesto
hrania odstráni.

**Limity záznamov** (`logs.nix`, na zariadení aj na edge). `/var` je na
rovnakom zväzku ako `/nix`, takže záznam, ktorý rastie bez konca, skončí
nočnou prestavbou, ktorá nemá kde zostavovať. Žurnál má strop
`losos.logs.journalMaxUse` (512 MB) a mesiac. Kubelet každého klastra si
nechá tri súbory po 10 MiB na kontajner (`containerLogMaxSize`,
`containerLogMaxFiles`), čo pokrýva LosOS Cloud, LosOS Git aj aplikácie,
lebo zapisujú na stdout. logrotate otáča `audit.log` a `rebuild.log` démona
lososd pri 10 MB a pravidlu pre nginx, ktoré dodáva nixpkgs, pridáva hranicu
20 MB. Pravidlo tmpfiles maže záznamy zostavení Nixu staršie ako 30 dní,
ktoré zberač odpadu nechá tak. rke2 a libvirt otáčajú svoje súbory
záznamov samy a na edge `bitcoind` zapisuje len do žurnálu.
`nix/tests/invariants.nix` overuje strop žurnálu, obe pravidlá logrotate a strop
záznamov podov v oboch klastroch.

**Automatický upgrade a nočný reštart** (`updates.nix`). `system.autoUpgrade`
prestavuje z `losos.upgradeFlakeUri` o 03:00, predvolene
`git+file:///etc/nixos#install`. Pred zostavením spustí
`nixos-upgrade.service` príkaz `losos-ctl config --update` a lososd posunie
`/etc/nixos` na najnovší commit vetvy `losos.configRepo.upstream.branch` na
`losos.configRepo.upstream.url` (predvolene `main` repozitára na GitHube).
Fetch je plytký a výsledkom je jeden commit na vlastnej vetve zariadenia,
nie merge: jeho strom je publikovaný strom, do ktorého zariadenie vráti
svoje `modules/overrides.nix` a `modules/install-target.nix`. Apply a nočná
prestavba potom zostavujú ten istý strom, čo by URI `github:` tu nedalo:
Apply by zostavil staršie `/etc/nixos` a vrátil nočnú aktualizáciu späť.
lososd krok preskočí a hlási `kept`, keď sa od poslednej aktualizácie
zmenil iný súbor, takže ručná zmena pushnutá do LosOS Git sa nikdy
neprepíše. `nix/tests/invariants.nix` krok pripína.

:::warning[Obe cesty prestavby nechajte nečisté]
**Obe cesty prestavby sú zámerne `--impure`**: `system.autoUpgrade.flags`
a `rebuild_command` v lososd. Publikovaný strom neobsahuje ani
`modules/install-target.nix` tohto zariadenia, ani jeho `modules/overrides.nix`,
takže `flake.nix` importuje živé kópie z `/etc/nixos`, keď ich dokáže
prečítať, a to dokáže iba nečisté (impure) vyhodnotenie. Vynechajte príznak a
upgrade z `github:` prepne zariadenie s kľúčovým súborom do tvaru s TPM, zabudne
jeho disky a režim firmvéru a resetuje každé nastavenie. Pri čistom
vyhodnotení je `builtins.pathExists` na absolútnej ceste `false`, nie chyba.
Takže CI, `nix flake check` a inštalátor zostávajú čisté a vidia súbor zo
stromu alebo predvolené hodnoty. `nix/tests/invariants.nix` príznak pripína.
:::

`nix.gc` beží o 04:30 s `--delete-older-than 14d` a `boot.nix` obmedzuje
oba zavádzače na päť generácií. `/nix` *je* `/persist`, ESP má 500 MiB a
`linuxPackages_latest` tam väčšinu nocí pridá nové jadro. Bez oboch limitov
sa zariadenie samo zaplní a prepnutie o 03:00 zlyhá v kroku zavádzača.
`nix/tests/invariants.nix` overuje oboje už pri vyhodnotení.

**Fragment `#install` je nosný.** Bez neho `nixos-rebuild` hľadá
`nixosConfigurations.$(hostname)`. Tento flake exportuje iba `iso` a
`install`, takže každý nočný beh skončil s „flake does not provide
attribute“, na zariadení bez shellu, z ktorého by si to niekto všimol. Démon to mal
celý čas správne (`io_backend.rs`, `/etc/nixos#install`). Chybná bola iba
predvolená hodnota na strane NixOS.

Samostatný `midnight-reboot.timer` reštartuje bezpodmienečne o 00:07, s
`Persistent=true`, aby to zariadenie, ktoré bolo vypnuté, dobehlo.
