---
title: The name does not resolve
sidebar_position: 3
slug: /troubleshooting/name-does-not-resolve
---

# The name does not resolve

<div className="losos-symptom">

**What you see:** `http://<name>.local` gives "server not found", "this
site can't be reached" or "hmm, we can't find that site", while
`http://<address>` works.

</div>

## Why

`<name>.local` is an mDNS name. The box announces it on the local network,
and your computer has to listen for it. Windows, macOS, iPhones, Android and
most Linux desktops do. These do not:

- **The host of a virtual machine** on a NAT network, which is the default
  in both libvirt and VirtualBox. The host never hears the guest's
  announcement. This is the usual case during a demo.
- **A computer on another network**, or behind a router that blocks
  multicast between Wi-Fi and wired clients, as some "AP isolation"
  settings do.
- **Linux without an mDNS resolver**, meaning no Avahi, or no `.local` in
  `nsswitch.conf`.

## What to do

![The banner on the box's screen: the IP address works where the .local name does not.](../img/tty1-banner.png)

- **Use the address.** Everything answers on it, the apps included. The
  About pane shows which address your browser is using. Only two things
  need the name: the `https://` certificate, which the box issues for the
  name, and LosOS Git's clone URLs, which print the name.
- **On a VM host**, add a line to the host's `hosts` file. That file is
  `/etc/hosts` on Linux and macOS and
  `C:\Windows\System32\drivers\etc\hosts` on Windows. The line is
  `192.168.122.56  <name>.local`. Then the name and the certificate both
  work.
- **On Linux without Avahi**, install `avahi` and `nss-mdns`, or use the
  hosts file as above.

## Not this page

If the address does not work either, it is
[Cannot reach the box](cannot-reach-the-box). If the name works but
`https://` warns, it is [Certificate warning](certificate-warning).
