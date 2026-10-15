---
sidebar_class_name: hb-icon hb-icon-home
slug: /
title: Príručka LosOS
sidebar_position: 0
sidebar_label: Vitajte
---

# Príručka LosOS

LosOS je malé zariadenie pre domácnosť alebo firmu. Uchováva vaše súbory,
fotky, kalendár a kód a stará sa samo o seba. Nemá obrazovku na prihlásenie,
shell ani SSH. Pristupujete k nemu z webového prehliadača v tej istej sieti
a raz za noc sa samo zostaví nanovo a reštartuje.

![Admin stránky zariadenia s názvom mattbox: dlaždice aplikácií, karta úložiska a nástenka s widgetmi.](@site/docs/img/overview.png)

Táto príručka je návod pre vlastníka, teda pre človeka, ktorý má zariadenie
na stole. Ľudia, ktorí LosOS vyvíjajú, prevádzkujú edge alebo pripravujú
vydanie, nájdu svoje v kapitole [Do hĺbky](/in-depth/index.md).

## Kde túto príručku nájdete \{#where-this-handbook-is}

- **Priamo na zariadení**, na `http://<address>/handbook/`, kde `<address>`
  je to, čo ukazuje obrazovka zariadenia. Zariadenie nesie celú príručku
  aj s vyhľadávaním, takže ju môžete čítať, aj keď nejde internet alebo keď
  sa k zariadeniu dostane iba vaša lokálna sieť. Odkazuje na ňu panel
  **O zariadení** na admin stránkach.
- **Na webe**, na [losos.dasmat.us](https://losos.dasmat.us). Text je rovnaký,
  len s novším dátumom.

## Začnite tu \{#start-here}

- Máte v rukách nové zariadenie: [Čo je LosOS](/start/what-is-losos.md), potom
  [Inštalácia](/start/install.md) a [Prvé spustenie](/start/first-run.md).
- Rozhodujete sa, ako ho nasadiť: [Typy nasadenia](/types/index.md) uvádzajú
  každú podobu, akú môže inštalácia LosOS mať, od jedného zariadenia v domácej
  sieti po firmu s vlastným edge.
- Niečo nefunguje a máte k dispozícii iba lokálnu sieť:
  [Keď niečo nefunguje](/troubleshooting/index.md) je zoradené podľa toho, čo
  vidíte, a každá stránka začína krokmi, ktorým stačí LAN.
- Správa zariadenia z terminálu na inom počítači:
  [Terminálový klient](/manual/terminal.md). Prečítajte si ho pred
  inštaláciou, pretože kľúč, ktorý potrebuje, patrí na inštalačný USB kľúč.
- Zariadenie funguje, ale niečo mimo neho nie, napríklad poskytovateľ, router
  alebo edge: [Funguje iba lokálna sieť](/troubleshooting/only-the-lan-works.md)
  hovorí, čo funguje ďalej, čo počká a že potom nemusíte nič robiť znova.

## Ako čítať adresy \{#how-to-read-the-addresses}

V celej príručke `<address>` znamená IP adresu na obrazovke zariadenia,
napríklad `192.168.1.23`, a `<name>` znamená názov, ktorý ste zariadeniu dali
pri nastavení. Zariadenie odpovedá na `http://<address>` aj na
`http://<name>.local`. Druhú adresu nájdu iba počítače, ktoré vedia
preložiť mDNS názvy, takže IP adresa je vždy istá voľba. Pozrite si
[Prístup k zariadeniu](/start/reaching-the-box.md).
