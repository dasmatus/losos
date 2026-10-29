# Ports and paths of the box's own public TLS (modules/public-tls.nix),
# shared with the modules that add the front vhost's listeners
# (modules/containers.nix, modules/tls.nix) and the proxy's tunnel client
# (modules/proxy.nix).
{ lib }:

rec {
  state = "/var/lib/losos-public-tls";
  # Every public name the edge routes to this box, one per line, from the
  # heartbeat's answer (edge/registrar/src/passthrough.rs, record_names).
  namesFile = "${state}/names";
  # The names this box holds a valid certificate for, one per line. Present,
  # even empty, is what asks the edge to pass TLS through.
  readyFile = "${state}/ready";
  # What the hourly check of the box's own names found (one JSON object).
  probeFile = "${state}/probe.json";
  webroot = "${state}/webroot";
  live = "${state}/live";

  # Where the tunnel delivers: nginx's stream module reads the first bytes
  # and sends a TLS ClientHello to `tlsPort` and anything else (the edge's
  # plain HTTP: HTTP-01, the manage server, an older edge) to port 80.
  splitPort = 8446;
  # nginx's stream module ends TLS here with the certificate for the name
  # the client asked for.
  tlsPort = 8447;
  # Then the front vhost's loopback listener, which LosOS Login takes as
  # https (modules/identity.nix, `$losos_proto`). Loopback, so the LAN guard
  # turns it away from the admin surface like the tunnel's port 80.
  frontPort = 8482;

  frontListen =
    on:
    lib.optional on {
      addr = "127.0.0.1";
      port = frontPort;
    };
}
