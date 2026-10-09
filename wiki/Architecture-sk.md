[English](Architecture) · **Slovenčina** · [Deutsch](Architecture-de)

# Architektúra

## Systémy vo flaku

| Výstup                        | Čo to je                                         |
| ----------------------------- | ------------------------------------------------ |
| `nixosConfigurations.iso`     | Živé inštalačné ISO (`options`, `disko`, `installer`) |
| `nixosConfigurations.install` | Nainštalované zariadenie (všetky moduly)         |
| `nixosModules.edge`           | Strana VPS [master proxy](Master-Proxy-sk) a mesh |

Všetky voľby projektu sú pod `losos.*` v `modules/options.nix`, s
predvolenými hodnotami v `defaults.nix`. Moduly čítajú `config.losos.*`.

## Impermanence

`/` je tmpfs. `/persist` je ext4 šifrovaný cez LUKS. `impermanence` do
systému bind-mountuje z `/persist` tieto cesty: `/nix`, `/var`, `/etc/ssh`,
`/etc/keys`, `/etc/nixos`, `/etc/rancher`, dva dátové domovské adresáre a
`machine-id`.

**Všetko, čo na tomto zozname nie je, sa reštartom stratí.** Nový stav
pridajte do `modules/impermanence.nix`.

## Riadiaca vrstva

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

## Pracovné záťaže

Nextcloud a Forgejo bežia vo vlastnom k3s klastri zariadenia
(`losos.<svc>.mode = "container"`) alebo natívne na hostiteľovi. Pody
používajú `hostNetwork`. nginx je jediná služba na verejných portoch a
smeruje podľa cesty.

## Rozloženie repozitára

```
modules/                 NixOS modules; options in options.nix
backend/                 lososd + losos-ctl (Rust)
backend-registrar/       master-proxy edge registrar (Rust)
admin-ui/app/            the admin SPA
admin-ui/themes/         LosOS cloud and LosOS Git: themes on the SPA's tokens.css, logos, names
admin-ui/design-system/  dev-only tokens and React wrapper; not shipped
tests/                   NixOS VM tests
docs/                    security model, design specs and plans
wiki/                    source of this wiki
```

