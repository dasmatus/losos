---
title: Master proxy
sidebar_position: 5
mdx:
  format: md
---

# Master proxy

Voliteľné. Sprístupní zariadenie z internetu bez toho, aby ste museli doma otvárať
port.

![internet → Traefik (VPS, :443) → rathole server ⇐ tunel ⇐ rathole klient (zariadenie) → nginx](/img/diagrams/master-proxy-sk.svg)

## Edge (VPS)

Importujte výstup flaku `nixosModules.edge` do konfigurácie VPS a nastavte
`losos.edge.*`:

- `losos.edge.publicDomain` je doména, pod ktorou sa zariadenia sprístupňujú.
- `losos.edge.tenants` je zoznam povolených zariadení, každé s názvom hostiteľa
  a súborom s tokenom. Registrar vytvára trasy a certifikáty iba pre
  identifikátory, ktoré sú uvedené tu.
- `losos.edge.cluster.enable` navyše spúšťa riadiacu vrstvu [mesh](/in-depth/mesh.md),
  server rke2 s Longhornom. Proxy funguje aj bez nej.

`losos-registrar` udržiava dynamickú konfiguráciu Traefiku a serverovú
konfiguráciu rathole v súlade s registrovanými zariadeniami.

## Zariadenie

Nastavte `losos.proxy.enable = true` a voľby `losos.proxy.*`
(`registrarUrl`, `edgeRatholeEndpoint`, `hostname`, `applianceId`,
`tokenFile`).

## Šifrovanie tunela

rathole používa svoj transport Noise. Edge si pri prvom štarte vygeneruje pár
kľúčov. Každé zariadenie si pri prvom spustení stiahne verejný kľúč z
registrara a uloží ho do `losos.proxy.noisePublicKeyFile`. Ak chcete kľúč
pripnúť mimo tohto kanála, vložte súbor na to miesto vopred; zariadenie ho
nikdy neprepíše. Nastavenie ktorejkoľvek z dvoch kľúčových volieb na `null`
prepne späť na obyčajné TCP. Nerobte to.

## Administrátorské UI zostáva len v LAN

Prevádzka z tunela prichádza do nginx z `127.0.0.1`, preto administrátorské
trasy odmietajú loopback. Nepridávajte k nim `allow 127.0.0.1`, inak bude
administrátorské UI dostupné z internetu.

## Edge v tej istej LAN

