# The coexistence gate: one host, two Kubernetes instances, two container
# runtimes.
#
# modules/cluster.nix runs a k3s server (the LOCAL cluster, this box's Nextcloud
# and Forgejo) and an rke2 agent (the MESH cluster, Longhorn and mesh compute)
# side by side on one appliance. That arrangement used to rest on an open
# question, and this file used to be the thing that answered it. The question is
# closed now, and the answer was the bad one:
#
#   Both rancher distributions serve their embedded containerd on the SAME
#   hardcoded path, /run/k3s/containerd/containerd.sock. rke2 vendors the k3s
#   agent code and inherits it — nixpkgs' own nixos/tests/rancher/auto-deploy.nix
#   carries the comment "for some reason, RKE2 also uses /run/k3s" and drives
#   crictl at that socket, and default.nix instantiates that file for BOTH
#   distros. Two embedded containerds cannot share one socket: whichever starts
#   second unlinks and rebinds it, and the first is left holding a path nothing
#   reaches it on.
#
# So the contingency the modules/cluster.nix header recorded is now the design:
# the LOCAL server runs a NATIVE containerd (virtualisation.containerd.enable,
# its own systemd unit, its own socket) and is pointed at it with an explicit
# --container-runtime-endpoint, which is also what makes k3s skip spawning an
# embedded one. The MESH agent keeps its embedded containerd on /run/k3s, where
# it is now the only claimant. This test no longer discovers whether a split
# exists; it asserts that the intended split is the one in place.
#
# The two things that did NOT get simpler, and that this file is now mostly
# about:
#
#   * Nothing stops a future edit from handing the local k3s back its embedded
#     containerd — dropping the endpoint flag is a one-line change that looks
#     like a cleanup. The symptom is not an error at the socket; it is one of the
#     two containerds silently losing the race at some later boot, on a box with
#     no shell. So the test asserts positively that NO containerd on the box
#     belongs to the k3s instance, and that the endpoint flag is really there.
#   * A native containerd has namespaces, and the CRI lives in exactly one of
#     them: k8s.io. `ctr images import` writes to `default` unless told
#     otherwise, and an image in `default` is invisible to the kubelet. With
#     imagePullPolicy: Never — which every workload on this box uses, because
#     nothing here may reach a registry — the pod then sits in ImagePullBackOff
#     against an image that is demonstrably present on disk, and neither the
#     kubelet nor crictl says the word "namespace" anywhere. k3s's own `images`
#     option never had this failure mode; the oneshot that replaced it does. It
#     is the single most likely way the new arrangement breaks, so it gets a
#     first-class assertion and a diagnostic that prints both namespaces.
#
# Two VMs:
#
#   edge       services.rke2, role = server. The mesh control plane, stripped to
#              what the appliance actually has to talk to.
#   appliance  services.k3s role = server (local, native containerd) AND
#              services.rke2 role = agent (mesh, embedded containerd), both from
#              modules/cluster.nix, joined to the edge.
#
# ── What is asserted, and what deliberately is not ──────────────────────────
#
# Node READINESS is not asserted, anywhere, for the local cluster. With
# --flannel-backend=none the kubelet reports NetworkReady=false and the node is
# legitimately NotReady for the life of the cluster — that is a design output of
# modules/cluster.nix, not a fault. A `kubectl wait --for=condition=Ready`
# against it never returns and the only thing it proves is that the test can
# time out. So the local half is asserted the way the kubelet actually behaves:
# the node REGISTERS, the static pod's container reaches Running (read straight
# off the CRI with crictl, which needs no scheduler and no apiserver), and the
# thing it serves answers on loopback. The kubelet runs hostNetwork pods even
# while NetworkReady is false — that carve-out is precisely what makes the local
# cluster viable without a CNI, so the test states it as a positive assertion
# rather than leaving it as folklore.
#
# The mesh half is asserted only as far as registration on the edge. That is
# enough to prove what this test is for: the agent authenticated, its kubelet
# came up on a port the local one is not holding, and its embedded containerd
# still owns /run/k3s. Waiting for the mesh node to go Ready would drag canal,
# the overlay and the inter-VM link into a test about runtime separation.
#
# The last two steps are the property the two-cluster split exists to buy, and
# nothing else in the suite covers it: with the edge VM gone, the appliance's
# own workload keeps serving, and it comes back after a reboot taken while the
# edge is still down. If that ever stops holding, every joined appliance loses
# Nextcloud and Forgejo for the length of any edge outage spanning the 00:07
# unconditional reboot (modules/updates.nix), silently, on a box with no shell.
#
# ── What this test does NOT cover ───────────────────────────────────────────
#
#   * modules/edge.nix. The edge here is a hand-rolled services.rke2 server, not
#     losos.edge.cluster.enable, because that module also brings Traefik, ACME,
#     rathole and the registrar — four failure modes orthogonal to the question
#     being asked, three of which tests/edge-vm.nix already covers. The cost is
#     real and worth naming: nothing asserts that edge.nix's server options and
#     cluster.nix's agent options agree.
#   * modules/workloads.nix. The real Nextcloud image is ~2.6 GiB; importing it
#     into a VM to prove a namespace is not the wrong one is not a trade worth
#     making. The probe pod below reuses the real pod shape (hostNetwork,
#     dnsPolicy Default, imagePullPolicy Never, system-node-critical, dropped
#     capabilities) so the shape is exercised even though the workload is not.
#     The IMPORT path is exercised for real, though: the probe image is handed to
#     modules/cluster.nix as losos.workloads.pauseImage and reaches the CRI by
#     whatever oneshot that module uses, with no help from this file.
#   * The registrar join. losos-mesh-join.service is present, and rke2-agent
#     REQUIRES it, so its script is stubbed to a success here and the mesh token
#     is seeded as a fixture instead — the reasoning is at the override. The
#     join itself is covered by backend-registrar/tests/cluster_join.rs, and the
#     registrar API it talks to by tests/edge-vm.nix.
#   * Longhorn, the compute-window taint, and any inbound mesh port. The
#     appliance keeps its default firewall here on purpose: joining needs no
#     inbound port because the agent dials out. Longhorn will need several, and
#     modules/cluster.nix opens none today.
#
# ── Cost ────────────────────────────────────────────────────────────────────
#
# Around 10 GiB of guest RAM and 20 GiB of scratch disk between the two VMs, and
# the better part of a coffee to run: an rke2 server importing its airgap
# bundles is the slow part, and the appliance pays for a k3s server, an rke2
# agent and a third container runtime at once. The resource numbers are lifted
# from nixpkgs' nixos/tests/rancher/default.nix, which is the only calibration
# available. This is a local-only gate — CI's 10-minute cap cannot hold it — so
# run it by hand when touching modules/cluster.nix, and only then.
{ pkgs }:

