# Application services. LosOS Cloud, the owner's files, calendars and
# contacts, is modules/cloud.nix; LosOS Git, the box's Git server, is
# modules/git.nix. This module keeps the nginx front door they share.
#
# State is persisted via impermanence's /var bind-mount (see impermanence.nix),
# so nothing is lost across the tmpfs-root reboot.
#
# Tahoe-LAFS used to be the third service here: a local introducer plus a
# `shared` storage/client node, fronted by a port-based Nginx vhost on :3456
# because the Tahoe WUI writes absolute links and has no path-prefix support.
# It is gone. The shared user's storage domain is served by Longhorn on the
# mesh rke2 cluster now (docs/superpowers/specs/2026-09-09-k3s-mesh-design.md),
# and `shared`'s data directory is an fscrypt policy under /persist rather than
# a Tahoe node directory. Nothing here replaces it, and that is the point: the
# :3456 vhost was the only firewall hole this module punched, and the WUI it
# fronted bound 0.0.0.0:3457 — kept off the LAN by nothing but the absence of a
# second firewall entry. Both are gone rather than ported.
_:

{
  # Nginx front door (the :80 front vhost lives in modules/containers.nix).
  # Enabled unconditionally: the admin endpoint and every routed service
  # live behind it.
  services.nginx = {
    enable = true;
    recommendedProxySettings = true;
  };
}
