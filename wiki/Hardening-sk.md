[English](Hardening) · **Slovenčina** · [Deutsch](Hardening-de)

# Hardening

`losos.hardening.enable` je predvolene zapnuté. Nastavuje:

- parametre kernelu podľa KSPP
- sysctl pre ukazovatele kernelu, logy, eBPF, ptrace, `userfaultfd`,
  `fs.protected_*` a sieťový stack
- čiernu listinu modulov kernelu, ktorá blokuje aj explicitné `modprobe`
- tmpfs `/tmp` a `noexec` na `/dev/shm`
- dbus-broker
- systemd sandboxing pre `nginx`, `avahi-daemon` a `lososd`

NixOS vo verzii 26.05 odstránil svoj hardened profil a `linux_hardened` už
neexistuje, preto ich `modules/hardening.nix` nastavuje priamo.

## Voliteľné

Každé z nasledujúcich môže niečo pokaziť, preto je každé predvolene vypnuté.
Zapnite ich v paneli Zabezpečenie na stránke nastavení alebo v Nixe:

```nix
losos.hardening.apparmor = true;   # mandatory access control
losos.hardening.malloc   = true;   # GrapheneOS hardened_malloc
losos.hardening.nosmt    = true;   # disable SMT; about half the cores
losos.hardening.usbguard = true;   # block USB devices not present at boot
```

`hardened_malloc` funguje cez preloading, takže sa nedostane do k3s, rke2
ani containerd, ktoré sú statické Go binárky, ani do ničoho vnútri podu.

## Zámerne vynechané

`tests/hardening.nix` kontroluje, že tieto zostávajú vypnuté:

| Nastavenie            | Prečo nie                                                       |
| --------------------- | --------------------------------------------------------------- |
| prísny `rp_filter`    | Zahadzuje odpovede mDNS (jediný spôsob, ako zariadenie nájsť) a rozbíja Calico. Je na `2` (voľný). |
| žiadne user namespaces | Zastaví oba kubelety aj containerd.                            |
| `noexec` na `/tmp`    | Bežia tam buildy Nixu a nočný rebuild musí fungovať.            |
