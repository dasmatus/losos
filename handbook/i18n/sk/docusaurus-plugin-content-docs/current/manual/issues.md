---
title: Úlohy v LosOS Git
sidebar_position: 4.55
slug: /manual/issues
---

# Úlohy v LosOS Git

LosOS Git drží úlohy zariadenia: chyby, práce a nápady, pre kód aj pre
čokoľvek iné. Nájdete ich pod **Úlohy** v bočnom paneli LosOS Git, na adrese
`https://<box>.local/git/issues`, v jednom zozname pre všetky projekty na
zariadení.

## Projekty a kľúče \{#projects-and-keys}

Každá úloha patrí do projektu a každý projekt má krátky kľúč. Úloha sa volá
kľúčom a číslom: `BT-12` je úloha 12 projektu s kľúčom `BT`. Toto meno
zostáva úlohe celý život, takže ho môžete pokojne napísať do commitu, chatu
alebo e-mailu.

Repozitár dostane vlastný projekt, keď niekto prvýkrát otvorí jeho
**Úlohy**. Kľúč vznikne z mena repozitára a čísla úloh projektu sú čísla
repozitára: `BT-12` a `backup-tool#12` sú tá istá úloha. Projekt môže stáť
aj sám, bez repozitára, pre prácu, ktorá nie je kód. Vytvorí ho **Nový
projekt** v bočnom paneli. Projekt je buď súkromný, otvorený len ľuďom,
ktorých doň pridáte, a tým, ktorí vidia jeho repozitár, alebo otvorený
každému s účtom na zariadení, ktorý potom môže jeho úlohy aj meniť.

## Stav, priorita a ostatné \{#status-priority-and-the-rest}

Úloha prechádza pracovným postupom projektu. Nový projekt má šesť stavov:
Backlog, Todo, In Progress, In Review, Done a Canceled. Každý stav je jedného
z piatich druhov: zásobník, na rade, rozpracované, hotové a zrušené.
Zoznamy, nástenka aj uzatváracie slová nižšie sa riadia druhom, takže
projekt môže stavy premenovať alebo pridať vlastné.

Okrem stavu má úloha:

- prioritu: naliehavá, vysoká, stredná, nízka alebo žiadna;
- jedného riešiteľa;
- štítky, ktoré vzniknú na mieste, keď napíšete nové meno;
- odhad v bodoch (1, 2, 3, 5, 8, 13 alebo 21);
- termín;
- cyklus (pozri nižšie);
- nadradenú úlohu, čím sa z nej stane podúloha. Nadradená úloha ukazuje,
  koľko jej podúloh je hotových.

Dve úlohy môžu súvisieť: jedna blokuje druhú, jedna je duplikátom druhej,
čím sa duplikát zruší, alebo jednoducho súvisia.

Každá úloha si pamätá svoju históriu: kto ju vytvoril, každú zmenu stavu,
priority, riešiteľa, štítku či cyklu, každý komentár a commity a pull
requesty, ktoré ju spomenuli.

## Zoznamy, nástenka a pohľady \{#lists-the-board-and-views}

**Moje úlohy** ukazuje, čo riešite vy, naprieč všetkými projektmi.
**Všetky úlohy** ukazuje všetko, čo smiete vidieť. Projekt má svoje úlohy,
svoj **Zásobník** s úlohami, ktoré nikto nezačal, a svoje **Cykly**.

Každý zoznam môžete zobraziť ako zoznam zoskupený podľa stavu alebo ako
nástenku so stĺpcom pre každý stav. Na nástenke kartu potiahnete do iného
stĺpca, aby ste zmenili jej stav, alebo hore a dole, aby ste zmenili
poradie. **Filter** zúži zoznam podľa stavu, priority, riešiteľa, štítku,
cyklu a ďalšieho a **Zobrazenie** vyberie, ako sa zoskupuje a zoraďuje.
**Uložiť pohľad** si filter a rozloženie zapamätá pod menom v bočnom paneli;
pohľad si môžete nechať pre seba alebo ho zdieľať s každým, kto vidí
projekt.

Keď označíte úlohy zaškrtávacími políčkami alebo klávesom `X`, otvorí sa
lišta, ktorá zmení stav, prioritu, riešiteľa alebo štítky všetkým naraz.

## Cykly \{#cycles}

Cyklus je úsek času, zvyčajne týždeň alebo dva, s úlohami, ktoré doň
patria. **Cykly** projektu ich vypíšu aj s tým, koľko ich úloh je hotových,
a **Tento cyklus** ukáže úlohy cyklu, ktorý beží dnes.

## Doručené \{#the-inbox}

