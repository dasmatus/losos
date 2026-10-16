# mDNS and DNS-SD through systemd-resolved: `<hostName>.local` and the
# services in losos.mdns.services, for the box (modules/configuration.nix) and
# for an edge that advertises itself on its LAN (modules/edge.nix).
#
# This used to be Avahi. resolved replaced it because it is the one responder
# a NixOS system already ships, sandboxed by its upstream unit, with no D-Bus
# publishing API for anything else on the machine to announce through, and
# because LosOS Desktop finds boxes through resolved too
# (handbook/docs/in-depth/hardening.md). Avahi is not coming back alongside
# it: two responders on UDP 5353 answer every query twice.
{
  config,
  lib,
  ...
}:

let
  cfg = config.losos.mdns;
in
{
  config = lib.mkIf cfg.enable {
    services.resolved = {
      enable = true;
      settings.Resolve = {
        MulticastDNS = true;
        # LLMNR is the Windows LAN's name protocol, which nothing that talks
        # to a box uses; off, it is one listener (UDP and TCP 5355) fewer.
        LLMNR = false;
      };
    };

    # resolved turns mDNS on for every link by default (since systemd 254),
    # and NetworkManager leaves it to resolved unless a profile says
    # otherwise. Saying "yes" on every profile, including the "Wired
    # connection 1" it makes for a NIC it has never seen, keeps it that way
    # with no interface list: the box is a repurposed mini-PC, its NIC is
    # eno1 on one machine and enp0s31f6 on the next, and mDNS is the only way
    # to reach it.
    networking.networkmanager.connectionConfig."connection.mdns" =
      lib.mkIf config.networking.networkmanager.enable 2;

    networking.firewall.allowedUDPPorts = [ 5353 ];

    environment.etc = lib.mapAttrs' (
      name: text: lib.nameValuePair "systemd/dnssd/${name}.dnssd" { text = "[Service]\n${text}"; }
    ) cfg.services;
    # resolved reads the .dnssd files once, at start.
    systemd.services.systemd-resolved.restartTriggers = lib.mapAttrsToList (
      name: _: config.environment.etc."systemd/dnssd/${name}.dnssd".source
    ) cfg.services;
  };
}
