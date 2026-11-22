# What this is:
#
# The nix-bend module: Nix interop for the Bend test port. Two things it
# gives the flake and the tests:
#
#   - `bend`: the pinned compiler binary from github.com/bendlang/bend. It
#     is not in nixpkgs, so the version and the sha256 sums come from the
#     install script bend-lang.com serves (curl -fsSL
#     https://bend-lang.com/install.sh) — bump `version` and `sums`
#     together, or fetchurl refuses the archive.
#   - `bendProofs`: the law gate as a Nix check — `bend PROOF.bend` over the
#     checked-out tree, so a law in LAWS.bend that no def in PROOF.bend
#     proves (or a proof that no longer holds) fails the build. CI runs it
#     as `checks.losos-bend-proofs` (flake.nix), next to the VM tests; the
#     git hooks run the same gate directly.
#
# The `*.bend` test files land beside the tests they replace (admin-ui/app
# tests, nix/tests drivers, ...) and are checked the same way, one
# `bend <file>.bend` per file, as the port grows. Nothing here configures a
# running box: this is a build/test module, not a NixOS module.
{
  pkgs,
  # The tree the law gate checks. The flake passes the repo root; the
  # default here is the repo root as seen from nix/bend.nix.
  src ? ../.,
}:

let
  version = "2.0.40";
  sums = {
    # From the install script, per OS and CPU: the sha256 of each release
    # archive on github.com/bendlang/bend/releases.
    "darwin-arm64" = "b2be003b041eb6cfc65326be18f79eb2e0bae2189be79a18c507e661b4b6900b";
    "darwin-x64" = "9229c8bdd0a93f63fa45cc5af5e9e56389ebb620d655920915daf966c6ca561c";
    "linux-arm64" = "2f42a6fa34ecdaa71ae105e5fa734f9a07e129d333e45448ab6d2ff03a837570";
    "linux-x64" = "c9ff8d67bd903fb6ad9a26b43d8a62c01021de82a44d55ce4a3dcd4732113d6b";
  };
  # Bend calls the CPU arm64/x64 where Nix says aarch64/x86_64.
  os =
    {
      "aarch64-darwin" = "darwin-arm64";
      "x86_64-darwin" = "darwin-x64";
      "aarch64-linux" = "linux-arm64";
      "x86_64-linux" = "linux-x64";
    }
    .${pkgs.system} or (throw "bend has no release for ${pkgs.system}");
  # fetchurl (not curl in a buildPhase): the sandbox has no network, and
  # fetchurl's hash check is what makes the pin a pin.
  archive = pkgs.fetchurl {
    url = "https://github.com/bendlang/bend/releases/download/v${version}/bend-${version}-${os}.tar.gz";
    sha256 = sums.${os};
  };
  # The compiler, shared by the gate below and by the test files as they
  # land. The flake exposes it so tests can reference the same binary.
  loader =
    if pkgs.stdenv.hostPlatform.isx86_64 then
      "ld-linux-x86-64.so.2"
    else if pkgs.stdenv.hostPlatform.isAarch64 then
      "ld-linux-aarch64.so.1"
    else
      throw "bend: no dynamic loader name for ${pkgs.system}";
  bend = pkgs.stdenvNoCC.mkDerivation {
    name = "bend-${version}";
    # The release binary links against /lib64 (the host's glibc), which a
    # Nix sandbox does not have. Repointing the interpreter at the store's
    # loader fixes that — and ONLY the interpreter: this is a Bun-packed
    # executable, and a heavier patch (autoPatchelfHook's rpath rewrite,
    # which grows the file) corrupts Bun's payload detection and the
    # compiler degenerates into plain bun, which then parses .bend as
    # TypeScript. Interpreter-only keeps the bytes it hashes intact and
    # the loader resolves libc from its own directory.
    nativeBuildInputs = [ pkgs.patchelf ];
    dontUnpack = true;
    installPhase = ''
      runHook preInstall
      mkdir -p $out
      # The archive nests everything under a top-level bend/ (the install
      # script moves bend/bin/bend, bend/bend2 and bend/guide from it).
      tar -xzf ${archive} -C $out --strip-components=1 bend/bin bend/bend2 bend/guide
      runHook postInstall
    '';
    postFixup = pkgs.lib.optionalString pkgs.stdenv.isLinux ''
      chmod u+w $out/bin/bend
      ${pkgs.patchelf}/bin/patchelf --set-interpreter ${pkgs.stdenv.cc.libc}/lib/${loader} $out/bin/bend
    '';
  };
in
{
  inherit bend;

  # The law gate. Every law LAWS.bend states, PROOF.bend must prove, or
  # `bend` exits non-zero and the check fails — the gate is the point.
  bendProofs = pkgs.stdenvNoCC.mkDerivation {
    name = "losos-bend-proofs";
    inherit src;
    nativeBuildInputs = [ bend ];
    # Bend needs a HOME for its check cache; the sandbox has none.
    # BEND_HOME stays pointed at the compiler package so telemetry and
    # any cache land somewhere bend already owns, not at $HOME.
    preBuild = "export HOME=$TMPDIR BEND_HOME=${bend} BEND_NO_TELEMETRY=1";
    buildPhase = ''
      runHook preBuild
      bend PROOF.bend
      runHook postBuild
    '';
    installPhase = "runHook preInstall; touch $out; runHook postInstall";
  };
}
