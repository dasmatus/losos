---
title: The name does not resolve
sidebar_position: 3
---

# The name does not resolve

<div className="losos-symptom">

**What you see:** `http://<name>.local` gives "server not found", "this
site can't be reached" or "hmm, we can't find that site", while
`http://<address>` works.

</div>

## Why

`<name>.local` is an mDNS name: the box announces it on the local network,
and your computer has to listen. Windows, macOS, iPhones, Android and most
Linux desktops do. These do not:

- **The host of a virtual machine** on a NAT network (libvirt, VirtualBox,
  the default in both). The host never hears the guest's announcement. This
  is the usual case during a demo.
- **A computer on another network**, or behind a router that blocks
  multicast between Wi-Fi and wired clients (some "AP isolation" settings).
- **Linux without an mDNS resolver** (no Avahi, or `.local` not in
  `nsswitch.conf`).

## What to do

- **Use the address.** Everything answers on it, the apps included; the
  About pane shows which address your browser is using. Only two things
  care about the name: the `https://` certificate, which is issued for the
  name, and LosOS Git's clone URLs, which print the name.
- **On a VM host**, add a line to the host's `hosts` file (`/etc/hosts` on
  Linux and macOS, `C:\Windows\System32\drivers\etc\hosts` on Windows):
  `192.168.122.56  <name>.local`. Then the name and the certificate both
  work.
- **On Linux without Avahi**, install `avahi` and `nss-mdns`, or use the
  hosts file as above.

## Not this page

If the address does not work either, it is
[Cannot reach the box](cannot-reach-the-box). If the name works but
`https://` warns, it is [Certificate warning](certificate-warning).
