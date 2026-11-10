---
title: Autoprovision
sidebar_position: 5
mdx:
  format: md
---

# Autoprovision, including the netboot medium

Autoprovision is the LosOS installer for machines the mesh boots from the
network, plus the sync that keeps the modified box configuration the same on
every box of the local mesh. The three pieces:

* `losos-ctl provision` — `losos-ctl install` with one extra step in front:
  the machine takes the mesh's configuration from a *mirror*, so a box that
  provisions runs what the mesh runs, not the stock defaults.
* `nixosConfigurations.netboot` — the netboot flavor of the install medium:
  kernel, initrd and squashfs store for a PXE/iPXE client, whose autorun ends
  in `losos-ctl provision`.
* `losos.autoprovision.enable` — the box-side sync: lososd keeps this box's
  `modules/overrides.nix` in step with the same mirror, so a change applied
  on one box reaches the others.

## The mirror

One branch of one git URL, reachable without a prompt from every box of the
mesh (plain http(s), a bare repository on a file path, or an ssh address the
daemon opens with no key questions). Everything on it is shared within the
mesh and nothing on it is secret: LUKS keys and tokens never leave the box
they belong to.

```
refs/heads/provision
├── modules/overrides.nix                the mesh's modified configuration
└── boxes/
    ├── box-9f13c4ad/install-target.nix  a provisioned box's own hardware
    ├── box-9f13c4ad/overrides.nix       the configuration it was installed with
    └── mattbox/…
```

A machine netbooted to provision calls itself `box-<first 8 hex of the
machine id>` unless an operator asks for a name (`losos-autoprovision
--name lease-3`). Within a mesh box's LosOS Git this is one more private
repository clone: create a repository there for the mirror, or point at any
git remote the mesh reaches.

## Provisioning a netbooted machine

Boot the machine with the netboot image's `boot.ipxe`, or run the command by
hand on any installer medium:

```
nix build .#nixosConfigurations.netboot.config.system.build.netbootImage
# serve the result over http, point iPXE at boot.ipxe, and
# replace SERVER in it with the netboot server
```

What one provision does, in order:

1. Read the mirror (`git ls-remote`, then a shallow fetch of the `provision`
   branch) and take its `modules/overrides.nix`. A mirror with no branch, a
   branch with no configuration, or a configuration that does not pass the
   same shape check an Apply runs on the box stops here, before disks.
2. The firmware menu, unchanged from the stick: it waits out its 30 seconds
   and shows what it will do.
3. The normal installer plan, with one extra step: the mesh's
   `modules/overrides.nix` is laid into the flake clone next to the generated
   `modules/install-target.nix`, so the install builds the mesh's settings.
4. After the tree is on the disk, the box records itself on the mirror, as
   `boxes/<name>/` on a new commit over the branch's head (fast-forward only,
   so two machines provisioning at once refute each other instead of
   overwriting). A mirror that cannot take a record is a warning, never a
   failed install.

Anything the install flags accept, `provision` accepts: the disk-opening
choices (`--tpm`/`--no-tpm`, `--tang`/`--tang-edge`, `--bios`/`--uefi`, the
drive list) and `--no-install`. It refuses the test paths a prebuilt disko
script or `--emit-target` forms, because those do not build the box's tree.

The medium can prefill its mesh's own choices, so nobody types at the box:
`losos.autoprovision.tang` (or `LOSOS_TANG=URL#THUMBPRINT[,more]` in the
environment of the build) has the autorun install with `--tang`, the disk
key sealed to the TPM2 *and* the named Clevis Tang server (`tang-show-keys`
on that server prints the thumbprint). A box provisioned that way cannot
be opened from its disk alone, from the first boot; `--tpm`, `--tang-edge`
and every other typed flag still outranks the prefill.

## How the boxes stay in step

Enabling (`modules/autoprovision.nix`):

```nix
losos = {
  autoprovision = {
    enable = true;
    mirror = "https://git.home.example/mesh-config.git";
    # box = "mattbox";  # defaults to losos.hostName
  };
};
```

lososd's configuration reconciler — the same thread that keeps the
configuration repository on the box's own LosOS Git — then also looks at the
mirror every half minute, and decides with two facts: the box's text and the
mirror head's text, against what it last recorded of both. Both moved → the
newest writer wins (the mirror head's commit time against the moment this
box last pushed). Only one side moved → that side is the truth; the box
pushes or takes.

A take goes through the gates every Apply does — the shape check, the option
document, and the edge gate — and if anything is refused it stays refused
with the reason in the History pane's mesh line (`GET /api/config`, field
`mesh`). No box ever force-pushes. When the mirror has no branch yet, the
first box to check pushes its own configuration to seed it. And when a
provisioned box installs a configuration that later moves on, the next sync
pushes it back and every box rebuilds with it; that is precisely the point.

`losos-ctl config` — the History pane — carries the mirror's answer beside
the LosOS Git sync's and the published branch's (`mesh.sync.state`).

## The interplay

All three keepers of `/etc/nixos` run on one daemon thread and take the one
state lock, so they cannot write at the same time:

* LosOS Git — this *box's own* configuration, private (`losos.configRepo.
  enable`), covering every file it holds.
* the published branch — new LosOS versions ([Install in depth](install.md)).
* the mirror — the *mesh's* `modules/overrides.nix` only, machine facts
  never coming back from a `boxes/` record.

Publishing to a mesh mirror that is public to strangers is exactly as
deliberate as lending storage: `losos.autoprovision.enable` is off by
default, and taking a mesh's configuration means running what the mesh
commits.
