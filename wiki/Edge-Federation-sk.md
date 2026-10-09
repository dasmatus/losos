[English](Edge-Federation) · **Slovenčina** · [Deutsch](Edge-Federation-de)

# Federácia edge

Edge si môže pri svojich boxoch prevádzkovať ktokoľvek ([Master proxy — Edge
v tej istej LAN](Master-Proxy-sk#edge-on-the-same-lan)). Táto stránka opisuje,
ako sa taký **lokálny edge** pripojí k **oficiálnym edge**, aby bol box za ním
dosiahnuteľný odkiaľkoľvek, ako si box vyberá, ktorý edge použije, a ako sa
lokálny edge dodáva bez druhého inštalačného ISO.

## Tvar

Hub a spoke. Oficiálny edge je **hub**. Edge, ktorý prevádzkuje používateľ, je
**spoke**: ponechá si všetko, čo má dnes (registrar, koncový bod tunela,
ohlasovanie v LAN, voliteľne riadiacu rovinu mesh), a pridá jeden **uplink**
k hubu. Dve lokality, každá za vlastným spoke, sa navzájom dosiahnu cez hub;
box sa nikdy nerozpráva priamo so spoke inej lokality.

![Lokalita A: box sa cez rathole pripája k spoke A a ohlasuje sa mu; spoke A sa cez rathole uplink pripája k hubu na internete a odovzdáva mu box cez POST /relay; lokalita B je zrkadlová. Každý spoke prevádzkuje Traefik v LAN; hub prevádzkuje Traefik s verejným TLS, ktorý smeruje podľa Host(<box>.<zone>).](images/edge-federation-sk.svg)

Tri roviny, z ktorých sa federujú iba dve:

| Rovina  | Čo                                               | Federovaná?                                                                |
| ------- | ------------------------------------------------ | -------------------------------------------------------------------------- |
| control | API registrara: register, heartbeat, join, market | spoke preposiela svoje boxy hubu (`POST /relay`); market nie              |
| data    | tunely rathole + Traefik                         | o jeden skok viac: hub → spoke → box                                       |
| mesh    | rke2 server + Longhorn                           | **nie**: každý edge je samostatný cluster, pozri „Čo zostáva lokálne“      |

## Strana hubu: tenant, ktorý smie preposielať

Spoke je bežný tenant hubu s jedným atribútom navyše, so zónou, pod ktorou
smie preposielať:

```nix
# on the hub (an official edge)
losos.edge.tenants.acme = {
  hostname = "acme.losos.cfd";           # the spoke's own name, as any tenant
  tokenFile = "/var/secrets/tenant-acme";
  relayZone = "acme.losos.cfd";          # it may relay <label>.acme.losos.cfd
};
```

Spoke v pravidelných intervaloch posiela `POST /relay {appliance_id: "acme",
token, tenants: [{id, hostname}, …]}`, overený presne ako `/register`
(rovnaký token, rovnaká kontrola v konštantnom čase, rovnaký limit veľkosti
tela). Hub prijme preposielaný box iba vtedy, ak je jeho id DNS label a jeho
hostname je **presne jeden label pod zónou** (`mattbox.acme.losos.cfd`), a
množinu preposielaných boxov drží zhodnú s posledným zoznamom, ktorý dostal:
box, ktorý spoke prestane uvádzať, pri ďalšom reconcile zmizne, a spoke, ktorý
stíchne, si po uplynutí heartbeat TTL odnesie všetky svoje boxy so sebou ako
ktorýkoľvek tenant. Preposielané záznamy žijú v registri hubu ako
`<spoke>.<box>` s `via: <spoke>`, dostanú port z toho istého rozsahu rathole,
router Traefiku `Host(<hostname>)` a službu rathole, ktorej token je token
**spoke**: overenou stranou je spoke, hub nikdy nedrží token boxu. Autorita
nad hostname zostáva tam, kde bola: zónu zvolil prevádzkovateľ hubu; labely v
nej volí spoke; čokoľvek iné sa odmietne pre každý box zvlášť a spoke to
zaloguje.

Prečo nenechať boxy spoke registrovať sa na hube priamo? Pretože potom by
prevádzkovateľ hubu evidoval každý box na svete, a práve tomu sa má lokálna
brána vyhnúť. Jeden riadok tenanta na lokalitu je celá cena lokality na
oficiálnom edge.

## Strana spoke: uplink

```nix
# on the local edge
losos.edge = {
  enable = true;
  lan.advertise = true;                     # boxes find it by DNS-SD
  lan.openEnrolment = true;                 # boxes on the LAN enrol themselves
  uplink = {
    enable = true;
    registrarUrl = "https://register.losos.cfd";   # the hub's API
    ratholeEndpoint = "edge.losos.cfd:2333";       # the hub's tunnel port
    id = "acme";                                   # the tenant row above
    tokenFile = "/var/secrets/losos-uplink-token";
    # bootstrapTokenFile is optional: every relayed service carries the
    # uplink token, so rathole's default_token is never consulted.
  };
};
```

Reconciler registrara, keď zapíše vlastné súbory pre Traefik a rathole,
prepošle hubu svojich **živých** tenantov (registrovaných, na whiteliste
alebo zapísaných, posielajúcich heartbeat) a vygeneruje
`/etc/rathole/uplink.toml`: konfiguráciu *klienta* rathole s jednou službou
na každý preposielaný box, `local_addr = 127.0.0.1:<port toho boxu na tomto
spoke>`, token = token uplinku. Druhá jednotka rathole,
`losos-rathole-uplink`, ju spúšťa (štartuje ju path unit v okamihu, keď súbor
vznikne); rathole súbor načíta za behu rovnako ako konfiguráciu servera.
Verejný kľúč Noise hubu sa pripne pri prvom kontakte (`GET
/noise-public-key`, zapíše ho do `uplink.noisePublicKeyFile` sám registrar),
tak ako box pripína kľúč svojho edge. Požiadavka na
`https://mattbox.acme.losos.cfd/` teda ukončí TLS na hube, vstúpi do služby
rathole hubu `acme.mattbox`, prejde uplinkom na lokálny port spoke pre
`mattbox`, prejde tunelom boxu a skončí v Nginxe boxu. Dva tunely, jedna
hlavička Host, žiadna zmena na boxe.

**Otvorený zápis** (open enrolment) umožňuje použiť bežný box s bránou, pre
ktorú nikto nepripravil tokeny: s `lan.openEnrolment` sa neznáme id
zariadenia, ktoré zavolá `/register`, zapíše systémom trust-on-first-use —
jeho token a hostname sa uložia pod `/var/lib/losos-registrar/enrolled/` a
každá ďalšia požiadavka pre toto id sa s nimi musí zhodovať. Udeľuje iba
členstvo v proxy (nikdy `cluster` ani `market`), na edge, ktorý sa neohlasuje
v LAN, sa odmietne (VPS ho nesmie nastaviť nikdy) a ohlasovanie to uvádza
(`enrol=open`). `losos-registrar enrol list|forget --dir
/var/lib/losos-registrar/enrolled` (na bráne `losos-edge boxes` a
`losos-edge forget <id>`) zobrazí a odstráni zapísané boxy; box, ktorý stále
posiela heartbeat, sa znova zapíše s tokenom, ktorý drží, takže zabudnutie je
pre box, ktorý odišiel. Hub ponecháva zatvorený zápis.

## Box: ktorý edge a keď žiadny

`lososd` už nájde každý edge v dosahu (v LAN cez DNS-SD, nakonfigurovaný
oficiálny podľa URL) a odmietne zapnúť zdieľanie, keď nie je žiadny. Jednu
cestu vyberie podľa tohto pravidla:

1. **najprv lokálny edge** — prvé ohlásenie v LAN, ktorého `/health`
   odpovedá; ohlásenie teraz nesie aj `rathole=<host:port>`, aby box vedel,
   kam vedie jeho tunel;
2. **potom oficiálny edge** — `losos.proxy.registrarUrl` a
   `losos.proxy.edgeRatholeEndpoint`, keď v LAN žiadny nie je;
3. **žiadny** — každá funkcia závislá od edge je vypnutá: jednotky tunela a
   announce sú zastavené, brána zdieľania odmieta, relay marketu odmieta.

`GET /api/edge` nesie voľbu ako `path` (`{name, url, rathole, source}` alebo
`null`) vedľa koncového bodu `rathole` každého edge. So zapnutým
`losos.proxy.enable` zapíše `lososd` cestu do `/run/losos/edge-path.env`
(`LOSOS_EDGE_PATH_URL`, `_RATHOLE`, `_SOURCE`, `_NOISE_PUB`) a podľa nej
riadi dve jednotky tunela: zmena cesty reštartuje `losos-rathole-client` a
`losos-registrar-announce`, žiadna cesta zapíše `/run/losos/edge-none` a
zastaví ich (obe jednotky majú `ConditionPathExists=!/run/losos/edge-none`,
takže ich nič nereštartuje, kým sa edge nevráti). Oba súbory sú na `/run`:
až do prvého skenovania po štarte jednotky volajú nakonfigurovaný edge, ako
vždy. Kľúč Noise každého edge je pripnutý vo vlastnom súbore — kľúč
nakonfigurovaného edge v `losos.proxy.noisePublicKeyFile`, kľúč edge v LAN
pod `/var/secrets/losos-edge-pins/<host>_<port>.pub` — vlastným
`ExecStartPre` klienta, takže druhá brána nikdy nezdedí kľúč prvej. Token
boxu je rovnaký na každej ceste — hub ho má na whiteliste, brána sa ho
dozvie pri prvom kontakte a `lososd` ho (aj bootstrap token) vytvorí pri
prvom štarte, keď súbory chýbajú — a rovnaké je aj jeho verejné meno:
`losos.proxy.hostname` vlastníka musí byť jeden label pod zónou lokality
(`mattbox.acme.losos.cfd`), aby ho relay prijal, a je to zároveň meno,
ktorému dôveruje Nextcloud. Box, ktorý sa vráti k oficiálnemu edge, si cez
svoj priamy riadok tenanta ponechá rovnaké meno. Relay marketu nepotrebuje
žiadny ďalší prepínač: je podmienený odpoveďou *oficiálneho* edge a keď
žiadny nie je v dosahu, už hlási `available: false`.

`tests/edge-federation.nix` spúšťa tri stroje (hub, brána, box) a prechádza
presne tým: cesta cez LAN, zápis, uplink, návrat k oficiálnemu edge,
vypnuté, späť.

## Čo zostáva lokálne

- **Mesh.** Replikácia Longhorn a výpočty v mesh bežia v jednom clustri rke2
  a tým clustrom je edge, ku ktorému sa box pripojil. Spoke s
  `losos.edge.cluster.enable` spája boxy svojej vlastnej lokality; hub
  nepremosťuje clustre dvoch lokalít a nič tu nepreposiela 9345/6443 cez
  uplink. Spájanie naprieč lokalitami je ďalší krok s vlastným návrhom.
- **Market.** Obchodovanie box odmietne, pokiaľ nie je v dosahu *oficiálny*
  edge ([Oficiálne edge](Master-Proxy-sk#official-edges)); spoke nie je
  nikdy oficiálny a preposielaný box nie je tenantom hubu, takže v jeho mene
  nemožno uskutočniť žiadne volanie `/market/*`. Relay prenáša dosiahnuteľnosť
  cez HTTP, nič iné.
- **Identita.** Spoke nepreposiela `/identity`; box, ktorý chce market,
  dosiahne oficiálny edge sám, cez vlastné internetové pripojenie.

## Vlastné domény za lokálnym edge

[Vlastné domény](Master-Proxy-sk#custom-domains) fungujú aj pre box za
lokálnym edge, s jedným pravidlom. Doménu smeruje iba oficiálny edge. Lokálny
edge nikdy neobsluhuje zónu, nikdy nekontroluje záznam a nikdy o nič nežiada
Let's Encrypt. Prenáša prevádzku oficiálneho edge k boxu tou istou
preposielanou službou, ktorá už prenáša vlastný hostname boxu.

Ťažká časť je dôvera. Lokálny edge je niečí stroj a jeho zoznam `/relay`
hovorí „môj box `mattbox`“. Keby tomu oficiálny edge uveril a smeroval tam
domény `mattbox`, ktokoľvek s bránou by mohol pomenovať svoj box po cudzom,
prevziať jeho doménu a získať pre ňu platný certifikát. Box preto musí sám
povedať, za ktorým lokálnym edge je, a lokálny edge to za box povedať nemôže.

Box to robí pomocou **relay pass**:

1. Box sa pýta oficiálneho edge na prehľad svojich domén, ako to už robí
   každé dve minúty, cez vlastné internetové pripojenie a s vlastným tokenom.
   Odpoveď teraz nesie `relay_pass`, `v1.<hour>.<64 hex>`. Hex časť je
   HMAC-SHA256 nad id boxu a hodinou, pod kľúčom
   `/var/lib/losos-registrar/relay-pass.key`. Ten kľúč nikdy neopustí
   oficiálny edge a registrar ho vytvorí pri prvom štarte.
2. lososd zapíše pass do `/run/losos/relay-pass` (iba pre root) a vynechá ho
   z toho, čo dostane administrátorská stránka.
3. `losos-registrar announce` súbor načíta pri každom register a heartbeat a
   pošle pass tomu edge, ktorý box práve používa. Lokálny edge si ho drží v
   pamäti, a to iba vtedy, ak má správny tvar.
4. Uplink lokálneho edge posiela pass každého boxu spolu s boxom v
   `POST /relay`. Oficiálny edge ho overí. Platný pass naviaže box na tento
   lokálny edge.

Pass platí v hodine, v ktorej bol vydaný, a v dvoch nasledujúcich. Lokálny
edge, ktorý box opustil, môže to, čo naposledy videl, prehrávať najviac
počas tejto doby, a nikdy nie cez novší pass z iného lokálneho edge, pretože
väzba sa presunie iba na pass, ktorý je aspoň rovnako nový. Lokálny edge môže
preposielať svoje boxy aj bez neho. Tie len nedostanú žiadne domény.

Okrem väzby musí byť box **v mesh**. Musí sa pripojiť ku clustru tohto
oficiálneho edge cez `/cluster/join`, čím sa pod jeho vlastným id zaznamená
jeho výpočtové okno. Box, ktorý má iba tunel, dostane svoj hostname a nič
viac. A ak je box zaregistrovaný aj priamo na oficiálnom edge, vyhráva
priama cesta a lokálny edge sa nepoužije.

### Tabuľka trás

Registrar si tabuľku vedie sám, rovnako ako svoj register. Nie je potrebná
žiadna databáza. Každý priechod reconcilera zistí, ktoré trasy majú
existovať, a podľa toho zostaví routery Traefiku. Keď sa tabuľka zmení,
registrar prepíše `/var/lib/losos-registrar/relay-routes.json` vedľa
`registry.json` a zaloguje každú trasu, ktorú pridá alebo odstráni:

```json
{
  "bindings": {
    "mattbox": { "spoke": "acme", "epoch": 493281 }
  },
  "routes": [
    { "domain": "cloud.example.org", "tenant": "mattbox", "spoke": "acme", "service": "acme.mattbox" }
  ]
}
```

`bindings` zaznamenáva, za ktorý lokálny edge sa každý box zaručil a passom
z ktorej hodiny. Práve túto časť načíta reštart, takže trasy sa vrátia pri
prvom priechode registrara a nečaká sa, kým každý lokálny edge znova zavolá
`/relay`. Väzba, ktorej pass vypršal, sa ignoruje. `routes` je tabuľka, ktorú
tieto väzby vytvorili, zapísaná na čítanie pre vás. Registrar ju znova
naplánuje z väzieb a nikdy ju nenačítava. Meno boxu v zóne edge,
`<label>.<zone>`, ide rovnakou cestou ako jeho domény, takže k nemu sa
dostane aj cieľ CNAME.

Lokálny edge dostane svoje riadky späť v odpovedi na `/relay`. Brána ich
zapíše do `/var/lib/losos-registrar/hub-routes.json` a zaloguje každú zmenu.
Ten súbor vám iba hovorí, čo oficiálny edge smeruje k vám. Nič na lokálnom
edge sa podľa neho nesmeruje.

### Zapnutie

Na oficiálnom edge:

```nix
losos.edge.dns = {
  enable = true;                 # the zone and custom domains
  relayRoutes.enable = true;     # and the route table
};
```

Nič ďalšie pre to nebeží. Súbor tabuľky aj kľúč pre pass sú v stavovom
adresári registrara. Na lokálnom edge ani na boxe sa nič nemení. Oba si pass
prevezmú od zostavenia s touto zmenou.

`backend-registrar/tests/domains.rs` prechádza celú cestu proti skutočným
registrarom. Box za bránou dostane svoju doménu, box mimo mesh nedostane nič
a tri podvrhnuté passy nedostanú nič. `tests/edge-dns.nix` naštartuje
zapojenie v NixOS. Kontroluje súbor s kľúčom, `/relay` s passom, väzbu, ktorá
skončí v `relay-routes.json`, a to, že väzba po reštarte registrara stále
existuje.

## Dodanie: brána bez ISO

Dve podoby, jedna konfigurácia:

1. **Obraz VM**, `nix build .#losos-disk-edge-qcow2` (`nix run
   .#losos-disk-edge-qcow2-run` ho spustí v QEMU), priložený ku každému
   označenému vydaniu ako `losos-edge-gateway-<tag>.qcow2`. Je to
   `nixosConfigurations.edge-gateway`: modul edge s
   `losos.edge.gateway.enable` (`modules/edge-gateway.nix`), ktorý nastaví
   `lan.advertise` a `lan.openEnrolment`, uplink číta za behu z
   `/var/lib/losos-edge/uplink.json`, takže jeden obraz poslúži každej
   lokalite, pri prvom štarte si vytvorí vlastný bootstrap token, dá rootu
   konzolu (prvé heslo `losos`, zmena vynútená pri prvom prihlásení; sshd je
   nainštalovaný, ale zastavený, kým nezadáte `losos-edge ssh on`) a na tty1
   vypíše svoju adresu. Spustite ho na Proxmoxe, libvirt alebo VirtualBoxe so
   sieťovou kartou v LAN; boxy nainštalované zo štandardného ISO ho nájdu do
   minúty. Potom na jeho konzole:

   ```sh
   losos-edge status
   losos-edge uplink set --id acme --token-file /root/acme.token \
     --registrar https://register.losos.cfd --rathole edge.losos.cfd:2333
   losos-edge boxes
   ```

   s riadkom tenanta, ktorý vám dal prevádzkovateľ hubu. `losos-edge uplink
   clear` preposielanie zastaví.
2. **Existujúci stroj s NixOS**: importujte `nixosModules.edge` a buď
   nastavte `losos.edge.gateway.enable = true` (tvar obrazu), alebo zapíšte
   voľby uvedené vyššie ručne, ako to robí demo s dvoma VM a
   `tests/edge-lan.nix`.

Strana hubu pre oficiálne edge je jeden riadok tenanta na lokalitu. Hostiteľ
Vercel má tú istú trasu `/relay`, a preto môže spoke na demo *prijať*, ale
keďže na ňom nebeží rathole, dátová cesta ako predtým potrebuje VPS.
