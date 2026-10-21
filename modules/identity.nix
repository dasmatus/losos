# Sign-in: LosOS Login, the box's own Authelia, in front of the admin pages.
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
# the box, and lososd owns the file. Authenticator apps and passkeys are
# Authelia's own (its settings page); lososd reads which ones each person
# has, and sets the rules an administrator chose (backend/src/login.rs).
#
# The session cookie needs a domain per name the box is opened by:
# <host>.local, each LAN address, and the public names an edge routes to the
# box (its tunnel name and the owner's own domains, which lososd writes to
# /var/lib/losos-public-names). Those change at runtime, so they are
# rendered into /run by a small unit, re-rendered by a timer and whenever
# the public names change, and Authelia is restarted when the set changes.
# Authelia wants https for all of it, so /auth/ redirects plain http to
# https, and the admin UI does the same before it sends anyone to sign in.
#
# LosOS Login answers from anywhere, the tunnel included, because LosOS
# cloud and LosOS Git have no sign-in page of their own any more: their
# sign-in pages always send people here. The edge ends TLS and sends the
# request on as http from 127.0.0.1, with `X-Forwarded-Proto: https`; that
# header is believed from loopback only (`$losos_proto`). The admin pages
# stay on the LAN (modules/lan-only.nix).
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
  inherit (cfg) port;
  user = "authelia-losos";
  dir = "/var/lib/losos-identity";
  peopleFile = "${dir}/users.yml";
  secretsDir = "/var/secrets/losos-identity";
  runDir = "/run/losos-identity";
  sessionFile = "${runDir}/session.yml";
  # Where LosOS Login keeps its sessions: a Redis of its own, on a socket
  # only Authelia's group opens. Authelia's in-memory store keys a session
  # by a string that shares bytes with the request buffer, so the next
  # request rewrites the key and someone else's sign-in silently ends.
  sessionRedis = "losos-login";
  sessionSocket = "/run/redis-${sessionRedis}/redis.sock";
  lanOnly = import ./lan-only.nix;
  # The owner's live custom domains, a JSON list lososd writes
  # (backend/src/io_backend.rs, DEFAULT_PUBLIC_NAMES_FILE).
  publicNamesFile = "/var/lib/losos-public-names/domains.json";
  # The name the edge routes to this box through the tunnel.
  tunnelName = lib.optionalString config.losos.proxy.enable config.losos.proxy.hostname;

  # What an app's sign-in page shows when LosOS Login cannot sign the person
  # in: while it starts (`waiting`, which asks again by itself), and for a
  # person a rule of LosOS Login holds to their own account (`held`). Plain
  # pages, so they work whatever is down.
  page = name: title: body: ''
    <!doctype html>
    <html lang="en">
    <head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    ${lib.optionalString (name == "waiting") ''<meta http-equiv="refresh" content="5">''}
    <title>${title}</title>
    <style>
    :root { color-scheme: light dark; --ground: #f3f5f7; --surface: #ffffff; --ink: #18212a; --muted: #5c6a77; --line: #dce1e6; --accent: #0e6e7d; }
    @media (prefers-color-scheme: dark) { :root { --ground: #0a0e12; --surface: #141b22; --ink: #e2e8ee; --muted: #94a2b0; --line: #212c36; --accent: #48b3c0; } }
    body { margin: 0; min-height: 100vh; display: grid; place-items: center; background: var(--ground); color: var(--ink); font: 15px/1.5 system-ui, sans-serif; }
    main { max-width: 30rem; margin: 1rem; padding: 1.5rem 1.75rem; background: var(--surface); border: 1px solid var(--line); border-radius: 12px; }
    h1 { font-size: 1.15rem; font-weight: 600; margin: 0 0 .5rem; }
    p { margin: .5rem 0; }
    p[lang] { color: var(--muted); font-size: 13.5px; }
    a { color: var(--accent); }
    </style>
    </head>
    <body><main>${body}</main></body>
    </html>
  '';
  signInPages = pkgs.runCommand "losos-sign-in-pages" { } ''
    mkdir -p $out
    cp ${
      pkgs.writeText "waiting.html" (
        page "waiting" "LosOS Login is starting" ''
          <h1>LosOS Login is starting</h1>
          <p>The box has just started. This page tries again by itself in a few seconds.</p>
          <p lang="sk">LosOS Login sa spúšťa. Stránka to o chvíľu skúsi znova sama.</p>
          <p lang="de">LosOS Login startet. Die Seite versucht es gleich von selbst erneut.</p>
        ''
      )
    } $out/waiting.html
    cp ${
      pkgs.writeText "held.html" (
        page "held" "Finish your account first" ''
          <h1>Finish your account first</h1>
          <p>This box asks something of your account before you can use LosOS cloud and LosOS Git: a second step at sign-in, or a new password. Open <a href="/settings/account">Your account</a> in the box's admin pages, on the box's own network.</p>
          <p lang="sk">Toto zariadenie od vášho účtu najprv niečo žiada: druhý krok pri prihlásení alebo nové heslo. Otvorte Váš účet na stránkach správy zariadenia, v jeho sieti.</p>
          <p lang="de">Diese Box verlangt zuerst etwas von deinem Konto: einen zweiten Schritt bei der Anmeldung oder ein neues Passwort. Öffne Dein Konto auf den Verwaltungsseiten der Box, in ihrem eigenen Netz.</p>
        ''
      )
    } $out/held.html
  '';
  # For the two apps: where their sign-in page sends a person back to, and
  # the person's account in them, the owner's own for the person who
  # claimed the box and their sign-in name for everyone else
  # (modules/app-sign-in.nix).
  appMaps = ''
    # Where a request comes from, for LosOS cloud's share below: the LAN,
    # or anywhere else (the tunnel arrives from 127.0.0.1). The ranges are
    # lanOnly's (modules/lan-only.nix).
    geo $losos_lan {
      default 0;
      10.42.0.0/16 0;
      10.0.0.0/8 1;
      172.16.0.0/12 1;
      192.168.0.0/16 1;
    }
    # LosOS cloud's share (`/internal/losos-cloud-share`): the client's
    # own credentials go on unless lososd named the person they are for.
    map $losos_user $losos_share_authorization {
      "" $http_authorization;
      default "";
    }
    map $uri $losos_app_back {
      ~^/git/ /git/;
      ~^/cloud/ $request_uri;
      default /nextcloud/login;
    }
    map "$losos_groups|$losos_user" $losos_app_user {
      "~(^|,)\s*${signIn.ownerGroup}\s*(,|\|)" ${signIn.ownerAccount};
      "~\|(?<losos_named>[a-z][a-z0-9._-]*)$" $losos_named;
      default "";
    }
  '';
  # Headers for those two pages: the strict policy a page with no script
  # needs, and no caching, since both change the moment LosOS Login does.
  # add_header here drops the server's own, so all of them are repeated.
  pageHeaders = ''
    add_header Content-Security-Policy "default-src 'none'; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'" always;
    add_header Cache-Control "no-store" always;
    add_header X-Content-Type-Options "nosniff" always;
    add_header X-Frame-Options "DENY" always;
    add_header Referrer-Policy "no-referrer" always;
  '';

  signIn = import ./app-sign-in.nix { inherit lib; };
  appSignIn = config.lososInternal.appSignIn;
  anyAppSignIn = appSignIn.nextcloud || appSignIn.git || appSignIn.cloud;
  nc = import ./nextcloud-stack.nix { inherit pkgs lib; };

  # Ask Authelia about the request nginx is handling, as the /api/
  # question below does.
  askAboutRequest = ''
    proxy_pass_request_body off;
    proxy_set_header Content-Length "";
    proxy_set_header X-Original-Method $request_method;
    proxy_set_header X-Original-URL https://$host$request_uri;
    proxy_set_header X-Forwarded-For $remote_addr;
  '';

  # Only these may open a connection to `port` on loopback: root, nginx and
  # the app's own uid. The same owner match as lososd's port
  # (modules/daemon.nix says why a match on OUTPUT and not a socket), here
  # because both apps take the person from a header: a Helm app on the host
  # network could otherwise name anyone it liked.
  loopbackRule =
    port: uid:
    lib.concatStringsSep " " (
      [
        "OUTPUT -o lo -p tcp -d 127.0.0.1 --dport ${toString port}"
        "-m owner ! --uid-owner 0"
        "-m owner ! --uid-owner ${toString uid}"
      ]
      ++ lib.optional config.services.nginx.enable "-m owner ! --uid-owner ${config.services.nginx.user}"
      ++ [ "-j REJECT --reject-with tcp-reset" ]
    );
  # LosOS Git needs none: it believes the headers only on its Unix socket,
  # which nginx alone may open (modules/git.nix).
  loopbackRules = lib.optional appSignIn.nextcloud (
    loopbackRule config.losos.nextcloud.apachePort nc.uid
  );

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
    proxy_set_header X-Losos-Via-Edge "";
    proxy_set_header X-Losos-Edge "";
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
      pkgs.gnugrep
      pkgs.iproute2
      pkgs.jq
      pkgs.systemd
    ];
    text = ''
      install -d -m 0750 -o root -g ${user} ${runDir}
      names=(${lib.escapeShellArg fqdn})
      while read -r addr; do
        names+=("''${addr%%/*}")
      done < <(ip -4 -o addr show scope global | awk '{print $4}')
      # The names an edge routes here. Plain host names only, whatever the
      # file says: each goes into YAML below.
      public=()
      for n in ${lib.escapeShellArg tunnelName} \
        $(jq -r '.[]? | strings' ${publicNamesFile} 2>/dev/null || true); do
        if [ -n "$n" ] && [ "''${#n}" -le 253 ] &&
          printf '%s' "$n" | grep -Eqx '[a-z0-9]([a-z0-9-]*[a-z0-9])?(\.[a-z0-9]([a-z0-9-]*[a-z0-9])?)+'; then
          public+=("$n")
        fi
      done
      tmp=$(mktemp ${runDir}/.session.XXXXXX)
      {
        echo "session:"
        echo "  cookies:"
        for n in "''${names[@]}"; do
          echo "    - domain: '$n'"
          echo "      authelia_url: 'https://$n/auth'"
          echo "      default_redirection_url: 'https://$n/'"
        done
        # The admin pages answer on the LAN only, so from outside a sign-in
        # with nowhere else to go lands in LosOS cloud.
        for n in $(printf '%s\n' "''${public[@]}" | sort -u); do
          echo "    - domain: '$n'"
          echo "      authelia_url: 'https://$n/auth'"
          echo "      default_redirection_url: 'https://$n/nextcloud/'"
        done
      } > "$tmp"
      chgrp ${user} "$tmp"
      chmod 0640 "$tmp"
      if cmp -s "$tmp" ${sessionFile}; then
        rm -f "$tmp"
      else
        mv "$tmp" ${sessionFile}
        echo "sign-in answers on: ''${names[*]}"
        # --no-block: Authelia starts after this unit, so a restart that
        # waited here would wait on itself, leaving Authelia stopped.
        systemctl --no-block try-restart authelia-losos.service
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
          # Look the person up on every request, not every five minutes: a
          # person removed on the Users pane loses LosOS cloud and LosOS Git
          # at their next click, and a changed group reaches nginx at once.
          # The file backend keeps everyone in memory, so it costs nothing.
          refresh_interval = "always";
          file = {
            path = peopleFile;
            watch = true;
          };
        };
        # Everyone with an account may pass; lososd decides what each person
        # may do (backend/sections.json). Two marks lososd keeps in the people
        # file come first (backend/src/login.rs): a person a rule holds to
        # their own account gets no LosOS cloud or LosOS Git, and a person
        # with an authenticator app or a passkey is asked for it.
        access_control = {
          default_policy = "deny";
          rules = [
            {
              domain_regex = ".*";
              subject = [ "group:losos-locked" ];
              resources = [ "^/(nextcloud|git|cloud)([/?].*)?$" ];
              policy = "deny";
            }
            {
              domain_regex = ".*";
              subject = [ "group:losos-two-factor" ];
              policy = "two_factor";
            }
            {
              domain_regex = ".*";
              policy = "one_factor";
            }
          ];
        };
        # Adding or removing a second factor asks for a one-time code, which
        # Authelia mails; the box has no mail, so the notifier below writes
        # it to a file and lososd hands it to the person behind their
        # password (POST /api/account/code). Always asked, a second factor
        # or not, so that code is the only way to change one.
        identity_validation.elevated_session = {
          require_second_factor = false;
          skip_second_factor = false;
        };
        totp.issuer = "LosOS";
        webauthn = {
          display_name = "LosOS Login";
          enable_passkey_login = true;
        };
        session = {
          name = "losos_session";
          expiration = "12h";
          inactivity = "2h";
          remember_me = "1M";
          redis.host = sessionSocket;
        };
        storage.local.path = "/var/lib/authelia-losos/db.sqlite3";
        notifier.filesystem.filename = "/var/lib/authelia-losos/notifications.txt";
      };
    };

    # Run as Authelia's group, because Authelia runs with PrivateUsers and
    # sees no other group: the socket is 0660 to it and to nobody else.
    services.redis.servers.${sessionRedis} = {
      enable = true;
      group = user;
      unixSocket = sessionSocket;
      unixSocketPerm = 660;
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

    # The public names change when the owner adds or drops a domain.
    systemd.paths.losos-identity-public-names = {
      description = "Re-render sign-in's session domains when the public names change";
      wantedBy = [ "multi-user.target" ];
      pathConfig = {
        PathChanged = publicNamesFile;
        Unit = "losos-identity-setup.service";
      };
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
      after = [
        "losos-identity-setup.service"
        "redis-${sessionRedis}.service"
      ];
      wants = [ "losos-identity-setup.service" ];
      requires = [ "redis-${sessionRedis}.service" ];
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
      LOSOS_AUTHELIA_DB = "/var/lib/authelia-losos/db.sqlite3";
      LOSOS_AUTHELIA_NOTES = "/var/lib/authelia-losos/notifications.txt";
    };

    # The scheme the person's browser used. The edge ends TLS and sends the
    # request through the tunnel as http from 127.0.0.1 with
    # `X-Forwarded-Proto: https`, which is believed from there only.
    services.nginx.appendHttpConfig = ''
      map "$remote_addr|$http_x_forwarded_proto" $losos_proto {
        "127.0.0.1|https" https;
        default $scheme;
      }
      ${lib.optionalString anyAppSignIn appMaps}
    '';

    networking.firewall = lib.mkIf (config.networking.firewall.enable && loopbackRules != [ ]) {
      extraCommands = lib.concatMapStrings (rule: ''
        iptables -w -D ${rule} 2>/dev/null || true
        iptables -w -I ${rule}
      '') loopbackRules;
      extraStopCommands = lib.concatMapStrings (rule: ''
        iptables -w -D ${rule} 2>/dev/null || true
      '') loopbackRules;
    };

    services.nginx.virtualHosts."losos-front".locations =
      lib.optionalAttrs anyAppSignIn {
        # Who is calling, for every request to LosOS Git (modules/containers.nix,
        # `signInMaybe`). Never a refusal: a person LosOS Login does not know,
        # a client with credentials of its own and a LosOS Login that is down
        # all go on as nobody.
        "= /internal/losos-app-authz" = {
          proxyPass = "http://127.0.0.1:${toString port}/auth/api/authz/auth-request";
          extraConfig = ''
            internal;
            if ($http_authorization) {
              return 200;
            }
            proxy_intercept_errors on;
            error_page 401 403 500 502 503 504 = @losos-app-nobody;
            ${askAboutRequest}
          '';
        };
        "@losos-app-nobody".extraConfig = "return 200;";
        # Whose request to LosOS cloud's share this is (backend/src/cloud_share.rs):
        # lososd checks a LosOS Login name and password in the Basic header
        # and names the person, or names nobody and the request goes on as it
        # came. Never a refusal, so app passwords and LosOS cloud's own
        # accounts keep working, off the LAN and while lososd is down too.
        "= /internal/losos-cloud-share" = {
          proxyPass = "http://127.0.0.1:${toString apiPort}/internal/cloud-share";
          extraConfig = ''
            internal;
            if ($losos_lan = 0) {
              return 200;
            }
            proxy_pass_request_body off;
            proxy_set_header Content-Length "";
            proxy_set_header X-Real-IP $remote_addr;
            proxy_set_header Remote-User "";
            proxy_set_header Remote-Groups "";
            proxy_set_header X-Losos-As "";
            proxy_intercept_errors on;
            error_page 500 502 503 504 = @losos-app-nobody;
          '';
        };
        # The apps' sign-in pages (modules/containers.nix, `signInPage`), from
        # the LAN and through the tunnel alike: 2xx signs the person in, 401
        # sends them to LosOS Login, 403 (a rule holds them to their own
        # account) shows `held`. LosOS Login down is nginx's own 500, which
        # shows `waiting`. Neither app shows a sign-in page of its own.
        "= /internal/losos-app-sign-in" = {
          proxyPass = "http://127.0.0.1:${toString port}/auth/api/authz/auth-request";
          extraConfig = ''
            internal;
            ${askAboutRequest}
          '';
        };
        # Back to the app's sign-in page once they are signed in, which then
        # goes on as above. https, because LosOS Login's cookie is Secure.
        "@losos-app-sign-in".extraConfig =
          "return 302 https://$host/auth/?rd=https://$host$losos_app_back;";
        "@losos-app-held" = {
          root = signInPages;
          extraConfig = ''
            ${pageHeaders}
            rewrite ^ /held.html break;
          '';
        };
        "@losos-app-waiting" = {
          root = signInPages;
          extraConfig = ''
            ${pageHeaders}
            add_header Retry-After "5" always;
            rewrite ^ /waiting.html break;
          '';
        };
      }
      // {
        # The portal: sign-in page, its API and assets, from anywhere (see
        # the header). https only, because Authelia's cookies are Secure and
        # it refuses an http address; behind the edge the browser's https
        # arrives as `$losos_proto`.
        "/auth/" = {
          proxyPass = "http://127.0.0.1:${toString port}";
          recommendedProxySettings = false;
          extraConfig = ''
            if ($losos_proto = http) {
              return 301 https://$host$request_uri;
            }
            proxy_set_header Host $host;
            proxy_set_header X-Real-IP $remote_addr;
            proxy_set_header X-Forwarded-For $remote_addr;
            proxy_set_header X-Forwarded-Proto $losos_proto;
            proxy_set_header X-Forwarded-Host $host;
          '';
        };
        # Whether the portal answers yet, over http too: the admin pages ask
        # before they send anyone to sign in, and wait with a spinner while
        # Authelia starts (nginx answers 502 until it does).
        "= /auth/api/health" = {
          proxyPass = "http://127.0.0.1:${toString port}/auth/api/health";
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
            # Authelia down (starting, restarting) is "not signed in", not a
            # 500: the admin pages then wait for LosOS Login with a spinner.
            proxy_intercept_errors on;
            error_page 500 502 503 504 = @losos-authz-down;
            ${askAboutRequest}
          '';
        };
        "@losos-authz-down".extraConfig = "return 401;";
        "/api/".extraConfig = askAuthelia;
        "= /api/vms/images".extraConfig = askAuthelia;
        "= /api/market/llm/chat".extraConfig = askAuthelia;
      }
      # The public routes, past Authelia. Exact matches beat the /api/ prefix.
      // lib.genAttrs (map (p: "= ${p}") publicPaths) (_: {
        proxyPass = "http://127.0.0.1:${toString apiPort}";
        extraConfig = toLososd;
      });
  };
}
