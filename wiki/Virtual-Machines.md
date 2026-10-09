**English** · [Slovenčina](Virtual-Machines-sk) · [Deutsch](Virtual-Machines-de)

# Virtual machines

A box that shares its disk with the mesh can rent virtual machines on the
mesh, and can host other boxes' machines. A machine is sold by the
**replica**: each replica is one copy of the machine with its own disk,
running for a month on the box that hosts it. Payment goes through the
[market](Market), and every sale is split **half and half** between the
owner of the hosting box and LosOS.

The hypervisor is [KubeVirt](https://kubevirt.io), with
[CDI](https://github.com/kubevirt/containerized-data-importer) importing the
disks, both on the edge's mesh cluster. The edge's `losos-registrar` sells the
replicas and creates the machines; the admin UI's **Machines** page (under
**Mesh** in the sidebar, at `/machines`) is where an owner does all of it.

## Who can use it

Only a box that shares its disk (`losos.sharingMyStorage`) is offered
machines, to rent or to host. lososd checks it on every request: a box that
does not share gets `{available: false, reason: "notSharing"}` from
`GET /api/vms`, every action is refused with 409, and the page shows one
greyed sentence instead of the form.

The disk-sharing switch is on the Market pane, and the market is not open yet
(see [Market](Market)). Until it opens, the admin UI cannot switch sharing on,
so the Machines page stays greyed on every box that has not set
`losos.sharingMyStorage` some other way.

Two more reasons the page can give:

| Reason           | Meaning                                                     |
| ---------------- | ----------------------------------------------------------- |
| `noOfficialEdge` | No edge in reach is one LosOS runs, so nothing can be sold. |
| `notOffered`     | The edge does not run machines (`losos.edge.vms.enable`).   |

## What you can start

| Source            | What it is                                                      |
| ----------------- | --------------------------------------------------------------- |
| LosOS             | The LosOS demo disk, pulled as a KubeVirt containerDisk          |
| Cloud images      | Systems Quickemu supports that publish a ready-to-boot cloud disk |
| Your own QCOW2    | An image you upload from the Machines page                      |

The catalogue is `backend-registrar/vm-images.json`, built into the
registrar; an operator can replace it with `--vm-catalogue`. It lists Ubuntu
Server 26.04 and 24.04, Debian 13 and 12, Fedora 43 Cloud, AlmaLinux 10, Rocky
Linux 10, CentOS Stream 10, Arch Linux, openSUSE Tumbleweed, Alpine Linux and
FreeBSD, each from its project's own download address.

**The LosOS image is the demo disk, not the appliance.** It is the same
`losos-disk-qcow2` the release publishes, with no disk encryption and no
stateless root (see `flake/disk-images.nix`). It boots with UEFI and asks for
4 GiB of memory, since it runs two clusters and LosOS cloud. The release job
pushes it as `ghcr.io/<owner>/<repo>-demo-containerdisk:<tag>` and moves its
`stable` tag, which the catalogue names.

**Most of Quickemu's list is not offered.** Those systems ship installer ISOs
only, and installing one needs a console in the browser, which the page does
not have yet. The page lists them under "systems not offered yet", together
with the few left out for another reason (macOS, whose licence allows only
Apple hardware; Windows, which needs a licence and a console; and a few whose
cloud images have no stable address or come in an archive CDI cannot read).

A cloud image reads **cloud-init**, so the order form takes an optional
`#cloud-config` document or script (up to 16 KiB) that the machine runs on its
first start, for example to install an SSH key. The registrar never echoes it
back, since it may hold a password.

### Your own QCOW2

The Machines page uploads a QCOW2 file straight to the edge, with a progress
bar. lososd streams it through without buffering it and without holding its
lock, and the edge checks it as it arrives:

- it must start with the QCOW2 header, or the edge answers 415;
- it must fit the edge's limit (`losos.edge.vms.uploadMaxGiB`, 32 GiB by
  default), or 413, and its virtual disk may be at most 512 GiB;
- it must keep moving, since a minute without a byte drops it.

A box may keep three images. Each is stored on the edge under
`/var/lib/losos-registrar/vm-images/`, mode 0600, and each replica that starts
from it imports it from a single-purpose address that only the buyer's own
machines are given. A replica of an image gets a disk at least as large as the
image's virtual size. Tick **Boots with UEFI** for an image that needs UEFI
firmware; Secure Boot is off either way.

## Replicas and the price

A hosting box lists machines priced **per replica-month**, with the number of
replicas it is willing to run (at most 50). A buyer picks a system, a name, a
host's offer and a number of replicas (at most 10, and no more than the host
has free), and pays on Stripe's page in a new tab.

Every replica on one edge has the same size: `losos.edge.vms.cpu` vCPU,
`memoryMiB` of memory and `diskGiB` of disk (1, 2048 and 20 by default), or
more where an image asks for more.

**The split is fixed at 50%.** The machine checkout is a destination charge
like every other market sale, with `application_fee_amount` exactly half of
the total. The operator's `losos.edge.market.feeBps` does not apply to
machines, and the Stripe gate refuses a machine checkout with any other fee.
At 6.00 EUR per replica-month, three replicas cost 18.00 EUR; the hosting
box's owner receives 9.00 and LosOS keeps 9.00.

## After payment

