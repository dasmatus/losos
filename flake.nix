{
  description = "losos — stateless NixOS: Nextcloud (notshared), Tahoe-LAFS (shared), auto-upgrade, midnight reboot, TPM-or-keyfile FDE";

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
    in
    {
      # Flake packages: losos-ctl (the Rust lososd daemon + losos-ctl facade),
      # losos-registrar (the master-proxy edge) and losos-admin-ui (the static
      # admin SPA). See flake/packages.nix.
      packages.${system} = import ./flake/packages.nix { inherit pkgs; };

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
      devShells.${system}.default = pkgs.mkShell {
        nativeBuildInputs = [
          pkgs.cargo
          pkgs.rustc
          pkgs.clippy
          pkgs.rustfmt
          pkgs.rust-analyzer
          pkgs.nodejs
        ];
        shellHook = ''
          echo "Toolchain-only shell. For the scripts and pre-commit hooks:"
          echo "  devenv shell      (or: direnv allow)"
        '';
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
              { modulesPath, pkgs, self, ... }:
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
                # Booting the ISO auto-runs losos-install as root's login shell:
                # insert the medium, boot, and the box reinstalls unattended —
                # the destructive factory-reset / reinstall path.
                losos.installer.autorun = true;
                # The default (zstd level 19, cache sized off host RAM) gets
                # OOM-killed on codeberg-medium CI runners (exit 137): the
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
                # LOSOS_FLAKE_URL (default: the public codeberg repo) into a
                # writable work dir at run time, drops in
                # modules/install-target.nix, and runs disko + nixos-install
                # against the clone. The ISO needs network for that clone —
                # and would anyway, since flake evaluation fetches nixpkgs.
              }
            )
            ./modules/options.nix
            ./modules/disko.nix
            ./modules/installer.nix
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
            ./modules/impermanence.nix
            ./modules/disko.nix
            ./modules/boot.nix
            ./modules/services.nix
            ./modules/nextcloud-common.nix
            ./modules/containers.nix
            ./modules/daemon.nix
            ./modules/overrides.nix
            ./modules/updates.nix
            ./modules/defaults.nix
            ./modules/proxy.nix
            # Host-specific drive list + TPM mode, written by losos-install at
            # install time. Only imported when it exists so the published
            # flake (without it) still evaluates against the default drive.
          ] ++ nixpkgs.lib.optional (builtins.pathExists ./modules/install-target.nix) ./modules/install-target.nix;
        };

      };

      # The master-proxy edge: Traefik (master), a rathole server, and
      # losos-registrar (serve) on a public VPS. Exported as a module, not a
      # nixosConfiguration: the VPS flake imports this next to its own
      # hardware/bootloader config and sets losos.edge.* (a bare
      # nixosConfiguration would fail the root-fs/bootloader assertions and
      # could not be configured from outside anyway). `self` is baked in so
      # losos.edge.registrar.package resolves to this flake's registrar.
      nixosModules.edge = {
        imports = [
          ./modules/options.nix
          ./modules/edge.nix
        ];
        _module.args.self = self;
      };

      # nixos-test-vms. Run with `nix build .#checks.x86_64-linux.<name>`:
      #   losos-install       — boots a VM with three empty disks and runs the
      #                         installer end-to-end through the disko format/mount
      #                         path (tests/install.nix).
      #   losos-admin-daemon  — boots a minimal losos appliance and exercises the
      #                         lososd daemon + losos-ctl facade + the Bearer-
      #                         authed admin HTTP API (tests/admin-vm.nix).
      #   losos-edge-proxy    — boots an edge VM (Traefik + rathole server +
      #                         registrar) + an appliance VM (rathole client +
      #                         announce + Nginx) and asserts the master-proxy
      #                         register/reconcile/forward path (tests/edge-vm.nix).
      #   losos-ds-render     — builds the losos-ds npm package and renders every
      #                         design-system component in headless chromium
      #                         (playwright-driver.browsers), asserting computed
      #                         styles from tokens.css (tests/design-system.nix).
      checks.${system} = {
        losos-install = import ./tests/install.nix { inherit pkgs disko; };
        losos-admin-daemon = import ./tests/admin-vm.nix { inherit pkgs; };
        losos-edge-proxy = import ./tests/edge-vm.nix { inherit pkgs; };
        losos-ds-render = import ./tests/design-system.nix { inherit pkgs; };
      };
    };
}
