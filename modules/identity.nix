# Sign-in: Authelia in front of the admin pages.
#
# People sign in once, at https://<host>/auth/, with a name and a password,
# and Authelia's session cookie opens every admin route after that. nginx
# asks Authelia about each /api/ request (auth_request) and hands lososd the
# answer as `Remote-User`; lososd looks the person up and lets them through
# to the sections their groups open (backend/sections.json,
# backend/src/sections.rs). The first person, made by the first-run claim,
# is in losos-admins.
#
# Three things keep this from locking the owner out of a box with no shell:
#
#   * The spare key still works. A request carrying `Authorization: Bearer`
#     skips Authelia (the `if` in the authz location below) and lososd checks
#     the token itself, so the printed key opens the box while Authelia is
#     down, misconfigured or not yet started.
#   * The public routes (the claim, health, the terminal's two) never ask
#     Authelia at all. They are the `public` entries of sections.json, read
#     here, so nginx and lososd cannot disagree on them.
#   * Authelia starts only once the people file exists. Its file backend
#     refuses to start with nobody in it, and without the file it would
#     write an example account of its own. lososd writes the file at the
#     claim, and the path unit below starts Authelia when it appears.
#
# lososd is the only writer of the people file (backend/src/people.rs):
# argon2id hashes in Authelia's format, JSON (which is YAML), root-owned and
# readable by Authelia's group. Authelia watches it and reloads on change.
# Password change and reset inside Authelia are off: there is no mail on
# the box, and lososd owns the file.
#
# The session cookie needs a domain per name the box is opened by:
# <host>.local and each LAN address. Those change at runtime, so they are
# rendered into /run by a small unit and re-rendered by a timer, which
# restarts Authelia when the set changes. Authelia wants https for all of
# it, so /auth/ redirects plain http to https, and the admin UI does the
# same before it sends anyone to sign in.
#
# The portal is Authelia's own web app built with the LosOS palette, logo
# and name (admin-ui/themes/default.nix, `authelia`).
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.losos.identity;
  themes = import ../admin-ui/themes { inherit pkgs; };
  sections = lib.importJSON ../backend/sections.json;
  # The paths of the public routes, each once.
  publicPaths = lib.unique (
    lib.mapAttrsToList (route: _: lib.last (lib.splitString " " route)) (
      lib.filterAttrs (_: section: section == "public") sections.routes
    )
  );

  enabled = cfg.enable && config.losos.admin.enable && config.losos.admin.ui != null;
  fqdn = "${config.losos.hostName}.local";
  apiPort = config.losos.admin.apiPort;
  port = cfg.port;
  user = "authelia-losos";
  dir = "/var/lib/losos-identity";
  peopleFile = "${dir}/users.yml";
  secretsDir = "/var/secrets/losos-identity";
  runDir = "/run/losos-identity";
  sessionFile = "${runDir}/session.yml";
  lanOnly = import ./lan-only.nix;

  package = pkgs.authelia.override {
    authelia-web = themes.authelia.web pkgs.authelia.web;
  };

  # Who nginx says is calling, on every location that proxies to lososd.
  # Empty when Authelia was skipped (the spare key) or not asked (a public
  # route), and nginx sends no header for an empty value, so whatever the
  # client put in either header never reaches lososd.
  passPerson = ''
    proxy_set_header Remote-User $losos_user;
    proxy_set_header Remote-Groups $losos_groups;
    proxy_set_header X-Losos-As "";
  '';
  askAuthelia = ''
    auth_request /internal/losos-authz;
    auth_request_set $losos_user $upstream_http_remote_user;
    auth_request_set $losos_groups $upstream_http_remote_groups;
    ${passPerson}
  '';
  toLososd = ''
    ${lanOnly}
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header Remote-User "";
    proxy_set_header Remote-Groups "";
    proxy_set_header X-Losos-As "";
    # The claim's first occ run outlives nginx's 60 s under emulation, and
    # a terminal call waits for its inner call (modules/containers.nix).
    proxy_read_timeout 600s;
    proxy_send_timeout 600s;
  '';

  renderSession = pkgs.writeShellApplication {
    name = "losos-identity-session";
    runtimeInputs = [
      pkgs.coreutils
      pkgs.diffutils
      pkgs.gawk
      pkgs.iproute2
      pkgs.systemd
    ];
    text = ''
      install -d -m 0750 -o root -g ${user} ${runDir}
      names=(${lib.escapeShellArg fqdn})
      while read -r addr; do
        names+=("''${addr%%/*}")
      done < <(ip -4 -o addr show scope global | awk '{print $4}')
      tmp=$(mktemp ${runDir}/.session.XXXXXX)
      {
        echo "session:"
        echo "  cookies:"
        for n in "''${names[@]}"; do
          echo "    - domain: '$n'"
          echo "      authelia_url: 'https://$n/auth'"
          echo "      default_redirection_url: 'https://$n/'"
        done
      } > "$tmp"
      chgrp ${user} "$tmp"
      chmod 0640 "$tmp"
      if cmp -s "$tmp" ${sessionFile}; then
        rm -f "$tmp"
      else
        mv "$tmp" ${sessionFile}
        echo "sign-in answers on: ''${names[*]}"
        systemctl try-restart authelia-losos.service
      fi
    '';
  };
