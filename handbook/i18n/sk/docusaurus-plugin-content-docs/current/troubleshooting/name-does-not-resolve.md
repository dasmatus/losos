---
title: Meno sa nedá preložiť
sidebar_position: 3
slug: /troubleshooting/name-does-not-resolve
---

# Meno sa nedá preložiť

<div className="losos-symptom">

**Čo vidíte:** `http://<name>.local` hlási „server sa nenašiel“, „táto
stránka nie je dostupná“ alebo „hmm, túto stránku nemôžeme nájsť“, zatiaľ
čo `http://<address>` funguje.

</div>

## Prečo \{#why}

`<name>.local` je mDNS meno. Zariadenie ho ohlasuje v lokálnej sieti a váš
počítač ho musí počúvať. Windows, macOS, iPhony, Android a väčšina
linuxových desktopov to robí. Tieto nie:

- **Hostiteľ virtuálneho stroja** v sieti NAT, ktorá je predvolená
  v libvirt aj vo VirtualBoxe. Hostiteľ ohlásenie hosťa nikdy nepočuje.
  Pri ukážke je to bežný prípad.
- **Počítač v inej sieti** alebo za routerom, ktorý blokuje multicast medzi
  klientmi na Wi-Fi a na kábli, ako to robia niektoré nastavenia
  „izolácie AP“.
- **Linux bez mDNS resolvera**, teda bez Avahi alebo bez `.local`
  v `nsswitch.conf`.

## Čo robiť \{#what-to-do}

![Banner pod lososom na obrazovke zariadenia: IP adresa funguje tam, kde meno .local nie.](@site/docs/img/tty1-banner.png)

- **Použite adresu.** Odpovedá na nej všetko, aplikácie nevynímajúc. Panel
  O zariadení ukazuje, ktorú adresu váš prehliadač používa. Meno potrebujú
  iba dve veci: certifikát pre `https://`, ktorý zariadenie vydáva pre
  meno, a URL na klonovanie v LosOS Git, ktoré meno vypisujú.
- **Na hostiteľovi VM** pridajte riadok do súboru `hosts` hostiteľa. Na
  Linuxe a macOS je to `/etc/hosts`, vo Windows
  `C:\Windows\System32\drivers\etc\hosts`. Riadok je
  `192.168.122.56  <name>.local`. Potom funguje meno aj certifikát.
- **Na Linuxe bez Avahi** nainštalujte `avahi` a `nss-mdns`, alebo použite
  súbor hosts, ako je opísané vyššie.

## Nie je to táto stránka \{#not-this-page}

Ak nefunguje ani adresa, ide o stránku
[Zariadenie je nedostupné](cannot-reach-the-box). Ak meno funguje, ale
`https://` varuje, ide o stránku [Varovanie o certifikáte](certificate-warning).
