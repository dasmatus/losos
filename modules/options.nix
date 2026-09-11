# Project-wide option declarations for losos.
{
  lib,
  pkgs,
  config,
  ...
}:

let
  # Type for a *runtime* path to a secret file (token, admin password).
  #
  # Deliberately not lib.types.path: that check is `isStringLike`, so it
  # silently accepts a derivation — `pkgs.writeText "tok" "hunter2"` type-checks
  # and lands the secret world-readable in /nix/store. `secretPath` still
  # coerces a derivation to its store path (the VM tests build fixtures that
  # way) but the final type is a plain string, and the check at the bottom of
  # this file warns about anything under builtins.storeDir. Type plus check,
  # because a type alone cannot see where the string points.
  storePathLike = lib.mkOptionType {
    name = "storePathLike";
    description = "derivation or store path";
    check = x: !(builtins.isString x) && lib.isStringLike x;
    merge = lib.options.mergeEqualOption;
  };
  secretPath = lib.types.coercedTo storePathLike toString lib.types.str;

  inNixStore = p: lib.hasPrefix builtins.storeDir (toString p);
in
{
  options.losos = {
    targetDrives = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ "/dev/sda" ];
      defaultText = lib.literalExpression ''[ "/dev/sda" ]'';
      description = ''
        Block devices to pool into the LVM volume group with disko. The first
        entry also carries the ESP. One drive is fine (a VG with a single PV);
        list several to merge their capacity into one logical volume.
      '';
    };

    tpm.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Use a TPM2 chip to unlock the persistent LUKS partition at boot.
        Disable on machines without TPM2; a random keyfile is used instead.
      '';
    };

    # ── Hardening ───────────────────────────────────────────────────────────
    # Staged rather than one boolean, because the thing everyone reaches for
    # first no longer exists: `nixos/modules/profiles/hardened.nix` was removed
    # in 26.05 and `linux_hardened` is now
    # `throw "linux_hardened has been removed due to lack of maintenance"`.
    # Upstream dropped the profile on the grounds that it "lacks a consistent
    # and transparent baseline" and was "more of a 'grab bag' of settings than
    # a cohesive security policy" — so modules/hardening.nix splits the safe
    # part from the parts that can break something, and the second group is
    # opt-in. Every flag is asserted in tests/hardening.nix, which also asserts
    # that nothing here blocks the kernel surface k3s, rke2, containerd and
    # Longhorn need.
    hardening.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        The baseline hardening layer: KSPP kernel parameters, sysctl
        tightening, a kernel-module blacklist that also blocks explicit
        modprobe, nosuid/nodev mount options, dbus-broker, and systemd
        sandboxing on the host services that face the network.

        On by default because nothing in it costs this appliance anything it
        was using — in particular it does not touch the container runtime, the
        CNI or Longhorn's kernel surface. The settings that *could* cost
        something live under the flags below instead.
      '';
    };

    hardening.apparmor = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Enable AppArmor. Off by default: nixpkgs ships profiles for only a
        fraction of what runs here, so it confines less than it looks like it
        does, and an unconfined-but-confinable process is a policy decision
        rather than a free win.
      '';
    };

    hardening.malloc = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Use GrapheneOS' hardened_malloc as the system allocator
        (environment.memoryAllocator.provider). Be clear about the blast
        radius: it works by LD_PRELOAD, so it covers the host's dynamically
        linked processes (nginx, avahi, lososd) and neither the static Go
        binaries of k3s/rke2/containerd nor anything inside a pod, which has
        its own rootfs. A real hardening win over a smaller surface than it
        first appears, at a throughput and memory cost.
      '';
    };

    hardening.nosmt = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Disable simultaneous multithreading
        (security.allowSimultaneousMultithreading = false). Closes the
        cross-thread side channels no software mitigation fully covers, and
        roughly halves the core count of the mini-PC this runs on. Off by
        default because the box transcodes video for Nextcloud and may run mesh
        compute for strangers; turn it on if that second one worries you more
        than the first.
      '';
    };

    hardening.usbguard = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Run usbguard, blocking USB devices that were not present at boot. A
        good fit for a set-and-forget appliance nobody is supposed to touch,
        and a nuisance on one you still plug things into. Off by default so a
        keyboard attached for console recovery keeps working.
      '';
    };

    # ── Master proxy (appliance side) ───────────────────────────────────────
    # Replaces the retired losos.cfd (Cloudflare Tunnel). The appliance dials
    # out to the edge over rathole (no inbound public port) and announces
    # itself to the edge's losos-registrar, which rewrites Traefik + rathole
    # config there. See docs/superpowers/specs/2026-08-17-master-proxy-design.md
    # and modules/proxy.nix. The edge side of the contract is losos.edge.*
    # (below). The on-box Nginx stays the slave proxy on :80.
    proxy.enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Enable the master-proxy appliance side: a rathole client that dials
        out to the edge (losos.proxy.edgeRatholeEndpoint) and a
        losos-registrar announce service that registers/heartbeats this
        appliance with the edge. The on-box Nginx stays the slave proxy on
        :80; rathole forwards the tunnel to it. No inbound public port is
        opened — the no-SSH/no-public-ports invariant is preserved.
      '';
    };

    proxy.edgeRatholeEndpoint = lib.mkOption {
      type = lib.types.str;
      default = "edge.losos.cfd:2333";
      description = "host:port the rathole client dials (the edge rathole server's [server] bind).";
    };

    proxy.registrarUrl = lib.mkOption {
      type = lib.types.str;
      default = "https://register.losos.cfd";
      description = "Base URL of the edge losos-registrar HTTP API (fronted by Traefik at a static hostname).";
    };

    proxy.hostname = lib.mkOption {
      type = lib.types.str;
      default = "${config.losos.hostName}.losos.cfd";
      defaultText = lib.literalExpression "\${config.losos.hostName}.losos.cfd";
      description = "Public hostname this appliance registers; Traefik routes Host(<hostname>) through the tunnel to this box's Nginx.";
    };

    proxy.applianceId = lib.mkOption {
      type = lib.types.str;
      default = config.losos.hostName;
      defaultText = lib.literalExpression "config.losos.hostName";
      description = "Stable appliance id; the registry key and the rathole service name.";
    };

    proxy.tokenFile = lib.mkOption {
      type = secretPath;
      default = "/var/secrets/losos-proxy-token";
      description = ''
        Per-appliance shared secret (0600, persisted via /var). Used both to
        authenticate /register and as the rathole service token. Must match
        losos.edge.tenants.<applianceId>.tokenFile on the edge. Provision out
        of band (agenix or a manual write), not via the nix store.
      '';
    };

    proxy.bootstrapTokenFile = lib.mkOption {
      type = secretPath;
      default = "/var/secrets/losos-rathole-bootstrap";
      description = ''
        The rathole default_token (0600, persisted via /var) — the shared
        transport secret. Must match losos.edge.bootstrapTokenFile on the
        edge. Provision out of band, not via the nix store.
      '';
    };

    proxy.heartbeatInterval = lib.mkOption {
      type = lib.types.str;
      default = "30s";
      description = "Cadence losos-registrar announce POSTs /heartbeat. Must be well under losos.edge.heartbeatTtl.";
    };

    proxy.rathole.package = lib.mkOption {
      type = lib.types.package;
      default = pkgs.rathole;
      defaultText = lib.literalExpression "pkgs.rathole";
      description = "rathole derivation for the appliance-side client.";
    };

    proxy.registrar.package = lib.mkOption {
      type = lib.types.nullOr lib.types.package;
      default = null;
      description = ''
        The losos-registrar derivation (Rust). When non-null, the announce
        service runs; wired by modules/defaults.nix to
        self.packages.<system>.losos-registrar. Leave null to run without.
      '';
    };

    # ── Compute mesh (appliance side) ───────────────────────────────────────
    # Two Kubernetes instances run on the appliance and they are deliberately
    # different clusters. See
    # docs/superpowers/specs/2026-09-09-k3s-mesh-design.md.
    #
    #   LOCAL  services.k3s, role=server, /var/lib/rancher/k3s — runs THIS
    #          box's Nextcloud and Forgejo. Always on when either service is in
    #          "container" mode. It has no off-box dependency, so the appliance
    #          boots and serves its own data with the edge unreachable.
    #   MESH   services.rke2, role=agent, /var/lib/rancher/rke2 — joins the
    #          edge's cluster for Longhorn storage and mesh compute. Gated on
    #          losos.cluster.enable.
    #
    # The split exists because an agent's kubelet cannot start while its server
    # is unreachable, and this box reboots unconditionally at 00:07
    # (modules/updates.nix). A single-cluster design would take every
    # appliance's own services down for any edge outage spanning midnight.
    cluster.enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Join the edge's mesh cluster as an rke2 agent, contributing storage
        (Longhorn) and, when losos.cluster.shareCompute is on, compute. Off by
        default: joining makes mesh workloads — but never this box's own
        Nextcloud and Forgejo — depend on the edge being reachable.
      '';
    };

    cluster.serverAddr = lib.mkOption {
      type = lib.types.str;
      default = "https://edge.losos.cfd:9345";
      description = ''
        The mesh control plane's rke2 supervisor endpoint. Note the port: rke2
        agents register on 9345, not on the apiserver's 6443.
      '';
    };

    cluster.tokenFile = lib.mkOption {
      type = secretPath;
      default = "/var/secrets/losos-mesh-token";
      description = ''
        The rke2 node token, fetched from the edge registrar's join route and
        written 0600 by losos-mesh-join. Persisted via /var. Never a store
        path.
      '';
    };

    cluster.nodeName = lib.mkOption {
      type = lib.types.str;
      default = config.losos.proxy.applianceId;
      defaultText = lib.literalExpression "config.losos.proxy.applianceId";
      description = ''
        This box's node name in the mesh cluster. Defaults to the appliance id
        rather than losos.hostName, because every appliance ships the same
        stock hostName ("mattbox") and the second box to join would collide —
        the edge rejects a duplicate node name permanently. The appliance id is
        already required to be unique: it is the registrar's registry key.
      '';
    };

    cluster.kubeletPort = lib.mkOption {
      type = lib.types.port;
      default = 10260;
      description = ''
        Kubelet port for the MESH (rke2) instance. Must differ from the local
        k3s server's kubelet, which holds the default 10250 — two kubelets on
        one host cannot share it. Applied via services.rke2.extraKubeletConfig.
      '';
    };

    cluster.shareCompute = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Contribute this box's CPU to the mesh during the window below ("share
        my compute when I sleep"). Outside the window the edge holds a
        NoSchedule taint on this box's mesh node, so mesh work is repelled.
        This box's OWN Nextcloud and Forgejo are unaffected in either case:
        they live in the local cluster, which has no taint and no scheduler.
      '';
    };

    cluster.computeWindow.start = lib.mkOption {
      type = lib.types.str;
      default = "23:00";
      description = ''
        Start of the compute-sharing window, HH:MM in this box's own timezone
        (config.time.timeZone). The zone is sent to the edge alongside the two
        bounds, because the edge is what writes the NoSchedule taint and so
        compares the window against its own clock — an edge VPS runs UTC while
        this appliance ships Europe/Berlin, and without the zone a window
        entered here would be enforced at the offset, sliding again at DST.

        Validated by lososd, not only by the browser — a token holder can POST
        to /api/apply directly.
      '';
    };

    cluster.computeWindow.end = lib.mkOption {
      type = lib.types.str;
      default = "07:00";
      description = ''
        End of the compute-sharing window, HH:MM. A window whose end is before
        its start wraps over midnight, which is the common case and the
        default. Note that midnight-reboot.timer (00:07) and system.autoUpgrade
        (03:00) both fall inside the default window.
      '';
    };

    cluster.rke2.package = lib.mkOption {
      type = lib.types.package;
      default = pkgs.rke2;
      defaultText = lib.literalExpression "pkgs.rke2";
      description = "rke2 derivation for the appliance-side mesh agent.";
    };

    cluster.k3s.package = lib.mkOption {
      type = lib.types.package;
      default = pkgs.k3s;
      defaultText = lib.literalExpression "pkgs.k3s";
      description = "k3s derivation for the appliance-side local cluster server.";
    };

    # ── Workload images ─────────────────────────────────────────────────────
    # The OCI images the LOCAL cluster runs, wired by modules/defaults.nix to
    # this flake's own derivations (flake/images.nix). Internal: they are an
    # implementation detail of container mode, not a knob.
    #
    # They are listed in the k3s instance's `images` option, which symlinks each
    # into /var/lib/rancher/k3s/agent/images for the agent to import, so the
    # pods can run with imagePullPolicy: Never and nothing is fetched from a
    # container registry at runtime.
    #
    # The images themselves are NOT built on the appliance: a Nextcloud image is
    # ~2.6 GiB, and system.autoUpgrade would rebuild it at 03:00 on a mini-PC
    # with a tmpfs root, three hours before the unconditional 00:07 reboot.
    # They come from this project's own Nix binary cache instead. That is a
    # deliberate, documented relaxation: nothing is pulled from a *container*
    # registry, but the box does substitute from a cache it trusts.
    workloads.pauseImage = lib.mkOption {
      type = lib.types.nullOr lib.types.package;
      default = null;
      internal = true;
      description = "Pause (sandbox) image for the local cluster's pods.";
    };

    workloads.nextcloudImage = lib.mkOption {
      type = lib.types.nullOr lib.types.package;
      default = null;
      internal = true;
      description = "OCI image running the Nextcloud stack in container mode.";
    };

    workloads.forgejoImage = lib.mkOption {
      type = lib.types.nullOr lib.types.package;
      default = null;
      internal = true;
      description = "OCI image running Forgejo in container mode.";
    };

    # ── The shared data domain (fscrypt) ────────────────────────────────────
    shared.fscrypt.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Protect /home/shared/data with an fscrypt policy that is unlocked only
        while losos.sharingMyStorage is on. This is defence in depth on top of
        the LUKS volume, not a replacement for it: LUKS protects a powered-off
        box, fscrypt keeps the shared domain opaque to the RUNNING system — a
        compromised Nextcloud pod, any other service, or root — whenever
        sharing is off.

        Requires an fscrypt-capable filesystem; modules/disko.nix formats
        /persist as ext4 with -O encrypt for exactly this reason.
      '';
    };

    shared.fscrypt.keyFile = lib.mkOption {
      type = secretPath;
      default = "/var/secrets/losos-shared-key";
      description = ''
        The fscrypt protector key. Sealed to the TPM when losos.tpm.enable is
        true, so it is released only to a known-good boot state; on the no-TPM
        path it is a 0600 keyfile, mirroring how modules/disko.nix already
        branches for the LUKS volume. A design that only supported TPM2 would
        brick every losos.tpm.enable = false machine.

        The key lives inside the LUKS-protected /persist, which is what makes
        the layering work: powered off, LUKS keeps it unreachable; booted with
        sharing off, it is simply absent from the kernel keyring.
      '';
    };

    hostName = lib.mkOption {
      type = lib.types.str;
      default = "mattbox";
      description = "System hostname. Avahi publishes <hostName>.local via mDNS.";
    };

    # sharing's caring btw
    sharingMyStorage = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Contribute this box's storage to the shared Longhorn pool on the mesh
        (rke2) cluster. This is the one option the Local/Mesh toggle in the
        admin UI drives: lososd patches the assignment on disk and starts a
        rebuild, so this option and the API's `mode = mesh` always mean the
        same thing (backend/schema.json, backend/src/model.rs).

        It does double duty, and the second job is the load-bearing one: it
        gates the fscrypt unlock of the shared data domain
        (modules/fscrypt.nix). With sharing off the protector key is not in the
        kernel keyring and /home/shared/data is opaque to every process on the
        running box, root included. Publishing and decrypting are deliberately
        one switch: a contributed volume nobody can read is not a
        contribution, and a domain left unlocked while nothing is shared is a
        standing liability on a box with no shell to lock it from.

        This used to read "expose local storage to the Tahoe-LAFS grid as a
        storage server". Tahoe-LAFS is gone; the option name is unchanged
        because persisted state files and the admin SPA carry it.
      '';
    };

    # ── Deployment mode switches ──────────────────────────────────────────
    nextcloud.mode = lib.mkOption {
      type = lib.types.enum [
        "native"
        "container"
      ];
      default = "container";
      description = ''
        "native" — run Nextcloud as a native NixOS service (services.nextcloud)
        on the host.
        "container" — run the same stack as a static pod in this box's own
        local k3s cluster (modules/cluster.nix runs the cluster,
        modules/workloads.nix writes the manifest and the config the pod
        mounts), fronted by Nginx path routing (modules/containers.nix). This
        is the default deployment.

        The value is still spelled "container" even though it no longer means a
        systemd-nspawn container on a private veth: that is the user-facing
        wording in the settings SPA, and there is no installed base worth
        breaking to rename it. modules/nextcloud-common.nix holds the truth
        both modes share, so the two cannot drift.
      '';
    };

    forgejo.mode = lib.mkOption {
      type = lib.types.enum [
        "native"
        "container"
      ];
      default = "container";
      description = ''
        "native" — run Forgejo as a native NixOS service (services.forgejo).
        "container" — run Forgejo as a static pod in this box's own local k3s
        cluster (modules/workloads.nix) behind Nginx path routing
        (modules/containers.nix). As with losos.nextcloud.mode, "container" is
        a kept spelling and no longer means systemd-nspawn.
      '';
    };

    forgejo.enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Run the Forgejo git host. Consulted in *both* deployment modes: native
        (modules/services.nix) and container (modules/workloads.nix gates the
        pod and the config it mounts, modules/cluster.nix the local cluster,
        modules/containers.nix the /forgejo/ route and its firewall entry).
        Set to true by modules/defaults.nix.
      '';
    };

    nextcloud.hostName = lib.mkOption {
      type = lib.types.str;
      default = "localhost";
      description = "Hostname Nextcloud is served on (set to your domain for remote access).";
    };

    nextcloud.adminpassFile = lib.mkOption {
      type = secretPath;
      default = "/var/secrets/nextcloud-admin-pass";
      description = ''
        Path to the file holding the Nextcloud admin password. Generated with
        mode 0600 on first boot by the losos-nextcloud-adminpass oneshot in
        modules/nextcloud-common.nix if absent; persisted via /var. Must be a
        runtime path: a store path is world-readable and gets warned about.
      '';
    };

    nextcloud.https = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "Serve Nextcloud over HTTPS (requires a certificate / domain in production).";
    };

    # Host loopback port the in-container Nextcloud publishes (port 80) on,
    # via containers.nextcloud.extraFlags = [ "--port=127.0.0.1:<port>:80" ]
    # (nspawn's own publish flag, not the NixOS forwardPorts option). Nginx
    # proxies /nextcloud to the container IP directly; this port is retained
    # for direct local access.
    nextcloud.apachePort = lib.mkOption {
      type = lib.types.port;
      default = 11000;
      description = ''
        Loopback port the Nextcloud pod's httpd listens on when
        losos.nextcloud.mode == "container". The pod is hostNetwork (the local
        cluster runs no CNI), so it binds 127.0.0.1 on the host itself and
        Nginx proxies /nextcloud straight there — there is no container address
        left to forward from.

        Both ends read this one option: modules/workloads.nix renders the
        Listen directive the pod mounts, modules/containers.nix writes the
        matching proxy_pass, so they cannot drift. It stays rendered config
        rather than an image layer because the settings SPA can retune it, and
        a ~2.6 GiB image cannot be rebuilt on a mini-PC at 03:00.
      '';
    };

    # ── GPU hardware acceleration for the Nextcloud container ──────────────
    gpu.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Enable host graphics (VA-API/Mesa) and hand /dev/dri to the Nextcloud
        pod for GPU acceleration; modules/workloads.nix adds the hostPath
        volume when this is on.

        Under systemd-nspawn this also carried `DeviceAllow = char-drm rw` on
        the container's unit, and a pod has no equivalent short of
        `privileged: true` or a device plugin — so the mount alone may not be
        sufficient. If VA-API comes back "permission denied" inside the pod,
        that missing cgroup device rule is why, not this option.
      '';
    };

    upgradeFlakeUri = lib.mkOption {
      type = lib.types.str;
      default = "git+file:///etc/nixos#install";
      description = ''
        Flake URI system.autoUpgrade rebuilds from. The fragment is mandatory:
        without one `nixos-rebuild --flake` looks for
        nixosConfigurations.<hostname>, and this flake only exports `iso` and
        `install` — so every unfragmented run dies with "flake does not provide
        attribute". lososd uses the same `#install` ref (backend/src/io_backend.rs).
        Use a github: URI (still with `#install`) for remote auto-updates.
      '';
    };

    # The Rust control-plane package holding both executables: the `lososd`
    # root daemon (D-Bus org.losos1 on the system bus + loopback Bearer-authed
    # admin HTTP API) and the `losos-ctl` facade CLI relaying to it. When
    # non-null the daemon runs system-wide (see modules/daemon.nix); set to
    # null to run without the control plane.
    backend.package = lib.mkOption {
      type = lib.types.nullOr lib.types.package;
      default = null;
      description = ''
        The losos-ctl/lososd derivation (Rust). When non-null, lososd is
        enabled as a systemd daemon and losos-ctl is installed for root.
        Leave null to run without a backend.
      '';
    };

    # ── Standalone admin endpoint (lososd HTTP API + static admin UI) ──────
    admin.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Serve the standalone losos admin UI (dashboard + settings SPA) on the
        front Nginx vhost and run lososd's loopback JSON API (/api/*).
      '';
    };

    admin.apiPort = lib.mkOption {
      type = lib.types.port;
      default = 8082;
      description = "Loopback port lososd serves the Bearer-authed JSON API on. Nginx proxies /api/ here.";
    };

    admin.tokenFile = lib.mkOption {
      type = secretPath;
      default = "/var/secrets/losos-admin-token";
      description = ''
        Bearer token for the admin API; created randomly with mode 0600 by
        lososd on first start if absent. Persisted via /var.
      '';
    };

    # Path of the packaged static admin UI (built from this flake's ./admin-ui
    # by flake/packages.nix; wired in via modules/defaults.nix). Internal.
    admin.ui = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      internal = true;
      description = "Store path of the static admin UI (dashboard/ + settings/ subdirectories).";
    };

    # ── Installer (the `losos-ctl install` subcommand) ──────────────────────
    installer.package = lib.mkOption {
      type = lib.types.nullOr lib.types.package;
      default = null;
      description = ''
        The losos-ctl derivation to draw the `losos-install` wrapper from. The
        Rust crate builds one `losos-ctl` binary containing both the control
        facade and the `install` subcommand, so this is usually
        `self.packages.<system>.losos-ctl`. Set to null to ship no installer
        binary.
      '';
    };

    installer.autorun = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Auto-run `losos-install` as root's login shell on tty1 at boot. This is
        what turns the installer ISO into a set-and-forget reinstall / factory-
        reset medium: insert it, boot, and the installer runs unattended. Leave
        false on a normal target system.
      '';
    };

    # ── Master proxy (edge side) ───────────────────────────────────────────
    # Options for the edge system (flake output `nixosModules.edge`), which
    # runs Traefik (master proxy), a rathole server, and losos-registrar
    # (serve). Only used by that module; the appliance uses losos.proxy.* above.
    edge.enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "Configure this system as the losos master-proxy edge (Traefik + rathole server + losos-registrar).";
    };

    edge.publicDomain = lib.mkOption {
      type = lib.types.str;
      default = "losos.cfd";
      description = "Apex domain. The static registration router is register.<publicDomain>; per-appliance hostnames live under it.";
    };

    edge.ratholeBindAddr = lib.mkOption {
      type = lib.types.str;
      default = "::";
      description = ''
        Address the rathole server listens on for appliance clients to dial.
        Defaults to `::` (IPv6 any), which on Linux with the default
        `net.ipv6.bindv6only=0` binds dual-stack — so appliances that resolve
        the edge over IPv6 (the common case, and the nixosTest inter-VM path)
        and over IPv4 both reach the tunnel. Serialised bracketed for IPv6
        (`[::]:<port>`) by the `fmtBind` helper in modules/edge.nix and the
        `format_bind` helper in backend-registrar/src/config.rs, which must
        agree byte-for-byte.
      '';
    };

    edge.ratholeBindPort = lib.mkOption {
      type = lib.types.port;
      default = 2333;
      description = "Port appliance rathole clients dial (rathole [server] bind).";
    };

    edge.ratholePortRange = lib.mkOption {
      type = lib.types.str;
      default = "50000-50100";
      description = "lo-hi range the registrar allocates per-appliance rathole edge ports from.";
    };

    edge.heartbeatTtl = lib.mkOption {
      type = lib.types.str;
      default = "120s";
      description = "Tenants with no heartbeat within this TTL are pruned (their Traefik router + rathole service removed).";
    };

    edge.reconcileInterval = lib.mkOption {
      type = lib.types.str;
      default = "15s";
      description = "How often the registrar reconciler re-derives Traefik + rathole config from the registry.";
    };

    edge.registrarApiPort = lib.mkOption {
      type = lib.types.port;
      default = 8443;
      description = "Loopback port the losos-registrar HTTP API listens on; Traefik forwards register.<publicDomain> here.";
    };

    edge.registrarApiBind = lib.mkOption {
      type = lib.types.str;
      default = "127.0.0.1";
      description = ''
        Address the losos-registrar HTTP API binds. Defaults to loopback — in
        production only Traefik (fronting register.<publicDomain>) reaches it.
        Set to `0.0.0.0` only in tests where there is no Traefik/TLS path and
        the appliance VM must dial the registrar directly; never expose it
        publicly in deployment.
      '';
    };

    edge.acmeEmail = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      description = "Let's Encrypt account email for the on-demand cert resolver. Required when losos.edge.enable.";
    };

    edge.bootstrapTokenFile = lib.mkOption {
      type = secretPath;
      default = "/var/secrets/losos-rathole-bootstrap";
      description = "rathole default_token (0600). Shared by all appliance tunnels as the transport Noise bootstrap.";
    };

    edge.tenants = lib.mkOption {
      type = lib.types.attrsOf (
        lib.types.submodule {
          options = {
            hostname = lib.mkOption {
              type = lib.types.str;
              description = "Public hostname Traefik routes to this appliance.";
            };
            tokenFile = lib.mkOption {
              type = secretPath;
              description = ''
                Path to this appliance's token (0600); must match the
                appliance's losos.proxy.tokenFile. Use an agenix/runtime secret
                path, not a store path, so the token stays out of the
                world-readable nix store.
              '';
            };
            cluster = lib.mkOption {
              type = lib.types.bool;
              default = false;
              description = ''
                Permit this appliance to fetch a mesh node token from the
                registrar's join route. Separate from registration: a tenant
                may be published through the proxy without being allowed into
                the cluster.

                NOTE: this reaches the registrar only because modules/edge.nix
                renders it into tenants.json. That writer hardcodes its
                attribute set, so any new per-tenant option must be added there
                too or it is silently dropped.
              '';
            };
          };
        }
      );
      default = { };
      description = "Closed-enrollment whitelist of appliances permitted to register. The registrar only ever writes Traefik routers for ids listed here.";
    };

    edge.rathole.package = lib.mkOption {
      type = lib.types.package;
      default = pkgs.rathole;
      defaultText = lib.literalExpression "pkgs.rathole";
      description = "rathole derivation for the edge-side server.";
    };

    edge.registrar.package = lib.mkOption {
      type = lib.types.nullOr lib.types.package;
      default = null;
      description = "The losos-registrar derivation (Rust). Wired by the edge module to self.packages.<system>.losos-registrar.";
    };

    # ── Mesh control plane (edge side) ──────────────────────────────────────
    # The edge is the cluster the appliances join. It runs services.rke2 with
    # role = "server"; appliances run the same module with role = "agent".
    # rke2 rather than k3s because nixpkgs generates both from one
    # name-parameterized generator, so the appliance can run k3s for its own
    # LOCAL cluster and rke2 for the MESH one without the two colliding on
    # /var/lib/rancher/<name>, on the systemd unit name, or on the module's
    # singleton-ness. See the spec for the full argument.
    edge.cluster.enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "Run the mesh control plane (rke2 server) on this edge, and Longhorn on top of it.";
    };

    edge.cluster.package = lib.mkOption {
      type = lib.types.package;
      default = pkgs.rke2;
      defaultText = lib.literalExpression "pkgs.rke2";
      description = "rke2 derivation for the edge-side mesh server.";
    };

    edge.cluster.agentTokenFile = lib.mkOption {
      type = secretPath;
      default = "/var/secrets/losos-mesh-agent-token";
      description = ''
        The node token appliances present when joining (rke2's agentTokenFile).
        The registrar hands its contents to enrolled tenants over the join
        route; it is never published in the nix store.
      '';
    };

    edge.cluster.advertiseAddr = lib.mkOption {
      type = lib.types.str;
      default = "";
      description = ''
        Address appliances reach this control plane on, passed as
        --node-external-ip and returned to appliances as their serverAddr.
        Required when losos.edge.cluster.enable is set — an rke2 server behind
        NAT that advertises a private address is unjoinable.
      '';
    };

    edge.cluster.apiPort = lib.mkOption {
      type = lib.types.port;
      default = 6443;
      description = "Kubernetes apiserver port on the edge.";
    };

    edge.cluster.supervisorPort = lib.mkOption {
      type = lib.types.port;
      default = 9345;
      description = ''
        rke2's supervisor/registration port. Agents dial THIS, not the
        apiserver's 6443, so it must be open in the edge firewall as well.
      '';
    };

    edge.cluster.cni = lib.mkOption {
      type = lib.types.str;
      default = "canal";
      description = ''
        Mesh cluster CNI. Appliances sit on separate LANs behind NAT, so
        cross-node replication needs an encrypted overlay — canal with a
        WireGuard backend, or cilium with WireGuard encryption. Set on the
        server only: rke2's own docs say an agent must not set `cni`.
      '';
    };

    edge.cluster.longhornChart = lib.mkOption {
      type = lib.types.nullOr lib.types.attrs;
      default = null;
      description = ''
        Longhorn Helm chart spec handed to services.rke2.autoDeployCharts
        (repo, version, hash, values). Longhorn is not packaged in nixpkgs, so
        it is deployed as a pinned chart — a fixed-output derivation, so the
        deployment stays reproducible. Null disables Longhorn.
      '';
    };
  };

  # /nix/store is world-readable, so a secret path that resolves into it is a
  # disclosure, not a configuration style. The `secretPath` type cannot catch
  # this on its own (a derivation coerces to a perfectly valid string — that is
  # exactly how `pkgs.writeText "tok" "hunter2"` used to slip through
  # types.path), so the check lives here.
  #
  # A warning rather than an assertion, and deliberately with no opt-out knob:
  # an escape hatch is a switch someone eventually sets for the wrong reason,
  # and the point is to make the mistake visible on every rebuild, not to
  # invent a supported way of doing it.
  config.warnings =
    let
      secrets = {
        "losos.nextcloud.adminpassFile" = config.losos.nextcloud.adminpassFile;
        "losos.admin.tokenFile" = config.losos.admin.tokenFile;
        "losos.proxy.tokenFile" = config.losos.proxy.tokenFile;
        "losos.proxy.bootstrapTokenFile" = config.losos.proxy.bootstrapTokenFile;
        "losos.edge.bootstrapTokenFile" = config.losos.edge.bootstrapTokenFile;
        "losos.cluster.tokenFile" = config.losos.cluster.tokenFile;
        "losos.shared.fscrypt.keyFile" = config.losos.shared.fscrypt.keyFile;
        "losos.edge.cluster.agentTokenFile" = config.losos.edge.cluster.agentTokenFile;
      }
      // lib.mapAttrs' (
        id: tenant: lib.nameValuePair "losos.edge.tenants.${id}.tokenFile" tenant.tokenFile
      ) config.losos.edge.tenants;
    in
    lib.mapAttrsToList (name: value: ''
      ${name} = "${value}" points into ${builtins.storeDir}, which is
      world-readable — the secret is published to every process on the box.
      Secrets must not enter the nix store: use a runtime path, provisioned out
      of band or generated on first boot.
    '') (lib.filterAttrs (_: inNixStore) secrets);
}
