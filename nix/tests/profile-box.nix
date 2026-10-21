# The box both profile tests boot (nix/tests/profiles.nix asserts,
# nix/tests/profile-screens.nix takes pictures): Authelia, lososd and
# nginx's front vhost in its container shape, LosOS cloud as a stub that
# says "installed", the way nix/tests/identity.nix sets it up.
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
let
  stubOcc = pkgs.writeShellScriptBin "nextcloud-occ" ''
    case "$1" in
      status) echo '{"installed":true,"maintenance":false,"needsDbUpgrade":false}' ;;
      user:resetpassword) echo "Successfully reset password for $2" ;;
      *) echo "stub occ: $*" >&2; exit 1 ;;
    esac
  '';
  # Any derivation with the two attributes imageRef reads: no cluster runs.
  standIn =
    name:
    pkgs.hello
    // {
      imageName = name;
      imageTag = "test";
    };
in
{
  imports = [
    ../../modules/options.nix
    ../../modules/containers.nix
    ../../modules/tls.nix
    ../../modules/daemon.nix
    ../../modules/identity.nix
    ../../modules/workloads.nix
  ];
  losos = {
    hostName = "mattbox";
    backend.package = lososCtl;
    admin.ui = adminUi;
    nextcloud.mode = "container";
    forgejo.enable = true;
    forgejo.mode = "container";
    workloads.nextcloudImage = standIn "losos-nextcloud-stand-in";
    workloads.forgejoImage = standIn "losos-forgejo-stand-in";
  };
  systemd.services.lososd.path = lib.mkForce [
    stubOcc
    pkgs.coreutils
  ];
  # The stub above, not a pod.
  systemd.services.lososd.environment.LOSOS_NEXTCLOUD_MODE = lib.mkForce "native";
  virtualisation = {
    memorySize = 1536;
    cores = 2;
    vlans = [ 1 ];
  };
}
