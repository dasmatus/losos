---
title: Mesh
sidebar_position: 3
mdx:
  format: md
---

# Mesh

![Panel Mesh na zariadení bez dostupného okrajového proxy servera.](@site/docs/img/mesh-none-sk.png)

Zariadenie môže požičať voľný disk a CPU iným zariadeniam losos. Pripojenie je predvolene
vypnuté. Panel Mesh má dva prepínače: pripojiť sa k sieti mesh a zdieľať výpočtový
výkon. Tretí prepínač, zdieľanie disku tohto zariadenia, je na paneli
[Trh](/in-depth/market.md), pretože požičať disk a dostať zaň zaplatené je jedno
rozhodnutie. Prepínač funguje bez ohľadu na to, či edge tomuto zariadeniu trh
ponúka.

## Dva klastre {#two-clusters}

Každé zariadenie beží s dvoma inštanciami Kubernetes:

|                     | Lokálny (k3s)               | Mesh (agent rke2)                |
| ------------------- | --------------------------- | -------------------------------- |
| Beží na ňom         | aplikácie inštalované ako shared | úložisko Longhorn, zdieľaný výpočtový výkon |
| Server              | toto zariadenie             | edge VPS                         |
| Potrebuje sieť      | nie                         | áno                              |

:::warning[Držte dva klastre oddelene]
Agent Kubernetes nemôže naštartovať, kým je jeho server nedostupný, a zariadenie sa
každú noc reštartuje. Keby vaše vlastné služby bežali v mesh klastri,
výpadok edge cez polnoc by ich odstavil. Držte ich oddelene.
:::

## Zdieľanie výpočtového výkonu {#sharing-compute}

Zariadenie prijíma prácu z mesh iba vtedy, keď platí oboje:

1. Aktuálny čas je vnútri okna, ktoré ste nastavili
   (`losos.cluster.computeWindow`).
2. Zariadenie je nečinné (`losos.cluster.idleLoadThreshold`).

Nečinnosť môže okno zavrieť skôr. Mimo okna ho otvoriť nemôže.

:::note[Neznáme sa počíta ako zaneprázdnené]
Všetko neznáme sa počíta ako zaneprázdnené: chýbajúce hlásenie o nečinnosti,
zastarané hlásenie alebo edge, ktorý sa práve reštartoval. Edge vyhodnocuje
okno v časovom pásme zariadenia.
:::

## Hľadanie edge {#finding-the-edge}

Mesh sú iné zariadenia za edge. Zariadenie bez edge v dosahu nemôže zdieľať nič, nech
jeho prepínače hovoria čokoľvek, preto `lososd` každých 20 sekúnd nejaký
hľadá, na dvoch miestach:

- **v lokálnej sieti**, cez DNS-SD. Edge nastavený s
  `losos.edge.lan.advertise = true` publikuje `_losos-edge._tcp` cez mDNS so
  záznamom `url=`, ktorý uvádza jeho API registrátora, a zariadenie ho nájde
  cez systemd-resolved.
- **na nastavenej adrese**, `losos.proxy.registrarUrl`, čo je na zariadení s
  internetom verejný edge. Zariadenie ho skúša bez ohľadu na to, či je master proxy
  zapnutá.

