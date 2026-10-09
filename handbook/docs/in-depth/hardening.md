---
title: Hardening
sidebar_position: 8
mdx:
  format: md
---

# Hardening

`losos.hardening.enable` is on by default. It sets:

- KSPP kernel parameters
- sysctls for kernel pointers, logs, eBPF, ptrace, `userfaultfd`,
  `fs.protected_*` and the network stack
- a kernel-module blacklist that also blocks explicit `modprobe`
- a tmpfs `/tmp` and `noexec` on `/dev/shm`
- dbus-broker
- systemd sandboxing for `nginx`, `avahi-daemon` and `lososd`

NixOS removed its hardened profile in 26.05 and `linux_hardened` no longer
exists, so `modules/hardening.nix` sets these directly.

## Opt-in

Each of these can break something, so each is off by default. Turn them on in
the Security pane of the settings page, or in Nix:

```nix
losos.hardening.apparmor = true;   # mandatory access control
losos.hardening.malloc   = true;   # GrapheneOS hardened_malloc
losos.hardening.nosmt    = true;   # disable SMT; about half the cores
losos.hardening.usbguard = true;   # block USB devices not present at boot
```

`hardened_malloc` works by preloading, so it does not reach k3s, rke2 or
containerd, which are static Go binaries, nor anything inside a pod.

## Left out on purpose

`tests/hardening.nix` checks that these stay off:

| Setting               | Why not                                                         |
| --------------------- | --------------------------------------------------------------- |
| strict `rp_filter`    | Drops mDNS replies (the only way to find the box) and breaks Calico. It is `2` (loose). |
| no user namespaces    | Stops both kubelets and containerd.                             |
| `noexec` on `/tmp`    | Nix builds run there, and the nightly rebuild must work.        |
