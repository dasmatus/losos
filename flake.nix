{
  description = "losos — stateless NixOS appliance: LosOS Cloud, LosOS Git, mesh storage and compute on rke2/Longhorn, auto-upgrade, midnight reboot, TPM-or-keyfile FDE";

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

    # pm, the LosOS package manager (nix/pm.nix): Mesh CI starts each job's
    # containers through it and builds workflows' pm recipes with it. Its
    # source only, built here with this flake's nixpkgs. Pinned to the newest
    # master whose root Cargo.lock still agrees with its Cargo.toml: from PR
    # #55 on the lock carries wit-component 0.262 while Cargo.toml asks for
    # 0.261, so an offline build cannot resolve it.
    pm.url = "git+https://github.com/losos-project/pm?ref=master&rev=30abe308587967a71ee9142915d735d784430a9e";
    pm.flake = false;

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
      pm,
    }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
      # The release this tree is, spelled like its Git tag ("v0.4.0"). CI
      # keeps the file on the newest tag; see its header.
      version = import ./nix/version.nix;
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
      # admin SPA). See nix/packages.nix.
      packages.${system} =
        import ./nix/packages.nix {
          inherit pkgs version;
          pmSrc = pm;
        }
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
        # nix/disk-images.nix says so at length; read it before handing the
        # image to anyone as a way to actually run this.
        // import ./nix/disk-images.nix { inherit self nixpkgs pkgs; }
        # The box CI's discovery jobs boot on Linux, macOS and Windows
        # runners (nix/tests/discovery/box.nix): test media, not a release,
        # under the `losos-disk-` prefix so `build-pkgs` leaves it to
        # `build-media discovery`.
        // {
          losos-disk-discovery-box = self.nixosConfigurations.discovery-box.config.system.build.discoveryBox;
        };

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
            # The wasm32 linker for lososd's plugins (backend/plugins), which
            # backend's tests/plugins.rs builds.
            pkgs.lld
          ];
          # mold as the linker, ccache and sccache in front of the compilers
          # — the same stdenv nix/packages.nix builds the crates with, so
          # `cargo` in either shell links and caches the way `nix build`
          # does. The stdenv carries ccache and mold (so `cc` on PATH is
          # both); `env` carries sccache. See the header of
          # nix/fast-build.nix for what each tool buys and for the one
          # piece (edge/vercel on Vercel) it cannot reach.
          fast = import ./nix/fast-build.nix { inherit pkgs; };
          mkShell = pkgs.mkShell.override { inherit (fast) stdenv; };
        in
        {
          default = mkShell {
            nativeBuildInputs =
              rust
              ++ fast.packages
              ++ [
                pkgs.rust-analyzer
                pkgs.bun
              ];
            inherit (fast) env;
            # The git hooks (nix/hooks) run CI's gates before a commit
            # or a push; this shell installs them as devenv's does.
            shellHook = ''
              root=$(git rev-parse --show-toplevel 2>/dev/null) && bash "$root/nix/hooks/install"
              echo "Toolchain-only shell. For the scripts:"
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
                lib,
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
                # Booting the ISO runs the installer on tty1, under the boot
                # screen where it draws, which presents the firmware-mode menu
                # before destructive work.
                losos.installer.autorun = true;
                # The GitHub App problem reports sign in with: CI's Actions
                # variable LOSOS_ABRT_VAR, read only by an `--impure` build
                # (pure evaluation sees ""). The medium's installer gets it
                # as its unit's credential `report-client-id` and hands it to
                # the box it installs (modules/installer.nix,
                # modules/problems.nix). Never written in this tree.
                losos.report.clientId =
                  let
                    id = builtins.getEnv "LOSOS_ABRT_VAR";
                  in
                  lib.mkIf (id != "") (lib.mkDefault id);
                # Prebuild the admin UI into the medium.
                #
                # `losos-admin-ui` is a React/Tailwind bundle built with Bun
                # from a node_modules fetch. Without this, `nixos-install`
                # realises that derivation on the target, so a repurposed
                # mini-PC with no shell fetches packages and runs Bun as part
                # of a first install, over a network it may not have.
                # storeContents puts the finished output in the live system's
                # store, and nix copies rather than builds anything it already
                # has.
                #
                # Cheap, unlike embedding the whole appliance closure: this is
                # one small output, so the medium stays well inside GitHub's
                # 2 GiB per-asset release limit.
                isoImage.storeContents = [
                  self.packages.${system}.losos-admin-ui
                  # Same argument: the handbook is a Docusaurus build, its
                  # packages and all, and belongs on the medium rather than
                  # on the target.
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
                # on the machine being installed. nix/tests/install.nix sets the
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
            # the workload images during `nixos-install`.
            ./modules/cache.nix
            ./modules/disko.nix
            ./modules/installer.nix
            # The medium's UEFI loader as one signable unified kernel image
            # in place of GRUB, so a firmware under Secure Boot can verify
            # it. `nix build` leaves it unsigned; CI and the release job
            # sign it in place with `losos-sign-iso` (see that module's
            # header and nix/tests/secure-boot.nix).
            ./modules/secure-boot.nix
            # LosOS and its release tag, not NixOS, on everything the medium
            # shows: boot menu, splash, volume label, console banner, host
            # name, os-release.
            ./modules/branding.nix
            ./modules/live-branding.nix
            # The boot screen, which hosts the installer: its menu, its
            # steps and how it ended on the panel under the salmon.
            ./modules/splash.nix
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
            # losos.tpm.tang: the disk opens only with the TPM2 and a Tang
            # server on the box's own network.
            ./modules/network-unlock.nix
            # LosOS and its release tag in the boot menu, os-release and the
            # console banners.
            ./modules/branding.nix
            # The banner on tty1 that tells whoever is standing at the box
            # which address to open in a browser.
            ./modules/console.nix
            # The boot screen, which draws that same status under the logo
            # and stays up after boot.
            ./modules/splash.nix
            # The end of the launch film's score, played when the box is
            # switched on.
            ./modules/startup-sound.nix
            # The model the widget builder asks, on loopback.
            ./modules/builder.nix
            ./modules/services.nix
            ./modules/containers.nix
            # The local k3s cluster (the `shared` apps) and the mesh rke2
            # agent.
            ./modules/cluster.nix
            # The fscrypt policy on the shared data domain, locked and unlocked
            # off losos.sharingMyStorage.
            ./modules/fscrypt.nix
            # The credential manager: every secret of the box sealed with
            # systemd-creds in /etc/credstore.encrypted and handed to the
            # services that read it by LoadCredentialEncrypted=, so a copy of
            # /persist that leaves the machine carries ciphertext. Next to
            # fscrypt.nix because the two use the same sealing mechanism and
            # cover different halves of the same problem.
            ./modules/credentials.nix
            ./modules/daemon.nix
            # What went wrong on the box, as reports the owner may send
            # (backend/plugins/problems).
            ./modules/problems.nix
            ./modules/identity.nix
            # LosOS Chat: Synapse as the shared user while the box shares
            # its storage in a mesh, behind LosOS Login (backend/src/chat.rs).
            ./modules/chat.nix
            # Calls from chat rooms and DMs: the SFU, as the shared user, which
            # forwards only frames the browsers encrypted (backend/src/calls/).
            ./modules/calls.nix
            # Backups to the owner's S3 bucket, restoring them, and the
            # boot-time wipe of an erase (backend/src/{backup,erase}.rs).
            ./modules/backup.nix
            # Apps installed from the catalogue into the box's own cluster
            # (backend/src/apps.rs): the Helm job, its shaping plugin and the
            # LAN forwards.
            ./modules/apps.nix
            # Text inside the owner's files for the admin pages' search
            # (backend/src/textsearch/): the cloud index and the Git search.
            ./modules/search.nix
            # Memory lent to models split across boxes (losos.inference.lend,
            # off by default): a llama.cpp RPC server the edge reaches through
            # this box's tunnel.
            ./modules/inference-lend.nix
            # LosOS Cloud (backend/src/cloud/): files, calendars, contacts and
            # boards at /cloud/, and to the Nextcloud desktop and phone apps.
            ./modules/cloud.nix
            # Office documents in the browser: Collabora Online and its WOPI
            # host (backend/src/office/).
            ./modules/office.nix
            # Its files in Windows' Network view, as an SMB share.
            ./modules/windows.nix
            # LosOS Lab's guests under libvirt on the box (losos.lab.libvirt,
            # off by default): the `losos-registrar lab` helper lososd relays
            # /api/lab to.
            ./modules/lab.nix
            # LosOS Git, the box's Git server (backend/src/forge/).
            ./modules/git.nix
            # LosOS Mail: Stalwart as the shared user, only while sharing.
            ./modules/mail.nix
            # Mesh CI: GitHub Actions workflows on the boxes of the mesh,
            # only while sharing (backend/src/ci/).
            ./modules/ci.nix
            # The option document and the configuration repository on LosOS
            # Git (modules/config-repo.nix, backend/src/{options,config_repo}.rs).
            ./modules/config-repo.nix
            # modules/overrides.nix is imported through onBox below, never here.
            ./modules/updates.nix
            ./modules/logs.nix
            # Settings › Logs: the journal for the admin pages, apart from
            # lososd (backend/src/logs/).
            ./modules/log-reader.nix
            ./modules/defaults.nix
            ./modules/proxy.nix
            # The box on Tor and I2P (losos.hidden.*).
            ./modules/hidden-services.nix
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
          # evaluation (CI, `nix flake check`, nix/tests/invariants.nix, the
          # installer) builtins.pathExists on an absolute path is false, not
          # an error, so the in-tree file or the defaults apply as before.
          # Only one copy of each is ever imported: two would double-define
          # every option in it.
          ++ onBox "overrides.nix"
          ++ onBox "install-target.nix";
        };

      };

      # The master-proxy edge: Traefik (master), the QUIC tunnel server, and
      # losos-registrar (serve) on a public VPS. Exported as a module, not a
      # nixosConfiguration: the VPS flake imports this next to its own
      # hardware/bootloader config and sets losos.edge.* (a bare
      # nixosConfiguration would fail the root-fs/bootloader assertions and
      # could not be configured from outside anyway). `self` is baked in so
      # losos.edge.registrar.package resolves to this flake's registrar.
      # The edge of the two-VM LAN demo (nix/demo/edge-lan/): the edge module on a
      # QEMU VM that is also the LAN's router. `nix build
      # .#nixosConfigurations.edge-demo.config.system.build.vm` makes the
      # run script; nix/demo/edge-lan/run.sh drives it beside the installer ISO.
      nixosConfigurations.edge-demo = nixpkgs.lib.nixosSystem {
        inherit system;
        modules = [
          self.nixosModules.edge
          "${nixpkgs}/nixos/modules/virtualisation/qemu-vm.nix"
          ./nix/demo/edge-lan/edge-vm.nix
        ];
      };
      nixosModules.edge = {
        imports = [
          ./modules/options.nix
          ./modules/edge.nix
          ./modules/edge-credentials.nix
          ./modules/edge-vms.nix
          ./modules/edge-inference.nix
          ./modules/edge-gateway.nix
          ./modules/edge-hidden.nix
          ./modules/edge-exit.nix
          ./modules/logs.nix
        ];
        _module.args.self = self;
      };
      # The netboot test box CI's discovery jobs boot
      # (nix/tests/discovery/box.nix), as `losos-disk-discovery-box`.
      nixosConfigurations.discovery-box = nixpkgs.lib.nixosSystem {
        inherit system;
        modules = [ ./nix/tests/discovery/box.nix ];
      };

      # The edge gateway (handbook/docs/in-depth/edge-federation.md): the edge
      # module with LAN advertising, open enrolment and a runtime uplink, as a
      # VM image for a LAN that has no NixOS machine to import nixosModules.edge
      # on. `nix build .#losos-disk-edge-qcow2` images it
      # (nix/disk-images.nix); this configuration is what the image boots.
      nixosConfigurations.edge-gateway = nixpkgs.lib.nixosSystem {
        inherit system;
        modules = [
          self.nixosModules.edge
          ./nix/edge-gateway-vm.nix
        ];
      };

      # Run one with `nix build .#checks.x86_64-linux.<name>`. Each file says
      # at its top what it asserts. Most boot VMs, which CI (no KVM) never
      # builds.
      checks.${system} = {
        losos-install = import ./nix/tests/install.nix { inherit pkgs disko; };
        # The real installer from the branch, offline, then the installed disk
        # booted on its own (nix/tests/iso-install.nix).
        losos-iso-install = import ./nix/tests/iso-install.nix { inherit pkgs self; };
        losos-tpm-unlock = import ./nix/tests/tpm.nix { inherit pkgs disko; };
        # losos.tpm.tang: the TPM2 and a Tang server on the network, the
        # recovery code at the console without one.
        losos-network-unlock = import ./nix/tests/network-unlock.nix { inherit pkgs disko; };
        # losos-install --tang-edge: the edge is the box's Tang server, and
        # refuses it once the owner reports it stolen.
        losos-edge-unlock = import ./nix/tests/edge-unlock.nix { inherit pkgs disko; };
        losos-admin-daemon = import ./nix/tests/admin-vm.nix { inherit pkgs; };
        losos-edge-proxy = import ./nix/tests/edge-vm.nix { inherit pkgs; };
        losos-quic = import ./nix/tests/quic.nix { inherit pkgs; };
        losos-edge-lan = import ./nix/tests/edge-lan.nix { inherit pkgs; };
        losos-edge-manage = import ./nix/tests/edge-manage.nix { inherit pkgs; };
        losos-edge-statistics = import ./nix/tests/edge-statistics.nix { inherit pkgs; };
        losos-edge-federation = import ./nix/tests/edge-federation.nix { inherit pkgs; };
        # Primary, secondary and tertiary links: two official edges, two
        # gateways in one secondary cluster, a box; failover, bans, the hold.
        losos-edge-clusters = import ./nix/tests/edge-clusters.nix { inherit pkgs; };
        losos-edge-lan-two-boxes = import ./nix/demo/edge-lan/two-boxes.nix { inherit pkgs; };
        # Two boxes and an edge gateway between them, the light mode of the
        # demo nix/demo/box-edge-box/bootstrap.sh sets up.
        losos-box-edge-box = import ./nix/demo/box-edge-box/demo.nix { inherit pkgs; };
        losos-edge-market = import ./nix/tests/market-vm.nix { inherit pkgs; };
        losos-edge-dns = import ./nix/tests/edge-dns.nix { inherit pkgs; };
        losos-box-wallet-name = import ./nix/tests/box-wallet-name.nix { inherit pkgs; };
        losos-binance-onramp = import ./nix/tests/binance-onramp.nix { inherit pkgs; };
        losos-ds-render = import ./nix/tests/design-system.nix { inherit pkgs; };
        losos-front-vhost = import ./nix/tests/front-vhost.nix { inherit pkgs; };
        losos-impermanence = import ./nix/tests/impermanence.nix { inherit pkgs impermanence; };
        losos-cluster = import ./nix/tests/cluster-vm.nix { inherit pkgs; };
        losos-hardening = import ./nix/tests/hardening.nix { inherit pkgs; };
        losos-resize = import ./nix/tests/resize.nix { inherit pkgs; };
        losos-tls = import ./nix/tests/tls.nix { inherit pkgs; };
        losos-setup = import ./nix/tests/setup.nix { inherit pkgs; };
        # The local copy of the binary caches on /persist (modules/cache.nix).
        losos-nix-cache = import ./nix/tests/nix-cache.nix { inherit pkgs; };
        losos-tour = import ./nix/tests/tour.nix { inherit pkgs; };
        # The wizard's last step: signed in by the page itself, the desktop
        # behind the card, and "Let's go" (nix/tests/setup-desktop.nix).
        losos-setup-desktop = import ./nix/tests/setup-desktop.nix { inherit pkgs; };
        # The game a long wait offers, played during a real wizard wait and a
        # real rebuild, with the pull request's pictures (nix/tests/wait-game.nix).
        losos-wait-game = import ./nix/tests/wait-game.nix { inherit pkgs; };
        losos-identity = import ./nix/tests/identity.nix { inherit pkgs; };
        losos-chat = import ./nix/tests/chat.nix { inherit pkgs; };
        # LosOS Contacts: the book in LosOS Cloud, vCard in and out, and the
        # pickers in Mail and Chat (nix/tests/contacts.nix).
        losos-contacts = import ./nix/tests/contacts.nix { inherit pkgs; };
        # LosOS Calendar: WebCal subscriptions through the sandboxed fetcher,
        # .ics import, published feeds and an invitation answered in Mail
        # (nix/tests/calendar.nix).
        losos-calendar = import ./nix/tests/calendar.nix { inherit pkgs; };
        # Calls from chat: three people in real browsers through the box's
        # SFU, whose copy of every frame is ciphertext (nix/tests/calls.nix).
        losos-calls = import ./nix/tests/calls.nix { inherit pkgs; };
        losos-calls-edge = import ./nix/tests/calls.nix {
          inherit pkgs;
          throughEdge = true;
        };
        # Screenshots of a booted box; the plan is read when the driver runs.
        losos-screens = import ./nix/tests/screens.nix { inherit pkgs; };
        losos-profiles = import ./nix/tests/profiles.nix { inherit pkgs; };
        losos-profile-screens = import ./nix/tests/profile-screens.nix { inherit pkgs; };
        losos-plugins = import ./nix/tests/plugins.nix { inherit pkgs; };
        losos-channels = import ./nix/tests/channels.nix { inherit pkgs; };
        # Problem reports: the box's list and a report sent to a stand-in GitHub.
        losos-problems = import ./nix/tests/problems.nix { inherit pkgs; };
        losos-hidden-services = import ./nix/tests/hidden-services.nix { inherit pkgs; };
        # The box ends its own TLS and the edge passes it through unread
        # (modules/public-tls.nix, edge/registrar/src/passthrough.rs).
        losos-edge-passthrough = import ./nix/tests/edge-passthrough.nix { inherit pkgs; };
        losos-design = import ./nix/tests/design.nix { inherit pkgs; };
        losos-agents = import ./nix/tests/agents.nix { inherit pkgs; };
        # The banner a box off the mesh shows over every page, both states
        # and the dismissal, with the pull request's pictures (nix/tests/mesh-notice.nix).
        losos-mesh-notice = import ./nix/tests/mesh-notice.nix { inherit pkgs; };
        # The phone's bottom bar and its sheet on a real box, with the pull
        # request's pictures (nix/tests/bottom-nav.nix).
        losos-bottom-nav = import ./nix/tests/bottom-nav.nix { inherit pkgs; };
        # The admin pages with JavaScript off, from sign-in to a rebuild, with
        # the pull request's pictures (nix/tests/no-js.nix).
        losos-no-js = import ./nix/tests/no-js.nix { inherit pkgs; };
        # The pages as MDX: an administrator's page and stylesheet saved as
        # commits, drawn with JavaScript on and off, and the editor beside
        # the page, with the pull request's pictures (nix/tests/pages.nix).
        losos-pages = import ./nix/tests/pages.nix { inherit pkgs; };
        # Settings › Logs: the journal, its redaction and the page that works
        # while lososd is down (nix/tests/logs.nix).
        losos-logs = import ./nix/tests/logs.nix { inherit pkgs; };
        # The Mesh page while the box has paused sharing for want of an edge,
        # with the pull request's pictures (nix/tests/sharing-pause.nix).
        losos-sharing-pause = import ./nix/tests/sharing-pause.nix { inherit pkgs; };
        # kindashared: sharing with the local gateways only, refused through
        # an edge reached over the internet, paused and resumed with a real
        # gateway, with the pull request's pictures (nix/tests/kindashared.nix).
        losos-kindashared = import ./nix/tests/kindashared.nix { inherit pkgs; };
        # LosOS Diagrams on a box with LosOS Cloud: the /lab/ move, a .canvas
        # file opened and saved back, and the pull request's pictures.
        losos-diagrams = import ./nix/tests/diagrams.nix { inherit pkgs; };
        # Two people on two boxes drawing in one shared diagram at once
        # (nix/tests/diagrams-together.nix).
        losos-diagrams-together = import ./nix/tests/diagrams-together.nix { inherit pkgs; };
        losos-app-sign-in = import ./nix/tests/app-sign-in.nix { inherit pkgs; };
        # Your apps: Chromium on a laptop asks the box for an app, watches it
        # written, uses it and saves it, and keeps every screen
        # (nix/tests/app-builder.nix).
        losos-app-builder = import ./nix/tests/app-builder.nix { inherit pkgs; };
        losos-mail = import ./nix/tests/mail.nix { inherit pkgs; };
        losos-mail-screens = import ./nix/tests/mail.nix {
          inherit pkgs;
          screens = true;
        };
        # OpenPGP in Mail: keys made in the browser, encrypted mail between
        # two people, the box's WKD, keys from an outside WKD, a keyserver and
        # Autocrypt (nix/tests/mail-pgp.nix).
        losos-mail-pgp = import ./nix/tests/mail-pgp.nix { inherit pkgs; };
        # Roles and moderation: reports, held mail, mutes and suspensions
        # across LosOS Login, lososd and Stalwart (nix/tests/moderation.nix).
        losos-moderation = import ./nix/tests/moderation.nix { inherit pkgs; };
        losos-moderation-screens = import ./nix/tests/moderation.nix {
          inherit pkgs;
          screens = true;
        };
        losos-cloud = import ./nix/tests/cloud.nix { inherit pkgs; };
        # How fast LosOS Cloud and LosOS Git take files in on a two-core,
        # 4 GiB box, as numbers to compare trees by (nix/tests/upload-speed.nix).
        losos-upload-speed = import ./nix/tests/upload-speed.nix { inherit pkgs; };
        # The right-click menu on the admin page and in LosOS Cloud, with
        # the pull request's pictures (nix/tests/context-menu.nix).
        losos-context-menu = import ./nix/tests/context-menu.nix { inherit pkgs; };
        losos-cloud-from-nextcloud = import ./nix/tests/cloud-from-nextcloud.nix { inherit pkgs; };
        # Photos: the camera's details and faces read on the box, grouped
        # into people the owner names (nix/tests/cloud-photos.nix).
        losos-cloud-photos = import ./nix/tests/cloud-photos.nix { inherit pkgs; };
        # Notes on a kitchen garden vault, and the folder published as a
        # knowledge base, on and off the mesh (nix/tests/notes.nix).
        losos-notes = import ./nix/tests/notes.nix { inherit pkgs; };
        # One file through Mail, LosOS Cloud, Notes and the box's AI on one
        # box, with the pull request's pictures (nix/tests/apps-together.nix).
        losos-apps-together = import ./nix/tests/apps-together.nix { inherit pkgs; };
        # Gemma answers the apps, the coder builds, one model in memory at
        # a time on a 4 GiB box (nix/tests/assistant-models.nix).
        losos-assistant-models = import ./nix/tests/assistant-models.nix { inherit pkgs; };
        # Two boxes sharing a note and editing it together, apart and
        # together again (nix/tests/mesh-collab.nix).
        losos-mesh-collab = import ./nix/tests/mesh-collab.nix { inherit pkgs; };
        losos-windows = import ./nix/tests/windows.nix { inherit pkgs; };
        # Apps run as notshared on rootless Podman, with no cluster: the
        # box's own, one in the LosOS app format and a chart from the
        # search (nix/tests/podman-apps.nix).
        losos-podman-apps = import ./nix/tests/podman-apps.nix { inherit pkgs; };
        losos-admin-ui = import ./nix/tests/admin-ui.nix {
          inherit pkgs;
          # The real option document, so the Advanced pane is drawn from
          # every option the install configuration declares and the check
          # fails on any row it cannot draw (tests/advanced.browser.mjs).
          optionsJson = self.checks.${system}.losos-options-doc;
          # The check is the shipped package with its browser checks added.
          ui = self.packages.${system}.losos-admin-ui;
          callsPackage = self.packages.${system}.losos-ctl;
        };
        # Not a VM: the option document of the install configuration
        # (nix/options-doc.nix via modules/config-repo.nix), forced here so
        # `nix flake check --no-build` fails the moment an option is declared
        # with a type the Advanced pane has no editor for.
        losos-options-doc =
          self.nixosConfigurations.install.config.environment.etc."losos/options.json".source;
        losos-credentials = import ./nix/tests/credentials.nix { inherit pkgs; };
        losos-erase = import ./nix/tests/erase.nix { inherit pkgs impermanence; };
        # Backups on storage bought on the mesh: through the edge's backup
        # door to a rest-server per order, ciphertext only on the volume and
        # on the edge (nix/tests/mesh-backup.nix).
        losos-mesh-backup = import ./nix/tests/mesh-backup.nix { inherit pkgs impermanence; };
        losos-console = import ./nix/tests/console.nix { inherit pkgs; };
        # The text search behind the admin palette: the cloud index pass in
        # its sandbox, PDFs, a changed file, the Git search and who may ask
        # (nix/tests/search.nix).
        losos-search = import ./nix/tests/search.nix { inherit pkgs; };
        # Collabora Online behind the front vhost over the folder host: open,
        # type, save, save as PDF, two people in one document, the token
        # rules and what nginx refuses (nix/tests/office.nix).
        losos-office = import ./nix/tests/office.nix { inherit pkgs; };
        # Office over LosOS Cloud's own files: discovery saved for a cloud
        # with no network, coolwsd through nginx's loopback bridge to the
        # cloud's socket, a save landing in the person's cloud
        # (nix/tests/office-cloud.nix).
        losos-office-cloud = import ./nix/tests/office-cloud.nix { inherit pkgs; };
        # Markdown documents and presentations in LosOS Office, with no
        # Collabora on the box (nix/tests/office-markdown.nix).
        losos-office-markdown = import ./nix/tests/office-markdown.nix { inherit pkgs; };
        # LosOS Office's own documents and presentations, .paper and .slides
        # edited as they look, and a document full of script that runs
        # nowhere (nix/tests/office-paper.nix).
        losos-office-paper = import ./nix/tests/office-paper.nix { inherit pkgs; };
        losos-quick-actions = import ./nix/tests/quick-actions.nix { inherit pkgs; };
        # Not a VM: losos-mdx's browser package in Node, against the crate's
        # own expectations and its limits inside a wasm stack
        # (nix/tests/mdx-wasm.nix).
        losos-mdx-wasm = import ./nix/tests/mdx-wasm.nix {
          inherit pkgs;
          mdxWasm = self.packages.${system}.losos-mdx-wasm;
        };
        # The box half of models split across boxes: the RPC server as
        # `shared`, its memory cap, the lend file, the loopback owner rule
        # and the `:llm` tunnel service (nix/tests/inference-lend.nix).
        losos-inference-lend = import ./nix/tests/inference-lend.nix { inherit pkgs; };
        # The whole pay-as-you-go story on a local chain: two lending boxes,
        # a USDC escrow, a quote, a paid chat, the settle split, and the
        # refusals (nix/tests/inference-payg.nix).
        losos-inference-payg = import ./nix/tests/inference-payg.nix { inherit pkgs; };
        # Two of three keys move an escrowed payment: an edge, the buyer's
        # box and the seller's box on a local chain (nix/tests/market-escrow.nix).
        losos-market-escrow = import ./nix/tests/market-escrow.nix { inherit pkgs; };
        losos-installer-screen = import ./nix/tests/installer-screen.nix { inherit pkgs disko; };
        losos-installer-report = import ./nix/tests/installer-report.nix { inherit pkgs disko; };
        # LosOS Git: Forgejo's repositories moved in, the configuration
        # repository's round trip through lososd, clone and push through
        # nginx, and who it believes (nix/tests/git.nix).
        losos-git = import ./nix/tests/git.nix { inherit pkgs; };
        # Its pages in Chromium: code, a pull request reviewed and merged, the
        # worktrees (nix/tests/git-pages.nix).
        losos-git-pages = import ./nix/tests/git-pages.nix { inherit pkgs; };
        # LosOS Git's issue tracker: Forgejo's issues and LosOS Cloud's
        # boards carried over, closing words in commits and pull requests,
        # and the pages from Chromium (nix/tests/git-issues.nix).
        losos-git-issues = import ./nix/tests/git-issues.nix { inherit pkgs; };
        # LosOS Wiki: spaces kept as LosOS Git wikis, written with git and in
        # the browser, the directory with the knowledge bases, and each
        # space's published site (nix/tests/wiki.nix).
        losos-wiki = import ./nix/tests/wiki.nix { inherit pkgs; };
        # People of another box in LosOS Git by their chat address, and a
        # repository's chat room between two boxes (nix/tests/git-federation.nix).
        losos-git-federation = import ./nix/tests/git-federation.nix { inherit pkgs; };
        # A Forgejo box's switch: everything it held, carried into LosOS Git.
        losos-git-forgejo = import ./nix/tests/git-forgejo.nix { inherit pkgs; };
        # Mesh CI: a push to LosOS Git runs its GitHub Actions workflows on
        # two boxes through pm and Mesh CI's pm plugin, a service container
        # and a pm recipe included, statuses back on the commit.
        losos-mesh-ci = import ./nix/tests/mesh-ci.nix {
          inherit pkgs;
          pmSrc = pm;
        };
        # The same with the real admin UI on boxa and a laptop that takes
        # the pull request's pictures of the Mesh CI pages.
        losos-mesh-ci-screens = import ./nix/tests/mesh-ci.nix {
          inherit pkgs;
          pmSrc = pm;
          screens = true;
        };
        # LosOS Reviewer: the code model's reviews of pull requests, on
        # recorded answers (nix/tests/git-reviewer.nix).
        losos-git-reviewer = import ./nix/tests/git-reviewer.nix { inherit pkgs; };
        # The signed installer medium under OVMF's Secure Boot build: the
        # real ISO (plus the test backdoor) signed with a throwaway key the
        # sandbox makes, boots with that key enrolled beside Microsoft's and
        # proves it from inside, and Ubuntu's Microsoft-signed shim starts on
        # the same firmware; the same medium is refused by a firmware holding only
        # Microsoft's keys, a tampered copy is refused, and so is the
        # unsigned build (nix/tests/secure-boot.nix). Builds the ISO, so it is
        # the slowest check here.
        losos-secure-boot = import ./nix/tests/secure-boot.nix {
          inherit pkgs;
          iso = self.nixosConfigurations.iso;
          inherit (self.packages.${system}) losos-sign-iso;
        };
        # Not a VM: evaluates the install configuration and asserts on the
        # merged option values, so `nix flake check --no-build` turns red the
        # moment a load-bearing default drifts (nix/tests/invariants.nix).
        losos-invariants = import ./nix/tests/invariants.nix {
          inherit pkgs;
          inherit (nixpkgs) lib;
          inherit (self.nixosConfigurations.install) config;
          mesh =
            (self.nixosConfigurations.install.extendModules {
              modules = [
                {
                  losos.proxy.enable = nixpkgs.lib.mkForce true;
                  losos.hostName = nixpkgs.lib.mkForce "invariants";
                  losos.cluster.enable = nixpkgs.lib.mkForce true;
                }
              ];
            }).config;
        };
      };
    };
}