Kandidát sa počíta, až keď odpovie jeho `/health`. Naraz ich môže byť v
dosahu viac, napríklad firemný edge v LAN a verejný cez internet.
`GET /api/edge` ich vypíše všetky, LAN ako prvé, a panel Mesh ukazuje jeden
riadok na každý edge, alebo "Žiadny okrajový proxy server sa nenašiel" spolu
s tým, čo skúšal. Zdieľanie je povolené, kým odpovedá ktorýkoľvek
z nich. Zväčšenie vlastného disku zariadenia nepotrebuje edge vôbec. Spájanie
naprieč zariadeniami áno, pretože riadiaca vrstva mesh beží na edge. V LAN bez
internetu je týmto edge jeden stále zapnutý počítač s modulom edge, pozri
[Master proxy, edge v tej istej
LAN](/in-depth/master-proxy.md#edge-on-the-same-lan).

Kým nič neodpovedá, zariadenie odmietne zapnúť zdieľanie závislé od siete.
Prepnutie do režimu mesh a akékoľvek použitie nastavení (apply), ktoré
zapína `losos.sharingMyStorage` alebo `losos.cluster.enable`, dostane
odpoveď 409 s dôvodom a prepínače sú sivé s tou istou vetou. Vypnúť
zdieľanie sa dá vždy a počas výpadku môže zariadenie zmeniť čokoľvek iné.
Vaše vlastné súbory a aplikácie od edge nikdy nezávisia.

Zariadenie, ktoré zdieľa úložisko a potom desať minút nenájde žiadny edge,
samo vypne zdieľanie, rovnako ako by to urobil jeho prepínač, a panel Mesh
ukáže **Zdieľanie pozastavené**. Keď edge minútu odpovedá, zariadenie
zdieľanie znova zapne. Vráti iba vlastné pozastavenie: ak medzitým
zdieľanie zapnete alebo vypnete, platí vaša voľba. `losos.proxy.pauseSharingAfter`
nastavuje čakanie v sekundách a `0` pozastavenie vypína.

Nájsť edge a zveriť mu peniaze sú dve rôzne otázky. Zdieľanie otvorí
ktorýkoľvek edge, ktorý odpovedá. Obchodovať na markete sa dá iba cez
**oficiálny** edge, bránu, ktorá prejde overením zariadenia: pas podpísaný
jej vlastníkom, TPM, ktorý ukazuje, že nezmenený naštartoval zverejnený
obraz brány LosOS, a adresy poplatkov LosOS. Zariadenie to kontroluje pri
každom skenovaní. Riadok každého edge nesie vedľa mena značku, fajku pre
overenú bránu a varovanie pre akýkoľvek iný edge. Tooltip varovania vypíše, čo tento edge
pre toto zariadenie urobiť nemôže: nakupovať na markete a predávať voľné
úložisko a výpočtový výkon tohto zariadenia. Vlastný edge firmy nesie varovanie a
nie je to chyba. Pozri [Master proxy, oficiálne
edge](/in-depth/master-proxy.md#official-edges).

:::note[Vyskúšajte to na virtuálnych strojoch]
`nix/demo/edge-lan/run.sh` nabootuje dve zariadenia a jeden edge v jednej virtuálnej
sieti a prejde presne týmto v poradí overovacieho kontrolného zoznamu
(`nix/demo/edge-lan/CHECKLIST.md`). Najprv bez edge kdekoľvek: nič sa nenašlo,
zdieľanie odmietnuté, lokálne používanie nedotknuté, reštart nič nezmení.
Potom so zapnutým edge: nájdený na oboch zariadeniach v rámci jedného skenovania,
povolené, edge preč, odmietnuté, späť. `nix/demo/edge-lan/record.sh` nahrá ten
istý priebeh z admin UI zariadení.
:::

## Zdieľanie úložiska {#sharing-storage}

Prispené úložisko žije v domovskom adresári používateľa `shared` pod
politikou fscrypt. Kľúč je zapečatený v TPM. Keď je zdieľanie vypnuté, kľúč
nie je načítaný a adresár sa nedá čítať, ani pre roota na bežiacom zariadení. LUKS
chráni zariadenie, keď je vypnuté; fscrypt chráni tento adresár, keď je zapnutý.

Prispené súbory sú oddelené menným priestorom podľa stroja.

Edge spúšťa v mesh klastri Longhorn, nainštalovaný zo zafixovaného chartu
(`losos.edge.cluster.longhornChart`). Jeho disky majú len zariadenia:
zariadenie, ktoré sa pripojí do mesh, dostane štítok
`node.longhorn.io/create-default-disk=true`, a edge, ktorý ho nemá, žiadne
repliky neukladá. Objednávky úložiska z trhu si berú zväzky z triedy
úložiska `longhorn`. Kľúč fscrypt chráni disk požičiavateľa, no je na jeho
zariadení, takže požičiavateľ dokáže čítať, čo pristane v jeho podiele.
Zálohy kupujúceho na takom zväzku sa preto najprv zašifrujú na zariadení
kupujúceho, nástrojom restic a kódom obnovy kupujúceho
([Trh](/in-depth/market.md)). Longhorn na edge nekontroluje aktualizácie, neposiela
štatistiky používania a jeho webové rozhranie sa nenasadzuje.

## Predaj {#selling}

Úložisko aj výpočtový výkon sa dajú predávať aj iným zariadeniam cez voliteľný
trh, v bočnom paneli pod Mesh, Trh. Kupujúci platia v USDC, bitcoinoch alebo
Monere, alebo kartou, ak edge karty prijíma. Zariadenie ukáže trh, len keď je
v dosahu oficiálny edge LosOS. Pozri [Trh](/in-depth/market.md).

## Perzistencia {#persistence}

Agent mesh pri prvom pripojení zapíše `/etc/rancher/node/password`. Táto cesta
musí zostať v zozname perzistencie, inak server zariadenie po ďalšom reštarte
odmietne.