**Doručené** vypíše, čo sa stalo s úlohami, ktoré sledujete: niekto vám
úlohu pridelil, spomenul vás cez `@`, napísal komentár, zmenil stav alebo
ju spomenul commit či pull request. Sledujete úlohu, ktorú ste vytvorili,
ktorú riešite, kde ste komentovali alebo vás niekto spomenul, a každú, pri
ktorej zapnete zvonček. Číslo pri **Doručené** počíta, čo ste ešte nečítali.

## Klávesnica \{#the-keyboard}

Všetko ide aj bez myši. Na stránkach úloh stlačte `?` a uvidíte zoznam.
Najčastejšie:

| Kláves | Urobí |
| --- | --- |
| `C` | novú úlohu |
| `J`, `K` | posun v zozname nadol a nahor |
| `Enter` | otvorí úlohu |
| `X` | označí ju |
| `S`, `P`, `A`, `L` | zmení jej stav, prioritu, riešiteľa alebo štítky |
| `0` až `4` | nastaví jej prioritu |
| `B` | prepne medzi zoznamom a nástenkou |
| `/` | hľadá v zozname |
| `Ctrl+K` | nájde ktorúkoľvek úlohu, projekt alebo pohľad podľa mena |

## Commity a pull requesty \{#commits-and-pull-requests}

Meno úlohy v správe commitu alebo v pull requeste ich prepojí: úloha ukáže
commit alebo pull request a Doručené to oznámi tým, ktorí úlohu sledujú.
Fungujú `BT-12`, `#12` aj `owner/repo#12`.

Uzatváracie slovo pred menom úlohu aj dokončí: `close`, `closes`, `closed`,
`fix`, `fixes`, `fixed`, `fixing`, `resolve`, `resolves` alebo `resolved`.
Commit s `Fixes BT-12` presunie BT-12 do Done, keď sa dostane do hlavnej
vetvy repozitára. Pull request, ktorý úlohu spomenie, ju presunie do In
Review, ak ju nikto nezačal, a keď sa pull request zlúči, úlohy, ktoré
podľa neho uzatvára, prejdú do Done. Úlohu spomenie aj vetva pomenovaná po
nej, napríklad `bt-12-free-space`.

Takto môže úlohu uzavrieť len ten, kto ju smie meniť.

## S ostatnými aplikáciami \{#with-the-other-apps}

Z ponuky **⋯** navrchu úlohy:

- **Poslať e-mailom** otvorí LosOS Mail s menom a odkazom úlohy;
- **Zdieľať v chate** ju otvorí v LosOS Chat;
- **Pridať termín do kalendára** dá termín ako súbor kalendára;
- **Kontakt riešiteľa** otvorí riešiteľa v Kontaktoch.

Mail, Chat a Kontakty sú tam, keď zariadenie zdieľa a je v sieti mesh.
Každá vaša úloha s termínom je aj v odbere kalendára na
`https://<box>.local/git/api/v1/tracker/calendar.ics`. Kalendárová
aplikácia sa naň prihlási vaším menom a prístupovým tokenom LosOS Git s
rozsahom pre úlohy ako heslom.

Keď je zapnuté Build with AI, **AI boxu** pri úlohe ju vie **Rozdeliť na
kroky**, z ktorých zaškrtnutím urobíte podúlohy, alebo **Zhrnúť**, čo sa
povedalo. Model beží na zariadení a nič sa nezmení, kým nevyberiete, čo si
nechať.

## Kto smie čo \{#who-may-do-what}

Kto vidí repozitár projektu, vidí aj jeho úlohy, môže ich komentovať a
otvárať nové; nová úloha od niekoho, kto repozitár smie len čítať, si
nechá názov a popis a nič iné. Na zmenu úlohy treba právo zápisu do
repozitára alebo projektu. Pracovný postup, štítky, cykly a členov projektu
menia jeho správcovia.

## Zariadenie, ktoré malo Forgejo alebo nástenky LosOS Cloud \{#a-box-that-had-forgejo-or-losos-cloud-boards}

Úlohy prenesené z Forgeja sa stanú úlohami projektu ich repozitára, s
rovnakými číslami, komentármi a štítkami.

Nástenky LosOS Cloud sa prevezmú raz, pri prvom štarte s úlohami: každá
nástenka sa stane projektom so stĺpcami ako pracovným postupom, jej karty
úlohami so štítkami, riešiteľmi, termínmi a komentármi a jej členovia
členmi projektu. Nástenka, ktorá patrila repozitáru, naplní projekt toho
repozitára, a karta, ktorá ukazovala na jeho úlohu, presunie tú úlohu
namiesto toho, aby vytvorila druhú. Kópia v LosOS Cloud zostane, ako bola.
Čo sa prenieslo, je vypísané v `/var/lib/losos-git/boards/import.json`.
