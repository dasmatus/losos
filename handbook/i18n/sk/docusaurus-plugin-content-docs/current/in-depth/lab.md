---
title: Lab
sidebar_position: 7
mdx:
  format: md
---

# Lab

LosOS Lab kreslí zostavu LosOS ako sieťový diagram a ukazuje prevádzku,
ktorá cez ňu tečie. Každé zariadenie ho podáva na `/lab/`, v bočnom paneli
administrácie pod položkou **Laboratórium**. Ako ostatné administračné stránky
odpovedá iba v lokálnej sieti.

Otvára sa na **This box** (toto zariadenie). Lab prečíta nastavenia zariadenia a edge,
ktoré zariadenie našlo, cez tie isté tri administračné routy ako ostatné stránky,
a podľa nich zariadenie nakreslí: jeho router, edge v dosahu, oficiálny edge, ak je
nastavený, a notebook. Router, káble a notebook sú predpokladané, pretože
zariadenie ich nevidí. Lab do zariadenia nikdy nič nezapisuje. Ak nie je nikto
prihlásený, povie to a otvorí vstavané zostavy.

## Čo ukazuje

- **Pohľady Logický a Fyzický.** Logický pohľad ukazuje
  podsiete, cesty cez edge a mesh. Fyzický pohľad ukazuje racky, stoly a
  káble medzi nimi.
- **Režimy Reálny čas a Simulácia.** Reálny čas prehráva prevádzku tak, ako sa
  deje. Má hodiny, ktoré môžete zrýchliť (1×, 10×, 60×, 10 min/s, 1 h/s), a
  tlačidlo, ktoré preskočí na najbližší časovač: reštart o 00:07, upgrade o
  03:00, garbage collection o 04:30 a začiatok a koniec výpočtového okna.
  Simulácia zadrží každý paket, kým nestlačíte Spustiť alebo Krok, a vypíše
  každý skok s jeho protokolom.
- **Vlastné zostavy.** Začnite od Empty canvas (prázdne plátno) alebo od
  ktorejkoľvek vstavanej zostavy, vytiahnite zariadenia z panela a natiahnite
  medzi ich portami meď, optiku alebo Wi-Fi. **Uložiť** stiahne zostavu ako
  súbor `.llf` (LosOS Lab file, vnútri JSON) a **Otvoriť** ho načíta späť.
  Posledná zostava, ktorú ste zmenili, sa uchováva aj v prehliadači a vo
  výbere je uvedená ako "Posledná zostava". Otvorený súbor sa znova postaví tými
  istými nástrojmi ako panel, takže čokoľvek v ňom, čo by panel nevedel
  vytvoriť, sa vynechá a Lab povie, koľko častí zahodil.
- **Vstavané zostavy.** Dve lokality za jedným oficiálnym edge, jedno domáce
  zariadenie, firma s vlastnou bránou, LAN bez internetu a tri učebnicové
  topológie: hviezda, zbernica a sieť. Sú to len východiskové body,
  postavené tým istým panelom.
- **Notebook s LosOS Desktop.** Má prihlasovaciu obrazovku, prehľad a
  prehliadač, ktorý môžete nasmerovať na ľubovoľné zariadenie alebo edge v
  diagrame.
- **Sieťové zariadenia** (routery, switche, Wi-Fi prístupové body a
  koaxiálna zbernica) bežia na vlastnom malom systéme s názvom Netzgeräte
  Betriebssystem. Jeho konzola má `show interfaces`, `show ip route`,
  `show arp` a `show dhcp`.

Diagram sa riadi tými istými pravidlami ako skutočný softvér. Príklady: zariadenie
bez edge v dosahu nespustí žiadny tunel; prepnutie na mesh sa odmietne s
`edgeRequired`; trh sa otvorí iba vedľa oficiálneho edge (takého, ktorého
certifikát podpísal koreňový kľúč LosOS); LAN brána s otvoreným zápisom
prijme neznáme zariadenia pri prvom použití; a administračné routy odmietajú
loopback.

