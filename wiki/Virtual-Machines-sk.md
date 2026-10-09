[English](Virtual-Machines) · **Slovenčina** · [Deutsch](Virtual-Machines-de)

# Virtuálne stroje

Zariadenie, ktoré zdieľa svoj disk so sieťou mesh, si v nej môže prenajať
virtuálne stroje a môže hosťovať stroje iných zariadení. Stroj sa predáva po
**replikách**: každá replika je jedna kópia stroja s vlastným diskom, ktorá
mesiac beží na zariadení, ktoré ju hosťuje. Platí sa cez [trh](Market-sk) a
každý predaj sa delí **na polovicu** medzi vlastníka hosťujúceho zariadenia a
LosOS.

Hypervízor je [KubeVirt](https://kubevirt.io) a disky importuje
[CDI](https://github.com/kubevirt/containerized-data-importer), oboje v
klastri mesh na okrajovom serveri. Repliky predáva a stroje vytvára
`losos-registrar` na okrajovom serveri; vlastník robí všetko na stránke
**Stroje** v administrácii (v bočnom paneli pod **Mesh**, na adrese
`/machines`).

## Kto to môže používať

Stroje, na prenájom aj na hosťovanie, sa ponúkajú len zariadeniu, ktoré
zdieľa svoj disk (`losos.sharingMyStorage`). lososd to kontroluje pri každej
požiadavke: zariadenie, ktoré nezdieľa, dostane z `GET /api/vms` odpoveď
`{available: false, reason: "notSharing"}`, každá akcia sa odmietne s kódom
409 a stránka namiesto formulára ukáže jednu sivú vetu.

Prepínač zdieľania disku je na paneli Trh a trh ešte nie je otvorený (pozri
[Trh](Market-sk)). Kým sa neotvorí, administrácia zdieľanie zapnúť nevie, takže
stránka Stroje zostáva sivá na každom zariadení, ktoré si
`losos.sharingMyStorage` nenastavilo inak.

Stránka môže uviesť ešte dva dôvody:

| Dôvod            | Význam                                                          |
| ---------------- | --------------------------------------------------------------- |
| `noOfficialEdge` | Žiadny dosiahnuteľný okrajový server neprevádzkuje LosOS, nič sa nepredá. |
| `notOffered`     | Okrajový server nespúšťa stroje (`losos.edge.vms.enable`).      |

## Čo sa dá spustiť

| Zdroj             | Čo to je                                                          |
| ----------------- | ----------------------------------------------------------------- |
| LosOS             | Ukážkový disk LosOS, sťahovaný ako containerDisk pre KubeVirt     |
| Cloudové obrazy   | Systémy podporované Quickemu, ktoré vydávajú hotový cloudový disk |
| Vlastný QCOW2     | Obraz, ktorý nahráte zo stránky Stroje                            |

Katalóg je `backend-registrar/vm-images.json`, zabudovaný do registrátora;
prevádzkovateľ ho môže nahradiť cez `--vm-catalogue`. Obsahuje Ubuntu Server
26.04 a 24.04, Debian 13 a 12, Fedora 43 Cloud, AlmaLinux 10, Rocky Linux 10,
CentOS Stream 10, Arch Linux, openSUSE Tumbleweed, Alpine Linux a FreeBSD,
každý z adresy vlastného projektu.

**Obraz LosOS je ukážkový disk, nie zariadenie.** Je to ten istý
`losos-disk-qcow2`, ktorý vydanie zverejňuje, bez šifrovania disku a bez
bezstavového koreňa (pozri `flake/disk-images.nix`). Štartuje cez UEFI a
potrebuje 4 GiB pamäte, pretože spúšťa dva klastre a LosOS cloud. Úloha vydania
ho zverejní ako `ghcr.io/<owner>/<repo>-demo-containerdisk:<tag>` a posunie
jeho značku `stable`, na ktorú sa katalóg odvoláva.

**Väčšina zoznamu Quickemu v ponuke nie je.** Tieto systémy existujú len ako
inštalačné ISO a ich inštalácia potrebuje konzolu v prehliadači, ktorú stránka
zatiaľ nemá. Stránka ich vypisuje pod „systémy zatiaľ nie sú v ponuke“ spolu s
niekoľkými vynechanými z iného dôvodu (macOS, ktorého licencia povoľuje len
hardvér Apple; Windows, ktoré potrebujú licenciu a konzolu; a niekoľko
systémov, ktorých cloudové obrazy nemajú stálu adresu alebo sú v archíve, ktorý
CDI neprečíta).

Cloudový obraz číta **cloud-init**, takže objednávkový formulár prijme
nepovinný dokument `#cloud-config` alebo skript (najviac 16 KiB), ktorý stroj
spustí pri prvom štarte, napríklad na nastavenie kľúča SSH. Registrátor ho
nikdy nevracia späť, pretože môže obsahovať heslo.

### Vlastný QCOW2

Stránka Stroje nahrá súbor QCOW2 priamo na okrajový server a ukazuje priebeh.
lososd ho posiela ďalej bez ukladania do medzipamäte a bez držania svojho
zámku a okrajový server ho kontroluje už počas prenosu:

- musí začínať hlavičkou QCOW2, inak odpovie 415;
- musí sa zmestiť do limitu (`losos.edge.vms.uploadMaxGiB`, predvolene
  32 GiB), inak 413, a jeho virtuálny disk môže mať najviac 512 GiB;
- musí sa hýbať: minúta bez jediného bajtu prenos ukončí.

Zariadenie si môže ponechať tri obrazy. Každý je uložený na okrajovom serveri
v `/var/lib/losos-registrar/vm-images/` s právami 0600 a každá replika, ktorá
z neho štartuje, ho importuje z jednoúčelovej adresy, ktorú dostanú len stroje
samotného kupujúceho. Replika obrazu dostane disk aspoň taký veľký, aká je
virtuálna veľkosť obrazu. Pri obraze, ktorý potrebuje firmvér UEFI, zaškrtnite
**Štartuje cez UEFI**; Secure Boot je v oboch prípadoch vypnutý.

## Repliky a cena

Hosťujúce zariadenie ponúka stroje s cenou **za repliku na mesiac** a s
počtom replík, ktoré je ochotné spustiť (najviac 50). Kupujúci vyberie systém,
názov, ponuku hostiteľa a počet replík (najviac 10 a nie viac, než má
hostiteľ voľných) a zaplatí na stránke Stripe v novej karte.

Všetky repliky na jednom okrajovom serveri majú rovnakú veľkosť:
`losos.edge.vms.cpu` vCPU, `memoryMiB` pamäte a `diskGiB` disku (predvolene 1,
2048 a 20), alebo viac, ak to obraz vyžaduje.

**Delenie je pevne 50 %.** Platba za stroj je destination charge ako každý iný
predaj na trhu a `application_fee_amount` je presne polovica sumy. Na stroje
sa nevzťahuje `losos.edge.market.feeBps` prevádzkovateľa a brána Stripe
odmietne platbu za stroj s akýmkoľvek iným poplatkom. Pri cene 6,00 EUR za
repliku na mesiac stoja tri repliky 18,00 EUR; vlastník hosťujúceho zariadenia
dostane 9,00 a LosOS si ponechá 9,00.

## Po zaplatení

Keď Stripe potvrdí platbu, reconciler registrátora:

1. vytvorí v sieti mesh menný priestor kupujúceho `market-<buyer id>`, ak
   ešte neexistuje;
2. vytvorí jeden `VirtualMachine` na repliku s menom `vm-<order>-0`, `-1` a
   tak ďalej, pripnutý k hosťujúcemu zariadeniu cez jeho štítok uzla
   `losos.dev/appliance`, každý s vlastným diskom, ktorý CDI importuje do
   triedy úložiska `losos-vm-local`;
3. vytvorí pred replikami jednu `Service` na porte 80.

Ak je nastavené `losos.edge.vms.domain`, Traefik na okrajovom serveri
sprístupní túto Service na `https://vm-<order>.<domain>`, rozloženú medzi
repliky, s certifikátom od Let's Encrypt. Bez neho stroje verejnú adresu
nemajú.

Stránka Stroje ukazuje každý stroj so stavom každej repliky (beží, štartuje,
kopíruje disk, pozastavený, zastavený alebo zlyhal) a jeho adresu a kým sa
niektorá replika ešte rozbieha, pýta sa znova každých 15 sekúnd.

**Keď mesiac uplynie**, každá replika sa zastaví (`runStrategy: Halted`) a jej
miesto sa vráti do ponuky hostiteľa. Disky zostávajú: obsahujú dáta
kupujúceho a ich odstránenie je rozhodnutím prevádzkovateľa, rovnako ako pri
zväzku objednávky úložiska.

## Hosťovanie strojov

Zariadenie hosťuje stroje, kým je zapojené do siete mesh, zdieľa svoj disk a
má zapnuté `losos.vms.host` (predvolené). Vtedy sa pripája s
`--host-vms true` a okrajový server mu dovolí ponúkať stroje. Na takom
zariadení sa načítajú moduly jadra `kvm-intel`, `kvm-amd`, `tun` a `vhost_net`
a disky replík sú v `losos.edge.vms.hostPath` (predvolene `/home/shared/vms`),
teda v domovskom priečinku zdieľaného používateľa a vnútri zdieľanej dátovej
domény.

Stroje bežia nepretržite. Uzlový agent KubeVirt aj importéry CDI tolerujú
taint výpočtového okna, pretože stroj kúpený na mesiac sa nemôže každé ráno
zastaviť. Ponúknite len toľko replík, koľko zariadenie unesie popri vlastnej
práci.

### Vnorená virtualizácia

KubeVirt potrebuje na hosťujúcom zariadení `/dev/kvm`. Na skutočnom mini-PC
je to VT-x alebo AMD-V procesora. Zariadenie, ktoré je samo virtuálnym
strojom, potrebuje od svojho hostiteľa **vnorenú virtualizáciu**
(`kvm_intel nested=1` alebo `kvm_amd nested=1` a `-cpu host` v QEMU). Bez nej
môže prevádzkovateľ nastaviť `losos.edge.vms.useEmulation`, pri ktorom repliky
bežia softvérovo: naštartujú, ale pomaly, a je to určené na vyskúšanie, nie na
predaj.

## Nastavenie prevádzkovateľa

Na okrajovom serveri:

```nix
losos.edge = {
  market.enable = true;   # machines are sold on the market
  cluster.enable = true;  # and run on the mesh
  vms = {
    enable = true;
    domain = "vms.example.net"; # optional: https://vm-<order>.vms.example.net
    # cpu = 1; memoryMiB = 2048; diskGiB = 20; uploadMaxGiB = 32;
    # useEmulation = true;      # only without /dev/kvm on the hosts
  };
};
```

`modules/edge-vms.nix` nasadí KubeVirt v1.9.0 a CDI v1.66.1 z ich pripnutých
manifestov vydania cez adresár manifestov rke2 a provisioner local-path
(rancher/local-path-provisioner v0.0.37) pod vlastnými menami LosOS pre triedu
úložiska `losos-vm-local`, ktorá disk vytvorí až keď má jeho stroj uzol a nikdy
žiadny nezmaže. Registrátor dostane právo čítať, vytvárať a meniť
`VirtualMachine` a vytvárať `Service`.

Pre `domain` nasmerujte na okrajový server záznam DNS so zástupným znakom
(`*.vms.example.net`). Kým sú stroje zapnuté, časový limit čítania na verejnom
vstupnom bode Traefiku je zvýšený na šesť hodín, aby sa veľké nahrávanie
neprerušilo.

## API

Na zariadení (s Bearer tokenom, za ochranou len pre LAN):

| Cesta                              | Robí                                           |
| ---------------------------------- | ---------------------------------------------- |
| `GET /api/vms`                     | Katalóg, ponuky, účet, stavy strojov           |
| `POST /api/vms/orders`             | `{listing_id, quantity, image, name, user_data?}` |
| `POST /api/vms/listings`           | `{unit_price, capacity}`, za repliku na mesiac |
| `POST /api/vms/listings/close`     | `{listing_id}`                                 |
| `PUT /api/vms/images?name=&efi=1`  | Nahrá QCOW2 (telo je súbor)                    |
| `POST /api/vms/images/remove`      | `{upload_id}`                                  |

Na okrajovom serveri vedľa zvyšku `/market/*`: `GET /market/vm-images`
(verejné, ako ponuka), `POST /market/vms/status`,
`POST /market/vm-images/ticket`, `POST /market/vm-images/remove` a dve cesty
na prenos `PUT /market/vm-images/upload/<ticket>` a
`GET /market/vm-images/fetch/<token>`, ktoré namiesto tokenu nájomníka
prijímajú oprávnenia a naraz zvládnu najviac štyri prenosy.

## Bezpečnostné vlastnosti

- Zariadenie, ktoré nezdieľa disk, si stroje nemôže prenajať ani ich
  hosťovať; lososd odmietne skôr, než sa spýta okrajového servera.
- Okrajový server prijme ponuku strojov len od zariadenia zapojeného do siete
  mesh, ktoré hlási, že stroje hosťuje.
- Poplatok z predaja stroja je polovica, čo ešte raz kontroluje brána Stripe.
- Nahraný obraz je dostupný len cez 64-znakové oprávnenie, ktoré sa zapisuje
  len do diskov samotného kupujúceho; lístok na nahranie je jednorazový a
  platí hodinu.
- Lístok na nahranie sa do curl dostane v dočasnom súbore, nie v príkazovom
  riadku.
- Nič, čo registrátor robí, nezmaže disk kupujúceho.

## Čo nie je pokryté

- Žiadna konzola v prehliadači, teda žiadne inštalačné ISO a žiadne
  prihlásenie okrem toho, čo nastaví cloud-init.
- Žiadna živá migrácia: replika beží na zariadení, na ktorom bola predaná.
- Žiadne snímky, žiadna zmena veľkosti bežiaceho stroja, žiadna voľba veľkosti
  pri objednávke.
- Testy vo VM nespúšťajú stroj KubeVirt; testy registrátora prechádzajú
  objednávkou, nahrávaním a vytvorením strojov proti falošnému apiserveru a
  Stripe.
