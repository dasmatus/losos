---
title: Firma s vlastným edge
sidebar_position: 3
---

# Firma s vlastným edge

Firma prevádzkuje niekoľko zariadení v jednej sieti a vlastný edge server,
buď v tej istej sieti, alebo na prenajatom serveri. Všetko zostáva vo firme.
Zariadenia nájdu edge v LAN, delia sa medzi sebou o disk a CPU a odpovedajú
pod doménou firmy cez firemný edge. V sieti úplne bez internetu takto
spojíte úložisko do jedného celku. Edge prevádzkuje jeden stále zapnutý
počítač v LAN a zariadenia ho nájdu samy.

## Čo je rovnaké ako s edge LosOS \{#what-is-the-same-as-the-losos-edge}

- Vzdialený prístup pod doménou edge bez otvárania portov na zariadeniach.
- Mesh so spoločným replikovaným priestorom na disku a voľným výpočtovým
  výkonom firemných zariadení, plánovaným v rámci hodín, ktoré si nastavilo
  každé zariadenie.
- Vyhľadávanie v LAN. Zariadenie v tej istej sieti nájde edge počas jedného
  20-sekundového prehľadávania bez toho, aby mu niekto povedal jeho adresu,
  a panel Mesh ho ukáže ako **V tejto sieti**.
- Admin stránky každého zariadenia zostávajú dostupné iba z LAN.

![Panel Mesh vo firemnej sieti: kancelársky edge nájdený V tejto sieti, s výstražným znakom, pretože nie je oficiálny.](@site/docs/img/mesh-company-sk.png)

## Čo je iné \{#what-is-different}

- **Žiadny trh.** Firemný edge nedokáže predložiť podpis LosOS, takže ho
  každé zariadenie považuje za neoficiálny. Kontrolu vysvetľuje časť
  [oficiálny alebo nie](official-edge#official-or-not). Jeho riadok na paneli
  Mesh nesie výstražný znak a panel Trh hlási „Okrajový proxy server, ktorý toto zariadenie našlo, neprevádzkuje LosOS.
  Zdieľanie úložiska cez neho funguje; obchodovanie vyžaduje dostupný oficiálny okrajový server LosOS.“
  Vlastné zariadenia firmy si medzi sebou zdieľajú úložisko aj výpočtový
  výkon zadarmo, o čo tu ide.
- **Kľúče drží firma.** Pár kľúčov tunela, token klastra a zoznam povolených
  zariadení sú na firemnom edge. Nič z dát ani prevádzky firmy sa projektu
  LosOS nedotkne.
- **Aktualizácie stále prichádzajú z projektu.** Firma môže namiesto toho
  nasmerovať svoje zariadenia na fork, pretože zdroj aktualizácií je
  nastavenie.

## Nastavenie \{#setting-it-up}

Edge je modul NixOS, ktorý je súčasťou LosOS, `nixosModules.edge`. Edge
server nainštalujete rovnako, ako sa konfiguruje zariadenie, krátkym súborom
v Nixe. Ten určuje verejnú doménu, zariadenia, ktoré sa smú pripojiť, či má
bežať riadiaca vrstva mesh a či sa má edge ohlasovať v LAN. Postup pre
prevádzkovateľa je v kapitolách [Master proxy](/in-depth/master-proxy.md)
a [Mesh](/in-depth/mesh.md). Repozitár obsahuje aj ukážku presne tohto
nasadenia v `demo/edge-lan/`: dve zariadenia a edge v jednej virtuálnej sieti.
`run.sh` ju spustí a `CHECKLIST.md` je poradie, v ktorom ňou prechádza.

![Obrazovka počítača s edge bránou z ukážky: ohlasuje sa v sieti a zariadenia v tej sieti ho nájdu samy.](@site/docs/img/edge-gateway-tty1.png)

Stručne:

1. Nainštalujte NixOS na edge server a importujte modul.
2. Nastavte `losos.edge.publicDomain`, uveďte každé zariadenie pod
   `losos.edge.tenants` so súborom s tokenom a pre mesh zapnite
   `losos.edge.cluster.enable`.
3. Pre edge vo vlastnej sieti firmy nastavte `losos.edge.lan.advertise` na
   `true`. Edge sa potom ohlasuje cez mDNS, predvolene ako
   `http://<edge>.local:8443`, a otvorí svoje API do LAN. Každé zariadenie
   v tej sieti ho nájde bez akejkoľvek konfigurácie.
4. Na každom zariadení mu dajte vlastný názov a zapnite **Dostupné aj mimo
   domova**, obe na paneli Sieť, a potom zapnite **Pripojiť sa k sieti
   mesh**. Zariadenie sa k edge prihlási svojím kľúčom tunela. Každé
   zariadenie začína s rovnakým názvom a edge ho prijme iba raz. Ak edge
   *nie je* v tej istej sieti, najprv zadajte jeho adresu v Nastavenia →
   Rozšírené, v `losos.proxy.registrarUrl`. Ohlasovania mDNS neprejdú cez
   routery.

## Pre koho je to \{#who-this-is-for}

Pre malé firmy, ktoré chcú nasadenie „jedno zariadenie na tím, jeden
spoločný priestor pre všetkých“, pri ktorom nič neopustí budovu. Potrebujú
niekoho, kto zvládne prevádzkovať jeden server s NixOS. V tejto podobe je
LosOS určený na predaj.
