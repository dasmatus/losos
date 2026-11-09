# The box on Tor and I2P (losos.hidden.tor.enable, losos.hidden.i2p.enable).
#
# With either switch on, the box has an address on that network: a `.onion`
# name, a `.b32.i2p` name. Each reaches what the box's public name reaches
# through an edge — LosOS Cloud, LosOS Git, LosOS Login and the admin pages
# — with no edge and no open port. The daemons and the list they serve are
# modules/hidden-lib.nix; this file is what the box adds.
#
# Where a visitor lands. Port 80 of the address goes to 127.0.0.1:8480 and
# port 443 to 127.0.0.1:8443, both on loopback only, so nothing on the LAN
# reaches them. Traefik holds the two listeners (modules/apps.nix):
#
#   * 8480 is its `onion-http` entry point, plain http. Never port 80
#     itself: the edge's manage server (modules/proxy.nix) listens there for
#     loopback, and a Tor client sends whatever Host header it likes.
#   * 8443 is its `onion-https` entry point, ending TLS with the certificate
#     below — the same certificate the front vhost's visitors were shown
#     before (nginx's stream module used to; Traefik took the listeners
#     over, and the admin pages ride the onion now, part B of
#     docs/superpowers/specs/2026-11-09-csr-offload-design.md). Authelia
#     refuses an http address and sets Secure cookies, so signing in needs
#     this half.
#
# The certificate names the box's addresses and is signed by the box's own
# certificate (modules/tls.nix, a CA the owner trusted on their devices in
# the first-run wizard), so a device that trusts the box trusts its
# addresses too. Any other visitor accepts it once, as with any onion site
# that has no certificate from a public authority. It is made again when
# the set of addresses changes, and Traefik restarts to read it.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.losos.hidden;
  on = cfg.tor.enable || cfg.i2p.enable;
  hidden = import ./hidden-lib.nix { inherit lib pkgs; };
  tlsDir = "${hidden.state}/tls";
  servicesFile = pkgs.writeText "losos-hidden-services.json" (
    builtins.toJSON [
      {
        id = "front";
        http = hidden.httpPort;
        https = hidden.httpsPort;
      }
    ]
  );
  # Traefik reads the certificate: the listeners it ends TLS on are the
  # hidden services' two ports. The group exists whenever this module's
  # certificate is served (modules/apps.nix runs Traefik whenever hidden
  # services are on).
  traefikGroup = config.users.groups.losos-traefik.name;
  boxTls = "/var/lib/losos-tls";

  mintTls = pkgs.writeShellApplication {
    name = "losos-hidden-tls";
    runtimeInputs = [
      pkgs.coreutils
      pkgs.jq
      pkgs.openssl
      pkgs.systemd
    ];
    text = ''
      install -d -m 0750 -o root -g ${traefikGroup} ${tlsDir}
      # A box that ran the nginx shape keeps the old group on the directory.
      chown root:${traefikGroup} ${tlsDir}
      chmod 0750 ${tlsDir}
      mapfile -t names < <(jq -r '[.[] | .tor, .i2p | strings] | sort | unique | .[]' \
        ${hidden.addressesFile} 2>/dev/null || true)
      want=$(printf '%s\n' "''${names[@]}")
      if [ -s ${tlsDir}/cert.pem ] && [ "$want" = "$(cat ${tlsDir}/names 2>/dev/null)" ]; then
        exit 0
      fi
      # Before Tor or I2P have made an address Traefik still needs a
      # certificate to start with; a name that never resolves.
      sans=""
      first=""
      for n in "''${names[@]}"; do
        [ -n "$n" ] || continue
        sans+="''${sans:+,}DNS:$n"
        first=''${first:-$n}
      done
      if [ -z "$sans" ]; then
        sans="DNS:losos-hidden.invalid"
        first="losos-hidden.invalid"
      fi
      work=$(mktemp -d)
      trap 'rm -rf "$work"' EXIT
      cat > "$work/ext" <<EOF
      basicConstraints = critical,CA:FALSE
      keyUsage = critical,digitalSignature
      extendedKeyUsage = serverAuth
      subjectAltName = $sans
      EOF
      openssl req -new -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes \
        -keyout "$work/key.pem" -subj "/CN=$first/O=losos" -out "$work/req.pem" 2>/dev/null
      if [ -r ${boxTls}/cert.pem ] && [ -r ${boxTls}/key.pem ]; then
        openssl x509 -req -in "$work/req.pem" -days 825 -sha256 \
          -CA ${boxTls}/cert.pem -CAkey ${boxTls}/key.pem -set_serial "0x$(openssl rand -hex 16)" \
          -extfile "$work/ext" -out "$work/leaf.pem" 2>/dev/null
        cat "$work/leaf.pem" ${boxTls}/cert.pem > "$work/cert.pem"
      else
        openssl x509 -req -in "$work/req.pem" -days 825 -sha256 -signkey "$work/key.pem" \
          -extfile "$work/ext" -out "$work/cert.pem" 2>/dev/null
      fi
      chown root:${traefikGroup} "$work/key.pem" "$work/cert.pem"
      chmod 0640 "$work/key.pem"
      chmod 0644 "$work/cert.pem"
      mv "$work/key.pem" ${tlsDir}/key.pem
      mv "$work/cert.pem" ${tlsDir}/cert.pem
      printf '%s' "$want" > ${tlsDir}/names
      echo "certificate for: ''${want:-no address yet}"
      # --no-block: Traefik is ordered after this unit (below), so a restart
      # that waited here would wait on itself. It reads the certificate at
      # start; the dynamic config's watcher does not follow the files'
      # contents, so a new one is worth a restart when it runs.
      if systemctl -q is-active losos-traefik.service; then
        systemctl restart --no-block losos-traefik.service || true
      fi
    '';
  };
in
{
  config = lib.mkIf on (
    lib.mkMerge [
      (hidden.config {
        tor = cfg.tor.enable;
        i2p = cfg.i2p.enable;
        inherit servicesFile;
        hops = config.lososInternal.hidden.i2pHops;
      })
      {
        systemd.services.losos-hidden-tls = {
          description = "TLS for this box's Tor and I2P addresses";
          wantedBy = [ "multi-user.target" ];
          before = [ "losos-traefik.service" ];
          # The box's own certificate signs this one.
          after = lib.optional config.losos.tls.enable "losos-tls-cert.service";
          serviceConfig = {
            Type = "oneshot";
            ExecStart = "${mintTls}/bin/losos-hidden-tls";
          };
        };
        systemd.paths.losos-hidden-tls = {
          wantedBy = [ "paths.target" ];
          pathConfig.PathChanged = hidden.addressesFile;
        };
        # Traefik ends TLS with the certificate above, so it waits for it
        # (modules/apps.nix).
        systemd.services.losos-traefik = {
          after = [ "losos-hidden-tls.service" ];
          wants = [ "losos-hidden-tls.service" ];
        };
      }
    ]
  );
}
