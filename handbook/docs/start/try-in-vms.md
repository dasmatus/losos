---
title: Try it in VMs
sidebar_position: 6
---

# Try it in VMs

One line on a Linux computer sets up two LosOS boxes with an edge between
them, as virtual machines on a private network, and walks through what they
do together:

```sh
curl -fsSL https://raw.githubusercontent.com/dasmatus/losos/main/nix/demo/box-edge-box/bootstrap.sh | sh
```

Nothing has to be installed first except [Nix](https://nixos.org/download).
Without it, the script prints the line that installs it and stops. Nothing
needs root, nothing is installed outside `/nix` and the demo folder
(`~/.local/share/losos-demo`), and the VMs are reached only through
`127.0.0.1`.

## What the script does

It looks at the computer first: Linux on x86_64, KVM, processors, memory,
free disk and Nix. Then it writes a second script, `stage2.sh` in the demo
folder, with what it found at the top, and runs it. The second script
builds or fetches everything and starts the VMs. You can run it again later
on its own.

There are two sizes of demo, and the script picks the bigger one the
computer can carry:

| | Full | Light |
| --- | --- | --- |
| Needs | KVM, 14 GiB of memory, 60 GB of free disk | 6 GiB of memory, 15 GB of free disk |
| The boxes | installed from the installer ISO, with LosOS cloud | the admin pages and the parts that talk to the edge |
| The edge | an edge VM that is also the network's router | the [edge gateway](../in-depth/edge-federation.md) |
| Takes | about half an hour with KVM | about 15 minutes, KVM or not, after the first build |

`--full` or `--light` after `sh -s --` picks the size yourself, for example
`... | sh -s -- --light`.

## What you see

1. Two boxes and no edge. Each looks for one and finds nothing. Asked to
   share its disk, each refuses, because there is no edge to share through.
2. The edge is switched on. Both boxes find it on the network by themselves.
3. The edge publishes both boxes at their public names, one tunnel each.
4. Box A opens box B by its public name through the edge, and box B opens
   box A. The admin pages are refused on that road. They stay on the local
   network.
5. Both owners let the edge manage their box, and the edge's own page lists
   both.
6. With the edge there, both boxes accept sharing their disk.
7. The edge is switched off. Both boxes notice, close their tunnels and
   refuse sharing again, and each keeps serving its own network.

To show it to someone step by step, add `--pause`. The light demo then waits
at each step and prints where to open each box's Mesh page and the edge's
page in a browser.

## How it differs from a real setup

- The edge has no certificate, because a private network has no public name
  to prove. A real edge with a public name gets one from Let's Encrypt.
- The boxes' public names are pointed at the edge by the demo. On a real
  network, DNS does that.
- The edge is a company edge, not an official one, so the market stays off.
- In the light demo LosOS cloud is not running. A small placeholder answers
  in its place, behind the box's real web server.
