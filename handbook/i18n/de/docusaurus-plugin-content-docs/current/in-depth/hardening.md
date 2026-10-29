---
title: Härtung
sidebar_position: 8
mdx:
  format: md
---

# Härtung

![Einstellungen, Sicherheit: die vier Schutzmaßnahmen, die standardmäßig aus sind, jede mit dem, was sie kostet.](@site/docs/img/settings-security-de.png)

`losos.hardening.enable` ist standardmäßig eingeschaltet. Es setzt:

- KSPP-Kernelparameter
- sysctls für Kernel-Pointer, Logs, eBPF, ptrace, `userfaultfd`,
  `fs.protected_*` und den Netzwerk-Stack
- eine Blacklist für Kernelmodule, die auch ein explizites `modprobe`
  blockiert
- ein tmpfs-`/tmp` und `noexec` auf `/dev/shm`
- dbus-broker
- systemd-Sandboxing für `nginx` und `lososd`

:::note[Warum das Modul sie selbst setzt]
NixOS hat sein Hardened-Profil in 26.05 entfernt, und `linux_hardened` gibt
es nicht mehr, deshalb setzt `modules/hardening.nix` diese Werte direkt.
:::

## mDNS {#mdns}

Die Box kündigt `<name>.local` und ihre Dienste über systemd-resolved an,
nicht über Avahi. resolved ist ohnehin Teil des Systems, läuft als eigener
Benutzer in der Sandbox seiner Upstream-Unit und gibt nichts anderem auf
der Box einen Weg, Dienste anzukündigen. Sein zweites Namensprotokoll,
LLMNR, ist nur an, solange die Windows-Freigabe an ist
(`losos.cloud.windows.enable`): Windows findet die Box über einen bloßen
Namen und fragt per LLMNR nach ihrer Adresse. Avahi lief als root mit einer leichteren Sandbox, weil es
Netlink brauchte, um den Netzwerkschnittstellen zu folgen. Es ist weg und
läuft nicht neben resolved weiter, denn zwei Responder an einem Port
beantworten jede Frage doppelt.

mDNS ist auf jeder physischen Netzwerkschnittstelle an, ohne Liste von
Namen, weil der Name der einzige Weg zu einer Box ohne Bildschirm und ohne
Shell ist. `nix/tests/hardening.nix` prüft, dass zwei Boxen einander
weiterhin finden.

## Optional {#opt-in}

Jede dieser Optionen kann etwas kaputt machen, deshalb ist jede standardmäßig
aus. Schalte sie im Bereich Sicherheit der Einstellungsseite ein oder in
Nix:

```nix
losos.hardening.apparmor = true;   # mandatory access control
losos.hardening.malloc   = true;   # GrapheneOS hardened_malloc
losos.hardening.nosmt    = true;   # disable SMT; about half the cores
losos.hardening.usbguard = true;   # block USB devices not present at boot
```

:::note[Was hardened_malloc nicht erreicht]
`hardened_malloc` arbeitet per Preloading und erreicht deshalb weder k3s,
rke2 noch containerd, die statische Go-Binaries sind, und auch nichts
innerhalb eines Pods.
:::

## Bewusst weggelassen {#left-out-on-purpose}

`nix/tests/hardening.nix` prüft, dass diese ausgeschaltet bleiben:

| Einstellung           | Warum nicht                                                     |
| --------------------- | --------------------------------------------------------------- |
| strikter `rp_filter`  | Verwirft mDNS-Antworten (der einzige Weg, die Box zu finden) und legt Calico lahm. Er steht auf `2` (lose). |
| keine User-Namespaces | Stoppt beide Kubelets und containerd.                           |
| `noexec` auf `/tmp`   | Dort laufen Nix-Builds, und der nächtliche Rebuild muss funktionieren. |
