# Per-output package definitions.
#   losos-ctl  — the control plane (Rust crate under backend/), built via
#                rustPlatform.buildRustPackage. doCheck is off; the suites
#                run via cargo test (see the note on the derivation below).
#                Provides both the `lososd` system daemon and
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
#                (`announce`, `join`). Built via rustPlatform.buildRustPackage
#                from backend-registrar/. See modules/edge.nix (edge) and
#                modules/proxy.nix (appliance). cargoHash is the SHA256 of the
#                vendored crate tarball; the first `nix build
#                .#losos-registrar` after a dependency change prints the real
#                hash to paste here.
#   losos-image-{pause,nextcloud,forgejo} — the OCI images the local k3s
#                cluster runs as static pods, defined in flake/images.nix and
#                merged in below. modules/defaults.nix wires them to
#                losos.workloads.*. They are exported as flake packages
#                because that is how they reach a binary cache: the appliance
#                substitutes them, it never builds them (see the header of
#                flake/images.nix).
{ pkgs, ... }:

let
  inherit (pkgs) lib;

  # Only derivations may come out of this file — every attribute here becomes a
  # flake package, and a non-derivation breaks `nix flake check` and
  # `nix flake show` for everything else too.
  images = import ./images.nix { inherit pkgs; };
in
images
// {
  losos-admin-ui = pkgs.runCommand "losos-admin-ui-0.1.0" { } ''
    cp -rT ${
      # admin-ui/ minus design-system/: the stylesheets ride in below by
      # explicit path, and react/ must never enter the closure (nor be
      # served by nginx).
      lib.cleanSourceWith {
        src = ./../admin-ui;
        filter = name: type: lib.cleanSourceFilter name type && baseNameOf name != "design-system";
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
    # doCheck is off, and the tests still run — in CI's lint job and in
    # `devenv test`, both of which invoke `cargo test` directly.
    #
    # The reason is Codeberg's 10-minute cap. doCheck does not merely execute
    # the suite (that takes ~0.2 s); it compiles the crate a second time in
    # test configuration and links a separate binary per test target. That
    # pushed `nix build .#losos-ctl` to between 7.7 and 10.3 minutes across
    # observed runs — a coin flip against the cap, which cancelled real runs
    # on unmodified main before any of this was touched.
    #
    # The trade is explicit: a bare `nix build` no longer runs the suite, so
    # the gate lives in the lint job (which has ~5 minutes of headroom and a
    # toolchain already loaded) rather than inside the package build.
    doCheck = false;
  };

  losos-registrar = pkgs.rustPlatform.buildRustPackage {
    pname = "losos-registrar";
    version = "0.1.0";
    src = lib.cleanSource ./../backend-registrar;
    # SHA256 of the vendored crate tarball. If deps change, `nix build
    # .#losos-registrar` will print the new hash to paste here.
    cargoHash = "sha256-uPQ/ftWa+GMR5EIMg3oXotjTi+dFBPubIR9qyFBsjA4=";
    # No system deps; pure Rust with rustls (no openssl).
    # Same reasoning as losos-ctl above: the suite runs via cargo test in the
    # lint job, not inside this derivation.
    doCheck = false;
  };
}
