# Per-output package definitions.
#   losos-ctl  — the Haskell backend (cabal project under backend/), built via
#                callCabal2nix with doCheck so the HUnit spec runs on build.
#                Provides both the `lososd` system daemon and the `losos-ctl`
#                facade CLI; modules/daemon.nix runs lososd system-wide and
#                installs the facade for root (see losos.backend.package).
#   losos-admin-ui — the standalone admin UI (no build step, no frameworks):
#                a dashboard/ + settings/ pair of self-contained plain-JS pages,
#                served by the front Nginx vhost (dashboard at /, settings at
#                /settings/) with lososd's loopback API proxied at /api/*. See
#                admin-ui/ and modules/containers.nix.
{ pkgs, ... }:

let
  lib = pkgs.lib;
in
{
  losos-admin-ui = pkgs.runCommand "losos-admin-ui-0.1.0" { } ''
    cp -rT ${lib.cleanSource ./../admin-ui} $out
    chmod -R u+rwX $out
  '';

  losos-ctl = pkgs.haskellPackages.callPackage ./../backend/losos-ctl.nix { };
}