`CLAUDE.md` je skomprimovaná podoba tejto stránky a záludností zo stránky
[Vývoj](Development-sk#nástrahy-ktoré-hryznú-potichu).

## Do hĺbky

Jeden odsek na každý mechanizmus: prečo je ktorý kus tam, kde je, a čo tam
pokazí dobre mienená zmena.

**Dva systémy NixOS zdieľajú jednu sadu modulov** (`flake.nix`).

- `iso` je minimálne živé ISO. Nesie rozloženie `disko`, aby mohlo
  naformátovať cieľový disk a zaregistrovať šifrovacie kľúče, a importuje
  iba `options.nix`, `disko.nix` a `installer.nix`.
- `install` je systém nainštalovaný na disku. Importuje všetko:
  `options`, `configuration`, `impermanence`, `disko`, `boot`, `services`,
  `nextcloud-common`, `containers`, `daemon`, `overrides`, `updates`,
  `defaults`. Flake odovzdáva `specialArgs.self = self`, aby `defaults.nix`
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
dalo zotaviť zo zablokovania; rovnaké rozhodnutie robí `keyring.nix`.

Ak inštalačné médium nevidí `/dev/tpmrm0`, alebo s `losos-ctl install
--no-tpm`, je zariadenie v režime kľúčového súboru. `install-target.nix` nastaví
`tpm.enable = false` a kľúčový súbor ide do initrd ako
`/crypto_keyfile.bin` na nešifrovanom ESP. Dva tvary crypttab sa navzájom
vylučujú: systemd-cryptsetup, ktorý dostane kľúčový súbor *aj* `tpm2-device=`,
číta súbor ako zapečatený blob. `tests/invariants.nix` pripína predvolenú
hodnotu a overuje, že cesta s TPM nedeklaruje žiadne tajomstvo v initrd.
`tests/tpm.nix` ju celú nabootuje pod swtpm.

Vzdialené URI `github:` pre upgrade vyhodnocuje strom bez
`install-target.nix`. Preto `flake.nix` uprednostní živé
`/etc/nixos/modules/{install-target,overrides}.nix` vždy, keď ich dokáže
prečítať, a obe cesty prestavby (`updates.nix`, `supervisor.rs`) kvôli tomu
odovzdávajú `--impure`. Pozrite odsek o automatickom upgrade nižšie.

**Dve izolované dátové domény, žiadny shell** (`configuration.nix`). `notshared`
(uid 1000) vlastní Nextcloud. `shared` (uid 1001) vlastní úložisko poskytnuté
do mesh. Oba domovské adresáre majú režim `700`, každý s vlastnou primárnou
skupinou, takže žiadny z používateľov nemôže čítať súbory toho druhého.
`isNormalUser` bez explicitnej `group` dá oboch do `users` a domovský adresár
s `750` potom tejto skupine povolí r-x. To potichu rušilo izoláciu, kým to
nezachytil `tests/impermanence.nix`. Žiadny z používateľov nemá heslo a
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

`losos-ctl` je tenké fasádne CLI. Zachováva staré podpríkazy a JSON výstup
(`state`, `change --mode`, `status`, `settings`, `apply` (stdin),
`factory-reset`; `--json` nerobí nič) a každé volanie posúva lososd cez
D-Bus. `losos-ctl install` zostáva lokálny, pretože inštalátor na ISO beží
ako root a zbernicu nepotrebuje.

Administračné UI (`admin-ui/app/`, zabalené ako `losos-admin-ui` cez
`buildNpmPackage`) je SPA v React 19, Vite a Tailwind v4 na skutočných
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
(`flake/options-doc.nix`, `modules/config-repo.nix`,
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
(`tests/admin-ui.nix` odovzdáva kontrolu `losos-options-doc`) a zlyhá pri
každom riadku, ktorý panel nevie vykresliť. `POST /api/apply` overí každý
riadok proti dokumentu (`check_body`). Odmietne nedeklarované kľúče, kľúče
iba na čítanie (hodnoty pevne dané inštalátorom, balíky), hodnoty zlého typu
a akékoľvek `${` a pomenuje kľúč, ktorý odmietol.

Každé použitie zmien (apply), zmena režimu a reset je commit v `/etc/nixos`,
repozitári, ktorý vytvoril inštalátor. Zosúlaďovacie vlákno v lososd sa
dopytuje každých 30 s. Vytvorí administrátorov účet vo Forgejo (`notshared`,
s heslom vlastníka, udržiavaným v súlade pri prihlásení a nastavení hesla) a
súkromný repozitár `losos-config`. Robí to cez účet bota, ktorého token
vyrobí štartovací skript Forgejo (`flake/forgejo-bootstrap.nix`, token v
`/var/lib/forgejo/.losos-token`). Keď Forgejo zaostáva, pushne. Keď klon
pushol dopredu, urobí fast-forward, nový `overrides.nix` prepustí rovnakou
bránou a spustí prestavbu. Rozdvojenú históriu nahlási a nikdy sa jej
nedotkne. Za tým všetkým zámerne **nie je žiadny runner Forgejo Actions**.

Panel Rozšírené odkazuje na šestnásť nastavení, ktoré vlastnia ostatné panely,
namiesto toho, aby ich upravoval druhýkrát. `OWNED` v
`admin-ui/app/src/lib/option-value.ts` a `OWNED_NAMES` v `lib/api.ts` sú
tento zoznam a `buildOverridesNix` podľa neho filtruje ďalšie kľúče, aby
súbor nikdy neobsahoval kľúč dvakrát. Dôležité sú tu dve vlastnosti parsera.
`parse_line` v `overrides.rs` odreže hodnotu pri prvom `#` *mimo* úvodzoviek;
kedysi ju odrezával aj v nich, čo pokazilo predvolené `upgradeFlakeUri`. A
zoznam je jeden riadok `[ "a" "b" ]`, iba reťazce.

**Pracovné záťaže a jediné predné dvere** (`modules/containers.nix`,
`modules/workloads.nix`, `modules/cluster.nix`,
`modules/nextcloud-common.nix`). Rootless Podman je preč a systemd-nspawn
tiež. Keď `losos.<svc>.mode == "container"`, Nextcloud a Forgejo bežia ako
Kubernetes záťaže vo vlastnom lokálnom k3s klastri zariadenia. Zdieľaná
konfigurácia Nextcloudu stále žije v `nextcloud-common.nix`, takže natívny a
kontajnerový režim sa nemôžu rozísť. nginx je jediný proces, ktorý drží
verejné porty. Smeruje podľa cesty na mDNS mene zariadenia
(`<hostName>.local:80/nextcloud`, `:80/forgejo`) a na tom istom vhoste `:80`
servíruje administračnú SPA.

`admin-ui/design-system/` (`tokens.css` a `losos.css`, plus obalový balík
`react/` pre claude.ai/design) je iba pre vývojársky stroj a už sa vôbec
neservíruje. Dodávaná SPA nesie vlastnú paletu v
`admin-ui/app/src/styles/tokens.css`, ktorú `index.css` znova publikuje ako
kľúče témy Tailwind v4. Dizajnový systém zostáva mimo Nix closure, pretože
*koreň* zdrojov `losos-admin-ui` je `admin-ui/app`. To je silnejšia záruka
než filter podľa názvu `design-system`, ktorý nahradil a ktorý by
premenovanie pokazilo.

**Jeden vzhľad pre domovskú stránku, Nextcloud a Forgejo** (`admin-ui/themes/`).
`tokens.css` je zámerne čisté CSS. Témy oboch aplikácií ho dodávajú bajt po
bajte ako `losos-tokens.css` a mapujú naň vlastné premenné, takže farba
zmenená tam pohne všetkými tromi.

Nextcloud dostane *priečinok témy*: `nextcloud-stack.nix` skopíruje
`themes/losos/` do balíka a konfigurácia nastaví `theme = "losos"`. Nextcloud
hľadá témy voči svojmu skutočnému koreňu servera, takže symbolický odkaz
vedľa balíka nikdy nenájde. Forgejo dostane `theme-losos-{auto,light,dark}.css`
v `$FORGEJO_CUSTOM/public/assets/css/`, natívne prepojené cez tmpfiles a
skopírované vstupným bodom obrazu, s `DEFAULT_THEME = losos-auto`. Forgejo to
vtlačí účtom pri ich vytvorení, takže existujúci účet si ponechá starú tému,
kým ju jeho vlastník neprepne.

Vlastníci vidia tieto dve aplikácie ako **LosOS cloud** a **LosOS Git**.
`defaults.php` v priečinku témy pomenúva Nextcloud a `APP_NAME` vo Forgejo
pomenúva Forgejo. Obe dostanú logá nakreslené z `admin-ui/themes/brand/`.
`marks.py` zabalí `salmon.png`, živého lososa kisuča vystrihnutého z
fotografie NOAA Fisheries, do SVG a skopíruje ho do SPA a príručky. Zostavenie
z nich vyrenderuje každé PNG aj `.ico`. 31. októbra, podľa vlastného dátumu
návštevníka, ukážu administračné stránky a príručka namiesto toho
`plate.png`, tanier lososa z Matusovej vlastnej fotografie. `brand/CREDITS.md`
uvádza, odkiaľ obe obrázky pochádzajú. Stará pixel-artová ryba bola
odstránená pre obavy z autorských práv a späť sa nevracia. Zvyšok
značky upstreamu sa rieši konfiguráciou (žiadne súbory skeleton, žiadne
odkazy na pomoc ani registráciu, žiadne „Powered by“) a dvoma prepísanými
šablónami Forgejo.

Dva detaily sa ľahko prehliadnu. Filter hlavičky Nextcloudu invertuje každé
logo, ktoré považuje za svoje vlastné biele, preto `server.css` nastavuje
`--image-logoheader-custom`. A aplikácia theming číta niekoľko súborov
`core/img/` podľa absolútnej cesty, preto ich `nextcloud-stack.nix` v balíku
prepíše. Preto obraz nastavuje `integrity.check.disabled`, rovnako ako to už
robí `services.nextcloud`. `admin-ui/themes/default.nix` sú čisté dáta, ktoré
importujú všetci traja volajúci, z rovnakého dôvodu ako `nextcloud-stack.nix`.
Pody záťaží používajú `hostNetwork`, keďže lokálny klaster nemá žiadne CNI,
takže odpovedajú na loopbacku. Čo to stojí, vysvetľuje
[záludnosť](Development-sk#nástrahy-ktoré-hryznú-potichu) o strážcovi `lanOnly`.

**Demo hostiteľ na Verceli** (`edge-vercel/`). Tretí Rust crate, mimo
flaku, ktorý spúšťa router registrátora ako jednu Vercel Function. Register je
jeden riadok `jsonb` v Neon Postgres (tokio-postgres cez rustls), s
`DATABASE_URL` z marketplace integrácie Vercelu, načítaný pred každou
požiadavkou a po nej uložený cez upsert. Pridáva dve trasy len pre demo
(`/status`, `/status/traefik`) a statickú stránku. Existuje pre prezentácie
a je určený na zmazanie. V `backend-registrar` potreboval iba dve zmeny.
`server::build` vracia router a zosúlaďovač bez poslucháča a časovača a
`serve` ho volá. `Registry::{export,import}` je snímka s vekmi, aby register
mohol putovať medzi procesmi, a `import` uplatní TTL heartbeatu, pretože
medzitým nebežal žiadny zosúlaďovač.

Tunel, Traefik, mesh ani brána Stripe tam bežať nemôžu, takže na tomto
hostiteľovi `/cluster/join` a `/market/*` odpovedajú 503 a `/noise-public-key`
404. Zoznam `crates` v `devenv.nix` a CI ho lintujú a testujú ako ostatné dva
crates. `nix build` ho nikdy nevidí. Jeho `tests/postgres.rs` beží iba vtedy,
keď je nastavené `LOSOS_TEST_DATABASE_URL`, čo testovacia úloha v CI
poskytuje so službou Postgres. Zvyšok jeho testovacej sady beží nad úložiskom
v pamäti.

Jeho druhá statická stránka, `public/find.html`, je postup „find my box“
(nájdi moje zariadenie). Povolenie Local Network Access v Chrome jej dovolí čítať
`/setup/state.json` zariadenia z verejného originu. `modules/setup.nix` povoľuje
presne originy v `losos.setup.finderOrigins` (nginx map, overený v
`tests/setup.nix`), nikdy `*`, pretože dokument je inventár LAN.

**Menný priestor volieb `losos.*`** (`options.nix`). Všetky nastavenia
špecifické pre projekt sú pod `options.losos`, s predvolenými hodnotami v
`defaults.nix`: `targetDrive`, `tpm.enable`, `sharingMyStorage`,
`forgejo.enable`, `nextcloud.*`, `cluster.*` (pripojenie k mesh, výpočtové
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
`boot.tmp.useTmpfs`, dbus-broker a systemd sandboxing pre `nginx`,
`avahi-daemon` a `lososd`. Čierna listina blokuje aj explicitný `modprobe`.
Obyčajné `boot.blacklistedKernelModules` to **nerobí**; zastaví iba
automatické načítanie cez aliasy. Štyri voliteľné príznaky nesú to, čo môže
niečo pokaziť: `apparmor`, `malloc`, `nosmt`, `usbguard`. Systém `iso` modul
neimportuje, pretože živé médium potrebuje squashfs. `tests/hardening.nix`
overuje obe polovice, vrátane toho, že nič neblokuje funkcie jadra, ktoré
potrebujú k3s, rke2, containerd a Longhorn.

**Online rozširovanie** (`grow.rs`, `losos.storage.fillPercent`). Logický
zväzok zámerne nezaberá celú skupinu zväzkov, aby sa `/persist` dal zväčšiť
bez otvárania zariadenia. `/persist` *je* `/nix`, cez impermanence.
`losos-ctl grow` spustí lvextend, potom `cryptsetup resize`, potom
`resize2fs`, v tomto poradí, za behu. Poradie je celý argument správnosti a
chyba v ňom zlyhá potichu. Preto ho `grow.rs` overuje voči *plánu* a
`tests/resize.nix` ho kontroluje od začiatku do konca.

**BIOS a banner na tty1** (`losos.bios`, `modules/console.nix`). Menu
firmvéru na inštalačnom ISO zapíše `losos.bios` do
`install-target.nix`. BIOS a UEFI sa dajú zvoliť explicitne. Automatická
detekcia zvolí BIOS, keď chýba `/sys/firmware/efi`. `--bios` a `--uefi`
zvolia bez menu. Menu existuje iba na inštalačnom ISO, nie v nainštalovanom
systéme, a po 30 s bez odpovede zvolí automatickú detekciu, pretože médium
má inštalovať bez obsluhy a `tests/iso-boot.py` nikdy nepíše. BIOS prepne
`boot.nix` na GRUB a pridá oddiel EF02 na prvý disk v `disko.nix`. ESP
zostáva v oboch prípadoch `/boot`. `console.nix` nahradí getty na tty1
službou s bannerom, ktorá ukazuje IPv4 adresu v LAN a `<hostName>.local`.

**Automatický upgrade a nočný reštart** (`updates.nix`). `system.autoUpgrade`
prestavuje z `losos.upgradeFlakeUri` o 03:00. Predvolené
`git+file:///etc/nixos#install` iba konzistentne posúva systém dopredu a
nesťahuje nové nixpkgs. Pre skutočný upgrade nastavte URI `github:`.

**Obe cesty prestavby sú zámerne `--impure`**: `system.autoUpgrade.flags`
a `rebuild_command` v lososd. Publikovaný strom neobsahuje ani
`modules/install-target.nix` tohto zariadenia, ani jeho `modules/overrides.nix`,
takže `flake.nix` importuje živé kópie z `/etc/nixos`, keď ich dokáže
prečítať, a to dokáže iba nečisté (impure) vyhodnotenie. Vynechajte príznak a
upgrade z `github:` prepne zariadenie s kľúčovým súborom do tvaru s TPM, zabudne
jeho disky a režim firmvéru a resetuje každé nastavenie. Pri čistom
vyhodnotení je `builtins.pathExists` na absolútnej ceste `false`, nie chyba.
Takže CI, `nix flake check` a inštalátor zostávajú čisté a vidia súbor zo
stromu alebo predvolené hodnoty. `tests/invariants.nix` príznak pripína.

`nix.gc` beží o 04:30 s `--delete-older-than 14d` a `boot.nix` obmedzuje
oba zavádzače na päť generácií. `/nix` *je* `/persist`, ESP má 500 MiB a
`linuxPackages_latest` tam väčšinu nocí pridá nové jadro. Bez oboch limitov
sa zariadenie samo zaplní a prepnutie o 03:00 zlyhá v kroku zavádzača.
`tests/invariants.nix` overuje oboje už pri vyhodnotení.

**Fragment `#install` je nosný.** Bez neho `nixos-rebuild` hľadá
`nixosConfigurations.$(hostname)`. Tento flake exportuje iba `iso` a
`install`, takže každý nočný beh skončil s „flake does not provide
attribute“, na zariadení bez shellu, z ktorého by si to niekto všimol. Démon to mal
celý čas správne (`io_backend.rs`, `/etc/nixos#install`). Chybná bola iba
predvolená hodnota na strane NixOS.

Samostatný `midnight-reboot.timer` reštartuje bezpodmienečne o 00:07, s
`Persistent=true`, aby to zariadenie, ktoré bolo vypnuté, dobehlo.
