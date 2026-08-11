# Runtime-editable losos settings — the single source of truth for the
# user-tunable losos.* options. The losos admin app (PHP, in nextcloud-app/)
# renders the settings as a macOS-style page, generates the Nix code for THIS
# file from the form, and sends it to the Haskell `losos-ctl` backend, which
# rewrites this file in place and triggers a `nixos-rebuild switch`. Because
# the flake is rebuilt from the local working tree (git+file:///etc/nixos),
# the rewritten file is picked up on the next eval (the "dirty tree" warning
# is expected and harmless).
#
# This file MUST stay tracked by git (git flakes only copy tracked files into
# the store) and MUST keep exactly the assignment lines losos-ctl rewrites —
# the backend parses them line-based to report current settings back to the
# UI. Don't hand-edit unless you know what you're doing; use the admin UI.
#
# Values here mirror the option defaults in options.nix so the box behaves
# unchanged until an admin changes something.
{ ... }:

{
  losos.sharingMyStorage = true;
  losos.nextcloud.mode = "aio";
  losos.forgejo.mode = "container";
  losos.hostName = "mattbox";
  losos.nextcloud.https = false;
  losos.gpu.enable = true;
  losos.aio.apachePort = 11000;
  losos.aio.interfacePort = 8000;
}