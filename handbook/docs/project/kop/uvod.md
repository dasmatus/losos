---
title: Úvod
sidebar_position: 1
---

# Úvod

## Motivácia

Fotky, dokumenty, kalendár a kód väčšiny ľudí ležia v cudzích cloudoch.
Alternatíva, vlastný server, znamená údržbu: prihlasovať sa, aktualizovať,
sledovať, či niečo nespadlo, a po čase nevedieť, v akom stave server vlastne
je. Pritom doma alebo v kancelárii často stojí vyradené mini-PC, ktoré väčšinu
dňa nerobí nič.

LosOS vznikol z dvoch otázok. Prvá: dá sa postaviť server, ktorý sa po
inštalácii už nikdy nemusí spravovať inak než klikaním na webovej stránke,
a ktorý sa sám opraví, keď sa niečo pokazí? Druhá: dá sa voľný disk
a procesor takého servera bezpečne požičať iným ľuďom v sieti, a raz za to
aj dostať zaplatené?

## Cieľ práce

Cieľom je navrhnúť, implementovať, nainštalovať a zdokumentovať operačný
systém pre takéto zariadenie (v práci ho voláme **box**), ktorý:

- sa spravuje len cez webové rozhranie, bez SSH a bez shellu;
- je **bezstavový**: pri každom štarte sa postaví nanovo z jedného textového
  popisu, takže nemôže „zvetrať“ do stavu, ktorý nikto nezapísal;
- chráni dáta šifrovaním disku s kľúčom v čipe TPM a bootuje bez obsluhy;
- poskytuje vlastný cloud (Nextcloud) a hosting kódu (Forgejo);
- sa voliteľne pripája do siete ďalších boxov (**mesh**) a požičiava im
  voľný disk a procesor podľa pravidiel, ktoré určí majiteľ;
- má pripravený trh, cez ktorý sa požičaná kapacita dá predávať.

## Rozsah a čo práca nie je

Práca pokrýva operačný systém a jeho riadiacu rovinu: NixOS moduly, démon
v Ruste, webové rozhranie, inštalátor, serverovú stranu siete (edge)
a testy. Aplikácie samotné (Nextcloud, Forgejo, Kubernetes, Longhorn) sú
prevzaté hotové projekty; práca ich konfiguruje, tematicky zjednocuje a
izoluje, ale nemení ich kód.

Ukážka systému prebieha **vo virtuálnom stroji**, nie na fyzickom mini-PC.
Tento spôsob bol s konzultantom dohodnutý, aby obhajoba nezávisela od
sieťového pripojenia boxu v škole. Obraz pre virtuálny stroj sa zostavuje
z toho istého zdroja ako inštalačné médium pre hardvér, takže komisia vidí
presne ten systém, ktorý sa inštaluje na fyzický počítač.

## Metodika

Projekt je verejný repozitár na GitHube s históriou každej zmeny. Pracovný
postup bol pri každom kroku rovnaký:

1. zapísať návrh a jeho zdôvodnenie (do hlavičky súboru alebo do dokumentácie);
2. implementovať;
3. pokryť testom, ktorý zlyhá, keď niekto rozhodnutie neskôr zmení;
4. nechať zmenu prejsť kontinuálnou integráciou (CI) a ukážkou vo
   virtuálnom stroji.

Veľká časť kódu vznikla s pomocou AI asistenta ako nástroja. Rozhodnutia
o architektúre, výber technológií, recenzia každej zmeny a overenie, že sa
systém správa tak, ako tvrdí, boli prácou autora; v histórii repozitára je
pri každej zmene vidieť, čo bolo navrhnuté, čo bolo vrátené a prečo.
Kapitola 7 opisuje aj nezávislú kritickú recenziu, ktorú autor dal urobiť
nad vlastným projektom, a ako sa jej nálezy opravili.

## Štruktúra dokumentu

Kapitoly 2 a 3 odpovedajú na prvý bod zadania (funkcionality, možnosti
a architektúra). Kapitoly 4 až 7 na druhý bod (kernel, inštalácia,
konfigurácia, overenie). Kapitola 8 je cenová ponuka, kapitola 9 opisuje
dokumentáciu projektu a kapitola 10 je záver. Prílohy obsahujú slovník,
zoznam testov, štruktúru repozitára a zdroje.
