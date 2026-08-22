# Browser render check for the losos design system. Builds the losos-ds npm
# package (design-system/react) in the sandbox and runs both its test suites:
# the node markup test, and the playwright browser test that mounts every
# component in headless chromium and asserts computed styles straight from
# tokens.css. Browsers come from nixpkgs' playwright-driver.browsers; the
# `playwright` devDependency in package.json must pin the SAME version as
# pkgs.playwright-driver. The assertion below fails the eval when the flake's
# nixpkgs moves playwright and the npm pin wasn't bumped to match.
#
# This is a flake check only; design-system/react stays dev-machine-only and
# never enters the appliance closure (see CLAUDE.md).
{ pkgs }:
let
  inherit (pkgs) lib;
  pinnedPlaywright =
    (builtins.fromJSON (builtins.readFile ../design-system/react/package.json))
      .devDependencies.playwright;
in
assert lib.assertMsg (pinnedPlaywright == pkgs.playwright-driver.version)
  "losos-ds-render: package.json pins playwright ${pinnedPlaywright} but nixpkgs ships playwright-driver ${pkgs.playwright-driver.version}; bump the devDependency (npm install --save-dev --save-exact playwright@${pkgs.playwright-driver.version}) and refresh npmDepsHash";
pkgs.buildNpmPackage {
  pname = "losos-ds-render";
  version = "0.1.0";

  # src is the whole design-system/ dir: the package build script reaches
  # ../tokens.css and ../losos.css.
  src = ../design-system;
  sourceRoot = "design-system/react";
  npmDepsHash = "sha256-Bbc5jJ/Shk117iQDik4I60ClOxegWf897ELoXfF9D/U=";

  # No postinstall scripts: esbuild's binary rides in via its platform
  # package, and playwright must not try to download browsers (no network).
  npmFlags = [ "--ignore-scripts" ];

  env = {
    PLAYWRIGHT_BROWSERS_PATH = pkgs.playwright-driver.browsers;
    PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS = "true";
    PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD = "1";
    FONTCONFIG_FILE = pkgs.makeFontsConf { fontDirectories = [ pkgs.dejavu_fonts ]; };
  };

  # `npm run build` already ran (npmBuildScript default); the install phase
  # is the actual check. The screenshot lands in $out for inspection.
  installPhase = ''
    runHook preInstall
    export HOME="$TMPDIR"
    npm test
    npm run test:browser
    mkdir -p $out
    cp dist/.browser-test.png $out/
    echo ok > $out/result
    runHook postInstall
  '';
}
