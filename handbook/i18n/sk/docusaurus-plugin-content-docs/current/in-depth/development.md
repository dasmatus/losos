---
title: Vývoj
sidebar_position: 10
mdx:
  format: md
---

# Vývoj

## Shell

```sh
devenv shell        # or: direnv allow
devenv test         # pin checks, lint, rust tests, flake eval (no builds)
```

`nix develop` dáva len toolchain, pre tých, čo nepoužívajú devenv.

| Skript                         | Čo robí |
| ------------------------------ | ---- |
| `fmt` · `lint` · `test-rust`   | Formátuje, lintuje a testuje oba Rust crates |
| `check-flake` · `check-eval`   | Vyhodnotí flake; vynúti úplné zlúčenie modulov |
| `check-pins`                   | Zlyhá, ak `flake.lock` a `devenv.lock` pripínajú rôzne nixpkgs |
| `build-pkgs` · `build-iso`     | Zostaví balíky; zostaví inštalačné ISO |
| `build-images` · `build-media` | OCI obrazy; demo QCOW2 a ISO s closure (veľké, voliteľné) |
| `vm-tests`                     | Všetky testy NixOS vo VM (potrebuje `/dev/kvm`) |

`devenv test` nezostavuje balíky ani inštalačné ISO a nespúšťa testy vo VM.
Na to sú samostatné voliteľné skripty.

## Rust

```sh
cargo test   --manifest-path backend/Cargo.toml
cargo clippy --manifest-path backend/Cargo.toml --all-targets -- -D warnings
```

To isté platí pre `backend-registrar/`. Oba crates musia prejsť cez clippy aj
rustfmt bez výhrad. Nix balíky nastavujú `doCheck = false`, takže
`nix build` nespúšťa žiadne testy.

Pri pushi do `main` spustí CI job `cargo fmt` a výsledok commitne. Clippy sa
automaticky neopravuje.

Ďalší job potom prepíše históriu tak, aby žiadny commit neuvádzal Claude ako
spoluautora ani neodkazoval na session na claude.ai, a force-pushne vetvy a
tagy, ktoré sa posunuli. Ak sa vám `main` medzitým zmenil, `git pull --rebase`
prevezme prepísané commity čisto. Obyčajný `git pull` by obe histórie zlúčil.

## Admin UI

V `admin-ui/app/`:

```sh
npm run dev           # set LOSOS_API_ORIGIN to a real box
npm run typecheck
npm run test:browser
```

`nix build .#losos-admin-ui` spustí `npm ci` a potom `tsc --noEmit && vite build`,
takže typová kontrola je súčasťou balíka. `npm run test:browser` je
playwright kontrola, ktorú obaľuje `.#checks.x86_64-linux.losos-admin-ui`.

`tests/advanced.browser.mjs` vykreslí každú voľbu, ktorú zariadenie deklaruje. Mimo
nixu číta `tests/fixtures/options.json`, kópiu dokumentu, ktorý generuje
`flake/options-doc.nix`; po zmene `modules/options.nix` ho obnovte pomocou

```sh
nix build --no-link --print-out-paths .#checks.x86_64-linux.losos-options-doc
cp "$(nix build --no-link --print-out-paths .#checks.x86_64-linux.losos-options-doc)" \
   admin-ui/app/tests/fixtures/options.json
```

`nix flake check` a `tests/admin-ui.nix` používajú čerstvo vygenerovaný
dokument, takže zastaraná fixture ovplyvní len beh na vývojárskom počítači.

## Testy vo VM

CI ich spustiť nevie (hostované runnery nemajú KVM). Spustite ich lokálne
pred zlúčením zmien v moduloch, démonovi alebo inštalátore:

```sh
devenv shell vm-tests
nix build .#checks.x86_64-linux.losos-admin-daemon
nix build .#checks.x86_64-linux.losos-install
```

Lokálne zostavte aj closure systému:

