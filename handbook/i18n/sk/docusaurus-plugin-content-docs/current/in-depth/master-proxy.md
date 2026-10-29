---
title: Master proxy
sidebar_position: 5
mdx:
  format: md
---

# Master proxy

Voliteľné. Sprístupní zariadenie z internetu bez toho, aby ste museli doma otvárať
port.

![internet → Traefik (VPS, :443) → tunelový server ⇐ tunel QUIC ⇐ tunelový klient (zariadenie) → nginx](/img/diagrams/master-proxy-sk.svg)

## Edge (VPS) {#edge-vps}

Importujte výstup flaku `nixosModules.edge` do konfigurácie VPS a nastavte
`losos.edge.*`:

- `losos.edge.publicDomain` je doména, pod ktorou sa zariadenia sprístupňujú.
- `losos.edge.tenants` je zoznam povolených zariadení, každé s názvom hostiteľa
  a súborom s tokenom. Registrar vytvára trasy a certifikáty iba pre
  identifikátory, ktoré sú uvedené tu.
- `losos.edge.cluster.enable` navyše spúšťa riadiacu vrstvu [mesh](/in-depth/mesh.md),
  server rke2 s Longhornom. Proxy funguje aj bez nej.
- `losos.edge.tang.enable` (predvolene zapnuté) umožní zariadeniam
  nainštalovaným s `--tang-edge` otvoriť disk cez edge, s párom kľúčov Tang
  pre každé zariadenie v `/var/lib/losos-registrar/tang` a stránkou
  `/tang/lost`, kde majiteľ nahlási ukradnuté zariadenie. Stranu zariadenia
  opisuje [TPM a kľúč disku](/reference/tpm.md).

`losos-registrar` udržiava súbor tunelového servera v súlade
s registrovanými zariadeniami a Traefik ich nachádza cez registrar.

## Ako Traefik nachádza boxy {#how-traefik-finds-the-boxes}

Traefik objavuje boxy cez svoj HTTP provider. Každé dve sekundy sa registrara
na porte dostupnom iba z loopbacku (`losos.edge.traefikProviderPort`,
predvolene 8090) spýta na aktuálne trasy: jednu pre každý názov živého boxu,
vlastnú doménu a zverejnený virtuálny stroj, každú smerom k jeho koncu tunela.
Box, ktorý sa zaregistruje, je dostupný do niekoľkých sekúnd, a box, ktorému
prestanú chodiť heartbeaty, stratí trasu, keď ho registrar vyradí. Registrar
do adresára Traefiku nič nezapisuje, sú v ňom iba trasy, ktoré spravuje NixOS.

Ak sa registrar zastaví alebo reštartuje, Traefik si ponechá trasy, ktoré
dostal naposledy, takže boxy zostanú dostupné. Pri aktualizácii edge, na ktorom
ešte zostali súbory `losos.yml` a `losos-vms.yml` od staršieho registrara, sa
tieto súbory pri prepnutí odstránia.

## HTTP/3 {#http3}

Verejný vstupný bod odpovedá cez HTTP/3 nad QUIC na porte UDP 443 a zároveň
cez HTTP/1.1 a HTTP/2 na porte TCP 443. Traefik ohlasuje HTTP/3 v hlavičke
`Alt-Svc`, takže prehliadač po prvej požiadavke prejde na QUIC. Sieť, ktorá
blokuje UDP, ďalej funguje cez TCP. Vo firewalle pred edge otvorte UDP 443;
vlastný firewall edge ho otvára sám.