## Dve kópie

| | Na zariadení (`/lab/`) | Hostovaná |
| --- | --- | --- |
| Zostavuje ju | `nix build .#losos-admin-ui` (Lab je jeho druhá stránka) | `admin-ui/lab/engine/build.sh` v `lab.yml` |
| Konzoly | simulované, alebo skutočné hosty pod libvirt, keď zariadenie spúšťa pomocníka | skutočné x86_64 hosty pod libvirt na vašom počítači, inak qemu-wasm |
| Potrebuje | nič okrem zariadenia | cross-origin izolovaný hostiteľ (COOP + COEP) |

Lab je súčasťou administračného rozhrania: `admin-ui/app/lab/index.html` je
druhá stránka Vite vedľa administračných stránok, postavená z tých istých
komponentov, palety, ikon a jazykov, a podávaná na `/lab/`. Všetko, čo nie je
kreslenie, beží vo WebAssembly: model, simulátor, hodiny, konzoly a stránky
sú Rust crate `admin-ui/lab/core` (`losos-lab-core`), ktorý nix zostaví pre
wasm32 ešte pred vlastným zostavením administračného rozhrania. Stránka beží
pod vlastnou content security policy: tou z administračnej stránky plus
`'wasm-unsafe-eval'`, aby prehliadač smel skompilovať jadro.

