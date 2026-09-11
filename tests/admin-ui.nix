# Browser check for the admin UI (admin-ui/app).
#
# Builds the real production bundle — `tsc --noEmit && vite build`, the same
# command that produces what nginx serves — and then drives it in headless
# chromium against a stubbed API.
#
# It exists because typechecking cannot answer the only question that matters
# on a box out of the carton: which screen does the owner land on. The page
# used to open on a dialog asking for an admin key that nothing on the
# appliance prints (`losos.admin.tokenFile` is 64 random hex characters written
# 0600 by lososd, on a machine with no SSH and no shell logins), so a new box
# could not be set up at all. That defect typechecked perfectly and shipped.
#
# lososd is not in the sandbox — it owns org.losos1 on the system bus and the
# policy for that ships in modules/daemon.nix — so the API is stubbed inside
# the browser with page.route(). That is not a weaker test than a running
# daemon would give: what is under test is the SPA's decision between the
# wizard and the sign-in prompt, and the input to that decision is one JSON
# field. tests/setup.nix is where the daemon half is exercised for real.
#
# Browsers come from nixpkgs' playwright-driver.browsers, and the `playwright`
# devDependency must pin the SAME version — the assertion below fails the eval
# when nixpkgs moves and the npm pin did not, rather than at run time inside a
# sandbox with no network to fix it from. Same arrangement, and same reasoning,
# as tests/design-system.nix.
{ pkgs }:
let
  inherit (pkgs) lib;
  pinnedPlaywright =
    (builtins.fromJSON (builtins.readFile ../admin-ui/app/package.json)).devDependencies.playwright;
in
assert lib.assertMsg (pinnedPlaywright == pkgs.playwright-driver.version)
  "losos-admin-ui-browser: admin-ui/app/package.json pins playwright ${pinnedPlaywright} but nixpkgs ships playwright-driver ${pkgs.playwright-driver.version}; bump the devDependency (npm install --save-dev --save-exact playwright@${pkgs.playwright-driver.version}) and refresh npmDepsHash";
pkgs.buildNpmPackage {
  pname = "losos-admin-ui-browser";
  version = "0.1.0";

  src = ../admin-ui/app;
  npmDepsHash = "sha256-xXHkWQdrs6B4OwWasb3vwTYHALRS8P94cnvihWlHbE4=";

  # No postinstall scripts: playwright must not try to fetch browsers (there is
  # no network here, and the ones it would fetch are not the ones this runs
  # against), and esbuild's binary arrives through its platform package.
  npmFlags = [ "--ignore-scripts" ];

  env = {
    PLAYWRIGHT_BROWSERS_PATH = pkgs.playwright-driver.browsers;
    PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS = "true";
    PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD = "1";
    FONTCONFIG_FILE = pkgs.makeFontsConf { fontDirectories = [ pkgs.dejavu_fonts ]; };
  };

  # npmBuildScript's default already ran `npm run build`, which is where the
  # typecheck lives. The install phase is the actual check.
  installPhase = ''
    runHook preInstall
    export HOME="$TMPDIR"
    npm run test:browser
    mkdir -p $out
    # The built bundle is worth keeping: when this check fails on a bundle that
    # differs from a developer's, the first question is what nix actually built.
    cp -r dist $out/dist
    echo ok > $out/result
    runHook postInstall
  '';
}
