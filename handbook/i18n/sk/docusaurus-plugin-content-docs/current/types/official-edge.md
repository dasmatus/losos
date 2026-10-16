---
title: Zariadenie s edge LosOS
sidebar_position: 2
---

# Zariadenie s edge LosOS

**Edge** je server na internete, ku ktorému si zariadenie udržiava otvorený
tunel. Cez neho zariadenie dostane verejnú adresu bez otvárania portu doma
a pripojí sa k mesh ďalších zariadení. Zariadenie môže obchodovať aj na trhu
edge. Projekt prevádzkuje **edge LosOS** a vyplácať peniaze môže iba trh
tohto edge.

## Čo edge robí pre zariadenie \{#what-the-edge-does-for-a-box}

| Funkcia                | Ako                                                                                                        |
| ---------------------- | ---------------------------------------------------------------------------------------------------------- |
| **Vzdialený prístup**  | `https://<vaše-meno>.<doména edge>` sa dostane k LosOS cloud a LosOS Git cez šifrovaný tunel, ktorý zariadenie otvára smerom von. Admin stránky zariadenie nikdy nezverejní, zostávajú dostupné iba z LAN, aj cez tunel. |
| **Mesh**               | Zariadenie sa pripojí ku Kubernetes klastru, ktorý prevádzkuje edge. Jeho voľný disk sa stane súčasťou spoločného replikovaného priestoru a jeho voľné CPU spúšťa prácu iných ľudí v hodinách, ktoré povolíte, a iba vtedy, keď je zariadenie nečinné. |
| **Trh**                | Dostanete zaplatené za disk a CPU, ktoré zdieľate, a môžete si ich kúpiť od iných. Kupujúci platia v USDC, Bitcoine alebo Monere, alebo kartou cez Stripe, ak edge karty prijíma, a edge si ponechá poplatok 4 %. Edge môže prijímať iba mince bez platieb kartou, takže na prevádzku trhu netreba firemný účet u spracovateľa kariet. Panel Trh je v bočnom paneli pod Mesh. |
| **Nájsť moje zariadenie** | Edge hostí stránku, ktorá z prehliadača nájde zariadenie vo vašej LAN.                                  |

![Panel Trh: čo toto zariadenie kúpilo, úložisko a výpočtový výkon na kúpu od iných zariadení a výplaty cez Stripe.](@site/docs/img/market-open-sk.png)

## Ako zariadenie nájde svoj edge \{#how-a-box-finds-its-edge}

Každých 20 sekúnd zariadenie hľadá edge na dvoch miestach:

![Ako zariadenie nájde edge: každých 20 sekúnd hľadá v LAN a na nastavenej adrese, vyzve každý edge, ktorý odpovie, a štyri kontroly rozhodnú, či je edge oficiálny; bez odpovede zostáva zdieľanie odmietnuté.](@site/docs/img/edge-discovery-sk.svg)

- **V lokálnej sieti**, cez DNS-SD. Edge nastavený tak, aby sa ohlasoval,
  zverejňuje cez mDNS `_losos-edge._tcp` s adresou svojho API. Zariadenie ho
  nájde rovnako, ako nachádza svoj vlastný názov `.local`.
- **Na nastavenej adrese**. To je riadiaca vrstva edge LosOS, pokiaľ ju
  vlastník nezmenil v Nastavenia → Rozšírené, v `losos.proxy.registrarUrl`.
  Zariadenie sa jej pýta bez ohľadu na to, či je zapnuté **Dostupné aj mimo
  domova**.

Edge sa považuje za nájdený, keď odpovie jeho `/health`. Dostupných ich môže
byť viac naraz, napríklad firemný edge v LAN a edge LosOS cez internet. Panel
Mesh ukazuje jeden riadok na každý edge, označený **V tejto sieti** alebo
**Cez internet**, so znakom, ktorý hovorí, či je edge oficiálny. Tento znak
vysvetľuje nasledujúca časť. Ak nič neodpovedá, panel ukáže **Žiadny okrajový
proxy server sa nenašiel** a čo zariadenie skúsilo: „Zariadenie prehľadalo
svoju sieť a oslovilo `<address>`; nič neodpovedalo.“

![Panel Mesh s oficiálnym edge LosOS nájdeným cez internet, označeným fajkou.](@site/docs/img/mesh-official-sk.png)