Once Stripe confirms the payment, the registrar's reconciler:

1. creates the buyer's namespace `market-<buyer id>` on the mesh, if needed;
2. creates one `VirtualMachine` per replica, named `vm-<order>-0`, `-1` and so
   on, pinned to the hosting box by its `losos.dev/appliance` node label, each
   with its own disk imported by CDI into the `losos-vm-local` storage class;
3. creates one `Service` in front of the replicas, on port 80.

With `losos.edge.vms.domain` set, the edge's Traefik serves that Service at
`https://vm-<order>.<domain>`, spread over the replicas, with a certificate
from Let's Encrypt. Without it the machines have no public address.

The Machines page shows each machine with the state of every replica
(running, starting, copying its disk, paused, stopped or failed) and its
address, and asks again every 15 seconds while a replica is still on its way
up.

**When the month runs out** every replica is halted (`runStrategy: Halted`)
and its place goes back to the host's listing. The disks stay. They hold the
buyer's data, and removing them is the operator's decision, as it is for a
storage order's volume.

## Hosting machines

A box hosts machines while it is enrolled in the mesh, shares its disk, and
has `losos.vms.host` on (the default). It then joins with `--host-vms true`,
and the edge lets it list machines. On such a box the `kvm-intel`, `kvm-amd`,
`tun` and `vhost_net` kernel modules are loaded, and the replicas' disks live
under `losos.edge.vms.hostPath` (`/home/shared/vms` by default), in the shared
user's home and so inside the shared data domain.

Machines run around the clock. KubeVirt's node agent and CDI's importers
tolerate the compute-window taint, because a machine bought for a month cannot
stop each morning. Offer only as many replicas as the box can carry next to
its own work.

### Nested virtualisation

KubeVirt needs `/dev/kvm` on the hosting box. On a real mini-PC that is the
processor's VT-x or AMD-V. A box that is itself a virtual machine needs
**nested virtualisation** from its host (`kvm_intel nested=1` or
`kvm_amd nested=1`, and `-cpu host` in QEMU). Without it the operator can set
`losos.edge.vms.useEmulation`, which runs the replicas in software. They boot,
but slowly, and it is meant for trying the feature out, not for selling it.

## Operator setup

On the edge:

```nix
losos.edge = {
  market.enable = true;   # machines are sold on the market
  cluster.enable = true;  # and run on the mesh
  vms = {
    enable = true;
    domain = "vms.example.net"; # optional: https://vm-<order>.vms.example.net
    # cpu = 1; memoryMiB = 2048; diskGiB = 20; uploadMaxGiB = 32;
    # useEmulation = true;      # only without /dev/kvm on the hosts
  };
};
```

`modules/edge-vms.nix` deploys KubeVirt v1.9.0 and CDI v1.66.1 from their
pinned release manifests through rke2's manifests directory, and a
local-path provisioner (rancher/local-path-provisioner v0.0.37) under LosOS's
own names for the `losos-vm-local` storage class, which binds a disk only once
its machine has a node and never deletes one. The registrar gets permission to
read, create and patch `VirtualMachine`s and to create `Service`s.

For `domain`, point a wildcard DNS record (`*.vms.example.net`) at the edge.
Traefik's read timeout on the public entry point is raised to six hours while
machines are on, so a large upload is not cut off.

## API

On the box (Bearer-authed, behind the LAN-only guard):

| Route                              | Does                                           |
| ---------------------------------- | ---------------------------------------------- |
| `GET /api/vms`                     | Catalogue, offers, account, machine states     |
| `POST /api/vms/orders`             | `{listing_id, quantity, image, name, user_data?}` |
| `POST /api/vms/listings`           | `{unit_price, capacity}`, per replica-month    |
| `POST /api/vms/listings/close`     | `{listing_id}`                                 |
| `PUT /api/vms/images?name=&efi=1`  | Upload a QCOW2 (the body is the file)          |
| `POST /api/vms/images/remove`      | `{upload_id}`                                  |

On the edge, beside the rest of `/market/*`: `GET /market/vm-images` (public,
like the shelf), `POST /market/vms/status`, `POST /market/vm-images/ticket`,
`POST /market/vm-images/remove`, and the two transfer routes
`PUT /market/vm-images/upload/<ticket>` and
`GET /market/vm-images/fetch/<token>`, which take capabilities rather than a
tenant token and are limited to four transfers at a time.

## Safety properties

- A box that does not share its disk can neither rent nor host; lososd
  refuses before the edge is asked.
- The edge accepts machine listings only from a box enrolled in the mesh that
  says it hosts machines.
- The fee on a machine sale is half, checked again by the Stripe gate.
- An uploaded image is reachable only through a 64-character capability that
  is written only into the buyer's own disks; the upload ticket is single-use
  and lapses after an hour.
- The upload ticket travels to curl in a temporary file, not on its command
  line.
- Nothing the registrar does deletes a buyer's disk.

## Not covered

- No console in the browser, so no installer ISOs and no way to log in except
  what cloud-init sets up.
- No live migration: a replica runs on the box it was sold on.
- No snapshots, no resizing a running machine, no choice of size per order.
- The VM tests do not boot a KubeVirt machine; the registrar's tests run the
  order, upload and fulfilment flow against a fake apiserver and Stripe.