```sh
nix build .#nixosConfigurations.install.config.system.build.toplevel
```

Jedna flake kontrola nie je VM. `losos-invariants` (`tests/invariants.nix`)
vyhodnotí publikovanú konfiguráciu `install` a overí hodnoty volieb, o ktoré
appliance nesmie prísť tým, že sa posunie nejaký default: garbage collection,
strop bootovacieho menu, režim odomykania, fragment `#install`.
`nix flake check --no-build` ho spúšťa, takže eval job v CI na ňom zlyhá bez
akýchkoľvek nákladov na build.

## Pull requesty

Šablóna v `.github/pull_request_template.md` určuje tvar popisu:
text Before / After, tabuľka so screenshotmi, How, Tested, poznámky pre
recenzenta. Zachovajte každú sekciu.

Screenshoty sú povinné pre každú zmenu, ktorú človek uvidí: admin UI a
sprievodca, inštalátor a banner na tty1, témy Nextcloudu a Forgejo, stránky
príručky a dokumentácie. Pre každú obrazovku urobte jeden Before a jeden After
pri rovnakej veľkosti okna a v rovnakom stave, aby jediným rozdielom bola
samotná zmena, a pretiahnite ich do tabuľky. Zmena bez viditeľného prejavu
uvedie pod tým nadpisom "No visible change." a prečo.

Pod Tested uveďte, čo bežalo a čo nie. Testy vo VM potrebujú KVM, tak to
napíšte, keď nebežali.

## Lock súbory a hashe

- `flake.lock` pripína nixpkgs, ktoré zostavujú appliance; `devenv.lock` tie,
  ktoré ho lintujú a testujú. Aktualizujte ich spolu.
- Po zmene `Cargo.lock` alebo `admin-ui/app/package-lock.json` aktualizujte
  `cargoHash` alebo `npmDepsHash` vo `flake/packages.nix`. Hash admin UI je
  aj v `tests/admin-ui.nix`.

## Príručka

Tieto stránky sú súčasťou príručky, `handbook/` v repozitári. Ako ju
zobraziť a zostaviť, nafotiť jej obrázky a preložiť stránku, opisuje
[Pre vývojárov](/reference/for-developers.md). Každý push do `main` ju
publikuje a každé zariadenie si zostaví vlastnú kópiu.

## Nástrahy, ktoré hryznú potichu

Za každú z nich sa raz zaplatilo. `CLAUDE.md` nesie jednoriadkové pravidlo
a toto je zdôvodnenie za ním.

- **Tri nastavenia hardeningu sa zámerne *neaplikujú***, hoci všetky tri sú
  v každom checkliste, z ktorého budete mať chuť odpisovať. `rp_filter` je `2`
  (voľný), nie `1`. Striktný režim zahadzuje multicastové odpovede, vďaka
  ktorým je zariadenie bez SSH dosiahnuteľné, a Calico pod ním tiež nefunguje.
  `user.max_user_namespaces` zostáva nenulové, lebo nula zastaví oba kubelety
  aj containerd. `/tmp` nie je `noexec`, lebo tam bežia nix buildy a nočný
  bezobslužný rebuild je jediný spôsob, ako sa zariadenie dokáže samo opraviť.
  `tests/hardening.nix` overuje, že všetky tri chýbajú, takže "oprava" jedného
  sfarbí test na červeno namiesto toho, aby zo zariadenia urobila tehlu.
- **`fileSystems."/tmp"` nič nepripojí.** NixOS maskuje `tmp.mount`, pokiaľ
  nie je nastavené `boot.tmp.useTmpfs`. Deklarácia nevytvorí žiadne pripojenie
  ani chybu a `/tmp` zdedí voľby koreňového súborového systému. Prísť na to
  stálo jedno kolo testov vo VM.
