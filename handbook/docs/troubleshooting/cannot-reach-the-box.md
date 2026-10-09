---
title: Cannot reach the box
sidebar_position: 2
slug: /troubleshooting/cannot-reach-the-box
---

# Cannot reach the box

<div className="losos-symptom">

**What you see:** the browser says the site cannot be reached, the
connection timed out, or the page spins forever. Nothing on the box
answers.

</div>

## From the LAN

![The banner the box shows under the salmon when it has booted and has a network, with the address to type.](../img/tty1-banner.png)

1. **Read the screen.** The banner shows the address the box has *now*. A
   router that restarted may have given it a new one, and the banner redraws
   by itself. Type that address, as `http://`, not the name and not `https://`.
2. **Same network?** A laptop on the guest Wi-Fi, or on a VPN, is on a
   different network. Both the box and the computer need an address in the
   same range, for example both `192.168.1.x`. The admin pages also refuse
   any address that is not private. A request that arrives through a tunnel
   or from the internet gets **403** on purpose.
3. **The banner says "No network address yet."** Plug the Ethernet cable in,
   or move it to a different port on the router. The box has no Wi-Fi setup.
   The screen updates on its own once the cable carries a link.
4. **No banner, black screen.** Give it two minutes after power-on. If the
   screen stays black, check the power light and the monitor input. A text
   login prompt instead of the banner is the one state the nightly restart
   repairs. Wait for 00:07, or pull the power.
5. **Only one page is down.** `http://<address>/api/health` answers but
   LosOS cloud does not. Either the app is still starting, which takes a few
   minutes after any restart, or see [An app tile gives 404](app-tile-404).

## From outside the LAN

A box on its own is not reachable from outside, and that is not a fault. A
box with an edge is reachable at its public name for LosOS cloud and LosOS
Git only. The box never publishes the admin pages. If the public name stops
answering, the tunnel is down. On the LAN, the Mesh pane shows whether the
box finds the edge, and [No edge found](edge-not-found) covers the case where
it does not. The box reconnects on its own once it finds the edge.

## Still nothing

If the banner shows an address and `http://<address>/api/health` answers,
but the admin pages do not load, your browser may have cached the page from
before an update. Reload with Ctrl+Shift+R to bypass the cache, or open a
private window.

If the banner shows an address and nothing at all answers on it, the box's
web server, nginx, is down, and the nightly restart repairs it. If it is
still down the next morning, [get help](getting-help) with the banner's
text.
