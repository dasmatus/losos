---
title: Reaching the box
sidebar_position: 4
---

# Reaching the box

Everything on a LosOS box is a web page on one address. There are two ways to
write that address, and they are not equally reliable.

## By IP address (always works)

The banner on the box's screen shows `http://192.168.x.y`, the address your
router gave the box. Every page answers on it: the admin pages at `/`, LosOS
cloud at `/nextcloud`, LosOS Git at `/forgejo/`, this handbook at
`/handbook/`. The box redraws the banner when the address changes.

![The banner under the salmon on the box's screen, with the address to type into a browser and the box's .local name.](../img/tty1-banner.png)

If the box has no screen attached, your router's device list shows the box
under the name you gave it, or as `losos` before setup.

## By name (works on most computers)

The box announces `<name>.local` over mDNS. Windows, macOS, iPhones, Android
phones and most Linux desktops resolve it, so `http://<name>.local` opens the
same pages. Two kinds of computer do not:

- The host of a virtual machine on a NAT network, such as libvirt or
  VirtualBox. The host never sees the guest's announcements.
- A computer on a different network, or behind a router that filters
  multicast.

The admin pages' **About** pane shows the address your browser is using right
now, and the `.local` name on its own row, so you can see which one you have.
Links the apps hand out, such as share links and clone URLs, use the name. The
name stays the same when the router hands out a new address.

![The About pane: the address this browser used, the .local name, storage mode, mesh state and the link to this handbook.](../img/settings-about.png)

## Over HTTPS

The box serves `https://` on port 443 with its own certificate for
`<name>.local`, valid for two years. After you install that certificate, as
in [first run, step 1](first-run#step-1-trust-this-box), `https://<name>.local`
shows no warning. The certificate does not contain the IP address, so
`https://<address>` always warns. Use plain `http://<address>` or the name
instead.

## "Find my box"

If you would rather not read a screen and type an address, open
[losos-edge.dasmat.us/find](https://losos-edge.dasmat.us/find) in Chrome 142
or newer and press **Find my box**. Chrome asks once whether the page may
look for devices on your local network. Say yes, and the page finds the box
by its name and links you to it. The lookup runs inside your browser, and
nothing about your network leaves your computer. Firefox and Safari have no
such permission, so there the page shows how to type the address instead.

<img src={require("../img/find-my-box.png").default} alt="The Find my box page after Chrome was allowed to look on the local network: the box found by its name, with a link to open it." width="440" />

## From outside your network

By default, nowhere. A box is reachable from the local network only. The
[official edge](../types/official-edge) setup gives it a public address
through a tunnel. Even then the admin pages stay LAN-only, and only LosOS
cloud and LosOS Git go out through the tunnel.

![Settings, Network: the name, Encrypt the connection, and Reachable from outside your home, the switch that opens the tunnel to an edge.](../img/settings-network.png)
