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
#
# LosOS Login is also the OAuth provider of the LosOS companion
# in people's AI tools (`losos agents install`, backend/src/companion.rs).
# It signs in with the device code flow: the terminal shows a link and a
# code, the person approves it here with their password, their second
# factor when they have one, and a consent page, and the companion gets an
# access token for an hour and a refresh token for 30 days. Its calls come
# as `Authorization: Bearer authelia_at_…`, which nginx passes on like the
# spare key, and lososd asks LosOS Login about each token (introspection,
# as the confidential client `lososd`) and lets it read a few routes as
# that person. LosOS Desktop's backups sign in the same way as a second
# client, `losos-backups`, with a ten-minute token and no refresh, good
# for one route that makes that person's own LosOS Cloud app password and
# LosOS Git token (`POST /api/devices/backups`). The keys and both secrets
# are made here on first start.
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
  allPublic = lib.unique (
    lib.mapAttrsToList (route: _: lib.last (lib.splitString " " route)) (
      lib.filterAttrs (_: section: section == "public") sections.routes
    )
  );
  # A computer whose logins this box manages checks in here
  # (backend/src/schizo.rs): from the office, from home through the edge,
  # and over the box's Tor address once schizo mode sends everything through
  # Tor. So these are the public routes the LAN guard does not hold; the
  # computer's own token is the whole check, and lososd throttles a wrong one.
  devicePaths = lib.filter (lib.hasPrefix "/api/login-devices/self") allPublic;
  publicPaths = lib.subtractLists devicePaths allPublic;

  enabled = cfg.enable && config.losos.admin.enable && config.losos.admin.ui != null;
  # The terminal's two routes, when the tunnel may reach them, are
  # modules/containers.nix's, without the LAN guard.
  throughEdge = [
    "/api/access/host-key"
    "/api/access/call"
  ];
  lanPublicPaths =
    if config.losos.proxy.enable && config.losos.access.throughEdge then
      lib.subtractLists throughEdge publicPaths
    else
      publicPaths;
  fqdn = "${config.losos.hostName}.local";
  apiPort = config.losos.admin.apiPort;
  inherit (cfg) port;
  user = "authelia-losos";
  dir = "/var/lib/losos-identity";
  peopleFile = "${dir}/users.yml";
  # Where an older build kept Authelia's secrets as plaintext; the
  # credential store adopts them from here (modules/credentials.nix). Only
  # the digest of lososd's client secret stays: it is a hash, not a secret.
  secretsDir = "/var/secrets/losos-identity";
  runDir = "/run/losos-identity";
  sessionFile = "${runDir}/session.yml";
  # lososd's own OAuth client secret, and the hash LosOS Login checks it by.
  lososdSecret = "${secretsDir}/oidc-lososd";
  lososdDigest = "${secretsDir}/oidc-lososd.digest";
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
  # The box's own Tor and I2P addresses, and those an edge made for it
  # (modules/hidden-lib.nix).
  hidden = import ./hidden-lib.nix { inherit lib pkgs; };
  # The box's own TLS for its public names (modules/public-tls.nix).
  publicTls = import ./public-tls-lib.nix { inherit lib; };

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
          <p>This box asks something of your account before you can use LosOS Cloud and LosOS Git: a second step at sign-in, or a new password. Open <a href="/settings/account">Your account</a> in the box's admin pages, on the box's own network.</p>
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
    # Where a request comes from, for LosOS Cloud's share below: the LAN,
    # or anywhere else (the tunnel arrives from 127.0.0.1). The ranges are
    # lanOnly's (modules/lan-only.nix).
    geo $losos_lan {
      default 0;
      10.42.0.0/16 0;
      10.0.0.0/8 1;
      172.16.0.0/12 1;
      192.168.0.0/16 1;
    }
    # LosOS Cloud's share (`/internal/losos-cloud-share`): the client's
    # own credentials go on unless lososd named the person they are for.
    map $losos_user $losos_share_authorization {
      "" $http_authorization;
      default "";
    }
    map $uri $losos_app_back {
      ~^/git/ /git/;
      default $request_uri;
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
  anyAppSignIn = appSignIn.git || appSignIn.cloud;

  # Ask Authelia about the request nginx is handling, as the /api/
  # question below does.
  # Every one of these asks is a subrequest of the request it is about, and
  # nginx holds that request's Content-Length to the asking location's
  # client_max_body_size (the server's 10m unless set) before anything else
  # runs there. The body never comes here, so no limit: with the server's,
  # anything over 10 MB (an ISO copied into LosOS Cloud, a video uploaded,
  # a push) failed the ask with 413, and the request itself with 500.
  askAboutRequest = ''
    client_max_body_size 0;
    proxy_pass_request_body off;
    proxy_set_header Content-Length "";
    proxy_set_header X-Original-Method $request_method;
    proxy_set_header X-Original-URL https://$host$request_uri;
    proxy_set_header X-Forwarded-For $remote_addr;
  '';

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
  basicHttps = ''
    if ($losos_proto = http) {
      return 301 https://$host$request_uri;
    }
  '';
  askAuthelia = ''
    auth_request /internal/losos-authz;
    auth_request_set $losos_user $upstream_http_remote_user;
    auth_request_set $losos_groups $upstream_http_remote_groups;
    ${passPerson}
  '';
  toLososd = ''
    ${lanOnly}
    ${toLososdAnywhere}
  '';
  toLososdAnywhere = ''
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
        $(jq -r '.[]? | strings' ${publicNamesFile} 2>/dev/null || true) \
        $(jq -r '.[]? | .tor, .i2p | strings' ${hidden.addressesFile} 2>/dev/null || true) \
        $(jq -r '.tor, .i2p | strings' ${hidden.edgeFile} 2>/dev/null || true); do
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
        # with nowhere else to go lands in LosOS Cloud.
        for n in $(printf '%s\n' "''${public[@]}" | sort -u); do
          echo "    - domain: '$n'"
          echo "      authelia_url: 'https://$n/auth'"
          echo "      default_redirection_url: 'https://$n/cloud/'"
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
  # The credential store every secret here lives in.
  imports = [ ./credentials.nix ];

  config = lib.mkIf enabled {
    services.authelia.instances.losos = {
      enable = true;
      inherit package;
      # Every secret is a credential (below): systemd opens them into the
      # unit's credential directory, and Authelia reads them from there
      # through its `_FILE` variables and, for the signing key, a template.
      secrets.manual = true;
      environmentVariables = {
        AUTHELIA_IDENTITY_VALIDATION_RESET_PASSWORD_JWT_SECRET_FILE = "%d/authelia-jwt";
        AUTHELIA_SESSION_SECRET_FILE = "%d/authelia-session";
        AUTHELIA_STORAGE_ENCRYPTION_KEY_FILE = "%d/authelia-storage";
        AUTHELIA_IDENTITY_PROVIDERS_OIDC_HMAC_SECRET_FILE = "%d/authelia-oidc-hmac";
        X_AUTHELIA_CONFIG_FILTERS = "template";
      };
      settingsFiles = [
        sessionFile
        # The signing key, from the credential directory. secrets.manual
        # drops the module's own copy of this file.
        (pkgs.writeText "oidc-jwks.yaml" ''
          identity_providers:
            oidc:
              jwks:
                - key: {{ mustEnv "CREDENTIALS_DIRECTORY" | printf "%s/authelia-oidc-jwks" | secret | mindent 10 "|" | msquote }}
        '')
      ];
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
          # person removed on the Users pane loses LosOS Cloud and LosOS Git
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
        # their own account gets no LosOS Cloud or LosOS Git, and a person
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
        # The LosOS companion (see the header). The client ids are
        # losos_access::COMPANION_CLIENT_ID and backend/src/companion.rs.
        identity_providers.oidc = {
          lifespans.custom.companion = {
            access_token = "1h";
            refresh_token = "30d";
          };
          # A computer's backups sign in once, fetch their credentials
          # (POST /api/devices/backups) and keep nothing of LosOS Login's.
          lifespans.custom.backups.access_token = "10m";
          # The same two marks as access_control above: a person held to
          # their own account gets no companion, and a person with a second
          # factor approves it with that factor.
          authorization_policies.losos = {
            default_policy = "one_factor";
            rules = [
              {
                policy = "deny";
                subject = "group:losos-locked";
              }
              {
                policy = "two_factor";
                subject = "group:losos-two-factor";
              }
            ];
          };
          clients = [
            {
              client_id = "losos-companion";
              client_name = "LosOS companion";
              public = true;
              token_endpoint_auth_method = "none";
              authorization_policy = "losos";
              lifespan = "companion";
              consent_mode = "explicit";
              grant_types = [
                "urn:ietf:params:oauth:grant-type:device_code"
                "refresh_token"
              ];
              response_types = [ ];
              redirect_uris = [ ];
              scopes = [
                "openid"
                "profile"
                "offline_access"
              ];
            }
            {
              # LosOS Desktop's backups (losos_access::BACKUPS_CLIENT_ID): the
              # same device code sign-in as the companion, for one call that
              # makes the person's own LosOS Cloud and LosOS Git credentials.
              client_id = "losos-backups";
              client_name = "LosOS Desktop backups";
              public = true;
              token_endpoint_auth_method = "none";
              authorization_policy = "losos";
              lifespan = "backups";
              consent_mode = "explicit";
              grant_types = [ "urn:ietf:params:oauth:grant-type:device_code" ];
              response_types = [ ];
              redirect_uris = [ ];
              scopes = [
                "openid"
                "profile"
              ];
            }
            {
              # Introspection only: lososd asks about the companion's and the
              # backups' tokens.
              client_id = "lososd";
              client_name = "lososd";
              client_secret = ''{{ mustEnv "CREDENTIALS_DIRECTORY" | printf "%s/oidc-lososd-digest" | secret }}'';
              public = false;
              token_endpoint_auth_method = "client_secret_basic";
              introspection_endpoint_auth_method = "client_secret_basic";
              authorization_policy = "one_factor";
              grant_types = [ "client_credentials" ];
              response_types = [ ];
              redirect_uris = [ ];
              scopes = [ ];
            }
          ];
        };
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
      description = "Prepare sign-in: the session domains";
      wantedBy = [ "multi-user.target" ];
      # lososd too: it reads the client secret this unit may make again.
      before = [
        "authelia-losos.service"
        "lososd.service"
      ];
      after = [
        "network.target"
        "losos-credentials.service"
      ];
      wants = [ "losos-credentials.service" ];
      serviceConfig = {
        Type = "oneshot";
        UMask = "0077";
      };
      path = [
        pkgs.coreutils
        pkgs.gnused
        package
      ];
      script = ''
        install -d -m 0700 ${secretsDir}
        # lososd's client secret is a credential, the digest LosOS Login
        # checks it by a file beside the old secrets. They are made together,
        # so the secret is never on a command line. Whenever the sealed
        # secret is missing or newer than the digest (the store made or
        # adopted it at boot), both are made again here and the secret is
        # sealed in place; lososd, ordered after this unit, reads the new one.
        blob=/etc/credstore.encrypted/oidc-lososd
        if [ ! -s ${lososdDigest} ] || [ ! -s "$blob" ] || [ "$blob" -nt ${lososdDigest} ]; then
          made=$(authelia crypto hash generate pbkdf2 --variant sha512 \
            --random --random.length 64)
          printf '%s' "$made" | sed -n 's/^Random Password: //p' | tr -d '\n' \
            | systemd-creds --no-ask-password --with-key="$(cat /run/losos-credentials/key)" \
              --tpm2-pcrs= --name=oidc-lososd encrypt - "$blob.new"
          printf '%s' "$made" | sed -n 's/^Digest: //p' | tr -d '\n' > ${lososdDigest}.tmp
          [ -s "$blob.new" ] && [ -s ${lososdDigest}.tmp ]
          chmod 0600 "$blob.new"
          mv -f "$blob.new" "$blob"
          mv -f ${lososdDigest}.tmp ${lososdDigest}
          if systemctl -q is-active lososd.service; then
            systemctl --no-block try-restart lososd.service authelia-losos.service
          fi
        fi
        # lososd writes the people file here; Authelia reads it.
        install -d -m 0750 -o root -g ${user} ${dir}
        ${lib.getExe renderSession}
      '';
    };

    # The public names change when the owner adds or drops a domain, and
    # when Tor, I2P or an edge make the box an address.
    systemd.paths.losos-identity-public-names = {
      description = "Re-render sign-in's session domains when the public names change";
      wantedBy = [ "multi-user.target" ];
      pathConfig = {
        PathChanged = [
          publicNamesFile
          hidden.addressesFile
          hidden.edgeFile
        ];
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

    losos.credentials.entries = {
      authelia-jwt = {
        app = "login";
        description = "Signs the links LosOS Login sends to confirm who someone is.";
        format = "hex-64";
        services = [ "authelia-losos" ];
        rotate = true;
        legacy = [ "${secretsDir}/jwt" ];
      };
      authelia-session = {
        app = "login";
        description = "Protects the sessions of LosOS Login. A new one signs everyone out.";
        format = "hex-64";
        services = [ "authelia-losos" ];
        rotate = true;
        legacy = [ "${secretsDir}/session" ];
      };
      # Not rotatable from the pane: Authelia encrypts its database with it,
      # and a new key without `authelia storage encryption change-key` is a
      # database it can no longer open.
      authelia-storage = {
        app = "login";
        description = "Encrypts what LosOS Login stores: second factors and passkeys.";
        format = "hex-64";
        services = [ "authelia-losos" ];
        legacy = [ "${secretsDir}/storage" ];
      };
      # Signs the companion's tokens; a new one would sign every companion
      # out, so the pane leaves both alone.
      authelia-oidc-hmac = {
        app = "login";
        description = "Signs the tokens LosOS Login gives the AI agent companion.";
        format = "hex-64";
        services = [ "authelia-losos" ];
        legacy = [ "${secretsDir}/oidc-hmac" ];
      };
      authelia-oidc-jwks = {
        app = "login";
        description = "The private key LosOS Login signs its identity tokens with.";
        format = "rsa-2048";
        services = [ "authelia-losos" ];
        legacy = [ "${secretsDir}/oidc-jwks.pem" ];
      };
      # Made again with its digest by losos-identity-setup above.
      oidc-lososd = {
        app = "login";
        description = "lososd's own secret for asking LosOS Login about a companion token.";
        format = "hex-64";
        services = [ "lososd" ];
        legacy = [ lososdSecret ];
      };
    };

    systemd.services.authelia-losos = {
      after = [
        "losos-identity-setup.service"
        "redis-${sessionRedis}.service"
      ];
      wants = [ "losos-identity-setup.service" ];
      requires = [ "redis-${sessionRedis}.service" ];
      serviceConfig.LoadCredential = [ "oidc-lososd-digest:${lososdDigest}" ];
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
      # How lososd asks LosOS Login about a companion token.
      LOSOS_LOGIN_ADDR = "127.0.0.1:${toString port}";
      # Where the pages without JavaScript (backend/plugins/basic/) sign
      # people in: LosOS Login's own API, as its sign-in page would.
      LOSOS_LOGIN_URL = "http://127.0.0.1:${toString port}";
      LOSOS_OIDC_SECRET_FILE = "%d/oidc-lososd";
    };

    # The scheme the person's browser used. The edge ends TLS and sends the
    # request through the tunnel as http from 127.0.0.1 with
    # `X-Forwarded-Proto: https`, which is believed from there only. The
    # box's Tor and I2P addresses (modules/hidden-services.nix) arrive on
    # loopback listeners of their own, one behind nginx's own TLS and one
    # without, so for them the port decides and no header does.
    services.nginx.appendHttpConfig = ''
      map "$server_port|$remote_addr|$http_x_forwarded_proto" $losos_proto {
        "~^${toString hidden.tlsPort}\|" https;
        "~^${toString publicTls.frontPort}\|" https;
        "~^${toString hidden.plainPort}\|" http;
        "~^[0-9]+\|127\.0\.0\.1\|https$" https;
        default $scheme;
      }
      ${lib.optionalString anyAppSignIn appMaps}
    '';

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
        # Whose request to LosOS Cloud's share this is (backend/src/cloud_share.rs):
        # lososd checks a LosOS Login name and password in the Basic header
        # and names the person, or names nobody and the request goes on as it
        # came. Never a refusal, so app passwords and LosOS Cloud's own
        # accounts keep working, off the LAN and while lososd is down too.
        "= /internal/losos-cloud-share" = {
          proxyPass = "http://127.0.0.1:${toString apiPort}/internal/cloud-share";
          extraConfig = ''
            internal;
            if ($losos_lan = 0) {
              return 200;
            }
            # Not the body's place, as in askAboutRequest above.
            client_max_body_size 0;
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
        # header is the spare key, or a LosOS companion token that lososd
        # asks LosOS Login about itself, so it skips this question and the
        # box stays reachable with the spare key when Authelia is down.
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
        # The same question for the pages without JavaScript, whose spare
        # key is a cookie (modules/containers.nix turns it into the Bearer
        # header). A 401 there is their own sign-in page.
        "= /internal/losos-basic-authz" = {
          proxyPass = "http://127.0.0.1:${toString port}/auth/api/authz/auth-request";
          extraConfig = ''
            internal;
            if ($cookie_losos_spare) {
              return 200;
            }
            proxy_intercept_errors on;
            error_page 500 502 503 504 = @losos-authz-down;
            ${askAboutRequest}
          '';
        };
        "/basic/".extraConfig = ''
          ${basicHttps}
          auth_request /internal/losos-basic-authz;
          auth_request_set $losos_user $upstream_http_remote_user;
          auth_request_set $losos_groups $upstream_http_remote_groups;
          ${passPerson}
        '';
        "/api/".extraConfig = askAuthelia;
        "= /api/vms/images".extraConfig = askAuthelia;
        "= /api/market/llm/chat".extraConfig = askAuthelia;
      }
      # LosOS Login's cookie is Secure, so the pages without JavaScript are
      # https too, the sign-in steps first.
      //
        lib.genAttrs
          (map (p: "= /basic/${p}") [
            "sign-in"
            "sign-in/code"
            "sign-in/key"
            "sign-out"
            "prefs"
            "counts.css"
          ])
          (_: {
            extraConfig = basicHttps;
          })
      # The public routes, past Authelia. Exact matches beat the /api/ prefix.
      // lib.genAttrs (map (p: "= ${p}") lanPublicPaths) (_: {
        proxyPass = "http://127.0.0.1:${toString apiPort}";
        extraConfig = toLososd;
      })
      # The check-ins of the computers this box manages, from anywhere
      # (devicePaths above). Their bodies are a few words of JSON.
      // lib.genAttrs (map (p: "= ${p}") devicePaths) (_: {
        proxyPass = "http://127.0.0.1:${toString apiPort}";
        extraConfig = ''
          ${toLososdAnywhere}
          client_max_body_size 4k;
        '';
      });
  };
}