Edge nemusí byť VPS. S `losos.edge.lan.advertise = true` sa navyše ohlasuje
cez mDNS ako `_losos-edge._tcp`, s URL registrara v zázname `url=`
(`losos.edge.lan.url`, predvolene `http://<edge>.local:8443`). API registrara
naviaže mimo loopbacku a otvorí jeho port. Zariadenie v tej istej sieti ho potom
nájde bez akejkoľvek konfigurácie a môže cez neho zdieľať úložisko, pozri
[Mesh](/in-depth/mesh.md#hľadanie-edge). Takto vyzerá nasadenie on-premises: jeden
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
cez mDNS. Dnes platia dve obmedzenia:

- Adresa tunela na registráciu (`losos.proxy.registrarUrl`) je voľba
  zostavenia, ktorá predvolene stále ukazuje na verejný edge. Lokalita, ktorá
  chce, aby tunel končil na jej vlastnom edge, ju nastaví vo svojom flaku.
- Ohlásenie ide cez mDNS, takže edge a zariadenia musia zdieľať jednu broadcastovú
  doménu (jednu VLAN).

`demo/edge-lan/run.sh` postaví celé toto usporiadanie na jednom stroji ako
VM na virtuálnom prepínači: edge, ktorý je zároveň routerom LAN, a dve zariadenia
nainštalované z ISO. Prejde overovací kontrolný zoznam
(`demo/edge-lan/CHECKLIST.md`). Bez edge obe zariadenia odmietnu. Edge sa objaví,
obe ho nájdu a povolia zdieľanie. Edge zmizne, znova odmietnu, a potom sa
vráti. Jeho README hovorí, ktoré časti čo zastupujú v skutočnej lokalite a
čo ešte potrebujú kroky so spoločným úložiskom.

## Viac edge: federácia

Edge hostovaný používateľom môže preposielať svoje zariadenia cez oficiálny edge,
takže dve lokality, každá za vlastným lokálnym edge, sa navzájom dosiahnu cez
internet, a zariadenie uprednostní svoj lokálny edge, prepne sa na oficiálny a keď
neodpovedá ani jeden, vypne svoje funkcie závislé od edge. Návrh, atribút
tenanta na strane hubu (`relayZone`), `losos.edge.uplink.*` na strane spoke,
otvorená registrácia v LAN a obraz VM brány sú na stránke
[Federácia edge](/in-depth/edge-federation.md).

## Oficiálne edge

Každý edge sa dá nájsť a môže preposielať úložisko. Iba edge, ktoré
prevádzkuje LosOS, smú spracúvať P2P obchodovanie s úložiskom a výpočtovým
výkonom ([trh](/in-depth/market.md)). Zariadenie si to overuje samo, pri každom skenovaní, a
vlastný edge firmy dostane všetko okrem trhu.

Overenie je identita Ed25519:

- **Koreňový kľúč LosOS.** Jeden pár kľúčov. Verejná polovica je súbor,
  ktorý má každé zariadenie, `keys/official-edge-root.pub` v tomto repozitári
  (`losos.proxy.officialRootKeyFile`). Súkromnú polovicu drží vlastník
  projektu offline. Nikdy sa nedostane do repozitára ani do žiadneho zariadenia a
  podpisuje certifikáty edge a nič iné. Kým niekto nezapíše verejný kľúč do
  tohto súboru, žiadny edge nie je oficiálny a trh je vypnutý na každom zariadení
  zostavenom zo stromu. Predvolené nastavenie zlyháva do zatvoreného stavu.
- **Certifikát edge.** Každý oficiálny edge má vlastný pár kľúčov a malý
  JSON certifikát `{name, url, public_key, not_after, signature}` podpísaný
  koreňom. Registrar odpovedá na `GET /identity?nonce=<hex>` certifikátom a
  podpisom nonce kľúčom edge.
- **Overenie na zariadení** (`backend/src/edge.rs`) beží pre každý edge, ktorého
  `/health` odpovedal. Koreň podpísal certifikát, certifikát uvádza URL, s
  ktorou zariadenie hovorí, neexpiroval a kľúč edge podpísal nonce, ktorý si zariadenie
  práve vymyslelo. Prejdú všetky štyri, inak edge nie je oficiálny. Výsledok
  je `official` pre každý edge v `GET /api/edge` a značka pri názve každého
  edge v paneli Mesh: fajka pri oficiálnom edge, varovanie pri každom inom,
  s tooltipom, ktorý vymenúva, čo pre zariadenie daný edge nedokáže. Zariadenie môže mať v
  dosahu viac edge. Zdieľanie funguje cez ktorýkoľvek z nich, obchodovanie
  iba cez oficiálny. Kým nie je v dosahu žiadny oficiálny edge, relay trhu
  odpovedá `{available: false, reason:
  "noOfficialEdge"}` a každú akciu odmietne s 409
  `officialEdgeRequired`. Nič neopustí zariadenie.

Edge si pri prvom spustení vytvorí vlastný kľúč identity
(`losos.edge.identity.keyFile`, predvolene
`/var/lib/losos-registrar/identity.key`) a súkromná polovica nikdy neopustí
edge. Pre oficiálny edge podpíše LosOS koreňom certifikát a nainštaluje ho
tam; edge ho zapíše vedľa kľúča (`losos.edge.identity.certFile`). Kým
certifikát nepríde, edge odpovedá na `/identity` kódom 404 a nie je
oficiálny. Tak vyzerá vlastný edge firmy, vrátane toho, ktorý spúšťa
`demo/edge-lan/`. Edge s oboma voľbami nastavenými na `null` neposkytuje
žiadnu identitu.

Čo to chráni: firma, ktorá prevádzkuje vlastný edge, dostane funkčné
nasadenie on-premises, ale nemôže cez neho zúčtovávať obchody. Cudzí, kto
postaví edge, tiež nemôže prinútiť zariadenia obchodovať cez neho, pretože edge
robí oficiálnym iba súkromný kľúč koreňa. Čo to nechráni: protokol trhu nie
je skrytý a súkromný kľúč koreňa je celé tajomstvo. Jeho strata znamená
vydať release, ktorý vymení kotvu dôvery každého zariadenia.

## DNS zóna a vlastné domény

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

Nič sa neprideľuje, kým edge nemá neexpirovaný certifikát identity
([Oficiálne edge](#oficiálne-edge)). Dovtedy zónu tvoria iba SOA, NS a
glue a `/domains/*` odpovedá 503.

### Vlastné domény

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
minúty a vždy, keď o to požiada panel Sieť. Pod Nextcloudu pripája tento
adresár len na čítanie, pri každej požiadavke pridá názvy do
`trusted_domains` a použije vlastnú doménu požiadavky pre `overwritehost`,
ak je jednou z nich. Robí to iba režim kontajnera. LosOS Git si ponecháva
svoj `ROOT_URL` s názvom edge.

Zariadenie za lokálnym edge dostane svoje domény smerované tiež, cez ten lokálny
edge, len čo sa za lokálny edge zaručilo relay passom a pripojilo sa k meshu
tohto edge. Registrar si tieto trasy uchováva sám, v `relay-routes.json`
vedľa svojho registra. [Federácia
edge](/in-depth/edge-federation.md#vlastné-domény-za-lokálnym-edge) obsahuje
podrobnosti a `losos.edge.dns.relayRoutes`.

`tests/edge-dns.nix` (`losos-edge-dns`) spustí edge a klienta a pýta sa
Knotu cez UDP aj TCP. `backend-registrar/tests/domains.rs` prevedie tok
nárokov proti skutočnému registraru a falošnému serveru DNS-over-HTTPS.

## Demo nasadenie na Verceli

`edge-vercel/` spúšťa API registrara, ten istý router a tú istú kontrolu
tokenu, ako Vercel Function na `https://losos-edge.dasmat.us`, čo je
predvolená hodnota `losos.proxy.registrarUrl`. Register je uložený v Neon
Postgres a stavová stránka na `/` ukazuje registrované zariadenia a konfiguráciu
Traefiku, ktorú by pre ne edge zapísal. Je to iba riadiaca vrstva. Tunel
rathole, TLS Traefiku, mesh a trh nemôžu bežať na serverless hostiteľovi,
takže dátová cesta, teda dosiahnutie zariadenia cez jeho verejný názov hostiteľa,
stále potrebuje VPS. Stránka navyše ukazuje id a názvy hostiteľov
komukoľvek, kto má URL. Je to demonštračný hostiteľ, nie edge. Kroky
nasadenia a premenné prostredia sú v
[edge-vercel/README.md](https://github.com/dasmatus/losos/blob/main/edge-vercel/README.md).
