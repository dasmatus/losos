# Per-output package definitions.
#   losos-ctl  — the control plane (Rust crate under backend/), built via
#                nix/fast-build.nix's buildRustPackage (mold as the
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
#                admin-ui/app/ with Bun. $out IS the document root
#                (index.html, hashed assets/, theme-boot.js), served by the
#                front Nginx vhost with an SPA fallback — one app on real
#                paths (/, /storage, /mesh, /apps, /settings/<pane>), not the
#                dashboard/ + settings/ page pair it replaced — with lososd's
#                loopback API proxied at /api/*. See admin-ui/app/ and
#                modules/containers.nix.
#
#                Its node_modules (nix/bun-deps.nix) is the one derivation here that
#                needs the network on a machine that cannot substitute it. That
#                costs a red CI job at worst, never a failed install: the
#                output has no store references, modules/cache.nix points every
#                box through its configured proxy, and flake.nix puts this path in the
#                ISO's storeContents.
#   losos-registrar — the Rust edge registration + Traefik and tunnel config
#                reconciler (`serve`) and appliance registration client
#                (`announce`, `join`). Built via the same buildRustPackage
#                from edge/registrar/. See modules/edge.nix (edge) and
#                modules/proxy.nix (appliance). cargoHash is the SHA256 of the
#                vendored crate tarball; the first `nix build
#                .#losos-registrar` after a dependency change prints the real
#                hash to paste here.
#   losos, losos-static — the terminal client (admin-ui/tui/), a Rust crate
#                with clap, miette, tracing and ratatui. Its build script
#                reads the web UI's catalogue, palette, pane keywords and the
#                salmon from admin-ui/, so the source is those files too.
#                losos-static is the same crate linked against musl for
#                machines without a Nix store; CI publishes it beside
#                install.sh. Both share one cargoHash.
#   losos-image-pause — the OCI image the local k3s
#                cluster builds its pods on, defined in nix/images.nix and
#                merged in below. modules/defaults.nix wires it to
#                losos.workloads.pauseImage. It is a flake package
#                because that is how it reaches a binary cache: the appliance
#                substitutes it, it never builds it (see the header of
#                nix/images.nix).
#   losos-image-rest-server — restic's REST server, which an edge runs on a
#                buyer's bought volume; modules/edge.nix hands it to the
#                edge's rke2 and names it to the registrar.
#   losos-lab-core — LosOS Diagrams' core in Rust (admin-ui/lab/core/),
#                built for wasm32 and run through wasm-bindgen. The page is
#                the admin UI's second page (admin-ui/app/diagrams/), served
#                at /diagrams/, and imports this package.
#   losos-lab-render is the Lab's canvas, built from admin-ui/lab/render/.
#                Bevy draws the two views as a 3D scene in one wasm module
#                with the core, through wasm-bindgen and then wasm-opt. It
#                is built twice, as webgpu/ and webgl2/, and the page picks.
#   losos-lab-virt — the Lab's libvirt client (admin-ui/lab/virt-rpc/):
#                libvirt's remote protocol for wasm32, through wasm-bindgen,
#                for a page that drives libvirt over `losos-registrar lab`'s
#                byte relay with no virsh on the host.
#   losos-mdx-wasm — losos-mdx (backend/mdx/), MDX and Markdown to sanitized
#                HTML, as the wasm-bindgen package Notes and LosOS Office
#                import, so the browser renders with the box's own code.
#   losos-sign-iso — signs the installer ISO's UEFI loader in place with the
#                Secure Boot db key, after `nix build`, so the key never
#                enters a store (modules/secure-boot.nix, nix/sign-iso.sh).
#                CI runs it with the SECURE_BOOT_DB_KEY secret;
#                nix/tests/secure-boot.nix with a throwaway key.
{
  pkgs,
  # nix/version.nix, the release tag. Only the terminal client carries
  # it; the crates, the SPA and the handbook keep their own package
  # versions, so a release does not rebuild what the release job imports
  # from its sibling jobs. flake.nix passes it; the tests that import this
  # file directly get the same file.
  version ? import ./version.nix,
  # losos-project/pm's source (the flake's `pm` input). The tests that
  # import this file directly pass none and get no pm.
  pmSrc ? null,
  ...
}:

