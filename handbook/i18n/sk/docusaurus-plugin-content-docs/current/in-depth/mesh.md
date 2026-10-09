---
title: Mesh
sidebar_position: 3
mdx:
  format: md
---

# Mesh

Zariadenie môže požičať voľný disk a CPU iným zariadeniam losos. Pripojenie je predvolene
vypnuté. Panel Mesh má dva prepínače: pripojiť sa k sieti mesh a zdieľať výpočtový
výkon. Tretí prepínač, zdieľanie disku tohto zariadenia, je na paneli
[Trh](/in-depth/market.md), pretože požičať disk a dostať zaň zaplatené je jedno
rozhodnutie. Kým je market iba plánovaný, je tento prepínač nedostupný spolu
s ním.

## Dva klastre

Každé zariadenie beží s dvoma inštanciami Kubernetes:

|                     | Lokálny (k3s)               | Mesh (agent rke2)                |
| ------------------- | --------------------------- | -------------------------------- |
| Beží na ňom         | Nextcloud a Forgejo tohto zariadenia | úložisko Longhorn, zdieľaný výpočtový výkon |
| Server              | toto zariadenie             | edge VPS                         |
| Potrebuje sieť      | nie                         | áno                              |

Agent Kubernetes nemôže naštartovať, kým je jeho server nedostupný, a zariadenie sa
každú noc reštartuje. Keby vaše vlastné služby bežali v mesh klastri,
výpadok edge cez polnoc by ich odstavil. Držte ich oddelene.

## Zdieľanie výpočtového výkonu

Zariadenie prijíma prácu z mesh iba vtedy, keď platí oboje:

1. Aktuálny čas je vnútri okna, ktoré ste nastavili
   (`losos.cluster.computeWindow`).
2. Zariadenie je nečinné (`losos.cluster.idleLoadThreshold`).

Nečinnosť môže okno zavrieť skôr. Mimo okna ho otvoriť nemôže.

Všetko neznáme sa počíta ako zaneprázdnené: chýbajúce hlásenie o nečinnosti,
zastarané hlásenie alebo edge, ktorý sa práve reštartoval. Edge vyhodnocuje
okno v časovom pásme zariadenia.

## Hľadanie edge

Mesh sú iné zariadenia za edge. Zariadenie bez edge v dosahu nemôže zdieľať nič, nech
jeho prepínače hovoria čokoľvek, preto `lososd` každých 20 sekúnd nejaký
hľadá, na dvoch miestach:

- **v lokálnej sieti**, cez DNS-SD. Edge nastavený s
  `losos.edge.lan.advertise = true` publikuje `_losos-edge._tcp` cez mDNS so
  záznamom `url=`, ktorý uvádza jeho API registrátora, a Avahi zariadenia ho nájde.
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
LAN](/in-depth/master-proxy.md#edge-v-tej-istej-lan).

Kým nič neodpovedá, zariadenie odmietne zapnúť zdieľanie závislé od siete.
Prepnutie do režimu mesh a akékoľvek použitie nastavení (apply), ktoré
zapína `losos.sharingMyStorage` alebo `losos.cluster.enable`, dostane
odpoveď 409 s dôvodom a prepínače sú sivé s tou istou vetou. Nastavenia,
ktoré sú už zapnuté, zostanú nedotknuté, takže zariadenie, ktorému edge zmizol, si
ponechá svoju konfiguráciu a stále môže zmeniť čokoľvek iné. Vypnúť
zdieľanie sa dá vždy. Vaše vlastné súbory a aplikácie od edge nikdy
nezávisia.

Nájsť edge a zveriť mu peniaze sú dve rôzne otázky. Zdieľanie otvorí
ktorýkoľvek edge, ktorý odpovedá. Obchodovať na markete sa dá iba cez
**oficiálny** edge, ktorý predloží certifikát podpísaný koreňovým kľúčom
LosOS a odpovie ním na čerstvý nonce zariadenia. Zariadenie to kontroluje pri každom
skenovaní. Riadok každého edge nesie vedľa mena značku, fajku pre oficiálny
edge a varovanie pre akýkoľvek iný. Tooltip varovania vypíše, čo tento edge
pre toto zariadenie urobiť nemôže: nakupovať na markete a predávať voľné
úložisko a výpočtový výkon tohto zariadenia. Vlastný edge firmy nesie varovanie a
nie je to chyba. Pozri [Master proxy, oficiálne
edge](/in-depth/master-proxy.md#oficiálne-edge).

`demo/edge-lan/run.sh` nabootuje dve zariadenia a jeden edge v jednej virtuálnej
sieti a prejde presne týmto v poradí overovacieho kontrolného zoznamu
(`demo/edge-lan/CHECKLIST.md`). Najprv bez edge kdekoľvek: nič sa nenašlo,
zdieľanie odmietnuté, lokálne používanie nedotknuté, reštart nič nezmení.
Potom so zapnutým edge: nájdený na oboch zariadeniach v rámci jedného skenovania,
povolené, edge preč, odmietnuté, späť. `demo/edge-lan/record.sh` nahrá ten
istý priebeh z admin UI zariadení.

## Zdieľanie úložiska

Prispené úložisko žije v domovskom adresári používateľa `shared` pod
politikou fscrypt. Kľúč je zapečatený v TPM. Keď je zdieľanie vypnuté, kľúč
nie je načítaný a adresár sa nedá čítať, ani pre roota na bežiacom zariadení. LUKS
chráni zariadenie, keď je vypnuté; fscrypt chráni tento adresár, keď je zapnutý.

Prispené súbory sú oddelené menným priestorom podľa stroja.

## Predaj

Úložisko aj výpočtový výkon bude možné predávať aj iným zariadeniam cez voliteľný
market Stripe Connect. Je hotový, ale ešte nie je otvorený, a admin UI
ukazuje jeho kartu sivú ako "soon(TM)". Pozri [Market](/in-depth/market.md).

## Perzistencia

Agent mesh pri prvom pripojení zapíše `/etc/rancher/node/password`. Táto cesta
musí zostať v zozname perzistencie, inak server zariadenie po ďalšom reštarte
odmietne.
