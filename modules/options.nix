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

  # The release this tree is (nix/version.nix). A box follows the daily
  # channel until LosOS is at 1.0.0 or later, without a pre-release suffix,
  # and the weekly one from then on; version.yml moving the tag is what
  # flips it.
  release = lib.removePrefix "v" (import ../nix/version.nix);
  stableRelease = !(lib.hasInfix "-" release) && lib.versionAtLeast release "1.0.0";
in
{
  # LosOS Git took over from Forgejo; an older overrides.nix still names
  # the settings by the old prefix, and keeps evaluating: each old name is a
  # hidden alias of the new one. No warning, because nix/options-doc.nix
  # reads every option and would print it on every rebuild.
  #
  # The edge tunnel moved from rathole to QUIC (edge/registrar/src/tunnel.rs)
  # and its settings from `rathole` names to `tunnel` ones, aliased the same
  # way. The rathole transport's secrets (bootstrap token, Noise keys) have
  # no successor: the QUIC tunnel authenticates the edge by its pinned
  # certificate and each service by the tenant token.
  imports =
    let
      alias =
        from: to:
        lib.doRename {
          from = [ "losos" ] ++ from;
          to = [ "losos" ] ++ to;
          visible = false;
          warn = false;
          use = x: x;
          withPriority = true;
        };
      gone =
        path:
        lib.mkRemovedOptionModule ([ "losos" ] ++ path)
          "The edge tunnel is QUIC now and authenticates the edge by its pinned certificate; drop this setting.";
    in
    map (path: alias ([ "forgejo" ] ++ path) ([ "git" ] ++ path)) [
      [ "enable" ]
      [
        "site"
        "requireSignIn"
      ]
      [
        "site"
        "defaultPrivate"
      ]
      [
        "site"
        "pushCreate"
      ]
    ]
    ++ [
      (alias [ "proxy" "edgeRatholeEndpoint" ] [ "proxy" "edgeTunnelEndpoint" ])
      (alias [ "edge" "ratholeBindAddr" ] [ "edge" "tunnelBindAddr" ])
      (alias [ "edge" "ratholeBindPort" ] [ "edge" "tunnelBindPort" ])
      (alias [ "edge" "ratholePortRange" ] [ "edge" "tunnelPortRange" ])
      (alias [ "edge" "lan" "ratholeEndpoint" ] [ "edge" "lan" "tunnelEndpoint" ])
      (alias [ "edge" "uplink" "ratholeEndpoint" ] [ "edge" "uplink" "tunnelEndpoint" ])
    ]
    ++ map gone [
      [
        "edge"
        "bootstrapTokenFile"
      ]
      [
        "edge"
        "noisePrivateKeyFile"
      ]
      [
        "edge"
        "rathole"
        "package"
      ]
      [
        "edge"
        "uplink"
        "bootstrapTokenFile"
      ]
      [
        "edge"
        "uplink"
        "noisePublicKeyFile"
      ]
    ];

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
        exists, and nix/tests/invariants.nix asserts the two agree: the published
        flake (no modules/install-target.nix) must describe a box the ISO
        actually produces. A box the installer put in keyfile mode has that
        recorded in install-target.nix, and flake.nix reads that file from
        /etc/nixos under --impure (which the nightly upgrade and lososd pass)
        so a `github:` upgradeFlakeUri keeps it; without the flag such an
        upgrade would drop the keyfile from the initrd and stop the 00:07
        reboot at a passphrase prompt.
      '';
    };

    tpm.tang = lib.mkOption {
      type = lib.types.listOf (
        lib.types.submodule {
          options = {
            url = lib.mkOption {
              type = lib.types.strMatching "https?://[^[:space:]\"'$`\\]+";
              example = "http://tang.lan:7500";
              description = "The Tang server's address, reachable from the box's own network.";
            };
            thumbprint = lib.mkOption {
              type = lib.types.strMatching "[A-Za-z0-9_-]{20,}";
              description = ''
                The thumbprint of the Tang server's signing key (`tang-show-keys`
                on the server). The installer binds to that key and no other, so
                a server that answers at the address with another key unlocks
                nothing.
              '';
            };
            edge = lib.mkOption {
              type = lib.types.bool;
              default = false;
              description = ''
                This server is the box's edge (`losos-install --tang-edge`,
                losos.edge.tang), at `<registrar>/tang/<id>`: a key made for
                this box alone, which the edge stops answering for once the
                owner reports the box stolen there. It is required on its
                own, not one of the servers any one of which will do.
              '';
            };
          };
        }
      );
      default = [ ];
      example = [
        {
          url = "http://tang.lan:7500";
          thumbprint = "x3ZDsT7uAzTcJiJ9qZ7f0wJ8Kq0pEJ2O5b5b7l7XyZk";
        }
      ];
      description = ''
        Unlock /persist only on the box's own network. Empty (the default):
        the TPM2 alone opens the volume, so a box carried away still boots
        with its data readable. With one or more Tang servers, the installer
        seals the disk key to the TPM2 AND any one of these servers (Clevis,
        `sss` with a threshold of two) instead of the TPM alone, and the
        initrd asks a server over the network at every boot. A box taken off
        its network, or whose servers have forgotten it, does not open; at
        the console it then asks for the box's recovery code, which the box
        adds as a passphrase once it has one.

        With the box's edge as a server (`losos-install --tang-edge`), the
        disk also needs the edge's answer, and the owner can make the edge
        refuse it from the edge's `/tang/lost` page with the box's recovery
        code: a stolen box then does not open anywhere.

        Set by the installer (`losos-install --tang URL#THUMBPRINT`,
        `--tang-edge [URL]`) into install-target.nix, like `tpm.enable`.
        Needs `tpm.enable`.
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
        LosOS Cloud login crossing the LAN in clear text, which
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
    # (edge/vercel/public/find.html) uses that to read `/setup/state.json`
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
      # The local cache server fetches signed per-store-path OCI artifacts
      # from GHCR with oras. GitHub Pages is a static fallback if GHCR is
      # unreachable. Neither the installer nor the installed appliance needs
      # a hosted cache proxy.
      default = [
        "http://127.0.0.1:8787"
        "https://losos.dasmat.us/proxy"
      ];
      description = ''
        Extra Nix substituters, added to the appliance *and* to the installer
        medium. The local `http://127.0.0.1:8787` service pulls signed
        per-store-path artifacts from GHCR with `oras`; the static copy of the
        newest build on GitHub Pages (`https://losos.dasmat.us/proxy`) is a
        remote fallback. `trustedPublicKeys` carries the key used to sign
        those paths. Set this to `[ ]` to use cache.nixos.org alone.

        This exists because `losos-ctl install` runs a bare `nixos-install`
        with no substituter flags, and the install closure carries the
        workload images and the LosOS packages, none of which cache.nixos.org
        has. The local server lets Nix pull those artifacts directly from GHCR
        without depending on a separately hosted cache proxy.
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

    # ── Local copy of the binary cache ──────────────────────────────────────
    # modules/cache.nix. A caching proxy on the box that every Nix download
    # goes through, keeping what it fetched under /var/cache (on /persist).
    cache.local.enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Keep a copy of every store path the box downloads, so it never fetches
        the same one twice. Nix on the box substitutes from a local proxy
        (`http://127.0.0.1:8790` by default) that
        asks `losos.cache.local.peers` and then `losos.cache.local.upstreams`
        and keeps each answer in `/var/cache/losos-nix-cache`, which lives on
        `/persist`. Nix still checks every signature against
        `nix.settings.trusted-public-keys`; the proxy changes where a path comes
        from, never what is accepted.

        It pays off on the nightly rebuild: the clean-up at 04:30 deletes the
        build tools the last rebuild fetched, and the next one fetches them
        again. On by default on the box; off on the installer medium.
      '';
    };

    cache.local.port = lib.mkOption {
      type = lib.types.port;
      default = 8790;
      description = "Port of the local cache proxy, on loopback (and on the LAN with `losos.cache.local.shareOnLan`).";
    };

    cache.local.maxSize = lib.mkOption {
      type = lib.types.strMatching "[1-9][0-9]*[kmg]";
      default = "8g";
      example = "20g";
      description = ''
        Most disk space the local copy may take, as nginx sizes (`512m`,
        `8g`). Past it the least recently used paths are deleted first.
      '';
    };

    cache.local.minFree = lib.mkOption {
      type = lib.types.strMatching "[1-9][0-9]*[kmg]";
      default = "4g";
      description = ''
        Free space the local copy always leaves on `/persist`. When the disk
        has less, the least recently used paths are deleted until it has this
        much again, whatever `maxSize` says. Your files come first.
      '';
    };

    cache.local.upstreams = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ "https://cache.nixos.org" ] ++ config.losos.cache.substituters;
      defaultText = lib.literalExpression ''[ "https://cache.nixos.org" ] ++ config.losos.cache.substituters'';
      description = ''
        Binary caches the local proxy asks, in this order, for a path it does
        not have. The first that has it wins. Every one is `http://` or
        `https://`, with an optional path.
      '';
    };

    cache.local.shareOnLan = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Answer other machines on the local network too, so several boxes in
        one home download each update once. Only private (RFC 1918) addresses
        are let in. What the copy holds is public software, signed by the
        caches it came from; a box that uses it still checks every signature.
      '';
    };

    cache.local.peers = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      example = [ "http://other-box.local:8790" ];
      description = ''
        Local copies on other boxes (`shareOnLan` on there) to ask before
        `upstreams`. A path one of them has is fetched over the LAN. A peer
        that does not have a path fetches it from its own upstreams and keeps
        it, so the next box finds it there; it never asks its own peers on
        your behalf, which keeps two boxes that name each other out of a loop.
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
    # opt-in. Every flag is asserted in nix/tests/hardening.nix, which also asserts
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
        linked processes (nginx, resolved, lososd) and neither the static Go
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
        default because the box makes previews for LosOS Cloud and may run
        mesh compute for strangers; turn it on if that second one worries you
        more than the first.
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
    # out to the edge over a QUIC tunnel (no inbound public port) and
    # announces itself to the edge's losos-registrar, which rewrites Traefik
    # and the tunnel's services there. See docs/superpowers/specs/2026-08-17-master-proxy-design.md
    # and modules/proxy.nix. The edge side of the contract is losos.edge.*
    # (below). The on-box Nginx stays the slave proxy on :80.
    proxy.enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Enable the master-proxy appliance side: a QUIC tunnel client that
        dials out to the edge (losos.proxy.edgeTunnelEndpoint) and a
        losos-registrar announce service that registers/heartbeats this
        appliance with the edge. The on-box Nginx stays the slave proxy on
        :80; the tunnel delivers each connection to it. No inbound public port is
        opened — the no-SSH/no-public-ports invariant is preserved.
      '';
    };

    proxy.edgeTunnelEndpoint = lib.mkOption {
      type = lib.types.str;
      default = "edge.losos.cfd:2333";
      description = "host:port the tunnel client dials, QUIC on UDP or, where UDP is blocked, TLS on TCP: the edge's losos.edge.tunnelBindPort.";
    };

    proxy.bansFile = lib.mkOption {
      type = lib.types.str;
      default = "/var/lib/losos/edge-bans.json";
      description = ''
        The ban list lososd reads on every edge scan,
        `{"edges": [<url>], "clusters": [<official cluster>]}`. A banned
        edge is never chosen as the path, and a box on one moves to another
        edge of the same tier; a ban also breaks the hold of a custom
        domain, which follows the box. A missing file bans nothing.
      '';
    };

    proxy.registrarUrl = lib.mkOption {
      type = lib.types.str;
      # The demo edge (edge/vercel/, the registrar as a Vercel Function at the
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
      description = "Stable appliance id; the registry key and the tunnel service name.";
    };

    proxy.tokenFile = lib.mkOption {
      type = secretPath;
      default = "/var/secrets/losos-proxy-token";
      description = ''
        Per-appliance shared secret (0600, persisted via /var). Used both to
        authenticate /register and as the tunnel service token. Must match
        losos.edge.tenants.<applianceId>.tokenFile on the edge. Provision out
        of band (agenix or a manual write), not via the nix store.
      '';
    };

    proxy.tunnelPinFile = lib.mkOption {
      type = secretPath;
      default = "/var/secrets/losos-tunnel-pin";
      description = ''
        Where the appliance keeps the SHA-256 of the configured edge's
        tunnel certificate (64 hex characters). Nothing to provision: before
        the tunnel first dials, losos-tunnel-client fetches it from
        losos.proxy.registrarUrl and keeps it here, and the tunnel accepts
        that certificate and no other from then on (trust on first use, over
        the registrar's TLS). A file already present is never overwritten,
        so a pin distributed out of band pins the edge from the start. An
        edge found on the LAN is pinned the same way into its own file under
        /var/secrets/losos-edge-pins/ (lososd names it in
        /run/losos/edge-path.env).
      '';
    };

    proxy.tlsPassthrough = {
      enable = lib.mkOption {
        type = lib.types.bool;
        default = true;
        description = ''
          End TLS for this box's public names on the box itself, with a
          Let's Encrypt certificate it gets for each, so the edge passes the
          encrypted bytes through and cannot read them (modules/public-tls.nix,
          edge/registrar/src/passthrough.rs). Off, the edge ends TLS with a
          certificate of its own and can read everything that goes through it.
          On, a switched-off box is a connection error rather than the edge's
          offline page, and its public names have no HTTP/3 or ECH.
        '';
      };
      acmeServer = lib.mkOption {
        type = lib.types.str;
        default = "https://acme-v02.api.letsencrypt.org/directory";
        description = "The ACME directory the box gets its public certificates from.";
      };
      acmeEmail = lib.mkOption {
        type = lib.types.nullOr lib.types.str;
        default = null;
        example = "owner@example.org";
        description = "The contact address given to the ACME server, if any. None is sent by default.";
      };
    };

    proxy.pauseSharingAfter = lib.mkOption {
      type = lib.types.ints.unsigned;
      default = 600;
      example = 1800;
      description = ''
        Seconds without any edge answering before a box that shares storage
        through one turns sharing off by itself, as the owner's switch
        would (backend/src/edge.rs, `sharing_step`). Once an edge has
        answered for a minute, the box turns sharing back on. Only a pause
        the box made itself is undone; the owner switching sharing in the
        meantime keeps their choice. `0` never pauses.
      '';
    };

    proxy.gateways = {
      tpmRoots = lib.mkOption {
        type = lib.types.path;
        default = "${import ../nix/gateway/tpm-roots.nix { inherit pkgs; }}/tpm-roots.pem";
        defaultText = lib.literalExpression ''"''${import ../nix/gateway/tpm-roots.nix { inherit pkgs; }}/tpm-roots.pem"'';
        description = ''
          The TPM makers' certificates (PEM; self-signed ones are roots, the
          rest intermediates) a box checks an edge's TPM against before it
          trusts the edge with trading (backend/src/edge.rs, "gateways").
          The default is Microsoft's TrustedTpm bundle.
        '';
      };
      images = lib.mkOption {
        type = lib.types.listOf (
          lib.types.submodule {
            options = {
              name = lib.mkOption {
                type = lib.types.str;
                description = "The image's name, shown beside a trusted edge.";
              };
              pcr11 = lib.mkOption {
                type = lib.types.strMatching "[0-9a-fA-F]{64}";
                description = "The SHA-256 PCR 11 its unified kernel image leaves, hex.";
              };
              apps = lib.mkOption {
                type = lib.types.listOf (lib.types.strMatching "[0-9a-fA-F]{64}");
                description = "Authenticode SHA-256 of every program the firmware may start for it, hex.";
              };
            };
          }
        );
        default = import ../nix/gateway/images.nix;
        defaultText = lib.literalExpression "import ../nix/gateway/images.nix";
        description = ''
          The published LosOS gateway images. A box trusts an edge with
          trading only when the edge's TPM quote shows one of them booted,
          its owner's passport holds, its fee addresses are LosOS's
          (`payouts`) and nothing changed since it booted. An edge that
          proves tampered is disabled on the box for good
          (/var/lib/losos/gateways.json). Empty, no edge is trusted with
          trading; storage sharing works through any edge regardless.
        '';
      };
      payouts = lib.mkOption {
        type = lib.types.attrsOf lib.types.str;
        default = import ../nix/gateway/payouts.nix;
        defaultText = lib.literalExpression "import ../nix/gateway/payouts.nix";
        example = {
          base = "0x0000000000000000000000000000000000000000";
        };
        description = ''
          LosOS's payout address on each network, where a gateway sends
          LosOS's 2% of a sale. A gateway whose measured settings name any
          other address, or another split, is not trusted. Empty, no gateway
          is.
        '';
      };
      domain = {
        enable = lib.mkOption {
          type = lib.types.bool;
          default = true;
          description = ''
            Hold a gateway that passes every other check to its name: it is
            trusted only at an https:// URL whose HTTPS record validates
            under DNSSEC and carries an Encrypted Client Hello key, which
            the box checks with `delv` on every scan. Off, the name is not
            looked at.
          '';
        };
        server = lib.mkOption {
          type = lib.types.nullOr lib.types.str;
          default = null;
          example = "192.0.2.53";
          description = ''
            The resolver `delv` asks for a gateway's name. `null` is the
            system's resolver. delv validates the answer itself either way.
          '';
        };
        trustAnchors = lib.mkOption {
          type = lib.types.nullOr lib.types.str;
          default = null;
          description = ''
            A `trust-anchors { … };` file for `delv -a`. `null` is delv's
            own copy of the DNS root's key, which is what a real gateway's
            name validates from.
          '';
        };
        trustAnchorRoot = lib.mkOption {
          type = lib.types.nullOr lib.types.str;
          default = null;
          example = "boxes.example.test";
          description = ''
            The name `trustAnchors` holds a key for, when that is not the
            DNS root (`delv +root=`).
          '';
        };
      };
    };

    proxy.heartbeatInterval = lib.mkOption {
      type = lib.types.str;
      default = "30s";
      description = "Cadence losos-registrar announce POSTs /heartbeat. Must be well under losos.edge.heartbeatTtl.";
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

    # ── Tor and I2P (modules/hidden-services.nix) ─────────────────────────
    hidden.tor.enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Give the box an address on Tor, a `.onion` name people open in Tor
        Browser. It reaches what the box's public name reaches through an
        edge, LosOS Cloud, LosOS Git and LosOS Login, never the admin pages,
        and needs no edge and no open port: the box connects out to Tor. The
        address appears on the Network page once Tor has made it, and stays
        the same across reboots.
      '';
    };

    hidden.i2p.enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Give the box an address on I2P, a `.b32.i2p` name people open
        through an I2P router. It reaches the same pages as the Tor address
        and, like it, needs no edge and no open port. The box takes part in
        I2P as a router with a small bandwidth share.
      '';
    };

    # ── Compute mesh (appliance side) ───────────────────────────────────────
    # Two Kubernetes instances run on the appliance and they are deliberately
    # different clusters. See
    # docs/superpowers/specs/2026-09-09-k3s-mesh-design.md.
    #
    #   LOCAL  services.k3s, role=server, /var/lib/rancher/k3s — runs THIS
    #          box's `shared` apps from the catalogue, while the box shares
    #          and is in a mesh. It has no off-box dependency, so the appliance
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
        cloud, Git and apps — depend on the edge being reachable.
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
        This box's OWN cloud, Git and apps are unaffected in either case:
        they live on the box and in the local cluster, which has no taint
        and no scheduler.
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

    apps.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Let the owner install apps: charts found in the catalogue search
        and apps in the LosOS app format, each running as `notshared` or
        `shared`, without root, with its files in that user's data folder.
        A `notshared` app runs on rootless Podman as that user; a `shared`
        one runs in this box's own cluster, so only while the box shares
        and is in a mesh (lososInternal.sharedSide).
      '';
    };

    apps.builtin = lib.mkOption {
      type = lib.types.attrsOf (
        lib.types.submodule {
          options = {
            compose = lib.mkOption {
              type = lib.types.attrs;
              description = ''
                The app's Compose file as Nix, in the LosOS app format
                (handbook/docs/in-depth/app-format.md). `x-losos.port` is
                the port its page listens on, and `x-losos.images` the image
                archives the box loads before it starts the app.
              '';
            };
            files = lib.mkOption {
              type = lib.types.nullOr lib.types.path;
              default = null;
              description = "A folder of files the app ships, mounted by `./` paths in the Compose file.";
            };
            path = lib.mkOption {
              type = lib.types.nullOr lib.types.str;
              default = null;
              example = "/kanban/";
              description = ''
                Where the front door serves the app, with a slash at each
                end, proxied to `x-losos.port` with that prefix stripped.
                Null serves it nowhere.
              '';
            };
            locationConfig = lib.mkOption {
              type = lib.types.lines;
              default = "";
              description = "More nginx configuration for that location (sign-in headers, limits).";
            };
            lanOnly = lib.mkOption {
              type = lib.types.bool;
              default = true;
              description = ''
                Serve the path to the LAN only, under the guard the admin
                routes carry. False serves it wherever the front door is
                reached, the tunnel included.
              '';
            };
          };
        }
      );
      default = { };
      visible = false;
      description = ''
        The box's own apps, run as `notshared` on rootless Podman like an
        installed app and started at every boot. Declared by the modules
        that ship them, never from the Advanced pane.
      '';
    };

    apps.ports.from = lib.mkOption {
      type = lib.types.port;
      default = 30000;
      description = ''
        First port of the range that installed apps are opened on, one port
        per app, reachable from the LAN only.
      '';
    };

    apps.ports.to = lib.mkOption {
      type = lib.types.port;
      default = 30099;
      description = "Last port of the range in losos.apps.ports.from.";
    };

    apps.traefikPort = lib.mkOption {
      type = lib.types.port;
      default = 8089;
      description = ''
        Loopback port of the Traefik that opens the installed apps on their
        LAN ports: its API and metrics, read by the install job and by the
        Statistics page. Never reachable from the LAN.
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
    # this flake's own derivations (nix/images.nix). Internal: they are an
    # implementation detail of container mode, not a knob.
    #
    # They are listed in the k3s instance's `images` option, which symlinks each
    # into /var/lib/rancher/k3s/agent/images for the agent to import, so the
    # pods can run with imagePullPolicy: Never and nothing is fetched from a
    # container registry at runtime.
    #
    # The images themselves are NOT built on the appliance: an image is
    # hundreds of MiB, and system.autoUpgrade would rebuild it at 03:00 on a mini-PC
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

    # ── The credential manager (modules/credentials.nix) ────────────────────
    # Every secret the box keeps, one systemd-creds file each in
    # /etc/credstore.encrypted, sealed to the TPM or the host key, opened by
    # PID 1 into the credential directory of each unit that reads it. Modules
    # declare their own; the Credentials pane (backend/src/credentials.rs)
    # lists them and makes new values. Kept off the Advanced pane
    # (nix/options-doc.nix): these are wiring, not settings.
    credentials.entries = lib.mkOption {
      default = { };
      description = ''
        The box's credentials, by name. The name is the systemd credential
        name: the file in /etc/credstore.encrypted, the name sealed into it,
        and the file in each reader's credential directory.
      '';
      type = lib.types.attrsOf (
        lib.types.submodule {
          options = {
            app = lib.mkOption {
              type = lib.types.str;
              description = "What it belongs to, for the pane's groups: login, mail, admin, backup, cloud, ...";
            };
            description = lib.mkOption {
              type = lib.types.str;
              description = "One sentence on what it is for.";
            };
            format = lib.mkOption {
              type = lib.types.nullOr (
                lib.types.enum [
                  "hex-32"
                  "hex-64"
                  "base64-24"
                  "rsa-2048"
                ]
              );
              default = null;
              description = ''
                How the box makes a value: 32 or 64 random bytes in hex, 24 in
                base64, or an RSA private key in PEM. Null for a value the owner gives, which lososd
                seals when they save the pane that asks for it (`pane`).
              '';
            };
            services = lib.mkOption {
              type = lib.types.listOf lib.types.str;
              default = [ ];
              description = "Units that read it; each gets `LoadCredentialEncrypted=<name>`.";
            };
            restart = lib.mkOption {
              type = lib.types.listOf lib.types.str;
              default = [ ];
              description = "Units restarted after a new value, when not just `services`.";
            };
            rotate = lib.mkOption {
              type = lib.types.bool;
              default = false;
              description = "Whether a person may make a new value from the Credentials pane.";
            };
            pane = lib.mkOption {
              type = lib.types.nullOr lib.types.str;
              default = null;
              description = "Where the owner sets a value they give, as a path of the admin pages.";
            };
            legacy = lib.mkOption {
              type = lib.types.listOf lib.types.str;
              default = [ ];
              description = ''
                Plaintext files an older build kept the value in. The first one
                found is adopted when nothing is sealed yet; every one is then
                overwritten and deleted.
              '';
            };
            legacySealed = lib.mkOption {
              type = lib.types.nullOr (
                lib.types.submodule {
                  options = {
                    path = lib.mkOption { type = lib.types.str; };
                    name = lib.mkOption { type = lib.types.str; };
                  };
                }
              );
              default = null;
              description = "A systemd-creds file an older build sealed under another name, adopted the same way.";
            };
            file = lib.mkOption {
              default = null;
              description = ''
                For a reader that can only take a path: the value is opened at
                boot into /run/losos-credentials (tmpfs) and `path` becomes a
                link to it.
              '';
              type = lib.types.nullOr (
                lib.types.submodule {
                  options = {
                    path = lib.mkOption { type = lib.types.str; };
                    owner = lib.mkOption {
                      type = lib.types.str;
                      default = "0";
                      description = "uid (and gid) of the opened file.";
                    };
                    mode = lib.mkOption {
                      type = lib.types.str;
                      default = "0600";
                    };
                    readers = lib.mkOption {
                      type = lib.types.listOf lib.types.str;
                      default = [ ];
                      description = ''
                        Units ordered after the link is in place. Ordering only
                        (systemd ignores a unit that does not exist), so one list
                        can name the readers of every mode.
                      '';
                    };
                  };
                }
              );
            };
          };
        }
      );
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
        compromised app pod, any other service, or root — whenever
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

    # ── The kindashared data domain (fscrypt) ──────────────────────────────
    kindashared.fscrypt.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Protect /home/kindashared/data with an fscrypt policy of its own that
        is unlocked only while losos.sharingLocally is on: the same second
        layer as the shared domain's (losos.shared.fscrypt.enable), with its
        own protector key, so turning one kind of sharing on never opens the
        other's files.
      '';
    };

    kindashared.fscrypt.keyFile = lib.mkOption {
      type = secretPath;
      default = "/var/secrets/losos-kindashared-key";
      description = ''
        The kindashared domain's fscrypt protector key, kept and sealed the
        way losos.shared.fscrypt.keyFile is: to the TPM when
        losos.tpm.enable is true, a 0600 file inside the LUKS volume when it
        is not.
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
      description = "System hostname. systemd-resolved publishes <hostName>.local via mDNS.";
    };

    # mDNS and DNS-SD through systemd-resolved (modules/mdns.nix). Set by the
    # box's own modules/configuration.nix and by an edge that advertises
    # itself (losos.edge.lan.advertise), never by an owner: nix/options-doc.nix
    # leaves the whole prefix out of the Advanced pane and /api/apply, because
    # a box with mDNS off is a box nobody can find.
    mdns.enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      internal = true;
      description = ''
        Answer and publish mDNS through systemd-resolved on every physical
        network interface, and announce losos.mdns.services.
      '';
    };

    mdns.services = lib.mkOption {
      type = lib.types.attrsOf lib.types.lines;
      default = { };
      internal = true;
      description = ''
        DNS-SD services to announce, by file name: the [Service] section of
        a systemd.dnssd(5) file, written to /etc/systemd/dnssd/<name>.dnssd.
      '';
    };

    splash.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Draw the boot screen and the box's status (its address, and the
        warning a box without a TPM chip carries) with Plymouth: the logo
        and a panel on the box's own palette, which stays on screen after
        boot. Off, the box boots in text and tty1 shows the same status as
        a text banner. On the installer medium the panel shows the
        installer instead.
      '';
    };

    calls.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Let people on the box call each other from LosOS chat, by voice or
        video. Calls belong to the box's shared side, like chat: they exist
        only while the box shares its storage and is in a mesh. Every
        frame is encrypted in the caller's browser and decrypted in the
        others', so the box forwards media it cannot read.
      '';
    };

    calls.udpPort = lib.mkOption {
      type = lib.types.port;
      default = 7882;
      description = ''
        The UDP port call media arrives on, on a box that tunnels through
        no edge. It is opened on the box's firewall while calls are on. A
        box with `losos.proxy.enable` opens no port: its media goes
        through the edge over QUIC (`losos.edge.calls.relay`).
      '';
    };

    cloud.enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Run LosOS Cloud: the owner's and every person's files at /cloud/,
        with calendars, contacts and boards, over WebDAV, CalDAV and CardDAV
        for file managers and calendar apps, and to the Nextcloud desktop and
        phone apps, which sync with it as they would with a Nextcloud server.
        People sign in with LosOS Login; each computer or phone gets a
        password of its own. On by default on an installed box
        (modules/defaults.nix).
      '';
    };

    cloud.windows.enable = lib.mkOption {
      type = lib.types.bool;
      # On wherever LosOS Cloud is (modules/windows.nix asks for both), and
      # declared so here rather than in defaults.nix: the Network pane draws
      # this default as the switch's state when overrides.nix has no line.
      default = true;
      description = ''
        Show the box under Network in Windows' File Explorer, and in Finder
        and Linux file managers, with each person's LosOS Cloud files as a
        network share they open with their LosOS name and password. Only on
        the local network, never through the edge. Windows takes a person's
        password once the box has seen it: when it is set, at a sign-in to
        the share from a file manager, or typed once on the Account page.
      '';
    };

    cloud.trashDays = lib.mkOption {
      type = lib.types.ints.between 1 365;
      default = 30;
      description = ''
        How many days a deleted file stays in LosOS Cloud's trash before it
        is deleted for good.
      '';
    };

    cloud.photos.faces = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Find the faces in LosOS Cloud's photos and group them into people,
        whom the owner can name. Two small models do it on this box's CPU, in
        the background at the lowest priority; no face, place or picture
        leaves the box, and only the photos' owner sees them. Turning it off
        deletes every face and person found so far; the camera's details
        (date, camera, lens, place) are read either way.
      '';
    };

    cloud.mesh.enable = lib.mkOption {
      type = lib.types.bool;
      default = config.losos.sharingMyStorage || config.losos.cluster.enable;
      defaultText = lib.literalExpression "config.losos.sharingMyStorage || config.losos.cluster.enable";
      description = ''
        Let people on this box share files and folders with people on other
        boxes, and edit text, Markdown and notes together with them live.
        The box answers other boxes at /mesh/v1/link and dials them there.
        On by default once the box is in a mesh.
      '';
    };

    cloud.mesh.name = lib.mkOption {
      type = lib.types.str;
      default =
        if config.losos.proxy.enable then config.losos.proxy.hostname else "${config.losos.hostName}.local";
      defaultText = lib.literalExpression ''if config.losos.proxy.enable then config.losos.proxy.hostname else "''${config.losos.hostName}.local"'';
      description = ''
        The name other boxes reach this one by, the part after the @ in
        people's addresses here (eva@kitchenbox.local). Other boxes keep
        this box's key under this name, so changing it later makes every
        share with this box start over.
      '';
    };

    cloud.mesh.selfSigned = lib.mkOption {
      type = lib.types.bool;
      default = !config.losos.proxy.enable;
      defaultText = lib.literalExpression "!config.losos.proxy.enable";
      description = ''
        Accept another box's self-signed certificate when dialling it. Who a
        box is rests on its own key, checked in the link's handshake, either
        way. Turn this off when every box answers on a name with a real
        certificate.
      '';
    };

    search.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Let the search on the admin pages find text inside the owner's
        files: an index of the files in LosOS Cloud, brought up to date
        every two minutes, and a search of the default branch of every
        repository in LosOS Git. The index is a copy of the files' text and
        never leaves the box; turning this off stops the updates, and a
        factory erase deletes it.
      '';
    };

    # ── Settings › Logs (modules/log-reader.nix) ─────────────────────────
    logReader.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Show the journal of the box's services in the admin pages, under
        Settings › Logs, to an administrator or whoever holds the spare key,
        with passwords, keys and tokens hidden. It runs apart from the
        control service, so the page keeps working when that service does
        not. The lines never leave the box unless an administrator
        downloads them.
      '';
    };

    # ── Office documents (modules/office.nix) ───────────────────────────
    office.enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Open, edit and co-edit Word, Excel, PowerPoint and OpenDocument
        files in the browser with Collabora Online, a LibreOffice that runs
        on the box. Documents never leave the box: the editor reads and
        saves them through LosOS Cloud. It runs while the box is up and
        uses about 550 MB of memory with no document open.
      '';
    };

    office.port = lib.mkOption {
      type = lib.types.port;
      default = 9980;
      description = "Loopback port Collabora Online listens on. nginx serves it at /browser/ and /cool/.";
    };

    office.wopiPort = lib.mkOption {
      type = lib.types.port;
      default = 8093;
      description = ''
        Loopback port of the server Collabora Online reads documents from
        and saves them to (the WOPI host): LosOS Cloud, or the folder below.
      '';
    };

    office.folder = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      example = "/var/lib/losos-office/files";
      description = ''
        Edit the documents in this folder instead of LosOS Cloud's, with a
        small host of its own (losos-office). For a box without LosOS Cloud.
      '';
    };

    startupSound.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Play the LosOS startup sound once the box is up, on every speaker,
        headphone and HDMI output it finds. It plays when the box is
        switched on or comes back after a power cut; the nightly reboot and
        the reboot after an update stay quiet.
      '';
    };

    logs.journalMaxUse = lib.mkOption {
      type = lib.types.strMatching "[1-9][0-9]*[KMG]";
      default = "512M";
      example = "1G";
      description = ''
        The most disk space the system journal may take, as a number with K,
        M or G after it. When the journal reaches it, journald deletes the
        oldest entries, and it deletes any entry older than a month. The other logs on the box have
        fixed limits of their own.
      '';
    };

    builder.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Build widgets and apps with AI: the "Build with AI" tabs of the
        widget editor and of Your apps ask a language model that runs on
        this box (Ollama, on loopback only). What the owner types never
        leaves the box, and building is free. The model is downloaded once,
        about 1 GB, in the background, the first time the box is online.
        While it writes, it needs about 1.5 GiB of memory; it lets go of it
        five minutes after the last build.
      '';
    };

    builder.model = lib.mkOption {
      type = lib.types.str;
      default = "qwen2.5-coder:1.5b";
      description = ''
        The Ollama model the builder asks. The default, Qwen2.5-Coder 1.5B
        (Apache License 2.0), fits the smallest box LosOS runs on: 4 GiB of
        memory and two processor cores, beside LosOS Cloud and LosOS Git.
        The prompts are written for it. A larger Qwen2.5-Coder such as
        `qwen2.5-coder:7b` writes better apps on a box with 8 GiB or more
        to spare, and takes several times as long.
      '';
    };

    builder.contextTokens = lib.mkOption {
      type = lib.types.ints.between 4096 131072;
      default = 12288;
      description = ''
        The builder's context window, in tokens: its instructions (about
        3,000), the widget or app to change, a second turn when the box finds
        a mistake, and the answer (up to 4,096). Each token of window costs
        the model some memory while it runs.
      '';
    };

    assistant.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Ask the box's AI: "Ask AI about this" in LosOS Cloud, Summarise in
        Mail and the other apps' questions about a file or a message go to
        a language model that runs on this box (Ollama, on loopback only,
        beside the builder's). The material and the question never leave
        the box. The model is downloaded once, about 800 MB, the first
        time the box is online. While it answers it needs about 1 GiB of
        memory; it lets go of it five minutes after the last question.
      '';
    };

    assistant.model = lib.mkOption {
      type = lib.types.str;
      default = "gemma3:1b";
      description = ''
        The Ollama model the apps' questions go to: everything that is not
        code. The builder and LosOS Reviewer keep `builder.model`. The
        default, Gemma 3 1B (Gemma Terms of Use, ai.google.dev/gemma/terms),
        fits the smallest box LosOS runs on: 4 GiB of memory and two
        processor cores. Ollama holds one model at a time, so the two never
        need memory together. On a box with 8 GiB or more, `gemma4:e2b`
        (Apache License 2.0, about 5 GiB while it answers) writes better
        summaries.
      '';
    };

    assistant.contextTokens = lib.mkOption {
      type = lib.types.ints.between 4096 131072;
      default = 8192;
      description = ''
        The assistant's context window, in tokens: its instructions, the
        material (cut at 9,000 characters, about 2,500 tokens), the question
        and the answer. Each token of window costs the model some memory
        while it runs.
      '';
    };

    # ── Lending memory to models split across boxes (modules/inference-lend.nix)
    inference.lend.enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Lend this box's memory to large AI models that run split across
        several boxes on the mesh. A llama.cpp RPC server holds its share of
        a model's layers and computes on them, on loopback only, reached
        only by the edge through this box's tunnel; buyers pay the edge per
        token. It runs as the `shared` user, away from the owner's data, and
        cannot take more than `inference.lend.memoryMiB`: a request for more
        stops the lending server, never LosOS Cloud. Needs the edge proxy
        (losos.proxy.enable).
      '';
    };

    inference.lend.memoryMiB = lib.mkOption {
      type = lib.types.ints.between 512 262144;
      default = 1024;
      description = ''
        The memory this box lends, in MiB. The edge never gives the box a
        larger share of a model than this, and the lending server is
        stopped if it grows past it. The default leaves a 4 GiB box room
        for LosOS Cloud and LosOS Git; raise it on a box with more.
      '';
    };

    inference.lend.threads = lib.mkOption {
      type = lib.types.nullOr (lib.types.ints.between 1 256);
      default = null;
      description = ''
        Processor threads the lending server computes with. Null lets it
        take half of the box's processors.
      '';
    };

    inference.lend.package = lib.mkOption {
      type = lib.types.package;
      default = pkgs.llama-cpp.override { rpcSupport = true; };
      defaultText = lib.literalExpression "pkgs.llama-cpp.override { rpcSupport = true; }";
      description = "The llama.cpp build the lending server runs: its `ggml-rpc-server`.";
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

    # Sharing as kindashared: only with the local gateways of this box's
    # own network, never with an official edge or the wider mesh.
    sharingLocally = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Share this box with the local edge gateways of its own network (a
        company's or a home's), and with nothing further: the third data
        account, `kindashared` (modules/configuration.nix), is the shared
        account's counterpart for a secondary cluster. Its data domain,
        /home/kindashared/data, is unlocked only while this is on
        (modules/fscrypt.nix), exactly as losos.sharingMyStorage opens the
        shared one.

        lososd turns it on only while a local gateway answers (an edge whose
        link from this box is secondary, backend/src/edge.rs): an official
        edge alone is refused with 409 `localGatewayRequired`. With no local
        gateway for losos.proxy.pauseSharingAfter seconds the box turns it
        off by itself, and back on a minute after one answers again. It is
        independent of losos.sharingMyStorage: a box can share with its
        local gateway and keep everything from the official edges.
      '';
    };

    # LosOS Git (modules/git.nix, backend/src/forge/): the box's own Git
    # server, a Rust service of lososd's crate. One mode: it runs on the
    # host, never in a cluster.
    git.enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Run LosOS Git, the box's Git server: repositories for every account
        on LosOS Login, cloned and pushed over https, and the box's own
        configuration repository. Set to true by modules/defaults.nix.
      '';
    };

    # The names an older overrides.nix may still carry. Each keeps
    # evaluating, does nothing, and is off the Advanced pane; the next Apply
    # writes the file without it.
    forgejo.mode = lib.mkOption {
      type = lib.types.enum [
        "native"
        "container"
      ];
      default = "container";
      visible = false;
      description = "No longer used: LosOS Git has one mode.";
    };
    forgejo.federation.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      visible = false;
      description = "No longer used.";
    };
    forgejo.site.landingPage = lib.mkOption {
      type = lib.types.enum [
        "home"
        "explore"
        "organizations"
        "login"
      ];
      default = "home";
      visible = false;
      description = "No longer used.";
    };
    forgejo.site.keepEmailPrivate = lib.mkOption {
      type = lib.types.bool;
      default = false;
      visible = false;
      description = "No longer used: LosOS Git shows an e-mail address only to its owner.";
    };

    # ── The box's configuration on LosOS Git ──────────────────────────────
    # /etc/nixos is a git repository; every Apply commits there
    # (backend/src/config_repo.rs). With this on, lososd keeps it in step
    # with a private repository on LosOS Git, owned by the owner's account, and takes a commit pushed there (after its
    # overrides.nix passes the gates every Apply does) and rebuilds from it.
    configRepo.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Publish this box's configuration (/etc/nixos) to a private repository
        on LosOS Git, and pick up commits pushed to it. Needs
        losos.git.enable. Off, every change is still committed on the
        box; it is just not published anywhere.
      '';
    };
    configRepo.owner = lib.mkOption {
      type = lib.types.str;
      default = "notshared";
      description = ''
        The LosOS Git account the configuration repository belongs to. lososd
        creates it as a site administrator if it is missing. The owner
        reaches it by signing in with LosOS Login, and their computers with
        a token of their own.
      '';
    };
    configRepo.name = lib.mkOption {
      type = lib.types.str;
      default = "losos-config";
      description = "The name of the configuration repository on LosOS Git.";
    };

    # ── Following the published tree ─────────────────────────────────────
    # The installer copies the published tree into /etc/nixos once, and both
    # rebuild paths build from there, so without this a box never gets new
    # software. With it, nixos-upgrade.service first asks lososd to move
    # /etc/nixos to the newest commit of this branch, keeping the box's own
    # modules/overrides.nix and modules/install-target.nix
    # (backend/src/config_repo.rs), and the 03:00 rebuild then builds it.
    configRepo.upstream.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Before each nightly update, move this box's configuration to the
        newest commit of losos.configRepo.upstream.branch at
        losos.configRepo.upstream.url, keeping its own settings, drive list,
        firmware mode and unlock mode. The box skips the step, and says so on
        the History pane, when files other than those two were changed in its
        configuration, so a hand edit pushed to LosOS Git is never
        overwritten. Off, the box rebuilds only from the configuration it has.
      '';
    };
    configRepo.upstream.url = lib.mkOption {
      type = lib.types.str;
      default = "https://github.com/dasmatus/losos.git";
      description = ''
        The git repository the box takes new versions from. A company can
        point it at its own fork. It must be reachable without credentials.
      '';
    };
    configRepo.upstream.branch = lib.mkOption {
      type = lib.types.str;
      default =
        if config.losos.updates.channel == "main" then
          "main"
        else
          "channel/${config.losos.updates.channel}";
      defaultText = lib.literalExpression ''"channel/''${config.losos.updates.channel}", or "main" for the main channel'';
      description = ''
        The branch of losos.configRepo.upstream.url the box follows. Set
        losos.updates.channel instead; this is for a fork that names its
        branches differently.
      '';
    };

    # ── Release channels ─────────────────────────────────────────────────
    # .github/workflows/channels.yml moves channel/daily, channel/weekly and
    # channel/monthly to a commit of main whose CI run and cache upload both
    # passed, and puts every merged pull request labelled `hotfix` on top of
    # each one the same day. The box only picks the branch; lososd never
    # takes a channel that is older than what the box already runs
    # (crate::config_repo::plan_follow).
    updates.channel = lib.mkOption {
      type = lib.types.enum [
        "daily"
        "weekly"
        "monthly"
        "main"
      ];
      default = if stableRelease then "weekly" else "daily";
      defaultText = lib.literalExpression ''"daily" before LosOS 1.0.0, "weekly" from then on'';
      description = ''
        How often this box takes a new version. "daily" takes the newest
        version that passed every check, at most a day after it was made.
        "weekly" takes one each Monday and "monthly" one on the 1st, so a
        change reaches the box up to a week or a month later. A fix marked as
        a hotfix reaches every channel the same day. "main" takes every change
        the night it is made, checked or not, and is for testing LosOS
        itself. Moving to a slower channel keeps the version the box has
        until that channel passes it.
      '';
    };

    # ── LosOS Git's box-wide settings ───────────────────────────────────────
    # What an administrator would otherwise change in a Git host's site
    # administration, kept here so it goes through Apply, the history and
    # LosOS Git like every other setting. The admin UI draws them on the Apps
    # pane. LosOS Git takes them as flags on every start (modules/git.nix) and
    # has no page for them, so the two places cannot drift.

    git.site.requireSignIn = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Show nothing on LosOS Git to anyone who is not signed in, public
        repositories included. Through an edge, LosOS Git is on the
        internet, so this is what keeps a public repository private to the
        accounts on the box.
      '';
    };

    git.site.defaultPrivate = lib.mkOption {
      type = lib.types.enum [
        "last"
        "private"
        "public"
      ];
      default = "last";
      description = ''
        Whether the "Private" box starts ticked when a repository is
        created: "last" repeats the account's previous choice, "private" and
        "public" always start the same way. A repository made by a push, or
        through the API without saying, is public only with "public".
      '';
    };

    git.site.pushCreate = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Let an account create a repository by pushing to an address that does
        not exist yet, instead of creating it on the page first.
      '';
    };

    # ── Retired settings ────────────────────────────────────────────────────
    # Nextcloud is gone, and with it the keyring, whose one secret was
    # Nextcloud's admin password. Every box installed before has lines for
    # them in its own modules/overrides.nix (`losos.nextcloud.mode` is in
    # every copy), and that file stays on the box when it follows main. These
    # take whatever such a file says and ignore it, so the 03:00 rebuild does
    # not stop on "option does not exist"; lososd leaves the lines out the
    # next time it writes the file (RETIRED in backend/src/overrides.rs).
    nextcloud = lib.mkOption {
      type = lib.types.attrsOf lib.types.anything;
      default = { };
      internal = true;
      visible = false;
      description = "Retired with Nextcloud. Accepted from an older overrides.nix and ignored.";
    };
    keyring = lib.mkOption {
      type = lib.types.attrsOf lib.types.anything;
      default = { };
      internal = true;
      visible = false;
      description = "Retired with Nextcloud's admin password. Accepted and ignored.";
    };
    # rathole's settings on the box. An Advanced-pane edit may have put them
    # in overrides.nix, so they are accepted and ignored like the two above
    # rather than removed: a removed option would stop the 03:00 rebuild.
    # The edge's own (edge.bootstrapTokenFile and the rest) are removed in
    # `imports`, since an edge's operator reads the evaluation error.
    proxy.bootstrapTokenFile = lib.mkOption {
      type = lib.types.anything;
      default = null;
      internal = true;
      visible = false;
      description = "Retired with rathole: the tunnel needs no bootstrap token. Accepted and ignored.";
    };
    proxy.noisePublicKeyFile = lib.mkOption {
      type = lib.types.anything;
      default = null;
      internal = true;
      visible = false;
      description = "Retired with rathole: the box pins the edge's certificate in proxy.tunnelPinFile. Accepted and ignored.";
    };
    proxy.rathole = lib.mkOption {
      type = lib.types.attrsOf lib.types.anything;
      default = { };
      internal = true;
      visible = false;
      description = "Retired with rathole. Accepted and ignored.";
    };

    # ── GPU hardware acceleration ──────────────────────────────────────────
    gpu.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Enable host graphics (VA-API/Mesa) for the services on the box that
        can use them (modules/configuration.nix).
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
        New versions arrive through losos.configRepo.upstream, which moves
        /etc/nixos itself before the rebuild, so an Apply and the nightly
        update build the same tree. A github: URI (still with `#install`)
        also works, but an Apply then rebuilds from the older tree in
        /etc/nixos and undoes the update until the next night.

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

    # lososd's plugins (backend/plugins/README.md): the store path lososd
    # loads its WebAssembly plugins from, `.#losos-plugins` by default. lososd
    # refuses a directory outside the read-only store.
    backend.plugins = lib.mkOption {
      type = lib.types.nullOr lib.types.package;
      default = null;
      description = ''
        The directory of lososd plugins, each a WebAssembly component
        (`<name>.wasm`) beside its manifest (`<name>.json`). lososd loads
        them only from the read-only Nix store. When null, lososd serves no
        plugin routes, and the panes they back (the look) answer 404.
      '';
    };

    # ── Sign-in (modules/identity.nix) ───────────────────────────────────
    identity.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Sign people in to the admin pages with Authelia, at /auth/ on the
        box. Each person has their own name and password, and their groups
        decide which parts of the admin pages they can use. The first person
        is made when the box is claimed and is an administrator. The spare
        key opens the box with or without this.
      '';
    };

    identity.port = lib.mkOption {
      type = lib.types.port;
      default = 9091;
      description = "Loopback port Authelia listens on. nginx serves it at /auth/.";
    };

    # ── LosOS Chat (modules/chat.nix, backend/src/chat.rs) ─────────────────
    chat.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Run LosOS Chat: a Matrix homeserver on the box and the chat page at
        /chat/, where everyone with a LosOS Login account can talk in rooms
        and direct messages, with end-to-end encryption, files, reactions,
        threads and voice and video calls on the box's network. Chat runs
        as the shared user and keeps everything in the shared data domain,
        so it is there only while the box shares its storage in a mesh it
        has joined. Signing in to
        chat is always through LosOS Login; there are no chat passwords.
      '';
    };

    chat.serverName = lib.mkOption {
      type = lib.types.strMatching "[a-z0-9]([a-z0-9-]*[a-z0-9])?(\\.[a-z0-9]([a-z0-9-]*[a-z0-9])?)+";
      default = "${config.losos.hostName}.local";
      defaultText = lib.literalExpression ''"''${config.losos.hostName}.local"'';
      description = ''
        The part after the colon in everyone's chat address,
        @name:server-name. Set it once, before people start chatting: the
        addresses and rooms made under one name do not move to another.
        Federation needs a name other servers can reach, such as a domain
        of your own that your edge routes to this box.
      '';
    };

    chat.federation.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Let people here talk with people on other chat servers, and let
        those servers reach this one. With a .local server name that means
        the other boxes on this network, and only them: the federation
        addresses answer the LAN alone. With a name other servers can
        reach, any Matrix server. LosOS Git uses it too: people of other
        boxes sign in to its repositories with their chat address, and a
        repository's room takes them in.
      '';
    };

    chat.port = lib.mkOption {
      type = lib.types.port;
      default = 8008;
      description = "Loopback port the homeserver listens on. nginx serves it at /_matrix/.";
    };

    chat.maxUploadMiB = lib.mkOption {
      type = lib.types.ints.between 1 4096;
      default = 100;
      description = "The largest file one person can send in chat, in MiB.";
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
        Where an older build kept the Bearer token for the admin API. The
        token is now the `admin-token` credential (modules/credentials.nix):
        made at first boot, sealed in /etc/credstore.encrypted, and adopted
        from this file, which is then deleted, on a box that had one.
      '';
    };

    access.gpgKeys = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      example = lib.literalExpression "[ (builtins.readFile ./owner.pub.asc) ]";
      description = ''
        Armoured GPG public keys (`gpg --armor --export <id>`) allowed to
        sign calls from the `losos` terminal client. A call needs a personal
        access token from the web UI's Access tokens pane AND a signature by
        the key that token is bound to. Seeded by the installer
        (`losos-ctl install --gpg-key FILE`, or this option on the medium),
        and fixed after that: the Advanced pane cannot change it and
        /api/apply refuses it, because a web session that could add a key
        could also make a token for it. See docs/terminal-access.md.
      '';
    };

    access.throughEdge = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Let the `losos` terminal client reach this box through its edge
        while it is away from the box's network. The terminal's two routes,
        `/api/access/host-key` and `/api/access/call`, then answer through
        the tunnel as well as on the LAN, and the host key's answer names
        the edge address (`https://` and `losos.proxy.hostname`) for the
        client to remember at login. A call is signed by a seeded key,
        carries a token and is encrypted to this box's own key, so the edge
        carries it without reading it. Only while `losos.proxy.enable`; off,
        both routes are LAN-only like the rest of `/api/`. See
        docs/terminal-access.md.
      '';
    };

    # Path of the built admin SPA (built from this flake's ./admin-ui/app by
    # nix/packages.nix with Bun; wired in via
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

    # Path of the built owner's handbook (handbook/, built by nix/packages.nix
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

    # LosOS Diagrams is the admin UI's second page (admin-ui/app/diagrams/, inside
    # losos-admin-ui), so there is nothing to point at, only whether to serve
    # it. Internal, like admin.ui.
    admin.lab = lib.mkOption {
      type = lib.types.bool;
      default = true;
      internal = true;
      description = ''
        Serve LosOS Diagrams, the diagram editor and network simulator, at
        /diagrams/ from the admin UI's own bundle, LAN-only, under its own
        content security policy.
      '';
    };

    # ── LosOS Mail (modules/mail.nix) ─────────────────────────────────────────
    mail.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        A mailbox for every person LosOS Login knows, read and written in
        the Mail page at /mail/. The mail server, Stalwart, runs as the
        shared user with its mailboxes in the shared data domain, so mail
        exists only while the box's storage counts as shared, that is with
        losos.sharingMyStorage on and the box in a mesh (losos.cluster.enable).
        Otherwise there is no server, no page and no menu entry. For now mail goes
        between the people on this box only.
      '';
    };

    mail.domain = lib.mkOption {
      type = lib.types.nullOr (lib.types.strMatching "[a-z0-9]([a-z0-9.-]*[a-z0-9])?");
      default = null;
      example = "mail.example.org";
      description = ''
        The domain of this box's addresses: a person signed in as `eva`
        gets eva@<domain>. Null is `<hostName>.home.arpa`, the name RFC 8375
        sets aside for home networks; mail servers need a domain under a
        public suffix, and `.local` is not one.
      '';
    };

    mail.port = lib.mkOption {
      type = lib.types.port;
      default = 8096;
      description = ''
        Loopback port of the mail server's HTTP listener, which lososd
        alone talks to.
      '';
    };

    mail.keyserver = lib.mkOption {
      type = lib.types.nullOr (lib.types.strMatching "https://[A-Za-z0-9.-]+(:[0-9]+)?");
      default = "https://keys.openpgp.org";
      example = "https://keys.mailvelope.com";
      description = ''
        The OpenPGP keyserver LosOS Mail asks for someone's key when
        neither LosOS Contacts, Autocrypt nor the recipient's own Web Key
        Directory has one. It is asked by email address over the VKS
        interface (`/vks/v1/by-email/`), which keys.openpgp.org and Hagrid
        servers answer. Null asks no keyserver.
      '';
    };

    # ── Mesh CI (modules/ci.nix) ─────────────────────────────────────────────
    ci.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Mesh CI: the GitHub Actions workflows in LosOS Git's repositories
        (.github/workflows/*.yml) run on pushes and pull requests, their
        jobs spread over the boxes of the mesh, and each job's result is a
        check on the commit. Like mail, it exists only while the box's
        storage counts as shared: losos.sharingMyStorage on and the box in a
        mesh (losos.cluster.enable). Other boxes' jobs run here as the shared
        user, in rootless Podman, with no way into this box's network.
      '';
    };

    ci.privateOnMesh = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Let the jobs of private repositories run on other boxes of the mesh.
        Off, they run on this box alone: another box would see the code.
      '';
    };

    ci.runner.memory = lib.mkOption {
      type = lib.types.strMatching "[0-9]+[kmg]";
      default = "2g";
      description = ''
        The most memory one job may use on this box, as Podman's --memory
        takes it. The box takes one job at a time.
      '';
    };

    ci.runner.cpus = lib.mkOption {
      type = lib.types.strMatching "[0-9]+([.][0-9]+)?";
      default = "1.5";
      description = ''
        The most processor time one job may use on this box, in cores. Jobs
        also run at a low share, so the box's own work comes first.
      '';
    };

    ci.runner.images = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      example = [ "ubuntu-latest=docker.io/library/ubuntu:24.04" ];
      description = ''
        Images for runs-on labels, as label=image. A job whose labels name
        none of these, and that sets no container of its own, runs in LosOS's
        own image: bash, the usual command-line tools, git and curl.
      '';
    };

    ci.pm = lib.mkOption {
      type = lib.types.nullOr lib.types.package;
      default = null;
      internal = true;
      description = "pm, which starts and stops Mesh CI's job containers and builds workflows' pm recipes.";
    };

    ci.pmPlugin = lib.mkOption {
      type = lib.types.nullOr lib.types.package;
      default = null;
      internal = true;
      description = "Mesh CI's signed pm plugin, as a pm configuration directory (nix/ci-pm-plugin.nix).";
    };

    # ── LosOS Diagrams' guests under libvirt (modules/lab.nix) ───────────────────
    lab.libvirt.enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Run LosOS Diagrams' guests under libvirt on this box (KVM when the CPU
        has it) instead of simulating their consoles. Turns on libvirtd and
        a `losos-registrar lab` helper on a loopback port; lososd relays
        /api/lab/ to it and nginx proxies the guests' console and network
        sockets, and the byte relay to libvirtd's socket that Diagrams'
        WebAssembly libvirt client uses (one ticket per socket, minted
        under the admin token). The guest images (bzImage, rootfs.bin, gear.bin from
        admin-ui/lab/engine/build.sh) go in `lab.libvirt.images`. Off by
        default: the appliance does not need libvirtd for anything else.
      '';
    };

    lab.libvirt.port = lib.mkOption {
      type = lib.types.port;
      default = 8095;
      description = "Loopback port Diagrams' libvirt helper listens on.";
    };

    lab.libvirt.images = lib.mkOption {
      type = lib.types.str;
      default = "/var/lib/losos-lab/images";
      description = ''
        Folder holding Diagrams' guest images: bzImage and rootfs.bin, and
        gear.bin for routers, switches and access points. A guest whose
        image is missing boots nowhere on the box and keeps its simulated
        console. Persisted via /var.
      '';
    };

    lab.libvirt.maxGuests = lib.mkOption {
      type = lib.types.ints.between 1 64;
      default = 8;
      description = "Most guests the helper runs at once.";
    };

    lab.libvirt.memoryMiB = lib.mkOption {
      type = lib.types.ints.between 32 4096;
      default = 96;
      description = "Memory of each guest, in MiB. The guests are busybox systems; 96 is what the in-browser engine gives them.";
    };

    lab.ordering.enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      visible = false;
      description = "Show the order button in LosOS Diagrams.";
    };

    # ── Problem reports (modules/problems.nix, backend/plugins/problems) ───
    report.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Offer to send problem reports to LosOS's issue tracker on GitHub.
        The box notes every service that fails, every program that crashes
        and every one the kernel kills for memory, and Settings › Problems
        lists them whatever this says. With this on, each problem the
        handbook does not already explain has a report you can read in
        full and send. Nothing is sent until you sign in to GitHub with the
        code the page shows. The installer offers the same for a failed
        install and for memory or processor use that spiked.
      '';
    };

    report.repository = lib.mkOption {
      type = lib.types.str;
      default = "dasmatus/losos";
      description = ''
        The GitHub repository, as owner/name, whose issues the reports go
        to.
      '';
    };

    report.clientId = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      description = ''
        The client ID of the GitHub App the reports sign in with: device
        flow on, no webhook, and only the Issues permission, installed on
        the repository. It is not a secret, because GitHub shows it on the
        sign-in page. Set to null, the box and the installer still find
        problems and write the reports, but cannot send them.
      '';
    };

    # Where the device flow and the API are, for a VM test's stand-in.
    report.login = lib.mkOption {
      type = lib.types.str;
      default = "https://github.com";
      internal = true;
      description = "GitHub's sign-in origin, for the device flow.";
    };

    report.api = lib.mkOption {
      type = lib.types.str;
      default = "https://api.github.com";
      internal = true;
      description = "GitHub's REST API origin.";
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
        Run `losos-install` on tty1 at boot, under the boot screen where it
        draws. On the installer ISO, this starts the installer and its
        firmware-mode menu. Leave false on a normal target system.
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
      default = ../nix/keys/secure-boot-db.pem;
      defaultText = lib.literalExpression "../nix/keys/secure-boot-db.pem";
      description = ''
        The LosOS Secure Boot signing certificate (X.509, PEM; `#` lines
        before the block are comments): the public half of the db key
        `losos-sign-iso` signs the medium with. Shipped on the medium's EFI
        system partition as `EFI/losos/losos-secure-boot.{cer,pem}` so a
        firmware's "enroll from file" dialog can take it from the stick, and
        the certificate `SHA256SUMS.sig` on a release verifies against. The
        committed file carries no certificate until the key ceremony in
        docs/provisioning/secure-boot/ has run: then the medium ships no
        certificate and nothing presents LosOS as a Secure Boot signer.
      '';
    };

    # ── Master proxy (edge side) ───────────────────────────────────────────
    # Options for the edge system (flake output `nixosModules.edge`), which
    # runs Traefik (master proxy), the QUIC tunnel server, and losos-registrar
    # (serve). Only used by that module; the appliance uses losos.proxy.* above.
    edge.enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "Configure this system as the losos master-proxy edge (Traefik + tunnel server + losos-registrar).";
    };

    edge.publicDomain = lib.mkOption {
      type = lib.types.str;
      default = "losos.cfd";
      description = "Apex domain. The static registration router is register.<publicDomain>; per-appliance hostnames live under it.";
    };

    edge.tunnelBindAddr = lib.mkOption {
      type = lib.types.str;
      default = "::";
      description = ''
        Address the tunnel server listens on, over UDP, for boxes to dial.
        Defaults to `::` (IPv6 any), which on Linux with the default
        `net.ipv6.bindv6only=0` binds dual-stack, so boxes that resolve the
        edge over IPv6 (the common case, and the nixosTest inter-VM path) and
        over IPv4 both reach it. Serialised bracketed for IPv6 (`[::]:<port>`)
        by the `fmtBind` helper in modules/edge.nix and the `format_bind`
        helper in edge/registrar/src/config.rs, which must agree byte for
        byte.
      '';
    };

    edge.tunnelBindPort = lib.mkOption {
      type = lib.types.port;
      default = 2333;
      description = "UDP port boxes dial the tunnel on (QUIC).";
    };

    edge.tunnelPortRange = lib.mkOption {
      type = lib.types.str;
      default = "50000-50100";
      description = "lo-hi range of loopback TCP ports the registrar gives each box's tunnel service on the edge, where Traefik reaches it.";
    };

    edge.calls.relay.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Carry call media for the boxes this edge serves. Each box keeps one
        QUIC connection to the edge and gets a UDP port of its own, which
        browsers in its calls send to; the edge passes the datagrams along
        without being able to read them (they are DTLS-SRTP around frames
        encrypted end to end). The edge sends from a box's port only to
        addresses that sent to it.
      '';
    };

    edge.calls.relay.port = lib.mkOption {
      type = lib.types.port;
      default = 7883;
      description = "UDP port boxes reach the calls relay on (QUIC).";
    };

    edge.calls.relay.mediaPorts = lib.mkOption {
      type = lib.types.str;
      default = "40000-40999";
      description = "lo-hi range of UDP ports the calls relay gives boxes, one each, for browsers to send call media to.";
    };

    edge.calls.relay.addresses = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      example = [ "10.0.0.5=203.0.113.7" ];
      description = ''
        Addresses browsers reach the calls relay's media ports on, `IP`, or
        `IP=PUBLIC` behind a 1:1 NAT that forwards the ports to IP. Empty:
        every address of every interface that is up.
      '';
    };

    edge.heartbeatTtl = lib.mkOption {
      type = lib.types.str;
      default = "120s";
      description = "Tenants with no heartbeat within this TTL are pruned (their Traefik routes and tunnel service removed).";
    };

    edge.reconcileInterval = lib.mkOption {
      type = lib.types.str;
      default = "15s";
      description = "How often the registrar reconciler re-derives the Traefik routes and the tunnel config from the registry.";
    };

    edge.registrarApiPort = lib.mkOption {
      type = lib.types.port;
      default = 8443;
      description = "Loopback port the losos-registrar HTTP API listens on; Traefik forwards register.<publicDomain> here.";
    };

    edge.traefikProviderPort = lib.mkOption {
      type = lib.types.port;
      default = 8090;
      description = ''
        Loopback port of the registrar's provider listener. Traefik's HTTP
        provider polls it every two seconds for the routes of the live boxes
        and published machines, and Traefik's offline page for a box it
        cannot reach is served from it. Never reachable off the machine.
      '';
    };

    edge.traefikMetricsPort = lib.mkOption {
      type = lib.types.port;
      default = 8091;
      description = ''
        Loopback port of Traefik's Prometheus counters. The registrar reads
        them every 15 seconds for the Statistics page of a company edge's
        manage page. Never reachable off the machine.
      '';
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

    edge.passport.enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Run this edge as an open gateway that boxes check for themselves
        (modules/edge-passport.nix): the registrar keeps an edge key at
        /var/lib/losos-registrar/edge.key and answers `/passport*` with the
        owner's signed settings, measured into the TPM's PCR 15 at boot, and
        a TPM quote over PCRs 4, 11 and 15. A box trusts the gateway when the
        owner's OpenPGP passport names it, the quote shows a published
        LosOS image, the TPM's maker vouches for the TPM, and the fee payout
        addresses are LosOS's own. Needs a TPM 2.0 at /dev/tpm0; the
        `losos-disk-edge-qcow2` image turns it on.
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
                and a tunnel service here, keyed `<id>.<box>` and authenticated
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
      description = "Closed-enrollment whitelist of appliances permitted to register. The registrar only ever hands Traefik routes for ids listed here.";
    };

    # ── Mesh CI (edge side) ─────────────────────────────────────────────────
    edge.ci.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Keep the Mesh CI job queue: boxes of the mesh hand jobs of their
        GitHub-style workflows to it with POST /ci/submit, and boxes that
        lend their compute claim them. Only boxes listed as tenants with
        `cluster` may use it. Off, every /ci/* route answers 503.
      '';
    };

    edge.tang.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Be the Tang server of the boxes installed with
        `losos-install --tang-edge`: each gets a key pair of its own here,
        and its disk opens only with its TPM2 and an answer from this edge
        (modules/network-unlock.nix). The answer is half of a blinded
        exchange, so the edge learns when the box boots and never sees the
        key. An owner whose box was stolen reports it at `/tang/lost` with
        the box's recovery code, which never leaves their browser, and the
        edge answers for that box no more until they take it back. Anyone
        may ask for a key, as of any public Tang server; the edge keeps at
        most 100 000 and makes at most 30 a minute. Off, every /tang/* route
        answers 404 and those boxes ask for their recovery code at boot.
      '';
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
        of them answers 503. With `losos.edge.market.cards.enable` (the
        default) it needs a Stripe platform account with Connect enabled; see
        handbook/docs/in-depth/market.md for the one-time dashboard setup.
      '';
    };

    edge.market.cards.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Let buyers pay by card through Stripe Connect, and sellers take
        payouts there. Off, the edge takes coins only (USDC, plus Bitcoin and
        Monero when they are on) and needs no Stripe account: no
        `losos-stripe-gate` unit runs, no Stripe key is sealed, and the
        market's mode is that of its USDC networks, so a live edge lists the
        mainnet ones. Needs `losos.edge.market.usdc.enable`. Hardware sales
        are paid by card, so they need it on.
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

    edge.market.usdc.enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Settle storage, compute and machine orders in USDC instead of Stripe,
        on the networks in `losos.edge.market.usdc.networks`; the buyer picks
        one per order. Hardware checkouts stay on Stripe. Needs
        `losos.edge.market.currency = "usd"`: a price in cents is that many
        USDC cents. See handbook/docs/in-depth/market.md.
      '';
    };

    edge.market.usdc.networks = lib.mkOption {
      type = lib.types.attrsOf (
        lib.types.submodule (
          { name, ... }:
          {
            options = {
              pay = lib.mkOption {
                type = lib.types.nullOr (lib.types.strMatching "0x[0-9a-fA-F]{40}|[1-9A-HJ-NP-Za-km-z]{32,44}");
                default = null;
                description = ''
                  The escrow payments wait in until each slot ends. Public. On
                  an EVM network, the LososEscrow contract (`0x`), required. On
                  Solana, the losos-escrow program (base58), whose config for
                  this treasury must name the releaser
                  `losos edge escrow releaser` shows. A Solana network
                  without it takes direct transfers to the seller, offered only
                  with `losos.edge.market.directPay`.
                '';
              };
              treasury = lib.mkOption {
                type = lib.types.str;
                description = ''
                  Who receives the platform fee: an `0x` address on an EVM
                  network, or the base58 owner of the fee's USDC account on
                  Solana. Public.
                '';
              };
              rpcUrlSealed = lib.mkOption {
                type = secretPath;
                default = "/var/secrets/losos-usdc-rpc-${name}.cred";
                defaultText = lib.literalExpression ''"/var/secrets/losos-usdc-rpc-<network>.cred"'';
                description = ''
                  The network's RPC URL, **sealed** with `systemd-creds` under
                  the credential name `usdc-rpc-<network>`. A provider key
                  usually sits in the URL, so it is a secret: the registrar
                  receives the plaintext only through `LoadCredentialEncrypted=`
                  and the browser never does. Seal it with `losos edge secret
                  set usdc-rpc-<network>`; until then the network answers 503.
                  Plain `http` is accepted only for a node on the edge's own
                  loopback.
                '';
              };
              genesisHash = lib.mkOption {
                type = lib.types.nullOr lib.types.str;
                default = null;
                description = ''
                  `solana-devnet` only: the genesis hash of a local validator
                  to settle against instead of devnet's. The live `solana`
                  network always checks mainnet's.
                '';
              };
            };
          }
        )
      );
      default = { };
      example = lib.literalExpression ''
        {
          base-sepolia = { pay = "0x…"; treasury = "0x…"; };
          arbitrum-sepolia = { pay = "0x…"; treasury = "0x…"; };
          solana-devnet = { pay = "…"; treasury = "5ZWj7a1f8tWkjBESHKgrLmXshuXxqeY9SYcfbshpAqPG"; };
        }
      '';
      description = ''
        The networks USDC is settled on, by name: `base`, `arbitrum`,
        `optimism` and `solana` (live, paired with a live Stripe key), or
        `base-sepolia`, `arbitrum-sepolia`, `optimism-sepolia` and
        `solana-devnet` (test, paired with a test key). One edge settles in
        one mode. Circle's USDC on each network is built into the registrar,
        not configured. The registrar refuses an RPC endpoint that answers for
        a different chain or cluster.
      '';
    };

    edge.market.usdc.releaserKeySealed = lib.mkOption {
      type = secretPath;
      default = "/var/secrets/losos-usdc-releaser-key.cred";
      description = ''
        The private key the escrow takes release and refund calls from, 64
        hex characters, **sealed** under the credential name
        `usdc-releaser-key`. It pays gas on every EVM network, so it holds a
        little ETH on each, and it can only release a deposit to the seller
        and treasury recorded at payment or refund it to the payer. The
        escrow's treasury can replace it. On Solana the edge signs with an
        ed25519 key derived from it, which holds a little SOL for fees and
        the token accounts it opens. Needed whenever a network has `pay`. The edge makes it itself (`losos edge escrow releaser
        --new`), so the key never exists anywhere else.
      '';
    };

    edge.market.usdc.ensRpcUrlSealed = lib.mkOption {
      type = lib.types.nullOr secretPath;
      default = null;
      example = "/var/secrets/losos-ens-rpc.cred";
      description = ''
        An Ethereum mainnet RPC URL (Sepolia on a test edge), sealed under the
        credential name `ens-rpc`, so the admin page can show ENS names next
        to payout addresses and send USDC to a name. Lookups only; nothing is
        paid on it. `null` leaves names off.
      '';
    };

    edge.market.usdc.binance = {
      enable = lib.mkEnableOption ''
        buying USDC with euros at Binance from the box's wallet. The edge
        signs each order with its Binance Connect partner key and hands the box
        Binance's checkout; the buyer signs in to Binance, passes its identity
        checks and pays there, and Binance withdraws the USDC on Base to the
        box's wallet. Needs the live `base` network in
        `losos.edge.market.usdc.networks` and a Binance Connect partner
        account, whose details fill the options below
      '';
      apiUrl = lib.mkOption {
        type = lib.types.nullOr (
          lib.types.strMatching "https://[^/@[:space:]]+|http://(127\\.0\\.0\\.1|localhost)(:[0-9]+)?"
        );
        default = null;
        description = ''
          The origin of Binance Connect's API that Binance gave with the
          partner account, `https://` and no path. Public. Plain `http` is
          accepted only on the edge's own loopback.
        '';
      };
      clientId = lib.mkOption {
        type = lib.types.nullOr (lib.types.strMatching "[[:graph:]]+");
        default = null;
        description = "The partner's client id from Binance. Public.";
      };
      webhookPublicKeyFile = lib.mkOption {
        type = lib.types.nullOr lib.types.path;
        default = null;
        example = lib.literalExpression "./binance-webhook.pub";
        description = ''
          A file with Binance's RSA public key for its order notifications,
          PEM or one base64 line, as Binance gives it. Public, so it may sit
          in the store. A notification it did not sign is refused.
        '';
      };
      keySealed = lib.mkOption {
        type = secretPath;
        default = "/var/secrets/losos-binance-connect-key.cred";
        description = ''
          The partner's RSA private key, 2048 bits or more, PEM (PKCS#8 or
          PKCS#1), **sealed** under the credential name
          `binance-connect-key`. Binance holds only its public half. Seal it
          with `losos edge secret set binance-connect-key`; until then a
          purchase answers 503.
        '';
      };
      tokenSealed = lib.mkOption {
        type = secretPath;
        default = "/var/secrets/losos-binance-connect-token.cred";
        description = ''
          The partner's sign access token from Binance, **sealed** under the
          credential name `binance-connect-token`.
        '';
      };
    };

    edge.market.bitcoin.enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Let buyers pay for storage, compute and machine orders in Bitcoin,
        beside USDC. The edge runs its own pruned `bitcoind` with a watch-only
        wallet; each order gets a fresh address derived from the seller's own
        extended public key, so the coins go straight to the seller and the
        edge never holds a private key. The platform fee on these orders is
        owed by the seller and settled in USDC (`coinFeeLimit`). Needs
        `losos.edge.market.usdc.enable`, and buyers see it only with
        `directPay`. See handbook/docs/in-depth/market.md.
      '';
    };

    edge.market.bitcoin.chain = lib.mkOption {
      type = lib.types.enum [
        "main"
        "testnet4"
        "signet"
        "regtest"
      ];
      default = "main";
      description = ''
        The chain the edge's node follows. A live edge (live Stripe key) must
        follow `main`; a test edge takes `testnet4`, `signet` or `regtest`.
        The registrar checks the pairing before it takes a seller's key or
        hands out an address, so a real edge cannot settle orders in test
        coins.
      '';
    };

    edge.market.bitcoin.pruneMiB = lib.mkOption {
      type = lib.types.ints.between 550 1000000;
      default = 10240;
      description = ''
        How much of the block chain the node keeps, in MiB. The watch-only
        wallet only follows addresses made after it started, so a pruned node
        is enough.
      '';
    };

    edge.market.monero.enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Let buyers pay for storage, compute and machine orders in Monero,
        beside USDC. The edge runs its own pruned `monerod` and a wallet
        service holding one view-only wallet per seller, made from the
        seller's primary address and private view key; each order gets a
        fresh subaddress of the seller's own wallet, so the coins go straight
        to the seller and nothing on the edge can spend them. The platform
        fee on these orders is owed by the seller and settled in USDC
        (`coinFeeLimit`). Needs `losos.edge.market.usdc.enable`, and buyers
        see it only with `directPay`. See handbook/docs/in-depth/market.md.
      '';
    };

    edge.market.monero.network = lib.mkOption {
      type = lib.types.enum [
        "mainnet"
        "stagenet"
        "testnet"
      ];
      default = "mainnet";
      description = ''
        The network the edge's node follows. A live edge (live Stripe key)
        must follow `mainnet`; a test edge takes `stagenet` or `testnet`. The
        registrar checks the pairing, and that a seller's address is for the
        same network, before it takes a key or hands out a subaddress.
      '';
    };

    edge.market.zcash.enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Let buyers pay for storage, compute and machine orders in shielded
        Zcash, beside USDC. The seller gives the edge a unified viewing key;
        the edge keeps only its Orchard incoming viewing key, which sees
        payments arrive and can spend nothing. Each order gets a fresh
        Orchard-only address of the seller's own wallet, so the coins go
        straight to the seller. The edge runs no node: it fetches compact
        blocks from the public servers in `servers` and decrypts them
        itself, so the key never leaves the edge, and counts a payment only
        when servers of two different operators agree on it. The platform fee
        on these orders is owed by the seller and settled in USDC
        (`coinFeeLimit`). Needs `losos.edge.market.usdc.enable`, and buyers
        see it only with `directPay`. See handbook/docs/in-depth/market.md.
      '';
    };

    edge.market.zcash.servers = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [
        "https://zec-node.cakewallet.com:443"
        "https://na.zec.rocks:443"
        "https://lwd.z0n.jp:443"
        "https://zec.0xrpc.io:443"
      ];
      description = ''
        Public lightwalletd-protocol gRPC servers the edge asks for compact
        blocks, as `https://host:port`. At least two, run by different
        operators (told apart by domain): a payment counts only when two of
        them show it in a block of the same hash. Each must report the
        network in `network`, or it is not asked. The default lists mainnet
        servers of four operators.
      '';
    };

    edge.market.zcash.network = lib.mkOption {
      type = lib.types.enum [
        "mainnet"
        "testnet"
        "regtest"
      ];
      default = "mainnet";
      description = ''
        The network the servers must follow. A live edge (live Stripe key)
        settles on `mainnet` only; `testnet` and `regtest` are for a test
        edge. The registrar checks the pairing, each server's reported
        chain, and that a seller's key is for the same network before it
        takes the key or hands out an address.
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
        `stripe-webhook-secret` and the USDC ones; a module that needs
        another secret adds an entry, or adds its unit to an entry's `units`
        (`losos.edge.credentials.secrets.stripe-webhook-secret.units = [ "x.service" ]`).
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

    edge.credentials.sealPackage = lib.mkOption {
      type = lib.types.package;
      internal = true;
      readOnly = true;
      description = ''
        `losos-seal-credential`, set by modules/edge-credentials.nix, for the
        registrar's terminal route that seals a secret.
      '';
    };

    edge.access.gpgKeys = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      example = lib.literalExpression "[ (builtins.readFile ./operator.pub.asc) ]";
      description = ''
        Armoured GPG public keys allowed to sign calls from the `losos`
        terminal client to this edge: its overview, its sealed secrets and a
        reconcile. A call also needs a token made with `losos-registrar
        provision token create`, behind the operator allowlist. Empty (the
        default), the edge serves no terminal routes. See
        docs/edge-terminal.md.
      '';
    };

    edge.market.feeBps = lib.mkOption {
      type = lib.types.ints.between 0 2000;
      default = 400;
      description = ''
        The platform's cut of every sale in basis points of the gross amount:
        400 is 4%. Kept by the platform when a held card payment is paid
        out to the seller, and by the treasury when an escrowed USDC payment
        is. Capped at 20% so a typo cannot take a third of every sale.
      '';
    };

    edge.market.offlineRefundAfter = lib.mkOption {
      type = lib.types.ints.between 60 604800;
      default = 86400;
      description = ''
        How long, in seconds, a seller's box may be offline in a row while a
        buyer's slot runs before the buyer gets back the share of the slot
        not delivered, and the slot ends. Twice this in all during one slot
        does the same. Offline means the seller's node is not `Ready` in the
        mesh, or, without a mesh apiserver, no heartbeat within the TTL. Time
        the edge itself was down or shedding load never counts. Keep it well
        above how long the boxes' nightly reboot takes.
        See handbook/docs/in-depth/market.md.
      '';
    };

    edge.market.directPay = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Also let buyers pay in ways that reach the seller's wallet at once:
        Bitcoin, Monero and Zcash (`bitcoin`, `monero`, `zcash`) and USDC on
        Solana without an escrow program. Nothing paid that way can be held
        for the slot, so a buyer whose seller goes offline cannot be refunded.
        Off, the market takes only cards and USDC through an escrow, which
        the edge holds until each slot ends.
      '';
    };

    edge.market.coinFeeLimit = lib.mkOption {
      type = lib.types.ints.between 0 100000;
      default = 1000;
      description = ''
        The platform fees, in the market currency's minor unit (cents), a
        seller may owe on Bitcoin, Monero and Zcash sales before buyers can no
        longer pay that seller in those coins: 1000 is US$10. A debt of any
        size older than 30 days stops them too. USDC keeps working, and each
        USDC sale pays toward the debt. 0 stops coin payments at the first
        cent owed.
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

    edge.market.backups.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Let a box keep its restic backups on the storage it bought. The edge
        runs restic's REST server on the bought volume, on its own node, and
        lets in only the box that bought it. restic on the box encrypts
        everything before it leaves, so the volume's replicas on other boxes,
        and the edge, hold only ciphertext.
      '';
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

    edge.market.stripeApi = lib.mkOption {
      type = lib.types.str;
      default = "https://api.stripe.com";
      visible = false;
      description = ''
        Where the Stripe gate reaches Stripe's API. Only the market VM test
        changes it, to point the gate at a stub on the edge itself.
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

    # ── Virtual machines on the mesh (edge side) ────────────────────────────
    # KubeVirt and CDI on the edge's rke2 cluster, sold through the market by
    # the replica (edge/registrar/src/vms.rs,
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

    # ── Models split across boxes (edge side, modules/edge-inference.nix) ───
    # Boxes lend memory (losos.inference.lend on the box); the registrar
    # groups lending boxes until a model fits, starts a llama.cpp head here
    # over their RPC servers through the tunnels, and bills buyers per token.
    edge.inference.enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Run large AI models split across the memory boxes lend, and sell
        their tokens. The registrar groups lending boxes until a model of
        `edge.inference.models` fits and starts a llama.cpp server for the
        group on this edge, which reaches each box's share through its
        tunnel.
      '';
    };

    edge.inference.package = lib.mkOption {
      type = lib.types.package;
      default = pkgs.llama-cpp.override { rpcSupport = true; };
      defaultText = lib.literalExpression "pkgs.llama-cpp.override { rpcSupport = true; }";
      description = "The llama.cpp build the edge's heads run: its `llama-server`, with RPC.";
    };

    edge.inference.models = lib.mkOption {
      type = lib.types.listOf (
        lib.types.submodule {
          options = {
            id = lib.mkOption {
              # As long as the registrar takes (inference::parse_models).
              type = lib.types.strMatching "[a-z0-9-]{1,64}";
              description = "The model's id in the catalogue and in the API, up to 64 characters.";
            };
            name = lib.mkOption {
              type = lib.types.str;
              description = "The model's name as buyers see it.";
            };
            file = lib.mkOption {
              type = lib.types.path;
              example = lib.literalExpression ''pkgs.fetchurl { url = "https://example.org/model.gguf"; hash = "sha256-..."; }'';
              description = "The model, one GGUF file. In the store, so every head reads the same bytes.";
            };
            needMiB = lib.mkOption {
              type = lib.types.ints.positive;
              description = "Lent memory, in MiB, a group of boxes needs in all to hold the model and its context.";
            };
            context = lib.mkOption {
              type = lib.types.ints.positive;
              default = 4096;
              description = "Context window of each head, in tokens.";
            };
            priceIn = lib.mkOption {
              type = lib.types.ints.positive;
              description = "Price of a million prompt tokens, in USDC base units (millionths of a dollar).";
            };
            priceOut = lib.mkOption {
              type = lib.types.ints.positive;
              description = "Price of a million generated tokens, in USDC base units (millionths of a dollar).";
            };
            licence = lib.mkOption {
              type = lib.types.str;
              example = "Apache-2.0";
              description = "The model's licence, shown to buyers.";
            };
          };
        }
      );
      default = [ ];
      description = "The models this edge offers. Written to /etc/losos/inference-models.json for the registrar.";
    };

    edge.inference.portRange = {
      from = lib.mkOption {
        type = lib.types.port;
        default = 23000;
        description = "First loopback port the tunnel server binds a lending box's RPC tunnel on.";
      };
      to = lib.mkOption {
        type = lib.types.port;
        default = 23999;
        description = ''
          Last loopback port for the lending boxes' RPC tunnels. The range
          holds at least as many ports as edge.tunnelPortRange: one per box.
        '';
      };
    };

    # ── Edge on the LAN ─────────────────────────────────────────────────────
    # The appliance looks for an edge proxy before it lets anyone share
    # storage (backend/src/edge.rs): at losos.proxy.registrarUrl, and on its
    # own network by DNS-SD. This is the network half: the edge publishes
    # `_losos-edge._tcp` over mDNS with a `url=` TXT record naming its
    # registrar API, and the box's browse through resolved finds it. Off by default
    # because a production edge is a VPS with no LAN to speak of; on for an
    # edge that sits beside the boxes (a home server, the two-VM demo in
    # nix/demo/edge-lan/), where it is what lets a box with no internet share.
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
        edge's own mDNS name, resolvable by every appliance (they resolve
        `.local` through systemd-resolved); set an IP literal when the LAN's
        mDNS is unreliable.
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
    # See edge/registrar/src/domains.rs and
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

    edge.dns.dnssec.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Sign the zone with DNSSEC (Knot's automatic signing, ECDSA P-256,
        keys in /var/lib/knot). A box that binds its wallet to its names
        gets an ENS record at each of them, and ENS reads a DNS name only
        from a signed zone. Signing does nothing for resolvers until the
        parent zone publishes the zone's DS record, which
        `keymgr <zone> ds` prints on the edge.
      '';
    };

    edge.dns.ech.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Encrypted Client Hello: an HTTPS record at every name in the zone
        carries the edge's ECH key, and Traefik decrypts with it, so the
        network between a browser and the edge sees only the public name,
        not which box was asked for. The key is made on the first start in
        /var/lib/losos-ech. No released Traefik decrypts ECH yet, so this
        runs Traefik built from its ECH change (nix/traefik-ech.nix).
      '';
    };

    edge.dns.ech.publicName = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      example = "edge.example.org";
      description = ''
        The name the outer, unencrypted handshake shows. It needs a
        certificate on the edge, because a browser checks it whenever the
        edge turns ECH down. `null` is `register.<publicDomain>`, which has
        one from the start.
      '';
    };

    # Custom domains for boxes behind a local edge
    # (handbook/docs/in-depth/edge-federation.md, "Custom domains behind a local
    # edge"). The official edge keeps the route table in the registrar's own
    # state and sends a domain to the local edge whose box proved it, once the
    # box has vouched for that local edge with a relay pass and joined this
    # edge's mesh. See edge/registrar/src/routes.rs.
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

    edge.lan.tunnelEndpoint = lib.mkOption {
      type = lib.types.str;
      default = "${config.networking.hostName}.local:${toString config.losos.edge.tunnelBindPort}";
      defaultText = lib.literalExpression ''"''${config.networking.hostName}.local:''${toString config.losos.edge.tunnelBindPort}"'';
      description = ''
        The `host:port` a box on the LAN dials its tunnel to, carried by the
        advertisement as the `tunnel=` TXT record beside `url=`. A box that
        picks this edge as its path (backend/src/edge.rs) points its tunnel
        client here instead of at losos.proxy.edgeTunnelEndpoint. Same
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

    # ── Clusters and bans (handbook/docs/in-depth/edge-federation.md,
    # "Three tiers") ─────────────────────────────────────────────────────
    edge.secondary.enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Let this gateway join a secondary cluster: gateways on one network
        that serve the same boxes and fail over for each other. Which
        cluster is runtime state, written from the manage page (start a
        cluster, or join one) into losos.edge.secondary.file and re-read every pass;
        with no file the gateway is a cluster of its own. Requires
        losos.edge.lan.advertise, because boxes find the members by their
        adverts.
      '';
    };

    edge.secondary.file = lib.mkOption {
      type = lib.types.str;
      default = "/var/lib/losos-edge/cluster.json";
      description = ''
        The runtime cluster file (`{name, member, secret, peers}`,
        edge/registrar/src/secondary.rs). Holds the cluster's shared secret,
        so it is 0600 and never a store path.
      '';
    };

    edge.secondary.interval = lib.mkOption {
      type = lib.types.str;
      default = "10s";
      description = "How often the gateways of a secondary cluster sync their boxes. Well under losos.edge.heartbeatTtl.";
    };

    edge.official.cluster = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      example = "eu";
      description = ''
        The official cluster this official edge belongs to: the official
        edges behind one load balancer. A DNS label. Reported to boxes and
        gateways at `GET /cluster/secondary`, and a ban may name the whole
        cluster.
      '';
    };

    edge.official.peers = lib.mkOption {
      type = lib.types.listOf (
        lib.types.submodule {
          options = {
            cluster = lib.mkOption {
              type = lib.types.str;
              description = "The official cluster of that edge.";
            };
            url = lib.mkOption {
              type = lib.types.str;
              description = "Its registrar API base URL.";
            };
            tunnel = lib.mkOption {
              type = lib.types.str;
              description = "Its tunnel endpoint, `host:port`.";
            };
          };
        }
      );
      default = [ ];
      description = ''
        The official edges this edge lists for boxes and gateways to spread
        over and to move to when one is banned, itself included or not. At
        most 64.
      '';
    };

    edge.bansFile = lib.mkOption {
      type = lib.types.str;
      default = "/var/lib/losos-registrar/bans.json";
      description = ''
        The ban list, `{"edges": [<url>], "clusters": [<official cluster>]}`,
        re-read on every uplink pass. A gateway whose hub, or the hub's
        official cluster, is banned moves its uplink to an official edge
        outside the ban. A missing or unreadable file bans nothing.
      '';
    };

    edge.manage.enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Serve the page a company's admins look after their boxes from, at
        `/manage/` on the registrar: every box this edge routes, and for each
        box whose owner let this edge manage it (the switch on the box's Mesh
        page), its LosOS Cloud, LosOS Git and app settings. The page signs in
        with a key the registrar makes on its first start, in
        /var/lib/losos-registrar/manage-key; a gateway's tty1 shows it with
        the page's address. Boxes refuse to be managed by an
        official LosOS edge, so leave this off on one.
      '';
    };

    edge.hidden.tor.enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Give every box this edge routes an address on Tor of its own, a
        `.onion` name that reaches the box through its tunnel the way its
        public name does. The addresses are made here and kept across
        restarts; each box learns its own on its next heartbeat and shows
        it on its Network page. The edge opens no port for it.
      '';
    };

    edge.hidden.i2p.enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Give every box this edge routes an address on I2P of its own, a
        `.b32.i2p` name, in the same way as edge.hidden.tor.enable. The edge
        takes part in I2P as a router with a small bandwidth share.
      '';
    };

    edge.manage.ui = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      description = ''
        The built admin UI whose `manage/` page the registrar serves. Null
        serves only the JSON routes. Set by modules/edge.nix to this flake's
        admin UI when losos.edge.manage.enable is on.
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
        keeps /etc/losos-tunnel/uplink.json, a tunnel *client* file with one
        service per relayed box, which `losos-tunnel-uplink.service` runs.
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
        one from the options below (fields: registrar_url, tunnel_endpoint,
        id, token_file, pin_file; paths, never secrets). The registrar re-reads
        it on every pass, and a missing file means "no uplink yet", so a
        gateway image can ship with the uplink enabled and let its owner
        fill the file in later from the manage page.
      '';
    };

    edge.uplink.registrarUrl = lib.mkOption {
      type = lib.types.str;
      default = "";
      example = "https://register.losos.cfd";
      description = "The hub's registrar API base URL. Required unless configFile is set.";
    };

    edge.uplink.tunnelEndpoint = lib.mkOption {
      type = lib.types.str;
      default = "edge.losos.cfd:2333";
      description = "The hub's tunnel server, `host:port` (UDP), that the uplink client dials.";
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
        token of every relayed tunnel service. Provision out of band.
      '';
    };

    edge.uplink.pinFile = lib.mkOption {
      type = secretPath;
      default = "/var/secrets/losos-uplink-tunnel-pin";
      description = ''
        Where the SHA-256 of the hub's tunnel certificate is pinned. Nothing
        to provision: when the file is absent the registrar fetches
        `<registrarUrl>/tunnel-pin` once and writes it (trust on first
        contact, as a box pins its edge); a file already there is never
        overwritten, so a hub whose certificate changed is refused.
      '';
    };

    edge.uplink.interval = lib.mkOption {
      type = lib.types.str;
      default = "30s";
      description = "How often the spoke posts `/relay` to the hub. Must be well under the hub's losos.edge.heartbeatTtl: the hub drops a silent spoke's boxes with the spoke.";
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
      # The newest release Longhorn lists as stable. The hash is the .tgz's
      # own sha256, the digest charts.longhorn.io/index.yaml publishes for it.
      default = {
        name = "longhorn";
        repo = "https://charts.longhorn.io";
        version = "1.12.1";
        hash = "sha256-yM9LNanYcs1ffkT9JtjmrHwquu5C9OLyoLDrvG46YRY=";
        targetNamespace = "longhorn-system";
        createNamespace = true;
        values = {
          # The edge is a small VPS and holds no data: only nodes labelled
          # node.longhorn.io/create-default-disk=true get a disk, and
          # modules/cluster.nix labels the appliances, never the edge.
          defaultSettings = {
            createDefaultDiskLabeledNodes = true;
            # No calls home: no version checks against Longhorn's upgrade
            # server, no usage reports.
            upgradeChecker = false;
            allowCollectingLonghornUsageMetrics = false;
          };
          # Nobody browses the Longhorn UI on an appliance mesh, and one copy
          # of each CSI controller is enough for a cluster this size; the
          # chart's default is three.
          longhornUI.replicas = 0;
          csi = {
            attacherReplicaCount = 1;
            provisionerReplicaCount = 1;
            resizerReplicaCount = 1;
            snapshotterReplicaCount = 1;
          };
        };
      };
      description = ''
        Longhorn Helm chart spec handed to services.rke2.autoDeployCharts
        (name, repo, version, hash, values). Longhorn is not packaged in
        nixpkgs, so it is deployed as a pinned chart, a fixed-output
        derivation, so the deployment stays reproducible. The default pins
        a stable release with a footprint cut for a small edge. Null
        disables Longhorn, and with it the market's storage orders, which
        claim their volumes from Longhorn's storage class.
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

  # ── Sign-in to the two apps with LosOS Login, as it actually runs ───────
  # LosOS Login runs (losos.identity.enable, with the admin pages it sits in
  # front of) and the app runs behind the front vhost, on a socket only nginx
  # may open (modules/app-sign-in.nix). Read by modules/identity.nix for the
  # routes and by modules/cloud.nix for its own settings, so the two cannot
  # disagree. Read-only.
  options.lososInternal.appSignIn =
    let
      identity =
        config.losos.identity.enable && config.losos.admin.enable && config.losos.admin.ui != null;
    in
    {
      git = lib.mkOption {
        type = lib.types.bool;
        internal = true;
        readOnly = true;
        default = identity && config.losos.git.enable;
        description = "LosOS Git signs people in with LosOS Login.";
      };
      cloud = lib.mkOption {
        type = lib.types.bool;
        internal = true;
        readOnly = true;
        default = identity && config.losos.cloud.enable && config.losos.backend.package != null;
        description = "LosOS Cloud runs, and signs people in with LosOS Login.";
      };
    };

  # Tunnel length of the box's and the edge's I2P services
  # (modules/hidden-lib.nix); null is i2pd's own default. Only a test's
  # private I2P network, with too few routers for three hops, sets it.
  options.lososInternal.hidden.i2pHops = lib.mkOption {
    type = lib.types.nullOr lib.types.ints.positive;
    internal = true;
    default = null;
    description = "Hops of each I2P tunnel of the box's or edge's addresses; null for i2pd's default.";
  };

  # ── The shared side ───────────────────────────────────────────────────
  # The shared user's apps (mail, chat) exist only while the box's storage
  # counts as shared: sharing on AND the box in a mesh. Sharing switched on
  # without a mesh to share with is not enough. Read-only.
  options.lososInternal.sharedSide = lib.mkOption {
    type = lib.types.bool;
    internal = true;
    readOnly = true;
    default = config.losos.sharingMyStorage && config.losos.cluster.enable;
    description = "The shared side is live: losos.sharingMyStorage and losos.cluster.enable.";
  };

  # ── The kindashared side ──────────────────────────────────────────────
  # The kindashared account's domain is open exactly while the box shares
  # with its local gateways (losos.sharingLocally). Whether a gateway is in
  # reach is lososd's to watch at run time: it refuses the switch without
  # one and pauses it when the gateways go away. Read-only.
  options.lososInternal.kindasharedSide = lib.mkOption {
    type = lib.types.bool;
    internal = true;
    readOnly = true;
    default = config.losos.sharingLocally;
    description = "The kindashared side is live: losos.sharingLocally.";
  };

  # ── Who runs the shared side's apps ───────────────────────────────────
  # Chat, mail, calls, Mesh CI and the hosted knowledge bases are one
  # instance each, and they run as whichever sharing account is live: `shared`
  # on a box in the mesh, else `kindashared` on a box that shares only with
  # its local gateways. Their data is in that account's own domain
  # (/home/<user>/data), so a box that moves from one side to the other
  # starts them afresh there and keeps the other side's files, locked with
  # their domain. Null: neither side is live and none of them run. Read-only.
  options.lososInternal.sideUser = lib.mkOption {
    type = lib.types.nullOr (
      lib.types.enum [
        "shared"
        "kindashared"
      ]
    );
    internal = true;
    readOnly = true;
    default =
      if config.lososInternal.sharedSide then
        "shared"
      else if config.lososInternal.kindasharedSide then
        "kindashared"
      else
        null;
    description = "The account the shared side's apps run as: shared in a mesh, kindashared with a local gateway only, else null.";
  };

  options.lososInternal.sideLive = lib.mkOption {
    type = lib.types.bool;
    internal = true;
    readOnly = true;
    default = config.lososInternal.sideUser != null;
    description = "The shared side's apps run: lososInternal.sideUser is set.";
  };

  # Whether this box runs chat, as it actually runs: the switch and the
  # sharing gate together. Chat belongs to the shared user and keeps its
  # data in the shared domain (modules/chat.nix). Read-only.
  options.lososInternal.chat = lib.mkOption {
    type = lib.types.bool;
    internal = true;
    readOnly = true;
    default =
      config.losos.chat.enable
      && config.lososInternal.sideLive
      && config.losos.identity.enable
      && config.losos.admin.enable
      && config.losos.admin.ui != null;
    description = "LosOS Chat runs: losos.chat.enable, the shared side's apps (lososInternal.sideLive) and LosOS Login.";
  };

  # ── Mesh CI, as it actually runs ────────────────────────────────────────
  # On the shared side, like mail, and only beside LosOS Git, whose pushes
  # and pull requests start the runs. modules/ci.nix, lososd's environment
  # and modules/containers.nix (the /ci page) read this one value. Read-only.
  options.lososInternal.ci.live = lib.mkOption {
    type = lib.types.bool;
    internal = true;
    readOnly = true;
    default =
      config.losos.ci.enable
      && config.lososInternal.sideLive
      && config.losos.git.enable
      && config.losos.backend.package != null;
    description = "Mesh CI runs: losos.ci.enable, the shared side (lososInternal.sideLive) and LosOS Git.";
  };

  # ── LosOS Mail, as it actually runs ─────────────────────────────────────
  # Mail is on the shared side: the switch and lososInternal.sharedSide.
  # modules/mail.nix (the server and lososd's environment) and
  # modules/containers.nix (the /mail/ page) read this one value, so the
  # page cannot outlive the server. Read-only.
  options.lososInternal.mail = {
    live = lib.mkOption {
      type = lib.types.bool;
      internal = true;
      readOnly = true;
      default =
        config.losos.mail.enable
        && config.lososInternal.sideLive
        && config.losos.admin.enable
        && config.losos.admin.ui != null;
      description = "LosOS Mail runs: losos.mail.enable, the shared side (lososInternal.sideLive) and the admin pages.";
    };
    domain = lib.mkOption {
      type = lib.types.str;
      internal = true;
      readOnly = true;
      default =
        if config.losos.mail.domain != null then
          config.losos.mail.domain
        else
          "${lib.toLower config.losos.hostName}.home.arpa";
      description = "The domain of this box's mail addresses.";
    };
  };

  # A directory the SFU writes every forwarded frame into, so a test can
  # show they are ciphertext (backend/src/calls/tap.rs). Tests only.
  options.lososInternal.callsTap = lib.mkOption {
    type = lib.types.nullOr lib.types.str;
    default = null;
    internal = true;
  };

  # The addresses the SFU takes media on, as losos-calls' `--address`
  # (`IP` or `IP=OFFERED`); empty for every address the box has. Tests set
  # it to reach the SFU through a forwarded port.
  options.lososInternal.callsAddresses = lib.mkOption {
    type = lib.types.listOf lib.types.str;
    default = [ ];
    internal = true;
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
        "losos.admin.tokenFile" = config.losos.admin.tokenFile;
        "losos.proxy.tokenFile" = config.losos.proxy.tokenFile;
        "losos.cluster.tokenFile" = config.losos.cluster.tokenFile;
        "losos.shared.fscrypt.keyFile" = config.losos.shared.fscrypt.keyFile;
        "losos.kindashared.fscrypt.keyFile" = config.losos.kindashared.fscrypt.keyFile;
        "losos.edge.cluster.agentTokenFile" = config.losos.edge.cluster.agentTokenFile;
        "losos.edge.market.stripeSecretKeySealed" = config.losos.edge.market.stripeSecretKeySealed;
        "losos.edge.market.webhookSecretSealed" = config.losos.edge.market.webhookSecretSealed;
        "losos.edge.uplink.tokenFile" = config.losos.edge.uplink.tokenFile;
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
