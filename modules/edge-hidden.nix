# An edge's boxes on Tor and I2P (losos.edge.hidden.tor.enable,
# losos.edge.hidden.i2p.enable).
#
# With either switch on, every box this edge routes gets an address of its
# own on that network, made here and kept here (/var/lib/tor, /var/lib/i2pd):
# a box's own switches (modules/hidden-services.nix) give it addresses that
# need no edge, these give it addresses that need no Tor or I2P on the box.
#
# The registrar (edge/registrar/src/hidden.rs) writes the live boxes to
# /var/lib/losos-registrar/hidden/services.json on every reconcile, and
# losos-hidden-sync (modules/hidden-lib.nix) makes one onion service and one
# I2P server tunnel per box from it. Every one of them delivers to the same
# two loopback ports, Traefik's `losos-hidden` (port 80 of an address, plain
# http) and `losos-hidden-tls` (port 443, Traefik's own certificate)
# entry points in modules/edge.nix. The registrar puts a router for each
# box's names there, and nothing else listens there: the public entry points
# know no onion name, so a request to the edge's public address cannot
# confirm which onion it serves, and a Tor visitor reaches a box only by
# that box's names. Traefik then sends the request down the box's tunnel
# like any other, with `X-Forwarded-Proto: https` on the TLS one, which is
# what LosOS Login on the box needs.
#
# A box learns its addresses from the answer to its heartbeat and keeps them
# for its Network page and LosOS Login (edge/registrar/src/announce.rs).
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.losos.edge.hidden;
  on = config.losos.edge.enable && (cfg.tor.enable || cfg.i2p.enable);
  hidden = import ./hidden-lib.nix { inherit lib pkgs; };
in
{
  config = lib.mkIf on (
    hidden.config {
      tor = cfg.tor.enable;
      i2p = cfg.i2p.enable;
      servicesFile = "/var/lib/losos-registrar/hidden/services.json";
      hops = config.lososInternal.hidden.i2pHops;
    }
  );
}
