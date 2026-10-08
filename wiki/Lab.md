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
- **Your own setups.** Start from Empty canvas or any built-in setup, drag
  devices up from the tray, and pull copper, fiber or Wi-Fi between their
  ports. **Save** downloads the setup as an `.llf` file (LosOS Lab file, JSON inside) and
  **Open** loads one back. The last setup you changed is also kept in the
  browser and listed as "Last setup" in the picker. An opened file is
  rebuilt with the same tools as the tray, so anything in it the tray could
  not make is left out, and the Lab says how many parts it dropped.
- **Built-in setups.** Two sites behind one official edge, a single home box,
  a company with its own gateway, a LAN with no internet, and three textbook
  shapes: star, bus and web. They are only starting points, built with the
  same tray.
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
| Consoles | simulated, or real guests under libvirt when the box runs the helper | real x86_64 guests under libvirt on your computer, else qemu-wasm |
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

## Guests under libvirt

qemu-wasm is slow: a guest takes a minute or more to boot in a tab, and a
tab runs three. If the computer the Lab is open on has libvirt (the thing
virt-manager drives), the Lab runs its guests there instead, under KVM when
the CPU has it. They boot in a few seconds, and up to eight run at once.

The bridge is a subcommand of the edge registrar, `losos-registrar lab`. It
talks to libvirt only through the `virsh` command, and starts each guest as
a transient domain named `losos-lab-...`. Transient means libvirt never
saves it, so nothing outlives the helper: Ctrl-C or SIGTERM destroys every
guest it started, a guest no Lab page has watched for a minute is destroyed
too, and on start it removes any `losos-lab-` domain a killed helper left
behind. A guest is on no libvirt network and no bridge. Its serial console
and its network card are connected to the helper on 127.0.0.1, and the
helper hands both to the page over WebSockets. The page is still the switch,
so a libvirt guest and a qemu-wasm guest can share a cable, and DHCP, ARP
and ping between them work as before.

To use it next to virt-manager on your own PC:

1. Install libvirt and QEMU (on most distributions, the packages virt-manager
   already pulled in) and the `losos-registrar` binary
   (`nix build .#losos-registrar`, or the static one the runbook in
   `provisioning/edge-identity/README.md` downloads).
2. Put the guest images in a folder named `guest`: `bzImage`, `rootfs.bin`
   and, for routers, switches and access points, `gear.bin`.
   `admin-ui/lab/engine/build.sh` makes all three, and the hosted Lab serves
   them under `/guest/`, so you can download them from there.
3. Start the helper in the folder above `guest`:

   ```sh
   losos-registrar lab --origin https://your-lab.example.org
   ```

   It listens on `127.0.0.1:8095` and uses `qemu:///session`, which needs no
   root. Pass `--connect qemu:///system` to have the guests show up beside
   your other VMs in virt-manager (your user must be in the `libvirt` group,
   and libvirt's own qemu user must be able to read the images folder).
   `--origin` names the address of the Lab page you open; the copy
   `serve.py` serves on `localhost:8080` is allowed without it. The other
   flags are `--images DIR`, `--max-guests N` (8), `--memory MiB` (96),
   `--virt-type auto|kvm|qemu`, `--idle 60s`, `--virsh PATH` and
   `--token-file FILE`. Without a token file the helper refuses to listen on
   anything but loopback, and answers only requests addressed to
   `127.0.0.1` or `localhost`.
4. Open the Lab. A copy on `localhost` looks for the helper by itself. A
   hosted copy looks only once you open it with `?libvirt` at the end of the
   address, because Chrome asks every visitor of a public page for local
   network access the moment it touches `127.0.0.1`. The Lab remembers the
   choice; `?libvirt=0` forgets it.

The badge in the top bar then says "KVM via libvirt" (or "QEMU via libvirt
(no KVM)" on a computer without it), and each console says what runs its
guest: libvirt, or "QEMU in this tab". If the helper is not running, or
libvirt refuses a guest, that guest boots under qemu-wasm as before, and the
Lab says so once. The box's own copy has no qemu-wasm, so there it keeps the
simulated console.

On a box, `losos.lab.libvirt.enable` (off by default) turns on libvirtd and
runs the same helper as a service. lososd relays the Lab's requests to it
under `/api/lab/` with the admin key, and the guests' consoles and network
cards go through nginx with a ticket only that key can get. Put the three
images in `/var/lib/losos-lab/images` (`losos.lab.libvirt.images`).

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