Box, ktorý TLS ukončuje sám (pozri
[Edge nevidí do prevádzky](#the-edge-cannot-read-the-traffic)), beží iba cez
HTTP/2 a HTTP/1.1 na TCP. HTTP/3 zostáva na vlastných názvoch edge.

## Keď box nie je dostupný {#when-a-box-cant-be-reached}

Keď sa Traefik k boxu nedostane (je vypnutý, reštartuje sa, je offline alebo
mu nefunguje tunel), návštevník namiesto holého „Bad Gateway“ dostane stránku
z edge, ktorá to vysvetlí, po anglicky, slovensky alebo nemecky podľa jazyka
prehliadača. Stavový kód zostáva 502, alebo 504, keď box neodpovedal včas.

Stránka potrebuje, aby TLS pre názov ukončoval edge, preto sa ukáže iba pri
názvoch, pre ktoré ho edge ešte ukončuje. Keď box ukončuje TLS sám,
návštevník vypnutého boxu dostane namiesto nej chybu spojenia z prehliadača.

## Zariadenie {#appliance}

Nastavte `losos.proxy.enable = true` a voľby `losos.proxy.*`
(`registrarUrl`, `edgeTunnelEndpoint`, `hostname`, `applianceId`,
`tokenFile`).

## Tunel {#the-tunnel}

Zariadenie udržiava s edge jedno spojenie QUIC na porte UDP 2333
(`losos.edge.tunnelBindPort`). Každé spojenie návštevníka ide v ňom ako
samostatný prúd, takže pomalé sťahovanie nezdrží ostatné požiadavky, nová
požiadavka nepotrebuje nový handshake a spojenie prežije zmenu adresy
zariadenia. Zariadenie sa iba pripája von, takže funguje za NAT aj za
operátorským NAT (CG-NAT) bez presmerovania portu. Udržiavací paket každých
10 sekúnd drží záznam v NAT otvorený.

Ak sieť zahadzuje UDP, zariadenie dá QUIC päť sekúnd a potom sa pripojí na
ten istý port cez TCP: TLS 1.3 s tým istým certifikátom, v ňom prúdy HTTP/2.
Každých päť minút skúsi QUIC znova a vráti sa k nemu, keď prejde. Firewall
edge otvára port pre oba.

## Šifrovanie tunela {#tunnel-encryption}

Tunel je pri oboch transportoch TLS 1.3. Edge si pri prvom štarte vytvorí
vlastnoručne podpísaný certifikát a uchová ho v `/var/lib/losos-tunnel`.
Každé zariadenie si pred prvým pripojením stiahne z registrara SHA-256
certifikátu (`GET /tunnel-pin`) a uloží ho do `losos.proxy.tunnelPinFile`;
odvtedy tunel prijme tento certifikát a žiadny iný. Ak ho chcete pripnúť
mimo tohto kanála, vložte súbor na to miesto vopred; zariadenie ho nikdy
neprepíše. Každú službu, ktorú zariadenie poskytuje, si nárokuje tokenom
zariadenia, porovnaným v konštantnom čase.

## Edge nevidí do prevádzky {#the-edge-cannot-read-the-traffic}

Box si pre každý svoj verejný názov vybaví vlastný certifikát od Let's
Encrypt a TLS ukončuje sám. Edge smeruje každé spojenie podľa názvu v TLS
hello a šifrované bajty boxu len posúva ďalej bez toho, aby ich otvoril.
Kto prevádzkuje edge alebo stroj, na ktorom beží, tak nemôže čítať
prihlásenia, súbory ani stránky. Predvolene je to zapnuté
(`losos.proxy.tlsPassthrough.enable`).

1. Edge smeruje `http://<názov>/.well-known/acme-challenge/` na porte 80 ku
   každému boxu, aby box mohol cez tunel odpovedať na výzvu HTTP-01 od
   Let's Encrypt. Box o certifikát žiada cez `lego` (`losos-public-tls`,
   každú hodinu a vždy, keď mu edge oznámi nový názov) a certifikáty drží v
   `/var/lib/losos-public-tls/live/<názov>/`.
2. Heartbeat boxu vymenúva názvy, pre ktoré má platný certifikát. Kým nemá
   prvý, edge ukončuje TLS ako doteraz, takže prechod nič nevypne.
3. Pre tieto názvy edge zruší svoj TLS router a ponechá TCP router s
   `HostSNI` a passthrough. Sám si pre ne certifikát nevyžiada.
4. Na boxe nginx prečíta prvé bajty každého spojenia z tunela: TLS ide na
   vlastný certifikát boxu, obyčajné HTTP (výzvy, starší edge) na port 80
   ako doteraz. Administrátorské UI zostáva v oboch prípadoch len v LAN.

Keď box raz požiada o passthrough, certifikát, ktorý vyprší, TLS edgu
nevráti. Návštevníci potom vidia varovanie o certifikáte, kým ho box
neobnoví.

Box tiež každú hodinu otvorí každý svoj názov zvonka, cez verejné DNS a
edge, a porovná certifikát, ktorý dostane, so svojím. Výsledok ukazuje
stránka Mesh: **Šifrované až po zariadenie**, keď sa zhodujú, **Čitateľná**,
kým TLS ukončuje edge, a **Zachytávané**, keď odpovie certifikát, ktorý box
nevytvoril. To posledné znamená, že niečo medzi návštevníkmi a boxom
ukončuje ich TLS, napríklad edge, ktorý si pre názov vybavil vlastný
certifikát. Taký certifikát sa objaví aj vo verejných logoch Certificate
Transparency.

Čo edge vidí aj naďalej: na aký názov sa návštevník pýta, kedy a koľko dát
prejde. Z tunela pozná aj domácu adresu boxu. Passthrough stojí na názvoch
boxu tri veci: stránku pre nedostupný box, HTTP/3 a Encrypted Client Hello,
lebo každá z nich potrebuje, aby kľúče držal edge. Vlastné názvy edge si
všetky tri ponechávajú.

Rovnako sú pokryté vlastné domény, ktoré edge smeruje na box: odpoveď na
heartbeat boxu oznámi každý názov, ktorý k nemu vedie, a box si pre každý
vybaví certifikát. Pri [federácii](edge-federation.md) spoke zoznam boxu
posunie ďalej a hub smeruje `<spoke>.<box>` podľa názvu na spoke, ktorý ho
odovzdá boxu.

:::note[Ako sa to testuje]
`nix/tests/edge-passthrough.nix` (`losos-edge-passthrough`) spustí testovací
ACME server, edge a box. Kým box nemá certifikát, záznam prevádzky na strane
edge v tuneli ukazuje heslo, ktoré návštevník pošle. Keď si box cez edge
vybaví certifikát, návštevník ho overí voči koreňu ACME servera, Traefik má
pre názov iba passthrough trasu a to isté heslo v zázname nie je. Keď názov
ukáže na iný stroj, vlastná kontrola boxu prejde na **Zachytávané**.
:::

## Administrátorské UI zostáva len v LAN {#admin-ui-stays-lan-only}

Prevádzka z tunela prichádza do nginx z `127.0.0.1`, preto administrátorské
trasy odmietajú loopback. Nepridávajte k nim `allow 127.0.0.1`, inak bude
administrátorské UI dostupné z internetu.

## Edge v tej istej LAN {#edge-on-the-same-lan}

Edge nemusí byť VPS. S `losos.edge.lan.advertise = true` sa navyše ohlasuje
cez mDNS ako `_losos-edge._tcp`, s URL registrara v zázname `url=`
(`losos.edge.lan.url`, predvolene `http://<edge>.local:8443`). API registrara
naviaže mimo loopbacku a otvorí jeho port. Zariadenie v tej istej sieti ho potom
nájde bez akejkoľvek konfigurácie a môže cez neho zdieľať úložisko, pozri
[Mesh](/in-depth/mesh.md#finding-the-edge). Takto vyzerá nasadenie on-premises: jeden
stále zapnutý stroj vo firemnej sieti, na ktorom beží `nixosModules.edge`, a
vedľa neho zariadenia nainštalované zo štandardného ISO.

```nix
# the edge machine's configuration
imports = [ losos.nixosModules.edge ];
losos.edge = {
  enable = true;
  acmeEmail = "ops@example.com";       # a public name gets a certificate as usual
  lan.advertise = true;                # announce on the LAN, bind the API off-loopback
  tenants.<id>.tokenFile = "/run/secrets/tenant-<id>";
};
```

Zariadenia nepotrebujú nič. ISO nainštaluje zverejnenú konfiguráciu `install`,
panel Mesh ukáže "Okrajový proxy server nájdený: … V tejto sieti", len čo edge
odpovie, a zdieľanie sa dá zapnúť.

Takto si úložisko spája aj LAN bez internetu. Zariadenie si zväčší vlastný disk aj
bez akéhokoľvek edge (`losos-ctl grow`, režim Local). Spájanie naprieč
zariadeniami je však mesh a riadiaca vrstva meshu (server rke2, Longhorn,
registrar) beží na stroji s edge. Edge na PC v LAN je teda to, vďaka čomu
funguje lokalita offline. WAN môžete odpojiť, keďže zariadenia ho hľadajú výlučne
cez mDNS.

:::note[Dnes platia dve obmedzenia]
- Adresa tunela na registráciu (`losos.proxy.registrarUrl`) je voľba
  zostavenia, ktorá predvolene stále ukazuje na verejný edge. Lokalita, ktorá
  chce, aby tunel končil na jej vlastnom edge, ju nastaví vo svojom flaku.
- Ohlásenie ide cez mDNS, takže edge a zariadenia musia zdieľať jednu broadcastovú
  doménu (jednu VLAN).
:::

`nix/demo/edge-lan/run.sh` postaví celé toto usporiadanie na jednom stroji ako
VM na virtuálnom prepínači: edge, ktorý je zároveň routerom LAN, a dve zariadenia
nainštalované z ISO. Prejde overovací kontrolný zoznam
(`nix/demo/edge-lan/CHECKLIST.md`). Bez edge obe zariadenia odmietnu. Edge sa objaví,
obe ho nájdu a povolia zdieľanie. Edge zmizne, znova odmietnu, a potom sa
vráti. Jeho README hovorí, ktoré časti čo zastupujú v skutočnej lokalite a
čo ešte potrebujú kroky so spoločným úložiskom.

## Viac edge: federácia {#several-edges-federation}

Edge hostovaný používateľom môže preposielať svoje zariadenia cez oficiálny edge,
takže dve lokality, každá za vlastným lokálnym edge, sa navzájom dosiahnu cez
internet, a zariadenie uprednostní svoj lokálny edge, prepne sa na oficiálny a keď
neodpovedá ani jeden, vypne svoje funkcie závislé od edge. Návrh, atribút
tenanta na strane hubu (`relayZone`), `losos.edge.uplink.*` na strane spoke,
otvorená registrácia v LAN a obraz VM brány sú na stránke
[Federácia edge](/in-depth/edge-federation.md).

## Oficiálne edge {#official-edges}

Každý edge sa dá nájsť a môže preposielať úložisko. Iba edge, ktorý prejde
overením brány, smie spracúvať P2P obchodovanie s úložiskom a výpočtovým
výkonom ([trh](/in-depth/market.md)). Takýto edge, otvorenú bránu, môže
prevádzkovať ktokoľvek, ak dokáže, čo na ňom beží. Zariadenie si to overuje
samo, pri každom skenovaní, a edge, ktorý neprejde, dostane všetko okrem trhu.

Čo zariadenie od edge žiada:

- **Pas.** Vlastník edge podpíše svojím kľúčom OpenPGP krátke vyhlásenie:
  URL edge, vlastný kľúč Ed25519 edge (vytvorený na edge, súkromná polovica
  ho nikdy neopustí), kam ide jeho podiel z poplatku v každej sieti, a
  dátum platnosti. Zariadenie overí podpis aj kľúč (vlastné podpisy, nie je
  odvolaný, nevypršal) a ako vlastníka edge zobrazí user id kľúča.
- **Poplatky.** Nastavenia vedľa pasu delia každý predaj 2 % pre LosOS a
  2 % pre vlastníka a adresy LosOS sú presne tie, ktoré zariadenie má
  (`losos.proxy.gateways.payouts`).
- **TPM.** Kľúč endorsement v TPM edge nesie certifikát výrobcu a zariadenie
  ho overí voči výrobcom, ktorých pozná (`losos.proxy.gateways.tpmRoots`,
  predvolene balík TrustedTpm od Microsoftu). Zariadenie vytvorí credential,
  ktorý otvorí iba tento TPM.
- **Quote.** TPM podpíše hodnoty PCR 4, 11 a 15 spolu s čerstvým nonce
  zariadenia a kľúčom edge. PCR 4 a 11 musia ukazovať niektorý zo
  zverejnených obrazov brány (`losos.proxy.gateways.images`) a PCR 15 musí
  obsahovať hash presne tých nastavení, ktoré edge poslal. Edge svoje
  nastavenia zmeria pri štarte, skôr ako sa spustí registrar, a PCR 15 sa
  nedá vynulovať, takže quote ukazuje nastavenia, s ktorými tento štart
  začal.

Prenesú to dve výmeny: `GET /passport?nonce=<hex>` a `POST /passport/attest`.
Polovica zariadenia je `backend/access/src/gateway/check.rs`, polovica edge
`edge/registrar/src/gateway.rs`.

Výsledok je `official` pre každý edge v `GET /api/edge`, so stavom brány
vedľa, a značka pri názve každého edge v paneli Mesh: fajka pri overenej
bráne, s vlastníkom a obrazom v tooltipe; varovanie pri každom inom edge, s
tooltipom, ktorý vymenúva, čo pre zariadenie daný edge nedokáže; stopka pri
edge, ktorý zariadenie pristihlo pri manipulácii. Zariadenie môže mať v
dosahu viac edge. Zdieľanie funguje cez ktorýkoľvek z nich, obchodovanie iba
cez overený. Kým žiadny nie je v dosahu, relay trhu odpovedá
`{available: false, reason: "noOfficialEdge"}` a každú akciu odmietne s 409
`officialEdgeRequired`. Nič neopustí zariadenie.

Odmietnutie je dvojakého druhu. To, čo zariadenie ešte nepozná, napríklad
obraz, pre ktorý nemá merania, výrobcu TPM, pre ktorého nemá koreň, alebo
adresu poplatku, ktorá sa líši od tých, ktoré má, sa zobrazí a pri ďalšom
skenovaní sa overí znova. Dôkaz manipulácie, napríklad sfalšovaná
atestácia, nastavenia zmenené po štarte edge alebo firmvér, ktorý spustil
program mimo zverejneného obrazu, edge na danom zariadení natrvalo vypne,
podľa jeho URL, TPM a kľúča vlastníka, v `/var/lib/losos/gateways.json`.
Zariadenie potom daný edge nepoužíva na nič, ani na zdieľanie. Overenej
bráne zariadenie verí desať minút, potom sa pýta znova.

Vlastník edge si ho prevezme cez `POST /passport/settings`, podpísané jeho
kľúčom OpenPGP (`losos-registrar passport statement` vypíše text na
podpis, `passport settings` zostaví súbor). Edge súbor zapíše a reštartuje
sa, aby ho zmeral. Dovtedy odpovedá na `/passport` kódom 404 a nie je
overený. Edge bez `losos.edge.passport.enable` pas neposkytuje vôbec; tak
zvyčajne vyzerá vlastný edge firmy, vrátane toho, ktorý spúšťa
`nix/demo/edge-lan/`.

V tomto strome sú dva zoznamy prázdne: `nix/gateway/images.nix`, kým
obraz brány nenaštartuje ako unified kernel image, a `nix/gateway/payouts.nix`,
kým sa tam nezapíšu adresy LosOS. Kým je ktorýkoľvek prázdny, žiadny edge
neprejde a trh je vypnutý na každom zariadení. Predvolené nastavenie
zlyháva do zatvoreného stavu.

Firma, ktorá prevádzkuje vlastný edge, teda dostane funkčné nasadenie
on-premises a môže cez neho aj obchodovať, keď na ňom beží zverejnený obraz
s jej pasom a platí podiel LosOS. Cudzí, kto zmení obraz, nastavenia alebo
adresy poplatkov, dostane edge, cez ktorý žiadne zariadenie neobchoduje.

## DNS zóna a vlastné domény {#dns-zone-and-custom-domains}

Oficiálny edge môže poskytovať vlastnú DNS zónu a smerovať domény, ktoré
vlastníci zariadení už majú. Je vypnutý, kým nie je nastavené
`losos.edge.dns.enable`.

```nix
losos.edge.dns = {
  enable = true;
  zone = "boxes.losos.dasmat.us";   # default: "boxes.${publicDomain}"
  ipv4 = [ "203.0.113.7" ];         # the edge's public addresses; one family at least
  ipv6 = [ "2001:db8::7" ];
  # nameservers defaults to [ "ns1.<zone>" ], served with glue from ipv4/ipv6.
};
```

Registrar vykreslí zónový súbor pri každom zosúladení a Knot ho poskytuje na
porte 53, ktorý modul otvorí. Path unit znovu načíta Knot, keď sa súbor
zmení. Zóna obsahuje SOA, NS a glue, každý názov hostiteľa tenanta, ktorý
spadá do zóny, a jeden názov pre každé zariadenie, ktorého účet Stripe je overený:
16 hexadecimálnych znakov z SHA-256 UUID zariadenia. Každý názov sa prekladá na
edge, pretože tam sú Traefik a tunel. Vlastná adresa zariadenia sa nikdy neobjaví
vo verejnom DNS a rovnako ani id účtu Stripe či UUID. Sériové číslo je
unixový čas zmeny, alebo o jedno viac ako predchádzajúce sériové číslo, ak
je väčšie.

Operátor deleguje zónu raz, u registrátora nadradenej domény:

```
boxes.losos.dasmat.us.      NS  ns1.boxes.losos.dasmat.us.
ns1.boxes.losos.dasmat.us.  A   203.0.113.7
ns1.boxes.losos.dasmat.us.  AAAA 2001:db8::7
```

Zóna je podpísaná DNSSEC ([nižšie](#wallets-at-box-names)). Jej záznam DS
zverejnite u toho istého registrátora, aby resolvery podpisom dôverovali:
vypíše ho `keymgr <zone> ds` na edge. Kým tam DS nie je, resolvery berú
zónu ako nepodpísanú a prehliadač nemá dôvod veriť ani kľúču ECH nižšie.

:::note[Iba na prevzatej bráne]
Zóna potrebuje `losos.edge.passport.enable` a nič sa neprideľuje, kým edge
neprevezme jeho vlastník nastaveniami podpísanými jeho kľúčom a zmeranými
pri tomto štarte. Dovtedy zónu tvoria iba SOA, NS a glue a `/domains/*`
odpovedá 503.
:::

### Encrypted Client Hello {#encrypted-client-hello}

Každý názov v zóne nesie aj záznam HTTPS s kľúčom Encrypted Client Hello
(ECH) tohto edge:

```
mattbox IN HTTPS 1 . alpn="h3,h2" ech="AEz+DQBI…"
```

Prehliadač, ktorý záznam nájde, zašifruje tú časť svojho TLS hello, ktorá
menuje zariadenie, a sieti ukáže iba verejný názov edge,
`register.<publicDomain>`, ak `losos.edge.dns.ech.publicName` neurčuje iný.
Kto sleduje linku, vidí, že prehliadač išiel na edge, nie na ktoré
zariadenie. ECH je zapnuté so zónou (`losos.edge.dns.ech.enable`) a modul
odmietne zónu bez DNSSEC alebo bez ECH: nepodpísaný záznam by sa dal cestou
k prehliadaču zmeniť.

Edge si kľúč vytvorí pri prvom štarte (`losos-ech-key`,
`/var/lib/losos-ech/ech.pem`, čitateľný pre root a Traefik) a ponechá si ho,
kým sa verejný názov nezmení. `losos-registrar ech-key --key-file …
--public-name …` vypíše hodnotu, ktorú záznamy nesú. Traefik ním dešifruje.
Žiadny vydaný Traefik to zatiaľ nevie, preto edge so zónou spúšťa Traefik
zostavený z jeho zmeny pre ECH (`nix/traefik-ech.nix`), ktorý neposkytuje
dashboard.

Zariadenie toto od oficiálneho edge vyžaduje. Edge, ktorý prejde všetkými
kontrolami brány, zostane dôveryhodný iba na URL `https://`, ktorej názov má
záznam HTTPS overiteľný cez DNSSEC a s kľúčom ECH. lososd sa pri každom
hľadaní pýta `delv`, ktorý overuje sám od kľúča koreňa DNS, a edge, ktorý
neprejde, uvádza ako odmietnutý (`domainNotHttps`, `domainUnsigned` alebo
`domainNoEch`), kým neprejde. `losos-ctl edge-domain <url>` spustí tú istú
kontrolu ručne. URL je tá, ktorú menuje pas edge, takže tento názov patrí do
zóny: edge, ktorého zóna je celá `publicDomain`, v nej má aj
`register.<publicDomain>`. `losos.proxy.gateways.domain` kontrolu vypne
alebo ju nasmeruje na iný resolver a kotvu dôvery.

:::note[Ako sa to testuje]
`nix/tests/edge-dns.nix` (`losos-edge-dns`) prečíta kľúč z podpísaného
záznamu HTTPS, overí ho cez `delv` a otvorí TLS k názvu zariadenia klientom
ECH z Go. Traefik ECH prijme, zachytené hello nesie iba verejný názov a
kľúč, ktorý Traefik nemá, odmietne. `losos-ctl edge-domain`
prijme názov zariadenia, pri názve, ktorého neexistenciu zóna dokazuje,
kľúč nenájde, od nesprávnej kotvy dôvery nič neoverí a obyčajné HTTP
odmietne.
:::

Box, ktorý TLS ukončuje sám, nemá pri svojich názvoch záznam HTTPS, lebo
hello dokáže rozšifrovať iba ten, kto drží kľúč ECH, a edge spojenia tohto
boxu posúva neotvorené. Jeho názov potom ide v TLS hello nešifrovane, ako
pred ECH.

### Vlastné domény {#custom-domains}

Zariadenie pridá doménu cez `POST /domains/add` (overené jeho tokenom tunela,
preposlané cez lososd z Nastavenia → Sieť). Edge ju prijme iba vtedy, keď
je pripojený účet Stripe zariadenia pripravený a nesie UUID zariadenia, čo je tá istá
kontrola, akú robí trh pred výplatou. Potom vlastník zverejní dva záznamy:

- `_losos-challenge.<domain> TXT "losos-domain-v1=<32 hex>"`, kde hex je
  SHA-256 z domény, UUID zariadenia a id účtu Stripe. Dokazuje kontrolu nad
  doménou a viaže ju na to zariadenie a ten účet. Záznam, ktorý tam nechal
  predchádzajúci vlastník, nedokazuje nič.
- `<domain> CNAME <label>.<zone>`, alebo záznamy A/AAAA s adresami edge na
  vrchole zóny.

Registrar si oba vyhľadá cez DNS-over-HTTPS (`losos.edge.dns.checkUrl`,
predvolene JSON API Cloudflaru), najviac 16 na jeden prechod, každých 30 s,
kým nárok čaká, a každú hodinu, keď je aktívny. Až keď sú oba záznamy
správne, dostane doména router Traefiku s Let's Encrypt, smerovaný cez
tunel zariadenia. Čakanie aj na CNAME bráni tomu, aby Traefik žiadal Let's
Encrypt o názov, ktorý nevedie na edge. Aktívna doména zanikne po troch
neúspešných opätovných kontrolách za sebou, alebo okamžite, keď účet Stripe
prestane byť pripravený. Zariadenie môže mať päť domén, doména patrí jednému zariadeniu a
neoverený nárok sa po siedmich dňoch zahodí. Nároky sú uložené v
`/var/lib/losos-registrar/domains.json`.

Na zariadení lososd udržiava aktívne domény v
`/var/lib/losos-public-names/domains.json`. Súbor obnovuje každé dve
minúty a vždy, keď o to požiada panel Sieť. LosOS Login ho číta, aby
cookie svojej relácie dal doménu pre každý z týchto názvov
(`modules/identity.nix`). LosOS Cloud a LosOS Git odpovedajú na ktorýkoľvek
názov, cez ktorý požiadavka prišla, takže zoznam nepotrebujú.

Zariadenie za lokálnym edge dostane svoje domény smerované tiež, cez ten lokálny
edge, len čo sa za lokálny edge zaručilo relay passom a pripojilo sa k meshu
tohto edge. Registrar si tieto trasy uchováva sám, v `relay-routes.json`
vedľa svojho registra. [Federácia
edge](/in-depth/edge-federation.md#custom-domains-behind-a-local-edge) obsahuje
podrobnosti a `losos.edge.dns.relayRoutes`.

### Peňaženky pod názvami zariadení {#wallets-at-box-names}

Zariadenie prepojí svoju peňaženku so svojimi názvami cez `POST /domains/wallet`
(`{address, signature}`, lososd to prenáša z Nastavenia → Sieť). Podpis je
`personal_sign` peňaženky nad správou

```
LosOS box name v1
zone: <zone>
box: <box id>
address: <0x address, lowercase>
```

takže na inom edge ani pre iné zariadenie nič neprepojí. Registrar zistí
podpisovateľa, uloží adresu aj podpis do `domains.json` a pod každý názov
zariadenia v zóne, jeho hostname aj jeho štítok, zapíše jeden záznam TXT:

```
mattbox IN TXT "ENS1 dnsname.ens.eth 0x…"
```

To je bezplatné DNS rozlíšenie ENS (ENSIP-17): peňaženka sa spýta ENS na
Ethereum mainnet, brána DNSSEC od ENS stiahne záznam TXT, kontrakt overí
podpisy až po koreň DNS a `dnsname.ens.eth`, prvý DNS resolver od ENS, vráti
adresu. `addr` vracia zakódované podľa ABI, tak, ako ho dekóduje každá
peňaženka. Jeho nástupca, `2.dnsname.ens.eth`, číta kľúče pre jednotlivé
siete (`a[e8453]=…`), ale vracia holých 20 bajtov, ktoré predvolené
vyhľadávanie vo viem ani ethers nedekódujú, takže peňaženka kupujúceho by
nenašla nič. Peňaženka zariadenia má na každej sieti EVM jednu adresu, preto
stačí odpoveď pre Ethereum. `{}` prepojenie zruší.

Volanie nesie token zariadenia, takže prepojiť alebo zrušiť ho môže len
zariadenie, a lososd oň požiada len za správcu (sekcia `network`). Podpis
ukazuje, že peňaženka s názvom tohto zariadenia súhlasí. Nezabráni však edge,
ktorý zónu podpisuje, aby podával niečo iné: názov je taký dôveryhodný ako
edge.

ENS číta len podpísanú zónu, takže Knot túto zónu podpisuje
(`losos.edge.dns.dnssec.enable`, predvolene zapnuté) svojou predvolenou
politikou, ECDSA P-256 s kľúčmi v `/var/lib/knot`. Resolvery podpisom
dôverujú, keď nadradená zóna zverejní záznam DS zóny, ktorý na edge vypíše
`keymgr <zone> ds`. Zverejnite ho vedľa záznamov NS vyššie.

To, že Knot odpovedá, ešte neznamená, že peňaženka názov nájde. Kým je
peňaženka prepojená, reconciler sa pýta resolvera DNS-over-HTTPS
(`losos.edge.dns.checkUrl`) na záznam TXT pri každom názve zariadenia, tak
ako by sa pýtala brána ENS, a hlási to pre každý názov vo `wallet.published`:
`found`, keď záznam prejde overením (bit AD resolvera), `unsigned`, kým
nadradená zóna nezverejní DS, `missing`, kým zónu nedeleguje, `other`, kým
resolver drží starší záznam. Názov, ktorý sa ešte nenašiel, sa znova vyhľadá
každú pol minútu, nájdený raz za hodinu. Panel Sieť to ukazuje pri každom
názve.

Peňaženka zariadenia rozlišuje názvy cez ten istý relay ako svoje čítania z
reťazca (`/api/market/rpc` s `network: "ens"`). Vyhľadanie názvu DNS má krok
mimo reťazca (EIP-3668): resolver sa vráti s URL brány, stránka pošle
`ens_ccipRead` s odosielateľom a calldata a edge ich odošle na
`https://dnssec-oracle.ens.domains/`, jedinú bránu, ktorej sa pýta, a vráti
odpoveď, ktorú overí kontrakt. CSP stránky správcu jej stále dovolí hovoriť
len s vlastným zariadením.

Vlastná doména majiteľa dostane ten istý záznam, ukázaný na paneli, na
pridanie priamo k doméne. Kým je peňaženka prepojená, každá kontrola vyhľadá
aj TXT pri doméne a hlási `wallet`: `found` (celý záznam tak, ako ho edge
zverejňuje, overený resolverom cez DNSSEC), `unsigned`, `cname` (CNAME
nenesie TXT a ENS číta len presný názov), `other` alebo `missing`. Nikdy nerozhoduje o tom, či sa doména smeruje.

:::note[Ako sa to testuje]
`nix/tests/edge-dns.nix` (`losos-edge-dns`) spustí edge a klienta a pýta sa
Knotu cez UDP aj TCP, prepojí peňaženku a jej podpísaný záznam overí cez
`delv` od záznamu DS zóny. `nix/tests/box-wallet-name.nix`
(`losos-box-wallet-name`) pridá zariadenie s LosOS Login a notebook: majiteľ
prepojí peňaženku v Chromiu na paneli Sieť cez lososd a záznam prejde
overením. Náhrada verejného DNS na edge potom neodpovie nič, nepodpísaný
záznam a overený záznam, ako test zónu deleguje a zverejní jej DS, a panel
nasleduje každý krok. `edge/registrar/tests/domains.rs` prevedie tok
nárokov aj peňaženiek proti skutočnému registraru a falošnému serveru
DNS-over-HTTPS. `admin-ui/app/tests/ens.unit.mjs` cez relay prehrá
vyhľadanie názvu DNS zaznamenané z Ethereum mainnet a brány ENS.
:::

## Demo nasadenie na Verceli {#demo-deployment-on-vercel}

`edge/vercel/` spúšťa API registrara, ten istý router a tú istú kontrolu
tokenu, ako Vercel Function na `https://losos-edge.dasmat.us`, čo je
predvolená hodnota `losos.proxy.registrarUrl`. Register je uložený v Neon
Postgres a stavová stránka na `/` ukazuje registrované zariadenia a konfiguráciu
Traefiku, ktorú by pre ne edge zapísal. Je to iba riadiaca vrstva. Tunel,
TLS Traefiku, mesh a trh nemôžu bežať na serverless hostiteľovi,
takže dátová cesta, teda dosiahnutie zariadenia cez jeho verejný názov hostiteľa,
stále potrebuje VPS. Stránka navyše ukazuje id a názvy hostiteľov
komukoľvek, kto má URL. Je to demonštračný hostiteľ, nie edge. Kroky
nasadenia a premenné prostredia sú v
[edge/vercel/README.md](https://github.com/dasmatus/losos/blob/main/edge/vercel/README.md).
