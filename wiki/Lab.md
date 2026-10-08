# Lab

LosOS Lab draws a LosOS setup as a network diagram and shows the traffic
that runs through it. Every box serves it at `/lab/`, under **Lab** in the
admin sidebar. Like the rest of the admin pages, it answers only on the local
network.

It opens on **This box**. The lab reads the box's settings and the edges it
found, using the same three admin routes as the other pages, and draws the
box from them: its router, the edges in reach, the official edge when one is
configured, and a laptop. The router, the cables and the laptop are assumed,
because the box cannot see them. The lab never writes to the box. If nobody
is signed in, it says so and opens the built-in setups.

## What it shows

- **Logical and Physical views.** The logical view shows subnets, the edge
  paths and the mesh. The physical view shows racks, desks and the cables
  between them.
- **Realtime and Simulation modes.** Realtime plays traffic as it happens. It
  has a clock you can speed up (1×, 10×, 60×, 10 min/s, 1 h/s) and a button
  that skips to the next timer: the 00:07 reboot, the 03:00 upgrade, the
  04:30 garbage collection, and the start and end of the compute window.
  Simulation holds every packet until you press Play or Step, and lists each
  hop with its protocol.
- **Built-in setups.** Two sites behind one official edge, a single home box,
  a company with its own gateway, a LAN with no internet, and three textbook
  shapes: star, bus and web.
- **A laptop running LosOS Desktop.** It has the sign-in screen, the
  overview, and the browser you can point at any box or edge in the diagram.
- **Network gear** (routers, switches, Wi-Fi access points and a coax bus)
  runs a small system of its own called Netzgeräte Betriebssystem. Its
  console has `show interfaces`, `show ip route`, `show arp` and
  `show dhcp`.

The diagram follows the same rules as the real software. Examples: a box
with no edge in reach starts no tunnel; a mesh switch is refused with
`edgeRequired`; the market opens only next to an official edge (one whose
certificate the LosOS root key signed); a LAN gateway with open enrolment
takes unknown boxes on first use; and the admin routes refuse loopback.

## The two copies

| | On the box (`/lab/`) | Hosted |
| --- | --- | --- |
| Built by | `nix build .#losos-lab` | `admin-ui/lab/engine/build.sh` in `lab.yml` |
| Consoles | simulated | real x86_64 guests under qemu-wasm |
| Needs | nothing beyond the box | a cross-origin isolated host (COOP + COEP) |

The box's copy is four files: `index.html`, `lab.js`, `lab.css` and the
plate. `build.py` assembles them with nothing but python3, so nix needs no
npm for it. It runs under a content security policy of its own, which allows
inline style attributes and scripts from the box only.

The hosted copy adds [qemu-wasm](https://github.com/ktock/qemu-wasm): QEMU
compiled to WebAssembly, so every console is a real Linux guest in the
browser tab. Boxes and edges boot a busybox stand-in for LosOS, and the
network gear boots Netzgeräte Betriebssystem, built by nix from
`admin-ui/lab/engine/gear/netzgeraete.nix`. The lab joins the guests'
network cards into the diagram, so DHCP, ARP, ping and HTTP between guests
travel the cables you drew. A router guest hands out leases with its own
`udhcpd`. A tab runs up to three guests at once. With a fourth, the
guests ran out of cores and stalled, so the lab refuses to boot one. qemu-wasm runs QEMU's threads as Web Workers that share memory,
and browsers allow that only on a page served with
`Cross-Origin-Opener-Policy: same-origin` and
`Cross-Origin-Embedder-Policy: require-corp`. `vercel.json` sets both
headers. `serve.py` does the same on your own machine:

```sh
admin-ui/lab/engine/build.sh /tmp/lab     # docker, nix, node, python3
cp admin-ui/lab/serve.py /tmp/lab/ && python3 /tmp/lab/serve.py
```

`lab.yml` builds the hosted copy for every pull request that touches
`admin-ui/lab/` and uploads it as the `losos-lab-hosted` artifact. On a push
to `main` it also deploys the copy to Vercel, if the repository has the
`VERCEL_TOKEN`, `VERCEL_ORG_ID` and `VERCEL_PROJECT_ID` secrets.

## What is not real

The guests are stand-ins: busybox and a 4 MiB image, not NixOS. Booting the
real appliance under TCG in a browser would take far too long. The edges,
the tunnel, the mesh and the market are modelled in JavaScript from the
rules above; they are not running code. The laptop's desktop is a picture of
LosOS Desktop, not the desktop itself.

## Changing it

The sources are plain JavaScript in `admin-ui/lab/src/`, concatenated in the
order `build.py` lists. Run `python3 admin-ui/lab/build.py box /tmp/lab` and
open the files through any static server. `build.py single FILE` writes one
self-contained page.
