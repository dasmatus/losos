---
title: Overenie konfigurácie a funkčnosti
sidebar_position: 7
---

# Overenie správnosti konfigurácie a funkčnosti

Bod 2d zadania: *overte správnosť konfigurácie a funkčnosť operačného
systému.* Overenie má v projekte jednu hlavnú skúšku a tri vrstvy. Hlavná
skúška je mesh v dvoch scenároch: **bez edge proxy** (box zdieľanie odmietne
a slúži lokálne) a **s edge proxy** (dva boxy v jednej sieti so spoločným
úložiskom), lebo správnosť konfigurácie tohto systému znamená, že mesh
funguje práve vtedy, keď má. Vrstvy sú
automatizované testy, ktoré bežia pri každej zmene; ručný kontrolný zoznam
vo virtuálnom stroji, ktorý je zároveň scenárom ukážky; a nezávislá kritická
recenzia, ktorej nálezy sa opravili a pokryli testami.

## Zásada: testy kódujú argumenty, nie len správanie

Box nemá shell, takže chyba, ktorá sa prejaví až na nainštalovanom stroji,
nemá kde byť opravená ručne. Testy preto nechránia len to, že niečo
funguje, ale aj *prečo* je niečo nastavené práve tak:
`tests/hardening.nix` overuje, čo je zámerne vypnuté; `tests/resize.nix`
overuje poradie `lvextend → cryptsetup resize → resize2fs`;
`tests/front-vhost.nix` overuje, že loopback dostane 403, a vysvetľuje,
prečo by 200 bola chyba; `tests/install.nix` overuje funkciu `encrypt` na
ext4, nie len typ súborového systému. Kto neskôr rozhodnutie zmení, dostane
červený test namiesto rozbitého boxu v skrini.

## Vrstva 1: automatizované testy

### Jednotkové testy (Rust)

Vrstva príkazov démona je napísaná proti rozhraniu `Losos` s reálnou
(`io_backend`) a pamäťovou (`fake`) implementáciou, takže stavový automat
sa testuje bez súborového systému. Plánovače (inštalácia, rast disku,
obnova, trh) vracajú plán ako dáta a testy overujú poradie deštruktívnych
krokov bez formátovania. Spolu vyše 200 testov v troch crate
(`backend`, `backend-registrar`, `edge-vercel`); bežia cez `cargo test` v
CI aj lokálne (`devenv test`). Oba crate musia byť čisté pre `clippy -D
warnings` a `rustfmt`.

### Testy vo virtuálnych strojoch (NixOS)

Osemnásť testov v `tests/` bootuje skutočné systémy z tých istých modulov,
z ktorých sa inštaluje. Prehľad:

| Test                    | Čo overuje                                                                                                      |
| ----------------------- | --------------------------------------------------------------------------------------------------------------- |
| `install.nix`           | inštalátor na troch prázdnych diskoch: detekcia, disko, LVM, LUKS, ext4 `encrypt`, otvorenie zväzku kľúčom      |
| `tpm.nix`               | od konca po koniec: formát, zapečatenie do swtpm, reštart, odomknutie bez obsluhy                               |
| `impermanence.nix`      | tmpfs koreň, zoznam trvalých adresárov, práva 700 a oddelené skupiny domén                                      |
| `hardening.nix`         | obe polovice: čo je zapnuté a čo je zámerne vypnuté; povrch jadra pre k3s, rke2, containerd a Longhorn           |
| `resize.nix`            | rast `/persist` za behu v správnom poradí                                                                        |
| `admin-vm.nix`          | lososd: D-Bus, HTTP API, token, prestavba                                                                        |
| `front-vhost.nix`       | nginx: smerovanie podľa cesty, guard len pre LAN, 403 na loopbacku, CSP vrátane arm pre `/widget-frame/` a `/handbook/` |
| `setup.nix`             | trasy prvého spustenia, certifikát, `trust.sh`, pôvody pre „nájdi môj box“                                       |
| `tls.nix`               | vlastný certifikát boxu a cache                                                                                  |
| `keyring.nix`           | kľúčenka tajomstiev boxu (fscrypt kľúč cez TPM)                                                                  |
| `console.nix`           | banner na tty1 s adresou                                                                                         |
| `cluster-vm.nix`        | koexistencia dvoch Kubernetes runtime na jednom hostiteľovi                                                      |
| `edge-vm.nix`           | edge + box: registrácia, tunel                                                                                   |
| `market-vm.nix`         | produkčné zapojenie Stripe gate                                                                                  |
| `nextcloud-httpd.nix`   | Apache v pode Nextcloudu proti fixtúre webroot (front-controller rewrite)                                        |
| `admin-ui.nix`          | produkčný zväzok SPA v prehliadači so skutočnou hlavičkou CSP                                                    |
| `design-system.nix`     | render React obalu dizajnového systému (len vývoj)                                                               |
| `secure-boot.nix`       | podpísané inštalačné médium pod OVMF so Secure Boot: nabootuje a zvnútra ukáže *Secure Boot: enabled*; firmvér len s kľúčmi Microsoftu, pozmenená kópia aj nepodpísané zostavenie sú odmietnuté; pozmenený systémový obraz zastaví stage 1 |

