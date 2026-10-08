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
| Built by | `nix build .#losos-admin-ui` (the Lab is its second page) | `admin-ui/lab/engine/build.sh` in `lab.yml` |
| Consoles | simulated, or real guests under libvirt when the box runs the helper | real x86_64 guests under libvirt on your computer, else qemu-wasm |
| Needs | nothing beyond the box | a cross-origin isolated host (COOP + COEP) |

The Lab is part of the admin UI: `admin-ui/app/lab/index.html` is a second
Vite page beside the admin pages, built from the same components, palette,
icons and languages, and served at `/lab/`. Everything that is not drawing
runs in WebAssembly: the model, the simulator, the clock, the consoles and
the pages are a Rust crate, `admin-ui/lab/core` (`losos-lab-core`), which
nix builds for wasm32 before the admin UI's own build. The page runs under
a content security policy of its own: the admin page's, plus
`'wasm-unsafe-eval'` so the browser may compile the core.

The hosted copy is the same page built with `VITE_LAB_HOSTED=1`, plus
[qemu-wasm](https://github.com/ktock/qemu-wasm): QEMU compiled to
WebAssembly, so every console is a real Linux guest in the browser tab.
Boxes and edges boot a busybox stand-in for LosOS, and the network gear
boots Netzgeräte Betriebssystem, built by nix from
`admin-ui/lab/engine/gear/netzgeraete.nix`. The lab joins the guests'
network cards into the diagram, so DHCP, ARP, ping and HTTP between guests
travel the cables you drew. A router guest hands out leases with its own
`udhcpd`. A tab runs up to three guests at once. With a fourth, the guests
ran out of cores and stalled, so the lab refuses to boot one. qemu-wasm
runs QEMU's threads as Web Workers that share memory, and browsers allow
that only on a page served with `Cross-Origin-Opener-Policy: same-origin`
and `Cross-Origin-Embedder-Policy: require-corp`. `vercel.json` sets both
headers on Vercel, and `serve.json` does the same for `serve` on your own
machine:

```sh
admin-ui/lab/engine/build.sh /tmp/lab     # docker, nix, node
npx --yes serve@14.2.4 -l 8080 /tmp/lab   # then open http://localhost:8080/lab/
```

`lab.yml` builds the hosted copy for every pull request that touches
`admin-ui/lab/` or the Lab's page in `admin-ui/app/` and uploads it as the `losos-lab-hosted` artifact. On a push
to `main` it also deploys the copy to Vercel, if the repository has the
`VERCEL_TOKEN`, `VERCEL_ORG_ID` and `VERCEL_PROJECT_ID` secrets.

## Guests under libvirt

qemu-wasm is slow: it emulates every instruction with no KVM, each guest
costs the tab about 450 MB, and a tab runs three. If the computer the Lab is open on has libvirt (the thing
virt-manager drives), the Lab can run its guests there instead, under KVM
when the CPU has it. They boot in a few seconds, and up to eight run at once.

There are three ways a guest can run, in order of preference:

1. **The helper drives libvirt through `virsh`.** The helper is a
   subcommand of the edge registrar, `losos-registrar lab`, running on the
   same computer as libvirt. It needs libvirt, QEMU and the `virsh` command
   there, and the guest images in a folder the helper can read.
2. **The page drives libvirt itself, through the helper's relay.** The
   helper also offers a WebSocket that it copies, byte for byte, to
   libvirt's own socket. A page that carries the WebAssembly libvirt client
   (`admin-ui/lab/virt-rpc`, the package `losos-lab-virt`) speaks libvirt's
   protocol over it, so this path needs the helper and a running libvirt
   daemon but no `virsh` and no libvirt client library on the computer.
3. **qemu-wasm in the tab.** Nothing on the computer at all, only the
   hosted Lab's engine. This is the slow path, and the fallback for any
   guest the first two cannot start.

### The helper and `virsh`

The helper starts each guest as a transient domain named `losos-lab-...`
with `virsh create`. Transient means libvirt never saves it, so nothing
outlives the helper: Ctrl-C or SIGTERM destroys every guest it started, a
guest no Lab page has watched for a minute is destroyed too, and on start
it removes any `losos-lab-` domain a killed helper left behind. A guest is
on no libvirt network and no bridge. Its serial console and its network
card are connected to the helper on 127.0.0.1, and the helper hands both to
the page over WebSockets. The page is still the switch, so a libvirt guest
and a qemu-wasm guest can share a cable, and DHCP, ARP and ping between
them work as before.

To use it next to virt-manager on your own PC:

1. Install libvirt and QEMU (on most distributions, the packages virt-manager
   already pulled in) and the `losos-registrar` binary
   (`nix build .#losos-registrar`, or the static one the runbook in
   `provisioning/edge-identity/README.md` downloads).
2. Put the guest images in a folder named `guest`: `bzImage`, `rootfs.bin`
   and, for routers, switches and access points, `gear.bin`.
   `admin-ui/lab/engine/build.sh` makes all three, and the hosted Lab serves
   them under `/lab/guest/`, so you can download them from there.
3. Start the helper in the folder above `guest`:

   ```sh
   losos-registrar lab --origin https://your-lab.example.org
   ```

   It listens on `127.0.0.1:8095` and uses `qemu:///session`, which needs no
   root. Pass `--connect qemu:///system` to have the guests show up beside
   your other VMs in virt-manager (your user must be in the `libvirt` group,
   and libvirt's own qemu user must be able to read the images folder).
   `--origin` names the address of the Lab page you open; a copy served on
   `localhost:8080`, as above, is allowed without it. The other
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

### The WebAssembly client and the relay

A web page cannot open libvirt's socket: browsers offer no raw TCP and no
unix sockets to a normal page, and libvirt has no WebSocket listener of its
own. So the helper relays. The page asks `POST /lab/v1/virt-ticket` for a
ticket, then opens the WebSocket `/lab/v1/virt` with the subprotocols
`losos-lab` and `ticket.<ticket>`. The ticket is good once, for 30 seconds.
The helper connects to the socket of its `--connect` URI and copies bytes
both ways without reading them. For `qemu:///session` that socket is
`$XDG_RUNTIME_DIR/libvirt/virtqemud-sock`, or `libvirt-sock` beside it for a
monolithic libvirtd. For `qemu:///system` it is
`/run/libvirt/virtqemud-sock`, then `/run/libvirt/libvirt-sock`. A
`?socket=PATH` in the URI names the socket directly, as it does for
`virsh`. The helper keeps at most `--max-guests` relayed sockets open at
once.

`GET /lab/v1/hello` reports this path on its own, as
`virt: {available, socket}`. `available` is true when the socket accepts a
connection right now, so it can be true while `virsh` is missing. The
helper does not start a session daemon the way `virsh` does, so with
`qemu:///session` the daemon must already be running or started by its
systemd socket.

The client creates its guests with libvirt's autodestroy flag, so they end
when the connection does: when the tab closes, and when the helper stops,
because the helper closes every relayed socket on Ctrl-C or SIGTERM. The
helper does not count, sweep or destroy these guests; they belong to the
page. To try the path, `admin-ui/lab/virt-rpc/README.md` shows how to open
its demo page through the helper.

### Security

Whoever holds a relayed socket has libvirt with the helper's rights.
libvirt identifies a socket's peer by its user id, so it sees the helper and
never the page, and polkit asks about the helper too. With
`qemu:///system` that is as good as root on the computer, because a page can
create a domain that attaches any host file or disk, and the relay cannot
refuse such a domain without parsing libvirt's protocol. Prefer
`qemu:///session` on your own PC: then the worst case is your own user's
files. The Lab's guests need nothing from the system instance.

The helper therefore admits the relay socket only with a ticket, and a
ticket costs the same as starting a guest: the bearer token when the helper
has `--token-file`, and otherwise a request addressed to `127.0.0.1` or
`localhost` from an allowed page. The `--origin` list matters most here.
A WebSocket is not bound by CORS, so any site open in the same browser can
try to reach `ws://127.0.0.1:8095`, and the helper refuses every request
whose Origin is not on the list or the helper's own address.

### On a box

`losos.lab.libvirt.enable` (off by default) turns on libvirtd and runs the
same helper as a service, with `qemu:///system`. lososd relays the Lab's
requests to it under `/api/lab/` with the admin key, including
`POST /api/lab/virt-ticket`. The guests' consoles and network cards
(`/api/lab/ws/`) and the libvirt relay (`/api/lab/virt`) go through nginx
straight to the helper, from the local network only, and each needs a
ticket only that key can get. The relay hands out libvirt as the helper's
user, which is in the `libvirtd` group, so it is root-equivalent on the box;
the admin key can already rebuild the whole system, so it gives away nothing
the key did not have. Put the three images in `/var/lib/losos-lab/images`
(`losos.lab.libvirt.images`).

## What is not real

The guests are stand-ins: busybox and a 4 MiB image, not NixOS. The edges,
the tunnel, the mesh and the market are modelled in the Rust core from the
rules above; they are not running code. The laptop's desktop is a picture of
LosOS Desktop, not the desktop itself.

## Changing it

- The page and its parts are in `admin-ui/app/src/lab/`. `npm run lab:core`
  builds the core into `src/lab/core-pkg/` (gitignored; it needs cargo with
  the `wasm32-unknown-unknown` target and `wasm-bindgen` 0.2.127), and then
  `npm run dev` serves the page at `/lab/`. `tests/lab.browser.mjs` checks
  it under the Lab's own policy.
- The core's contract is `admin-ui/lab/core/README.md`. Its tests compare
  every built-in setup against the behaviour of the JavaScript Lab it
  replaced.
- Guests: `admin-ui/app/src/lab/engine/` tries libvirt through the helper,
  then qemu-wasm, in that order.
