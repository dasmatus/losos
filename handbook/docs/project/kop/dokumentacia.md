---
title: Dokumentácia projektu
sidebar_position: 9
---

# Technická dokumentácia projektu

Bod 4 zadania: *vypracujte technickú dokumentáciu k projektu.* Tento
dokument je jej odovzdávaná podoba; táto kapitola opisuje, z čoho
dokumentácia projektu pozostáva, pre koho je ktorá časť, ako sa zostavuje
a publikuje, a ako sa udržiava v súlade s kódom.

## Vrstvy dokumentácie

| Vrstva                                  | Pre koho                     | Kde                                                                 |
| --------------------------------------- | ---------------------------- | ------------------------------------------------------------------- |
| **Príručka majiteľa** (tento web)       | človek s boxom na stole      | `handbook/docs/`, publikovaná na losos.dasmat.us a na každom boxe pod `/handbook/` |
| **Technická dokumentácia (KOP)**        | komisia, konzultant          | `handbook/docs/project/kop/`, táto kapitola príručky a PDF         |
| **Vývojárska wiki**                     | kto systém stavia a mení     | `wiki/` v repozitári, publikovaná do GitHub wiki                    |
| **CLAUDE.md a hlavičky súborov**        | vývojár (a AI asistent)      | koreň repozitára; prvé desiatky riadkov takmer každého `.nix` a `.rs` súboru |
| **Bezpečnostný model a baseline**       | recenzent                    | `docs/security-model.md`, `docs/baseline.md`                        |
| **Drôtový kontrakt API**                | klienti démona               | `backend/schema.json`                                               |
| **Testy**                               | každý, kto mení rozhodnutie  | `tests/`, s komentárom, prečo test existuje                         |

Rozdelenie podľa čitateľa je zámerné. Príručka nikdy nepredpokladá
terminál a každá stránka o problémoch začína krokmi, ktoré potrebujú len
lokálnu sieť. Wiki predpokladá vývojára s Nixom a Rustom. Hlavičky súborov
zaznamenávajú *prečo* je kód taký, aký je, spravidla aj chybu, ktorá bola
raz zaplatená; recenzent, ktorý sa pýta „prečo“, nájde odpoveď najprv tam.

## Príručka majiteľa

Príručka je web postavený v Docusaurus so šiestimi časťami: Začíname, Typy
nasadenia, Príručka vlastníka, Keď niečo nefunguje (podľa príznaku),
Referencia a Projekt. Má offline vyhľadávanie (index je súčasťou zväzku),
svetlú a tmavú tému zhodnú s administračnými stránkami (paleta je bajt po
bajte tá istá a kontrola `check:tokens` zlyhá, keď sa rozíde) a rozhranie
v angličtine, slovenčine a nemčine; stránky sú zatiaľ anglické a prekladajú
sa po jednej.

Zostavuje sa dvakrát z jedného zdroja: verejná stránka na GitHub Pages
pri každom push do `main` (`.github/workflows/handbook.yml`) a kópia na
box (`losos-handbook` vo `flake/packages.nix`, koreň `/handbook/`), ktorú
nginx podáva len z LAN pod vlastnou politikou CSP. Chybové hlásenia
administračných stránok odkazujú na konkrétnu stránku príručky; kontrola
`check:links` zlyhá, keď hlásenie odkazuje na stránku, ktorá neexistuje.
Rozbitý odkaz je v návode chyba, nie varovanie, preto `onBrokenLinks:
'throw'`.

## Tento dokument

Zdrojom sú stránky Markdown v `handbook/docs/project/kop/` s obrázkami
vedľa nich. Z nich sa zostavuje:

- **web**: ako kapitola príručky (toto, čo čítate), s navigáciou,
  vyhľadávaním a oboma témami;
- **PDF na odovzdanie**: skript `handbook/scripts/build-kop.sh` zreťazí
  kapitoly v poradí, pandoc ich prevedie do Typstu s vlastnou šablónou
  (titulná strana, obsah, číslované kapitoly, popisy obrázkov) a Typst
  vysádza PDF. Beží v CI pri každej zmene príručky a výsledok sa
  publikuje na `losos.dasmat.us/kop/LosOS-KOP.pdf`; ten istý skript beží
  aj lokálne (`nix shell --inputs-from . nixpkgs#pandoc nixpkgs#typst`).

Jeden zdroj pre web aj PDF znamená, že dokument sa nemôže rozísť sám so
sebou, a Markdown v repozitári znamená, že každá zmena dokumentu prechádza
tým istým pull requestom a recenziou ako kód.

## Ako sa dokumentácia drží v súlade s kódom

- **Tvrdenia s testom.** Čo dokumentácia tvrdí o správaní (poradie krokov
  pri raste disku, čo je zámerne vypnuté, 403 na loopbacku), má test, ktorý
  zlyhá, keď kód tvrdenie prestane spĺňať.
- **Pull request šablóna.** Každá zmena viditeľná človeku musí mať v popise
  snímku pred a po, pri rovnakej veľkosti okna; čo sa netestovalo, musí
  byť povedané (VM testy bez KVM sa uvádzajú ako „nespustené“, nie
  zamlčané).
- **Kontroly v CI.** Tokeny príručky proti UI, odkazy z chybových hlásení,
  typová kontrola, zostavenie bez rozbitých odkazov.
- **Pravidlá písania.** Príručka vedie tým, čo čitateľ vidí; príčina druhá,
  náprava tretia; IP adresa vždy pred `.local` menom; pomenovať pull
  request, keď sa správanie nedávno zmenilo, aby čitateľ so starším boxom
  vedel, prečo sa jeho obrazovka líši.

## Čo dokumentácii chýba

Otvorene: stránky príručky zatiaľ nie sú preložené do slovenčiny a
nemčiny (preklad je pripravený technicky, nie obsahovo); niekoľko
návrhových špecifikácií v `docs/superpowers/` nesie zastarané statusy
z čias pred prepisom do Rustu; a časť wiki je stručnejšia než príručka,
lebo ťažisko sa presunulo k nej. Zoznam je v kapitole 10 medzi plánovanou
prácou.
