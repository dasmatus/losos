---
title: Hardening
sidebar_position: 8
mdx:
  format: md
---

# Hardening

![Nastavenia, Zabezpečenie: štyri ochrany, ktoré sú predvolene vypnuté, každá s tým, čo stojí.](@site/docs/img/settings-security-sk.png)

`losos.hardening.enable` je predvolene zapnuté. Nastavuje:

- parametre kernelu podľa KSPP
- sysctl pre ukazovatele kernelu, logy, eBPF, ptrace, `userfaultfd`,
  `fs.protected_*` a sieťový stack
- čiernu listinu modulov kernelu, ktorá blokuje aj explicitné `modprobe`
- tmpfs `/tmp` a `noexec` na `/dev/shm`
- dbus-broker
- systemd sandboxing pre `nginx` a `lososd`

:::note[Prečo ich modul nastavuje sám]
NixOS vo verzii 26.05 odstránil svoj hardened profil a `linux_hardened` už
neexistuje, preto ich `modules/hardening.nix` nastavuje priamo.
:::

## mDNS {#mdns}

Zariadenie ohlasuje `<name>.local` a svoje služby cez systemd-resolved, nie
cez Avahi. resolved je aj tak súčasťou systému, beží pod vlastným
používateľom v sandboxe svojej upstream jednotky a ničomu inému na
zariadení nedáva spôsob, ako ohlasovať služby. Jeho druhý protokol mien,
LLMNR, je zapnutý len vtedy, keď je zapnutý priečinok pre Windows
(`losos.cloud.windows.enable`): Windows nájde zariadenie podľa holého
názvu a na jeho adresu sa pýta cez LLMNR. Avahi bežalo ako root s ľahším sandboxom, lebo
potrebovalo netlink, aby sledovalo sieťové rozhrania. Je preč a nebeží
vedľa resolved, pretože dva respondery na jednom porte odpovedajú na každú
otázku dvakrát.

mDNS je zapnuté na každom fyzickom sieťovom rozhraní, bez zoznamu mien,
lebo meno je jediná cesta k zariadeniu bez obrazovky a bez shellu.
`nix/tests/hardening.nix` overuje, že sa dve zariadenia stále navzájom
nájdu.

## Voliteľné {#opt-in}

Každé z nasledujúcich môže niečo pokaziť, preto je každé predvolene vypnuté.
Zapnite ich v paneli Zabezpečenie na stránke nastavení alebo v Nixe:

```nix
losos.hardening.apparmor = true;   # mandatory access control
losos.hardening.malloc   = true;   # GrapheneOS hardened_malloc
losos.hardening.nosmt    = true;   # disable SMT; about half the cores
losos.hardening.usbguard = true;   # block USB devices not present at boot
```

:::note[Kam sa hardened_malloc nedostane]
`hardened_malloc` funguje cez preloading, takže sa nedostane do k3s, rke2
ani containerd, ktoré sú statické Go binárky, ani do ničoho vnútri podu.
:::

## Zámerne vynechané {#left-out-on-purpose}

`nix/tests/hardening.nix` kontroluje, že tieto zostávajú vypnuté:

| Nastavenie            | Prečo nie                                                       |
| --------------------- | --------------------------------------------------------------- |
| prísny `rp_filter`    | Zahadzuje odpovede mDNS (jediný spôsob, ako zariadenie nájsť) a rozbíja Calico. Je na `2` (voľný). |
| žiadne user namespaces | Zastaví oba kubelety aj containerd.                            |
| `noexec` na `/tmp`    | Bežia tam buildy Nixu a nočný rebuild musí fungovať.            |
