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
        Unlock the persistent LUKS volume from a TPM2 chip at boot. The
        installer seals the disk key into a LUKS2 token (`systemd-cryptenroll
        --tpm2-device=auto`, bound to no PCRs — see backend/src/installer_io.rs
        for why) right after formatting, so the box boots unattended from its
        first boot and the initrd carries no secret. The same random keyfile
        the volume was formatted with stays as a recovery slot at
        /etc/keys/persist-keyfile, inside the encrypted volume; `losos-ctl
        grow` authenticates `cryptsetup resize` with it.

        Set to false by the installer on a machine with no TPM2 (and by
        `losos-ctl install --no-tpm`): then that keyfile is baked into the
        initrd as /crypto_keyfile.bin, on the unencrypted ESP, and whoever has
        the disk has the data.

        This default is the mode the installer ISO picks wherever a chip
        exists, and tests/invariants.nix asserts the two agree: the published
        flake (no modules/install-target.nix) must describe a box the ISO
        actually produces. A box the installer put in keyfile mode has that
        recorded in install-target.nix, and flake.nix reads that file from
        /etc/nixos under --impure (which the nightly upgrade and lososd pass)
        so a `github:` upgradeFlakeUri keeps it; without the flag such an
        upgrade would drop the keyfile from the initrd and stop the 00:07
        reboot at a passphrase prompt.
      '';
    };

    bios = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Boot the appliance with GRUB in legacy BIOS mode instead of
        systemd-boot on UEFI. The first target drive then also carries a 1 MiB
        BIOS boot partition for GRUB's core image. The installer sets this
        itself: it is true when the installer medium was booted in BIOS mode
        (no /sys/firmware/efi); `--uefi` requires booting the installer in UEFI
        mode so systemd-boot can write the firmware boot entry.
        Mostly for testing the installed system in a BIOS-only VM.
      '';
    };

    # ── TLS ─────────────────────────────────────────────────────────────────
    # A self-signed certificate, generated on the box, valid for two years.
    #
    # Not ACME, and not because ACME is hard: this appliance is reached at
    # `<hostName>.local` on the LAN and no certificate authority will ever issue
    # for a name in `.local`. The master proxy has a real certificate, but that
    # covers the tunnel, not the box, and the admin plane deliberately does not
    # travel through it.
    #
    # Two years rather than 90 days is the answer to renewal on a machine with
    # no shell. Nothing here can run certbot and nothing can prompt the owner to
    # re-trust a new certificate on every device twice a year.
    tls.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Serve HTTPS on :443 with a self-signed certificate generated on first
        boot, alongside the existing plaintext :80.

        This is what makes the appliance a *secure context* in a browser, which
        two things need: WebAuthn refuses to run outside one, so passkeys are
        impossible without it; and it is what stops the admin token and every
        Nextcloud login crossing the LAN in clear text, which
        docs/security-model.md currently lists as an accepted limitation.

        The certificate is not trusted by anything until the owner installs it,
        which is a step the first-run wizard owns.
      '';
    };

    tls.validityDays = lib.mkOption {
      type = lib.types.ints.positive;
      default = 730;
      description = ''
        How long the generated certificate is valid, in days. Two years.

        Short lifetimes are good practice precisely because something automated
        renews them, and here nothing can: the renewal is a person installing a
        new certificate on every device they use. A 90-day certificate on this
        box would mean that chore four times a year, or an appliance that
        silently stops being reachable over HTTPS.
      '';
    };

    # ── Finding the box from a web page ──────────────────────────────────────
    # Chrome's Local Network Access permission lets a public, HTTPS page ask
    # the owner once ("look for and connect to devices on your local network")
    # and then fetch from `<hostName>.local`. The finder page on the edge host
    # (edge-vercel/public/find.html) uses that to read `/setup/state.json`
    # and hand the owner a link to their box, instead of a tty1 banner and an
    # address to type. The document is LAN-only and holds no secret, but it
    # is inventory — the box's name and its certificate's fingerprint — and
    # CORS is the only thing deciding which *web pages* on the owner's own
    # machine may read it. So it is an allow-list of exact origins, not `*`:
    # a page the owner merely happens to have open does not learn there is a
    # LosOS box on the LAN. modules/setup.nix turns this into an nginx map.
    setup.finderOrigins = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ "https://losos-edge.dasmat.us" ];
      example = [ "https://find.example.org" ];
      description = ''
        Web origins (scheme://host[:port], no path, no trailing slash) that may
        read `/setup/state.json` cross-origin, which is what the "find my box"
        page on the edge host needs. Everything else on the admin surface
        stays same-origin. An empty list switches the header off.
      '';
    };

    # ── Binary cache ────────────────────────────────────────────────────────
    cache.substituters = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      # The LosOS cache proxy: the `losos-cache-proxy` Vercel project in front
      # of the GHCR-hosted cache CI pushes to. CI reads the same URL from the
      # `LOSOS_PROXY_URL` repository variable; the two must name one host or
      # the appliance substitutes from a cache nothing fills. The second is
      # the same Vercel project under its own vercel.app name, for when the
      # custom domain does not resolve. The third is the static copy the
      # GitHub Pages site serves (.github/scripts/pages-cache.sh), signed
      # with the same key, which nix asks only for what the proxy and
      # cache.nixos.org did not answer.
      default = [
        "https://proxy.losos.dasmat.us"
        "https://losos-cache-proxy.vercel.app"
        "https://losos.dasmat.us/proxy"
      ];
      description = ''
        Extra Nix substituters, added to the appliance *and* to the installer
        medium. The default is the LosOS cache proxy's public URL
        (`https://proxy.losos.dasmat.us`), then the same proxy under
        Vercel's own name (`https://losos-cache-proxy.vercel.app`), then the
        copy of the newest build's paths on the GitHub Pages site
        (`https://losos.dasmat.us/proxy`); `trustedPublicKeys` carries the
        key all three are signed with. Set it to `[ ]` to use cache.nixos.org
        alone.

        This exists because of one number. `losos.nextcloud.mode` defaults to
        `container`, so the install closure contains `losos-image-nextcloud` —
        a 2.3 GiB content closure over 244 store paths — and `losos-ctl install`
        runs a bare `nixos-install` with no substituter flags. Without a cache
        that serves it, a fresh install *builds* that image on a repurposed
        mini-PC.

        The installer medium matters as much as the installed system: a
        substituter the target only learns about after it is installed does not
        make installing it any faster.
      '';
    };

    cache.trustedPublicKeys = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      # The public half of `NIX_CACHE_SIGNING_KEY`, the key CI signs the cache
      # with (`nix key generate-secret --key-name losos-1`; the repository
      # variable `NIX_CACHE_PUBLIC_KEY` holds this same value). Public, so it
      # belongs in the tree; the secret half never does.
      default = [ "losos-1:35OIuYFMKDrlC6yTzqivDPqNqZTMjM0l2MB89bRRreg=" ];
      description = ''
        Public keys trusted for `losos.cache.substituters`. The default is the
        LosOS cache's signing key; rotate both together.

        A substituter whose key is missing or wrong does not fail loudly. Nix
        declines to trust the signature, falls back to building from source, and
        says so in a line nobody reads on a box with no shell. The symptom is
        "the install is still slow", which is the exact thing the cache was
        added to fix, so the two lists are asserted to be set together in
        modules/cache.nix.
      '';
    };

    # ── Storage headroom ────────────────────────────────────────────────────
    storage.fillPercent = lib.mkOption {
      type = lib.types.ints.between 50 100;
      default = 90;
      description = ''
        Percentage of the `persist-vg` volume group the `persist` logical
        volume claims at install time. The remainder is left unallocated on
        purpose, and that is the whole point of the option.

        This used to be 100%FREE, which made the layout — ext4 inside LUKS
        inside an LV — technically resizable and practically frozen: with no
        free extents there is nothing for `lvextend` to grow into, so the only
        way to add space was to physically add a disk.

        Leaving a margin buys two things on a box with no shell. `/nix` lives
        on `/persist` through impermanence, so every generation grows it and
        the 03:00 unattended rebuild is what fills it; growing out of that is
        three online commands (see `losos-ctl grow`) instead of a house call.
        And free extents are what an LVM snapshot needs, so a rollback point
        taken before a risky rebuild becomes possible at all.

        Set to 100 to get the old behaviour back on a box where capacity
        matters more than being able to grow it.
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
      # The demo edge (edge-vercel/, the registrar as a Vercel Function at the
      # owner's domain). Not proxy.losos.dasmat.us: that name is the Nix binary
      # cache proxy (`losos.cache.substituters`). A
      # self-hosted edge is register.<losos.edge.publicDomain>, e.g.
      # https://register.losos.cfd; set this to that when one exists.
      default = "https://losos-edge.dasmat.us";
      description = "Base URL of the edge losos-registrar HTTP API (fronted by Traefik at a static hostname, or the Vercel demo host).";
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

    proxy.noisePublicKeyFile = lib.mkOption {
      type = lib.types.nullOr secretPath;
      default = "/var/secrets/losos-rathole-noise-pub";
      description = ''
        Where the appliance keeps the configured edge's rathole Noise
        **public** key (base64). Nothing to provision: before the tunnel
        first dials, losos-rathole-client fetches it from
        losos.proxy.registrarUrl and keeps it here, and the tunnel is
        encrypted to it from then on (trust on first use, over the
        registrar's TLS). A file already present is never overwritten, so a
        key distributed out of band pins the edge from the start. An edge
        found on the LAN is pinned the same way into its own file under
        /var/secrets/losos-edge-pins/ (lososd names it in
        /run/losos/edge-path.env). `null` runs the tunnel in plain TCP; the
        tunnel carries the admin token, so leave it on.
      '';
    };

    proxy.officialRootKeyFile = lib.mkOption {
      type = lib.types.path;
      default = ../keys/official-edge-root.pub;
      defaultText = lib.literalExpression "../keys/official-edge-root.pub";
      description = ''
        The LosOS root **public** key (Ed25519, 64 hex characters; `#` lines
        are comments), read by lososd on every edge scan. An edge counts as
        *official* only if it presents a certificate signed by the matching
        private key and answers a fresh nonce with the certificate's key
        (`GET <url>/identity?nonce=`, backend/src/edge.rs); only official
        edges may process P2P storage and compute trading, so the market
        relay answers `{available: false, reason: "noOfficialEdge"}` and
        refuses actions (409, `officialEdgeRequired`) through any other edge.
        Discovery and storage sharing are not gated on this: a company's own
        edge keeps working. A file with no key in it (the committed default
        until the project owner writes the key) makes no edge official, so a
        tree without the key fails closed. A public key, so a store path is
        fine here.
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
        my compute when I sleep"), and only while the box is actually idle.
        Outside the window, or while the box is busy, the edge holds a
        NoSchedule taint on this box's mesh node, so mesh work is repelled.
        This box's OWN Nextcloud and Forgejo are unaffected in either case:
        they live in the local cluster, which has no taint and no scheduler.
      '';
    };

    vms.host = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Let the mesh market rent out virtual machines that run on this box.
        Takes effect only while losos.sharingMyStorage is on: the machines'
        disks live in the shared user's home, the storage the box already
        lends the mesh. Half of what each machine earns goes to this box's
        owner. Turn it off to share storage without hosting machines.
      '';
    };

    cluster.idleLoadThreshold = lib.mkOption {
      type = lib.types.float;
      default = 0.25;
      description = ''
        One-minute load average per core below which this box reports itself
        idle, and so lets the mesh schedule onto it *inside* its compute
        window. The window is permission; this is whether the owner is using
        the machine right now. Idle can withdraw availability inside a window
        and can never grant it outside one.

        The default is 0.25 rather than something near zero because an idle
        losos box is not at rest: two kubelets, containerd, Postgres, Redis and
        a PHP-FPM pool all tick over, so a threshold near zero would mean
        "never idle" and compute sharing would quietly never happen. 1.0 per
        core is the other end — fully committed, by which point the owner is
        already waiting on their own machine.

        Set to 0 to stop reporting idle at all, which withdraws the box from
        compute sharing without touching the window.
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

    # ── The keyring (modules/keyring.nix) ───────────────────────────────────
    # Seals the secrets this box mints for itself with systemd-creds, so a copy
    # of /persist that leaves the machine carries ciphertext instead of the
    # Nextcloud admin password in plaintext. Not GNOME keyring and nothing
    # shaped like it: there is no session bus, no desktop and nobody to type a
    # passphrase, so an agent holding an unlocked store in RAM would have
    # nothing to be unlocked by.
    keyring.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Seal this box's self-minted secrets with systemd-creds instead of
        leaving them readable on disk. Covers a /persist copy that leaves the
        box and a file-read bug in a service that can reach /var/secrets; it
        deliberately does not cover root on the running box, because boot has
        to work with nobody present. See modules/keyring.nix for the full list
        of what it does and does not defend against.
      '';
    };

    keyring.store = lib.mkOption {
      type = lib.types.str;
      default = "/var/secrets";
      description = ''
        Directory holding the sealed blobs (`<name>.cred`). Must NOT be under
        /run: these have to survive a reboot, and the module asserts it.
      '';
    };

    keyring.runtimeDir = lib.mkOption {
      type = lib.types.str;
      default = "/run/losos-keyring";
      description = ''
        Where a secret is unsealed to for the moment a service needs it. Must
        be under /run, and the module asserts it — this is a systemd
        RuntimeDirectory, which is tmpfs, and that is the whole point: the
        plaintext never reaches the disk and does not outlive the boot.
      '';
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

    # ── Erasing the box (modules/backup.nix, backend/src/erase.rs) ──────────
    reset.graceMinutes = lib.mkOption {
      type = lib.types.ints.between 1 1440;
      default = 15;
      description = ''
        How long an erase counts down before it changes anything. Started
        from the Reset pane, an erase first finishes its backup (when one was
        asked for), then waits this long, and only then removes the box's
        custom domains and listings from its edge, resets the settings and
        restarts into the wipe. Until the countdown ends it can be cancelled
        from any page of the admin UI, and nothing has changed.
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

        It is also the gate for federation: LosOS Git and LosOS cloud talk
        to other servers only while this is on
        (losos.forgejo.federation.enable, losos.nextcloud.federation.enable).

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

    # Forgejo's ActivityPub side. Upstream labels it experimental, and in the
    # shipped Forgejo (16.x) it amounts to: nodeinfo at
    # /.well-known/nodeinfo and /api/v1/nodeinfo, one ActivityPub actor per
    # account and per repository under /api/v1/activitypub/, stars given from
    # another Forgejo counted on repositories that list that server, and
    # accounts here followable from other servers (their activity goes out as
    # notes). Every inbound request has to carry a valid HTTP signature
    # (Forgejo's SIGNATURE_ENFORCED default); nothing is accepted unsigned.
    #
    # Reach decides what it does: Forgejo advertises itself under ROOT_URL,
    # which modules/workloads.nix builds from losos.proxy.{enable,hostname}.
    # Through an edge that is a public https address and any server can
    # federate with the box; on a LAN-only box it is http://<name>.local/,
    # which only the LAN resolves, so federation reaches other boxes on the
    # same LAN and nothing further.
    #
    # Federation is the box talking to servers it does not know, so it runs
    # only while the box shares at all: the switch below is ANDed with
    # losos.sharingMyStorage (lososInternal.federation, near the end of this
    # file), the one setting that unlocks the shared data pool. With sharing
    # off, Forgejo runs with [federation] off and the front vhost carries no
    # discovery routes, whatever this option says.
    forgejo.federation.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Let LosOS Git federate over ActivityPub with other Forgejo servers:
        stars from other servers count on your repositories, accounts here
        can be followed from elsewhere, and the box publishes nodeinfo.
        Forgejo calls this experimental. Other servers reach the box at the
        address it is served on, so through an edge this is the internet
        and on a LAN-only box it is the LAN. Usage statistics (how many
        accounts, how active) are never published.

        Takes effect only while losos.sharingMyStorage is on. With disk
        sharing off the shared data pool is locked and LosOS Git does not
        federate, whatever this says.
      '';
    };

    # ── The box's configuration on LosOS Git ──────────────────────────────
    # /etc/nixos is a git repository; every Apply commits there
    # (backend/src/config_repo.rs). With this on, lososd keeps it in step
    # with a private repository on the box's own Forgejo, owned by the
    # owner's account, and takes a commit pushed there (after its
    # overrides.nix passes the gates every Apply does) and rebuilds from it.
    configRepo.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Publish this box's configuration (/etc/nixos) to a private repository
        on LosOS Git, and pick up commits pushed to it. Needs
        losos.forgejo.enable. Off, every change is still committed on the
        box; it is just not published anywhere.
      '';
    };
    configRepo.owner = lib.mkOption {
      type = lib.types.str;
      default = "notshared";
      description = ''
        The LosOS Git account the configuration repository belongs to. lososd
        creates it as a site administrator if it is missing and gives it the
        owner's password whenever that password is proved (the first-run
        claim, a password change, a sign-in), so one password opens LosOS
        Git too.
      '';
    };
    configRepo.name = lib.mkOption {
      type = lib.types.str;
      default = "losos-config";
      description = "The name of the configuration repository on LosOS Git.";
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

    # Nextcloud's side of federation: Federated Cloud Sharing (files shared
    # with accounts on other Nextcloud servers, user@host addresses), the
    # trusted-servers app (`federation`), calendar federation, and the OCM
    # discovery and share endpoints other servers call. Gated on
    # losos.sharingMyStorage like LosOS Git's (lososInternal.federation).
    # Off, the front vhost answers 404 for those endpoints and Nextcloud's
    # own switches are set to no on every start (flake/images.nix in
    # container mode, modules/services.nix natively), so neither an inbound
    # share nor an outbound one can be made.
    nextcloud.federation.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Let LosOS cloud share files and calendars with accounts on other
        Nextcloud servers (Federated Cloud Sharing), and answer the
        discovery and share addresses those servers call. Other servers
        reach the box at the address it is served on, so through an edge
        this is the internet and on a LAN-only box it is the LAN.

        Takes effect only while losos.sharingMyStorage is on. With disk
        sharing off the shared data pool is locked and LosOS cloud does not
        federate, whatever this says.
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

        A remote URI evaluates the published tree, which has neither
        modules/install-target.nix (this box's drive list, firmware mode and
        unlock mode, written by the installer) nor this box's
        modules/overrides.nix (the settings the admin UI writes). flake.nix
        therefore reads both from /etc/nixos when the evaluation is impure,
        and the nightly upgrade (updates.nix) and lososd both pass --impure.
        Evaluate the flake purely against a remote URI and the defaults
        apply instead: TPM2 unlock, /dev/sda, UEFI, every setting reset.
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
        Serve the losos admin SPA on the front Nginx vhost and run lososd's
        loopback JSON API (/api/*).
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

    # Path of the built admin SPA (built from this flake's ./admin-ui/app by
    # flake/packages.nix with buildNpmPackage; wired in via
    # modules/defaults.nix). Internal.
    admin.ui = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      internal = true;
      description = ''
        Store path of the built admin SPA: a Vite `dist/` tree with index.html
        at its root, content-hashed bundles under assets/, and the
        deliberately unhashed theme-boot.js beside them. This is the front
        vhost's document root, served with an SPA fallback
        (`try_files $uri $uri/ /index.html`) because the app routes on real
        paths rather than hashes.
      '';
    };

    # Path of the built owner's handbook (handbook/, built by flake/packages.nix
    # as losos-handbook with its base set to /handbook/; wired in via
    # modules/defaults.nix). Internal, like admin.ui.
    admin.handbook = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      internal = true;
      description = ''
        Store path of the built handbook: a Docusaurus `build/` tree rooted at
        /handbook/, served by the front vhost under that prefix, LAN-only, so
        the manual is readable from the local network alone. Null serves no
        handbook.
      '';
    };

    # LosOS Lab is the admin UI's second page (admin-ui/app/lab/, inside
    # losos-admin-ui), so there is nothing to point at, only whether to serve
    # it. Internal, like admin.ui.
    admin.lab = lib.mkOption {
      type = lib.types.bool;
      default = true;
      internal = true;
      description = ''
        Serve LosOS Lab, the setup visualizer, at /lab/ from the admin UI's
        own bundle, LAN-only, under its own content security policy.
      '';
    };

    # ── LosOS Lab's guests under libvirt (modules/lab.nix) ───────────────────
    lab.libvirt.enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Run LosOS Lab's guests under libvirt on this box (KVM when the CPU
        has it) instead of simulating their consoles. Turns on libvirtd and
        a `losos-registrar lab` helper on a loopback port; lososd relays
        /api/lab/ to it and nginx proxies the guests' console and network
        sockets, and the byte relay to libvirtd's socket that the Lab's
        WebAssembly libvirt client uses (one ticket per socket, minted
        under the admin token). The guest images (bzImage, rootfs.bin, gear.bin from
        admin-ui/lab/engine/build.sh) go in `lab.libvirt.images`. Off by
        default: the appliance does not need libvirtd for anything else.
      '';
    };

    lab.libvirt.port = lib.mkOption {
      type = lib.types.port;
      default = 8095;
      description = "Loopback port the Lab's libvirt helper listens on.";
    };

    lab.libvirt.images = lib.mkOption {
      type = lib.types.str;
      default = "/var/lib/losos-lab/images";
      description = ''
        Folder holding the Lab's guest images: bzImage and rootfs.bin, and
        gear.bin for routers, switches and access points. A guest whose
        image is missing boots nowhere on the box and keeps its simulated
        console. Persisted via /var.
      '';
    };

    lab.libvirt.maxGuests = lib.mkOption {
      type = lib.types.ints.between 1 64;
      default = 8;
      description = "Most Lab guests the helper runs at once.";
    };

    lab.libvirt.memoryMiB = lib.mkOption {
      type = lib.types.ints.between 32 4096;
      default = 96;
      description = "Memory of each Lab guest, in MiB. The guests are busybox systems; 96 is what the in-browser engine gives them.";
    };

    lab.ordering.enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      visible = false;
      description = "Show the order button in LosOS Lab.";
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
        Auto-run `losos-install` as root's login shell on tty1 at boot. On the
        installer ISO, this starts the installer and its firmware-mode menu.
        Leave false on a normal target system.
      '';
    };

    # ── Secure Boot (the installer medium) ──────────────────────────────────
    secureBoot.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Build the installer medium so UEFI firmware can verify it under
        Secure Boot (modules/secure-boot.nix, installer ISO only). The UEFI
        loader on the medium's EFI system partition is then one unified
        kernel image (`EFI/BOOT/BOOTX64.EFI`: systemd-stub + kernel + initrd
        + command line) in place of GRUB, so the firmware's one signature
        check covers everything that runs before the squashfs is mounted.
        `nix build` leaves it unsigned; `losos-sign-iso` signs it in place
        with the LosOS db key, which never enters the Nix store. BIOS boot
        (syslinux) is unchanged: Secure Boot is a UEFI feature. Off, the
        medium ships the stock GRUB loader, which no firmware can verify.
      '';
    };

    secureBoot.certFile = lib.mkOption {
      type = lib.types.path;
      default = ../keys/secure-boot-db.pem;
      defaultText = lib.literalExpression "../keys/secure-boot-db.pem";
      description = ''
        The LosOS Secure Boot signing certificate (X.509, PEM; `#` lines
        before the block are comments): the public half of the db key
        `losos-sign-iso` signs the medium with. Shipped on the medium's EFI
        system partition as `EFI/losos/losos-secure-boot.{cer,pem}` so a
        firmware's "enroll from file" dialog can take it from the stick, and
        the certificate `SHA256SUMS.sig` on a release verifies against. The
        committed file carries no certificate until the key ceremony in
        provisioning/secure-boot/ has run: then the medium ships no
        certificate and nothing presents LosOS as a Secure Boot signer.
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

    edge.noisePrivateKeyFile = lib.mkOption {
      type = lib.types.nullOr secretPath;
      default = "/var/secrets/losos-rathole-noise-key";
      description = ''
        The rathole server's Noise private key (base64, 0600, persisted via
        /var), read by the registrar at runtime — never from the store.
        Generated on first boot if absent, with the public half written beside
        it as `<file>.pub` and served to appliances at /noise-public-key. To
        use your own, put the key here before the first boot. `null` runs the
        tunnel in plain TCP.
      '';
    };

    edge.identity.keyFile = lib.mkOption {
      type = lib.types.nullOr secretPath;
      default = "/var/lib/losos-registrar/identity.key";
      description = ''
        This edge's identity **private** key (PKCS#8 hex, 0600, persisted via
        /var, never in the store). The registrar makes it on its first start
        if the file is missing, so the private half never leaves the edge;
        `GET /identity/public-key` serves the public half. Together with
        `edge.identity.certFile` it is what can make the edge *official*:
        once an operator has pushed a certificate the LosOS root signed for
        this key (`losos-registrar provision edge`, from their own machine,
        behind a GitHub sign-in that the edge checks too), the registrar
        serves `GET /identity?nonce=` and boxes whose
        losos.proxy.officialRootKeyFile matches the signer let it process
        their market traffic. Until then the edge is found and shares
        storage but is not official — the shape of an edge a company runs
        for itself. `null` (with `certFile = null`) runs an edge that
        cannot become official at all: every `/identity*` route is 404.
      '';
    };

    edge.identity.certFile = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = "/var/lib/losos-registrar/identity.cert.json";
      description = ''
        Where the certificate for `edge.identity.keyFile` lives: the JSON
        `POST /identity/cert` installs (the primitive is `losos-registrar
        identity sign`), signed by the LosOS root key the project owner
        holds offline. Public, and the registrar writes it, so it must be a
        writable runtime path for the push to work; a store path is read
        but can never be replaced by a push. Must name this edge's public
        URL (the one boxes probe) and be unexpired, or boxes reject it; a
        certificate that is not for the key is logged and ignored. Set
        with `keyFile` or not at all.
      '';
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
            market = lib.mkOption {
              type = lib.types.bool;
              default = false;
              description = ''
                Permit this appliance to buy and sell storage and compute on
                the edge's Stripe Connect market (losos.edge.market). Separate
                from registration and from `cluster`: being published through
                the proxy, or lending compute to the mesh, does not mean the
                operator agreed to settle money with this box. Inert unless
                losos.edge.market.enable is set.

                Like `cluster`, this reaches the registrar only because
                modules/edge.nix renders it into tenants.json.
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
            relayZone = lib.mkOption {
              type = lib.types.nullOr lib.types.str;
              default = null;
              example = "acme.losos.cfd";
              description = ''
                Make this tenant a **spoke** of this edge
                (handbook/docs/in-depth/edge-federation.md): a user-hosted edge
                that may `POST /relay` the boxes behind it, each of which must
                be named exactly one label under this zone
                (`mattbox.acme.losos.cfd`). Relayed boxes get a Traefik router
                and a rathole service here, keyed `<id>.<box>` and authenticated
                with *this tenant's* token; they never get `cluster` or
                `market`. `null` (the default) is an ordinary tenant that may
                relay nothing. Rendered into tenants.json as `relay_zone`, like
                `cluster` and `market`.
              '';
            };
          };
        }
      );
      default = { };
      description = "Closed-enrollment whitelist of appliances permitted to register. The registrar only ever writes Traefik routers for ids listed here.";
    };

    # ── Market (edge side) ──────────────────────────────────────────────────
    # Optional storage / compute marketplace settled through Stripe Connect.
    # The edge is the Stripe platform account: buyers pay it, the seller's share
    # is forwarded to the seller's connected account, and `feeBps` stays behind.
    # Off by default; see handbook/docs/in-depth/market.md.
    edge.market.enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Serve the /market/* routes of losos-registrar. Without this every one
        of them answers 503. Needs a Stripe platform account with Connect
        enabled; see handbook/docs/in-depth/market.md for the one-time
        dashboard setup.
      '';
    };

    edge.market.stripeSecretKeySealed = lib.mkOption {
      type = secretPath;
      default = "/var/secrets/losos-stripe-secret-key.cred";
      description = ''
        The platform's Stripe secret (`sk_...`) or restricted (`rk_...`) key,
        **sealed** with `systemd-creds` under the credential name
        `stripe-secret-key`. The key is never stored in plaintext on disk: the
        `losos-stripe-gate` unit receives this blob through
        `LoadCredentialEncrypted=`, so a copy of `/var` carries ciphertext only
        and the registrar process never holds the key. Seal it from stdin so the
        plaintext never touches the disk either (see
        handbook/docs/in-depth/market.md). Rotating it means sealing a new blob
        and restarting the gate. A blob that is missing skips the gate, so the
        market answers 503 and the rest of the registrar is untouched. A key of
        any other shape is refused before anything is sent to Stripe. The key's
        prefix is the market's mode: `sk_test_`/`rk_test_` run it in test mode
        on the `market-test.json` ledger, `sk_live_`/`rk_live_` in live mode on
        `market.json`, and a key that names neither stops the gate.
      '';
    };

    edge.market.webhookSecretSealed = lib.mkOption {
      type = secretPath;
      default = "/var/secrets/losos-stripe-webhook-secret.cred";
      description = ''
        The signing secrets (`whsec_...`), one per line, of the Stripe webhook
        endpoints that point at
        `https://register.<publicDomain>/market/webhook`, **sealed** with
        `systemd-creds` under the credential name `stripe-webhook-secret` and
        handed to the gate like `stripeSecretKeySealed`. Stripe needs two
        endpoints: one for events on the platform account
        (checkout.session.completed, checkout.session.expired) and one for
        events on Connected accounts
        (account.updated). The registrar accepts a signature from either.
      '';
    };

    # ── Sealed credentials delivered from GitHub Actions ───────────────────
    # `.github/workflows/edge-credentials.yml` reads the keys from the
    # repository's Actions secrets and hands each one over SSH to
    # `losos-seal-credential` on the edge, which seals it with systemd-creds.
    # The deploy key can run nothing else. See "Edge keys from Actions secrets"
    # in docs/ci-and-releases.md.
    edge.credentials.secrets = lib.mkOption {
      type = lib.types.attrsOf (
        lib.types.submodule {
          options = {
            sealed = lib.mkOption {
              type = secretPath;
              description = ''
                Where the sealed blob is written. The credential name is the
                attribute name, so the unit that reads it says
                `LoadCredentialEncrypted=<name>:<this path>`.
              '';
            };
            pattern = lib.mkOption {
              type = lib.types.str;
              description = ''
                An extended regular expression every non-empty line of the
                secret must match in full. A secret of any other shape is
                refused before it is sealed, so a key pasted into the wrong
                Actions secret never reaches a unit.
              '';
            };
            units = lib.mkOption {
              type = lib.types.listOf lib.types.str;
              default = [ ];
              description = ''
                Units restarted once the blob is sealed, so they decrypt the
                new one. A unit that does not exist on this edge is skipped.
              '';
            };
          };
        }
      );
      default = { };
      description = ''
        The secrets `losos-seal-credential` may seal, by systemd credential
        name. The edge module lists `stripe-secret-key`,
        `stripe-webhook-secret` and `claude-api-key`; a module that needs
        another secret adds an entry, or adds its unit to an entry's `units`
        (`losos.edge.credentials.secrets.claude-api-key.units = [ "x.service" ]`).
      '';
    };

    edge.credentials.deployKey = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      example = "ssh-ed25519 AAAAC3Nza... losos-edge-credentials";
      description = ''
        The public half of the SSH key the edge-credentials workflow signs in
        with (its private half is the `EDGE_SSH_KEY` Actions secret). Set,
        it turns sshd on and lets that key in as root with
        `restrict,command="losos-seal-credential"`: it gets no shell, no
        forwarding and no terminal, and can only seal one of the secrets in
        `losos.edge.credentials.secrets`, named by the command it sends. Null (the
        default), nothing is added and secrets are sealed by hand as
        handbook/docs/in-depth/market.md describes (Operator setup).
      '';
    };

    edge.market.feeBps = lib.mkOption {
      type = lib.types.ints.between 0 2000;
      default = 400;
      description = ''
        The platform's cut of every sale in basis points of the gross amount:
        400 is 4%. Collected as Stripe's application_fee_amount on a
        destination charge. Capped at 20% so a typo cannot take a third of
        every sale.
      '';
    };

    edge.market.currency = lib.mkOption {
      type = lib.types.enum [
        "aud"
        "cad"
        "chf"
        "eur"
        "gbp"
        "nzd"
        "usd"
      ];
      default = "eur";
      description = "Supported two-decimal ISO 4217 currency, lowercase. One currency per edge; listings are priced in its minor unit (cents).";
    };

    edge.market.storageClass = lib.mkOption {
      # A DNS-1123 subdomain, as Kubernetes requires of the name: at most
      # 253 characters, each dot-separated label at most 63. The registrar
      # applies the same check to --market-storage-class.
      type =
        lib.types.addCheck
          (lib.types.strMatching "[a-z0-9]([-a-z0-9]*[a-z0-9])?(\\.[a-z0-9]([-a-z0-9]*[a-z0-9])?)*")
          (s: lib.stringLength s <= 253 && lib.all (l: lib.stringLength l <= 63) (lib.splitString "." s));
      default = "longhorn";
      description = ''
        The Kubernetes StorageClass a purchased storage volume is claimed
        from: the mesh's Longhorn pool, which is what appliances sharing their
        storage contribute to.
      '';
    };

    edge.market.returnUrl = lib.mkOption {
      type = lib.types.str;
      default = "";
      example = "https://losos.cfd/market";
      description = ''
        Where Stripe sends a buyer back to after Checkout and a seller back to
        after onboarding. `?order=<id>&status=paid|cancelled` is appended for
        Checkout. Required when the market is enabled.
      '';
    };

    edge.market.hardware.enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      visible = false;
      description = "Sell `hardware.items` through the market's Stripe account.";
    };

    edge.market.hardware.countries = lib.mkOption {
      type = lib.types.listOf (lib.types.strMatching "[A-Z]{2}");
      default = [
        "SK"
        "CZ"
        "AT"
        "DE"
        "PL"
        "HU"
      ];
      visible = false;
      description = "Countries a hardware order may ship to (ISO 3166-1 alpha-2).";
    };

    edge.market.hardware.items = lib.mkOption {
      type = lib.types.attrsOf (
        lib.types.submodule {
          options = {
            name = lib.mkOption { type = lib.types.str; };
            detail = lib.mkOption {
              type = lib.types.str;
              default = "";
            };
            unitAmount = lib.mkOption {
              type = lib.types.ints.between 50 1000000;
              description = "Price in minor units of `market.currency`.";
            };
          };
        }
      );
      # Placeholder prices. The Lab counts `box` and `gateway` in a setup.
      default = {
        box = {
          name = "LosOS box";
          detail = "x86_64 mini PC, 16 GB RAM, 1 TB SSD, LosOS installed";
          unitAmount = 44900;
        };
        gateway = {
          name = "LosOS edge gateway";
          detail = "x86_64 mini PC, 16 GB RAM, 1 TB SSD, edge gateway installed";
          unitAmount = 44900;
        };
      };
      visible = false;
      description = "What the edge sells, by sku.";
    };

    # ── Widget builder (edge side) ──────────────────────────────────────────
    # An owner describes a widget in the admin UI and a Claude Managed Agent
    # writes it. The edge holds the Anthropic key and pays Anthropic; the box
    # pays the edge from a prepaid balance topped up through the market's
    # Stripe gate. Off by default; see handbook/docs/in-depth/widget-builder.md.
    edge.builder.enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Serve the /builder/* routes of losos-registrar. Needs the market
        (top-ups are paid through its Stripe gate), the agent and environment
        made once by `losos-registrar builder-setup`, and the Anthropic key
        sealed at `claudeKeySealed` before the switch: the registrar unit
        loads the blob and does not start without it.
      '';
    };

    edge.builder.claudeKeySealed = lib.mkOption {
      type = secretPath;
      default = "/var/secrets/losos-claude-api-key.cred";
      description = ''
        The Anthropic API key (`sk-ant-...`), **sealed** with `systemd-creds`
        under the credential name `claude-api-key`. The registrar unit receives
        it through `LoadCredentialEncrypted=` and reads it per request, so it
        is plaintext only in that unit's credential tmpfs. Boxes never see
        it. Give it a workspace of its own with a spend limit in the Claude
        Console: that limit, not this edge, is the last word on what a bug
        can cost.
      '';
    };

    edge.builder.agentId = lib.mkOption {
      type = lib.types.nullOr (lib.types.strMatching "agent_[A-Za-z0-9]+");
      default = null;
      example = "agent_011CZkYpogX7uDKUyvBTophP";
      description = "The agent `losos-registrar builder-setup` printed. Required with `enable`.";
    };

    edge.builder.environmentId = lib.mkOption {
      type = lib.types.nullOr (lib.types.strMatching "env_[A-Za-z0-9]+");
      default = null;
      example = "env_011CZkZ9X2dpNyB7HsEFoRfW";
      description = "The environment `losos-registrar builder-setup` printed. Required with `enable`.";
    };

    edge.builder.markupBps = lib.mkOption {
      type = lib.types.ints.between 0 10000;
      default = 2000;
      description = ''
        What the edge adds to Anthropic's list price per million input and
        output tokens, in basis points: 2000 is 20%. Container time is not
        passed on.
      '';
    };

    edge.builder.usdRate = lib.mkOption {
      type = lib.types.strMatching "[0-9]+(\\.[0-9]{1,6})?";
      default = "1.0";
      description = ''
        Units of `market.currency` per US dollar. Anthropic prices in USD and
        the market charges in its own currency; 1.0 is right for `usd` and
        a rounded figure for the others.
      '';
    };

    edge.builder.packs = lib.mkOption {
      type = lib.types.addCheck (lib.types.listOf (lib.types.ints.between 100 50000)) (
        l: l != [ ] && lib.length l <= 6
      );
      default = [
        500
        1000
        2000
      ];
      description = "The top-ups an owner can buy, in minor units of `market.currency`. One to six.";
    };

    edge.builder.maxBuildCents = lib.mkOption {
      type = lib.types.ints.between 20 10000;
      default = 300;
      description = ''
        The most one build may spend at Anthropic's list price, in US cents.
        It becomes the session's hard budget, lowered to what the owner's
        balance covers.
      '';
    };

    # ── Virtual machines on the mesh (edge side) ────────────────────────────
    # KubeVirt and CDI on the edge's rke2 cluster, sold through the market by
    # the replica (backend-registrar/src/vms.rs,
    # handbook/docs/in-depth/virtual-machines.md). Boxes host them only while
    # they share their storage.
    edge.vms.enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Run virtual machines on the mesh and sell them on the market. Deploys
        the KubeVirt and CDI operators to the edge's rke2 cluster, a local
        storage class that keeps each machine's disk on the box that runs it,
        and the registrar's machine routes. A box hosts machines only while
        it shares its storage (losos.sharingMyStorage). Each sale is split
        evenly between the hosting box's owner and the platform. Needs
        losos.edge.market.enable.
      '';
    };

    edge.vms.useEmulation = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Run guests under software emulation when a hosting box has no
        /dev/kvm, for example a box that is itself a virtual machine without
        nested virtualisation. Guests then run many times slower. For demos
        only; a real box has hardware virtualisation.
      '';
    };

    edge.vms.cpu = lib.mkOption {
      type = lib.types.ints.between 1 16;
      default = 1;
      description = "Virtual CPUs of every replica.";
    };

    edge.vms.memoryMiB = lib.mkOption {
      type = lib.types.ints.between 512 65536;
      default = 2048;
      description = "Memory of every replica, in MiB. An image that needs more (LosOS asks for 4096) gets what it needs.";
    };

    edge.vms.diskGiB = lib.mkOption {
      type = lib.types.ints.between 1 512;
      default = 20;
      description = "Disk of every replica, in GiB. An image that needs more gets what it needs.";
    };

    edge.vms.hostPath = lib.mkOption {
      type = lib.types.strMatching "/[-_./a-zA-Z0-9]+";
      default = "/home/shared/vms";
      description = ''
        Where on a hosting box the replicas' disks live. The default is inside
        the shared user's home, the storage the box already lends the mesh.
      '';
    };

    edge.vms.domain = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      example = "vms.losos.cfd";
      description = ''
        Publish each machine order at `https://<order>.<domain>`, port 80 of
        its replicas behind the edge's Traefik. Needs a wildcard DNS record
        for `*.<domain>` pointing at this edge. Null publishes nothing; the
        machines are then reachable only inside the mesh.
      '';
    };

    edge.vms.uploadMaxGiB = lib.mkOption {
      type = lib.types.ints.between 1 512;
      default = 32;
      description = "The largest QCOW2 image a buyer may upload, in GiB. Uploads are kept in the registrar's state directory.";
    };

    # ── Edge on the LAN ─────────────────────────────────────────────────────
    # The appliance looks for an edge proxy before it lets anyone share
    # storage (backend/src/edge.rs): at losos.proxy.registrarUrl, and on its
    # own network by DNS-SD. This is the network half: the edge publishes
    # `_losos-edge._tcp` over mDNS with a `url=` TXT record naming its
    # registrar API, and the box's `avahi-browse` finds it. Off by default
    # because a production edge is a VPS with no LAN to speak of; on for an
    # edge that sits beside the boxes (a home server, the two-VM demo in
    # demo/edge-lan/), where it is what lets a box with no internet share.
    edge.lan.advertise = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Advertise this edge on the local network over mDNS/DNS-SD as
        `_losos-edge._tcp`, so appliances on the same LAN find it without
        any configuration. Also opens the registrar API port in the firewall
        and binds it off-loopback when losos.edge.registrarApiBind is still
        the loopback default: a box that found the edge dials the advertised
        URL directly, there being no Traefik hostname on a LAN.
      '';
    };

    edge.lan.url = lib.mkOption {
      type = lib.types.str;
      default = "http://${config.networking.hostName}.local:${toString config.losos.edge.registrarApiPort}";
      defaultText = lib.literalExpression ''"http://''${config.networking.hostName}.local:''${toString config.losos.edge.registrarApiPort}"'';
      description = ''
        The registrar API URL the advertisement carries (`url=` TXT record).
        Appliances probe `<url>/health` before they count the edge as found,
        so it must be reachable from the LAN as written. The default is the
        edge's own mDNS name, resolvable by every appliance (they run Avahi's
        NSS module); set an IP literal when the LAN's mDNS is unreliable.
      '';
    };

    # ── DNS and custom domains (official edges) ──────────────────────────
    # The edge serves one zone itself (Knot), and losos-registrar writes it:
    # one name per box whose Stripe connected account is ready and carries
    # the box's UUID, plus every tenant hostname that falls inside the zone.
    # An owner points a domain of their own at that name and proves it with a
    # TXT record; the edge then routes and certifies the domain. The whole of
    # it only on an edge that holds an identity certificate, and the box
    # names only with the market on (that is where the Stripe data lives).
    # See backend-registrar/src/domains.rs and
    # handbook/docs/in-depth/master-proxy.md.
    edge.dns.enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Serve the zone `edge.dns.zone` from this edge (Knot on port 53) and
        let boxes bring their own domains (`/domains/*` on the registrar).
        Needs `edge.dns.ipv4` and/or `edge.dns.ipv6`, an identity
        certificate (the routes answer 503 until one is installed), and the
        parent zone delegating `edge.dns.zone` to `edge.dns.nameservers`.
      '';
    };

    edge.dns.zone = lib.mkOption {
      type = lib.types.str;
      default = "boxes.${config.losos.edge.publicDomain}";
      defaultText = lib.literalExpression ''"boxes.''${config.losos.edge.publicDomain}"'';
      description = ''
        The zone this edge is authoritative for. Each box Stripe has checked
        gets `<label>.<zone>`, a 16-hex-character label derived from its box
        UUID; owners point their own domain at it. Set it to `publicDomain`
        itself to have the edge assign every tenant hostname as well, in
        place of a hand-made wildcard record.
      '';
    };

    edge.dns.nameservers = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ "ns1.${config.losos.edge.dns.zone}" ];
      defaultText = lib.literalExpression ''[ "ns1.''${config.losos.edge.dns.zone}" ]'';
      description = ''
        The zone's NS names. One inside the zone gets the edge's addresses as
        glue; the parent zone needs the same NS records (and glue) to
        delegate to this edge.
      '';
    };

    edge.dns.hostmaster = lib.mkOption {
      type = lib.types.str;
      default = "hostmaster.${config.losos.edge.dns.zone}";
      defaultText = lib.literalExpression ''"hostmaster.''${config.losos.edge.dns.zone}"'';
      description = "SOA contact mailbox, written as a domain (`hostmaster.example.org` for hostmaster@example.org).";
    };

    edge.dns.ipv4 = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      example = [ "203.0.113.7" ];
      description = "The edge's public IPv4 addresses. Every name in the zone resolves to these.";
    };

    edge.dns.ipv6 = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      example = [ "2001:db8::7" ];
      description = "The edge's public IPv6 addresses. Every name in the zone resolves to these.";
    };

    edge.dns.listen = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [
        "0.0.0.0@53"
        "::@53"
      ];
      description = "Knot `server.listen` addresses (`address@port`).";
    };

    edge.dns.checkUrl = lib.mkOption {
      type = lib.types.str;
      default = "https://cloudflare-dns.com/dns-query";
      description = ''
        DNS-over-HTTPS endpoint (JSON API) the registrar looks an owner's
        TXT and CNAME records up with. A public resolver sees the domain
        names being checked; point this at your own if that matters.
      '';
    };

    # Custom domains for boxes behind a local edge
    # (handbook/docs/in-depth/edge-federation.md, "Custom domains behind a local
    # edge"). The official edge keeps the route table in the registrar's own
    # state and sends a domain to the local edge whose box proved it, once the
    # box has vouched for that local edge with a relay pass and joined this
    # edge's mesh. See backend-registrar/src/routes.rs.
    edge.dns.relayRoutes.enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Route custom domains to boxes that reach this edge through a local
        edge (a spoke relaying under losos.edge.tenants.<id>.relayZone).
        The registrar keeps the route table itself, in
        /var/lib/losos-registrar/relay-routes.json beside its registry.
        Only a box in this edge's mesh is routed, and only through the local
        edge it named with its relay pass. Needs edge.dns.enable.
      '';
    };

    edge.lan.ratholeEndpoint = lib.mkOption {
      type = lib.types.str;
      default = "${config.networking.hostName}.local:${toString config.losos.edge.ratholeBindPort}";
      defaultText = lib.literalExpression ''"''${config.networking.hostName}.local:''${toString config.losos.edge.ratholeBindPort}"'';
      description = ''
        The `host:port` a box on the LAN dials its tunnel to, carried by the
        advertisement as the `rathole=` TXT record beside `url=`. A box that
        picks this edge as its path (backend/src/edge.rs) points its rathole
        client here instead of at losos.proxy.edgeRatholeEndpoint. Same
        default and same caveat as losos.edge.lan.url.
      '';
    };

    edge.lan.openEnrolment = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Enrol unknown appliances trust-on-first-use: an id not in
        losos.edge.tenants that registers is accepted with the token and
        hostname it presented, both kept under
        /var/lib/losos-registrar/enrolled/, and every later request for that id
        must carry the same token. An enrolled box gets a route and nothing
        else, never `cluster` or `market`. What makes a stock box work with a
        gateway nobody provisioned tokens for
        (handbook/docs/in-depth/edge-federation.md), and exactly as dangerous as
        it sounds on an edge the internet can reach, so it requires
        losos.edge.lan.advertise and is refused without it. `losos-registrar
        enrol list|forget --dir /var/lib/losos-registrar/enrolled` on the edge
        shows and drops enrolled boxes; a box that is still heartbeating
        re-enrols with the token it holds, so forgetting is for a box that left.
      '';
    };

    # ── Uplink: this edge as a spoke of another ─────────────────────────────
    # handbook/docs/in-depth/edge-federation.md. A spoke keeps everything it has
    # and adds one outbound tunnel plus one `POST /relay` loop to a hub, which
    # must list it as a tenant with a relayZone. The registrar reads a JSON file
    # for all of it (`--uplink-file`), rendered from the options below, or
    # pointed at a runtime path (`configFile`) so one gateway image can serve
    # every site.
    edge.uplink.enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Relay the boxes registered here to a hub edge: the registrar posts
        the live tenant list to `<registrarUrl>/relay` every `interval` and
        keeps /etc/rathole/uplink.toml, a rathole *client* config with one
        service per relayed box, which `losos-rathole-uplink.service` runs.
        The hub must name this edge in its losos.edge.tenants with a
        relayZone; the boxes here must be named one label under that zone.
      '';
    };

    edge.uplink.configFile = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      example = "/var/lib/losos-edge/uplink.json";
      description = ''
        Read the uplink target from this runtime file instead of rendering
        one from the options below (fields: registrar_url, rathole_endpoint,
        id, token_file, optional bootstrap_token_file and
        noise_public_key_file; paths, never secrets). The registrar re-reads
        it on every pass, and a missing file means "no uplink yet", so a
        gateway image can ship with the uplink enabled and let its owner
        fill the file in later with `losos-edge uplink set`.
      '';
    };

    edge.uplink.registrarUrl = lib.mkOption {
      type = lib.types.str;
      default = "";
      example = "https://register.losos.cfd";
      description = "The hub's registrar API base URL. Required unless configFile is set.";
    };

    edge.uplink.ratholeEndpoint = lib.mkOption {
      type = lib.types.str;
      default = "edge.losos.cfd:2333";
      description = "The hub's rathole server, `host:port`, that the uplink client dials.";
    };

    edge.uplink.id = lib.mkOption {
      type = lib.types.str;
      default = "";
      example = "acme";
      description = "This edge's tenant id on the hub (the attribute name under the hub's losos.edge.tenants). A DNS label. Required unless configFile is set.";
    };

    edge.uplink.tokenFile = lib.mkOption {
      type = secretPath;
      default = "/var/secrets/losos-uplink-token";
      description = ''
        0600 file holding this edge's tenant token on the hub (the hub's
        losos.edge.tenants.<id>.tokenFile). Authenticates `/relay` and is the
        token of every relayed rathole service. Provision out of band.
      '';
    };

    edge.uplink.bootstrapTokenFile = lib.mkOption {
      type = lib.types.nullOr secretPath;
      default = null;
      description = ''
        0600 file holding the hub's rathole bootstrap token, for the client's
        `default_token`. Optional: every relayed service carries the uplink
        token, so `default_token` is never consulted and the uplink token
        stands in when this is null.
      '';
    };

    edge.uplink.noisePublicKeyFile = lib.mkOption {
      type = lib.types.nullOr secretPath;
      default = "/var/secrets/losos-uplink-noise-pub";
      description = ''
        Where the hub's rathole Noise public key is pinned. Nothing to
        provision: when the file is absent the registrar fetches
        `<registrarUrl>/noise-public-key` once and writes it (trust on first
        contact, as a box pins its edge); a file already there is never
        overwritten. `null` runs the uplink in plain TCP.
      '';
    };

    edge.uplink.interval = lib.mkOption {
      type = lib.types.str;
      default = "30s";
      description = "How often the spoke posts `/relay` to the hub. Must be well under the hub's losos.edge.heartbeatTtl: the hub drops a silent spoke's boxes with the spoke.";
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
  # ── Federation, as it actually runs ─────────────────────────────────────
  # The one place the per-service switch meets the sharing gate, so the
  # modules that act on it (services.nix, workloads.nix, containers.nix,
  # flake/images.nix through the rendered config) cannot disagree about
  # whether a box federates. Read-only: set losos.<service>.federation.enable
  # or losos.sharingMyStorage instead.
  options.lososInternal.federation = {
    forgejo = lib.mkOption {
      type = lib.types.bool;
      internal = true;
      readOnly = true;
      default = config.losos.forgejo.federation.enable && config.losos.sharingMyStorage;
      description = "LosOS Git federates: losos.forgejo.federation.enable and losos.sharingMyStorage.";
    };
    nextcloud = lib.mkOption {
      type = lib.types.bool;
      internal = true;
      readOnly = true;
      default = config.losos.nextcloud.federation.enable && config.losos.sharingMyStorage;
      description = "LosOS cloud federates: losos.nextcloud.federation.enable and losos.sharingMyStorage.";
    };
  };

  # Whether this box hosts machines, as it actually runs: the switch and
  # the sharing gate together, read by modules/cluster.nix for the join.
  options.lososInternal.vmHost = lib.mkOption {
    type = lib.types.bool;
    internal = true;
    readOnly = true;
    default = config.losos.vms.host && config.losos.sharingMyStorage;
    description = "This box hosts mesh virtual machines: losos.vms.host and losos.sharingMyStorage.";
  };

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
        "losos.edge.market.stripeSecretKeySealed" = config.losos.edge.market.stripeSecretKeySealed;
        "losos.edge.market.webhookSecretSealed" = config.losos.edge.market.webhookSecretSealed;
        "losos.edge.builder.claudeKeySealed" = config.losos.edge.builder.claudeKeySealed;
        "losos.edge.uplink.tokenFile" = config.losos.edge.uplink.tokenFile;
      }
      // lib.optionalAttrs (config.losos.edge.uplink.bootstrapTokenFile != null) {
        "losos.edge.uplink.bootstrapTokenFile" = config.losos.edge.uplink.bootstrapTokenFile;
      }
      // lib.optionalAttrs (config.losos.edge.noisePrivateKeyFile != null) {
        "losos.edge.noisePrivateKeyFile" = config.losos.edge.noisePrivateKeyFile;
      }
      // lib.optionalAttrs (config.losos.edge.identity.keyFile != null) {
        "losos.edge.identity.keyFile" = config.losos.edge.identity.keyFile;
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