let
  inherit (pkgs) lib;
  bunDeps = import ./bun-deps.nix { inherit pkgs; };

  # Only derivations may come out of this file — every attribute here becomes a
  # flake package, and a non-derivation breaks `nix flake check` and
  # `nix flake show` for everything else too.
  images = import ./images.nix { inherit pkgs; };
  # What Mesh CI's jobs run in unless they name an image.
  losos-image-ci-base = import ./ci-base-image.nix { inherit pkgs; };

  # mold as the linker, ccache in front of gcc, sccache in front of rustc —
  # for both crates, and for the dev shells through the same file, so that
  # `cargo` in a shell and `nix build` agree on how a binary is linked. The
  # caches are gated on the host exposing their directories to the sandbox
  # and are otherwise never invoked; mold is not gated on anything. The
  # header of fast-build.nix says what each one is worth here, and how to
  # turn the caches on for `nix build` on a dev machine.
  inherit (import ./fast-build.nix { inherit pkgs; }) buildRustPackage;

  # A Lab crate built for wasm32 as the wasm-bindgen web package the Lab
  # page imports: <wasm>.js, its .d.ts and the .wasm. Plain rustPlatform
  # rather than fast-build.nix's: mold and ccache are composed for the native
  # toolchain, and rustc links wasm32 with its own rust-lld. wasm-bindgen-cli
  # must be the version the crates pin (=0.2.127); the CLI refuses a module
  # built against another one.
  labWasm =
    {
      pname,
      dir,
      wasm,
      cargoHash,
      features ? [ ],
    }:
    pkgs.rustPlatform.buildRustPackage {
      inherit pname cargoHash;
      version = "0.1.0";
      src = lib.cleanSource dir;
      nativeBuildInputs = [
        pkgs.wasm-bindgen-cli
        pkgs.lld
      ];
      buildPhase = ''
        runHook preBuild
        cargo build --release --offline --target wasm32-unknown-unknown ${
          lib.optionalString (features != [ ]) "--features ${lib.concatStringsSep "," features}"
        }
        runHook postBuild
      '';
      doCheck = false;
      installPhase = ''
        runHook preInstall
        wasm-bindgen --target web --out-dir $out \
          target/wasm32-unknown-unknown/release/${wasm}.wasm
        runHook postInstall
      '';
    };

  # The Lab page imports a package from src/lab/<name>-pkg (gitignored;
  # `bun run lab:<name>` writes it in a checkout). Both Bun builds of
  # admin-ui/app copy it in first: this one and nix/tests/admin-ui.nix.
  labPkg = name: pkg: ''
    mkdir -p src/lab/${name}-pkg
    cp ${pkg}/* src/lab/${name}-pkg/
    chmod -R u+w src/lab/${name}-pkg
  '';

  # LosOS Lab's core (admin-ui/lab/core/README.md).
  losos-lab-core = labWasm {
    pname = "losos-lab-core";
    dir = ./../admin-ui/lab/core;
    wasm = "losos_lab_core";
    cargoHash = "sha256-TXH+ZimT0PygfRRnjOCNpFgi3Acmb4kxN0S7vcL9Pxk=";
  };
  labCorePkg = labPkg "core" losos-lab-core;

  # losos-mdx for the browser (backend/mdx/README.md): the same parser and
  # allowlist the box renders documents with, so a page's preview and an
  # export cannot disagree. Notes and LosOS Office import it.
  losos-mdx-wasm = labWasm {
    pname = "losos-mdx-wasm";
    dir = ./../backend/mdx;
    wasm = "losos_mdx";
    features = [ "wasm" ];
    cargoHash = "sha256-/PsXCBJFqy5hJ1ov20niOX5WhMcbm30UIkCoppjWiBI=";
  };
  # Copied to src/mdx/pkg (gitignored; `bun run mdx` writes it in a
  # checkout) by both builds of the UI, in preConfigure rather than preBuild:
  # the VM tests that build the UI without the Lab replace preBuild, and
  # LosOS Office and Notes still need the parser there.
  mdxPkg = ''
    mkdir -p src/mdx/pkg
    cp ${losos-mdx-wasm}/* src/mdx/pkg/
    chmod -R u+w src/mdx/pkg
  '';

  # The Lab's libvirt client (admin-ui/lab/virt-rpc/README.md), loaded
  # lazily when a guest starts on that path.
  losos-lab-virt = labWasm {
    pname = "losos-lab-virt";
    dir = ./../admin-ui/lab/virt-rpc;
    wasm = "losos_lab_virt";
    cargoHash = "sha256-m5B7NE6dWVgI1bu31+OQ0Kf71AvmrI5L9MeqldQAvlE=";
  };
  labVirtPkg = labPkg "virt" losos-lab-virt;

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
  # (gitignored; `bun run lab:render` writes them in a checkout). The page
  # imports one of them lazily, after it has drawn with the SVG canvas
  # (src/lab/render.ts). Copied in by the same two Bun builds.
  labRenderPkg = ''
    mkdir -p src/lab/render-pkg
    cp -r ${losos-lab-render}/webgpu ${losos-lab-render}/webgl2 src/lab/render-pkg/
    chmod -R u+w src/lab/render-pkg
  '';
  # The registrar depends on the terminal envelope crate by path
  # (`../backend/access`), so its source is both trees, rooted at the repo.
  registrarSource = {
    src = lib.fileset.toSource {
      root = ./..;
      fileset = lib.fileset.unions [
        (lib.fileset.fromSource (lib.cleanSource ./../edge/registrar))
        ./../backend/access/Cargo.toml
        ./../backend/access/src
      ];
    };
    cargoRoot = "edge/registrar";
    buildAndTestSubdir = "edge/registrar";
  };
  # The terminal client reads the web UI's catalogue, palette, pane keywords
  # and the salmon at build time (admin-ui/tui/build.rs) and links the
  # envelope crate by path, so its source is those files as well.
  tuiSource = {
    src = lib.fileset.toSource {
      root = ./..;
      fileset = lib.fileset.unions [
        ./../admin-ui/tui/Cargo.toml
        ./../admin-ui/tui/Cargo.lock
        ./../admin-ui/tui/build.rs
        ./../admin-ui/tui/src
        ./../admin-ui/app/src/i18n
        ./../admin-ui/app/src/styles/tokens.css
        ./../admin-ui/app/src/screens/settings/panes.ts
        ./../admin-ui/themes/brand/salmon-128.png
        ./../backend/access/Cargo.toml
        ./../backend/access/src
      ];
    };
    cargoRoot = "admin-ui/tui";
    buildAndTestSubdir = "admin-ui/tui";
  };

  # lososd's plugins (backend/plugins/README.md): every crate of that
  # workspace but the test-only probe, built for wasm32 and wrapped as a
  # component, each beside its manifest. lososd loads this directory and
  # nothing else (losos.backend.plugins). Plain rustPlatform like the Lab's
  # crates: rustc links wasm32 with rust-lld, not mold. `names` lets a test
  # build a set with the probe in it.
  # backend/plugins' vendored crates: lososd's plugins and Mesh CI's pm
  # plugin are built from the one workspace.
  pluginsCargoHash = "sha256-QjOlI0fBfoVpVz2sFyLzyLTfEgN4IVJHnwHNsgmZrnA=";
  lososPlugins =
    {
      pname ? "losos-plugins",
      names ? [
        "assistant"
        "look"
        "pins"
        "mail"
        "ci"
        "statistics"
        "hidden"
        "basic"
      ],
    }:
    pkgs.rustPlatform.buildRustPackage {
      inherit pname;
      version = "0.1.0";
      src = lib.cleanSource ./../backend/plugins;
      cargoHash = pluginsCargoHash;
      nativeBuildInputs = [
        pkgs.wasm-tools
        pkgs.lld
      ];
      buildPhase = ''
        runHook preBuild
        cargo build --release --offline --target wasm32-unknown-unknown \
          ${lib.concatMapStringsSep " " (n: "-p losos-plugin-${n}") names}
        runHook postBuild
      '';
      doCheck = false;
      installPhase = ''
        runHook preInstall
        mkdir -p $out
        ${lib.concatMapStringsSep "\n" (n: ''
          wasm-tools component new \
            target/wasm32-unknown-unknown/release/losos_plugin_${n}.wasm \
            -o $out/${n}.wasm
          wasm-tools validate --features component-model $out/${n}.wasm
          cp ${n}/manifest.json $out/${n}.json
        '') names}
        runHook postInstall
      '';
    };

in
images
// lib.optionalAttrs (pmSrc != null) (
  let
    pm = import ./pm.nix { inherit pkgs pmSrc; };
  in
  {
    inherit pm;
    # Mesh CI's pm plugin, signed, as a pm configuration directory.
    losos-ci-pm-plugin = import ./ci-pm-plugin.nix {
      inherit pkgs pm pmSrc;
      baseImage = losos-image-ci-base;
      cargoHash = pluginsCargoHash;
    };
  }
)
// {
  inherit losos-image-ci-base;
  inherit losos-lab-core losos-lab-render losos-lab-virt;
  inherit losos-mdx-wasm;

  losos-admin-ui = pkgs.stdenvNoCC.mkDerivation (finalAttrs: {
    pname = "losos-admin-ui";
    version = "0.1.0";

    # admin-ui/app, not admin-ui/. The source ROOT is the exclusion: it is what
    # keeps admin-ui/design-system/react out of the closure, and unlike the
    # `baseNameOf name != "design-system"` filter this replaces, a root cannot
    # quietly stop matching when somebody renames a directory.
    src = ./../admin-ui/app;

    nativeBuildInputs = [ pkgs.bun ];
    # The person segmenter behind a call's black background
    # (src/calls/background.ts): MediaPipe's selfie segmenter, Apache-2.0,
    # a fixed version of the model, copied beside the chat page.
    env.LOSOS_SEGMENTER_MODEL = "${pkgs.fetchurl {
      url = "https://storage.googleapis.com/mediapipe-models/image_segmenter/selfie_segmenter/float16/1/selfie_segmenter.tflite";
      hash = "sha256-GRrJUprlBu4L7vprLJRaFy2rnQfR6AKikKTkA4ImZYs=";
    }}";
    # The nudity warning (src/lib/nudity.ts): Marqo's nsfw-image-detection-384
    # (Apache-2.0), exported to ONNX at a fixed revision. Its safetensors in
    # that repository are Marqo's own, byte for byte; the export's report
    # gives the largest logit difference against PyTorch as 7e-7. Copied to
    # nudity/model.onnx and only fetched by a page that turned the warning on.
    env.LOSOS_NUDITY_MODEL = "${pkgs.fetchurl {
      url = "https://huggingface.co/KanariKanaru/nsfw-image-detection-384-onnx/resolve/8edc47eedf74b30fd379673bc202fe3b754b1538/model.onnx";
      hash = "sha256-6TUOV2YIr+S1egif/rDrr6E4nNzqSILdYd8oxF8cJNI=";
    }}";

    # The dependencies, from admin-ui/app/bun.lock alone (nix/bun-deps.nix).
    configurePhase = ''
      runHook preConfigure
      ${finalAttrs.passthru.nodeModules.install}
      runHook postConfigure
    '';
    # LosOS Lab (lab/index.html, the second Vite entry) imports the Rust core,
    # the GPU canvas and, for guests on the relay path, the libvirt client.
    preConfigure = mdxPkg;
    preBuild = labCorePkg + labVirtPkg + labRenderPkg;
    # `bun run build` is `tsc --noEmit && vite build` (twice, the guest page
    # too), all on Bun's runtime, so unlike the Rust crates (doCheck = off,
    # see below) the typecheck IS part of building the shipping artefact. It
    # costs seconds, not the minutes doCheck costs rustc.
    buildPhase = ''
      runHook preBuild
      export HOME=$TMPDIR
      bun run build
      runHook postBuild
    '';

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

    passthru = {
      inherit
        labCorePkg
        labVirtPkg
        labRenderPkg
        mdxPkg
        ;
      # nix/tests/admin-ui.nix and the VM tests' Playwright reuse it. After a
      # change to bun.lock, `nix build .#losos-admin-ui.nodeModules` prints
      # the hash to paste here.
      nodeModules = bunDeps {
        pname = "losos-admin-ui";
        src = ./../admin-ui/app;
        hash = "sha256-LUyu1qNDz5MHIdLaDE59k/sU7lJeBoKfgjuZznPv+Z0=";
      };
    };
  });

  # The owner's handbook: the Docusaurus site in handbook/, built once more
  # here so every box carries its own copy at /handbook/ (served LAN-only by
  # modules/containers.nix). The same source is published to GitHub Pages by
  # .github/workflows/handbook.yml with the default base; this build sets
  # LOSOS_HANDBOOK_BASE so every asset and link is rooted under /handbook/.
  #
  # Why on the box at all: the manual is needed most when the box is reachable
  # from the LAN alone, which is also when the internet copy is not. The
  # search index is built into the bundle for the same reason.
  losos-handbook = pkgs.stdenvNoCC.mkDerivation (finalAttrs: {
    pname = "losos-handbook";
    version = "0.1.0";
    src = ./../handbook;

    nativeBuildInputs = [ pkgs.bun ];
    env.LOSOS_HANDBOOK_BASE = "/handbook/";
    # Docusaurus reads the git log for "last updated" stamps when asked; the
    # config does not ask, and there is no .git in the sandbox anyway.
    env.CI = "true";

    configurePhase = ''
      runHook preConfigure
      ${finalAttrs.passthru.nodeModules.install}
      runHook postConfigure
    '';
    # `bun run build` is `docusaurus build` on Bun's runtime, which fails on
    # a broken link.
    buildPhase = ''
      runHook preBuild
      export HOME=$TMPDIR
      bun run build
      runHook postBuild
    '';
    # $out is the built site: index.html at the top, hashed assets under
    # assets/, one directory per page with its own index.html.
    installPhase = ''
      runHook preInstall
      cp -r build $out
      runHook postInstall
    '';

    # From handbook/bun.lock and the vendored braces it installs, same
    # convention as losos-admin-ui above.
    passthru.nodeModules = bunDeps {
      pname = "losos-handbook";
      src = ./../handbook;
      local = [ "vendor/braces" ];
      hash = "sha256-YRXOmYkE9v7PZH5yWnQ5Wrr/qeygE8uN60iqmvaofS8=";
    };
  });

  # The LosOS look for LosOS Login: the admin SPA's tokens.css plus its
  # mapping onto it. Consumed by modules/identity.nix through
  # admin-ui/themes/default.nix,
  # not through this attribute; it is exported so the files can be built and
  # read on their own.
  losos-themes = (import ./../admin-ui/themes { inherit pkgs; }).package;

  losos-plugins = lososPlugins { };

  # Photos' face models (nix/face-models.nix), read by losos-cloud-photos.
  losos-face-models = import ./face-models.nix { inherit pkgs; };
  # The TPM makers' certificates boxes check gateways against
  # (nix/gateway/tpm-roots.nix, losos.proxy.gateways.tpmRoots).
  losos-tpm-roots = import ./gateway/tpm-roots.nix { inherit pkgs; };
  # The same with the probe, a plugin that tries every import, for
  # nix/tests/plugins.nix. Never on a box.
  losos-plugins-probe = lososPlugins {
    pname = "losos-plugins-probe";
    names = [
      "assistant"
      "look"
      "pins"
      "mail"
      "probe"
    ];
  };

  losos-ctl = buildRustPackage {
    pname = "losos-ctl";
    version = "0.1.0";
    src = lib.cleanSource ./../backend;
    # SHA256 of the vendored crate tarball. If deps change, `nix build
    # .#losos-ctl-rs` will print the new hash to paste here.
    cargoHash = "sha256-DNgHyFru61Vc9j6IxR+otvMh3iwoUprN7ccRsJ38jAk=";
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
    inherit (registrarSource) src cargoRoot buildAndTestSubdir;
    # SHA256 of the vendored crate tarball. If deps change, `nix build
    # .#losos-registrar` will print the new hash to paste here.
    cargoHash = "sha256-LedCH3lBYEFRAqqO4XKcqtmtWtbXLbqN5JEulQ9QcbY=";
    # No system deps; pure Rust with rustls (no openssl).
    # Same reasoning as losos-ctl above: the suite runs via cargo test in the
    # lint job, not inside this derivation.
    doCheck = false;
  };

  # The same crate for the operator's own machine: the key ceremony
  # (`losos-registrar provision`, docs/provisioning/edge-identity/README.md) runs
  # there, on a computer that has no Nix store, so the binary above — linked
  # against glibc under /nix/store — cannot run on it. This one is linked
  # statically against musl and depends on nothing; CI publishes it to GHCR as
  # `images:<channel>-x86_64` and the LosOS proxy serves it at
  # `/updates/<channel>/x86_64/losos-registrar` (see
  # docs/ci-and-releases.md). Plain rustPlatform of pkgsStatic
  # rather than fast-build.nix's: the mold and ccache stdenv is composed for the
  # native glibc toolchain, and the minutes it saves are not worth a second
  # composition for one cross build. The crate is pure Rust (rustls, ring), so
  # musl needs no system library. cargoHash is the vendored tarball's and does
  # not depend on the target, so it is the hash losos-registrar uses: bump both
  # together.
  losos-registrar-static = pkgs.pkgsStatic.rustPlatform.buildRustPackage {
    pname = "losos-registrar-static";
    version = "0.1.0";
    inherit (registrarSource) src cargoRoot buildAndTestSubdir;
    cargoHash = "sha256-LedCH3lBYEFRAqqO4XKcqtmtWtbXLbqN5JEulQ9QcbY=";
    doCheck = false;
  };

  # The terminal client. Its suites (a mock box and edge over TLS, a pseudo
  # terminal) run via cargo test in CI's test job, as with the crates above.
  losos = buildRustPackage {
    pname = "losos";
    version = "0.1.0";
    inherit (tuiSource) src cargoRoot buildAndTestSubdir;
    cargoHash = "sha256-0VQK6reYxlHCgLQuyAb0ttNCIRVy3PBvs9nFSAqj/7U=";
    # `losos --version` and the About pane say the release (VERSION in
    # admin-ui/tui/src/tui/panes.rs); without it they would say 0.1.0.
    env.LOSOS_VERSION = version;
    doCheck = false;
  };

  # The same client for any x86_64 Linux machine, which is where the curl
  # install line puts it: musl, no dynamic library, same cargoHash.
  losos-static = pkgs.pkgsStatic.rustPlatform.buildRustPackage {
    pname = "losos-static";
    version = "0.1.0";
    inherit (tuiSource) src cargoRoot buildAndTestSubdir;
    cargoHash = "sha256-0VQK6reYxlHCgLQuyAb0ttNCIRVy3PBvs9nFSAqj/7U=";
    env.LOSOS_VERSION = version;
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
