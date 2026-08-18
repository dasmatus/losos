# Runtime-editable losos settings — the single source of truth for the
# user-tunable losos.* options. The standalone settings SPA (admin-ui/, served
# at /settings on the front vhost) generates the Nix code for THIS file and
# POSTs it to `losos-ctl apply` (lososd), which rewrites this file in place
# and triggers a supervised `nixos-rebuild switch` (systemd-run transient
# unit). Because the flake is rebuilt from the local working tree
# (git+file:///etc/nixos), the rewritten file is picked up on the next eval
# (the "dirty tree" warning is expected and harmless).
#
# This file MUST stay tracked by git (git flakes only copy tracked files into
# the store) and MUST keep exactly the assignment lines lososd rewrites — the
# backend parses them line-based to report current settings back to the UI.
# Don't hand-edit unless you know what you're doing; use the settings UI.
#
# Values here mirror the option defaults in options.nix so the box behaves
# unchanged until an admin changes something.
{ ... }:

{
  losos.sharingMyStorage = true;
  losos.nextcloud.mode = "container";
  losos.forgejo.mode = "container";
  losos.hostName = "mattbox";
  losos.nextcloud.https = false;
  losos.gpu.enable = true;
  losos.nextcloud.apachePort = 11000;
  losos.proxy.enable = false;
}
