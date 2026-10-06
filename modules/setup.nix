# First-run setup: the certificate the owner has to trust, and the few facts
# the wizard needs before it holds a token.
#
# Step 1 of the wizard is the owner installing this appliance's self-signed
# certificate on their devices. modules/tls.nix mints it at
# /var/lib/losos-tls/cert.pem — 0644 on purpose, two years, SAN'd for
# <hostName>.local — and deliberately stops there: getting those bytes into a
# browser is a front-door question, not a certificate-generation one. This file
# answers it, and adds the one thing the page cannot work out for itself, the
# certificate's SHA-256 fingerprint.
#
# Two routes, both on the front vhost modules/containers.nix owns:
#
#   GET /setup/losos-ca.crt   the certificate, typed and named so a browser
#                             does something useful with it
#   GET /setup/state.json     hostName, fqdn, and the certificate's fingerprint
#                             and expiry
#   GET /setup/trust.sh       a POSIX sh script that installs the certificate
#   GET /setup/trust.ps1      the same for PowerShell on Windows
#
# The two scripts are modules/setup/trust.{sh,ps1} with the certificate, its
# fingerprint and the box's name filled in at boot. They exist because the
# alternative — "download this file, open your OS's certificate manager, find
# the trust setting, restart the browser" — is a wall of instructions that
# most owners read as "skip this step", and skipping it costs them the passkey
# and sends every password over the LAN in the clear. One line pasted into a
# terminal is the shortest honest version of the step. The scripts are served
# as text so the line before the `| sh` can be read in a browser tab first,
# and they are deliberately boring: no sudo, no download, no second file,
# one store per browser family, undone by deleting one entry.
#
# They are attached by merging into
# `services.nginx.virtualHosts."losos-front".locations`, not by editing
# containers.nix: same attribute path, ordinary module-system merge, and the
# front door keeps being described in one place. A second vhost of our own was
# the obvious alternative and would have silently shipped without the
# CSP/X-Frame-Options headers, without the LAN guard, and without the TLS
# listener modules/tls.nix layers onto this same name.
#
# ── Why /setup/* is LAN-only, though a certificate is public ────────────────
#
# The certificate is a public key and two names; publishing it leaks no secret.
# The guard is not about the bytes:
#
#   * Master-proxy traffic reaches this vhost from 127.0.0.1 (rathole's
#     local_addr), so anything *not* guarded here is on the public internet the
#     moment losos.proxy.enable is on. "Guarded" is therefore the default a new
#     route has to argue its way out of. /nextcloud and /forgejo/ argued their
#     way out — carrying them is what the tunnel is for.
#   * This route cannot. The certificate is valid for <hostName>.local,
#     localhost and 127.0.0.1 and nothing else, so it is of no use to anyone
#     who is not already on the LAN. The tunnel terminates TLS at the edge with
#     a real certificate; this one never travels through it.
#   * What would actually be published is inventory: state.json names the box
#     and fingerprints its certificate. Not a secret. Not something to hand the
#     internet either.
#
# The cost of being wrong is asymmetric, which settles it: a wizard that 403s
# on the LAN is noticed in a minute, an appliance quietly announcing its
# identity to the internet is noticed by nobody.
#
# These routes do NOT follow losos.admin.enable, and that is deliberate. The
# certificate is what makes /nextcloud usable over :443, and Nextcloud is not
# part of the admin plane — a box with the dashboard switched off still serves
# HTTPS, and cutting the only sanctioned way to fetch its certificate would
# leave that HTTPS permanently untrusted, with no shell to work around it. What
# decides who may fetch this is the LAN guard, not the dashboard flag.
{
  pkgs,
  lib,
  config,
  ...
}:

