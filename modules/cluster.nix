# The appliance's two Kubernetes instances, and the mesh join that feeds one
# of them.
#
#   LOCAL  services.k3s, role = server, state in /var/lib/rancher/k3s, unit
#          k3s.service. Runs *this box's* Nextcloud and Forgejo as static pods
#          (modules/workloads.nix) whenever losos.<svc>.mode == "container".
#          It has no off-box dependency of any kind.
#   MESH   services.rke2, role = agent, state in /var/lib/rancher/rke2, unit
#          rke2-agent.service. Joins the edge VPS's cluster for Longhorn and
#          mesh compute. Gated on losos.cluster.enable, off by default.
#
# They are two clusters on purpose, and collapsing them into one is the
# tempting mistake this file exists to prevent. An agent's kubelet cannot start
# while its server is unreachable — it fetches node config and certificates
# from the server on every start — and modules/updates.nix reboots this box
# unconditionally at 00:07 with Persistent = true. Put the box's own services
# in the edge's cluster and any edge outage spanning midnight takes Nextcloud
# and Forgejo down on every joined appliance, and keeps them down for the rest
# of the outage, on a box with no SSH and no shell to notice it from.
#
# The mesh instance is rke2 rather than a second k3s because nixpkgs builds
# both from one name-parameterized generator (mkRancherModule in
# nixos/modules/services/cluster/rancher/default.nix): every path interpolates
# the name, so /var/lib/rancher/{k3s,rke2} and k3s.service / rke2-agent.service
# do not collide. services.k3s is a singleton with no dataDir option, so a
# second k3s is not expressible at all — lib.mkForce cannot help, because the
# obstacle is a missing instance, not a value.
#
# What still overlaps between the two instances, and how each is settled:
#
#   kubelet port    Both default to 10250, and both default their healthz to
#                   10248. The mesh agent is moved to losos.cluster.kubeletPort
#                   — 10260 — and its healthz to 10268, below. 10260 is the
#                   number modules/containers.nix names in the closed-port set
#                   (6443/9345/10250/10260); if that list and this one ever
#                   disagree again, this file is not the authority — the option
#                   default in modules/options.nix is.
#   /etc/cni/net.d  The local server runs --flannel-backend=none and installs
#                   no CNI config at all, so only rke2's canal writes there.
#   KUBE-* chains   The local server runs --disable-kube-proxy; its pods are
#                   all hostNetwork and need no Service routing. Only the mesh
#                   agent runs a kube-proxy.
#   node password   /etc/rancher/node/password is a fixed path shared by both
#                   instances — it is not name-parameterized. That is fine on
#                   its own: the two servers are different clusters and each
#                   hashes the same password under its own node name. What is
#                   not fine is a duplicate node NAME (asserted below) or a
#                   regenerated password, which is why modules/impermanence.nix
#                   persists /etc/rancher. On a tmpfs root without it the agent
#                   invents a new password every boot and the server refuses
#                   the rejoin.
#   containerd      SETTLED — and it is the one overlap that had to change the
#                   shape of this file rather than just a number in it. Both
#                   instances serve their EMBEDDED containerd on the same
#                   hardcoded path, /run/k3s/containerd/containerd.sock: rke2
#                   vendors the k3s agent code and inherits the literal. The
#                   evidence is nixpkgs' own
#                   nixos/tests/rancher/auto-deploy.nix, which carries the
#                   comment "for some reason, RKE2 also uses /run/k3s" and
#                   drives crictl at that socket — and that file takes
#                   rancherDistro as a parameter while default.nix instantiates
#                   it for BOTH distros, so it is an rke2 statement and not
#                   only a k3s one. Two embedded containerds cannot share one
#                   socket: whichever starts second unlinks and rebinds the
#                   path, and the first is left advertising a socket nothing
#                   answers on.
#
#                   So the LOCAL server does not run one. It gets a NATIVE
#                   containerd (virtualisation.containerd.enable, socket
#                   /run/containerd/containerd.sock) and is pointed at it with
#                   --container-runtime-endpoint, which is what makes k3s skip
#                   starting its embedded containerd at all and never create
#                   /run/k3s. The MESH agent keeps its embedded one, so
#                   /run/k3s belongs to rke2 alone and the collision cannot
#                   happen. nixpkgs documents this exact arrangement in
#                   pkgs/applications/networking/cluster/k3s/docs/examples/EXTERNAL_CONTAINERD.md,
#                   down to the socket URI and the ctr invocation below.
#
#                   Two things stop working the moment k3s stops running its
#                   own containerd, and both are paid for further down:
#                     * services.k3s.images feeds the EMBEDDED containerd's
#                       airgap importer, so it becomes a no-op. The workload
#                       images are imported by losos-workload-images.service
#                       instead — `ctr` against the native socket, into the
#                       k8s.io namespace.
#                     * --pause-image only ever reached the embedded
#                       containerd's config template (and kubelet's old
#                       --pod-infra-container-image, which Kubernetes removed),
#                       so it too becomes a no-op. The sandbox image is set on
#                       the native containerd directly, as sandbox_image.
#
#                   Do NOT "fix" the collision the other way round by pointing
#                   the rke2 agent at the k3s socket, or by giving both
#                   instances the one native containerd: two kubelets against
#                   one CRI garbage-collect each other's pods — each sees the
#                   other's sandboxes as orphans it is responsible for
#                   reaping — which is worse than one of them not starting.
#                   tests/cluster-vm.nix is the gate that proves the two really
#                   are separate; run it before trusting this file.
#
# Nothing in this module ever wipes /var/lib/rancher. systemd-tmpfiles runs at
# sysinit.target and creates the MESH instance's airgap image symlinks under
# /var/lib/rancher/rke2/agent/images exactly once per boot; any later unit that
# rm -rf's the tree leaves the agent with no images to import, and nothing
# recreates them until the next reboot. If a state-wiping guard is ever
# genuinely needed, scope it below agent/images or re-run
# `systemd-tmpfiles --create` after it. (No guard is needed today:
# `losos-ctl factory-reset` only rewrites overrides.nix and state.json, and a
# reinstall wipes /persist — which takes the whole of /var with it, image
# symlinks included, in one consistent step.) The LOCAL cluster no longer has
# an agent/images directory to protect: with a native containerd its images
# live in containerd's own content store under /var/lib/containerd, which the
# same whole-of-/var persistence covers.
#
# The mesh join (losos-mesh-join.service) fetches the rke2 node token from the
# edge registrar, authenticating with the appliance's existing master-proxy
# token. It is also the SOLE producer of the edge's compute window:
# --share-compute and the two window bounds are passed by joinArgs below and by
# nothing else on the box — modules/proxy.nix's announce body carries no window
# fields at all. So whenever this unit does not run, the settings SPA's "share
# my compute when I sleep" toggle and both HH:MM fields do not reach the edge.
# They still validate in the browser, still validate again in lososd, still
# land in modules/overrides.nix and still cost a full rebuild; they simply
# change nothing. That is not hypothetical: the unit used to carry
# `ConditionFileNotEmpty = !<token file>`, which ran it on exactly one boot per
# enrolment and never again, so the toggle was a no-op from the day it shipped.
# The unit is keyed on its rendered ARGUMENTS now, and the decision is made
# inside the script rather than by a Condition — the distinction is load-bearing
# and is explained at the unit.
#
# rke2-agent.service REQUIRES it. That is a reversal of what this header used to
# claim ("deliberately advisory, wants + before, never requires") and it makes
# the module agree with backend-registrar/src/join.rs, which has documented a
# Requires= relationship since it was written — a contract and an
# implementation that disagree in writing are an invitation to flip one of them
# back. The argument is recorded at the unit so it is not re-litigated.
#
# The assertions below still carry the eval-time half of the visible-failure
# duty: nothing on the appliance creates /var/secrets/losos-proxy-token (unlike
# the admin token, which lososd mints, and the Nextcloud adminpass, which
# nextcloud-common generates), so flipping the mesh toggle on a box that never
# set up the master proxy fails the rebuild instead of failing at 03:00 on a
# box nobody can log into.
#
# See docs/superpowers/specs/2026-09-09-k3s-mesh-design.md; the workloads that
# run in the local cluster live in modules/workloads.nix, and the edge side of
# the mesh in modules/edge.nix.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.losos.cluster;

  # The LOCAL cluster exists to run this box's own workloads, so it follows the
  # two mode switches and nothing else. losos.forgejo.enable is consulted here
  # as well as the mode, exactly as modules/containers.nix does — enable = false
  # must not leave a git host running.
  localEnabled =
    config.losos.nextcloud.mode == "container"
    || (config.losos.forgejo.mode == "container" && config.losos.forgejo.enable);

  meshEnabled = cfg.enable;

  registrar = config.losos.proxy.registrar.package;
  applianceId = config.losos.proxy.applianceId;

  # A literal, never derived from losos.hostName: every appliance ships the
  # stock "mattbox", and this name is only ever seen by this box's own
  # single-node cluster, where a stable value is worth more than a unique one.
  # The MESH node name is the one that has to be unique (asserted below).
  localNodeName = "losos-local";

  # The local cluster's static-pod directory, and deliberately NOT k3s's own
  # /var/lib/rancher/k3s/agent/pod-manifests: a k3s data-dir wipe must not be
  # able to take the appliance's own workloads with it, and this way the path
  # is a losos-owned name that modules/workloads.nix can pin its symlinks to.
  # That module repeats the literal, and has to: it is not a losos.* option, and
  # options.nix is closed for this change. Change one, change the other.
  #
  # The flag is --kubelet-arg, not services.k3s.extraKubeletConfig.staticPodPath:
  # k3s always passes its own --pod-manifest-path, a command-line flag beats the
  # same setting in a KubeletConfiguration file, and the config-file spelling
  # would therefore be ignored without a word. --kubelet-arg entries override
  # k3s's own argsMap, so the flag wins.
  staticPodDir = "/var/lib/losos/k3s-static-pods";

  pauseImage = config.losos.workloads.pauseImage;

  # dockerTools derivations wired by modules/defaults.nix from flake/images.nix.
  # Filtered rather than assumed non-null so a stripped-down evaluation (a VM
  # test that only wants the cluster, a config that sets one of them to null)
  # still builds. Only the pause image gets an assertion, because without it no
  # pod on the box gets a sandbox at all.
  workloadImages = lib.filter (img: img != null) [
    pauseImage
    config.losos.workloads.nextcloudImage
    config.losos.workloads.forgejoImage
  ];

  # The same helper modules/workloads.nix uses to spell an image in a pod's
  # `image:` field. It has to agree with this file to the byte, because the pods
  # run with imagePullPolicy: Never — a ref the kubelet asks for and containerd
  # does not hold is not an error anyone sees, it is ImagePullBackOff forever.
  imageRef = img: "${img.imageName}:${img.imageTag}";

  # The native containerd's socket: the stock path for
  # virtualisation.containerd (its unit gets RuntimeDirectory = "containerd"),
  # and the one nixpkgs' own EXTERNAL_CONTAINERD.md example hands to k3s. It is
  # written out here rather than left implicit because the whole point of this
  # arrangement is that the LOCAL cluster's socket is NOT the one under
  # /run/k3s that the mesh agent owns.
  localCriSocket = "/run/containerd/containerd.sock";

  rke2Pkg = cfg.rke2.package;

  # The rke2 airgap bundles are per-architecture fetchurl derivations folded
  # into the package's passthru from images-versions.json, so the attribute
  # name carries the arch. Selecting it rather than hardcoding amd64 keeps a
  # future aarch64 board from failing inside a passthru lookup, which reads
  # like a nixpkgs bug rather than the port task it would actually be.
  rke2ImageArch = if pkgs.stdenv.hostPlatform.isAarch64 then "arm64" else "amd64";
  rke2Images = [
    rke2Pkg."images-core-linux-${rke2ImageArch}-tar-zst"
    rke2Pkg."images-canal-linux-${rke2ImageArch}-tar-zst"
  ];

  # Same rule the SPA's regex and lososd's valid_hhmm enforce, applied a third
  # time at eval: overrides.nix is a file, and a file can be edited by hand or
  # restored from an older generation without ever passing through
  # POST /api/apply. builtins.match anchors the whole string.
  validWindow = s: builtins.match "([01][0-9]|2[0-3]):[0-5][0-9]" s != null;

  meshTokenDir = builtins.dirOf (toString cfg.tokenFile);

  # --token-file and --out-token-file are paths read at runtime, not secrets,
  # so they are safe on a command line that lands in the world-readable store —
  # which this one does twice over, in the unit's script and in joinArgsFile
  # below. Same shape as modules/proxy.nix's announceArgs.
  joinArgs = lib.concatStringsSep " " [
    "join"
    "--registrar-url"
    (lib.escapeShellArg config.losos.proxy.registrarUrl)
    "--appliance-id"
    (lib.escapeShellArg applianceId)
    "--node-name"
    (lib.escapeShellArg cfg.nodeName)
    "--token-file"
    (lib.escapeShellArg (toString config.losos.proxy.tokenFile))
    "--out-token-file"
    (lib.escapeShellArg (toString cfg.tokenFile))
    "--share-compute"
    (if cfg.shareCompute then "true" else "false")
    # The zone the two bounds below are wall-clock times IN. It has to travel
    # with them: the edge writes the NoSchedule taint (it is the only node the
    # NodeRestriction admission plugin lets write it) and therefore compares the
    # window against the EDGE's clock. An edge VPS runs UTC and this appliance
    # ships Europe/Berlin, so before the zone was sent a 23:00-07:00 window
    # entered by the owner was enforced 00:00-08:00 in winter and 01:00-09:00 in
    # summer, sliding an hour at each DST change — handing strangers' pods the
    # first hours of that owner's working day, which is the exact thing this
    # feature exists to prevent.
    #
    # config.time.timeZone rather than the losos namespace on purpose: this must
    # be the zone the box's own clock actually uses, and that is the value the
    # rest of NixOS reads.
    "--window-tz"
    (lib.escapeShellArg config.time.timeZone)
    "--window-start"
    (lib.escapeShellArg cfg.computeWindow.start)
    "--window-end"
    (lib.escapeShellArg cfg.computeWindow.end)
    # The edge answers with the supervisor endpoint it believes in. Handing it
    # ours lets the client warn when the two disagree, which is what an
    # operator who moved losos.edge.cluster.advertiseAddr without updating
    # losos.cluster.serverAddr gets instead of a box that silently joins
    # nothing.
    "--expect-server-addr"
    (lib.escapeShellArg cfg.serverAddr)
  ];

  # The rendered arguments as a file, so the unit can compare what it is about
  # to send against what it last sent. Every byte of it is already in the
  # world-readable store (the credentials are paths, read at run time), so this
  # is a copy, not a leak.
  joinArgsFile = pkgs.writeText "losos-mesh-join-args" joinArgs;

  # The stamp: the arguments the last SUCCESSFUL join sent. It sits beside the
  # mesh token on purpose. The pair has to be destroyed by the same events —
  # a reinstall wipes /persist and takes both — because the two halves fail in
  # opposite directions: a stamp that outlives its token is a box holding a
  # "nothing to do" marker and no credential, and a token that outlives its
  # stamp costs one redundant re-join. The script tests the token first, so
  # only the second, cheap direction is even reachable.
  joinStamp = "${meshTokenDir}/losos-mesh-join.args";
