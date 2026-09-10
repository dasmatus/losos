# Compute mesh: k3s, Longhorn, and the end of Tahoe-LAFS

Date: 2026-09-09
Status: Design (pending implementation)

## Goal

Turn losos from a single appliance into a member of a distributed cluster,
without giving up the property that makes it an appliance: **it boots and
serves its own data with nothing else reachable.**

Three things change:

1. **k3s** replaces systemd-nspawn as the container runtime for Nextcloud and
   Forgejo (`losos.<svc>.mode == "container"` now means a Kubernetes workload).
2. **Rancher Longhorn** replaces Tahoe-LAFS as the distributed storage layer,
   and also backs the two services' volumes.
3. A **mesh toggle** (`losos.cluster.enable`) enrolls the box in a cluster whose
   control plane is the edge VPS, and a **compute window** lets it contribute
   CPU only while the owner is asleep.

## Decisions (approved)

| # | Decision |
|---|---|
| 1 | Kubernetes. The **edge VPS** (`nixosModules.edge`) is the mesh control plane. |
| 2 | `losos-registrar` issues mesh node tokens, gated on the existing closed-enrollment tenant whitelist. |
| 3 | `mode = "container"` becomes a k8s workload; the nspawn path is **deleted**. `mode = "native"` is unchanged. |
| 4 | Workloads ship as `pkgs.dockerTools` OCI images, preloaded via the `images` option of the relevant instance. |
| 5 | Full vertical slice: modules, options, images, the lososd settings contract, the settings SPA, a VM test, and the docs. |
| 6 | **Tahoe-LAFS is removed entirely.** Longhorn takes its place. |
| 7 | `/persist` becomes **ext4 with `-O encrypt`** (was btrfs), so fscrypt is available. |
| 8 | fscrypt is **defence in depth**, not the only layer — see the three-state table below. |
| 9 | The fscrypt protector key is **TPM2-sealed**, with a keyfile fallback for `losos.tpm.enable = false`. |
| 10 | Sharing storage unlocks the `shared` domain, namespaced per machine-id + username. |
| 11 | "Share my compute when I sleep" — a scheduled window, enforced by a taint. |
| 12 | **No installed base.** No migration path, no compatibility shims. |
| 13 | **Two clusters per appliance** (supersedes 1 for the box's own services). |
| 14 | Images come from **our own Nix binary cache**, not built on the appliance. |
| 15 | Verified findings from the first design pass are folded in (see Gotchas). |
| 16 | The mesh instance is **`services.rke2`**, not a hand-rolled unit — nixpkgs already instantiates the shared generator twice. |

## Architecture

### Why two clusters

Decision 1 alone does not survive contact with `modules/updates.nix`.

A k3s **agent**'s kubelet cannot start while its server is unreachable — it
fetches node config and certificates from the server on every start. This
appliance reboots unconditionally at 00:07 (`midnight-reboot.timer`, with
`Persistent = true` so it catches up if the box was off). Therefore any edge
outage spanning midnight would take Nextcloud and Forgejo down on **every**
joined appliance and keep them down for the rest of the outage — silently, on a
box with no shell, no SSH, and a dashboard that cannot be reached to diagnose
it.

So the appliance's own services must not depend on an off-box apiserver:

```
LOCAL CLUSTER                            MESH CLUSTER
services.k3s, role=server                services.rke2, role=server
on the appliance                         on the edge VPS
/var/lib/rancher/k3s                     appliance joins as services.rke2
--flannel-backend=none                   role=agent -> /var/lib/rancher/rke2
--disable-kube-proxy                     unit rke2-agent.service
hostNetwork pods only                    canal CNI + kube-proxy from server

  * Nextcloud   (this box's own)           * Longhorn (needs ONE cluster
  * Forgejo     (this box's own)             spanning boxes to replicate)
                                           * mesh compute workloads
boots with the edge DOWN
no off-box apiserver dependency          gated on losos.cluster.enable
```

The sleep-window taint lands only on the **mesh** node object. The appliance's
own pods live in a different cluster entirely, so the own-workload carve-out —
the hardest problem in the single-cluster design — becomes trivial.

### Running the second instance: `services.rke2`

`services.k3s` is a singleton and **has no `dataDir` option** (verified: zero
matches in both `nixos/modules/services/cluster/rancher/{default,k3s}.nix`).
`lib.mkForce` cannot help: it overrides the *value* of a declared option, and
neither obstacle is a value — there is no second instance to configure, and the
paths are let-bindings rather than options.

But nixpkgs generates both modules from one name-parameterized generator,
`mkRancherModule` (`default.nix:8`), where every path interpolates the name —
`/var/lib/rancher/${name}/...` — and `serviceName ? name`. The generator is
deliberately **not exported**: `default.nix:958-968` calls its two submodules by
hand ("*pass mkRancherModule explicitly instead of via `_modules.args` to
prevent infinite recursion*"), so it is reachable only from `k3s.nix` and
`rke2.nix`.

That is enough, because nixpkgs already instantiates it twice. **`services.rke2`
is the second instance**, and it is used as-is:

| | Local cluster | Mesh cluster |
|---|---|---|
| Module | `services.k3s` | `services.rke2` |
| Role | `server` (this box) | `agent` (joins the edge) |
| State | `/var/lib/rancher/k3s` | `/var/lib/rancher/rke2` |
| Unit | `k3s.service` | `rke2-agent.service` (`serviceName = "rke2-${cfg.role}"`) |
| Runs | this box's Nextcloud + Forgejo | Longhorn, mesh compute |

No vendoring, no hand-rolled unit, and the mesh side keeps the full option
surface (`images`, `manifests`, `charts`, the tmpfiles wiring). The edge runs
`services.rke2` with `role = "server"`.

Remaining conflicts between the two, and how each is resolved:

| Conflict | Resolution |
|---|---|
| Kubelet port (both want 10250) | set a distinct port on the rke2 agent via `extraKubeletConfig` (a real module option, rendered to `--kubelet-arg=config=<store path>`). |
| `/etc/cni/net.d` | the local k3s server runs `--flannel-backend=none` and installs **no** CNI config, so only rke2's canal writes here. |
| kube-proxy `KUBE-*` iptables chains | the local k3s server runs `--disable-kube-proxy`; its pods are all `hostNetwork` and need no Service routing. Only rke2 runs a kube-proxy. |
| `/etc/rancher/node/password` | fixed path, shared by both. Node names must differ, and the file must be persisted — see Gotchas. |
| containerd socket | **both want `/run/k3s/containerd/containerd.sock`.** The local k3s server is moved to a native containerd; rke2 keeps the embedded one. See below. |

rke2's own `role` description documents the same server-only-flag rule verified
for k3s: for an agent, "`agentToken`, `agentTokenFile`, `disable` and `cni`
should not be set". rke2 also needs port **9345** (the supervisor/registration
port) open on the edge, in addition to 6443.

**Coexistence needed one contingency, and it is now the design rather than a
fallback.** Both k3s and rke2 serve their embedded containerd on the *same*
hardcoded path, `/run/k3s/containerd/containerd.sock` — rke2 vendors the k3s
agent and inherits it. The evidence is nixpkgs' own
`nixos/tests/rancher/auto-deploy.nix:245`, which carries the comment "for some
reason, RKE2 also uses /run/k3s" and drives `crictl` at that socket; that file
takes `rancherDistro` as a parameter and `default.nix` instantiates it for both
distros, so it covers the rke2 case. Two embedded containerds cannot share one
socket path: whichever starts second unlinks and rebinds it, breaking the first.

So the **local k3s server runs against a native containerd**
(`virtualisation.containerd.enable`) with an explicit
`--container-runtime-endpoint`, and the mesh rke2 agent keeps its embedded one at
`/run/k3s`. The cost is that `services.k3s.images` no longer applies — that
option feeds the *embedded* containerd's airgap importer — so the workload images
are imported by a oneshot running `ctr` against the native socket, into the
`k8s.io` namespace the CRI reads. Getting that namespace wrong fails silently as
`ImagePullBackOff` under `imagePullPolicy: Never`, which is why the VM test
asserts the images landed rather than only that the pod started.

Never solve this the other way round by pointing the rke2 agent at the k3s
socket: two kubelets against one CRI garbage-collect each other's pods, which is
worse than one of them refusing to start.

The rest of the coexistence story — kubelet ports, `/etc/cni/net.d`, the
`KUBE-*` chains, the shared `/etc/rancher/node/password` — is settled by the
table above and asserted by `tests/cluster-vm.nix`. That test remains the gate;
it has never been run, because it needs KVM and roughly 10 GiB of guest RAM.

Cost: an rke2 agent is heavier than a k3s agent — roughly 400–500 MiB RSS
against 250 MiB — so budget ~1.5 GiB for Kubernetes on a box running both.

### Storage

Longhorn is not in nixpkgs, so it is deployed from the edge as a pinned Helm
chart via the edge's `services.rke2.autoDeployCharts` (a fixed-output
derivation, so it stays reproducible). It lives in the **mesh** cluster, because
cross-appliance replication requires one cluster spanning the boxes.

