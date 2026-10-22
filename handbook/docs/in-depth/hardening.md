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
- systemd sandboxing for `nginx` and `lososd`

:::note[Why the module sets these itself]
NixOS removed its hardened profile in 26.05 and `linux_hardened` no longer
exists, so `modules/hardening.nix` sets these directly.
:::

## mDNS

The box announces `<name>.local` and its services through
systemd-resolved, not Avahi. resolved is already part of the system, runs
as its own user under its upstream sandbox, and gives nothing else on the
box a way to announce services. Its other name protocol, LLMNR, is on only
while the Windows share is (`losos.cloud.windows.enable`): Windows finds
the box by a bare name and asks for its address with LLMNR.
Avahi ran as root with a lighter sandbox, because it needed netlink to
follow the network interfaces. It is gone, not kept beside resolved, since
two responders on one port answer every question twice.

mDNS is on for every physical network interface, with no list of names,
because the name is the only way to reach a box with no screen and no
shell. `nix/tests/hardening.nix` checks that two boxes still find each
other.

## Opt-in

Each of these can break something, so each is off by default. Turn them on in
the Security pane of the settings page, or in Nix:

```nix
losos.hardening.apparmor = true;   # mandatory access control
losos.hardening.malloc   = true;   # GrapheneOS hardened_malloc
losos.hardening.nosmt    = true;   # disable SMT; about half the cores
losos.hardening.usbguard = true;   # block USB devices not present at boot
```

:::note[What hardened_malloc does not reach]
`hardened_malloc` works by preloading, so it does not reach k3s, rke2 or
containerd, which are static Go binaries, nor anything inside a pod.
:::

## Left out on purpose

`nix/tests/hardening.nix` checks that these stay off:

| Setting               | Why not                                                         |
| --------------------- | --------------------------------------------------------------- |
| strict `rp_filter`    | Drops mDNS replies (the only way to find the box) and breaks Calico. It is `2` (loose). |
| no user namespaces    | Stops both kubelets and containerd.                             |
| `noexec` on `/tmp`    | Nix builds run there, and the nightly rebuild must work.        |
