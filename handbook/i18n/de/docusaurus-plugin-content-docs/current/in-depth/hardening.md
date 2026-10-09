---
title: Härtung
sidebar_position: 8
mdx:
  format: md
---

# Härtung

`losos.hardening.enable` ist standardmäßig eingeschaltet. Es setzt:

- KSPP-Kernelparameter
- sysctls für Kernel-Pointer, Logs, eBPF, ptrace, `userfaultfd`,
  `fs.protected_*` und den Netzwerk-Stack
- eine Blacklist für Kernelmodule, die auch ein explizites `modprobe`
  blockiert
- ein tmpfs-`/tmp` und `noexec` auf `/dev/shm`
- dbus-broker
- systemd-Sandboxing für `nginx`, `avahi-daemon` und `lososd`

NixOS hat sein Hardened-Profil in 26.05 entfernt, und `linux_hardened` gibt
es nicht mehr, deshalb setzt `modules/hardening.nix` diese Werte direkt.

## Optional

Jede dieser Optionen kann etwas kaputt machen, deshalb ist jede standardmäßig
aus. Schalte sie im Bereich Sicherheit der Einstellungsseite ein oder in
Nix:

```nix
losos.hardening.apparmor = true;   # mandatory access control
losos.hardening.malloc   = true;   # GrapheneOS hardened_malloc
losos.hardening.nosmt    = true;   # disable SMT; about half the cores
losos.hardening.usbguard = true;   # block USB devices not present at boot
```

`hardened_malloc` arbeitet per Preloading und erreicht deshalb weder k3s,
rke2 noch containerd, die statische Go-Binaries sind, und auch nichts
innerhalb eines Pods.

## Bewusst weggelassen

`tests/hardening.nix` prüft, dass diese ausgeschaltet bleiben:

| Einstellung           | Warum nicht                                                     |
| --------------------- | --------------------------------------------------------------- |
| strikter `rp_filter`  | Verwirft mDNS-Antworten (der einzige Weg, die Box zu finden) und legt Calico lahm. Er steht auf `2` (lose). |
| keine User-Namespaces | Stoppt beide Kubelets und containerd.                           |
| `noexec` auf `/tmp`   | Dort laufen Nix-Builds, und der nächtliche Rebuild muss funktionieren. |
