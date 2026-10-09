# Per-output package definitions.
#   losos-ctl  — the control plane (Rust crate under backend/), built via
#                flake/fast-build.nix's buildRustPackage (mold as the
#                linker; ccache and sccache where the host exposes their
#                caches). doCheck is off; the suites
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
#                box through its configured proxy, and flake.nix puts this path in the
#                ISO's storeContents.
#   losos-registrar — the Rust edge registration + Traefik/rathole config
#                reconciler (`serve`) and appliance registration client
#                (`announce`, `join`). Built via the same buildRustPackage
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
#   losos-lab-core — LosOS Lab's core in Rust (admin-ui/lab/core/), built
#                for wasm32 and run through wasm-bindgen. The Lab itself is
#                the admin UI's second page (admin-ui/app/lab/), served at
#                /lab/, and imports this package.
#   losos-lab-render is the Lab's canvas, built from admin-ui/lab/render/.
#                Bevy draws the two views as a 3D scene in one wasm module
#                with the core, through wasm-bindgen and then wasm-opt. It
#                is built twice, as webgpu/ and webgl2/, and the page picks.
#   losos-lab-virt — the Lab's libvirt client (admin-ui/lab/virt-rpc/):
#                libvirt's remote protocol for wasm32, through wasm-bindgen,
#                for a page that drives libvirt over `losos-registrar lab`'s
#                byte relay with no virsh on the host.
#   losos-sign-iso — signs the installer ISO's UEFI loader in place with the
#                Secure Boot db key, after `nix build`, so the key never
#                enters a store (modules/secure-boot.nix, flake/sign-iso.sh).
#                CI runs it with the SECURE_BOOT_DB_KEY secret;
#                tests/secure-boot.nix with a throwaway key.
{ pkgs, ... }:

