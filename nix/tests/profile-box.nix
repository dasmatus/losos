# The box both profile tests boot (nix/tests/profiles.nix asserts,
# nix/tests/profile-screens.nix takes pictures): Authelia, lososd and
# nginx's front vhost in its container shape, the way nix/tests/identity.nix
# sets it up.
{
  pkgs,
  # What the front vhost serves. A one-line page when only the API is under
  # test.
  adminUi ? pkgs.runCommand "stub-admin-ui" { } ''
    mkdir -p $out
    echo "admin ui" > $out/index.html
  '',
  lososCtl ? (import ../packages.nix { inherit pkgs; }).losos-ctl,
}:
{ lib, ... }:
{
  imports = [
    ../../modules/options.nix
    ../../modules/containers.nix
    ../../modules/tls.nix
    ../../modules/daemon.nix
    ../../modules/identity.nix
  ];
  losos = {
    hostName = "mattbox";
    backend.package = lososCtl;
    admin.ui = adminUi;
  };
  # The test driver names the machine after its node; lososd only lets a
  # claim through for its own name.
  networking.hostName = "mattbox";
  systemd.services.lososd.path = lib.mkForce [ pkgs.coreutils ];
  virtualisation = {
    memorySize = 1536;
    cores = 2;
    vlans = [ 1 ];
  };
}
