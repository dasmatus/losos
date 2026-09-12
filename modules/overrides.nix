# Runtime-editable losos settings — the single source of truth for the
# user-tunable losos.* options. The standalone settings SPA (admin-ui/, served
# at /settings on the front vhost) generates the Nix code for this file and
# POSTs it to `losos-ctl apply` (lososd), which rewrites this file in place
# and triggers a supervised `nixos-rebuild switch` (systemd-run transient
# unit). Because the flake is rebuilt from the local working tree
# (git+file:///etc/nixos), the rewritten file is picked up on the next eval
# (the "dirty tree" warning is expected and harmless).
#
# This file must stay tracked by git (git flakes only copy tracked files into
# the store) and must keep exactly the assignment lines lososd rewrites — the
# backend parses them line-based to report current settings back to the UI.
# Don't hand-edit unless you know what you're doing; use the settings UI.
#
# Values here mirror the option defaults in options.nix so the box behaves
# unchanged until an admin changes something.
#
# The four losos.hardening.* flags are here for a reason worth stating: this
# file is the ONLY way to change a losos option on a running box. There is no
# SSH and no shell login, so an option declared in options.nix but absent here
# is not merely off — it is unreachable, and was therefore a dead option on
# every appliance anyone installed. `losos.hardening.enable`, the baseline that
# costs nothing, is deliberately NOT exposed: turning the whole layer off from
# a web form is not a setting.
_:

{
  losos.sharingMyStorage = true;
  losos.nextcloud.mode = "container";
  losos.forgejo.mode = "container";
  losos.hostName = "mattbox";
  losos.nextcloud.https = false;
  losos.gpu.enable = true;
  losos.nextcloud.apachePort = 11000;
  losos.proxy.enable = false;
  losos.cluster.enable = false;
  losos.cluster.shareCompute = false;
  losos.cluster.computeWindow.start = "23:00";
  losos.cluster.computeWindow.end = "07:00";
  losos.hardening.apparmor = false;
  losos.hardening.malloc = false;
  losos.hardening.nosmt = false;
  losos.hardening.usbguard = false;
}