- **Súbor na preload alokátora je `/etc/ld-nix.so.preload`**, ktorý číta
  upravený loader NixOS. Štandardný glibc `/etc/ld.so.preload` tu nikdy
  neexistuje. Overenie na štandardnej ceste prejde, keď je alokátor vypnutý,
  a zlyhá, keď je zapnutý.
- **`losos-ctl grow` spúšťa tri príkazy a záleží len na ich poradí.**
  `resize2fs` sa pýta na veľkosť LUKS *mapovania*. Ak beží pred `cryptsetup
  resize`, prečíta veľkosť pred zväčšením, vypíše "Nothing to do!" a skončí
  s 0. A `lvextend -l 77` bez `+` je absolútny počet extentov, ktorý väčší
  zväzok *zmenší* a pripojený ext4 na ňom zničí.
  - `lososd` potrebuje lvm2, cryptsetup a e2fsprogs vo svojej unit `path`,
    inak prvý krok zlyhá s "No such file or directory". To isté platí pre
    `curl`, ktorý volá `GET /api/apps/search`. `path` PATH **nahrádza**, takže
    žiadny binárny súbor v ňom nie je náhodou a chýbajúci sa k vlastníkovi
    dostane ako funkcia, ktorá nikdy nefunguje a nikdy nepovie prečo.
  - `nixos-rebuild` je v tom zozname z rovnakého dôvodu. systemd-run
    vyhľadáva holý príkaz v PATH *volajúceho*, takže bez neho každé Použiť
    zlyhalo skôr, než rebuild unit vôbec existovala.
  - lososd nesmie dostať `ProcSubset=pid`. Skryje `/proc/devices` a vgs bez
    neho skončí so 4.
  - `tests/invariants.nix` pripína obe posledné veci.
- **LUKS keyfile je hex text a musí zostať bez NUL.** disko odovzdá
  `passwordFile` príkazu `luksFormat` ako `<(echo -n "$(cat FILE)")`, teda
  substitúciu príkazu, ktorá zahodí NUL bajty aj koncový nový riadok. Initrd
  (`/crypto_keyfile.bin`) a `systemd-cryptenroll --unlock-key-file` čítajú
  súbor surovo. Inštalátor kedysi zapisoval 4096 náhodných bajtov, takže
  takmer pri každej inštalácii sa zväzok naformátoval jedným kľúčom a odomykal
  iným. Na túto chybu nič nenarazilo, kým `tests/tpm.nix` nezaregistroval čip.
  `ensure_keyfile` teraz zapisuje 2048 náhodných bajtov ako 4096 hex znakov.
  Ne"sprísňujte" ho späť na surové bajty a nedávajte doň nový riadok.
- **LUKS key file nesmie ležať pod `/root` ani `/home`.** `lososd` beží
  s `ProtectHome=true`, zámerne, aby skompromitovaný obslužný kód požiadaviek
  nemohol čítať ani jednu dátovú doménu. To skryje aj `/root`. `cryptsetup
  resize` potom zlyhá s "Failed to open key file", *až potom*, čo `lvextend`
  zväzok už zväčšil. `/etc/keys/persist-keyfile` je cesta, na ktorej sa všetko
  zhoduje. `disko.nix` z nej formátuje, inštalátor ju drží vnútri `/persist`
  na oboch cestách odomykania, `daemon.nix` ju odovzdáva ako
  `LOSOS_LUKS_KEYFILE` a `tests/resize.nix` ju používa namiesto vlastnej
  fixture, takže tieto štyri sa nemôžu nepozorovane rozísť.
- **`lososd` sa reštartuje uprostred rebuildu.** `nixos-rebuild switch`
  reštartuje zmenenú `lososd.service` a zabije vlákno strážcu, ktoré rebuild
  sledovalo. `Daemon.startSupervisor` sa pri štarte démona znovu pripojí: ak
  `state.json` hovorí `Building`, spustí nového strážcu pre
  `losos-rebuild-<job>`. Bez toho rebuild navždy ukazuje `building`.
  Opätovné pripojenie neodstraňujte.
