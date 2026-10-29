# Browser render check for the losos design system. Builds the losos-ds
# package (admin-ui/design-system/react) with Bun in the sandbox and runs
# both its test suites: the markup test, and the playwright browser test that mounts every
# component in headless chromium and asserts computed styles straight from
# tokens.css. Browsers come from nixpkgs' playwright-driver.browsers; the
# `playwright` devDependency in package.json must pin the SAME version as
# pkgs.playwright-driver. The assertion below fails the eval when the flake's
# nixpkgs moves playwright and the package.json pin wasn't bumped to match.
#
# This is a flake check only; admin-ui/design-system/react stays
# dev-machine-only and never enters the appliance closure (see .claude/CLAUDE.md).
{ pkgs }:
let
  inherit (pkgs) lib;
  # From admin-ui/design-system/react/bun.lock (nix/bun-deps.nix).
  nodeModules = import ../bun-deps.nix { inherit pkgs; } {
    pname = "losos-ds";
    src = ../../admin-ui/design-system/react;
    hash = "sha256-uqZQDnSRq8dXFCFCZavaVZW0GyT/8isrhfAccmG5KFE=";
  };
  pinnedPlaywright =
    (builtins.fromJSON (builtins.readFile ../../admin-ui/design-system/react/package.json))
    .devDependencies.playwright;
in
assert lib.assertMsg (pinnedPlaywright == pkgs.playwright-driver.version)
  "losos-ds-render: package.json pins playwright ${pinnedPlaywright} but nixpkgs ships playwright-driver ${pkgs.playwright-driver.version}; bump the devDependency (bun add --dev --exact playwright@${pkgs.playwright-driver.version}) and refresh the node_modules hash below";
pkgs.stdenvNoCC.mkDerivation {
  pname = "losos-ds-render";
  version = "0.1.0";

  # src is the whole admin-ui/design-system/ dir: the package build script
  # reaches ../tokens.css and ../losos.css. sourceRoot stays relative to the
  # unpacked store copy, whose name is the path's basename ("design-system").
  src = ../../admin-ui/design-system;
  sourceRoot = "design-system/react";

  nativeBuildInputs = [ pkgs.bun ];
  env = {
    # Chromium only, all the test launches; nixpkgs' playwright-webkit does
    # not build (the same override as nix/tests/admin-ui.nix).
    PLAYWRIGHT_BROWSERS_PATH = pkgs.playwright-driver.browsers.override {
      withFirefox = false;
      withWebkit = false;
    };
    PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS = "true";
    # fonts.nix: the generic families mapped, or the pictures come out serif.
    FONTCONFIG_FILE = import ./fonts.nix { inherit pkgs; };
  };

  configurePhase = ''
    runHook preConfigure
    ${nodeModules.install}
    runHook postConfigure
  '';
  buildPhase = ''
    runHook preBuild
    export HOME="$TMPDIR"
    bun run build
    runHook postBuild
  '';
  # .github/scripts/fix-hashes.py finds the hash here.
  passthru = { inherit nodeModules; };

  # The install phase is the actual check. The screenshot lands in $out for
  # inspection.
  installPhase = ''
    runHook preInstall
    bun run test
    bun run test:browser
    mkdir -p $out
    cp dist/.browser-test.png $out/
    echo ok > $out/result
    runHook postInstall
  '';
}
