---
title: Nenašiel sa žiadny edge
sidebar_position: 11
slug: /troubleshooting/edge-not-found
---

# Nenašiel sa žiadny edge

<div className="losos-symptom">

**Čo vidíte:** panel Mesh hlási **Žiadny okrajový proxy server sa nenašiel**
a **Zdieľanie je vypnuté**; prepínače mesh a zdieľania disku sú sivé
s textom „Vyžaduje dostupný okrajový proxy server.“; použitie zmien bolo
odmietnuté s hláškou „no edge proxy is reachable from this box“; verejné
meno zariadenia prestalo odpovedať.

</div>

## Čo zariadenie hľadá \{#what-the-box-looks-for}

![Čo zariadenie skúša každých 20 sekúnd: LAN cez DNS-SD a nastavenú adresu; keď neodpovie ani jedno, výsledok je Žiadny okrajový proxy server sa nenašiel.](@site/docs/img/edge-discovery-sk.svg)

Každých 20 sekúnd zariadenie hľadá edge na dvoch miestach. Počúva ohlásenia
v lokálnej sieti, a tak nájde [firemný edge](../types/company-edge) v tej
istej LAN. Okrem toho sa pýta nastavenej adresy, a tak sa dostane k
[edge LosOS](../types/official-edge) alebo k firemnému edge inde. Edge sa
počíta, len čo odpovie jeho `/health`. Panel Mesh vypíše každý edge, ktorý
zariadenie našlo, a pri každom značku, či preukázal, že je oficiálny. Ak
nenašlo žiadny, veta pod **Žiadny okrajový proxy server sa nenašiel** hovorí,
čo zariadenie skúsilo:

| Panel hlási                                                                | Význam                                                                        |
| -------------------------------------------------------------------------- | ----------------------------------------------------------------------------- |
| „Zariadenie prehľadalo svoju sieť a oslovilo `<address>`; nič neodpovedalo.“ | Zariadenie skúsilo oboje. Adresa je tá nastavená.                            |
| „Zariadenie prehľadalo svoju sieť a nič neodpovedalo.“                     | Žiadna adresa nie je nastavená, takže zariadenie prehľadalo iba lokálnu sieť. |
| „Zariadenie nemohlo prehľadať svoju sieť a inde nič neodpovedalo.“         | Služba mDNS zariadenia nebežala, takže edge v LAN by nenašlo, ani keby tam bol. Reštart službu vráti. |

## Z LAN \{#from-the-lan}

![Panel Mesh s hlásením Žiadny okrajový proxy server sa nenašiel a s tým, čo zariadenie prehľadalo a koho oslovilo.](@site/docs/img/mesh-none.png)

1. **Má zariadenie internet?** Edge na internete ho potrebuje, firemný edge
   v LAN nie. Prehľad ukazuje, či zariadenie odpovedá, ale kontrolku
   „internet“ zariadenie nemá. Skúste adresu edge vo vlastnom prehliadači
   z tej istej siete. Ak funguje iba lokálna sieť, pozrite
   [Funguje iba lokálna sieť](only-the-lan-works).
2. **Je adresa správna?** Otvorte Nastavenia → Rozšírené a nájdite
   `losos.proxy.registrarUrl`. Je to základná adresa API edge, pre riadiacu
   vrstvu edge LosOS `https://losos-edge.dasmat.us`. Firemný edge v LAN
   ohlasuje svoju vlastnú, zvyčajne `http://<edge>.local:8443`.
3. **Firemný edge v LAN: ten istý segment siete?** Ohlásenia mDNS
   neprechádzajú cez routery ani VLAN. Namiesto spoliehania sa na
   vyhľadávanie zadajte adresu edge.
4. **Ohlasuje sa edge?** Zariadenia nájdu firemný edge v LAN iba vtedy, keď
   je zapnuté `losos.edge.lan.advertise`. Bez neho zariadenia potrebujú
   adresu edge. Túto možnosť nastavuje ten, kto edge prevádzkuje.
5. **Je toto zariadenie zaregistrované?** Edge prijíma iba zariadenia zo
   svojho zoznamu povolených, každé s tokenom. Vyhľadávanie registráciu
   nepotrebuje, tunel áno. Preinštalované zariadenie má novú identitu a ten,
   kto edge prevádzkuje, ho musí zaregistrovať znova.
6. **Bol edge preinštalovaný?** Zariadenie si pri prvom kontakte zapamätá
   kľúč tunela edge a znovu postavený edge s novým kľúčom zámerne odmietne.
   Prevádzkovateľ buď zapamätaný kľúč na zariadení vymaže, alebo zariadenie
   zaregistruje znova. Zapamätaný kľúč je súbor medzi tajomstvami zariadenia
   a dostane sa k nemu iba prestavba.

## Zariadenie sa pripojí znova samo \{#the-box-reconnects-on-its-own}

Keď edge znova odpovie, najbližšie hľadanie ho nájde do 20 sekúnd a panel
Mesh ho o pár sekúnd neskôr ukáže. Tunel sa obnoví bez reštartu. Výpadok
nevypne nič, čo ste zapli. Kým zariadenie nenachádza žiadny edge, odmieta
iba *zapínanie* a to rieši stránka [Zdieľanie je odmietnuté](sharing-refused).
