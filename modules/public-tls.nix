# The box's own certificate for its public names, so the edge carries TLS it
# cannot read.
#
# Behind an edge (modules/proxy.nix), a browser asks for
# https://<box>.losos.cfd at the edge. The edge used to end that TLS with a
# certificate of its own and send the request on through the tunnel as
# plain HTTP, so whoever ran the edge, or the machine under it, could read
# every sign-in, file and page. Now the box gets a Let's Encrypt
# certificate for each of its public names itself, and the edge routes them
# by the name in the ClientHello alone, passing the encrypted bytes through
# (edge/registrar/src/passthrough.rs has the conversation, and what it
# costs: no offline page, no HTTP/3 and no ECH at those names).
#
# The pieces, all on loopback:
#
#   * the tunnel delivers to nginx's stream module on `splitPort`, which
#     reads the first bytes: a TLS ClientHello goes to `tlsPort`, anything
#     else (the edge's plain HTTP: Let's Encrypt's HTTP-01, the edge's manage
#     calls, an older edge) to port 80 as before;
#   * `tlsPort` ends TLS with the certificate for the name asked for,
#     HTTP/1.1 only (there is no h2 to the vhost behind it), and hands the
#     request to the front vhost's `frontPort`, which LosOS Login takes as
#     https; loopback, so the LAN guard keeps the admin surface off it as it
#     keeps it off the tunnel's port 80;
#   * `losos-public-tls` (a timer, and a path unit on the names the edge
#     sends) gets and renews a certificate per name with lego over HTTP-01,
#     answered from `webroot` on port 80, writes the names it holds one for
#     to `readyFile`, which the heartbeat sends, and checks each name from
#     outside: a certificate there that this box did not make means someone
#     between the browser and the box is ending its TLS.
#
# The first certificate is got before the box says anything: the edge routes
# the HTTP-01 path of every box, and `readyFile` does not exist until one
# name is covered, so the edge keeps ending TLS until the box can.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.losos.proxy.tlsPassthrough;
  proxy = config.losos.proxy;
  on = proxy.enable && cfg.enable && config.services.nginx.enable;
  lib' = import ./public-tls-lib.nix { inherit lib; };
  inherit (lib')
    state
    namesFile
    readyFile
    probeFile
    webroot
    live
    ;
  nginxGroup = config.services.nginx.group;

  issue = pkgs.writeShellApplication {
    name = "losos-public-tls";
    runtimeInputs = [
      pkgs.coreutils
      pkgs.diffutils
      pkgs.gnugrep
      pkgs.jq
      pkgs.lego
      pkgs.openssl
    ];
    text = ''
      server=${lib.escapeShellArg cfg.acmeServer}
      install -d -m 0755 ${state} ${webroot} ${webroot}/.well-known ${webroot}/.well-known/acme-challenge
      install -d -m 0750 -g ${nginxGroup} ${live}

      # The box's own hostname, and every name the edge said it routes here.
      # Each lands in a path, so only DNS names pass.
      mapfile -t names < <(
        { printf '%s\n' ${lib.escapeShellArg proxy.hostname}; cat ${namesFile} 2>/dev/null || true; } |
          tr '[:upper:]' '[:lower:]' |
          grep -E '^([a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$' |
          sort -u | head -n 64 || true
      )

      # A certificate for $2 that is good for $3 more seconds.
      good() {
        [ -s "$1/fullchain.pem" ] && [ -s "$1/key.pem" ] &&
          openssl x509 -in "$1/fullchain.pem" -noout -checkend "$3" >/dev/null 2>&1 &&
          openssl x509 -in "$1/fullchain.pem" -noout -checkhost "$2" 2>/dev/null | grep -q 'does match'
      }

      ready=""
      changed=""
      for name in "''${names[@]}"; do
        dir=${live}/$name
        # Renewed with a month left, as Let's Encrypt asks.
        if ! good "$dir" "$name" $((30 * 86400)); then
          stamp=$(mktemp)
          if lego run --accept-tos --server "$server" ${
            lib.optionalString (cfg.acmeEmail != null) "--email ${lib.escapeShellArg cfg.acmeEmail}"
          } \
            --path ${state}/lego --http --http.webroot ${webroot} \
            --no-random-sleep --renew-force --domains "$name"; then
            # Where lego keeps them moved between releases, so find them.
            crt=$(find ${state}/lego -type f -name "$name.crt" -newer "$stamp" | head -n 1)
            key=$(find ${state}/lego -type f -name "$name.key" -newer "$stamp" | head -n 1)
            if [ -z "$crt" ] || [ -z "$key" ]; then
              echo "losos-public-tls: lego said yes for $name but left no new certificate" >&2
              rm -f "$stamp"
              continue
            fi
            install -d -m 0750 -g ${nginxGroup} "$dir"
            # Key first: nginx reads both on each handshake, and a new key
            # under the old certificate fails one handshake at worst.
            install -m 0640 -g ${nginxGroup} "$key" "$dir/key.pem.new"
            install -m 0644 "$crt" "$dir/fullchain.pem.new"
            mv "$dir/key.pem.new" "$dir/key.pem"
            mv "$dir/fullchain.pem.new" "$dir/fullchain.pem"
            echo "losos-public-tls: a certificate for $name, made on this box"
          else
            echo "losos-public-tls: no certificate for $name this time" >&2
          fi
          rm -f "$stamp"
        fi
        if good "$dir" "$name" 0; then
          ready="$ready$name"$'\n'
        fi
      done

      # The file the heartbeat reads. Never written empty the first time:
      # until one name is covered the edge keeps ending TLS. Once written,
      # it stays, even empty, so a lapse never hands TLS back to the edge.
      if [ -n "$ready" ] || [ -e ${readyFile} ]; then
        printf '%s' "$ready" > ${readyFile}.new
        if cmp -s ${readyFile}.new ${readyFile}; then
          rm -f ${readyFile}.new
        else
          mv ${readyFile}.new ${readyFile}
          changed=1
          echo "losos-public-tls: TLS ends on this box for: $(printf '%s' "$ready" | tr '\n' ' ')"
        fi
      fi

      # Look at each name from outside, as a browser would: through public
      # DNS, the edge and the tunnel. The same public key as the one here is
      # this box answering; anything else is someone in between ending TLS
      # with a certificate of their own.
      empty=$(printf "" | sha256sum | cut -d' ' -f1)
      look() {
        timeout 20 openssl s_client -connect "$1:443" -servername "$1" </dev/null 2>/dev/null |
          openssl x509 -noout -pubkey 2>/dev/null | sha256sum | cut -d' ' -f1 || true
      }
      results='{}'
      while IFS= read -r name; do
        [ -n "$name" ] || continue
        mine=$(openssl x509 -in "${live}/$name/fullchain.pem" -noout -pubkey | sha256sum | cut -d' ' -f1)
        seen=$(look "$name")
        # Just asked for passthrough: the edge ends TLS with its own
        # certificate until its next heartbeat and reconcile, so give it two
        # minutes before calling that someone else.
        tries=0
        while [ -n "$changed" ] && [ "$seen" != "$mine" ] && [ "$tries" -lt 24 ]; do
          sleep 5
          tries=$((tries + 1))
          seen=$(look "$name")
        done
        if [ "$seen" = "$mine" ]; then
          verdict=ok
        elif [ -z "$seen" ] || [ "$seen" = "$empty" ]; then
          verdict=unreachable
        else
          verdict=foreign
          echo "losos-public-tls: $name answered from outside with a certificate this box did not make" >&2
        fi
        results=$(jq -c --arg n "$name" --arg v "$verdict" '.[$n] = $v' <<<"$results")
      done <<<"$ready"
      jq -n --argjson names "$results" --arg at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
        '{checkedAt: $at, names: $names}' > ${probeFile}.new
      mv ${probeFile}.new ${probeFile}
    '';
  };
in
{
  config = lib.mkIf on {
    systemd.services.losos-public-tls = {
      description = "Get this box's own certificates for its public names";
      after = [
        "network-online.target"
        "nginx.service"
      ];
      wants = [ "network-online.target" ];
      serviceConfig = {
        Type = "oneshot";
        ExecStart = lib.getExe issue;
        PrivateTmp = true;
        NoNewPrivileges = true;
        ProtectSystem = "strict";
        ProtectHome = true;
        ReadWritePaths = [ state ];
        StateDirectory = "losos-public-tls";
        StateDirectoryMode = "0755";
      };
    };
    systemd.timers.losos-public-tls = {
      wantedBy = [ "timers.target" ];
      timerConfig = {
        OnBootSec = "1min";
        OnUnitActiveSec = "1h";
        RandomizedDelaySec = "10min";
      };
    };
    # A new name from the edge (a custom domain, a name in its zone) is
    # covered within seconds, not the hour.
    systemd.paths.losos-public-tls = {
      wantedBy = [ "paths.target" ];
      pathConfig.PathChanged = namesFile;
    };

    # Port 80 answers the HTTP-01 challenges from the webroot above. No LAN
    # guard: Let's Encrypt comes through the edge, from loopback.
    services.nginx.virtualHosts."losos-front".locations."^~ /.well-known/acme-challenge/" = {
      root = webroot;
      extraConfig = ''
        default_type text/plain;
      '';
    };

    services.nginx.streamConfig = ''
      # The tunnel's port (modules/public-tls.nix): TLS to this box's own
      # certificate, anything else to the front vhost's port 80.
      map $ssl_preread_protocol $losos_tunnel_next {
        "" 127.0.0.1:80;
        default 127.0.0.1:${toString lib'.tlsPort};
      }
      server {
        listen 127.0.0.1:${toString lib'.splitPort};
        ssl_preread on;
        proxy_pass $losos_tunnel_next;
      }
      # Only DNS names reach a path; anything else asks for a file that is
      # never there, and the handshake fails.
      map $ssl_server_name $losos_public_name {
        "~^([a-z0-9-]+\.)+[a-z0-9-]+$" $ssl_server_name;
        default none.invalid;
      }
      server {
        listen 127.0.0.1:${toString lib'.tlsPort} ssl;
        ssl_certificate ${live}/$losos_public_name/fullchain.pem;
        ssl_certificate_key ${live}/$losos_public_name/key.pem;
        ssl_protocols TLSv1.2 TLSv1.3;
        ssl_alpn http/1.1;
        proxy_pass 127.0.0.1:${toString lib'.frontPort};
      }
    '';
  };
}
