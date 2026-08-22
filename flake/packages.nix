# Per-output package definitions.
#   losos-ctl  — the control plane (Rust crate under backend/), built via
#                rustPlatform.buildRustPackage with doCheck so `cargo test`
#                runs on build. Provides both the `lososd` system daemon and
#                the `losos-ctl` facade CLI (which also carries the
#                `install` subcommand the installer ISO runs);
#                modules/daemon.nix runs lososd system-wide and installs the
#                facade for root (see losos.backend.package), and
#                modules/installer.nix draws the installer wrapper from the
#                same derivation (losos.installer.package).
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
    cp -rT ${
      # admin-ui/ minus design-system/: the stylesheets ride in below by
      # explicit path, and react/ must never enter the closure (nor be
      # served by nginx).
      lib.cleanSourceWith {
        src = ./../admin-ui;
        filter = name: type:
          lib.cleanSourceFilter name type
          && baseNameOf name != "design-system";
      }
    } $out
    chmod -R u+rwX $out
    mkdir -p $out/ds
    cp ${./../admin-ui/design-system/tokens.css} $out/ds/tokens.css
    cp ${./../admin-ui/design-system/losos.css} $out/ds/losos.css
  '';

  losos-ctl = pkgs.rustPlatform.buildRustPackage {
    pname = "losos-ctl";
    version = "0.1.0";
    src = lib.cleanSource ./../backend;
    # SHA256 of the vendored crate tarball. If deps change, `nix build
    # .#losos-ctl-rs` will print the new hash to paste here.
    cargoHash = "sha256-vwDnOequ7M/lBVfkrgT2uLMC8shQxnbPQsssEeigeE8=";
    # No system deps: zbus speaks the D-Bus wire protocol natively, so there is
    # no libdbus to link against.
    doCheck = true;
  };

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