Longhorn on NixOS requires `services.openiscsi` (with a name), `nfs-utils`, and
the iscsiadm shim path Longhorn expects. Cross-LAN replication between
appliances behind NAT needs an encrypted overlay on the mesh cluster — with rke2
that is the `cni` option (canal with a WireGuard backend, or cilium with
WireGuard encryption) rather than k3s's `--flannel-backend` spelling — and will
be slow over WAN links — this is the one place where
Tahoe-LAFS was genuinely better suited to the topology than its replacement.

### The shared domain and fscrypt

`/persist` becomes ext4 with the `encrypt` feature. Verified: disko's filesystem
type carries an `extraArgs` option (`listOf str`) passed to `mkfs.<format>`, so
the incantation is `extraArgs = [ "-O" "encrypt" ]`.

fscrypt is layered **on top of** the existing LUKS, not instead of it:

| State | LUKS | fscrypt key | Result |
|---|---|---|---|
| Powered off | locked | unreachable | data opaque |
| Booted, sharing **off** | unlocked | not in the kernel keyring | opaque **even to root** |
| Booted, sharing **on** | unlocked | in the keyring | Longhorn and the pods can read it |

The protector key is TPM2-sealed, mirroring how `modules/disko.nix` already
branches on `losos.tpm.enable`; the no-TPM path uses a keyfile, or the whole
no-TPM configuration would brick. The package attribute is
**`fscrypt-experimental`**, not `fscrypt`.