`tests/invariants.nix` nie je VM: pri vyhodnotení konfigurácie `install`
pripína hodnoty, ktoré si box nemôže dovoliť stratiť driftom predvolenej
hodnoty (GC, strop boot menu, režim odomykania, `--impure`, fragment
`#install`, cache). Beží v `nix flake check --no-build`, teda v CI pri
nulových nákladoch na zostavenie.

VM testy potrebujú KVM a bežia lokálne pred zlúčením zmeny (`nix build
.#checks.x86_64-linux.<názov>`); bez KVM bežia pod TCG pomalšie, ale bežia.

### Testy v prehliadači

`admin-ui/app/tests/*.browser.mjs` (Playwright, Chromium) spúšťajú
skutočný zostavený SPA so skutočnou hlavičkou CSP: sprievodca (36 scenárov),
aplikácia (10), oznámenia, vzhľad. Zlyhajú pri každom novom porušení CSP,
takže výmena knižnice, ktorá vkladá štýly za behu, zafarbí kontrolu
na červeno.

### Kontinuálna integrácia

`.github/workflows/ci.yml` pri každom push a pull requeste: zhoda pinov
(`flake.lock` = `devenv.yaml` = `devenv.lock`), clippy a rustfmt, testy
oboch crate, `nix flake check` s invariantmi, zostavenie balíkov (lososd,
registrátor, administračné UI, príručka, OCI obrazy), **zostavenie
inštalačného ISO, podpis jeho zavádzača UEFI** kľúčom z tajomstva
repozitára a **jeho boot** pod OVMF aj SeaBIOS (`tests/iso-boot.py` čaká
na DNS dotaz inštalátora), plus dve vetvy Secure Boot: OVMF len s kľúčmi
Microsoftu musí médium odmietnuť a OVMF so zapísaným certifikátom LosOS ho
musí spustiť; publikovanie zostavených ciest do cache. Vydanie (tag) pridá
k ISO `SHA256SUMS` s odpojeným podpisom tým istým kľúčom a certifikát.
Príručka má vlastný workflow (typová kontrola, kontrola odkazov a tokenov,
zostavenie, nasadenie na GitHub Pages, zostavenie tohto PDF).

## Hlavná skúška: mesh funguje, s edge proxy aj bez neho

Správnosť konfigurácie tohto systému sa nedá overiť na jednom stroji, lebo
jeho zmysel je v sieti: box musí nájsť edge, pripojiť sa k meshu a zdieľané
úložisko musí byť vidieť z oboch strán. Rovnako dôležité je však to, čo box
urobí, keď edge v sieti **nie je**: musí to zistiť sám, zdieľanie odmietnuť
a ďalej slúžiť svojmu majiteľovi lokálne. Hlavné overenie sú preto **dva
scenáre na jednej sieti bez internetu**, každý s očakávaným výsledkom, ktoré
sa predvedú za sebou na tej istej zostave dvoch boxov.

### Scenár A: bez edge proxy

Zostava: dva boxy v jednej sieti, žiadny edge (alebo edge vypnutý).

| #  | Krok                                                   | Očakávaný výsledok                                                                                     |
| -- | ------------------------------------------------------ | ------------------------------------------------------------------------------------------------------ |
| A1 | spustiť oba boxy a prejsť sprievodcom                  | Nextcloud a adminské rozhranie fungujú na každom boxe samostatne; panel Mesh hlási *Edge proxy: none*  |
| A2 | pokúsiť sa zapnúť *Join the mesh* alebo zdieľanie disku | prepínače sú sivé s dôvodom; priame volanie API odpovie 409 s vetou, že edge nebol nájdený             |
| A3 | nechať boxy bežať a sledovať panel Mesh                 | box skenuje sieť ďalej (mDNS `_losos-edge._tcp` a nastavená adresa), stav sa nemení, nič sa nepokazí   |
| A4 | reštartovať box                                         | po štarte je stav rovnaký: lokálne služby bežia, mesh je vypnutý, nič sa nepokúša pripojiť naslepo    |