let
  lososPkgs = import ../flake/packages.nix { inherit pkgs; };

  # ── Fixtures ──────────────────────────────────────────────────────────────
  # Runtime paths written by tmpfiles, never pkgs.writeText store paths:
  # options.nix warns when a secret option points into the world-readable store,
  # and a test that models the anti-pattern teaches it to whoever copies it.
  # Same reasoning, and the same shape, as tests/edge-vm.nix.
  #
  # The edge sets agentTokenFile, so appliances authenticate with the AGENT
  # token rather than the server token — which is what the registrar's join
  # route hands out (backend-registrar/src/join.rs). One value, two files, one
  # binding, so they cannot drift.
  agentTokenValue = "test-mesh-agent-token-0123456789abcdef";
  proxyTokenValue = "test-proxy-token-0123456789abcdef";

  agentTokenFile = "/var/secrets/losos-mesh-agent-token";
  meshTokenFile = "/var/secrets/losos-mesh-token";
  proxyTokenFile = "/var/secrets/losos-proxy-token";

  # Not "mattbox": modules/cluster.nix asserts the mesh node name is neither
  # empty nor the stock hostName, because every appliance ships the same one and
  # the second box to join collides permanently. losos.cluster.nodeName defaults
  # to this, so setting the appliance id is all it takes.
  applianceId = "losos-test-appliance";

  # Both literals belong to modules/cluster.nix, which spells them out rather
  # than declaring options for them. Repeating them here is the price; if either
  # changes there, this test stops finding what it is looking for and says so
  # rather than passing vacuously.
  localNodeName = "losos-local";
  staticPodDir = "/var/lib/losos/k3s-static-pods";

  # The one socket path this file is entitled to hardcode. It is not a losos
  # choice and not a nixpkgs option — it is compiled into both rancher
  # distributions, which is the whole reason the local server was moved off it.
  # The mesh agent must still be found here; the local one must not be found
  # here at all.
  meshSock = "/run/k3s/containerd/containerd.sock";

  # The local k3s kubelet holds the default. The mesh kubelet is pinned here
  # rather than left to the option default so the test asserts a value it chose,
  # not a value it read from the thing under test.
  localKubeletPort = 10250;
  meshKubeletPort = 10260;

  probePort = 8099;
  probeBanner = "losos-local-ok";

  # The rke2 airgap bundles are per-architecture derivations in the package's
  # passthru, so the attribute name carries the arch. Selected the same way
  # modules/cluster.nix selects them, for the same reason: a future aarch64
  # board should fail as a port task, not inside a passthru lookup.
  rke2ImageArch = if pkgs.stdenv.hostPlatform.isAarch64 then "arm64" else "amd64";
  rke2Images = [
    pkgs.rke2."images-core-linux-${rke2ImageArch}-tar-zst"
    pkgs.rke2."images-canal-linux-${rke2ImageArch}-tar-zst"
  ];

  # One image serves as both the local cluster's pause (sandbox) image and the
  # probe workload, exactly as nixpkgs' own single-node rancher test does. It
  # has to sleep forever to be usable as a pause container, and it has to carry
  # socat and echo to be usable as the probe.
  probeImage = pkgs.dockerTools.buildImage {
    name = "test.local/losos-probe";
    tag = "local";
    copyToRoot = pkgs.buildEnv {
      name = "losos-probe-env";
      paths = with pkgs; [
        tini
        coreutils
        socat
      ];
    };
    config.Entrypoint = [
      "/bin/tini"
      "--"
      "/bin/sleep"
      "inf"
    ];
  };

  probeImageRef = "${probeImage.imageName}:${probeImage.imageTag}";

  yaml = pkgs.formats.yaml { };

  # The probe pod mirrors the shape modules/workloads.nix gives Nextcloud and
  # Forgejo, field for field where the field is load-bearing:
  #
  #   hostNetwork          the local cluster runs no CNI, so this is the only
  #                        kind of pod it can schedule at all — and it is what
  #                        makes the kubelet's NetworkReady=false carve-out
  #                        apply.
  #   bind=127.0.0.1       under hostNetwork a listener without an explicit bind
  #                        takes every address on the box. The real workloads
  #                        pin loopback for that reason; so does this.
  #   imagePullPolicy      nothing on the appliance may reach a registry — and
  #                        with a native containerd this is also the field that
  #                        turns a wrong-namespace import into ImagePullBackOff
  #                        rather than a silent pull, which is the failure this
  #                        test exists to name.
  #   priorityClassName    resolved locally by the kubelet for static pods, no
  #                        apiserver object involved. Copied from the real pods
  #                        so that if a kubelet ever starts rejecting it, it is
  #                        caught here rather than on a box with no shell.
  #   the workload label   the kubelet copies a static pod's labels onto its
  #                        mirror pod, which is how the diagnostics below find
  #                        this pod in the apiserver without reconstructing the
  #                        "<name>-<nodeName>" mirror naming convention.
  probePod = yaml.generate "losos-probe.yaml" {
    apiVersion = "v1";
    kind = "Pod";
    metadata = {
      name = "losos-probe";
      namespace = "kube-system";
      labels."losos.dev/workload" = "probe";
    };
    spec = {
      hostNetwork = true;
      dnsPolicy = "Default";
      priorityClassName = "system-node-critical";
      restartPolicy = "Always";
      containers = [
        {
          name = "probe";
          image = probeImageRef;
          imagePullPolicy = "Never";
          command = [
            "socat"
            "TCP4-LISTEN:${toString probePort},bind=127.0.0.1,fork,reuseaddr"
            "EXEC:echo ${probeBanner}"
          ];
          resources.limits.memory = "32Mi";
          securityContext = {
            allowPrivilegeEscalation = false;
            capabilities.drop = [ "ALL" ];
          };
        }
      ];
    };
  };