Layout, namespaced per contributing box so a pooled volume stays attributable:

```
/home/shared/data/<machine-id>/<username>/    <- one fscrypt policy each
```

Losing btrfs costs `compress=zstd` and data checksums. Since the LVM pool is a
concatenation with no RAID, checksums were the only thing that *detected*
single-disk corruption; Longhorn replication now covers that at a different
layer. This must be recorded in the disko.nix header and the CLAUDE.md gotchas.

### Images and the binary cache

Nextcloud as an OCI image is roughly 2.6 GiB, and `system.autoUpgrade` runs
`nixos-rebuild switch` on the appliance at 03:00 — three hours before an
unconditional reboot, on a tmpfs root. Building that locally every nixpkgs bump
is not viable on a mini-PC.

Images therefore come from a Nix binary cache we control (Attic on the edge VPS,
which already runs Traefik and can front it; Cachix is the lower-effort
alternative). CI pushes; the appliance substitutes.

This **knowingly relaxes** the current invariant. The honest wording, which
must replace the existing claim in CLAUDE.md and the `modules/containers.nix`
header, is: *nothing is pulled from a container registry; images come from our
own Nix binary cache.* The behaviour when the cache is unreachable at 03:00 is
that the rebuild fails and the previous generation keeps running — acceptable,
but it must be a dashboard-visible condition.

## Option namespace

All new options go under `options.losos` in `modules/options.nix`; modules read
`config.losos.X`, never bare `config.X`.

Appliance:

- `cluster.enable` — join the mesh cluster (runtime-tunable from the SPA)
- `cluster.serverAddr`, `cluster.tokenFile`, `cluster.nodeName`
- `cluster.dataDir` (mesh instance), `cluster.kubeletPort`
- `cluster.shareCompute`, `cluster.computeWindow.{start,end}`
- `workloads.{pauseImage,nextcloudImage,forgejoImage}` (internal, wired by `defaults.nix`)
- `shared.fscrypt.{enable,keyFile}`

Edge:

- `edge.cluster.{enable,package,agentTokenFile,advertiseAddr,apiPort,flannelBackend}`
- `edge.tenants.<id>.cluster` — permits that tenant to fetch a node token

Wire names for the settings contract (camelCase, per `schema.json`):
`clusterEnable`, `shareCompute`, `computeWindowStart`, `computeWindowEnd`.

## Gotchas

These are **verified** against the pinned nixpkgs. Do not re-derive them.

- **`--disable`, `--flannel-backend` and `--disable-network-policy` are k3s
  server-only flags.** `k3s agent` hard-errors on them. Proof:
  `nixos/tests/rancher/multi-node.nix` gives both server nodes
  `disable = disabledComponents` (lines 98, 161) and gives the agent node
  (line 194) neither — only `--pause-image` and `--flannel-iface`. Gate every
  server-only flag on role.
- **`services.k3s.disableAgent` does exist** (`k3s.nix:108`, with an assertion
  at `k3s.nix:127` that `role == "agent" -> !disableAgent`). An earlier review
  claimed otherwise, having read only the shared `default.nix`.