- **Admin cesty zámerne odmietajú loopback** (`lanOnly` v
  `containers.nix`). Prevádzka tunela master proxy prichádza na predný vhost
  *zo 127.0.0.1* (`local_addr` rathole), takže ochrana len pre LAN na `/`,
  `/assets/`, `/setup/` a `/api/` nesmie povoliť loopback.
  `curl localhost/` na zariadení alebo v teste vo VM dostane 403 podľa návrhu.
  "Oprava" cez `allow 127.0.0.1` vystaví celé admin rozhranie internetu
  vždy, keď je zapnuté `losos.proxy.enable`. Testujte priamo proti :8082
  lososd, alebo curlom so zdrojovou adresou z LAN.
- **Heslo vlastníka odomyká admin stránky a sudcom je Nextcloud.**
  `POST /api/sign-in` (`backend/src/signin.rs`) pošle heslo Nextcloudu cez
  loopback (`$LOSOS_NEXTCLOUD_LOGIN_URL`, nastavené pre každý režim v
  `daemon.nix`) v hlavičke Basic a pri 200 odpovie admin tokenom.
  lososd zámerne neuchováva hash hesla, lebo druhá kópia by sa rozišla v
  momente, keď by si ho vlastník zmenil v Nextcloude. Cenou je, že cesta
  odpovedá 503, kým Nextcloud nebeží, a práve na to slúži vytlačený
  náhradný kľúč. Preto neodstraňujte cestu s kľúčom z dialógu odomknutia
  a nerobte zo sondy prihlásenia `curl -u`, ktorý by dal heslo do argv. Nové
  heslo potrebuje 12+ znakov, obe veľkosti písmen, číslicu a symbol
  (`setup::validate_password`). Prihlásenie kontroluje len tvar kandidáta
  (`signin::validate_candidate`). Čokoľvek prísnejšie by zamklo vlastníka,
  ktorého heslo bolo nastavené podľa starších pravidiel.
- **Nextcloud dôveruje vlastnej IP adrese zariadenia cez jednu nginx hlavičku.**
  Lokácia `/nextcloud` nastavuje `X-Losos-Server-Addr $server_addr` a
  `losos.config.php` v pode pridá túto hodnotu pri každej požiadavke do
  `trusted_domains`, len IP literály. Tak sa na libvirt VM, ktorá nedostane
  mDNS meno, dá dostať cez `http://192.168.122.x/nextcloud` bez "Untrusted
  domain". Nenahrádzajte to wildcardom `192.168.*`. `*` v Nextcloude
  zodpovedá `[-.a-zA-Z0-9]*`, takže dôveryhodné by bolo aj
  `192.168.attacker.example`. `localhost` a `127.0.0.1` nepotrebujú nič,
  lebo im Nextcloud dôveruje vždy.
- **Dlaždice aplikácií v admin UI odkazujú cez `index.php`.** Nextcloud
  odpovedá na `/nextcloud/index.php/apps/<id>/` v každej konfigurácii.
  Krátky tvar `/nextcloud/apps/<id>/` funguje len vtedy, keď Apache v pode
  prepisuje pekné URL, a bez toho to bola Apache 404 priamo z admin domovskej
  stránky. `admin-ui/app/src/lib/apps.ts` preto používa dlhý tvar.
  Ne"upratujte" dlaždice späť na krátky.