let
  hostName = config.losos.hostName;
  fqdn = "${hostName}.local";
  tlsEnabled = config.losos.tls.enable;

  # Written by modules/tls.nix. Spelled out here rather than shared, because
  # tls.nix keeps the path in a `let` and not in an option — so the two files
  # can drift. tests/setup.nix fetches this route and byte-compares the result
  # against that file, which turns a drift into a failing test instead of a 404
  # in a wizard, on a box with no shell to debug it from.
  certFile = "/var/lib/losos-tls/cert.pem";

  # Not under /var/lib/losos: modules/daemon.nix owns that one with
  # StateDirectoryMode = "0700" so state.json stays root-only, which makes
  # anything beneath it unreadable to nginx whatever mode the file itself
  # carries. modules/tls.nix hit the same wall and drew the same conclusion.
  stateDir = "/var/lib/losos-setup";
  stateFile = "${stateDir}/state.json";

  # The last path segment is also the filename the browser saves, since there
  # is deliberately no Content-Disposition here — see the location below. `.crt`
  # because that is the extension every desktop OS has a certificate handler
  # registered for; Windows opens a `.pem` in a text editor.
  certUrl = "/setup/losos-ca.crt";
  stateUrl = "/setup/state.json";
  # `.sh` and `.ps1`, because the owner is asked to pipe one into `sh` and the
  # other into `iex`, and an extension that says which is which is the one
  # thing a person checks before pressing Enter.
  trustShUrl = "/setup/trust.sh";
  trustPs1Url = "/setup/trust.ps1";
  trustSh = "${stateDir}/trust.sh";
  trustPs1 = "${stateDir}/trust.ps1";

  # Verbatim from `lanOnly` in modules/containers.nix, which cannot be reached
  # from here (it is a `let` binding, not an option) and which is being edited
  # under a different change right now. Copying four lines was the cheaper of
  # the two risks. What keeps them honest is tests/setup.nix asserting the
  # *behaviour* — 403 from loopback, 200 from a LAN source — rather than the
  # text, so a divergence shows up as a red test and not as a route that
  # quietly guards nothing.
  #
  # The leading `if` refuses traffic from the box itself (source = the address
  # it arrived on), and `deny 10.42.0.0/16` is the mesh's pod network;
  # containers.nix says why each is there.
  #
  # The same warning applies here as there: never "fix" a loopback 403 with
  # `allow 127.0.0.1`. Loopback is where the tunnel arrives.
  lanOnly = ''
    if ($remote_addr = $server_addr) {
      return 403;
    }
    deny 10.42.0.0/16;
    allow 10.0.0.0/8;
    allow 172.16.0.0/12;
    allow 192.168.0.0/16;
    deny all;
  '';

  # Rendered through toJSON so a hostName containing a quote cannot produce a
  # document the wizard fails to parse.
  json = builtins.toJSON;

  # ── Cross-origin read of state.json, for the finder page ─────────────────
  # options.nix (`losos.setup.finderOrigins`) says what this is for and why it
  # is an allow-list. The shape here is the one modules/containers.nix uses
  # for its security headers, for the same reason: `add_header` at location
  # scope would replace the inherited set, so the header is added at server
  # level with a value that is empty — and therefore omitted — everywhere it
  # does not apply. The value depends on origin *and* path together, so a
  # matching Origin on any other route (the SPA, the certificate, /nextcloud)
  # still gets nothing. A browser only sends Origin on cross-origin requests,
  # so the wizard's own same-origin read is unaffected.
  #
  # No `Vary: Origin`: the wizard fetches with `cache: 'no-store'`, the finder
  # does the same, and nothing else reads this document. Exact matches only,
  # so an origin that reads like a pattern is a non-match, not a hole.
  finderOrigins = config.losos.setup.finderOrigins;
  corsEnabled = finderOrigins != [ ];
  # Two maps rather than one keyed on "$http_origin $uri": a combined key is
  # long enough to overflow nginx's default map_hash_bucket_size (64), and the
  # fix for that is a global tunable nobody would connect to this file. The
  # second map's value is the first map's variable, which nginx resolves
  # per request, so the header carries the origin only when both match.
  corsMap = ''
    map $http_origin $losos_finder_origin {
        default "";
    ${
      lib.concatMapStrings (o: ''
        "${o}" "${o}";
      '') finderOrigins
    }}
    map $uri $losos_setup_cors {
        default "";
        "${stateUrl}" $losos_finder_origin;
    }
  '';

  # temp + rename: nginx must never read a half-written document, and the
  # request that caught it would be the first one the wizard ever makes.
  writeState = body: ''
    cat > ${stateFile}.new <<EOF
    ${body}
    EOF
    mv ${stateFile}.new ${stateFile}
  '';

  # The static half of the document is known at evaluation time; only the
  # fingerprint and the expiry have to wait for a certificate that does not
  # exist until first boot. `tlsEnabled` is a build-time value, so these are two
  # scripts rather than one script with a branch in it.
  measureCert = ''
    display=$(openssl x509 -in ${certFile} -noout -fingerprint -sha256 | cut -d= -f2)
    hex=$(printf '%s' "$display" | tr -d ':' | tr 'A-F' 'a-f')
    # The fingerprint is the one field whose entire job is being exact: the
    # owner compares it against what their browser reports before trusting the
    # certificate. A wrong one is worse than none, so a surprising openssl
    # output fails the unit instead of being published.
    [ "''${#hex}" = 64 ] || {
      echo "not a SHA-256 fingerprint: $display" >&2
      exit 1
    }
    expires=$(date -u -d "$(openssl x509 -in ${certFile} -noout -enddate | cut -d= -f2)" \
      +%Y-%m-%dT%H:%M:%SZ)
  '';

  # The install scripts, from their templates. Three placeholders: the PEM
  # line is replaced by the certificate file itself (sed's `r` queues the file
  # and `d` drops the marker line), the fingerprint is the display spelling
  # the owner is told to compare, and the name is for the entry's label and
  # the closing sentence. The certificate is pasted raw, so what the script
  # installs is byte for byte what ${certUrl} serves — tests/setup.nix
  # extracts it back out and cmp's it. temp + rename, as for state.json.
  renderInstaller = template: out: ''
    sed -e "/^@PEM@$/{r ${certFile}" -e "d}" \
      -e "s/@FINGERPRINT@/$display/g" \
      -e "s/@HOST@/${hostName}/g" \
      ${template} > ${out}.new
    mv ${out}.new ${out}
  '';

  stateScript =
    if tlsEnabled then
      measureCert
      + renderInstaller ./setup/trust.sh trustSh
      + renderInstaller ./setup/trust.ps1 trustPs1
      + writeState ''
        {
          "hostName": ${json hostName},
          "fqdn": ${json fqdn},
          "tls": true,
          "certificate": {
            "url": ${json certUrl},
            "fingerprint": "sha256:$hex",
            "fingerprintDisplay": "$display",
            "expires": "$expires",
            "install": {
              "sh": ${json trustShUrl},
              "ps1": ${json trustPs1Url}
            }
          }
        }
      ''
    else
      writeState ''
        {
          "hostName": ${json hostName},
          "fqdn": ${json fqdn},
          "tls": false,
          "certificate": null
        }
      '';