Výsledok scenára A: box bez edge je plnohodnotné lokálne úložisko a **nikdy
nezačne zdieľať disk do siete, v ktorej nie je dôveryhodný edge**. Toto je
brána *enable-only* z kapitoly 6: zdieľanie sa dá zapnúť len s edge
v dosahu.

### Scenár B: s edge proxy

Zostava: tie isté dva boxy plus jeden edge v tej istej sieti.

| #  | Krok                                                         | Očakávaný výsledok                                                                                   |
| -- | ------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------- |
| B1 | spustiť edge (alebo ho zapnúť k zostave zo scenára A)        | do minúty oba boxy ukážu na paneli Mesh *Edge proxy found* so zdrojom *On this network*             |
| B2 | na oboch boxoch zapnúť *Join the mesh*                       | prestavba prejde; `kubectl get nodes` na edge ukáže oba uzly v stave Ready                            |
| B3 | na oboch boxoch zapnúť zdieľanie disku                       | Longhorn na edge ukáže dva uzly s pridelenou kapacitou; adresár `/home/shared` je odomknutý (fscrypt) |
| B4 | vytvoriť zväzok v poole a zapísať doň dáta                   | zväzok má repliky na oboch boxoch; dáta sú čitateľné po odpojení jedného boxu                         |
| B5 | vypnúť edge                                                   | do minúty oba boxy hlásia *Edge proxy: none*; zapnutie zdieľania odpovie 409; vlastné aplikácie boxov bežia ďalej (návrat do scenára A) |
| B6 | zapnúť edge späť                                              | brána sa otvorí bez zásahu; uzly sa vrátia do Ready bez opätovného pridania (`/etc/rancher` prežil)   |
| B7 | nastaviť okno 23:00 až 07:00 a pozrieť uzol na edge mimo okna | uzol má taint NoSchedule; v okne a pri nečinnosti taint zmizne                                        |
| B8 | pripojiť neoficiálny edge (bez podpisu koreňovým kľúčom)      | zdieľanie je dovolené, trh hlási *noOfficialEdge* a objednávka odpovie 409                            |

Výsledok scenára B: **úložisko dvoch boxov je spojené do jedného poolu
a vidieť ho z oboch strán**, členstvo prežije výpadok edge aj reštart, a
obchodovanie sa povolí len oficiálnemu edge.

### Čím sú scenáre doložené