in
{
  config = lib.mkIf enabled {
    services.authelia.instances.losos = {
      enable = true;
      inherit package;
      secrets = {
        jwtSecretFile = "${secretsDir}/jwt";
        sessionSecretFile = "${secretsDir}/session";
        storageEncryptionKeyFile = "${secretsDir}/storage";
      };
      settingsFiles = [ sessionFile ];
      settings = {
        theme = "auto";
        server = {
          address = "tcp://127.0.0.1:${toString port}/auth";
          asset_path = themes.authelia.assets;
        };
        log.level = "info";
        # The box may have no internet, and the check only warns.
        ntp.disable_startup_check = true;
        authentication_backend = {
          password_reset.disable = true;
          password_change.disable = true;
          file = {
            path = peopleFile;
            watch = true;
          };
        };
        # Everyone with an account may pass; lososd decides what each person
        # may do (backend/sections.json).
        access_control = {
          default_policy = "deny";
          rules = [
            {
              domain_regex = ".*";
              policy = "one_factor";
            }
          ];
        };
        session = {
          name = "losos_session";
          expiration = "12h";
          inactivity = "2h";
          remember_me = "1M";
        };
        storage.local.path = "/var/lib/authelia-losos/db.sqlite3";
        notifier.filesystem.filename = "/var/lib/authelia-losos/notifications.txt";
      };
    };

    systemd.services.losos-identity-setup = {
      description = "Prepare sign-in: Authelia's secrets and session domains";
      wantedBy = [ "multi-user.target" ];
      before = [ "authelia-losos.service" ];
      after = [ "network.target" ];
      serviceConfig = {
        Type = "oneshot";
        UMask = "0077";
      };
      path = [
        pkgs.coreutils
        pkgs.openssl
      ];
      script = ''
        install -d -m 0700 ${secretsDir}
        for s in jwt session storage; do
          if [ ! -s ${secretsDir}/$s ]; then
            openssl rand -hex 64 > ${secretsDir}/$s.tmp
            mv ${secretsDir}/$s.tmp ${secretsDir}/$s
          fi
        done
        # lososd writes the people file here; Authelia reads it.
        install -d -m 0750 -o root -g ${user} ${dir}
        ${lib.getExe renderSession}
      '';
    };

    # The LAN addresses change at runtime; follow them.
    systemd.timers.losos-identity-setup = {
      wantedBy = [ "timers.target" ];
      timerConfig = {
        OnUnitActiveSec = "2min";
        OnBootSec = "2min";
      };
    };

    systemd.services.authelia-losos = {
      after = [ "losos-identity-setup.service" ];
      wants = [ "losos-identity-setup.service" ];
      # Not before the claim: see the header.
      unitConfig.ConditionPathExists = peopleFile;
    };
    systemd.paths.losos-identity = {
      description = "Start sign-in once the box has its first person";
      wantedBy = [ "multi-user.target" ];
      pathConfig = {
        PathExists = peopleFile;
        Unit = "authelia-losos.service";
      };
    };

    systemd.services.lososd.environment = {
      LOSOS_PEOPLE_FILE = peopleFile;
      LOSOS_PEOPLE_GROUP = user;
      # lososd believes `Remote-User` only when this is set, so a box
      # without this module ignores the header whoever sends it.
      LOSOS_IDENTITY = "authelia";
    };

    services.nginx.virtualHosts."losos-front".locations = {
      # The portal: sign-in page, its API and assets. https only, because
      # Authelia's cookies are Secure and it refuses an http address.
      "/auth/" = {
        proxyPass = "http://127.0.0.1:${toString port}";
        extraConfig = ''
          ${lanOnly}
          if ($scheme = http) {
            return 301 https://$host$request_uri;
          }
        '';
      };
      # The question nginx asks before every /api/ request. A Bearer
      # header is the spare key or a token lososd checks itself, so it
      # skips Authelia and the box stays reachable when Authelia is down.
      "= /internal/losos-authz" = {
        proxyPass = "http://127.0.0.1:${toString port}/auth/api/authz/auth-request";
        extraConfig = ''
          internal;
          if ($http_authorization ~* "^bearer ") {
            return 200;
          }
          proxy_pass_request_body off;
          proxy_set_header Content-Length "";
          proxy_set_header X-Original-Method $request_method;
          proxy_set_header X-Original-URL https://$host$request_uri;
          proxy_set_header X-Forwarded-For $remote_addr;
        '';
      };
      "/api/".extraConfig = askAuthelia;
      "= /api/vms/images".extraConfig = askAuthelia;
    }
    # The public routes, past Authelia. Exact matches beat the /api/ prefix.
    // lib.genAttrs (map (p: "= ${p}") publicPaths) (_: {
      proxyPass = "http://127.0.0.1:${toString apiPort}";
      extraConfig = toLososd;
    });
  };
}
