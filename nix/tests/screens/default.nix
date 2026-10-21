# Screenshots of a booted box for any VM test: a NixOS module that turns a
# node into a laptop with a browser and `losos-capture` (capture.mjs says
# what a plan looks like). The pictures in a pull request and the frames of
# a film come from here, so they show the real box: lososd, nginx, LosOS
# Login and the built admin UI, never a dev server or a mocked API.
#
#   nodes.laptop = { imports = [ (import ./screens { inherit pkgs; }).browser ]; … };
#   laptop.succeed("losos-capture --base https://192.168.1.1 --plan /etc/plan.json --out /tmp/shots …")
#   laptop.copy_from_vm("/tmp/shots", "")
{ pkgs }:

let
  driver = pkgs.playwright-driver;
  # node with Playwright's own driver as its one module, and Chromium only.
  capture = pkgs.writeShellScriptBin "losos-capture" ''
    export PLAYWRIGHT_BROWSERS_PATH=${driver.browsers-chromium}
    export NODE_PATH=${pkgs.linkFarm "losos-capture-modules" [
      {
        name = "playwright-core";
        path = driver;
      }
    ]}
    exec ${pkgs.nodejs}/bin/node ${./capture.mjs} "$@"
  '';
in
{
  inherit capture;

  browser = _: {
    environment.systemPackages = [ capture ];
    # The admin UI's own fonts are bundled; these cover anything else.
    fonts.packages = [ pkgs.dejavu_fonts ];
    # Chromium under TCG wants room.
    virtualisation = {
      memorySize = 2048;
      cores = 2;
    };
  };
}