in
{
  config = lib.mkMerge [

    # ── LOCAL cluster: this box's own workloads ─────────────────────────────
    (lib.mkIf localEnabled {
      assertions = [
        {
          assertion = pauseImage != null;
          message = ''
            losos.workloads.pauseImage is null but Nextcloud or Forgejo is in
            "container" mode. Without it the native containerd keeps its own
            default sandbox image (a registry.k8s.io/pause reference), which
            this box neither preloads nor can fetch — every pod would fail to
            create a sandbox, and the failure surfaces as pods that never
            start rather than as anything naming the pause image. It is wired
            by modules/defaults.nix from flake/images.nix.
          '';
        }
      ];

      # ── The LOCAL cluster's container runtime ────────────────────────────
      # Native, not k3s's embedded one, because the mesh agent's embedded
      # containerd holds /run/k3s/containerd/containerd.sock and two of them
      # cannot share it. See the containerd row in this file's header for the
      # evidence and for why the reverse arrangement is worse.
      virtualisation.containerd = {
        enable = true;

        # Everything below is spelled under "io.containerd.grpc.v1.cri"
        # because the NixOS module pins `version = 2` in the config it
        # generates, and containerd 2.x migrates that plugin's keys forward on
        # load. Writing the 3.x spellings (io.containerd.cri.v1.images and
        # friends) into a version-2 config would be ignored — silently, since
        # an unknown key is not an error. If the module ever moves to version
        # 3, these keys move with it, and nothing will say so: the symptom is
        # pods that never get a sandbox.
        settings.plugins."io.containerd.grpc.v1.cri" = {
          # An empty, unwritable store path, and deliberately NOT the default
          # /etc/cni/net.d. That directory belongs to the MESH agent's canal
          # (see the /etc/cni/net.d row in the header): pointed at it, this
          # cluster's containerd would load the mesh's CNI config on any box
          # that has joined, and the two instances would quietly share a pod
          # network. Every pod here is hostNetwork, so CNI is never invoked for
          # them and finding nothing is the intended outcome — the kubelet
          # reporting NetworkReady=false is this cluster's documented steady
          # state, not a fault.
          cni.conf_dir = "${pkgs.emptyDirectory}";
        }
        # k3s's --pause-image is inert once k3s is no longer the thing writing
        # containerd's config, so the sandbox image has to be set here or
        # containerd keeps its compiled-in registry.k8s.io/pause default and
        # tries to fetch it from a registry this appliance cannot reach.
        # optionalAttrs rather than lib.mkIf: the assertion above is what turns
        # a null pause image into a readable eval failure, and this must not
        # evaluate imageRef on the way there.
        // lib.optionalAttrs (pauseImage != null) {
          sandbox_image = imageRef pauseImage;
        };
      };

      # ── Preloading the workload images ──────────────────────────────────
      # services.k3s.images is not usable here: it symlinks tarballs into
      # /var/lib/rancher/k3s/agent/images, which only the EMBEDDED containerd's
      # airgap importer ever reads, and there is no embedded containerd on this
      # box. So the same tarballs are imported by hand.
      #
      # The namespace is the whole trick. containerd namespaces are hard
      # partitions of the image store, and the CRI plugin — hence the kubelet,
      # hence these pods — only ever looks in "k8s.io". Import into the default
      # "default" namespace and `ctr images ls` shows the image sitting right
      # there while every pod stays in ImagePullBackOff, because
      # imagePullPolicy: Never gives the kubelet no way to go and find it.
      # There is no error message anywhere that names the namespace.
      #
      # Ordering and gating, deliberately:
      #   after/requires containerd.service   ctr is a client; without the
      #                                       daemon it has nothing to talk to.
      #   before k3s.service                  the kubelet must not reach a pod
      #                                       sandbox before the pause image
      #                                       exists.
      # k3s Requires this unit (below) rather than merely Wants it, on the same
      # argument the rke2-agent block makes at length: a missing image is
      # otherwise an invisible failure — no crash loop, no failed unit, just
      # pods that never start — whereas a failed dependency names the cause
      # once, in order, in the journal of a box with no shell.
      #
      # No Restart=: every input is a local store path, so a failure here is
      # deterministic and retrying it changes nothing.
      #
      # The store paths are baked into the script, so adding, dropping or
      # rebuilding an image changes the unit file and switch-to-configuration
      # restarts it on the next `nixos-rebuild switch` — the same mechanism
      # losos-mesh-join relies on, and the reason there is no ConditionX here
      # either.
      systemd.services.losos-workload-images = {
        description = "losos workload images — import the local cluster's OCI images into containerd";
        wantedBy = [ "multi-user.target" ];
        after = [ "containerd.service" ];
        requires = [ "containerd.service" ];
        before = [ "k3s.service" ];

        path = [
          pkgs.containerd # ctr
          pkgs.gnugrep
        ];

        # The loop body, once, so the reasons live here rather than three times
        # over in the rendered script:
        #
        #   grep -Fxq   the ref is matched whole and literally. A substring
        #               match would let losos.local/nextcloud:<old tag> satisfy
        #               the test for a new one, and the box would go on running
        #               the previous image with nothing to suggest why. This is
        #               also what makes the unit idempotent and cheap: on every
        #               boot after the first, all three refs are already there
        #               and nothing is decompressed.
        #   .tar.zst    no decompression flag is needed. containerd sniffs the
        #               zstd magic in DetectCompression exactly as it sniffs
        #               gzip; flake/images.nix chose zstd because the mini-PC
        #               is the machine doing the decompressing.
        #   import      unpacks into the snapshotter unless --no-unpack is
        #               passed. That default is load-bearing, not incidental:
        #               an image present in the content store but never
        #               unpacked fails at container create, long after the step
        #               that would have explained it.
        script = ''
          set -eu

          ${lib.concatMapStrings (img: ''
            if ctr --address ${localCriSocket} --namespace k8s.io images ls -q \
              | grep -Fxq ${lib.escapeShellArg (imageRef img)}; then
              echo "workload images: ${imageRef img} already imported"
            else
              echo "workload images: importing ${imageRef img}"
              ctr --address ${localCriSocket} --namespace k8s.io images import ${img}
            fi
          '') workloadImages}
        '';

        serviceConfig = {
          Type = "oneshot";
          RemainAfterExit = true;
        };
      };

      services.k3s = {
        enable = true;
        role = "server";
        package = cfg.k3s.package;
        nodeName = localNodeName;

        # Every one of these is a *server-only* flag. That is fine here — this
        # is a server — but the same list handed to an agent is a hard error on
        # an unknown flag and a unit that crash-loops forever on a box with no
        # shell. Do not copy this block to the mesh agent below.
        #
        # traefik/servicelb: nginx is the single front door and holds the only
        #   public port; a second ingress and a LoadBalancer implementation
        #   would both want to bind one.
        # metrics-server: nothing consumes it and it is not free on a mini-PC.
        # local-storage: the workloads hostPath-mount their state directly out
        #   of the persisted /var, so there is no PVC to provision.
        # coredns: without a CNI it would sit Pending for the life of the
        #   cluster. The workload pods use dnsPolicy: Default and resolve
        #   through the host's /etc/resolv.conf instead.
        disable = [
          "traefik"
          "servicelb"
          "metrics-server"
          "local-storage"
          "coredns"
        ];

        # `images` is deliberately NOT set. The generator would symlink the
        # tarballs into /var/lib/rancher/k3s/agent/images, which only the
        # embedded containerd's airgap importer reads — and there is no
        # embedded containerd here. losos-workload-images.service above does
        # the job instead. The images themselves are still substituted from
        # this project's own Nix binary cache; a 2.6 GiB Nextcloud image cannot
        # be rebuilt at 03:00 on a mini-PC with a tmpfs root, and nothing is
        # ever fetched from a container registry either way.

        extraFlags = [
          # The flag that makes all of the above true: k3s only starts its own
          # containerd when this is empty. With it set, k3s never creates
          # /run/k3s, and the mesh agent's embedded containerd owns that path
          # alone.
          "--container-runtime-endpoint=unix://${localCriSocket}"
          # No CNI, no pod network, no kube-proxy: every pod in this cluster is
          # hostNetwork, so there is nothing to route and nothing to overlay.
          # The cost is that the kubelet reports NetworkReady=false and the node
          # is legitimately NotReady for the life of the cluster — which is why
          # the workloads are static pods, created by the kubelet straight off
          # disk with no scheduler in the path. Anything converted to a
          # Deployment later must tolerate node.kubernetes.io/not-ready
          # (NoSchedule *and* NoExecute) or it will sit Pending forever.
          "--flannel-backend=none"
          "--disable-network-policy"
          "--disable-kube-proxy"
          "--disable-cloud-controller"
          "--disable-helm-controller"
          "--kubelet-arg=pod-manifest-path=${staticPodDir}"
        ];

        # --pause-image is deliberately absent. It is still a valid k3s flag,
        # which is exactly why leaving it here would be worse than useless: it
        # would read as the thing that selects the sandbox image while doing
        # nothing at all. k3s only ever used it to template the EMBEDDED
        # containerd's config.toml and to fill kubelet's
        # --pod-infra-container-image, and Kubernetes removed that kubelet flag.
        # The sandbox image is set on the native containerd above.
      };

      # The runtime this cluster depends on, and the images it depends on
      # having been imported. Requires, not Wants, for both — see the comment
      # on losos-workload-images above for the argument, which is the same one
      # the rke2-agent block makes further down.
      #
      # The known cost, so it is not rediscovered as a bug: Requires propagates
      # STOP, so `nixos-rebuild switch` restarting either of these bounces
      # k3s.service with them. For containerd that is unavoidable and correct —
      # the CRI went away underneath the kubelet. For the image import it means
      # a changed workload image restarts the cluster as well as the pod. That
      # is rare (an image changes only when this flake's own image derivation
      # does) and the alternative, Wants, buys a quiet ImagePullBackOff instead.
      systemd.services.k3s = {
        after = [
          "containerd.service"
          "losos-workload-images.service"
        ];
        requires = [
          "containerd.service"
          "losos-workload-images.service"
        ];
      };

      # Two things this module does NOT declare, on purpose:
      #
      #   * the static-pod directory itself. modules/workloads.nix creates it
      #     with the same tmpfiles pass that symlinks the manifests into it, on
      #     the same condition. A second rule for one path only earns a
      #     "Duplicate line for path" from systemd-tmpfiles on every boot.
      #   * k3s.service's ordering against postgresql, redis-nextcloud,
      #     losos-nextcloud-adminpass and losos-fscrypt-shared. Those exist
      #     because the pods hostPath-mount those services' sockets and files,
      #     so they belong to the module that writes the mounts, and it carries
      #     them (scoped per workload, which matters: a Forgejo-only box must
      #     not Require a Nextcloud secret). The containerd and image-import
      #     ordering just above is this file's, because the runtime is this
      #     file's choice; the module system merges the two `after` lists.
      #
      # Both are load-bearing, so this file and modules/workloads.nix have to
      # land together — which the shared image refs already force anyway.
    })

    # ── MESH cluster: the edge's rke2 agent ─────────────────────────────────
    (lib.mkIf meshEnabled {
      assertions = [
        {
          assertion = registrar != null;
          message = ''
            losos.cluster.enable needs losos.proxy.registrar.package (wired by
            modules/defaults.nix): losos-mesh-join runs the registrar binary's
            `join` subcommand to fetch this box's rke2 node token.
          '';
        }
        {
          # The visible failure the spec asks for. The join authenticates with
          # /var/secrets/losos-proxy-token, which is provisioned out of band as
          # part of the master-proxy setup and by nothing else on the box — so a
          # mesh toggle flipped on a proxy-less appliance has no credential to
          # present, and would otherwise show up as a join unit failing and an
          # agent crash-looping on a token file that never appears.
          assertion = config.losos.proxy.enable;
          message = ''
            losos.cluster.enable requires losos.proxy.enable. The mesh join
            authenticates to the edge registrar with the appliance's existing
            per-appliance secret (losos.proxy.tokenFile, default
            /var/secrets/losos-proxy-token), which is provisioned out of band
            with the master proxy — nothing on the appliance creates it. Set up
            the master proxy first, or leave the mesh toggle off.
          '';
        }
        {
          assertion = cfg.nodeName != "" && cfg.nodeName != "mattbox";
          message = ''
            losos.cluster.nodeName must not be empty or the stock "mattbox":
            every appliance ships losos.hostName = "mattbox", so the second box
            to join collides and the edge rejects it permanently as a duplicate.
            Set losos.proxy.applianceId (or losos.cluster.nodeName) to something
            unique.
          '';
        }
        {
          assertion = cfg.serverAddr != "";
          message = ''
            losos.cluster.enable needs losos.cluster.serverAddr — the edge's
            rke2 supervisor endpoint, e.g. https://edge.losos.cfd:9345. Note the
            port: agents register on 9345, not on the apiserver's 6443.
          '';
        }
        {
          assertion = validWindow cfg.computeWindow.start && validWindow cfg.computeWindow.end;
          message = ''
            losos.cluster.computeWindow.start and .end must be HH:MM,
            00:00-23:59 (got "${cfg.computeWindow.start}" and
            "${cfg.computeWindow.end}"). The registrar rejects a malformed
            window with a 400, so catching it here is the difference between a
            failed rebuild and a box that joins without the window it was
            configured for.
          '';
        }
      ];

      services.rke2 = {
        enable = true;
        role = "agent";
        package = rke2Pkg;
        inherit (cfg) serverAddr tokenFile nodeName;

        # The edge's reconciler and the compute-window taint address this box by
        # its appliance id; the label makes that id selectable from inside the
        # cluster without parsing node names.
        nodeLabel = [ "losos.dev/appliance=${applianceId}" ];

        # The one genuine collision between the two instances. A non-empty
        # extraKubeletConfig makes the generator emit
        # --kubelet-arg=config=<store path>, so all three land in one
        # KubeletConfiguration file.
        #   port         the local k3s kubelet holds the default 10250, so the
        #                mesh kubelet takes losos.cluster.kubeletPort (10260).
        #   healthzPort  it holds the default 10248 too. 10268 is a literal
        #                because there is no option for it and options.nix is
        #                closed for this change. It is deliberately NOT derived
        #                from the port above and is not at the same offset
        #                (10248 + 10 would be 10258): it is picked to sit clear
        #                of the whole 10248-10259 block Kubernetes' own
        #                components claim — 10249 kube-proxy metrics, 10256
        #                kube-proxy healthz, 10257 controller-manager, 10259
        #                scheduler — so a listener there is unambiguously this
        #                one. Do not "restore the offset" by moving it to
        #                10258; that buys a tidy story and claims a port
        #                between two reserved ones.
        #   readOnlyPort 0 — the deprecated unauthenticated kubelet port has no
        #                business existing on an appliance.
        extraKubeletConfig = {
          port = cfg.kubeletPort;
          healthzPort = 10268;
          readOnlyPort = 0;
        };

        # Preloaded so a joining box brings up canal and the rke2 core
        # components without pulling from a registry.
        images = rke2Images;

        # Deliberately NOT set, because rke2's own `role` description says an
        # agent must not: disable, cni, agentToken, agentTokenFile. Nor
        # manifests or charts — the generator warns that an agent ignores them,
        # and they belong to the edge server (modules/edge.nix).
      };

      # Requires, not Wants — settled here so it stops being re-litigated.
      #
      # With Wants, a box whose join failed (no proxy token, edge unreachable,
      # or a 403 because losos.edge.tenants.<id>.cluster was never set) starts
      # the agent anyway. nixpkgs' rancher generator gives every agent
      # Restart = "always" with RestartSec = "5s" and no condition on the token
      # file, so what Wants actually produces is rke2-agent crash-looping every
      # five seconds against a token that is not there, forever — the EFFECT
      # printed a thousand times over while the single line naming the CAUSE
      # scrolls out of the journal. Two failed units, and the loud one is the
      # wrong one.
      #
      # With Requires the failure stays attributable: losos-mesh-join is the
      # one failed unit, its journal line carries the status the edge actually
      # returned, and the agent is discarded with "Dependency failed for
      # rke2-agent.service" — cause and effect, in that order, once. On a box
      # with no shell, where the whole diagnosis is somebody reading a journal
      # after the fact, that is the more visible of the two failures. It is
      # also what backend-registrar/src/join.rs already documents, and what its
      # bounded five-minute retry budget is sized for: the join gives up so the
      # agent's job resolves inside one boot instead of hanging in `activating`.
      #
      # The known cost, stated so nobody rediscovers it as a bug: a job
      # discarded on a failed dependency is NOT retried when the oneshot later
      # succeeds. losos-mesh-join's own Restart = "on-failure" keeps trying and
      # will eventually write the token, but nothing starts the agent in that
      # boot. midnight-reboot.timer bounds that at one night — the box reboots
      # unconditionally at 00:07 and joins on the way up. Do not close the gap
      # by having the join unit `systemctl start` the agent from ExecStartPost;
      # if it ever needs closing properly the systemd primitive is Upholds= on
      # the join unit, and that wants testing against a real agent first.
      systemd.services.rke2-agent = {
        after = [
          "losos-mesh-join.service"
          "iscsid.service"
        ];
        requires = [ "losos-mesh-join.service" ];
      };

      systemd.services.losos-mesh-join = {
        description = "losos mesh join — fetch this box's rke2 node token from the edge registrar";
        wantedBy = [ "multi-user.target" ];
        after = [ "network-online.target" ];
        wants = [ "network-online.target" ];
        before = [ "rke2-agent.service" ];

        # WHY THERE IS NO ConditionX ON THIS UNIT, AND WHY THAT IS THE FIX.
        #
        # This unit has to re-run when the compute window changes, because
        # joinArgs is the only thing that ever carries the window to the edge.
        # What re-runs it is `nixos-rebuild switch`: the arguments are part of
        # the unit file, so a changed window changes the unit and
        # switch-to-configuration restarts it. modules/fscrypt.nix leans on the
        # same mechanism for the sharing toggle.
        #
        # switch-to-configuration only restarts units it finds in state
        # "active" or "activating" (the guard around handle_modified_unit in
        # pkgs/by-name/sw/switch-to-configuration-ng). A unit skipped by a
        # failing ConditionX is INACTIVE, so it is invisible to that pass —
        # which is exactly how the old `ConditionFileNotEmpty = !<token file>`
        # turned the settings SPA's compute-window controls into a no-op:
        # after the one boot that enrolled the box, no rebuild ever restarted
        # this unit again. Any Condition added here re-breaks that, however
        # correct the condition looks on its own. The decision belongs in the
        # script, where both outcomes end in `active (exited)`.
        #
        # The decision itself: re-join when we hold no usable token, or when
        # the arguments differ from the ones the last successful join sent.
        # Deliberately NOT on every boot — the edge's /cluster/join deletes
        # this box's Node object and its node-password Secret before issuing a
        # token (the stale-node cleanup a reinstalled box depends on), and
        # midnight-reboot.timer fires at 00:07 unconditionally, so an
        # unconditional join would delete and recreate the node object nightly
        # and have Longhorn rebuild this box's replicas over a WAN link every
        # single night.
        #
        # What that costs, so it is not discovered as a surprise: a window
        # change on an already-joined box now spends one node-object deletion.
        # /cluster/join is the only route that carries a window, and it runs
        # the stale-node cleanup unconditionally, so telling the edge about a
        # new window means being deleted and re-registering — the kubelet
        # recreates its own Node object within a heartbeat, but Longhorn drops
        # this box's node record and rebuilds its replicas. Rare and
        # user-initiated (someone moved a slider) rather than nightly and
        # automatic, which is the trade this unit is making. The way to stop
        # paying it at all is a window-only route on the registrar
        # (backend-registrar/src/server.rs) that skips the cleanup; until that
        # exists, do NOT try to save the rebuild by suppressing the re-join —
        # that is the bug this comment replaced.
        #
        # To force a re-join by hand — a rotated agent token on the edge —
        # delete the token file; the stamp beside it does not matter, because
        # the token is tested first.

        # Both commands the script calls beyond the registrar itself. `cmp` is
        # diffutils, which is already in environment.defaultPackages, so this
        # adds nothing to the closure.
        path = [
          pkgs.coreutils
          pkgs.diffutils
        ];

        script = ''
          set -eu

          # modules/nextcloud-common.nix already tmpfiles /var/secrets into
          # existence, but this unit creates its own directory so it stays
          # correct on a box where Nextcloud is off. A second tmpfiles entry
          # for the same path would make systemd-tmpfiles log a duplicate-line
          # warning on every boot, which is why this is here and not a rule.
          install -d -m 0700 ${lib.escapeShellArg meshTokenDir}

          # -s, not -e: a zero-byte token — a write interrupted before the
          # rename, a hand-cleared file — must re-join rather than lock the box
          # out of the mesh forever.
          if [ -s ${lib.escapeShellArg (toString cfg.tokenFile)} ] \
            && cmp -s ${joinArgsFile} ${lib.escapeShellArg joinStamp}; then
            echo "mesh join: token held and enrolment unchanged; not re-joining"
            exit 0
          fi

          ${registrar}/bin/losos-registrar ${joinArgs}

          # Only after the join returned 0. A stamp written ahead of the fetch
          # would tell the next boot that a join which never happened had.
          install -m 0600 ${joinArgsFile} ${lib.escapeShellArg joinStamp}
        '';

        serviceConfig = {
          Type = "oneshot";
          RemainAfterExit = true;
          # The client gives up after roughly five minutes so it cannot hold the
          # boot open; this outer retry is what turns a longer edge outage into
          # a box that has a token by the time it next boots, rather than one
          # that needs an operator. RestartSec is far longer than systemd's
          # default StartLimitIntervalSec, so the start limit never trips and
          # the retry really is indefinite. What it does not do is start
          # rke2-agent when it finally succeeds — see the Requires= note above.
          Restart = "on-failure";
          RestartSec = "5min";
          UMask = "0077";
          PrivateTmp = true;
          NoNewPrivileges = true;
        };
      };

      # ── Longhorn prerequisites ────────────────────────────────────────────
      # Longhorn attaches its volumes over iSCSI to the host and mounts RWX
      # volumes over NFS, so both clients have to exist on the node itself —
      # the storage engine runs in a pod, but the mount happens in the host's
      # namespace. Longhorn is deployed into the mesh cluster from the edge
      # (services.rke2.autoDeployCharts), so nothing about the chart appears
      # here; only the node-side prerequisites do.
      services.openiscsi = {
        enable = true;
        # An initiator IQN is mandatory (the option has no default) and must be
        # unique per node, or two appliances present the same initiator name to
        # the same target. iqn.<yyyy-mm>.<reversed domain>:<unique>, with the
        # appliance id as the unique part for the same reason the node name uses
        # it: it is the registrar's registry key.
        name = "iqn.2026-09.dev.losos:${applianceId}";
      };

      # The openiscsi module loads iscsi_tcp itself. Naming it again costs
      # nothing (NixOS deduplicates the list) and keeps this the complete set of
      # kernel modules a mesh node needs. dm_crypt is Longhorn's volume
      # encryption.
      boot.kernelModules = [
        "iscsi_tcp"
        "dm_crypt"
      ];

      environment.systemPackages = [ pkgs.nfs-utils ];
    })
  ];
}