Kým nič neodpovedá, zariadenie odmieta zapnúť zdieľanie, ktoré závisí od
siete. Prepnutie úložiska do režimu mesh, **Pripojiť sa k sieti mesh**
a **Zdieľať disk tohto zariadenia so sieťou mesh** sú sivé s textom
„Vyžaduje dostupný okrajový proxy server.“ a zariadenie s rovnakým dôvodom
odmietne aj každé použitie nastavení, ktoré by niektoré z nich napriek tomu
zaplo. Nastavenia, ktoré sú už zapnuté, zariadenie nechá tak. Zariadenie,
ktorému edge zmizol, si ponechá konfiguráciu, môže naďalej meniť všetko
ostatné a zdieľanie môže vždy *vypnúť*. Vaše vlastné súbory a aplikácie od
edge nikdy nezávisia. Ani prepínač tunela nie je blokovaný. Tunel sa znova
pripojí, keď edge opäť odpovie.

## Oficiálny alebo nie \{#official-or-not}

Zdieľanie otvorí akýkoľvek edge, ktorý odpovie. Trh smie prevádzkovať iba
edge LosOS, takže ho zariadenie musí vedieť rozlíšiť od akéhokoľvek iného
edge. Nájsť edge a zveriť mu peniaze sú dve rôzne otázky. Každé zariadenie
nesie koreňový verejný kľúč LosOS. Oficiálny edge sa preukazuje pri každom
prehľadávaní. Predloží certifikát podpísaný zodpovedajúcim súkromným kľúčom,
ktorý má iba projekt, a to offline. Okrem toho kľúčom samotného certifikátu
podpíše čerstvú výzvu, ktorú si zariadenie práve vymyslelo. Zariadenie
urobí štyri kontroly a edge je oficiálny iba vtedy, ak prejdú všetky štyri.

Panel Mesh ukazuje výsledok ako znak vedľa názvu každého edge. Oficiálny edge
dostane fajku. Každý iný edge dostane výstrahu s popisom, ktorý vymenúva, čo
tento edge pre zariadenie urobiť nedokáže: nakupovať na trhu ani predávať
voľné úložisko a výpočtový výkon tohto zariadenia. Zariadenie, ktoré podpis
nedokáže overiť, berie edge ako [firemný edge](company-edge). Vyhľadávanie,
vzdialený prístup a zdieľanie fungujú, obchodovanie nie, a panel Trh hlási
„Okrajový proxy server, ktorý toto zariadenie našlo, neprevádzkuje LosOS.“
Nič, čo firma nastaví, nedokáže z jej vlastného edge urobiť oficiálny. Je to
zámer.

![Výstražný znak vedľa edge, ktorý nie je oficiálny, s popisom, ktorý vymenúva, čo tento edge pre zariadenie urobiť nedokáže.](@site/docs/img/mesh-warning-tooltip-sk.png)

:::note[Kým nie je kľúč zverejnený]
Každé zariadenie sa dodáva s prázdnym súborom koreňového kľúča, kým projekt
nezverejní verejný kľúč. Dovtedy nie je žiadny edge oficiálny, každý edge má
výstražný znak a trh je na každom zariadení vypnutý. Toto je bezpečný smer,
ktorým zlyhať.
:::

Vlastník projektu uchováva koreňový kľúč offline, nikdy nie na zariadení ani
na edge. Každý oficiálny edge má vlastný kľúč, ktorý edge nikdy neopustí,
a certifikát, ktorý mu podpísal koreňový kľúč.

Registrácia na edge, ktorá rozhoduje, ktoré zariadenie cez neho smie
tunelovať, je samostatná vec. Edge vedie zoznam povolených zariadení, každé
s tokenom, a zariadenie doň pridáva ten, kto edge prevádzkuje.

## Vlastná doména \{#your-own-domain}

Zariadenie za edge LosOS môže odpovedať aj na doméne, ktorú už jeho vlastník
má, napríklad `cloud.novak-family.eu`. Prácu, ktorá potrebuje internet, robí
edge. Prevádzkuje malý DNS server pre vlastnú zónu, dá v nej zariadeniu
názov, skontroluje záznamy vlastníka a získa certifikát.

Domény prideľuje iba oficiálny edge a iba zariadeniu, ktorého účet Stripe
Stripe úplne overil. Na tento účet trh vypláca peniaze. Viaže zariadenie na
osobu, ktorú Stripe overil, a cudzí človek si bez takého účtu nemôže
požičať certifikáty edge. Kým zariadenie taký účet nemá, časť na paneli Sieť
uvádza, čo chýba, a neponúka nič na vyplnenie.

![Časť Vlastná doména na paneli Sieť: názov zariadenia na edge, jedna funkčná doména, jedna čakajúca na svoj CNAME s dvoma záznamami na zverejnenie a pole na pridanie ďalšej.](@site/docs/img/custom-domain-sk.png)

