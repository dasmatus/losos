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
#   losos-registrar — the Rust edge registration + Traefik/rathole config
#                reconciler (`serve`) and appliance registration client
#                (`announce`). Built via rustPlatform.buildRustPackage from
#                backend-registrar/. See modules/edge-registrar.nix (edge) and
#                modules/proxy.nix (appliance). cargoHash is the SHA256 of the
#                vendored crate tarball; `lib.fakeHash` is a placeholder — the
#                first `nix build .#losos-registrar` will print the real hash
#                to paste here.
{ pkgs, ... }:

let
  lib = pkgs.lib;
in
{
  losos-admin-ui = pkgs.runCommand "losos-admin-ui-0.1.0" { } ''
    cp -rT ${lib.cleanSource ./../admin-ui} $out
    chmod -R u+rwX $out
    # Design-system stylesheets, copied by explicit path — never
    # cleanSource of design-system/ (react/ must not enter the closure).
    mkdir -p $out/ds
    cp ${./../design-system/tokens.css} $out/ds/tokens.css
    cp ${./../design-system/losos.css} $out/ds/losos.css
  '';

  losos-ctl = pkgs.haskellPackages.callPackage ./../backend/losos-ctl.nix { };

  losos-registrar = pkgs.rustPlatform.buildRustPackage {
    pname = "losos-registrar";
    version = "0.1.0";
    src = lib.cleanSource ./../backend-registrar;
    # SHA256 of the vendored crate tarball. If deps change, `nix build
    # .#losos-registrar` will print the new hash to paste here.
    cargoHash = "sha256-eFIod0WRyTE+QespzQqQpiMaT0cX9yM8h0XtMNr7jSs=";
    # No system deps; pure Rust with rustls (no openssl).
    doCheck = true;
  };
}