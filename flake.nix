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
      # The losos Nextcloud app packaged as a store path (see flake/packages.nix).
      # NixOS' nextcloud module symlinks this into extraApps.
      packages.${system} = import ./flake/packages.nix { inherit pkgs; };

      # Dev shell for the PHP app + Haskell backend: fish (host config minus
      # Zellij) + full Haskell stdlib + PHP + Nextcloud occ. See flake/devshell.nix.
      devShells.${system} = import ./flake/devshell.nix { inherit pkgs; };

      # The modules shared by every losos host: option declarations, users,
      # impermanence, the disko disk layout, and (for the installed system)
      # the boot/crypto/services/update modules.
      nixosConfigurations = {
        # Installer medium: a minimal NixOS live ISO that carries the disko
        # layout so it can format the target disk and enroll encryption keys.
        iso = nixpkgs.lib.nixosSystem {
          inherit system;
          modules = [
            impermanence.nixosModules.impermanence
            disko.nixosModules.disko
            (
              { modulesPath, pkgs, ... }:
              {
                imports = [ (modulesPath + "/installer/cd-dvd/installation-cd-minimal.nix") ];
                environment.systemPackages = [
                  pkgs.disko
                  pkgs.cryptsetup
                ];
              }
            )
            ./modules/options.nix
            ./modules/disko.nix
          ];
        };
        # The target system installed onto the machine.
        install = nixpkgs.lib.nixosSystem {
          inherit system;
          # Make the flake `self` available to modules (services.nix references
          # self.packages.${system}.losos-app to install our plugin app).
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
            ./modules/containers.nix
            ./modules/overrides.nix
            ./modules/updates.nix
            ./modules/defaults.nix
          ];
        };
      };
    };
}
