# pm, the LosOS package manager (github.com/losos-project/pm), built from the
# flake's `pm` input as it is: nothing in its tree is changed.
#
# Mesh CI uses two things of it (modules/ci.nix): `pm container`, which the
# runner starts and stops each job's containers through, with LosOS's own
# plugin (backend/plugins/pm-ci/) answering for them; and `pm build`, which
# builds and signs the .rhai recipes a workflow names in pm's own jail.
#
# Three things a plain buildRustPackage does not do:
#
#   * pm's build.rs builds pm's bundled plugins from plugins/, a cargo
#     workspace of its own with its own Cargo.lock, with a nested cargo. Both
#     lockfiles are vendored and the two vendor directories joined, so the
#     nested build finds its crates through the same source replacement.
#   * The bundled tree-sitter grammars are C compiled for wasm32: clang
#     without nixpkgs' wrapper, which would add the host's libc flags.
#   * build.rs also builds one integration-test fixture for wasm32-wasip2, a
#     target nixpkgs' rustc has no standard library for. The fixture is only
#     for pm's own tests and is never embedded, so a rustc wrapper builds it
#     for wasm32-unknown-unknown instead.
{ pkgs, pmSrc }:

let
  inherit (pkgs) lib;

  vendorRoot = pkgs.rustPlatform.fetchCargoVendor {
    name = "pm";
    src = pmSrc;
    hash = "sha256-ZWQcjRKR98GAFgYoLrJkCDORtqNbjGA5QfJHFKcOzgA=";
  };
  vendorPlugins = pkgs.rustPlatform.fetchCargoVendor {
    name = "pm-plugins";
    src = pmSrc;
    cargoRoot = "plugins";
    hash = "sha256-Pzz6j785j9cCmiuPvVKHp9zO4yDnKuSidoSBPJWrb/4=";
  };
  # pm's vendor directory (its Cargo.lock and source replacement) with the
  # plugins' crates added. A crate both lockfiles name is the same crate.
  vendor = pkgs.runCommand "pm-vendor-joined" { } ''
    cp -r --no-preserve=mode ${vendorRoot} $out
    for source in ${vendorPlugins}/source-*/; do
      into=$out/$(basename "$source")
      mkdir -p "$into"
      for crate in "$source"*/; do
        name=$(basename "$crate")
        [ -e "$into/$name" ] || cp -r --no-preserve=mode "$crate" "$into/$name"
      done
    done
  '';

  # RUSTC_WRAPPER, which the nested cargo inherits: a compile for
  # wasm32-wasip2 is done for wasm32-unknown-unknown instead, whose standard
  # library nixpkgs' rustc has. Only the test fixture asks for wasip2, and
  # nothing in pm reads it.
  rustcWrapper = pkgs.writeShellScript "rustc-wasip2-as-unknown" ''
    rustc=$1
    shift
    args=()
    compile=0
    for arg in "$@"; do
      case $arg in
        --crate-name) compile=1 ;;
      esac
    done
    for arg in "$@"; do
      if [ "$compile" = 1 ] && [ "$arg" = wasm32-wasip2 ]; then
        arg=wasm32-unknown-unknown
      fi
      args+=("$arg")
    done
    exec "$rustc" "''${args[@]}"
  '';

  clang = pkgs.llvmPackages.clang-unwrapped;
in
pkgs.rustPlatform.buildRustPackage {
  pname = "pm";
  version = "0.1.0-${builtins.substring 0 7 (pmSrc.rev or "dirty")}";
  src = pmSrc;
  cargoDeps = vendor;

  nativeBuildInputs = [
    pkgs.lld
    pkgs.cmake
    pkgs.perl
    pkgs.pkg-config
  ];

  env = {
    RUSTC_WRAPPER = "${rustcWrapper}";
    CC_wasm32_unknown_unknown = "${clang}/bin/clang";
    AR_wasm32_unknown_unknown = "${pkgs.llvmPackages.llvm}/bin/llvm-ar";
  };

  cargoBuildFlags = [
    "--bin"
    "pm"
  ];
  passthru = { inherit vendorRoot vendorPlugins; };

  # pm's own suite needs the wasip2 fixture and a jail; it runs in pm's CI.
  doCheck = false;

  meta = {
    description = "Builds signed recipes into .cpkg archives and runs them in a Linux sandbox";
    homepage = "https://github.com/losos-project/pm";
    license = lib.licenses.gpl3Only;
    mainProgram = "pm";
  };
}