- **Ručne písané widgety bežia v `/widget-frame/`, nikdy nie v admin stránke.**
  `<iframe sandbox="allow-scripts" src="/widget-frame/">` vykresľuje HTML a
  skript vlastníka (`backend/src/look.rs`, `GET/POST /api/look*`). Rámec
  je v `admin-ui/app/src/widgets/hand-frame.tsx` a jeho stránka v
  `admin-ui/app/public/widget-frame/`. Nosné sú dve veci. Nie je tam
  `allow-same-origin`, takže rámec je nepriehľadný origin bez tokenu a
  bez API. A vetva `~^/widget-frame/` v nginx mape hlavičiek v
  `containers.nix` je jediná cesta obsluhovaná pod benevolentnou CSP.
  `<iframe srcdoc>` alebo dokument `blob:` by zdedil prísnu politiku admin
  stránky a inline skript by rovnako odmietol, takže rámec na ne
  "nezjednodušujte". `tests/front-vhost.nix` overuje túto vetvu aj to, že
  prísna politika stále pokrýva všetko ostatné.
- **Admin token vytvára lososd, nie NixOS.** lososd zapíše
  `losos.admin.tokenFile` (predvolene `/var/secrets/losos-admin-token`,
  trvalé cez `/var`) pri prvom štarte, ak chýba, ako náhodnú hodnotu
  so 64 hex znakmi a právami 0600. NixOS ho nespravuje. Nepokúšajte sa
  ho deklarovať ako cestu v store.
