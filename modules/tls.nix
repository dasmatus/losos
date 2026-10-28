# HTTPS on the appliance, from a certificate the box makes itself.
#
# Why self-signed, and why not ACME: this appliance answers at
# `<hostName>.local` on the LAN, and no certificate authority issues for a name
# in `.local` — there is nothing to prove ownership of. The master proxy does
# have a real certificate, but that terminates at the edge VPS and covers the
# tunnel, not the box, and the admin plane deliberately never travels through
# it (see the lanOnly guard in modules/containers.nix).
#
# Why two years: renewal here is a person walking to each of their devices and
# trusting a new certificate. Nothing on this box can run certbot and nothing
# can prompt them. A 90-day certificate would mean that chore four times a year,
# or an appliance that quietly stops answering on :443. Short lifetimes are good
# practice precisely because something automated renews them, and here nothing
# can.
#
# What this buys, beyond feeling tidier:
#
#   * A browser secure context, which WebAuthn requires outright ("available
#     only in secure contexts" — MDN). Passkeys are impossible without it.
#   * An end to two limitations docs/security-model.md currently accepts: the
#     admin token and every LosOS Cloud login crossing the LAN in clear text.
#
# What it does not buy: trust. Nothing believes this certificate until the owner
# installs it, and that step belongs to the first-run wizard. :80 therefore
# stays open and serving — an appliance that answered only on a port every
# browser warns about would be worse than one that answers on both.
{
  pkgs,
  lib,
  config,
  ...
}:

let
  cfg = config.losos.tls;
  hostName = config.losos.hostName;
  fqdn = "${hostName}.local";

  # Under /var, so impermanence keeps it across the tmpfs-root reboot. A
  # certificate regenerated on every boot would be a certificate the owner has
  # to re-trust on every boot.
  #
  # Its own directory, NOT /var/lib/losos/tls, and that is not a style choice.
  # modules/daemon.nix owns /var/lib/losos with StateDirectoryMode = "0700" so
  # that state.json and the rebuild log are root-only. NixOS runs nginx as the
  # `nginx` user — including the `nginx -t` config check in its ExecStartPre —
  # so a certificate under that directory is unreadable no matter what mode the
  # file itself carries. The failure is `[emerg] cannot load certificate ...
  # Permission denied` at every start, on a box with no shell to read it from.
  dir = "/var/lib/losos-tls";
  cert = "${dir}/cert.pem";
  key = "${dir}/key.pem";

  # The certificate has to be readable by whoever nginx runs as, which is an
  # option rather than a constant.
  nginxGroup = config.services.nginx.group;
in
{
  config = lib.mkIf cfg.enable {
    systemd.services.losos-tls-cert = {
      description = "Generate the appliance's self-signed TLS certificate";
      wantedBy = [ "multi-user.target" ];
      before = [ "nginx.service" ];
      # ConditionPathExists=! makes this a no-op on every boot after the first.
      # The certificate must be stable: the owner trusted *this* one on their
      # devices, and silently minting a new one would break every client with no
      # way to say why.
      unitConfig.ConditionPathExists = "!${cert}";
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
        UMask = "0077";
      };
      path = [
        pkgs.openssl
        pkgs.coreutils
      ];
      script = ''
        set -eu
        # 0750 root:${nginxGroup} — nginx must be able to traverse in, and
        # nobody else has any business here.
        install -d -m 0750 -o root -g ${nginxGroup} ${dir}

        # -addext subjectAltName is the load-bearing flag. Every browser
        # released this decade ignores the Common Name entirely and matches the
        # SAN, so a certificate with only a CN is one that fails to validate
        # while looking perfectly correct in the output of `openssl x509`.
        #
        # DNS:${fqdn} is the name the appliance is reached by. localhost and
        # 127.0.0.1 are there so anything on the box — a health check, a VM test
        # — can use the same certificate rather than needing -k.
        openssl req -x509 -newkey rsa:4096 -sha256 \
          -days ${toString cfg.validityDays} \
          -nodes \
          -keyout ${key} \
          -out ${cert} \
          -subj "/CN=${fqdn}/O=losos" \
          -addext "subjectAltName=DNS:${fqdn},DNS:${hostName},DNS:localhost,IP:127.0.0.1" \
          -addext "basicConstraints=critical,CA:TRUE" \
          -addext "keyUsage=critical,digitalSignature,keyCertSign"

        # 0640 root:${nginxGroup}, not 0600 root. NixOS does not run nginx as
        # root and drop privileges — the whole unit runs as the nginx user, with
        # CAP_NET_BIND_SERVICE for the low ports — so a root-only key is one
        # nginx cannot load.
        chown root:${nginxGroup} ${key} ${cert}
        chmod 0640 ${key}
        # The certificate is public by definition, and the wizard hands it to
        # the owner to trust.
        chmod 0644 ${cert}
      '';
    };

    # The same vhost gains a TLS listener rather than getting a second one of
    # its own. Everything that guards the admin surface — the lanOnly allow/deny
    # list, the CSP and X-Frame-Options maps, the /api proxy — is attached to
    # `losos-front` in modules/containers.nix, and a separate HTTPS vhost would
    # silently serve the admin plane without any of it.
    #
    # HTTP/3 rides on the same :443 listener, over QUIC on UDP. `quic` adds a
    # `listen 0.0.0.0:443 quic` beside the TCP one for every ssl entry below,
    # so TCP stays the fallback for every client that does not speak QUIC or
    # whose network drops UDP. `reuseport` gives each nginx worker its own UDP
    # socket, which is how nginx spreads QUIC across workers; without it every
    # QUIC packet lands on one worker. Listen options may be given once per
    # address, so this is the one vhost that sets any on these ports.
    #
    # Clients learn of HTTP/3 from Alt-Svc on a response over TCP, then switch.
    # The header is only sent over HTTPS: an http:// origin cannot be upgraded
    # this way, and traffic through the edge's tunnel arrives here on :80, so
    # the edge's own Alt-Svc is the one browsers see there.
    #
    # A client that will not use QUIC with a certificate from a root the
    # user installed (Chromium has refused that) simply stays on HTTP/2.
    # nix/tests/quic.nix fetches over both with curl and times them.
    services.nginx.virtualHosts."losos-front" = {
      addSSL = true;
      sslCertificate = cert;
      sslCertificateKey = key;
      quic = true;
      http3 = true;
      reuseport = true;
      # Restated whole, the loopback listeners of Tor and I2P included
      # (modules/containers.nix).
      listen = [
        {
          addr = "0.0.0.0";
          port = 80;
        }
        {
          addr = "0.0.0.0";
          port = 443;
          ssl = true;
        }
      ]
      ++ (import ./hidden-lib.nix { inherit lib pkgs; }).frontListen (
        config.losos.hidden.tor.enable || config.losos.hidden.i2p.enable
      );
      extraConfig = ''
        add_header Alt-Svc $losos_alt_svc always;
      '';
    };

    # Empty over plain HTTP, and add_header sends no header for an empty value.
    services.nginx.appendHttpConfig = ''
      map $https $losos_alt_svc {
        on 'h3=":443"; ma=86400';
        default "";
      }
    '';

    # nginx cannot start before the certificate exists, and on first boot the
    # oneshot above is what creates it.
    systemd.services.nginx = {
      after = [ "losos-tls-cert.service" ];
      requires = [ "losos-tls-cert.service" ];
    };

    networking.firewall.allowedTCPPorts = [ 443 ];
    networking.firewall.allowedUDPPorts = [ 443 ];
  };
}