Keď je účet pripravený, panel Sieť ukáže **Názov tohto zariadenia na
okrajovom serveri**, niečo ako `3f9a1c0e7b2d4a55.boxes.losos.dasmat.us`.
Názov vzniká z identity zariadenia, ale neprezrádza ju. Doménu pridáte takto:

1. Napíšte doménu do poľa **Pridať doménu, ktorú vlastníte** a stlačte
   **Pridať**.
2. U firmy, od ktorej ste doménu kúpili, pridajte dva záznamy, ktoré panel
   ukáže pod ňou. Jeden je **CNAME** z vašej domény na názov zariadenia na
   edge. Druhý je záznam **TXT** na `_losos-challenge.<vaša doména>`
   s uvedenou hodnotou. Táto hodnota dokazuje, že doména je vaša. Vzniká
   z domény, identity zariadenia a jeho účtu Stripe a neprezrádza nič z toho.
3. Počkajte. Edge si záznamy každú pol minútu sám vyhľadá a každý, ktorý
   nájde, označí **✓ nájdené**. Keď sú oba správne, doména sa zmení na
   **Funguje**, edge pre ňu získa certifikát a LosOS cloud odpovedá na
   `https://<vaša doména>/nextcloud`.

Doména, pred ktorou nič nie je, napríklad samotná `novak-family.eu`,
nemôže niesť CNAME. Použite namiesto neho adresy A a AAAA, ktoré panel uvádza.

Zariadenie môže mať najviac päť domén a doména patrí naraz iba jednému
zariadeniu. Keď ju na paneli odstránite, edge ju pri ďalšom prechode prestane
obsluhovať. Funkčnú doménu edge znova kontroluje každú hodinu. Ak záznamy
zmiznú, prestane doménu obsluhovať po troch neúspešných kontrolách za sebou,
takže jeden výpadok DNS stránku neodstaví. Ak účet Stripe prestane byť
pripravený, prestane okamžite. V oboch prípadoch panel uvedie prečo.
LosOS Git stále zostavuje svoje odkazy s názvom zariadenia na edge.

Ak je vaše zariadenie doma za vaším vlastným edge, vaše domény fungujú aj
tak, pokiaľ sa zariadenie pripojilo k mesh. Edge LosOS pošle prevádzku na váš
edge a váš edge ju odovzdá zariadeniu. Vaše zariadenie oznámi edge LosOS, za
ktorým domácim edge je, takže vašu doménu si nemôže privlastniť nikoho iného
edge. Netreba na to nič nastavovať.

## Dôležité nastavenia \{#settings-that-matter}

| Panel      | Nastavenie                             | Čo robí                                                                 |
| ---------- | -------------------------------------- | ----------------------------------------------------------------------- |
| Sieť       | Dostupné aj mimo domova                | otvorí tunel k edge                                                     |
| Sieť       | Vlastná doména                         | pridá doménu, ktorú vlastníte; vyžaduje oficiálny edge a účet Stripe, ktorý Stripe overil |
| Rozšírené  | `losos.proxy.registrarUrl`             | adresa edge, ktorej sa zariadenie pýta cez internet, popri lokálnej sieti |
| Mesh       | Pripojiť sa k sieti mesh               | zaregistruje toto zariadenie v klastri edge; vyžaduje dostupný edge, zapnutý tunel a vlastný názov |
| Mesh       | Požičiavať toto zariadenie, kým spím   | požičiava CPU v hodinách nižšie, keď je nečinné; vyžaduje pripojenie k mesh |
| Mesh       | Hodiny                                 | denné časové okno, v časovom pásme tohto zariadenia                     |
| Trh        | Zdieľať disk tohto zariadenia so sieťou mesh | požičiava voľné miesto do spoločného priestoru; vyžaduje dostupný edge |

## Čo edge vidí \{#what-the-edge-sees}

Edge vidí názov zariadenia, jeho signál o činnosti (heartbeat), ktoré
funkcie má zapnuté, a prevádzku, ktorú tuneluje. Táto prevádzka je šifrovaná
od konca po koniec kľúčom, ktorý si zariadenie zapamätá pri prvom spojení.
Edge nevidí admin stránky, heslo vlastníka ani kľúč disku.
[Bezpečnostný model](../reference/security-model) uvádza hranice dôvery.

:::note[Demo edge]
Na prezentácie beží riadiaca vrstva edge aj na Verceli na adrese
`losos-edge.dasmat.us`, s registrom v malej databáze. Tento hostiteľ dokáže
registrovať zariadenia a ukázať ich na stavovej stránke, ale nedokáže niesť
tunel, prevádzkovať mesh ani prijímať platby. Na to treba skutočný edge
server.
:::