in
{
  assertions = [
    {
      # The origins are pasted into an nginx map verbatim; the shape check is
      # what keeps a stray quote or path from becoming an nginx config error on
      # a box with no shell to read it from.
      assertion = lib.all (o: builtins.match "https?://[A-Za-z0-9.-]+(:[0-9]+)?" o != null) finderOrigins;
      message = "losos.setup.finderOrigins entries must be bare origins, scheme://host[:port]: ${toString finderOrigins}";
    }
  ];

  services.nginx.appendHttpConfig = lib.mkIf corsEnabled corsMap;
  services.nginx.virtualHosts."losos-front".extraConfig = lib.mkIf corsEnabled ''
    add_header Access-Control-Allow-Origin $losos_setup_cors always;
  '';

  # ── The document the wizard reads before it has a token ───────────────────
  #
  # Deliberately only what is true *without* one: the box's name, and the
  # certificate that has to be trusted before anything else in the wizard can
  # happen over HTTPS. Everything that depends on lososd's own state — whether
  # setup was completed, whether the Nextcloud admin password has ever been
  # set, whether a recovery code has been minted — belongs to `GET /api/setup`,
  # which is Bearer-authed. Two documents because they answer to two different
  # authorities, not because anyone wanted two.
  systemd.services.losos-setup-state = {
    description = "Publish the first-run setup facts for the admin UI";
    wantedBy = [ "multi-user.target" ];
    # Ordering only, no Requires: a missing state.json is a wizard that shows
    # one step less, a blocked nginx is a box nobody can reach at all.
    before = [ "nginx.service" ];
    # Requires on the certificate generator, though — with nothing to
    # fingerprint there is nothing honest to publish. Its ConditionPathExists
    # skip on every boot after the first counts as success, which is the
    # assumption modules/tls.nix already makes for nginx itself.
    after = lib.optional tlsEnabled "losos-tls-cert.service";
    requires = lib.optional tlsEnabled "losos-tls-cert.service";
    path = [
      pkgs.openssl
      pkgs.coreutils
      pkgs.gnused
    ];
    serviceConfig = {
      Type = "oneshot";
      RemainAfterExit = true;
      # 0755/0644, unlike every other losos state directory. Nothing here is
      # secret — a hostname, and the fingerprint of a certificate handed to
      # anyone on the LAN — and nginx has to be able to read it.
      #
      # It survives a reboot because modules/impermanence.nix persists the
      # whole of /var, but nothing depends on that and nothing should: the unit
      # rewrites the file from the certificate on every boot. The source of
      # truth is the PEM, never this cache of two facts about it.
      StateDirectory = "losos-setup";
      StateDirectoryMode = "0755";
      UMask = "0022";
    };
    script = ''
      set -eu
      ${stateScript}
    '';
  };

  services.nginx.virtualHosts."losos-front".locations = lib.mkMerge [
    (lib.mkIf tlsEnabled {
      # ── The certificate, for a human with a browser ────────────────────────
      "= ${certUrl}" = {
        alias = certFile;
        extraConfig = ''
          ${lanOnly}
          # An empty `types` block drops the inherited map, so the content type
          # is this line and not whatever nginx's stock mime.types happens to
          # say about `.crt` in some future release.
          #
          # application/x-x509-ca-cert is the conventional type for exactly
          # this file, and the certificate really is a CA (tls.nix sets
          # basicConstraints CA:TRUE — nothing else would be trustable as a
          # root). Browsers that recognise the type act on it: Firefox's
          # certificate-import dialog, iOS' configuration-profile flow.
          # Everything else falls back to saving the file, which is what the
          # wizard's instructions cover anyway. Either outcome is the right
          # one; a certificate rendered as text/plain in the page is not.
          types { }
          default_type application/x-x509-ca-cert;

          # And deliberately NO `add_header Content-Disposition`, which is the
          # obvious next line to write here.
          #
          # `add_header` at location scope REPLACES the inherited set rather
          # than extending it, so one Content-Disposition would silently take
          # the four security headers modules/containers.nix adds at server
          # level off this response. NixOS does not let that through quietly
          # either: it runs gixy over the merged config at build time and
          # `add_header_redefinition` fails the build — "Parent headers
          # x-frame-options, content-security-policy was dropped in current
          # level". Restating all four here to satisfy it would mean reaching
          # into containers.nix' `$losos_csp` map from this file, i.e. coupling
          # two modules through an nginx variable name.
          #
          # What it would have bought is a nicer saved filename. Without it the
          # browser names the download after the last path segment, which is
          # already `losos-ca.crt` — and `losos.hostName` defaults to the same
          # "mattbox" on every appliance, so a host-derived name would not even
          # have distinguished two boxes. Not a trade worth a hole in the front
          # door's headers.
        '';
      };

      # ── The install scripts, for a human with a terminal ──────────────────
      # text/plain, so the URL opens as readable text in a browser tab — the
      # wizard links it with "read it first" — and so `irm` returns a string
      # for `iex` rather than trying to parse it. The same LAN guard and the
      # same no-add_header rule as the certificate: nothing here is secret,
      # and the inherited security headers do not hurt a script.
      "= ${trustShUrl}" = {
        alias = trustSh;
        extraConfig = ''
          ${lanOnly}
          types { }
          default_type text/plain;
          charset utf-8;
        '';
      };
      "= ${trustPs1Url}" = {
        alias = trustPs1;
        extraConfig = ''
          ${lanOnly}
          types { }
          default_type text/plain;
          charset utf-8;
        '';
      };
    })
    {
      # ── The setup document ────────────────────────────────────────────────
      # No add_header here either, for the same reason as above — including no
      # Cache-Control, and including the CORS header, which lives at server
      # level (see `corsMap`). The wizard asks for this with `cache: 'no-store'`, which
      # costs nothing; nginx serves Last-Modified and ETag for a static file,
      # so even a client that does cache it revalidates and sees a new
      # fingerprint as soon as the file is rewritten.
      "= ${stateUrl}" = {
        alias = stateFile;
        extraConfig = ''
          ${lanOnly}
          types { }
          default_type application/json;
        '';
      };
    }
  ];
}