let
  inherit (pkgs) lib;

  # Only derivations may come out of this file — every attribute here becomes a
  # flake package, and a non-derivation breaks `nix flake check` and
  # `nix flake show` for everything else too.
  images = import ./images.nix { inherit pkgs; };

  # mold as the linker, ccache in front of gcc, sccache in front of rustc —
  # for both crates, and for the dev shells through the same file, so that
  # `cargo` in a shell and `nix build` agree on how a binary is linked. The
  # caches are gated on the host exposing their directories to the sandbox
  # and are otherwise never invoked; mold is not gated on anything. The
  # header of fast-build.nix says what each one is worth here, and how to
  # turn the caches on for `nix build` on a dev machine.
  inherit (import ./fast-build.nix { inherit pkgs; }) buildRustPackage;

  # LosOS Lab's core (admin-ui/lab/core/README.md) as the wasm-bindgen web
  # package the Lab page imports: losos_lab_core.js, its .d.ts and the .wasm.
  # Plain rustPlatform rather than fast-build.nix's: mold and ccache are
  # composed for the native toolchain, and rustc links wasm32 with its own
  # rust-lld. wasm-bindgen-cli must be the version the crate pins (=0.2.127);
  # the CLI refuses a module built against another one.
  losos-lab-core = pkgs.rustPlatform.buildRustPackage {
    pname = "losos-lab-core";
    version = "0.1.0";
    src = lib.cleanSource ./../admin-ui/lab/core;
    cargoHash = "sha256-TXH+ZimT0PygfRRnjOCNpFgi3Acmb4kxN0S7vcL9Pxk=";
    nativeBuildInputs = [
      pkgs.wasm-bindgen-cli
      pkgs.lld
    ];
    buildPhase = ''
      runHook preBuild
      cargo build --release --offline --target wasm32-unknown-unknown
      runHook postBuild
    '';
    doCheck = false;
    installPhase = ''
      runHook preInstall
      wasm-bindgen --target web --out-dir $out \
        target/wasm32-unknown-unknown/release/losos_lab_core.wasm
      runHook postInstall
    '';
  };

  # The Lab page imports the core from src/lab/core-pkg (gitignored; `npm run
  # lab:core` writes it in a checkout). Both npm builds of admin-ui/app copy
  # it in first: this one and tests/admin-ui.nix.
  labCorePkg = ''
    mkdir -p src/lab/core-pkg
    cp ${losos-lab-core}/* src/lab/core-pkg/
    chmod -R u+w src/lab/core-pkg
  '';

  # The Lab's libvirt client (admin-ui/lab/virt-rpc/README.md) as a
  # wasm-bindgen web package: losos_lab_virt.js, its .d.ts and the .wasm.
  # Built the same way as losos-lab-core, for the same reasons, and pinned
  # to the same wasm-bindgen.
  losos-lab-virt = pkgs.rustPlatform.buildRustPackage {
    pname = "losos-lab-virt";
    version = "0.1.0";
    src = lib.cleanSource ./../admin-ui/lab/virt-rpc;
    cargoHash = "sha256-m5B7NE6dWVgI1bu31+OQ0Kf71AvmrI5L9MeqldQAvlE=";
    nativeBuildInputs = [
      pkgs.wasm-bindgen-cli
      pkgs.lld
    ];
    buildPhase = ''
      runHook preBuild
      cargo build --release --offline --target wasm32-unknown-unknown
      runHook postBuild
    '';
    doCheck = false;
    installPhase = ''
      runHook preInstall
      wasm-bindgen --target web --out-dir $out \
        target/wasm32-unknown-unknown/release/losos_lab_virt.wasm
      runHook postInstall
    '';
  };

  # The Lab page imports the client from src/lab/virt-pkg (gitignored; `npm
  # run lab:virt` writes it in a checkout), lazily, when a guest starts on
  # that path. Copied in beside the core by the same two npm builds.
  labVirtPkg = ''
    mkdir -p src/lab/virt-pkg
    cp ${losos-lab-virt}/* src/lab/virt-pkg/
    chmod -R u+w src/lab/virt-pkg
  '';

  # The Lab's canvas (admin-ui/lab/render/README.md): the logical and
  # physical views as a 3D scene drawn by Bevy on WebGPU or WebGL2,
  # compiled to wasm32 in one module with the core (it depends on ../core
  # by path, so the source is both crates). wasm-bindgen first, then
  # wasm-opt on its output, never before: wasm-bindgen reads the custom
  # sections wasm-opt would rewrite.
  losos-lab-render = pkgs.rustPlatform.buildRustPackage {
    pname = "losos-lab-render";
    version = "0.1.0";
    src = lib.fileset.toSource {
      root = ./../admin-ui/lab;
      fileset = lib.fileset.unions [
        ./../admin-ui/lab/core/Cargo.toml
        ./../admin-ui/lab/core/Cargo.lock
        ./../admin-ui/lab/core/src
        ./../admin-ui/lab/render/Cargo.toml
        ./../admin-ui/lab/render/Cargo.lock
        ./../admin-ui/lab/render/src
        ./../admin-ui/lab/render/assets
      ];
    };
    cargoRoot = "render";
    buildAndTestSubdir = "render";
    cargoHash = "sha256-LvuaFRE8Wa4cAuIC9ICKVMG4zg6IcNWTCJQRhbXKm7g=";
    nativeBuildInputs = [
      pkgs.wasm-bindgen-cli
      pkgs.binaryen
      pkgs.lld
    ];
    # Two modules: Bevy picks its WebGPU or WebGL2 code paths at compile
    # time, so a loader picks the module (src/lab/render.ts in the admin
    # UI, render/demo/loader.js in the demo).
    # The builds share no Bevy crate, so the first one's target directory
    # goes before the second starts.
    buildPhase = ''
      runHook preBuild
      for backend in webgpu webgl2; do
        (cd render && cargo build --release --offline --target wasm32-unknown-unknown \
          --no-default-features --features $backend)
        mkdir -p dist/$backend
        wasm-bindgen --target web --out-dir dist/$backend --out-name losos_lab_render \
          render/target/wasm32-unknown-unknown/release/losos_lab_render.wasm
        wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int \
          --enable-sign-ext --enable-mutable-globals --enable-reference-types \
          --enable-multivalue \
          dist/$backend/losos_lab_render_bg.wasm -o dist/$backend/losos_lab_render_bg.wasm
        rm -rf render/target
      done
      runHook postBuild
    '';
    doCheck = false;
    installPhase = ''
      runHook preInstall
      mkdir -p $out
      cp -r dist/webgpu dist/webgl2 $out/
      runHook postInstall
    '';
  };

  # The canvas's two modules, copied to src/lab/render-pkg/{webgpu,webgl2}
  # (gitignored; `npm run lab:render` writes them in a checkout). The page
  # imports one of them lazily, after it has drawn with the SVG canvas
  # (src/lab/render.ts). Copied in by the same two npm builds.
  labRenderPkg = ''
    mkdir -p src/lab/render-pkg
    cp -r ${losos-lab-render}/webgpu ${losos-lab-render}/webgl2 src/lab/render-pkg/
    chmod -R u+w src/lab/render-pkg
  '';
in
images
// {
  inherit losos-lab-core losos-lab-render losos-lab-virt;

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
    npmDepsHash = "sha256-Ci7BRAyK2N8M9zHHa5neoRSTdJgC1G0t7ollDfFrlP8=";

    # No postinstall scripts. playwright is a devDependency (tests/admin-ui.nix
    # drives the browser check with it) and its postinstall downloads browsers:
    # there is no network in the sandbox, and the browsers it would fetch are
    # not the ones anything here runs against. esbuild, rollup and
    # @tailwindcss/oxide get their native binaries from the linux-x64-gnu
    # packages pinned in package-lock.json rather than from a download, so
    # nothing else needs a script to run.
    npmFlags = [ "--ignore-scripts" ];
    env.PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD = "1";
    # LosOS Lab (lab/index.html, the second Vite entry) imports the Rust core,
    # the GPU canvas and, for guests on the relay path, the libvirt client.
    preBuild = labCorePkg + labVirtPkg + labRenderPkg;
    passthru = { inherit labCorePkg labVirtPkg labRenderPkg; };

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

  # The owner's handbook: the Docusaurus site in handbook/, built once more
  # here so every box carries its own copy at /handbook/ (served LAN-only by
  # modules/containers.nix). The same source is published to GitHub Pages by
  # .github/workflows/handbook.yml with the default base; this build sets
  # LOSOS_HANDBOOK_BASE so every asset and link is rooted under /handbook/.
  #
  # Why on the box at all: the manual is needed most when the box is reachable
  # from the LAN alone, which is also when the internet copy is not. The
  # search index is built into the bundle for the same reason.
  losos-handbook = pkgs.buildNpmPackage {
    pname = "losos-handbook";
    version = "0.1.0";
    src = ./../handbook;

    # SHA256 of the npm dependency cache built from handbook/package-lock.json,
    # same convention as losos-admin-ui above. After changing the lock file:
    # `nix run nixpkgs#prefetch-npm-deps -- handbook/package-lock.json`.
    npmDepsHash = "sha256-OLptgb87kkhvEZsevuS2gTG5RMLGonUMsnCY2eMr9eo=";

    npmFlags = [ "--ignore-scripts" ];
    env.LOSOS_HANDBOOK_BASE = "/handbook/";
    # Docusaurus reads the git log for "last updated" stamps when asked; the
    # config does not ask, and there is no .git in the sandbox anyway.
    env.CI = "true";

    # `npm run build` is `docusaurus build`, which fails on a broken link.
    # $out is the built site: index.html at the top, hashed assets under
    # assets/, one directory per page with its own index.html.
    installPhase = ''
      runHook preInstall
      cp -r build $out
      runHook postInstall
    '';
  };

  # The LosOS look for Nextcloud and Forgejo — the admin SPA's tokens.css plus
  # each app's mapping onto it. Consumed by modules/nextcloud-stack.nix,
  # modules/services.nix and images.nix through admin-ui/themes/default.nix,
  # not through this attribute; it is exported so the files can be built and
  # read on their own.
  losos-themes = (import ./../admin-ui/themes { inherit pkgs; }).package;

  losos-ctl = buildRustPackage {
    pname = "losos-ctl";
    version = "0.1.0";
    src = lib.cleanSource ./../backend;
    # SHA256 of the vendored crate tarball. If deps change, `nix build
    # .#losos-ctl-rs` will print the new hash to paste here.
    cargoHash = "sha256-1KoeePbWpae3P7+FsqNQXg1r+VdmQqrR8T0cBYUdaBU=";
    # No system deps: zbus speaks the D-Bus wire protocol natively, so there is
    # no libdbus to link against.
    # doCheck is off, and the tests still run — in CI's lint job and in
    # `devenv test`, both of which invoke `cargo test` directly.
    #
    # The reason was the 10-minute cap on Codeberg, the project's former CI
    # host. doCheck does not merely execute
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

  losos-registrar = buildRustPackage {
    pname = "losos-registrar";
    version = "0.1.0";
    src = lib.cleanSource ./../backend-registrar;
    # SHA256 of the vendored crate tarball. If deps change, `nix build
    # .#losos-registrar` will print the new hash to paste here.
    cargoHash = "sha256-qGOi6XSptf4nAr4GTYlI8kzeuR5U81f3BWxteMjkmU4=";
    # No system deps; pure Rust with rustls (no openssl).
    # Same reasoning as losos-ctl above: the suite runs via cargo test in the
    # lint job, not inside this derivation.
    doCheck = false;
  };

  # The same crate for the operator's own machine: the key ceremony
  # (`losos-registrar provision`, provisioning/edge-identity/README.md) runs
  # there, on a computer that has no Nix store, so the binary above — linked
  # against glibc under /nix/store — cannot run on it. This one is linked
  # statically against musl and depends on nothing; CI publishes it to GHCR
  # as `images:<channel>-x86_64` and the LosOS proxy serves it at
  # `/updates/<channel>/x86_64/losos-registrar` (see wiki/CI-and-Releases.md).
  # Plain rustPlatform of pkgsStatic rather than fast-build.nix's: the mold
  # and ccache stdenv is composed for the native glibc toolchain, and the
  # minutes it saves are not worth a second composition for one cross build.
  # The crate is pure Rust (rustls, ring), so musl needs no system library.
  # cargoHash is the vendored tarball's and does not depend on the target,
  # so it is the hash losos-registrar uses: bump both together.
  losos-registrar-static = pkgs.pkgsStatic.rustPlatform.buildRustPackage {
    pname = "losos-registrar-static";
    version = "0.1.0";
    src = lib.cleanSource ./../backend-registrar;
    cargoHash = "sha256-qGOi6XSptf4nAr4GTYlI8kzeuR5U81f3BWxteMjkmU4=";
    doCheck = false;
  };

  losos-sign-iso = pkgs.writeShellApplication {
    name = "losos-sign-iso";
    runtimeInputs = [
      pkgs.xorriso
      pkgs.mtools
      pkgs.sbsigntool
      pkgs.coreutils
      pkgs.gawk
      pkgs.gnused
    ];
    text = builtins.readFile ./sign-iso.sh;
  };
}