- **Automatizovaný test `tests/edge-lan.nix`** (pripravovaný v PR #75):
  dva virtuálne stroje na jednej sieti, `edge` s modulom edge a ohlasovaním
  služby `_losos-edge._tcp` cez mDNS, `box` s riadiacou rovinou zariadenia.
  Test prejde oba scenáre v jednom behu: box bez edge zdieľanie odmietne
  (A2); s edge v dosahu je prepnutie do režimu mesh prijaté (B1, B2); keď
  edge zmizne, box to do dvoch skenov zbadá a zapnutie zdieľania odmietne
  s vlastnou vetou (B5); po návrate edge sa brána znovu otvorí (B6). Druhý,
  nepodpísaný edge v tom istom teste overuje, že obchodovanie sa povolí len
  oficiálnemu edge (B8).
- **Ukážka `demo/edge-lan/run.sh`** (ten istý PR): skript postaví edge VM
  ako smerovač virtuálnej siete (VDE prepínač, 10.77.0.1/24, DHCP, NAT),
  nainštaluje box z inštalačného ISO, zaberie ho, a prevedie oba scenáre
  cez API: box bez edge zdieľanie odmietne; edge sa zapne, box ho nájde
  sám, panel Mesh to ukáže a dovolí zdieľať úložisko; edge sa vypne, box to
  do minúty zbadá a zdieľanie odmietne; edge sa vráti a brána sa otvorí.
  Nahrávka oboch vetiev je súčasťou materiálov k obhajobe. Skript je
  zároveň základom reprodukovateľného firemného nasadenia (edge a boxy
  v jednej sieti), takže overenie konfigurácie a návod pre operátora sú
  jeden a ten istý postup.

Body A1 až A4, B1, B2, B5, B6 a B8 pokrýva test `edge-lan.nix` a ukážka;
B3, B4 a B7 sú overené v `cluster-vm.nix` (koexistencia runtime) a na edge
ručne, a sú to body, ktoré ukážka na obhajobe predvedie naživo.

### Scenár C: podpísané médium a Secure Boot

Inštalačné médium je podpísané (zavádzač UEFI je jeden zjednotený obraz
jadra podpísaný certifikátom LosOS a pred pripojením systémového obrazu
overí jeho odtlačok z podpísaného príkazového riadku). Overenie beží vo
virtuálnom stroji s OVMF v zostave so Secure Boot (SMM), s dočasným
testovacím kľúčom, ktorý test vytvorí a zahodí; produkčný kľúč vzniká len
na počítači autora (`provisioning/secure-boot/keygen.sh`).

| #  | Krok                                                                 | Očakávaný výsledok                                                                                                   |
| -- | -------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| C1 | nabootovať podpísané médium vo firmvéri so zapísaným certifikátom    | inštalátor beží; nad menu firmvéru vypíše *Secure Boot: enabled*; `bootctl status` hlási *Secure Boot: enabled (user)*, premenná `SecureBoot` je 1, `db` obsahuje certifikát, ktorým bolo médium podpísané; stage 1 zapísal, že systémový obraz sedí s podpísaným odtlačkom |
| C2 | to isté médium vo firmvéri len s kľúčmi Microsoftu (bežné PC)        | firmvér médium odmietne: *Access Denied -- rejected probably by Secure Boot*, boot sa nezačne                           |
| C3 | kópia média s jedným zmeneným bajtom zavádzača                       | odmietnuté rovnako: podpis pokrýva celý obraz zavádzača                                                               |
| C4 | nepodpísaný výstup `nix build`                                       | odmietnutý rovnako                                                                                                    |
| C5 | podpísané médium s jedným zmeneným bitom v systémovom obraze          | firmvér zavádzač spustí (je nedotknutý), stage 1 vypíše *THE MEDIUM HAS BEEN ALTERED* a stroj vypne                    |

Všetkých päť bodov vykonáva test `tests/secure-boot.nix` v jednom behu a
ku každému uloží snímku obrazovky do výstupu kontroly; CI opakuje C1 a C2
na skutočnom vydanom ISO (`tests/iso-boot.py`). Snímky z behu sú v
materiáloch k obhajobe.

## Overenie jedného boxu vo virtuálnom stroji

Kontrolný zoznam po inštalácii, s očakávaným výsledkom. Je to zároveň
scenár ukážky na obhajobe; pri každom bode je uvedené, ktorý bod zadania
dokladá.

| #  | Krok                                                                     | Očakávaný výsledok                                                                                                    | Dokladá |
| -- | ------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------- | ------- |
| 1  | Nabootovať ISO, nechať menu vybrať autodetekciu                          | inštalátor beží bez otázok, vypíše `unlock: TPM` (so swtpm) alebo `unlock: keyfile in the initrd`                      | 2b      |
| 2  | Reštart bez média                                                        | žiadna výzva na heslo; modrý banner s IP adresou a `<názov>.local`                                                     | 2b, 2a  |
| 3  | Otvoriť IP adresu v prehliadači                                           | sprievodca, krok 1 s odtlačkom certifikátu                                                                             | 2c      |
| 4  | Nastaviť heslo (12+ znakov, pravidlá sa odškrtnú)                         | *Password set*, náhradný kľúč zobrazený raz                                                                            | 2c      |
| 5  | Prihlásiť sa do LosOS cloudu v kroku 3                                    | Nextcloud prihlásený, presmerovanie na prehľad                                                                         | 2c, 2d  |
| 6  | Otvoriť `/forgejo/`                                                       | LosOS Git odpovedá, téma `losos-auto`                                                                                  | 2d      |
| 7  | Zavrieť kartu a otvoriť administráciu znova, odomknúť heslom              | dialóg *Unlock this box* prijme heslo; nesprávne heslo dá 401                                                          | 2d      |
| 8  | Zmeniť názov boxu na paneli Sieť, Použiť                                  | oznámenie *Changes applied*, box odpovedá na novom `.local` mene, IP nezmenená                                          | 2c, 2d  |
| 9  | Vytvoriť súbor mimo `/persist` (napr. cez widget) a nahrať súbor do cloudu | po reštarte prvý zmizol, druhý ostal                                                                                   | 2d      |
| 10 | Pozrieť `uname -r` a `/proc/cmdline` (cez konzolu VM, v ukážke)           | najnovšie stabilné jadro; KSPP parametre (`slab_nomerge`, `init_on_alloc=1`, …)                                        | 2a      |
| 11 | `cryptsetup luksDump /dev/persist-vg/persist` (konzola VM)                | dva kľúčové sloty, token `systemd-tpm2` v slote 1                                                                      | 2b      |
| 12 | Panel Úložisko → Použiť rezervu                                           | zväzok narastie za behu, `df` ukáže viac miesta                                                                        | 2d      |
| 13 | Panel Mesh bez edge                                                       | *Edge proxy: none*, prepínače zdieľania sivé s dôvodom                                                                 | 2d      |
| 14 | `curl http://127.0.0.1/` na boxe                                          | 403 (guard len pre LAN), kým `curl http://127.0.0.1:8082/api/health` odpovie                                           | 2d      |
| 15 | Nabootovať ISO s OVMF so Secure Boot a zapísaným certifikátom LosOS      | prvý riadok inštalátora *Secure Boot: enabled*; s kľúčmi Microsoftu *Access Denied* (scenár C)                         | 2b      |

Body 10, 11 a 14 vyžadujú konzolu, ktorú nainštalovaný box nemá; v ukážke
sa robia z inštalačného média alebo z VM testu, nie z boxu. Body 1 až 9 sú
to, čo majiteľ naozaj robí.

## Vrstva 3: nezávislá kritická recenzia

5. októbra 2026 autor nechal nad repozitárom urobiť recenziu z pohľadu
skúšajúceho („čo by tu komisia našla ako prvé“), s nálezmi overenými v kóde.
Nástroje (clippy, testy, typová kontrola) nenašli nič; nálezy boli o
správaní a dokumentácii. Najdôležitejšie a ich stav:

| Nález                                                                                        | Oprava                                                                                              |
| -------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| ISO spúšťalo inštalátor bez TPM, hoci dokumentácia opisovala TPM ako predvolené               | inštalátor autodetekuje `/dev/tpmrm0`, zapečatí kľúč hneď po formáte, `--no-tpm` je opt-out (PR #47, #48); `tests/tpm.nix` |
| aktualizácia z `github:` zdroja by zabudla disky a režim odomykania boxu                     | živé `install-target.nix` a `overrides.nix` z `/etc/nixos`, obe cesty prestavby s `--impure`; invariant (PR #49) |
| sprievodca nevedel box zabrať (volal trasu s tokenom, ktorý ešte nemal)                       | krok 2 najprv volá `POST /api/setup/claim` (PR #45)                                                  |
| administračný kľúč sa vydal raz a nikde nezobrazil                                            | zobrazenie v sprievodcovi s Kopírovať a Tlačiť; neskôr heslo ako hlavná cesta (PR #65)              |
| žiadne GC a žiadny strop generácií na samoaktualizujúcom sa boxe                              | `nix.gc` 14 dní, `configurationLimit = 5`; invariant                                                 |
| piny nixpkgs v `devenv.yaml` a `flake.lock` sa rozišli a CI to nestrážilo                     | úloha `pins` v CI                                                                                    |
| kľúčový súbor LUKS so surovými bajtmi (NUL) sa formátoval iným kľúčom, než akým sa odomykal   | hexadecimálny zápis, test otvorenia kľúčom hneď po disko (PR #47)                                    |

Ponechané a priznané v bezpečnostnom modeli: certifikát boxu je zároveň
CA bez obmedzení; synchronné obsluhy HTTP v démone; trh nikdy nebežal
proti skutočnému testovaciemu režimu Stripe (bežal proti náhradám s rovnakým
rozhraním). Recenzia a pripravené odpovede sú súčasťou materiálov
k obhajobe.

## Čo overené nie je

Pre úplnosť: Nextcloud a Forgejo ako celok nebootujú v žiadnom VM teste
(obraz Nextcloudu má 2,6 GiB a jeho zostavenie v testovacom VM by trvalo
hodiny); ich funkčnosť sa overuje ručne vo VM ukážke a Apache podu
samostatným testom. Mesh s Longhornom a taintom beží v `cluster-vm.nix`
len po koexistenciu runtime; objavenie edge a brána zdieľania majú test
`edge-lan.nix` (PR #75), replikácia a taint sa overujú na edge ručne podľa
zoznamu B3, B4 a B7. Trh je overený proti náhradám
Stripe a apiservera.