in

pkgs.testers.nixosTest {
  name = "losos-cluster-coexistence";

  nodes = {
    edge =
      { config, ... }:
      {
        environment.systemPackages = with pkgs; [
          kubectl
          jq
        ];

        # The mesh control plane. Deliberately the stock nixpkgs module rather
        # than modules/edge.nix — see the header for what that costs. What is
        # mirrored from edge.nix is the part the appliance's agent actually
        # depends on: canal as the CNI, and the agent token in a file.
        services.rke2 = {
          enable = true;
          role = "server";
          package = pkgs.rke2;
          inherit agentTokenFile;
          cni = "canal";
          # The nixos test driver gives every VM a default route via eth0, which
          # carries the same address on every machine; inter-node traffic is on
          # eth1. Without pinning the node IP, canal picks eth0 and the overlay
          # is built on an address the other node also owns. A harness artifact,
          # not something a real edge needs.
          nodeIP = config.networking.primaryIPAddress;
          images = rke2Images;

          # Same list nixpkgs' own rancher tests disable, for the same reason:
          # none of it is exercised here and all of it costs memory.
          disable = [
            "rke2-coredns"
            "rke2-metrics-server"
            "rke2-ingress-nginx"
            "rke2-snapshot-controller"
            "rke2-snapshot-controller-crd"
            "rke2-snapshot-validation-webhook"
          ];

          # The eth1 pinning above only reaches k3s' flannel through a flag rke2
          # does not have; rke2's canal takes it as chart values instead. Lifted
          # verbatim from nixpkgs' nixos/tests/rancher/multi-node.nix.
          manifests.canal-config.content = {
            apiVersion = "helm.cattle.io/v1";
            kind = "HelmChartConfig";
            metadata = {
              name = "rke2-canal";
              namespace = "kube-system";
            };
            spec.valuesContent = builtins.toJSON { flannel.iface = "eth1"; };
          };
        };

        # The real edge opens exactly 6443 and 9345 (modules/edge.nix), and
        # tests/edge-vm.nix is where the edge's own configuration is on trial.
        # Here the firewall is only in the way, and leaving it up would make a
        # missing hole look like a coexistence failure.
        networking.firewall.enable = false;

        systemd.tmpfiles.rules = [
          "d /var/secrets 0700 root root - -"
          "f ${agentTokenFile} 0600 root root - ${agentTokenValue}"
        ];

        virtualisation = {
          cores = 4;
          memorySize = 4096;
          diskSize = 8192;
        };
      };

    appliance =
      {
        config,
        lib,
        nodes,
        ...
      }:
      {
        # options.nix and cluster.nix, and nothing else. Not defaults.nix (it
        # would wire the real workload images), not workloads.nix (2.6 GiB of
        # Nextcloud), not proxy.nix (a rathole client dialling an edge that runs
        # no rathole server would only add a failing unit to read past).
        imports = [
          ../modules/options.nix
          ../modules/cluster.nix
        ];

        losos = {
          proxy = {
            # cluster.nix asserts this at eval time: the mesh join authenticates
            # with the master-proxy token, which nothing on the appliance
            # creates, so flipping the mesh toggle on a proxy-less box must fail
            # the rebuild rather than wedge a unit. Nothing else here consumes
            # it, because modules/proxy.nix is not imported.
            enable = true;
            inherit applianceId;
            tokenFile = proxyTokenFile;
            # Also an eval-time requirement, and used only in the ExecStart of a
            # unit this test skips. The real derivation rather than a stub, so
            # the module is evaluated against the binary it will actually run.
            registrar.package = lososPkgs.losos-registrar;
          };

          cluster = {
            enable = true;
            # By IP, not by hostname: an agent validates the supervisor's
            # certificate, and the server's SANs are its own addresses. Note the
            # port — agents register on 9345, never on the apiserver's 6443.
            serverAddr = "https://${nodes.edge.networking.primaryIPAddress}:9345";
            tokenFile = meshTokenFile;
            kubeletPort = meshKubeletPort;
          };

          # Enough to turn the LOCAL cluster on: cluster.nix keys it off
          # losos.nextcloud.mode, whose default is already "container". The pause
          # image is asserted non-null there — without it no pod on the box gets
          # a sandbox — and it is the only one of the three this test supplies.
          #
          # It is also the whole payload of the image import: modules/cluster.nix
          # is what carries this derivation into the native containerd's k8s.io
          # namespace, and this file deliberately does not lend a hand. An
          # explicit `ctr images import` here would make step 3 pass on a box
          # where the module's own import is broken, which is the one thing step
          # 3 is for.
          workloads.pauseImage = probeImage;
        };

        # Merged into the instance modules/cluster.nix declares. Same harness
        # artifact as on the edge: pin the mesh kubelet to eth1 so it registers
        # with an address the edge can tell apart from its own.
        services.rke2.nodeIP = config.networking.primaryIPAddress;

        # The enrolment is stubbed out, and this is the one substitution in the
        # file that could hide a real failure, so here is exactly what it trades.
        #
        # rke2-agent.service REQUIRES losos-mesh-join.service — deliberately, so
        # that a box which could not enrol reports the cause once instead of
        # crash-looping the effect. The consequence for a test with no registrar
        # on the other end is total: the join fails, the agent's job is discarded
        # with "Dependency failed", no mesh kubelet ever starts, and every
        # assertion about two coexisting instances times out having proved
        # nothing about coexistence. The real script would also spend the join
        # client's five-minute retry budget before failing, on every boot, twice
        # over given step 6.
        #
        # So the script is replaced and nothing else is. The Requires edge, the
        # ordering, and rke2-agent's dependence on this unit reporting success
        # all still hold and are still exercised — what is skipped is the HTTP
        # round trip, which backend-registrar/tests/cluster_join.rs covers
        # against the real route and tests/edge-vm.nix covers against the real
        # registrar. The token that would have been fetched is seeded below.
        #
        # The echo is not decoration: without it a reader working through this
        # VM's journal after a failure finds a mesh join that "succeeded"
        # instantly and has no way to know why.
        systemd.services.losos-mesh-join.script = lib.mkForce ''
          echo "mesh join stubbed by tests/cluster-vm.nix; the token is a fixture"
        '';

        # modules/workloads.nix owns the static pod directory on a real
        # appliance, and creates it in the same tmpfiles pass that symlinks the
        # manifests in. It is not imported here, so the test does that job — and
        # only because it is not imported: a second rule for one path earns a
        # "Duplicate line for path" from systemd-tmpfiles on every boot.
        systemd.tmpfiles.rules = [
          "d /var/secrets 0700 root root - -"
          "f ${proxyTokenFile} 0600 root root - ${proxyTokenValue}"
          # What the stubbed losos-mesh-join would have fetched. 0600, and the
          # same value the edge holds in its agentTokenFile, because that is the
          # credential the join route hands out.
          "f ${meshTokenFile} 0600 root root - ${agentTokenValue}"
          "d /var/lib/losos 0700 root root -"
          "d ${staticPodDir} 0700 root root -"
          "L+ ${staticPodDir}/losos-probe.yaml - - - - ${probePod}"
        ];

        environment.systemPackages = with pkgs; [
          kubectl
          cri-tools # crictl: reads the CRI directly, no apiserver in the path
          # ctr, and the only tool on the box that can see a containerd NAMESPACE
          # at all — crictl speaks CRI, and CRI is k8s.io by definition, so it
          # can report an image missing but never report where it actually went.
          # Named here rather than relied on from virtualisation.containerd's own
          # systemPackages: this test must fail on the assertion that the local
          # runtime is native, not on a missing binary.
          containerd
          socat
        ];

        # The appliance's firewall stays at its default. Joining the mesh needs
        # no inbound port — the agent dials out — so this is the real posture,
        # not a concession. Longhorn is a different story and modules/cluster.nix
        # opens nothing for it yet.

        virtualisation = {
          cores = 4;
          memorySize = 6144;
          diskSize = 12288;
        };
      };
  };

  testScript = ''
    import json
    import re

    LOCAL_KUBECONFIG = "/etc/rancher/k3s/k3s.yaml"
    MESH_KUBECONFIG = "/etc/rancher/rke2/rke2.yaml"
    MESH_SOCK = "${meshSock}"

    # The CRI's namespace inside a native containerd, and not a choice anybody
    # gets to make: it is compiled into containerd's CRI plugin. Everything the
    # kubelet can see lives here; everything `ctr images import` writes without
    # -n lands in "default" and is invisible.
    CRI_NS = "k8s.io"

    # Explicit KUBECONFIG on every call rather than environment.sessionVariables:
    # whether the driver's backdoor shell has sourced /etc/profile is not a thing
    # this test should be able to fail on, and a kubectl that silently talks to
    # the wrong cluster would make every assertion below meaningless.
    def kubectl_local(cmd):
        return f"KUBECONFIG={LOCAL_KUBECONFIG} kubectl {cmd}"

    def kubectl_mesh(cmd):
        return f"KUBECONFIG={MESH_KUBECONFIG} kubectl {cmd}"

    # Both endpoints spelled out. crictl falls back to the runtime endpoint for
    # images when only -r is given, but it also has a list of well-known default
    # sockets it probes first in other configurations — and an assertion about
    # WHICH socket holds an image is worthless if crictl is free to pick one.
    def crictl(sock, cmd):
        return f"crictl -r unix://{sock} -i unix://{sock} {cmd}"

    # Same rule for ctr: --address always, never the built-in default.
    def ctr(sock, ns, cmd):
        return f"ctr --address {sock} --namespace {ns} {cmd}"

    # Every process on the box whose executable is exactly "containerd", with its
    # pid and its full argv. Read out of /proc rather than assumed, because the
    # argv is the only authoritative statement of which state directory a
    # containerd belongs to and which socket it was told to bind.
    #
    # The line is assembled only after xargs has succeeded, so a process that
    # exits mid-scan drops out instead of splicing half a record into the next
    # one — the failure mode the earlier version of this helper had, which turned
    # a busy machine into a flaky parse.
    CONTAINERD_SCAN = """
        for c in /proc/[0-9]*/cmdline; do
          d=$(dirname "$c")
          args=$(xargs -0 echo < "$c" 2>/dev/null) || continue
          [ -n "$args" ] || continue
          echo "$(basename "$d") $args"
        done
        true
    """

    def containerd_procs(machine):
        procs = []
        for line in machine.succeed(CONTAINERD_SCAN).splitlines():
            fields = line.split()
            # <pid> <exe> <args...>; containerd-shim-runc-v2 is not containerd.
            if len(fields) < 2 or fields[1].split("/")[-1] != "containerd":
                continue
            args = fields[2:]
            sock = None
            for flag in ("-a", "--address"):
                if flag in args:
                    sock = args[args.index(flag) + 1]
            procs.append({"pid": fields[0], "sock": sock, "line": line})
        return procs

    # Which pids hold a given unix socket. Used to tie a socket path to the
    # process that actually serves it, rather than to the process that claims on
    # its command line that it would. containerd also binds "<sock>.ttrpc", which
    # the substring match picks up too — same pid, so it changes no answer.
    def unix_socket_pids(machine, path):
        out = machine.succeed(f"ss -xlnp | grep -F {path} || true")
        return set(re.findall(r"pid=(\d+)", out))

    # The endpoint a unit's ExecStart hands the kubelet. This is read from the
    # unit file rather than from /proc because it is a statement of INTENT: it is
    # what modules/cluster.nix asked for, and its absence is the regression this
    # test is guarding against — a future edit dropping the flag as a tidy-up and
    # handing k3s its embedded containerd back.
    def cri_endpoint_flag(machine, unit):
        text = machine.succeed(f"systemctl cat {unit}")
        m = re.search(r"--container-runtime-endpoint[= ]+(\S+)", text)
        return m.group(1) if m else None

    # The probe pod answers with a banner and closes. `-t 5` is the half-close
    # timeout: socat sees EOF on stdin immediately, shuts down its write side and
    # would otherwise give the server barely half a second to answer.
    PROBE = (
        "socat -t 5 TCP:127.0.0.1:${toString probePort} - < /dev/null "
        "| grep -q ${probeBanner}"
    )

    start_all()

    # ── 1. Both control planes, three runtimes, all on one host ─────────────
    edge.wait_for_unit("rke2-server.service")
    # containerd.service is the unit virtualisation.containerd.enable declares,
    # and the local cluster's runtime now. If this is what times out, the module
    # has not been switched to a native containerd at all and everything below is
    # asserting an arrangement that does not exist yet.
    appliance.wait_for_unit("containerd.service")
    appliance.wait_for_unit("k3s.service")
    # rke2-agent Requires losos-mesh-join, so this also proves the dependency
    # resolved rather than discarding the agent's job. It is worth little beyond
    # that — rke2-agent is Type=exec with Restart=always, so an agent
    # crash-looping against an unreachable server still shows "active" between
    # restarts. Step 2 is what proves the agent works.
    appliance.wait_for_unit("losos-mesh-join.service")
    appliance.wait_for_unit("rke2-agent.service")

    # ── 2. Both node objects register, in their own clusters ────────────────
    appliance.wait_until_succeeds(kubectl_local("get node ${localNodeName}"), timeout=900)
    edge.wait_until_succeeds(kubectl_mesh("get node ${applianceId}"), timeout=900)

    # Registration on the edge is the real coexistence evidence: the agent
    # authenticated with the token, its kubelet bound a port the local one is not
    # holding, and its own containerd came up far enough to report a runtime.
    #
    # The label is modules/cluster.nix's, and the edge's reconciler and the
    # compute-window taint address this box by it — so a node that registers
    # without it is a node the mesh cannot steer.
    labels = edge.succeed(kubectl_mesh("get node ${applianceId} --show-labels"))
    assert "losos.dev/appliance=${applianceId}" in labels, (
        f"the mesh node registered without its appliance label:\n{labels}"
    )

    # And the two node names really are different objects in different clusters.
    # /etc/rancher/node/password is a fixed path shared by both instances; it
    # survives only because each server hashes the same password under its own
    # node name. Two registrations from one host is that arrangement working.
    appliance.succeed("test -s /etc/rancher/node/password")
    print("node password file:", appliance.succeed("stat -c '%a %U:%G %n' /etc/rancher/node/password"))

    # ── 3. Two runtimes, two sockets, two disjoint image stores ─────────────
    #
    # This is the whole reason the file exists. Both rancher distributions
    # hardcode /run/k3s/containerd/containerd.sock for their embedded containerd,
    # so the arrangement that makes the topology work is: the mesh agent keeps
    # that socket, and the local server does not have an embedded containerd to
    # want it.
    print("containerd sockets present:", appliance.succeed("find /run -name 'containerd*.sock' 2>/dev/null || true"))
    # Printed unconditionally, because the interesting failures show up as a
    # runtime that died rather than as two processes disagreeing, and the
    # assertions below abort before anything else can be gathered.
    print("runtime complaints:", appliance.succeed(
        "journalctl -u k3s.service -u rke2-agent.service -u containerd.service --no-pager "
        "| grep -iE 'address already in use|connection refused|containerd' | tail -40 || true"
    ))

    procs = containerd_procs(appliance)
    print("containerd processes:", json.dumps(procs, indent=2))

    # No containerd anywhere on the box belongs to the k3s instance. That is the
    # positive form of "k3s did not spawn an embedded containerd", and it is the
    # assertion that catches the tempting one-line regression: drop
    # --container-runtime-endpoint from modules/cluster.nix and k3s starts its
    # own again, races the rke2 agent for /run/k3s, and one of the two runtimes
    # loses — at some later boot, on a box with no shell.
    k3s_owned = [p for p in procs if "/var/lib/rancher/k3s/" in p["line"]]
    assert not k3s_owned, (
        f"the local k3s server is running an embedded containerd: {k3s_owned}. "
        "It must use the native one instead — both rancher distributions serve "
        "their embedded containerd on the same hardcoded "
        f"{MESH_SOCK}, so this one and the mesh agent's are now fighting over a "
        "single path and whichever starts second wins it. Restore "
        "--container-runtime-endpoint in modules/cluster.nix; do NOT resolve it "
        "the other way by pointing the rke2 agent at k3s's socket, because two "
        "kubelets against one CRI garbage-collect each other's pods."
    )

    mesh_procs = [p for p in procs if "/var/lib/rancher/rke2/" in p["line"]]
    assert len(mesh_procs) == 1, (
        f"expected exactly one containerd owned by the mesh rke2 agent, found "
        f"{mesh_procs}. An agent with none never started its runtime; an agent "
        "with two is a restart that did not clean up."
    )
    mesh_proc = mesh_procs[0]
    assert mesh_proc["sock"] == MESH_SOCK, (
        f"the mesh agent's containerd serves {mesh_proc['sock']}, not "
        f"{MESH_SOCK}. That path is compiled into rke2 (it vendors the k3s agent "
        "code), so a change here means rke2 itself moved — which would make the "
        "whole native-containerd arrangement unnecessary, and is worth checking "
        "before anything else in this file is believed."
    )

    # The local endpoint is whatever modules/cluster.nix told the kubelet to use.
    # Taken from the flag rather than hardcoded, so the module stays free to pick
    # a path; what is asserted is the property, not the string.
    local_endpoint = cri_endpoint_flag(appliance, "k3s.service")
    assert local_endpoint is not None, (
        "k3s.service carries no --container-runtime-endpoint, so it is running "
        "its own embedded containerd on the same hardcoded socket the mesh rke2 "
        "agent uses. modules/cluster.nix must point the local server at the "
        "native containerd; see this file's header for why the two cannot share."
    )
    local_sock = local_endpoint.removeprefix("unix://")
    assert local_sock != MESH_SOCK and not local_sock.startswith("/run/k3s/"), (
        f"the local server's CRI endpoint is {local_sock}, inside the mesh "
        "agent's hardcoded socket directory. The two runtimes must not share a "
        "path — see the assertion above for what pointing both at one CRI costs."
    )

    # Intent matched against reality: the socket the kubelet was pointed at is
    # really served by the native containerd unit, and the mesh socket is really
    # served by the agent's child. Without this pair, both assertions above would
    # still pass on a box where the local socket is a leftover file nothing is
    # listening on.
    native_pid = appliance.succeed("systemctl show -P MainPID containerd.service").strip()
    print("native containerd MainPID:", native_pid, "mesh containerd pid:", mesh_proc["pid"])
    assert native_pid != mesh_proc["pid"], (
        "containerd.service and the rke2 agent's containerd are the same process"
    )
    assert native_pid in unix_socket_pids(appliance, local_sock), (
        f"{local_sock} is not served by containerd.service (pid {native_pid}); "
        f"listeners: {unix_socket_pids(appliance, local_sock)}"
    )
    assert mesh_proc["pid"] in unix_socket_pids(appliance, MESH_SOCK), (
        f"{MESH_SOCK} is not served by the mesh agent's containerd "
        f"(pid {mesh_proc['pid']}); listeners: {unix_socket_pids(appliance, MESH_SOCK)}"
    )
    for sock in (local_sock, MESH_SOCK):
        appliance.succeed(f"test -S {sock}")

    # ── 3b. The workload image is in the CRI's namespace, not beside it ─────
    #
    # The new failure mode, and the reason this subtest is separate. k3s's
    # `images` option imported straight into its embedded containerd's CRI store;
    # a native containerd has namespaces, and `ctr images import` writes to
    # "default" unless handed -n k8s.io. An image in "default" is on the disk, in
    # the store, listable — and completely invisible to the kubelet. Every pod on
    # this box sets imagePullPolicy: Never, so what the operator sees is
    # ImagePullBackOff against an image that is demonstrably present, with the
    # word "namespace" appearing nowhere in the kubelet's log, crictl's output or
    # the pod's events.
    #
    # Asserted through ctr (which can name a namespace) and through crictl (which
    # is the kubelet's own view, and is k8s.io by definition). Both, because ctr
    # alone would not prove the CRI plugin can see it and crictl alone could not
    # say where it went instead.
    try:
        appliance.wait_until_succeeds(
            ctr(local_sock, CRI_NS, "images ls -q") + " | grep -q test.local/losos-probe",
            timeout=600,
        )
    except Exception:
        print("containerd namespaces:", appliance.execute(f"ctr --address {local_sock} namespaces ls")[1])
        print(
            "images in the CRI namespace:",
            appliance.execute(ctr(local_sock, CRI_NS, "images ls"))[1],
        )
        # If the probe image shows up HERE and not above, the import ran without
        # -n k8s.io and modules/cluster.nix's oneshot is the thing to fix.
        print(
            "images in the default namespace:",
            appliance.execute(ctr(local_sock, "default", "images ls"))[1],
        )
        print("failed units:", appliance.execute("systemctl --failed --no-legend --plain")[1])
        raise

    appliance.wait_until_succeeds(
        crictl(local_sock, "images") + " | grep -q test.local/losos-probe", timeout=600
    )

    # Each runtime holds its own, disjoint image set — the second, independent
    # way of saying these are not one CRI wearing two socket paths. The mesh side
    # has rke2's airgap bundle and has never heard of the probe; the local side
    # has the probe and none of rke2's bundle. The second half of that is not
    # decoration: an import oneshot pointed at the wrong --address would put the
    # probe in the mesh store, and an rke2 that somehow imported into the native
    # runtime would be the socket collision back in a new costume.
    appliance.wait_until_succeeds(
        crictl(MESH_SOCK, "images") + " | grep -q 'rancher/mirrored-'", timeout=900
    )
    appliance.fail(crictl(MESH_SOCK, "images") + " | grep -q test.local/losos-probe")
    appliance.fail(crictl(local_sock, "images") + " | grep -q 'rancher/mirrored-'")

    # Kubelet ports. Both instances default to 10250, so modules/cluster.nix
    # moves the mesh one; two listeners owned by two different processes is the
    # proof that the move took.
    appliance.wait_for_open_port(${toString localKubeletPort})
    appliance.wait_for_open_port(${toString meshKubeletPort})

    def listener_pid(machine, port):
        out = machine.succeed(f"ss -tlnp sport = :{port}")
        pids = re.findall(r"pid=(\d+)", out)
        assert pids, f"nothing owns port {port}:\n{out}"
        return pids[0]

    local_pid = listener_pid(appliance, ${toString localKubeletPort})
    mesh_pid = listener_pid(appliance, ${toString meshKubeletPort})
    assert local_pid != mesh_pid, (
        f"one process owns both kubelet ports (pid {local_pid}); the two "
        "instances are not two instances"
    )

    # State directories. The whole reason the mesh instance is rke2 rather than a
    # second k3s is that nixpkgs' name-parameterized generator keeps these apart;
    # the native containerd adds a third, under its own StateDirectory, which is
    # where the local cluster's image store now lives.
    print("rancher state:", appliance.succeed("ls -1 /var/lib/rancher"))
    appliance.succeed("test -d /var/lib/rancher/k3s/agent")
    appliance.succeed("test -d /var/lib/rancher/k3s/server")
    appliance.succeed("test -d /var/lib/rancher/rke2/agent")
    appliance.succeed("test -d /var/lib/containerd")

    # And the units are active at the same instant, which is the claim in one
    # line. `systemctl is-active` is checked here rather than in step 1 because
    # by now each has been shown to be doing its job.
    appliance.succeed("systemctl is-active containerd.service")
    appliance.succeed("systemctl is-active k3s.service")

    # ── 4. The local cluster runs its workload ──────────────────────────────
    #
    # Read off the CRI, not the apiserver: a static pod is created by the kubelet
    # straight off disk, with no scheduler and no Ready node in the path. This is
    # the assertion that replaces the `kubectl wait --for=condition=Ready` that
    # would never return.
    #
    # The diagnostic on the failure path is the same wrong-namespace story from a
    # different angle, and it is here because this is where a reader will be
    # looking: with imagePullPolicy: Never the mirror pod's STATUS column reads
    # ImagePullBackOff and says nothing about why, so the namespaces are dumped
    # beside it.
    try:
        appliance.wait_until_succeeds(
            crictl(local_sock, "ps --state Running --name probe -q") + " | grep -q .",
            timeout=900,
        )
    except Exception:
        print("mirror pod as the local apiserver sees it:", appliance.execute(
            kubectl_local("-n kube-system get pods -l losos.dev/workload=probe -o wide")
        )[1])
        print(
            "images in the CRI namespace:",
            appliance.execute(ctr(local_sock, CRI_NS, "images ls"))[1],
        )
        print(
            "images in the default namespace:",
            appliance.execute(ctr(local_sock, "default", "images ls"))[1],
        )
        print("kubelet view:", appliance.execute(
            "journalctl -u k3s.service --no-pager | grep -iE 'probe|image' | tail -40"
        )[1])
        raise

    # The mesh CRI knows nothing about it — the pod belongs to one kubelet only.
    appliance.fail(crictl(MESH_SOCK, "ps -a --name probe -q") + " | grep -q .")

    # It serves, on loopback, which is where the Nginx front door proxies to.
    appliance.wait_until_succeeds(PROBE, timeout=300)

    # The local node is NotReady, and that is the correct answer. Stated as an
    # assertion so the design's cost is written down where a future reader trips
    # over it: if this ever flips to True, the local cluster grew a CNI and the
    # static-pods-only constraint can be revisited.
    node = json.loads(appliance.succeed(kubectl_local("get node ${localNodeName} -o json")))
    ready = [c for c in node["status"]["conditions"] if c["type"] == "Ready"][0]
    print("local node Ready condition:", ready["status"], ready.get("reason"), ready.get("message"))
    assert ready["status"] != "True", (
        "the local node reports Ready, which --flannel-backend=none should make "
        f"impossible: {ready}"
    )

    # ── 5. The edge disappears ──────────────────────────────────────────────
    #
    # A crash, not a graceful shutdown: an edge outage is a power cut or a
    # network partition, not a maintenance window. Everything below has to hold
    # with nothing on the other end of the mesh link.
    edge.crash()

    appliance.succeed("systemctl is-active containerd.service")
    appliance.succeed("systemctl is-active k3s.service")
    appliance.succeed(kubectl_local("cluster-info"))
    appliance.succeed(PROBE)

    # rke2-agent is expected to be unhappy and is deliberately not asserted on:
    # with Restart=always it oscillates between activating and active, so any
    # assertion about its state is a coin flip. Printed because a reader
    # debugging a failure below will want to see it.
    print(
        "mesh agent with the edge gone:",
        appliance.execute("systemctl is-active rke2-agent.service")[1],
    )

    # ── 6. And it survives a reboot taken while the edge is down ────────────
    #
    # This is the failure the two-cluster split was designed against, reproduced
    # exactly: modules/updates.nix reboots unconditionally at 00:07, so an edge
    # outage spanning midnight reboots every joined appliance into a world where
    # the mesh control plane is unreachable. If the box's own workloads lived in
    # the mesh cluster, they would not come back until the edge did.
    #
    # The native containerd makes this step carry a second question it did not
    # used to: the image import is now a unit that has to run again on every
    # boot, into a namespace, before the kubelet gives up on a pod it may not
    # pull. A first boot that works and a second boot that does not is exactly
    # the shape an ordering bug takes here.
    appliance.shutdown()
    appliance.start()

    appliance.wait_for_unit("containerd.service", timeout=900)
    appliance.wait_for_unit("k3s.service", timeout=900)
    # The same sockets, not rediscovered ones: where each runtime serves is a
    # property of the arrangement, not of the boot, so waiting for these exact
    # paths also says neither moved across a reboot.
    appliance.wait_until_succeeds(f"test -S {local_sock}", timeout=900)
    appliance.wait_until_succeeds(f"test -S {MESH_SOCK}", timeout=900)
    print("containerd processes after the reboot:", json.dumps(containerd_procs(appliance), indent=2))
    # And the image is in the CRI namespace again, which on a fresh boot is a
    # statement about the import unit rather than about the store.
    appliance.wait_until_succeeds(
        ctr(local_sock, CRI_NS, "images ls -q") + " | grep -q test.local/losos-probe",
        timeout=600,
    )
    appliance.wait_until_succeeds(
        crictl(local_sock, "ps --state Running --name probe -q") + " | grep -q .",
        timeout=900,
    )
    appliance.wait_until_succeeds(PROBE, timeout=300)
    appliance.succeed(kubectl_local("get node ${localNodeName}"))
  '';
}
