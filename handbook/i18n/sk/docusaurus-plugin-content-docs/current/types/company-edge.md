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

- **Žiadny trh, ak nejde o overenú bránu.** Firemný edge zvyčajne neukazuje
  pas brány, takže ho každé zariadenie považuje za neoficiálny. Kontrolu
  vysvetľuje časť [oficiálny alebo nie](official-edge#official-or-not). Jeho
  riadok na paneli Mesh nesie výstražný znak a panel Trh hlási „Okrajový
  proxy server, ktorý toto zariadenie našlo, neprešiel overením brány.
  Zdieľanie úložiska cez neho funguje; obchodovanie vyžaduje dostupnú
  overenú bránu LosOS.“ Vlastné zariadenia firmy si medzi sebou zdieľajú
  úložisko aj výpočtový výkon zadarmo, o čo tu ide. Firma, ktorá chce trh
  cez vlastný edge, na ňom spustí zverejnený obraz brány na počítači s TPM a
  podpíše preň pas; potom platí podiel LosOS ako každá iná brána.
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
nasadenia v `nix/demo/edge-lan/`: dve zariadenia a edge v jednej virtuálnej sieti.
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

## Správa zariadení z edge \{#managing-the-boxes-from-the-edge}

Ľudia, ktorí spravujú firemný edge, môžu na jednej stránke edge meniť LosOS
Cloud, LosOS Git a aplikácie na jeho zariadeniach. Či to edge smie, rozhoduje
vlastník každého zariadenia.

1. Na zariadení otvorte Nastavenia, potom Mesh, a zapnite **Nechať firemný
   okrajový server spravovať toto zariadenie**. Ak zariadenie pozná meno
   edge, prepínač ho ukáže. Je vypnutý, kým ho vlastník nezapne, a patrí
   edge, ktorý zariadenie práve používa. Iný edge ho nikdy nezdedí.

   ![Panel Mesh so zapnutým prepínačom, ktorý dovoľuje firemnému edge spravovať zariadenie.](@site/docs/img/mesh-manage-sk.png)

2. Na edge nastavte `losos.edge.manage.enable` (počítač s edge bránou ho má
   zapnuté). Edge si pri prvom spustení vytvorí kľúč na správu a uloží ho do
   `/var/lib/losos-registrar/manage-key`. Počítač s edge bránou ho ukazuje na
   obrazovke spolu s adresou stránky.
3. Otvorte `/manage/` na adrese edge, napríklad
   `http://<edge>.local:8443/manage/`, a prihláste sa kľúčom. Stránka ukáže
   zariadenia, ktoré edge smeruje, a otvorí tie, ktorých vlastník prepínač
   zapol.
4. Stránka zariadenia má nastavenia LosOS Cloud a LosOS Git z panela
   Aplikácie na zariadení, nainštalované aplikácie a hľadanie aplikácií.
   Zmena zariadenie prebuduje rovnako ako zmena urobená priamo na ňom.

![Stránka zariadenia na firemnom edge: nastavenia LosOS Cloud a LosOS Git, nainštalované aplikácie a hľadanie aplikácií.](@site/docs/img/edge-manage-box-sk.png)

Edge nevidí ničie súbory, nepridáva ľudí, nemení heslá a nesiaha na
úložisko, zálohy, sieť ani na samotný prepínač. Zariadenie kontroluje každé
volanie samo, takže upravená stránka na edge sa ďalej nedostane. Spravovať
zariadenie môže len edge v jeho vlastnej sieti, nikdy edge LosOS: LosOS
nikomu zariadenie nespravuje. Keď vlastník prepínač vypne, ďalšie volanie
edge zariadenie odmietne.

Stránku celú opisuje [Webová stránka edge](/in-depth/edge-web-ui.md).

## Pre koho je to \{#who-this-is-for}

Pre malé firmy, ktoré chcú nasadenie „jedno zariadenie na tím, jeden
spoločný priestor pre všetkých“, pri ktorom nič neopustí budovu. Potrebujú
niekoho, kto zvládne prevádzkovať jeden server s NixOS. V tejto podobe je
LosOS určený na predaj.
