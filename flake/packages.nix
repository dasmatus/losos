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
#   losos-admin-ui — the admin SPA: React 19 + Vite + Tailwind v4, built from
#                admin-ui/app/ with buildNpmPackage. $out IS the document root
#                (index.html, hashed assets/, theme-boot.js), served by the
#                front Nginx vhost with an SPA fallback — one app on real
#                paths (/, /storage, /mesh, /apps, /settings/<pane>), not the
#                dashboard/ + settings/ page pair it replaced — with lososd's
#                loopback API proxied at /api/*. See admin-ui/app/ and
#                modules/containers.nix.
#
#                Its npm dependency fetch is the one derivation here that
#                needs the network on a machine that cannot substitute it. That
#                costs a red CI job at worst, never a failed install: the
#                output has no store references, modules/cache.nix points every
#                box at losos.cachix.org, and flake.nix puts this path in the
#                ISO's storeContents.
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
  losos-admin-ui = pkgs.buildNpmPackage {
    pname = "losos-admin-ui";
    version = "0.1.0";

    # admin-ui/app, not admin-ui/. The source ROOT is the exclusion: it is what
    # keeps admin-ui/design-system/react out of the closure, and unlike the
    # `baseNameOf name != "design-system"` filter this replaces, a root cannot
    # quietly stop matching when somebody renames a directory.
    src = ./../admin-ui/app;

    # SHA256 of the npm dependency cache built from
    # admin-ui/app/package-lock.json — same convention as cargoHash above. If
    # the lock file changes, `nix build .#losos-admin-ui` prints the real hash
    # to paste here (or `nix run nixpkgs#prefetch-npm-deps -- \
    # admin-ui/app/package-lock.json` gets it without a failed build first).
    #
    # tests/admin-ui.nix carries the SAME hash over the SAME lock file: two
    # derivations, one dependency set. Change both together.
    npmDepsHash = "sha256-xXHkWQdrs6B4OwWasb3vwTYHALRS8P94cnvihWlHbE4=";

    # No postinstall scripts. playwright is a devDependency (tests/admin-ui.nix
    # drives the browser check with it) and its postinstall downloads browsers:
    # there is no network in the sandbox, and the browsers it would fetch are
    # not the ones anything here runs against. esbuild, rollup and
    # @tailwindcss/oxide get their native binaries from the linux-x64-gnu
    # packages pinned in package-lock.json rather than from a download, so
    # nothing else needs a script to run.
    npmFlags = [ "--ignore-scripts" ];
    env.PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD = "1";

    # The default npmBuildScript is `npm run build`, which here is
    # `tsc --noEmit && vite build` — so unlike the Rust crates (doCheck = off,
    # see below) the typecheck IS part of building the shipping artefact. It
    # costs seconds over ~80 files, not the minutes doCheck costs rustc.
    #
    # $out is the document root itself rather than a packed tarball: index.html
    # at the top, content-hashed bundles under assets/, and the deliberately
    # unhashed theme-boot.js beside them (admin-ui/app/vite.config.ts explains
    # why that one file must keep a stable name). modules/containers.nix serves
    # $out directly, with an SPA fallback because the app routes on real paths.
    installPhase = ''
      runHook preInstall
      cp -r dist $out
      runHook postInstall
    '';
  };

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
