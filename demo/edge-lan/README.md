# An edge and a box on one network

This directory is the reference deployment of the two halves of LosOS on
one network: an **edge proxy** (`modules/edge.nix`, the piece that holds a
mesh together and makes boxes reachable) and a **box** installed from the
installer ISO beside it. `run.sh` stands the whole thing up on one machine,
as two QEMU VMs on a private virtual switch, and walks through what the box
does on its own: it finds the edge over mDNS, sharing is allowed; the edge is
powered off, the box refuses to share and says why; the edge returns and the
gate reopens. The same script is the demo and the rehearsal for a real site.

    demo/edge-lan/run.sh build          # or: build --iso path/to/losos.iso
    demo/edge-lan/run.sh up             # LAN, edge, install, boot, claim
    demo/edge-lan/run.sh walk           # the walkthrough, over the API
    demo/edge-lan/run.sh edge off|on    # take the edge away, bring it back
    demo/edge-lan/run.sh down           # stop (keeps the disks); `clean` wipes

Open <http://127.0.0.1:8080/mesh> while it runs and sign in with the
password `up` printed, to watch the Mesh pane move. The host needs `nix`
with flakes; QEMU, VDE and swtpm come through `nix shell`, nothing runs as
root, and the only host ports are two forwards on 127.0.0.1. With `/dev/kvm`
the whole thing takes a few minutes; without it, budget about an hour for
the install and another for the first boot. Requirements, knobs and the
state directory are listed at the top of `run.sh`.

## What the pieces are

| In the demo | On a company network |
| --- | --- |
| `vde_switch`, a unix socket | the office switch |
| `edge-vm.nix`: `nixosModules.edge` with `losos.edge.lan.advertise = true`, plus DHCP, DNS and NAT for the LAN on its second NIC | one always-on machine or VM running `nixosModules.edge` with `lan.advertise` on; the existing router keeps DHCP and DNS if it has them (the module does not need to be the router, the demo's is only so the LAN has one) |
| the box: the stock installer ISO, unattended | the same ISO, written to a USB stick, one per mini-PC |
| `run.sh walk` over `/api/*` | the Mesh pane in a browser on the same LAN |

Nothing on the box is demo-specific: it is the published `install`
configuration, and finding the edge is what every box does
(`backend/src/edge.rs`, [Mesh — Finding the edge](../../wiki/Mesh.md)).
`edge-vm.nix` is `modules/edge.nix` plus the LAN-side plumbing; the edge-side
option it turns on, `losos.edge.lan.advertise`, is the one a real site sets
([Master proxy — Edge on the same LAN](../../wiki/Master-Proxy.md)).

## What is demo-only

- The tenant tokens are fixtures written by tmpfiles; a site issues real ones
  (see the Master proxy page).
- The edge's hostname is `edge`, so the advert names `http://edge.local:8443`;
  a site picks its own, and `losos.edge.lan.url` follows the hostname.
- There is no public DNS and no certificate: ACME fails in the VM and is
  logged, not fatal. A site with a public name gets Let's Encrypt as the
  edge module always did.
- The box's `losos.proxy.registrarUrl` (the address the tunnel enrols at)
  is a build-time option that still defaults to the public edge. Discovery
  opens the sharing gate; a site that wants the tunnel to terminate on its
  LAN edge sets that option to the edge's URL in its own flake. Feeding the
  discovered URL into the tunnel is not done yet.
- The two VMs talk over slirp to the outside: the edge NATs the LAN to its
  own user-mode network, so a box without the edge really has no internet.
