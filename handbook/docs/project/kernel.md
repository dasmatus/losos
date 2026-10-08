---
title: The kernel
sidebar_position: 2
---

# The kernel

Requirement 2: *choose a suitable kernel.* The installed appliance runs the
latest upstream Linux kernel that NixOS packages, `linuxPackages_latest`.
The installer medium and the edge run the NixOS default kernel.

## The choice

| Candidate                  | What it offers                                                | Why not, or why                                                                          |
| -------------------------- | ------------------------------------------------------------- | ---------------------------------------------------------------------------------------- |
| **Latest upstream**        | Newest drivers, newest fixes, a release every ~10 weeks       | **Chosen.** Repurposed mini-PCs have NVMe, Wi-Fi and sleep quirks that mainline fixes first. The box updates itself nightly, so tracking the latest costs no effort. |
| Long-term support (LTS)    | Fewer changes, two to six years of fixes                      | Behind on hardware. LTS buys stability as fewer surprises per manual update. A box updates itself unattended, so here the test suite and the nightly rollback buy that stability instead. |
| A hardened kernel          | Extra mitigations compiled in                                 | NixOS removed `linux_hardened` and its hardened profile in 26.05, so it is no longer there to choose. LosOS applies its useful parts as settings on the latest kernel, as described below. |
| A real-time kernel         | Deterministic latency                                         | Nothing on the box needs it.                                                              |

## What is layered on it

On the latest kernel, the hardening module applies the parts of the old
hardened profile that cost nothing: KSPP boot parameters, sysctls for kernel
pointers, logs, eBPF, ptrace, `userfaultfd`, protected files and the network
stack, a module blacklist that also blocks explicit loading, a tmpfs `/tmp`,
`noexec` on `/dev/shm`, and systemd sandboxing of the services that face the
network. Four more that can break something are opt-in: AppArmor, a hardened
memory allocator, disabling SMT, USBGuard.

![Settings, Security: the four opt-in protections layered on the kernel baseline, each with its cost.](../img/settings-security.png)

LosOS leaves out three "obvious" hardening settings on purpose, and a test
fails if one is applied. Strict reverse-path filtering drops the mDNS
replies that make a shell-less box findable, and it breaks the mesh's
network plugin. Zero user namespaces stops both Kubernetes agents. `noexec`
on `/tmp` would stop the nightly rebuild, which compiles there.

## What tracking the latest kernel costs, and how it is paid

- **Boot partition growth.** Nearly every nightly update brings a new kernel
  into the 500 MiB boot partition. The box caps the boot menu at five
  generations and collects old system versions after 14 days. An invariant
  check pins both.
- **A kernel that breaks something.** The previous system stays in the boot
  menu, and the next night rebuilds again. The VM tests boot the installed
  system, the installer, the TPM unlock, the resize path and the hardening
  on every change to those modules.
- **The TPM key is not bound to the kernel.** Measured-boot binding would
  lock the box out after every kernel update with nobody to type a recovery
  key. The [security model](../reference/security-model) states the trade.
