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

      # The installer module needs the flake's own source (`self.outPath`) to
      # bake it into the ISO; pass `self` through to the iso system. The
      # install system gets `self` so defaults.nix can reach
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
                # Bake the flake source (git-tracked tree self.outPath resolves
                # to) into the ISO so losos-install can copy it to a writable
                # work dir, drop in modules/install-target.nix, and run disko +
                # nixos-install against it. self is in scope via specialArgs.
                environment.etc."losos/flake-source".source = self.outPath;
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
            # Host-specific drive list + TPM mode, written by losos-install at
            # install time. Only imported when it exists so the published
            # flake (without it) still evaluates against the default drive.
          ] ++ nixpkgs.lib.optional (builtins.pathExists ./modules/install-target.nix) ./modules/install-target.nix;
        };
      };

      # nixos-test-vms. Run with `nix build .#checks.x86_64-linux.<name>`:
      #   losos-install       — boots a VM with three empty disks and runs the
      #                         installer end-to-end through the disko format/mount
      #                         path (tests/install.nix).
      #   losos-admin-daemon  — boots a minimal losos appliance and exercises the
      #                         lososd daemon + losos-ctl facade + the Bearer-
      #                         authed admin HTTP API (tests/admin-vm.nix).
      checks.${system} = {
        losos-install = import ./tests/install.nix { inherit pkgs disko; };
        losos-admin-daemon = import ./tests/admin-vm.nix { inherit pkgs; };
      };
    };
}