- **`/etc/rancher` musí zostať trvalé** (`impermanence.nix`). `/var` pokrýva
  väčšinu stavu oboch inštancií Kubernetes, ale agent pri prvom pripojení
  zapíše `/etc/rancher/node/password` a server si uloží jeho hash podľa mena
  uzla. Na tmpfs koreni sa ten súbor pri každom boote vygeneruje nanovo a
  server potom opätovné pripojenie odmietne ("Node password
  rejected"). Vynechajte ten riadok a zariadenie pri prvom reštarte po registrácii
  bez slova vypadne z mesh.
- **`--disable`, `--flannel-backend` a `--disable-network-policy` sú
  príznaky len pre server.** `k3s agent` na neznámom príznaku natvrdo zlyhá,
  takže odovzdanie ktoréhokoľvek z nich agentovi pošle unit do nekonečnej
  slučky pádov, na zariadení bez shellu. Vlastný test nixpkgs
  `nixos/tests/rancher/multi-node.nix` dáva svojim serverovým uzlom `disable`
  a agentovi ani jedno a popis `role` v rke2 hovorí to isté. Každý príznak
  len pre server podmieňujte rolou.
- **Dve inštancie Kubernetes sú zámerne samostatné clustre.**
  `services.k3s` (rola `server`) prevádzkuje Nextcloud a Forgejo *tohto zariadenia*.
  `services.rke2` (rola `agent`) sa pripája k mesh edge. Kubelet agenta
  nemôže naštartovať, kým je jeho server nedosiahnuteľný, a
  `midnight-reboot.timer` sa spúšťa bezpodmienečne o 00:07. Keby vlastné
  služby zariadenia bežali v clustri edge, každý výpadok cez polnoc by ich zhodil,
  takže to "nezjednodušujte" na jeden cluster. Je to rke2 a nie druhý k3s,
  lebo nixpkgs zostavuje oba z jedného generátora parametrizovaného menom,
  takže ich stavové adresáre (`/var/lib/rancher/{k3s,rke2}`) a mená units
  nekolidujú. Neexistuje voľba `dataDir`, s ktorou by druhý k3s
  fungoval.
- **Pody `hostNetwork` nemajú rozlíšiteľnú zdrojovú adresu.** Lokálny
  cluster beží s `--flannel-backend=none`, takže jeho pody zdieľajú sieťový
  namespace hostiteľa a nginx ich vidí ako `127.0.0.1` alebo IP v LAN. Starý
  návrh s nspawn dával kontajnerom vlastnú podsieť, ktorú ochrana `lanOnly`
  odmietala ako prvú. Táto vrstva je preč. Nikdy nepíšte pravidlo
  `deny <podCidr>`, lebo nemôže na nič sedieť.
- **Výpočtové okno vynucuje edge, v časovom pásme appliance.**
  Taint NoSchedule môže zapísať len edge (NodeRestriction nikomu inému
  nedovolí), takže porovnanie beží na stroji, ktorý nepatrí vlastníkovi: edge
  VPS na UTC oproti appliance dodávanej s `Europe/Berlin`. Pásmo preto
  cestuje spolu s oboma hranicami (`--window-tz`, `ComputeWindow.tz`)
  a edge vyhodnocuje každý uzol cez `TZ="$tz" date`. Predtým čítal
  vlastné hodiny. Okno 23:00 až 07:00 zadané v Berlíne sa vynucovalo od
  00:00 do 08:00 v zime a od 01:00 do 09:00 v lete a pri každej zmene
  letného času sa posunulo o hodinu. Tak dostali pody cudzích ľudí prvé hodiny
  pracovného dňa vlastníka, presne to, čomu má táto funkcia zabrániť.
  Nevyťahujte `now` späť z cyklu cez uzly v `modules/edge.nix`. Je pre
  každý uzol zvlášť, lebo pásmo je tiež.
- **Trh je tretí opt-in registrátora.** `/market/*` (Stripe
  Connect, `backend-registrar/src/market.rs`, [Trh](/in-depth/market.md)) odpovedá 503,
  pokiaľ nie je nastavené `losos.edge.market.enable`, a 403, pokiaľ nie je
  nastavené `losos.edge.tenants.<id>.market`. `tenantsJson` v
  `modules/edge.nix` má svoje atribúty natvrdo, takže kľúč `market` tam
  musí zostať uvedený, inak každý obchod dostane 403, ktorú nikto nevysvetlí.
  - Zaplatená objednávka je nárok. Objednávky úložiska dostanú namespace a PVC
    v mesh; ich vytvorenie je idempotentné, 409 sa počíta ako hotovo a
    nič sa nikdy nemaže. Výpočtový výkon je len kredit v účtovnej knihe.
  - Ponuky a objednávky závisia od toho, čo uzol predávajúceho už zdieľa
    (`Sharing`, z výpočtových okien v registri). Trh je spôsob, ako dostať
    zaplatené za mesh, nie druhý produkt.
  - Admin UI sa k nemu dostane cez relay `/api/market*` v lososd
    (`backend/src/market.rs`). Relay odpovedá 200 `{available:false}`
    namiesto 404, keď je trh vypnutý, lebo SPA si 404 zapamätá ako
    "route not served".
  - Stripe kľúč drží len unit `losos-stripe-gate` (`losos-registrar
    stripe-gate`). Zapečatené bloby
    (`losos.edge.market.{stripeSecretKey,webhookSecret}Sealed`) dostáva cez
    `LoadCredentialEncrypted=`. Registrátor s ňou hovorí cez
    `/run/losos-stripe-gate/gate.sock` a kľúč nikdy nevidí. Nepridávajte
    operáciu, ktorá preposiela ľubovoľné volania Stripe, a kontroly
    požiadaviek v bráne (cieľ, mena, strop poplatku, životnosť session,
    https endpoint) držte prísnejšie, než čo registrátor práve posiela.
    Chýbajúci blob bránu preskočí (`ConditionPathExists`) a `/market/*`
    odpovedá 503.
  - Onboarding zapíše UUID zariadenia do Stripe účtu. Je to odvodenina
    kódu na obnovenie cez SHA-256 (`backend/src/boxid.rs`), nikdy nie samotný
    kód.
- **Zdieľanie otvorí akýkoľvek edge; trh otvorí len *oficiálny* edge.**
  lososd hľadá edge (`backend/src/edge.rs`: DNS-SD `_losos-edge._tcp`
  plus `losos.proxy.registrarUrl`, sondované na `/health`). Ak žiadny nie je
  v dosahu, odmietne zapnúť `sharingMyStorage` alebo `cluster.enable` (409
  `edgeRequired`). Odmietnutie platí len pre zapínanie, takže zariadenie môže
  vždy odísť.
  - Navyše lososd vyzve každý odpovedajúci edge
    (`GET /identity?nonce=`). Edge sa počíta za oficiálny, len ak je jeho
    certifikát podpísaný koreňovým kľúčom LosOS v
    `keys/official-edge-root.pub` (`losos.proxy.officialRootKeyFile`), uvádza
    tú URL a nevypršal, a kľúč certifikátu podpísal nonce. Relay trhu
    odmietne všetko ostatné (`{available:false,
    reason:"noOfficialEdge"}`, 409 `officialEdgeRequired`).
  - Commitnutý súbor s kľúčom je **zámerne prázdny**, kým doň vlastník
    nezapíše verejný kľúč. Bez koreňa nie je nič oficiálne a trh je
    vypnutý. Súkromný kľúč je offline u vlastníka a nikdy sa
    necommituje.
  - `losos-registrar identity {keygen,sign,show,verify}` sú nízkoúrovňové
    príkazy (`backend-registrar/src/identity.rs`). `tests/edge-lan.nix`
    ich používa na zostavenie vlastného koreňa pri builde, namiesto aby
    dôveroval dodanému súboru. `losos.edge.identity.{keyFile,certFile}` sú
    strana edge.
  - Ceremónia, ktorú operátor vykonáva, je `losos-registrar provision
    {whoami,root-keygen,publish,edge,verify}`
    (`backend-registrar/src/provision.rs`), **na vlastnom počítači
    operátora**. Každé sloveso, ktoré koreňový kľúč vytvára alebo používa,
    najprv operátora prihlási cez OAuth device flow GitHubu (len verejné
    client id, žiadne tajomstvo). Odmietne, pokiaľ *číselné id* účtu nie je v
    `backend-registrar/operators.json`, ktorý je zakompilovaný do binárky.
  - `provision edge` prečíta vlastný verejný kľúč edge z
    `GET /identity/public-key`; registrátor ten kľúč vytvorí pri prvom
    štarte a súkromná polovica nikdy neopustí edge. Nástroj podpíše
    certifikát, pošle ho na `POST /identity/cert` s GitHub tokenom
    z prihlásenia a spustí štyri kontroly. Edge overí token
    voči tomu istému zakompilovanému allowlistu skôr, než čokoľvek nainštaluje
    (`server::identity_push`). Žiadne SSH. `root-keygen --publish` a
    `publish` otvoria PR, ktorý vyplní `keys/official-edge-root.pub`, s tým
    istým prihlásením (`public_repo`). `tests/provision.rs` to celé spúšťa
    proti falošnému GitHubu a skutočnému registrátorovi.
  - Allowlist rozhoduje, komu nástroje slúžia. Celým tajomstvom zostáva
    koreňový kľúč.
  - Forgejo Actions je **vypnuté** v oboch režimoch Forgejo a zariadenie nespúšťa
    žiadny runner. Skoršia ceremónia zo zariadenia (workflowy Actions plus
    `modules/git-runner.nix`) je preč a
    `provisioning/edge-identity/README.md` je runbook operátora.
  - Počítač operátora nemá Nix store, takže binárka preň je
    `.#losos-registrar-static` (`pkgsStatic`, musl, rovnaký `cargoHash`).
    Job `publish-tool` v CI ju pushne do GHCR ako `images:<channel>-x86_64`
    (`losos-registrar` a `SHA256SUMS`, každý v jednej vrstve) a proxy ju
    obsluhuje na `/updates/<channel>/x86_64/<file>`. To je jediná cesta
    proxy LosOS Desktop, ktorá rozdáva obyčajné súbory. Runbook obsahuje
    príslušný riadok s curl.
- **`system.stateVersion = "26.11"` sa nastavuje raz.** Zodpovedá
  nixos-unstable, ktoré tento flake sleduje. Nemeňte ho.
- **Symlink `result` je artefakt `nix build`** ukazujúci do
  `/nix/store`. Je v gitignore a nikdy sa necommituje.