- **`/etc/rancher` is not persisted.** `modules/impermanence.nix` lists
  `/etc/ssh`, `/etc/keys`, `/etc/nixos` — not `/etc/rancher`. The agent
  generates `/etc/rancher/node/password` on first join and the server stores its
  hash; on a tmpfs root it is regenerated every boot and the server rejects the
  rejoin. **Add `/etc/rancher` to the persistence list.**
- **systemd-tmpfiles runs at `sysinit.target`**, long before `multi-user.target`.
  The k3s module creates its airgap image symlinks via tmpfiles. Any guard unit
  that later `rm -rf`s `/var/lib/rancher/k3s` destroys them, and nothing
  recreates them until the next boot. Scope such wipes narrowly, or re-run
  `systemd-tmpfiles --create` afterwards.
- **`hostNetwork` pods share the host's addresses.** nginx sees them as
  `127.0.0.1` or the LAN IP, so a `deny <podCidr>` rule in the `lanOnly` guard
  matches nothing. The nspawn design relied on the container having a
  distinguishable source address; that depth is gone. The admin-surface guard
  must move off source-IP for the local cluster, or keep a distinguishable
  identity. Do not ship deny rules that cannot match.
- **`pkgs.nextcloud34` has no `phpPackage` passthru** (`passthru` is
  `{ tests; packages; }`). Build the PHP env explicitly with `php84.buildEnv`
  and the extension list `services.nextcloud` uses.
- **Every appliance ships `losos.hostName = "mattbox"`**, so node names collide
  on the second box to join. Default the mesh node name to
  `losos.proxy.applianceId` (already the unique registry key) and assert it is
  not the stock value when `cluster.enable` is true.
- **`losos-ctl factory-reset` and the reinstall ISO wipe `/persist`**, so the
  box regenerates its node password and the edge rejects it permanently
  ("duplicate hostname"). The registrar's join route must delete the stale node
  and its password secret for that tenant before returning a token.
- **Nothing creates `/var/secrets/losos-proxy-token`** — unlike the admin token
  (minted by lososd) and the Nextcloud adminpass, it is provisioned out of band.
  If the mesh join reuses it, flipping the cluster toggle on a box that never
  set up the master proxy must fail *visibly*, not by wedging k3s.
- **`losos.nextcloud.apachePort` is runtime-tunable from the SPA**, so it must
  not be baked into an OCI image. Render port-bearing config in module context
  and hostPath-mount it.
- **`modules/edge.nix`'s `tenantsJson` hardcodes its attribute set.** Any new
  per-tenant option must be added there too or it never reaches the registrar.
- **`losos-nextcloud-adminpass.service`** is ordered before `nextcloud-setup`
  and `phpfpm-nextcloud`, neither of which exists on the k8s path. Re-order it
  against whatever consumes it now.

### The compute window's timezone

The window is enforced by a NoSchedule taint, and only the edge may write it —
NodeRestriction refuses everyone else. So the comparison runs on the edge's
machine, which is a VPS on UTC, against hours the owner typed on an appliance
pinned to `Europe/Berlin`.

The zone travels with the window: `losos-mesh-join` sends `--window-tz` from
`config.time.timeZone`, the registrar stores it on `ComputeWindow.tz`
(`#[serde(default)]` to `UTC`, because a failed registry load is fatal at boot),
and the edge's taint script evaluates each node with `TZ="$tz" date` inside the
per-node loop.

This was got wrong first, and the wrongness was invisible: the edge read its own
clock while a comment asserted the two "agree by default (both UTC)", the SPA
told the owner "the appliance reads both times on its own clock", and the option
docs said the value was in `time.timeZone`. All three were false. A 23:00–07:00
window was enforced 00:00–08:00 in winter and 01:00–09:00 in summer.

## Testing

- `tests/cluster-vm.nix` (new) — an edge VM running the mesh k3s server and an
  appliance VM running both instances. Assert node registration and a running
  static pod via `crictl`, **not** the node `Ready` condition: with
  `--flannel-backend=none` the kubelet reports `NetworkReady=false` and the node
  is legitimately NotReady for the life of the cluster.
- `tests/front-vhost.nix` — every assertion coupled to the nspawn container IPs
  (`10.231.1.2`, `10.231.2.2`) must be rewritten.
- `tests/impermanence.nix` — extend to assert `/etc/rancher` survives a reboot,
  and that the shared domain is opaque when sharing is off.

## Out of scope

- Migrating an existing appliance. There is no installed base (decision 12).
- Rancher the management platform (Fleet, the multi-cluster UI). Only Longhorn
  and k3s are adopted.
- Idle-based compute sharing. The window is a fixed clock schedule; load-based
  detection is a possible refinement, noted but not built.
