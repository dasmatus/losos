{
  description = "losos — stateless NixOS: Nextcloud (notshared), Tahoe-LAFS (shared), auto-upgrade, midnight reboot, TPM-or-keyfile FDE";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    impermanence.url = "github:nix-community/impermanence";
    disko.url = "github:nix-community/disko";
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
      # Flake packages: losos-ctl (Haskell daemon + facade) and losos-admin-ui
      # (static admin SPA). See flake/packages.nix.
      packages.${system} = import ./flake/packages.nix { inherit pkgs; };

      # Dev shell for the Haskell backend: fish (host config minus Zellij) +
      # full Haskell stdlib. See flake/devshell.nix.
      devShells.${system} = import ./flake/devshell.nix { inherit pkgs; };

      # Both systems get `self` via specialArgs: the iso system reaches
      # self.packages.${system}.losos-ctl to wire the installer binary, the
      # install system so defaults.nix can reach
      # self.packages.${system}.{losos-ctl,losos-admin-ui}.
      nixosConfigurations = {
        # Installer medium: a minimal NixOS live ISO that carries the disko
        # layout and the losos auto-installer (losos-install). Boot it on the
        # target machine and run `losos-install` — it finds every fixed disk,
        # merges them into one LVM volume group via disko, and installs.
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
                # both built from the same cabal project). installer.nix packages
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
