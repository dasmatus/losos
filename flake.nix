{
  description = "losos — stateless NixOS appliance: Nextcloud and Forgejo as k3s workloads, mesh storage and compute on rke2/Longhorn, auto-upgrade, midnight reboot, TPM-or-keyfile FDE";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    # Both follow nixpkgs. They are pure Nix libraries — a module set and a
    # partitioner — so they build nothing of their own and gain nothing from a
    # separate cache, while an unfollowed input drags a second nixpkgs (and,
    # via impermanence, a whole home-manager) into flake.lock to be fetched
    # and evaluated for no benefit.
    impermanence.url = "github:nix-community/impermanence";
    impermanence.inputs.nixpkgs.follows = "nixpkgs";
    disko.url = "github:nix-community/disko";
    disko.inputs.nixpkgs.follows = "nixpkgs";

    # devenv is deliberately NOT a flake input. The dev environment lives in
    # devenv.nix, driven by the devenv CLI against devenv.yaml (standalone
    # mode). Wiring it through the flake instead — devenv.lib.mkShell — fails
    # under pure evaluation: devenv needs an absolute path to the project root
    # for its .devenv state directory, cannot derive one during a pure eval,
    # and asserts "devenv was not able to determine the current directory".
    # The documented escape is a devenv-root input pointing at /dev/null plus
    # a direnv hook writing $PWD into a file, which needs --impure and would
    # make CI impure too. Standalone mode has neither problem.
    #
    # The cost is a second lock: devenv.lock pins its own nixpkgs. devenv.yaml
    # pins it to the same revision as flake.lock's nixpkgs, and the two must
    # be bumped together — see the header of devenv.yaml.
  };

  outputs =
    {
      self,
      nixpkgs,
      impermanence,
      disko,
    }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
      # The module list entry for one of the box-specific files, see the
      # `install` system below: the live copy under /etc/nixos when it can be
      # read (impure evaluation on the box), else the in-tree copy when the
      # tree has one, else nothing.
      onBox =
        name:
        let
          live = /etc/nixos/modules + "/${name}";
          shipped = ./modules + "/${name}";
        in
        if builtins.pathExists live then
          [ live ]
        else
          nixpkgs.lib.optional (builtins.pathExists shipped) shipped;
    in
    {
      # Flake packages: losos-ctl (the Rust lososd daemon + losos-ctl facade),
      # losos-registrar (the master-proxy edge) and losos-admin-ui (the static
      # admin SPA). See flake/packages.nix.
      packages.${system} =
        import ./flake/packages.nix { inherit pkgs; }
        # Bootable media, built off nixosConfigurations.install. Separate from
        # packages.nix because these are the only outputs that reach back into
        # `self` — they overlay the install system rather than building
        # alongside it — and because they are gigabytes: `devenv shell
        # build-media` builds everything matching `losos-disk-` and is
        # deliberately opt-in, kept out of `devenv test`.
        #
        # losos-disk-qcow2 is a DEMO medium and not the appliance: it gives up
        # full-disk encryption, the TPM and the tmpfs root, because a file
        # cannot carry a key sealed to hardware it does not have. The header of
        # flake/disk-images.nix says so at length; read it before handing the
        # image to anyone as a way to actually run this.
        // import ./flake/disk-images.nix { inherit self nixpkgs pkgs; };

      # The dev environment is devenv.nix, entered with `devenv shell` (see
      # devenv.yaml). This devShell exists so a bare `nix develop` still
      # resolves for anyone without the devenv CLI, and so `nix develop -c`
      # keeps working in scripts.
      #
      # It carries the toolchain ONLY — no scripts, no hooks, no shell UX.
      # That is the point: it cannot drift from devenv.nix in any way that
      # matters, because the things worth drifting (the lint/test/build
      # commands, which must be byte-identical between CI and a developer)
      # live in exactly one place, devenv.nix, and are not duplicated here.
      devShells.${system} =
        let
          # The toolchain both shells below share, and the same one
          # buildRustPackage compiles with, because both come from this
          # flake's pinned nixpkgs.
          rust = [
            pkgs.cargo
            pkgs.rustc
            pkgs.clippy
            pkgs.rustfmt
          ];
          # mold as the linker, ccache and sccache in front of the compilers
          # — the same stdenv flake/packages.nix builds the crates with, so
          # `cargo` in either shell links and caches the way `nix build`
          # does. The stdenv carries ccache and mold (so `cc` on PATH is
          # both); `env` carries sccache. See the header of
          # flake/fast-build.nix for what each tool buys and for the one
          # piece (edge-vercel on Vercel) it cannot reach.
          fast = import ./flake/fast-build.nix { inherit pkgs; };
          mkShell = pkgs.mkShell.override { inherit (fast) stdenv; };
        in
        {
          default = mkShell {
            nativeBuildInputs =
              rust
              ++ fast.packages
              ++ [
                pkgs.rust-analyzer
                pkgs.nodejs
              ];
            inherit (fast) env;
            shellHook = ''
              echo "Toolchain-only shell. For the scripts and pre-commit hooks:"
              echo "  devenv shell      (or: direnv allow)"
            '';
          };

          # What CI lints in. Deliberately lean — no rust-analyzer, no node,
          # no shell UX — because it is realised from scratch on every CI run
          # against a 10-minute cap.
          #
          # It exists so lint uses the SAME rustc as `nix build .#losos-ctl`.
          # The lint job used to run on docker.io/rust:latest, which is a
          # different and floating Rust: it went red on a tree that both this
          # flake's toolchain and the maintainer's local one call clean, which
          # is version skew, not a defect in the code. Linting with the
          # compiler you ship with is the only way that stays true, and it
          # also means an upstream Rust release cannot turn CI red on its own.
          ci = mkShell {
            nativeBuildInputs = rust ++ fast.packages;
            inherit (fast) env;
          };
        };

      # Both systems get `self` via specialArgs: the iso system reaches
      # self.packages.${system}.losos-ctl to wire the installer binary, the
      # install system so defaults.nix can reach
      # self.packages.${system}.{losos-ctl,losos-admin-ui}.
      nixosConfigurations = {
        # Installer medium: a minimal NixOS live ISO carrying the losos
        # auto-installer (losos-install). Boot it on the target machine and run
        # `losos-install` — it finds every fixed disk, merges them into one LVM
        # volume group via disko, and installs. The ISO imports ./modules/disko.nix
        # only so the layout evaluates alongside losos.targetDrives; the format
        # actually run against the target disk comes from the flake clone the
        # installer makes at run time, not from this evaluation (see
        # disko.enableConfig below).
        iso = nixpkgs.lib.nixosSystem {
          inherit system;
          specialArgs.self = self;
          modules = [
            impermanence.nixosModules.impermanence
            disko.nixosModules.disko
            (
              {
                modulesPath,
                pkgs,
                self,
                ...
              }:
              {
                imports = [ (modulesPath + "/installer/cd-dvd/installation-cd-minimal.nix") ];
                environment.systemPackages = [
                  pkgs.disko
                  pkgs.cryptsetup
                ];
                # The installer binary (losos-install wraps `losos-ctl install`,
                # both built from the same Rust crate). installer.nix packages
                # it; self.packages is in scope via specialArgs.
                losos.installer.package = self.packages.${system}.losos-ctl;
                # Booting the ISO auto-runs losos-install as root's login shell,
                # which presents the firmware-mode menu before destructive work.
                losos.installer.autorun = true;
                # Prebuild the admin UI into the medium.
                #
                # `losos-admin-ui` is a React/Tailwind bundle built with
                # buildNpmPackage — a Node toolchain and an npm dependency
                # fetch. Without this, `nixos-install` realises that derivation
                # on the target, so a repurposed mini-PC with no shell runs npm
                # as part of a first install, over a network it may not have.
                # storeContents puts the finished output in the live system's
                # store, and nix copies rather than builds anything it already
                # has.
                #
                # Cheap, unlike embedding the whole appliance closure: this is
                # one small output, so the medium stays well inside GitHub's
                # 2 GiB per-asset release limit.
                isoImage.storeContents = [
                  self.packages.${system}.losos-admin-ui
                  # Same argument: the handbook is a Docusaurus build, npm and
                  # all, and belongs on the medium rather than on the target.
                  self.packages.${system}.losos-handbook
                ];
                # The default (zstd level 19, cache sized off host RAM) gets
                # OOM-killed on the former Codeberg CI runners (exit 137): the
                # container sees the host's RAM but runs under a smaller
                # cgroup limit. Level 6 + a 1 GiB cap trades a slightly
                # larger ISO for a build that fits; boot speed is unaffected.
                isoImage.squashfsCompression = "zstd -Xcompression-level 6 -mem 1G";
                # The live medium must not inherit the *target's* disk layout.
                # Left on, disko turns ./modules/disko.nix's `disko.devices`
                # into real fileSystems."/persist", boot.initrd.luks.devices
                # .persist and swapDevices entries in this ISO's own config —
                # so stage 1 waits forever for a LUKS volume that exists only
                # on the machine being installed. tests/install.nix sets the
                # same guard for the same reason.
                disko.enableConfig = false;
                # No flake source is baked into the ISO: losos-install clones
                # LOSOS_FLAKE_URL (default: the public GitHub repo) into a
                # writable work dir at run time, drops in
                # modules/install-target.nix, and runs disko + nixos-install
                # against the clone. The ISO needs network for that clone —
                # and would anyway, since flake evaluation fetches nixpkgs.
              }
            )
            ./modules/options.nix
            # Both systems import this. The installer medium needs the substituter
            # as much as the installed box does: the whole cost is realising
            # the 2.3 GiB Nextcloud image during `nixos-install`.
            ./modules/cache.nix
            ./modules/disko.nix
            ./modules/installer.nix
            # The medium's UEFI loader as one signable unified kernel image
            # in place of GRUB, so a firmware under Secure Boot can verify
            # it. `nix build` leaves it unsigned; CI and the release job
            # sign it in place with `losos-sign-iso` (see that module's
            # header and tests/secure-boot.nix).
            ./modules/secure-boot.nix
            # LosOS and its release tag, not NixOS, on everything the medium
            # shows: boot menu, splash, volume label, console banner, host
            # name, os-release.
            ./modules/branding.nix
            ./modules/live-branding.nix
          ];
        };
        # The target system installed onto the machine.
        install = nixpkgs.lib.nixosSystem {
          inherit system;
          # Make the flake `self` available to modules so defaults.nix can
          # reach self.packages.${system}.{losos-ctl,losos-admin-ui}.
          specialArgs.self = self;
          modules = [
            impermanence.nixosModules.impermanence
            disko.nixosModules.disko
            ./modules/options.nix
            ./modules/configuration.nix
            ./modules/cache.nix
            # HTTPS from a certificate the box generates itself. `install`
            # only: the live medium serves nothing.
            ./modules/tls.nix
            # The first-run setup routes: the certificate tls.nix generates,
            # served for download, and the facts the wizard needs before it
            # holds an admin token. Must come with containers.nix — it merges
            # its locations into that file's front vhost.
            ./modules/setup.nix
            # Where lososd keeps the appliance recovery code. One environment
            # variable, but it must be imported for the daemon to agree with
            # modules/impermanence.nix about a file that has to outlive a
            # factory reset — see that module's header.
            ./modules/recovery.nix
            # Imported by `install` only, never by `iso`: the live medium must
            # keep squashfs loadable and its module policy permissive, and
            # hardening an installer that is discarded at the end of the
            # install protects nothing.
            ./modules/hardening.nix
            ./modules/impermanence.nix
            ./modules/disko.nix
            ./modules/boot.nix
            # LosOS and its release tag in the boot menu, os-release and the
            # console banners.
            ./modules/branding.nix
            # The banner on tty1 that tells whoever is standing at the box
            # which address to open in a browser.
            ./modules/console.nix
            ./modules/services.nix
            ./modules/nextcloud-common.nix
            ./modules/containers.nix
            # The local k3s cluster + the mesh rke2 agent, and the workload
            # manifests the local cluster runs. These must be imported together
            # with containers.nix: that file now proxies /nextcloud and
            # /forgejo to loopback ports that only these two modules make
            # anything listen on, so dropping either leaves the front door
            # serving 502s.
            ./modules/cluster.nix
            ./modules/workloads.nix
            # The fscrypt policy on the shared data domain, locked and unlocked
            # off losos.sharingMyStorage.
            ./modules/fscrypt.nix
            # Seals the secrets this box mints for itself with systemd-creds,
            # so a copy of /persist that leaves the machine (a backup, a
            # Longhorn replica, a support image) carries ciphertext rather than
            # the Nextcloud admin password in plaintext. Next to fscrypt.nix
            # because the two use the same sealing mechanism and cover
            # different halves of the same problem.
            ./modules/keyring.nix
            ./modules/daemon.nix
            # Backups to the owner's S3 bucket, restoring them, and the
            # boot-time wipe of an erase (backend/src/{backup,erase}.rs).
            ./modules/backup.nix
            # LosOS Lab's guests under libvirt on the box (losos.lab.libvirt,
            # off by default): the `losos-registrar lab` helper lososd relays
            # /api/lab to.
            ./modules/lab.nix
            # The option document and the configuration repository on LosOS
            # Git (modules/config-repo.nix, backend/src/{options,config_repo}.rs).
            ./modules/config-repo.nix
            # modules/overrides.nix is imported through onBox below, never here.
            ./modules/updates.nix
            ./modules/defaults.nix
            ./modules/proxy.nix
          ]
          # The two files that describe *this* box rather than the appliance:
          # modules/install-target.nix (drive list, firmware mode, unlock
          # mode, written by losos-install at install time; not in the
          # published tree) and modules/overrides.nix (the settings the admin
          # UI writes through lososd; the published tree carries only the
          # defaults). A local evaluation — the installer's work dir, lososd's
          # /etc/nixos#install, the default git+file:///etc/nixos upgrade —
          # sees them in-tree. A remote upgradeFlakeUri (`github:…#install`)
          # evaluates the published tree, which has neither, and used to flip
          # a keyfile box to the TPM shape, forget its drives and firmware
          # mode, and reset every setting. So the live copies on the box are
          # preferred when they can be read: updates.nix and lososd pass
          # `--impure` to nixos-rebuild for exactly this. Under pure
          # evaluation (CI, `nix flake check`, tests/invariants.nix, the
          # installer) builtins.pathExists on an absolute path is false, not
          # an error, so the in-tree file or the defaults apply as before.
          # Only one copy of each is ever imported: two would double-define
          # every option in it.
          ++ onBox "overrides.nix"
          ++ onBox "install-target.nix";
        };

      };

      # The master-proxy edge: Traefik (master), a rathole server, and
      # losos-registrar (serve) on a public VPS. Exported as a module, not a
      # nixosConfiguration: the VPS flake imports this next to its own
      # hardware/bootloader config and sets losos.edge.* (a bare
      # nixosConfiguration would fail the root-fs/bootloader assertions and
      # could not be configured from outside anyway). `self` is baked in so
      # losos.edge.registrar.package resolves to this flake's registrar.
      # The edge of the two-VM LAN demo (demo/edge-lan/): the edge module on a
      # QEMU VM that is also the LAN's router. `nix build
      # .#nixosConfigurations.edge-demo.config.system.build.vm` makes the
      # run script; demo/edge-lan/run.sh drives it beside the installer ISO.
      nixosConfigurations.edge-demo = nixpkgs.lib.nixosSystem {
        inherit system;
        modules = [
          self.nixosModules.edge
          "${nixpkgs}/nixos/modules/virtualisation/qemu-vm.nix"
          ./demo/edge-lan/edge-vm.nix
        ];
      };
      nixosModules.edge = {
        imports = [
          ./modules/options.nix
          ./modules/edge.nix
          ./modules/edge-vms.nix
          ./modules/edge-gateway.nix
        ];
        _module.args.self = self;
      };
      # The edge gateway (wiki/Edge-Federation.md): the edge module with LAN
      # advertising, open enrolment and a runtime uplink, as a VM image for a
      # LAN that has no NixOS machine to import nixosModules.edge on.
      # `nix build .#losos-disk-edge-qcow2` images it (flake/disk-images.nix);
      # this configuration is what the image boots.
      nixosConfigurations.edge-gateway = nixpkgs.lib.nixosSystem {
        inherit system;
        modules = [
          self.nixosModules.edge
          ./flake/edge-gateway-vm.nix
        ];
      };

      # nixos-test-vms. Run with `nix build .#checks.x86_64-linux.<name>`:
      #   losos-install       — boots a VM with three empty disks and runs the
      #                         installer end-to-end through the disko format/mount
      #                         path (tests/install.nix).
      #   losos-tpm-unlock    — boots a VM with swtpm, runs the installer's
      #                         format + TPM2 enrolment against the production
      #                         layout, reboots into it and asserts /persist
      #                         unlocks from the chip unattended (tests/tpm.nix).
      #   losos-admin-daemon  — boots a minimal losos appliance and exercises the
      #                         lososd daemon + losos-ctl facade + the Bearer-
      #                         authed admin HTTP API (tests/admin-vm.nix).
      #   losos-edge-proxy    — boots an edge VM (Traefik + rathole server +
      #                         registrar) + an appliance VM (rathole client +
      #                         announce + Nginx) and asserts the master-proxy
      #                         register/reconcile/forward path (tests/edge-vm.nix).
      #   losos-edge-market   — boots an edge with the market on and asserts the
      #                         Stripe gate's production wiring: sealed secrets,
      #                         DynamicUser, a 0600 socket, blobs hidden from the
      #                         registrar, a signed webhook through the gate, and a
      #                         missing blob that darkens only the market
      #                         (tests/market-vm.nix).
      #   losos-edge-dns      — boots an edge with losos.edge.dns on and a
      #                         client that queries it: Knot serves the zone the
      #                         registrar renders, names appear only once the
      #                         edge holds a certificate, the path unit reloads
      #                         the zone, and the domain routes answer
      #                         (tests/edge-dns.nix).
      #   losos-ds-render     — builds the losos-ds npm package and renders every
      #                         design-system component in headless chromium
      #                         (playwright-driver.browsers), asserting computed
      #                         styles from tokens.css (tests/design-system.nix).
      #   losos-front-vhost   — boots two appliance VMs and asserts the Nginx
      #                         front door: the admin surface is LAN-only (403
      #                         from loopback), /nextcloud + /forgejo/ are not,
      #                         and the security headers land. There is no
      #                         container-subnet case any more — hostNetwork
      #                         pods have no address of their own, so the deny
      #                         rule that used to carry it is gone on purpose
      #                         (tests/front-vhost.nix).
      #   losos-impermanence  — boots a tmpfs-root VM with a real /persist,
      #                         reboots it, and asserts that exactly the
      #                         persisted set survives (tests/impermanence.nix).
      #   losos-cluster       — boots an edge VM running the mesh rke2 server and
      #                         an appliance VM running BOTH Kubernetes instances,
      #                         and asserts they coexist: separate containerd
      #                         sockets, kubelet ports and state dirs, both nodes
      #                         registered, and — the property the two-cluster
      #                         split exists to buy — this box's own workload
      #                         still answering on loopback with the edge VM
      #                         crashed and after a reboot (tests/cluster-vm.nix).
      #   losos-setup         — boots one appliance and asserts the first-run
      #                         setup routes: the certificate downloads byte for
      #                         byte from /setup/losos-ca.crt with a type and a
      #                         filename a browser can act on, /setup/state.json
      #                         fingerprints the certificate that is actually on
      #                         disk, and both are LAN-only (tests/setup.nix).
      #   losos-console       — boots one VM and asserts the address banner owns
      #                         tty1 instead of getty and shows the LAN address
      #                         and the .local name (tests/console.nix).
      #   losos-forgejo-federation — boots the real Forgejo in native mode with
      #                         losos.forgejo.federation on and asks it what the
      #                         fediverse would (nodeinfo, the server actor, an
      #                         account actor refusing an unsigned request), reads
      #                         the same section off the container manifest's
      #                         losos.ini, then switches federation off and
      #                         asserts the endpoints are gone
      #                         (tests/forgejo-federation.nix).
      #   losos-nextcloud-httpd — not a VM: the Nextcloud pod's Apache on a
      #                         fixture webroot, asserting the URL map
      #                         (tests/nextcloud-httpd.nix).
      #   losos-secure-boot   — the signed installer ISO under OVMF Secure Boot
      #                         with a throwaway key enrolled beside Microsoft's
      #                         keys: boots and proves it from inside, and a
      #                         Microsoft-signed shim still starts; Microsoft-only
      #                         keys, a tampered copy and the unsigned build are
      #                         refused
      #                         (tests/secure-boot.nix).
      #   losos-edge-lan-two-boxes — the mesh verification walkthrough as a
      #                         check: two boxes with the real admin UI and an
      #                         edge on one LAN, scenario A (no edge: both
      #                         refuse sharing, serve locally, survive a reboot)
      #                         then B (edge on: both find it, the gate opens,
      #                         edge off/on, an unsigned edge opens sharing but
      #                         not trading). With LOSOS_RECORD_DIR set the
      #                         same run is photographed by
      #                         demo/edge-lan/record.mjs (demo/edge-lan/two-boxes.nix).
      #   losos-erase         — a backup through lososd to a MinIO bucket holds
      #                         only ciphertext; an erase can be cancelled, and
      #                         one that runs out wipes the box at boot; a
      #                         restore with the recovery code brings files,
      #                         database rows and the fscrypt folder back
      #                         (tests/erase.nix).
      checks.${system} = {
        losos-install = import ./tests/install.nix { inherit pkgs disko; };
        losos-tpm-unlock = import ./tests/tpm.nix { inherit pkgs disko; };
        losos-admin-daemon = import ./tests/admin-vm.nix { inherit pkgs; };
        losos-edge-proxy = import ./tests/edge-vm.nix { inherit pkgs; };
        losos-edge-lan = import ./tests/edge-lan.nix { inherit pkgs; };
        losos-edge-federation = import ./tests/edge-federation.nix { inherit pkgs; };
        losos-edge-lan-two-boxes = import ./demo/edge-lan/two-boxes.nix { inherit pkgs; };
        losos-edge-market = import ./tests/market-vm.nix { inherit pkgs; };
        losos-edge-dns = import ./tests/edge-dns.nix { inherit pkgs; };
        losos-ds-render = import ./tests/design-system.nix { inherit pkgs; };
        losos-front-vhost = import ./tests/front-vhost.nix { inherit pkgs; };
        losos-impermanence = import ./tests/impermanence.nix { inherit pkgs impermanence; };
        losos-cluster = import ./tests/cluster-vm.nix { inherit pkgs; };
        losos-hardening = import ./tests/hardening.nix { inherit pkgs; };
        losos-resize = import ./tests/resize.nix { inherit pkgs; };
        losos-tls = import ./tests/tls.nix { inherit pkgs; };
        losos-setup = import ./tests/setup.nix { inherit pkgs; };
        losos-admin-ui = import ./tests/admin-ui.nix {
          inherit pkgs;
          # The real option document, so the Advanced pane is drawn from
          # every option the install configuration declares and the check
          # fails on any row it cannot draw (tests/advanced.browser.mjs).
          optionsJson = self.checks.${system}.losos-options-doc;
          # LosOS Lab's page imports the Rust core, the GPU canvas and the
          # libvirt client; the bundle under test copies them in the way the
          # shipped one does.
          inherit (self.packages.${system}.losos-admin-ui) labCorePkg labVirtPkg labRenderPkg;
        };
        # Not a VM: the option document of the install configuration
        # (flake/options-doc.nix via modules/config-repo.nix), forced here so
        # `nix flake check --no-build` fails the moment an option is declared
        # with a type the Advanced pane has no editor for.
        losos-options-doc =
          self.nixosConfigurations.install.config.environment.etc."losos/options.json".source;
        losos-keyring = import ./tests/keyring.nix { inherit pkgs; };
        losos-erase = import ./tests/erase.nix { inherit pkgs impermanence; };
        losos-console = import ./tests/console.nix { inherit pkgs; };
        # The real Forgejo with losos.forgejo.federation on, asked what another
        # server would ask, in both deployment modes; then off, and gone
        # (tests/forgejo-federation.nix).
        losos-forgejo-federation = import ./tests/forgejo-federation.nix { inherit pkgs; };
        # The signed installer medium under OVMF's Secure Boot build: the
        # real ISO (plus the test backdoor) signed with a throwaway key the
        # sandbox makes, boots with that key enrolled beside Microsoft's and
        # proves it from inside, and Ubuntu's Microsoft-signed shim starts on
        # the same firmware; the same medium is refused by a firmware holding only
        # Microsoft's keys, a tampered copy is refused, and so is the
        # unsigned build (tests/secure-boot.nix). Builds the ISO, so it is
        # the slowest check here.
        losos-secure-boot = import ./tests/secure-boot.nix {
          inherit pkgs;
          iso = self.nixosConfigurations.iso;
          inherit (self.packages.${system}) losos-sign-iso;
        };
        # Not a VM either: starts the Nextcloud pod's httpd and php-fpm in the
        # build sandbox on the config text flake/images.nix bakes, against a
        # fixture webroot, and asserts which entry point every URL shape
        # reaches — the pretty /nextcloud/apps/<app>/ links the homepage
        # tiles use included (tests/nextcloud-httpd.nix).
        losos-nextcloud-httpd = import ./tests/nextcloud-httpd.nix { inherit pkgs; };
        # Not a VM: evaluates the install configuration and asserts on the
        # merged option values, so `nix flake check --no-build` turns red the
        # moment a load-bearing default drifts (tests/invariants.nix).
        losos-invariants = import ./tests/invariants.nix {
          inherit pkgs;
          inherit (nixpkgs) lib;
          inherit (self.nixosConfigurations.install) config;
          # The same configuration with disk sharing forced each way, for
          # the federation gate (losos.sharingMyStorage).
          sharing =
            on:
            (self.nixosConfigurations.install.extendModules {
              modules = [ { losos.sharingMyStorage = nixpkgs.lib.mkForce on; } ];
            }).config;
        };
      };
    };
}
