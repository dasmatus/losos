---
title: Modely AI
sidebar_position: 4.6
mdx:
  format: md
---

# Modely AI

Zariadenia v sieti mesh môžu požičať pamäť jazykovým modelom, ktoré sú na
každé z nich samo príliš veľké. Edge spája požičiavajúce zariadenia do
skupiny, kým sa do nej model nezmestí, spustí model naprieč skupinou a
predáva jeho odpovede po tokenoch. Kupujúci platí priebežne v USDC zo
zostatku, ktorý drží [escrow](/in-depth/market.md#paying-with-usdc); každý
účet sa delí medzi zariadenia, ktoré prácu urobili, a treasury LosOS.

Modely spúšťa [llama.cpp](https://github.com/ggml-org/llama.cpp). Každé
požičiavajúce zariadenie spúšťa jeho RPC server, ktorý drží časť vrstiev
modelu a počíta na nich. Edge spúšťa jeden server llama.cpp na skupinu. Ten
sa ku každej časti dostane cez tunel daného zariadenia a odpovedá
kupujúcemu.

## Požičiavanie pamäte {#lending-memory}

Požičiavanie má vlastný prepínač `losos.inference.lend.enable`. Koľko pamäte
zariadenie požičia, určuje `losos.inference.lend.memoryMiB`, predvolene
4096 MiB, a s koľkými vláknami procesora počíta, určuje
`losos.inference.lend.threads`. Bez počtu vlákien použije polovicu vlákien
zariadenia. Na paneli Trh je to prepínač **Požičiavať pamäť modelom AI** v
časti **Voľná pamäť**, s množstvom v GiB.

- RPC server beží ako používateľ `shared`, ten istý ako úložisko mesh, a
  súbory vlastníka nikdy nevidí.
- Počúva len na loopbacku. Pravidlo firewallu pustí k jeho portu len roota,
  ktorý prevádzkuje tunel; každého iného používateľa na zariadení odmietne.
- Nevezme si viac pamäte, než vlastník nastavil. Jednotka má pevný limit
  pamäte, a keď server tento limit prekročí, systemd zastaví server, nikdy
  nie LosOS cloud.
- Beží s nízkou váhou procesora a disku, takže vlastná práca vlastníka má
  prednosť.
- Edge sa k nemu dostane cez druhú službu na existujúcom tuneli zariadenia.
  Zariadenie, ktoré nemá tunel k edge, požičiavať nemôže.

Výpočtové okno sa tu neuplatňuje: zariadenie požičiava, kým je prepínač
zapnutý.

Aby ho edge mohol vybrať, potrebuje zariadenie aj výplatnú adresu na sieti,
na ktorej kupujúci platí, teda EVM alebo Solana, a jeho edge mu musí
dovoliť predávať. Zariadenie dosiahnuteľné cez niečí vlastný edge v LAN
zatiaľ požičiavať nemôže.

## Skupiny {#groups}

Pre každý model a platobnú sieť drží edge najviac jednu skupinu. Vyberá
požičiavajúce zariadenia od tých s najväčšou pamäťou, najviac 16, kým ich
pamäť spolu nepokryje to, čo model potrebuje. Zariadenie je naraz len v
jednej skupine. Všetci kupujúci toho istého modelu na tej istej sieti
používajú tú istú skupinu.

| Stav        | Význam                                                                            |
| ----------- | --------------------------------------------------------------------------------- |
| Načítava sa | Server edge štartuje a posiela model zariadeniam.                                 |
| Pripravená  | Model odpovedá.                                                                   |
| Nedostupná  | Niektoré zariadenie vypadlo. Skupina sa reštartuje, keď sú všetky zariadenia späť. |

Skupinu, ktorá sa reštartuje viac ako päťkrát alebo sa načítava dlhšie ako
30 minút, edge vzdá a jej zariadenia hodinu nezaradí do nových skupín.
Zariadenie, ktoré chýba viac ako 15 minút, skupinu ukončí. Skupinu, ktorú
nikto hodinu nepoužil, edge rozoberie.

Prvé načítanie pošle vrstvy modelu každému zariadeniu. RPC server si ich
nechá v medzipamäti, takže ďalšie načítanie toho istého modelu je rýchlejšie.

## Priebežná platba {#paying-as-you-go}

Kupujúci si raz doplní zostatok USDC v escrow na jednej sieti a potom môže
na tej sieti používať ktorýkoľvek model bez ďalšej platby.

1. Kupujúci vyberie model a najvyššiu sumu, ktorú za hodinu minie, od 0,01 $
   do 100 $.
2. Edge odpovie cenovou ponukou: jeden **tap** na každé zariadenie v
   skupine. Tap uvádza jedného predajcu a najvyššiu sumu, ktorú cez neho
   edge smie za hodinu účtovať, teda hodinový limit kupujúceho krát podiel
   tohto zariadenia na pamäti. Cenová ponuka platí 30 minút.
3. Peňaženka kupujúceho otvorí všetky tapy jednou transakciou.
4. Edge si tapy prečíta späť z reťazca, overí, že každý uvádza predajcu,
   sadzbu a poplatok z cenovej ponuky, a začne odpovedať.
5. Edge účtuje každú odpoveď podľa jej tokenov, za cenu modelu za milión
   tokenov promptu a za milión vygenerovaných tokenov, a účet rozdelí medzi
   tapy podľa podielu každého zariadenia.

Tokeny počíta edge sám. Prompt zostaví a spočíta skôr, než ho pošle ďalej,
a odpoveď obmedzí na najvyšší počet tokenov, ktorý požiadavka uviedla, alebo
na 256, ak žiadny neuvádza. Zariadenia len počítajú vrstvy a nič nehlásia,
takže žiadne zariadenie nemôže účet zvýšiť. Odpoveď, ktorá zlyhá, sa
neúčtuje. Jeden merač odpovedá naraz len na jednu požiadavku.

Pred každou odpoveďou edge overí, že zostatok pokryje najvyššiu sumu, ktorú
tá odpoveď môže stáť, a že každý tap má pod svojou hodinovou sadzbou ešte
rezervu. Nový tap začína s rezervou na hodinu, takže kupujúci dostane
odpoveď hneď.

### Vyrovnanie {#settling}

Edge každý tap z času na čas vyrovná na reťazci:

| Sieť    | Kedy                                       |
| ------- | ------------------------------------------ |
| Solana  | Každú hodinu.                              |
| EVM     | Raz denne, alebo hneď, ako tap dlhuje 1 $. |

Uzavretý tap, merač, ktorý skončí, a zostatok, ktorý chce kupujúci vybrať,
sa vyrovnajú hneď. Každé vyrovnanie presunie sumu zo zostatku kupujúceho,
pošle poplatok 4 % do treasury a zvyšok na výplatnú adresu zariadenia.

### Čo escrow vynucuje {#what-the-escrow-enforces}

Escrow nezverí edge viac, než kupujúci dovolil:

- Vyrovnanie môže zaplatiť len predajcovi, ktorého tap uvádza.
- Môže účtovať najviac sadzbu tapu za čas od posledného vyrovnania, k tomu
  hodinu, s ktorou nový tap začína, a nikdy viac ako sedem dní naraz.
- Môže účtovať len toľko, koľko zostatok obsahuje.
- Po uzavretí tapu môže účtovať len čas do uzavretia.

Ukradnutý kľúč edge teda kupujúceho stojí najviac jeho vlastný hodinový
limit za čas, kým svoje tapy neuzavrie.

### Uzavretie a výber {#closing-and-withdrawing}

Kupujúci môže merač zastaviť na paneli Trh alebo uzavrieť ktorýkoľvek tap zo
svojej peňaženky; tapy merača, ktorý skončí, uzavrie edge. Keď chce kupujúci
peniaze vybrať, požiada o výber, počká 48 hodín a potom vyberie. Počas
týchto 48 hodín môže edge ešte vyrovnať, čo sa minulo. Výber edge nikdy
nepotrebuje, takže zostatok zostane dostupný, aj keby edge zmizol.

Panel Trh upozorní, keď zostatok pri aktuálnom hodinovom limite pokryje
menej ako 24 hodín.

## Čo to nedokáže {#what-it-cannot-do}

- **Zariadenia v skupine prácu vidia.** Každé zariadenie počíta s promptom,
  keď prechádza vrstvami modelu. Neposielajte nič, čo nesmie vidieť
  zariadenie cudzieho človeka.
- **Cez domáce pripojenia je to pomalé.** Každý token prechádza každým
  zariadením v skupine, takže model rozdelený medzi zariadenia na domácich
  pripojeniach odpovedá oveľa pomalšie než ten istý model na jednom stroji.
- **Veľké modely sa načítavajú pomaly.** Prvé načítanie pošle každému
  zariadeniu gigabajty.
- **Skupina má najviac 16 zariadení**, takže najväčší model, ktorý edge môže
  ponúknuť, je obmedzený tým, čo požičia 16 zariadení.
- **Karty sa pri modeloch neponúkajú.** Úložisko, výpočtový výkon a stroje sa
  naďalej predávajú po zaplatených obdobiach, ako opisuje
  [trh](/in-depth/market.md).
