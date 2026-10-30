# Mesh CI's pm plugin (backend/plugins/pm-ci/), laid out as the pm
# configuration directory the runner points XDG_CONFIG_HOME at:
#
#   pm/plugins/losos-ci.wasm       the component
#   pm/plugins/losos-ci.wasm.sig   its signature
#   pm/trusted/<key>.pub           the key that made it
#
# pm loads a plugin only when a trusted key signed it. The key is made here,
# signs this one file and is thrown away with the build directory, so the
# only thing it ever vouches for is this build of this plugin, and the
# configuration the runner uses trusts nothing else. A different build gets a
# different key; nothing outside this output needs to know it.
#
# The plugin is built from lososd's plugin workspace (same Cargo.lock, same
# cargoHash as losos-plugins) against pm's own WIT, kept beside it in
# pm-ci/wit/. The build fails when that copy and the pinned pm's differ.
{
  pkgs,
  pm,
  pmSrc,
  baseImage,
  cargoHash,
}:

let
  inherit (pkgs) lib;
in
pkgs.rustPlatform.buildRustPackage {
  pname = "losos-ci-pm-plugin";
  version = "0.1.0";
  inherit (import ./plugins-src.nix { inherit lib; }) src sourceRoot;
  inherit cargoHash;
  nativeBuildInputs = [
    pkgs.wasm-tools
    pkgs.lld
    pm
  ];
  env = {
    LOSOS_CI_BASE_IMAGE = "localhost/${baseImage.imageName}:${baseImage.imageTag}";
    LOSOS_CI_BASE_ARCHIVE = "${baseImage}";
  };
  postPatch = ''
    if ! cmp -s ${pmSrc}/wit/plugin.wit pm-ci/wit/plugin.wit; then
      echo "backend/plugins/pm-ci/wit/plugin.wit is not the pinned pm's wit/plugin.wit; copy it over" >&2
      exit 1
    fi
  '';
  buildPhase = ''
    runHook preBuild
    cargo build --release --offline --target wasm32-unknown-unknown -p losos-pm-ci
    runHook postBuild
  '';
  doCheck = false;
  installPhase = ''
    runHook preInstall
    mkdir -p $out/pm/plugins $out/pm/trusted
    wasm-tools component new \
      target/wasm32-unknown-unknown/release/losos_pm_ci.wasm \
      -o $out/pm/plugins/losos-ci.wasm
    wasm-tools validate --features component-model $out/pm/plugins/losos-ci.wasm
    export HOME=$TMPDIR XDG_CONFIG_HOME=$TMPDIR/config
    pm sign $out/pm/plugins/losos-ci.wasm \
      --key $TMPDIR/signing.key --trust-dir $out/pm/trusted
    # The plugin loads under its own configuration, as the runner will run it.
    XDG_CONFIG_HOME=$out RUST_LOG=warn pm plugins | grep losos-ci >/dev/null
    runHook postInstall
  '';
  passthru.baseImageName = "localhost/${baseImage.imageName}:${baseImage.imageTag}";
}
