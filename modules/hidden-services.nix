# The box on Tor and I2P (losos.hidden.tor.enable, losos.hidden.i2p.enable).
#
# With either switch on, the box has an address on that network: a `.onion`
# name, a `.b32.i2p` name. Each reaches what the box's public name reaches
# through an edge, LosOS Cloud, LosOS Git and LosOS Login, and never the
# admin pages, with no edge and no open port. The daemons and the list they
# serve are modules/hidden-lib.nix; this file is what the box adds.
#
# Where a visitor lands. Port 80 of the address goes to 127.0.0.1:8480 and
# port 443 to 127.0.0.1:8443, both on loopback only, so nothing on the LAN
# reaches them:
#
#   * 8480 is a listener of the front vhost (modules/containers.nix), plain
#     http. Never port 80 itself: the edge's manage server
#     (modules/proxy.nix) listens there for loopback, and a Tor client sends
#     whatever Host header it likes.
#   * 8443 is nginx's stream module ending TLS with the certificate below
#     and passing the bytes to 127.0.0.1:8481, the vhost's second loopback
#     listener, which LosOS Login takes as https (modules/identity.nix,
#     `$losos_proto`). Authelia refuses an http address and sets Secure
#     cookies, so signing in needs this half.
#
# Both arrive from 127.0.0.1, which the admin guard turns away
# (modules/lan-only.nix: loopback is no LAN source, and the source is the
# listener's own address), exactly as it turns away the edge's tunnel.
#
# The certificate names the box's addresses and is signed by the box's own
# certificate (modules/tls.nix, a CA the owner trusted on their devices in
# the first-run wizard), so a device that trusts the box trusts its
# addresses too. Any other visitor accepts it once, as with any onion site
# that has no certificate from a public authority. It is made again when
# the set of addresses changes, and nginx reloads.
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
        http = hidden.plainPort;
        https = hidden.streamPort;
      }
    ]
  );
  nginxGroup = config.services.nginx.group;
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
      install -d -m 0750 -o root -g ${nginxGroup} ${tlsDir}
      mapfile -t names < <(jq -r '[.[] | .tor, .i2p | strings] | sort | unique | .[]' \
        ${hidden.addressesFile} 2>/dev/null || true)
      want=$(printf '%s\n' "''${names[@]}")
      if [ -s ${tlsDir}/cert.pem ] && [ "$want" = "$(cat ${tlsDir}/names 2>/dev/null)" ]; then
        exit 0
      fi
      # Before Tor or I2P have made an address nginx still needs a
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
      chown root:${nginxGroup} "$work/key.pem" "$work/cert.pem"
      chmod 0640 "$work/key.pem"
      chmod 0644 "$work/cert.pem"
      mv "$work/key.pem" ${tlsDir}/key.pem
      mv "$work/cert.pem" ${tlsDir}/cert.pem
      printf '%s' "$want" > ${tlsDir}/names
      echo "certificate for: ''${want:-no address yet}"
      if systemctl -q is-active nginx.service; then
        systemctl reload nginx.service || true
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
          before = [ "nginx.service" ];
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
        systemd.services.nginx = {
          after = [ "losos-hidden-tls.service" ];
          wants = [ "losos-hidden-tls.service" ];
        };

        services.nginx.streamConfig = ''
          # The https of this box's Tor and I2P addresses
          # (modules/hidden-services.nix): TLS here, then the front vhost's
          # loopback listener that LosOS Login takes as https.
          server {
            listen 127.0.0.1:${toString hidden.streamPort} ssl;
            ssl_certificate ${tlsDir}/cert.pem;
            ssl_certificate_key ${tlsDir}/key.pem;
            proxy_pass 127.0.0.1:${toString hidden.tlsPort};
          }
        '';
      }
    ]
  );
}
