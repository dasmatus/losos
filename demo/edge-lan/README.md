# Two boxes and an edge on one network

This directory is the reference deployment of the two halves of LosOS on
one network: an **edge proxy** (`modules/edge.nix`, the piece that holds a
mesh together and makes boxes reachable) and **boxes** installed from the
installer ISO beside it. `run.sh` stands the whole thing up on one machine,
as QEMU VMs on a private virtual switch, and walks through what the boxes do
on their own, in the order of the verification checklist
([CHECKLIST.md](CHECKLIST.md)): **scenario A, no edge anywhere** — both boxes
search their network, find nothing, refuse to turn sharing on and say why,
keep serving their owners, and one of them is rebooted to show the state
holds; **scenario B, the edge is switched on** — both find it over mDNS
within a scan, the Mesh pane says so and sharing is allowed; the edge is
powered off and both notice; it returns and the gate reopens. The same
script is the demo and the rehearsal for a real site.

    demo/edge-lan/run.sh build          # or: build --iso path/to/losos.iso
    demo/edge-lan/run.sh up             # LAN, edge, install both boxes, boot, claim
    demo/edge-lan/run.sh walk           # scenario A (edge off), then B, over the API
    demo/edge-lan/run.sh edge off|on    # take the edge away, bring it back
    demo/edge-lan/run.sh down           # stop (keeps the disks); `clean` wipes

Open <http://127.0.0.1:8080/mesh> (box 1) and <http://127.0.0.1:8081/mesh>
(box 2) while it runs and sign in with the password `up` printed, to watch
the Mesh panes move. The host needs `nix` with flakes; QEMU, VDE and swtpm
come through `nix shell`, nothing runs as root, and the only host ports are
the forwards on 127.0.0.1. With `/dev/kvm` the whole thing takes minutes;
without it, budget about an hour per box for the install and another for its
first boot. `LOSOS_BOXES=1` runs it with one box. Requirements, knobs and the
state directory are listed at the top of `run.sh`.

## The recording

[`record.sh`](record.sh) makes the recording of the same walk from the
boxes' own admin UI, in about twenty minutes without KVM: it builds and runs
`checks.losos-edge-lan-two-boxes` ([`two-boxes.nix`](two-boxes.nix) — two
boxes with the real front nginx and admin SPA, one edge, the checklist's
order as assertions) with `LOSOS_RECORD_DIR` set, so the VM script pauses at
each step while [`record.mjs`](record.mjs) photographs the Mesh pane of both
boxes; [`compose.sh`](compose.sh) then turns the pictures and captions into a
slideshow video. The boxes there are the appliance's control plane rather
than full installs (the install itself is in the installer recordings), which
is what makes the run fit a laptop; the pictures are of the product. The
videos and pictures are project material, not repository content: they live
in the project's `demo/edge-lan/` folder and are linked from Markdown.

## Pooling storage with no internet

This is the answer to "can storage be expanded on the local network with no
edge proxy reachable": a box's **own** disk grows without any edge
(`losos-ctl grow`, a bigger drive, Local mode are never gated), but
**pooling** storage across boxes is the mesh, and the mesh's control plane
(rke2 server, Longhorn, the registrar) runs on the edge machine, so there is
nothing to pool into without one. For a site with no internet, the edge is
one always-on PC on the LAN running `nixosModules.edge` with
`losos.edge.lan.advertise = true`, exactly as `edge-vm.nix` does here: the
boxes find it over mDNS and pooling works with the WAN cable unplugged. That
is the decided shape (2026-10-07); a box acting as its own LAN's edge is not
planned.

## What the pieces are

| In the demo | On a company network |
| --- | --- |
| `vde_switch`, a unix socket | the office switch |
| `edge-vm.nix`: `nixosModules.edge` with `losos.edge.lan.advertise = true`, plus DHCP, DNS and NAT for the LAN on its second NIC | one always-on machine or VM running `nixosModules.edge` with `lan.advertise` on; the existing router keeps DHCP and DNS if it has them (the module does not need to be the router, the demo's is only so the LAN has one) |
| the boxes: the stock installer ISO, unattended, twice | the same ISO, written to a USB stick, one per mini-PC |
| `run.sh walk` over `/api/*` | the Mesh pane in a browser on the same LAN, [CHECKLIST.md](CHECKLIST.md) in hand |

Nothing on a box is demo-specific: it is the published `install`
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
- The demo edge carries no identity (`losos.edge.identity.*` unset), so the
  box shows it with a warning sign ("Not an official LosOS edge"): discovery
  and sharing work, the market stays off. That is also what a company's own edge
  looks like; only an edge with a certificate signed by the LosOS root key is
  official ([Master proxy — Official edges](../../wiki/Master-Proxy.md)).
- The VMs talk over slirp to the outside: the edge NATs the LAN to its
  own user-mode network, so a box without the edge really has no internet.
- The walk stops at the gate. `run.sh join N` performs a box's real join
  (the rebuild into mesh mode); the pooled-storage steps of the checklist
  (B3, B4) need the mesh wiring listed at its end, and cannot be shown
  before it lands.