Hostovaná kópia je tá istá stránka zostavená s `VITE_LAB_HOSTED=1`, plus
[qemu-wasm](https://github.com/ktock/qemu-wasm): QEMU skompilované do
WebAssembly, takže každá konzola je skutočný linuxový host v karte
prehliadača. Zariadenia a edge bootujú busybox ako náhradu za LosOS a sieťové
zariadenia bootujú Netzgeräte Betriebssystem, ktorý zostaví nix z
`admin-ui/lab/engine/gear/netzgeraete.nix`. Lab zapojí sieťové karty hostov
do diagramu, takže DHCP, ARP, ping a HTTP medzi hostmi idú po kábloch, ktoré
ste nakreslili. Host routera prideľuje leasy vlastným `udhcpd`. Jedna karta
spustí naraz najviac tri hosty. So štvrtým hostom došli jadrá a hosty
zamrzli, preto Lab odmietne ďalší nabootovať. qemu-wasm spúšťa vlákna QEMU
ako Web Workery so zdieľanou pamäťou a prehliadače to dovolia iba na
stránke podávanej s `Cross-Origin-Opener-Policy: same-origin` a
`Cross-Origin-Embedder-Policy: require-corp`. `vercel.json` nastaví obe
hlavičky na Verceli a `serve.json` urobí to isté pre `serve` na vašom
vlastnom počítači:

```sh
admin-ui/lab/engine/build.sh /tmp/lab     # docker, nix, node
npx --yes serve@14.2.4 -l 8080 /tmp/lab   # then open http://localhost:8080/lab/
```

`lab.yml` zostaví hostovanú kópiu pre každý pull request, ktorý sa dotkne
`admin-ui/lab/` alebo stránky Labu v `admin-ui/app/`, a nahrá ju ako
artefakt `losos-lab-hosted`. Pri pushi do `main` kópiu navyše nasadí na
Vercel, ak má repozitár secrets `VERCEL_TOKEN`, `VERCEL_ORG_ID` a
`VERCEL_PROJECT_ID`.

## GPU plátno

Diagram má dve plátna. Stránka vždy začína tým SVG, takže zostava je na
obrazovke hneď, ako sa načíta jadro. Potom sa prehliadača opýta, čo ponúka.
S WebGPU načíta WebGPU zostavenie `admin-ui/lab/render` (`losos-lab-render`).
Bez WebGPU, ale s WebGL2 načíta zostavenie pre WebGL2. Bez oboch zostane pri
SVG.

Každé zostavenie je Bevy 0.19.1 skompilované do WebAssembly spolu s jadrom
Labu. Stránka presunie zostavu do vlastného jadra modulu tak, že zopakuje
každú zmenu, ktorú urobila v prvom jadre, a pokračuje iba vtedy, ak potom obe
opisujú tú istú zostavu. Plátno Bevy potom zaujme miesto plátna SVG a ponechá
si výber, nástroj a kameru.

Scéna je 3D. Každá miestnosť, dosah Wi-Fi, kábel a zariadenie je samostatný
plochý mesh na základnej rovine a ortografická kamera sa na ňu pozerá
kolmo zhora, takže oba pohľady vyzerajú ako v SVG. Názvy a adresy sú text v
priestore obrazovky nad scénou. Kliknutia sú lúče vrhané z kamery do
scény.

Stránka sa vráti k SVG a raz to oznámi, ak sa modul nenačíta, ak do 20
sekúnd nepríde žiadny snímok, alebo ak sa neskôr stratí GPU zariadenie.
Stratené zariadenie WebGPU si prehliadač zapamätá a ďalšia návšteva začne
na WebGL2.

Každé zostavenie má asi 16 MB WebAssembly, s brotli 3,4 MB pre WebGPU a
3,6 MB pre WebGL2. Prehliadač stiahne iba to, ktoré použije, a to až po
vykreslení plátna SVG. Content security policy stránky netreba meniť,
pretože `'wasm-unsafe-eval'` už prehliadaču dovoľuje skompilovať jadro.

Ak chcete plátno vybrať, pridajte k adrese `?canvas=svg`, `?canvas=webgpu`
alebo `?canvas=webgl2`. Kľúč lokálneho úložiska `losos-lab-canvas` berie tie
isté hodnoty a uchová voľbu v danom prehliadači. Odznak enginu v hornej
lište uvádza plátno vo svojom tooltipe.

## Hosty pod libvirt

qemu-wasm je pomalé: emuluje každú inštrukciu bez KVM, každý host stojí
kartu asi 450 MB a jedna karta spustí tri. Ak má počítač, na ktorom je Lab
otvorený, libvirt (to, čo ovláda virt-manager), Lab môže svoje hosty spúšťať
tam, pod KVM, ak ho procesor má. Nabootujú za pár sekúnd a naraz ich beží až
osem.

Host môže bežať tromi spôsobmi, v poradí podľa preferencie:

1. **Pomocník ovláda libvirt cez `virsh`.** Pomocník je podpríkaz edge
   registrátora, `losos-registrar lab`, bežiaci na tom istom počítači ako
   libvirt. Potrebuje tam libvirt, QEMU a príkaz `virsh` a obrazy hostov v
   priečinku, ktorý pomocník vie čítať.
2. **Stránka ovláda libvirt sama, cez relay pomocníka.** Pomocník ponúka aj
   WebSocket, ktorý bajt po bajte kopíruje do vlastného socketu libvirt.
   Stránka Labu nesie WebAssembly klienta libvirt
   (`admin-ui/lab/virt-rpc`, balík `losos-lab-virt`) a hovorí cez neho
   protokolom libvirt, takže táto cesta potrebuje pomocníka a bežiaceho
   démona libvirt, ale na počítači žiadny `virsh` ani klientsku knižnicu
   libvirt. Stránka ju použije, keď pomocníkovi chýba `virsh`, a pre každý
   host, ktorý prvý spôsob nevedel spustiť.
3. **qemu-wasm v karte.** Na počítači nič, iba engine hostovaného Labu. Je
   to pomalá cesta a záloha pre každý host, ktorý prvé dve nevedia
   spustiť.

### Pomocník a `virsh`

Pomocník spúšťa každý host ako dočasnú (transient) doménu s názvom
`losos-lab-...` cez `virsh create`. Transient znamená, že libvirt ju nikdy
neuloží, takže nič neprežije pomocníka: Ctrl-C alebo SIGTERM zničí každý
host, ktorý spustil, host, ktorý žiadna stránka Labu minútu nesledovala, sa
zničí tiež a pri štarte pomocník odstráni každú doménu `losos-lab-`, ktorú
po sebe nechal zabitý pomocník. Host nie je v žiadnej sieti libvirt ani na
žiadnom bridge. Jeho sériová konzola a sieťová karta sú pripojené k
pomocníkovi na 127.0.0.1 a pomocník obe odovzdá stránke cez WebSockety.
Switchom je stále stránka, takže host pod libvirt a host pod qemu-wasm môžu
zdieľať kábel a DHCP, ARP a ping medzi nimi fungujú ako predtým.

Ako ho použiť vedľa virt-managera na vlastnom PC:

1. Nainštalujte libvirt a QEMU (na väčšine distribúcií balíky, ktoré už
   stiahol virt-manager) a binárku `losos-registrar`
   (`nix build .#losos-registrar`, alebo statickú, ktorú stiahne runbook v
   `provisioning/edge-identity/README.md`).
2. Dajte obrazy hostov do priečinka s názvom `guest`: `bzImage`,
   `rootfs.bin` a pre routery, switche a prístupové body `gear.bin`.
   `admin-ui/lab/engine/build.sh` vyrobí všetky tri a hostovaný Lab ich
   podáva pod `/lab/guest/`, takže si ich môžete stiahnuť odtiaľ.
3. Spustite pomocníka v priečinku nad `guest`:

   ```sh
   losos-registrar lab --origin https://your-lab.example.org
   ```

   Počúva na `127.0.0.1:8095` a používa `qemu:///session`, ktoré nepotrebuje
   root. Zadajte `--connect qemu:///system`, ak chcete, aby sa hosty
   zobrazili vo virt-manageri vedľa vašich ostatných VM (váš používateľ musí
   byť v skupine `libvirt` a vlastný qemu používateľ libvirt musí vedieť
   čítať priečinok s obrazmi). `--origin` uvádza adresu stránky Labu, ktorú
   otvárate; kópia podávaná na `localhost:8080`, ako vyššie, je povolená aj
   bez neho. Ďalšie prepínače sú `--images DIR`, `--max-guests N` (8),
   `--memory MiB` (96), `--virt-type auto|kvm|qemu`, `--idle 60s`,
   `--virsh PATH` a `--token-file FILE`. Bez súboru s tokenom pomocník
   odmietne počúvať na čomkoľvek inom ako loopback a odpovedá iba na
   požiadavky adresované na `127.0.0.1` alebo `localhost`.
4. Otvorte Lab. Kópia na `localhost` hľadá pomocníka sama. Hostovaná kópia
   ho hľadá až vtedy, keď ju otvoríte s `?libvirt` na konci adresy, pretože
   Chrome sa každého návštevníka verejnej stránky pýta na prístup k
   lokálnej sieti v okamihu, keď sa dotkne `127.0.0.1`. Lab si voľbu
   zapamätá; `?libvirt=0` ju zabudne.

Odznak v hornej lište potom hovorí "KVM via libvirt" (alebo "QEMU via
libvirt (no KVM)" na počítači bez neho) a každá konzola uvádza, čo spúšťa
jej host: libvirt, alebo "QEMU in this tab". Ak pomocník nebeží alebo
libvirt host odmietne, ten host nabootuje pod qemu-wasm ako predtým a Lab to
raz oznámi. Vlastná kópia zariadenia qemu-wasm nemá, takže tam si ponechá
simulovanú konzolu.

### WebAssembly klient a relay

Webová stránka nevie otvoriť socket libvirt: prehliadače bežnej stránke
neponúkajú surové TCP ani unixové sockety a libvirt nemá vlastný
WebSocket listener. Preto pomocník robí relay. Stránka si cez
`POST /lab/v1/virt-ticket` vypýta tiket a potom otvorí WebSocket
`/lab/v1/virt` so subprotokolmi `losos-lab` a `ticket.<ticket>`. Tiket platí
raz, 30 sekúnd. Pomocník sa pripojí k socketu svojho `--connect` URI a
kopíruje bajty oboma smermi bez toho, aby ich čítal. Pre `qemu:///session`
je tým socketom `$XDG_RUNTIME_DIR/libvirt/virtqemud-sock`, alebo
`libvirt-sock` vedľa neho pre monolitický libvirtd. Pre `qemu:///system` je
to `/run/libvirt/virtqemud-sock`, potom `/run/libvirt/libvirt-sock`.
`?socket=PATH` v URI pomenuje socket priamo, rovnako ako pri `virsh`.
Pomocník drží naraz otvorených najviac `--max-guests` relayovaných socketov.

`GET /lab/v1/hello` hlási túto cestu samostatne, ako
`virt: {available, socket}`. `available` je true, keď socket práve teraz
prijme spojenie, takže môže byť true, aj keď `virsh` chýba. Pomocník
nespúšťa session démona tak, ako to robí `virsh`, takže pri
`qemu:///session` musí démon už bežať alebo ho musí spustiť jeho systemd
socket.

Stránka Labu túto cestu používa sama od seba (`engine/virt-rpc.ts`). Pre
každý host si vypýta tiket, otvorí jedno relayované spojenie, vytvorí
doménu pozastavenú, otvorí jej konzolu a potom ju obnoví, takže konzola
uvidí prvý bajt, ktorý host vypíše. Doména je tá istá, akú by pomocník
napísal pre `virsh`, až na to, že sériový port je pseudoterminál, ktorý
klient číta ako konzolu domény. Modul klienta, asi 220 KB, sa načíta až
vtedy, keď sa na tejto ceste spustí prvý host. Hlavička konzoly hovorí
"KVM via libvirt from WebAssembly", alebo "QEMU via libvirt from
WebAssembly (no KVM)". Bez KVM sa prvá doména odmietne a stránka sa opýta
znova, na obyčajné QEMU.

Ku každému tiketu patrí aj sieťová karta: pomocník naviaže ten istý druh
UDP tunela, aký dáva vlastným hostom, a odpovie jeho dvoma portami, názvom
pre doménu (`losos-lab-virt-...`) a druhým tiketom. Stránka zapíše tunel do
domény a otvorí kartu na `guests/<key>/nic/0`, takže tieto hosty sú na
kábloch Labu ako ostatné a host jednej cesty vie pingnúť host druhej.
Karta sa započítava do `--max-guests`; plný pomocník stále vydá relay
tiket, ale žiadnu kartu, a host potom nabootuje bez siete, čo uvedie
hlavička jeho konzoly.

Klient vytvára svoje hosty s príznakom autodestroy libvirt, takže skončia,
keď skončí spojenie: keď sa karta zatvorí a keď sa pomocník zastaví, pretože
pomocník pri Ctrl-C alebo SIGTERM zatvorí každý relayovaný socket. Keď
stránka host vypne, zničí doménu sama a kartu vráti cez
`DELETE guests/<key>`. Karta, ktorú žiadna stránka nesleduje, sa ukončí po
`--idle`, ako host samotného pomocníka, a keďže Lab pomenúva svoju doménu
rovnako ako pomocník, pomocníkovo upratovanie a kontrola nečinnosti
zasiahnu aj túto doménu. `admin-ui/lab/virt-rpc/README.md` ukazuje klienta
samostatne, cez jeho demo stránku.

### Bezpečnosť

Kto drží relayovaný socket, má libvirt s právami pomocníka. libvirt
identifikuje protistranu socketu podľa jej user id, takže vidí pomocníka a
nikdy nie stránku, a polkit sa pýta tiež na pomocníka. Pri
`qemu:///system` je to rovnako dobré ako root na počítači, pretože stránka
môže vytvoriť doménu, ktorá pripojí ľubovoľný súbor alebo disk hostiteľa, a
relay nevie takú doménu odmietnuť bez parsovania protokolu libvirt. Na
vlastnom PC uprednostnite `qemu:///session`: potom sú najhorším prípadom
súbory vášho vlastného používateľa. Hosty Labu zo systémovej inštancie nič
nepotrebujú.

Pomocník preto pustí k relay socketu iba s tiketom a tiket stojí toľko isto
ako spustenie hosta: bearer token, keď má pomocník `--token-file`, a inak
požiadavku adresovanú na `127.0.0.1` alebo `localhost` z povolenej stránky.
Zoznam `--origin` je tu najdôležitejší. WebSocket nie je viazaný CORS,
takže ktorákoľvek stránka otvorená v tom istom prehliadači sa môže pokúsiť
dostať na `ws://127.0.0.1:8095`, a pomocník odmietne každú požiadavku,
ktorej Origin nie je na zozname ani nie je vlastnou adresou pomocníka.

### Na zariadení

`losos.lab.libvirt.enable` (predvolene vypnuté) zapne libvirtd a spustí
toho istého pomocníka ako službu, s `qemu:///system`. lososd mu relayuje
požiadavky Labu pod `/api/lab/` s administračným kľúčom, vrátane
`POST /api/lab/virt-ticket`. Konzoly a sieťové karty hostov
(`/api/lab/ws/`) a relay libvirt (`/api/lab/virt`) idú cez nginx priamo k
pomocníkovi, iba z lokálnej siete, a každé potrebuje tiket, ktorý môže
získať iba tento kľúč. Relay vydáva libvirt ako používateľ pomocníka, ktorý
je v skupine `libvirtd`, takže na zariadení je ekvivalentný rootu;
administračný kľúč už vie prestavať celý systém, takže neprezradí nič, čo
kľúč predtým nemal. Dajte tri obrazy do `/var/lib/losos-lab/images`
(`losos.lab.libvirt.images`).

## Čo nie je skutočné

Hosty sú náhrady: busybox a obraz s veľkosťou 4 MiB, nie NixOS. Edge,
tunel, mesh a trh sú modelované v jadre v Ruste podľa pravidiel uvedených
vyššie; nie je to bežiaci kód. Pracovná plocha notebooku je obrázok
LosOS Desktop, nie samotná plocha.

## Ako ho meniť

- Stránka a jej časti sú v `admin-ui/app/src/lab/`. `npm run lab:core`
  zostaví jadro do `src/lab/core-pkg/` a `npm run lab:virt` klienta
  libvirt do `src/lab/virt-pkg/` (oboje v gitignore; potrebujú cargo s
  cieľom `wasm32-unknown-unknown` a `wasm-bindgen` 0.2.127) a potom
  `npm run dev` podáva stránku na `/lab/`. `npm run lab:render` rovnako
  zostaví dva moduly GPU plátna do `src/lab/render-pkg/` a spustí na nich
  `wasm-opt`, keď ho `WASM_OPT` pomenúva. Stránka sa bez nich nezostaví a
  ich zostavenie trvá asi 13 minút. `tests/lab.browser.mjs` ju kontroluje
  pod vlastnou politikou Labu, cestu cez relay proti falošnému démonovi
  libvirt.
- Kontrakt jadra je `admin-ui/lab/core/README.md`. Jeho testy porovnávajú
  každú vstavanú zostavu so správaním JavaScriptového Labu, ktorý
  nahradilo.
- Hosty: `admin-ui/app/src/lab/engine/` skúša libvirt cez `virsh`
  pomocníka, potom libvirt cez vlastného klienta stránky a relay
  pomocníka, potom qemu-wasm, v tomto poradí.
