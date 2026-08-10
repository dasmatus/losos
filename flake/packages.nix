# Per-output package definitions.
#   losos-app  — the Nextcloud app (PHP) packaged as a store path NixOS'
#                nextcloud module symlinks into extraApps (read-only store).
#   losos-ctl  — the Haskell backend (cabal project under backend/), built via
#                callCabal2nix with doCheck so the HUnit spec runs on build.
#                NixOS installs it system-wide and grants the nextcloud user
#                sudo for it (see services.nix + losos.backend.package).
{ pkgs, ... }:

let
  lib = pkgs.lib;
  appRoot = ./../nextcloud-app;
  # cleanSource only strips .git; also drop dev-only/vendor cruft so the
  # deployed app store path isn't bloated with composer's vendor/, node_modules,
  # build caches, etc. The app has no runtime composer deps (only require-dev),
  # so vendor/ is purely a dev artifact and Nextcloud resolves its own deps.
  appSrc = lib.cleanSourceWith {
    src = appRoot;
    filter = path: _type:
      let
        under = sub: lib.hasPrefix "${toString appRoot}/${sub}" (toString path);
        base = baseNameOf path;
      in
      !(under "vendor"
        || under "node_modules"
        || under ".php-cs-fixer.cache"
        || under "tests"
        || lib.elem base [ "composer.lock" "phpunit.xml.dist" ".php-cs-fixer.php" ]);
  };
in
{
  # A Nextcloud app is just a directory with `appinfo/` at its root. We copy
  # the filtered source tree into a store path and make sure the bits are
  # readable. The NixOS nextcloud module's `extraApps` symlinks this under the
  # webroot's `nix-apps/losos`.
  losos-app = pkgs.runCommand "losos-app-0.1.0"
    {
      # Keep the source derivation name predictable for `nix path-info`.
      passthru.appId = "losos";
    }
    ''
      cp -rT ${appSrc} $out
      chmod -R u+rwX $out
    '';

  # The Haskell backend. callCabal2nix reads backend/losos-ctl.cabal and builds
  # the library + executable; haskellPackages' default doCheck=true runs the
  # tasty/hunit spec in backend/test during the build.
  losos-ctl = pkgs.haskellPackages.callCabal2nix "losos-ctl" ./../backend { };
}