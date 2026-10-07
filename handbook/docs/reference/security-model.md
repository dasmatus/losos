---
title: Security model
sidebar_position: 4
---

# Security model

A summary for owners. The full document, versioned with the code and written
to be argued with, is
[`docs/security-model.md`](https://github.com/dasmatus/losos/blob/main/docs/security-model.md)
in the repository.

## Trust boundaries

1. **Internet → edge.** Only the edge is internet-facing. A box opens a
   tunnel outwards and listens on nothing from outside.
2. **Edge → box.** The tunnel is encrypted end to end (Noise); the box pins
   the edge's key on first contact and refuses a changed one.
3. **LAN → box.** HTTPS with the box's own certificate, trusted once per
   computer. Until then, someone on the same Wi-Fi who can intercept
   traffic could intercept first use.
4. **Your files ↔ the mesh's copies.** Two accounts on the box with separate
   groups and mode-700 homes; the mesh's directory is encrypted with its own
   key, loaded only while sharing is on.
5. **An official edge ↔ any edge.** A root public key baked into the box
   lets it tell the LosOS edge from a company's own; trading needs the
   former, everything else works with either. The private half is made and
   used only on the project owner's own computer, by a tool that signs the
   person in with GitHub against a committed allowlist; no box and no edge
   ever holds it.

## What the admin key is

The 64-character spare key is as powerful as root: it can write any setting
and rebuild the box. It is created by the box, never leaves it except on the
wizard page that shows it once, and is handed back only to a browser that
proves the owner's password. The control API listens on the box's loopback
only, behind the LAN-only guard. Ten failed attempts throttle an address.
Settings changes, password resets and resets are written to an audit log on
the box.

## Hardening

On by default: kernel hardening parameters and sysctls, a kernel-module
blacklist, a tmpfs `/tmp`, systemd sandboxing of the web server, mDNS and the
control daemon. Four protections that can break something are opt-in on the
Security pane: AppArmor, a hardened memory allocator, no SMT, USBGuard.

## Known limitations, stated plainly

- **One origin.** The admin pages, LosOS cloud and LosOS Git share one
  address. A cross-site scripting hole in either app could read the admin
  key from a tab that has it. The strict content-security policy on the
  admin pages, the LAN-only guard and keeping the apps patched are the
  defences; a separate name for the admin pages was declined in favour of
  one address.
- **Theft of the whole box.** The TPM releases the disk key to any software
  booted on that machine, so a thief with the box can read it; only the
  disk alone is protected. On a box without a TPM, even the disk alone is
  readable.
- **The audit log** can be rewritten by root and does not rotate.
- **Throttling is per address**, so an attacker on the LAN who spoofs
  addresses gets a fresh budget each time.
- **A hand-written widget** runs in every browser that opens the box. It is
  sandboxed away from the admin key and the API, but it can show whatever its
  author wrote and fetch the internet.
