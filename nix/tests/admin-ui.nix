# Browser check for the admin UI (admin-ui/app).
#
# Builds the real production bundle — `tsc --noEmit && vite build`, the same
# command that produces what nginx serves — and then drives it in headless
# chromium against a stubbed API.
#
# "the same command" is literal: this check is nix/packages.nix's
# losos-admin-ui itself with a test step added, same source, same
# node_modules, and the front vhost serves the package's dist/ directly.
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
# field. nix/tests/setup.nix is where the daemon half is exercised for real.
#
# Browsers come from nixpkgs' playwright-driver.browsers, and the `playwright`
# devDependency must pin the SAME version — the assertion below fails the eval
# when nixpkgs moves and the package.json pin did not, rather than at run time inside a
# sandbox with no network to fix it from. Same arrangement, and same reasoning,
# as nix/tests/design-system.nix.
{
  pkgs,
  # The option document (flake.nix passes the `losos-options-doc` check). With
  # it, tests/advanced.browser.mjs renders every option the install
  # configuration declares; without it the test falls back to its fixture.
  optionsJson ? null,
  # The shipped package (nix/packages.nix, losos-admin-ui): its source, its
  # node_modules and the lines that copy LosOS Lab's three WebAssembly
  # packages into src/lab/ and losos-mdx's into src/mdx/, without which the
  # pages' imports do not resolve and `tsc` fails.
  ui,
  # The SFU behind calls (the losos-ctl package's losos-calls), which
  # tests/calls.browser.mjs runs a call through. Without it that file
  # cannot run, so the check needs it.
  callsPackage,
}:
let
  inherit (pkgs) lib;
  pinnedPlaywright =
    (builtins.fromJSON (builtins.readFile ../../admin-ui/app/package.json)).devDependencies.playwright;
in
assert lib.assertMsg (pinnedPlaywright == pkgs.playwright-driver.version)
  "losos-admin-ui-browser: admin-ui/app/package.json pins playwright ${pinnedPlaywright} but nixpkgs ships playwright-driver ${pkgs.playwright-driver.version}; bump the devDependency (bun add --dev --exact playwright@${pkgs.playwright-driver.version}) and refresh the node_modules hash in nix/packages.nix";
ui.overrideAttrs (old: {
  pname = "losos-admin-ui-browser";

  # tests/calls.browser.mjs decodes the SFU's tap with ffmpeg.
  # The calls browser test starts the SFU with its edge client; curl is a
  # runtime dependency of that client and must exist in the sandbox.
  nativeBuildInputs = old.nativeBuildInputs ++ [
    pkgs.ffmpeg-headless
    pkgs.curl
  ];

  env =
    old.env
    // {
      # Chromium only: that is all the suite launches. The full set also builds
      # playwright-webkit, which currently fails in nixpkgs (auto-patchelf cannot
      # find libmanette) and would take this check down with it.
      PLAYWRIGHT_BROWSERS_PATH = pkgs.playwright-driver.browsers.override {
        withFirefox = false;
        withWebkit = false;
      };
      PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS = "true";
      # fonts.nix: the generic families mapped, or the pictures come out serif.
      FONTCONFIG_FILE = import ./fonts.nix { inherit pkgs; };
      LOSOS_CALLS_BIN = "${callsPackage}/bin/losos-calls";
      # The sandbox has only loopback; Chromium takes a loopback candidate.
      LOSOS_CALLS_ADDRESS = "127.0.0.1";
    }
    // lib.optionalAttrs (optionsJson != null) {
      LOSOS_OPTIONS_JSON = optionsJson;
    };

  # The build phase already ran `bun run build`, which is where the
  # typecheck lives. The install phase is the actual check.
  installPhase = ''
    runHook preInstall
    export HOME="$TMPDIR"
    bun run test:browser

    # The UI runs on Bun and the browser, never on Node: no node: module
    # may come back into the source, the checks or the build scripts.
    if grep -rnE "(from |import\()[\"']node:|require\([\"']" src tests scripts vite*.ts; then
      echo "admin-ui/app: Node modules are not used here; use Bun's or the web's APIs" >&2
      exit 1
    fi

    # Two shape checks on the artefact nginx will serve. The browser test runs
    # against a bare Bun.serve server with no CSP, so it cannot see either of
    # these break — and both break silently, at first paint, on a box with no
    # shell to read a console from.
    #
    # theme-boot.js must exist at that exact unhashed name (the <script> tag in
    # index.html is hand-written, so Vite's HTML pipeline cannot rewrite it),
    # and it must still be referenced as a CLASSIC script: adding `type=module`
    # or `defer` would defer it past the first paint and reintroduce the
    # light/dark flash it exists to prevent. See admin-ui/app/vite.config.ts.
    test -f dist/theme-boot.js
    grep -q 'src="/theme-boot.js"' dist/index.html
    # The tag must NOT have picked up type=module/defer/async. Matching the
    # attributes rather than the whole tag keeps this from failing on
    # whitespace if Vite ever reformats the document.
    ! grep -E 'src="/theme-boot\.js"' dist/index.html | grep -Eq 'type=|defer|async'

    mkdir -p $out
    # The built bundle is worth keeping: when this check fails on a bundle that
    # differs from a developer's, the first question is what nix actually built.
    cp -r dist $out/dist
    echo ok > $out/result
    runHook postInstall
  '';
})
