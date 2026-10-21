# Box, edge, box

Two LosOS boxes and an edge between them, as VMs on one private network, set
up and walked through by one line:

    curl -fsSL https://raw.githubusercontent.com/dasmatus/losos/main/nix/demo/box-edge-box/bootstrap.sh | sh

The script is served straight from this repository on GitHub, so the line
always runs what is on `main`. Put a tag in place of `main` to run a release.
Options go after `sh -s --`, for example `| sh -s -- --light --pause --host-mdns`.

## The two stages

[`bootstrap.sh`](bootstrap.sh) is stage one. It reads the machine (Linux on
x86_64, KVM, processors, memory, free disk, Nix), picks the size of demo the
machine can carry, pins the LosOS flake to the revision `main` points at,
and writes stage two to `~/.local/share/losos-demo/stage2.sh` with those
choices at its top. Then it runs it. Stage two can be run again on its own
(`bash stage2.sh`, `build`, `demo`, `down`, `clean`), and its first lines can
be edited.

Nix is the only thing the machine needs. QEMU, swtpm, VDE and every image
come through it. Nothing runs as root, and the only ports the demo opens are
forwards on `127.0.0.1`. The exception is `--host-mdns`, below, which makes one
network device with `sudo`. Without Nix the script prints the installer line
and stops.

| | full | light |
| --- | --- | --- |
| Picked when | KVM, 14 GiB of memory and 60 GB of disk | anything else with 6 GiB and 15 GB |
| The boxes | installed from the installer ISO, unattended, with LosOS cloud | the control plane, the admin pages and the edge path, as test VMs |
| The edge | `nixosConfigurations.edge-demo` ([../edge-lan/edge-vm.nix](../edge-lan/edge-vm.nix)) | the edge gateway, as `losos-disk-edge-qcow2` runs it |
| What runs it | [../edge-lan/run.sh](../edge-lan/run.sh) `up`, `walk`, `publish` | [demo.nix](demo.nix), `checks.losos-box-edge-box` |
| Time | about half an hour with KVM | about 15 minutes without KVM, after the first build |

## What the demo shows

1. No edge anywhere: each box finds the other by mDNS, by its `.local` name
   and its `_losos._tcp` advert, over the wire only. Each looks for an edge,
   finds none, and refuses to share its disk. The other box cannot reach it
   by its public name.
2. The edge is switched on: both boxes find it on the network by mDNS.
3. The edge enrols each box the first time it calls and publishes it at
   `<box>.losos.cfd`, down one tunnel per box.
4. Box A reaches box B by B's public name through the edge, and back. The
   admin pages answer 403 on that road. They stay on the LAN.
5. Both owners let the edge manage their box, and the edge's page lists both
   (light demo).
6. With an edge in reach both boxes accept sharing their disk.
7. The edge is switched off: both notice, stop their tunnels and refuse
   sharing again, and keep serving their own network.

The full demo runs the [verification checklist](../edge-lan/CHECKLIST.md)
first, then names the boxes `boxa` and `boxb`, turns their edge path on with
an Apply (a rebuild on each box), and asks for each box's LosOS cloud through
the edge. In the full demo the computer running it asks, standing in for any
computer on the network.

## What differs from a real site

- The edge's certificate: a network with no public DNS cannot get one, so
  Traefik presents its own and the demo accepts it (`curl -k`). A site with
  a public name gets Let's Encrypt.
- The public names point at the edge by `curl --resolve`, where a site's DNS
  would point them.
- The edge carries no identity, so it is a company edge: sharing works
  through it, the market does not.
- In the light demo LosOS cloud does not run. A one-file stand-in answers on
  the loopback port the cloud would, behind the box's real front nginx.
- Boxes in the light demo send a heartbeat every 5 seconds instead of 30, so
  the demo does not wait on it.
- The VMs' `.local` names live on their own network. `--host-mdns` makes
  one tap device, `vde-tap77` (the only step that needs `sudo`), at
  `192.168.77.254` (light) or `10.77.0.254` (full) on that network, so the
  host's own responder (Avahi or systemd-resolved, which stage one detects)
  hears the VMs and a browser opens `http://boxa.local/`. The light demo's
  test driver plugs a tap of that name into its vlan 77 itself; the full
  demo runs `vde_plug2tap` into `run.sh`'s switch. `stage2.sh
  host-mdns-off` removes it.
- `--pause` stops the light demo at each step and prints where to open each
  box's Mesh page and the edge's page, so it can be shown live.
