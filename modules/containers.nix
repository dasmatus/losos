# The Nginx front door.
#
# One default_server vhost on :80 carries everything the appliance publishes:
# the admin SPA at /, lososd's JSON API at /api/*, LosOS Cloud
# (modules/cloud.nix) and LosOS Git at /git/. default_server because
# master-proxy (Traefik plus the edge tunnel) traffic arrives with a public hostname, not
# <hostName>.local. Nothing but Nginx binds a public port. The admin surface
# (/, /assets/, /setup/, /api/) is LAN-only (see `lanOnly` below); LosOS
# Cloud and /git/ are reachable through the master-proxy tunnel.
#
# This file used to *define* the services as well, as systemd-nspawn
# containers on private veths (10.231.1.2 and 10.231.2.2) with NAT for egress.
# All of that is gone: LosOS Cloud and LosOS Git run on the host, and the
# local cluster (modules/cluster.nix) runs only the `shared` apps.
#
# The local cluster runs no CNI (--flannel-backend=none), so its pods share the
# host's network namespace and bind 127.0.0.1. That is why the routes below
# proxy to loopback and to sockets instead of container addresses — and why
# `lanOnly` lost a layer of depth. That loss is spelled out where the guard is defined;
# do not re-add a deny rule for a subnet that no longer exists.
#
# The vhost itself is unconditional: it owns the *service* routes, so gating it
# on losos.admin.enable (as it used to be) took the services offline
# along with the dashboard. Only the admin locations, the document root and the
# admin-only firewall entry follow losos.admin.enable.
{
  lib,
  config,
  pkgs,
  ...
}:

let
  # LosOS Git (modules/git.nix) answers on a Unix socket only nginx may open.
  gitOn = config.losos.git.enable;
  gitSocket = "/run/losos-git/http.sock";
  # LosOS Git's pages come from the admin UI's bundle.
  gitPages = gitOn && adminEnabled;

  # Sign-in to the two apps with LosOS Login (modules/app-sign-in.nix,
  # modules/identity.nix for the questions nginx asks it).
  signIn = import ./app-sign-in.nix { inherit lib; };
  signInGit = config.lososInternal.appSignIn.git;
  # Whatever a client sent, the apps never see these headers from it.
  clearSignIn = ''
    proxy_set_header ${signIn.header.user} "";
    proxy_set_header ${signIn.header.name} "";
    proxy_set_header ${signIn.header.email} "";
  '';
  # Ask LosOS Login with `authz` and pass on whom it names. The values are
  # empty, so the headers are not sent, when it names nobody.
  passSignIn = authz: ''
    auth_request ${authz};
    auth_request_set $losos_user $upstream_http_remote_user;
    auth_request_set $losos_groups $upstream_http_remote_groups;
    auth_request_set $losos_person_name $upstream_http_remote_name;
    auth_request_set $losos_person_email $upstream_http_remote_email;
    proxy_set_header ${signIn.header.user} $losos_app_user;
    proxy_set_header ${signIn.header.name} $losos_person_name;
    proxy_set_header ${signIn.header.email} $losos_person_email;
  '';
  # Every request: the person if there is one, nobody otherwise.
  signInMaybe = passSignIn "/internal/losos-app-authz";
  adminUi = config.losos.admin.ui;
  adminApiPort = config.losos.admin.apiPort;
  # Only nginx's `losos-manage.invalid` server (modules/proxy.nix) may
  # tell lososd a call came from the edge that manages the box; on the front
  # door the headers are cleared, whatever the client sent.
  notTheEdge = ''
    proxy_set_header X-Losos-Via-Edge "";
    proxy_set_header X-Losos-Edge "";
  '';
  adminEnabled = config.losos.admin.enable && adminUi != null;
  # The pages without JavaScript (backend/plugins/basic/): the routes a
  # visitor reaches before signing in, those of them that speak to LosOS
  # Login for the visitor, and what every /basic/ request carries to lososd.
  basicSession = [
    "sign-in"
    "sign-in/code"
    "sign-in/key"
    "sign-out"
  ];
  basicOpen = [
    "prefs"
    "counts.css"
  ];
  basicToLososd = ''
    proxy_set_header X-Real-IP $remote_addr;
    ${notTheEdge}
    proxy_set_header X-Losos-Basic-Prefs $cookie_losos_basic;
    proxy_set_header X-Losos-Basic-Flash $cookie_losos_basic_flash;
    proxy_set_header X-Losos-Basic-Job $cookie_losos_basic_job;
  '';
  # The owner's handbook (handbook/, built as losos-handbook with its base at
  # /handbook/). Served under the admin guard: it is part of the admin
  # surface, not a public page, and a box reached through the tunnel must not
  # hand out its manual any more than its settings.
  handbook = config.losos.admin.handbook;
  handbookEnabled = adminEnabled && handbook != null;
  # LosOS Diagrams (once the Lab): a screen of the admin UI at /diagrams
  # (admin-ui/app/src/lab/), so it comes out of the same bundle and sits
  # behind the same guard: it reads this box's settings with the owner's
  # token.
  labEnabled = adminEnabled && config.losos.admin.lab;
  # The Lab's guests under libvirt (modules/lab.nix): their console and NIC
  # WebSockets skip lososd and go to the helper, which checks a per-guest
  # ticket only a token holder can get.
  labLibvirt = labEnabled && config.losos.lab.libvirt.enable;
  # LosOS Mail's screen (admin-ui/app/src/mail/), only while mail is live:
  # on the shared side, so with sharing off there is no /mail at all
  # (modules/mail.nix).
  mailEnabled = adminEnabled && config.lososInternal.mail.live;
  # LosOS Contacts' screen (admin-ui/app/src/contacts/): the book is LosOS
  # Cloud's address books (backend/src/cloud/contacts.rs), so it exists
  # exactly where LosOS Cloud runs. LosOS Calendar (src/calendar/) is its
  # calendars, under the same condition.
  contactsEnabled = adminEnabled && config.lososInternal.appSignIn.cloud;
  # Mesh CI's screen (admin-ui/app/src/screens/ci/), the same way: only
  # while Mesh CI runs (modules/ci.nix).
  ciEnabled = adminEnabled && config.lososInternal.ci.live;

  # Access guard for the admin surface (the SPA, its assets, the setup routes,
  # the lososd API): local network only. Master-proxy traffic must never reach
  # these routes — and it arrives from *loopback* (proxy.nix points the
  # tunnel client at 127.0.0.1:80), so loopback is deliberately not allowed. That costs nothing
  # for humans: the box has no shell logins, so no legitimate client browses
  # from localhost. The vhost only listens on 0.0.0.0, so the IPv4 ranges below
  # are exhaustive; if an IPv6 listener is ever added, these rules fail closed
  # (LAN IPv6 clients get 403) rather than open. Link-local (169.254/16) is not
  # allowed at all — a self-assigned address is not a trust signal on an admin
  # plane.
  #
  # What this guard no longer does, honestly:
  #
  # Under nspawn the workloads had their own subnet, so this list began with
  # `deny 10.231.0.0/16;` — nginx takes the first matching rule, and a PHP RCE
  # in a workload reaching Nginx on 0.0.0.0:80 *from* 10.231.1.2 was refused
  # before `allow 10.0.0.0/8` could wave it through. hostNetwork pods have no
  # address of their own, so there is nothing left to deny: nginx sees them as
  # 127.0.0.1 or as the box's own LAN address. A pod connecting over loopback
  # still matches none of the allows and falls through to `deny all`, so the
  # naive case is covered — but a compromised pod can source-bind the LAN
  # address and would then pass this guard. Do not paper that over with a deny
  # rule for the *local* cluster's pods: it has no pod CIDR, and a rule that
  # cannot match is worse than an acknowledged gap, because it reads like
  # protection.
  #
  # The source-bound pod is closed now too, by the `if` at the top: a request
  # whose source address is the very address it arrived on came from this
  # box, because no other host can send from the box's own address. That
  # matches a hostNetwork pod that binds the LAN address before connecting
  # to it, and no real LAN client. What it does not match is a box with two
  # local addresses where the pod binds one and connects to the other; such a
  # pod still reaches only what a LAN client reaches, and /api still wants the
  # token, but the claim route is unauthenticated while the box is unowned.
  # `if` with nothing but `return` is the use of it nginx documents as safe.
  #
  # The *mesh* is different, and `deny 10.42.0.0/16` is for it. Once
  # losos.cluster.enable is on, the rke2 agent runs canal with rke2's default
  # cluster-cidr, 10.42.0.0/16 (modules/edge.nix does not override it), and
  # schedules other people's pods here. A pod that connects to this box's own
  # LAN address is delivered locally, so flannel's MASQUERADE in POSTROUTING
  # never runs and nginx sees the pod address, which `allow 10.0.0.0/8` would
  # wave through to the whole admin surface, the unauthenticated claim route
  # included. Hence the deny first. A LAN that numbers itself inside
  # 10.42.0.0/16 would collide with the mesh's pod network anyway.
  #
  # What is genuinely left: /api is Bearer-authed against
  # losos.admin.tokenFile, which is 0600 root-only, and the workload pods run
  # as their own uids (1003, the apps' 1000/1001) — so they cannot read the token and cannot drive the API
  # even from an address this guard accepts. The static admin pages behind the
  # other guarded locations are not secrets.
  #
  # And never "fix" any of this with `allow 127.0.0.1`: that hands the whole
  # admin surface to the internet the moment losos.proxy.enable is on.
  #
  # (A LAN that genuinely numbers itself inside RFC1918 is the normal case and
  # is what the allows are for; there is no longer a carve-out to collide with.)
  lanOnly = import ./lan-only.nix;

  # Tor and I2P (modules/hidden-services.nix) arrive on loopback listeners
  # of their own. Loopback, so the guard above turns them away from the
  # admin surface like the tunnel.
  hiddenListen = (import ./hidden-lib.nix { inherit lib pkgs; }).frontListen (
    config.losos.hidden.tor.enable || config.losos.hidden.i2p.enable
  );

  # ── Security response headers ─────────────────────────────────────────────
  # The admin UI, /api/, /cloud/ and /git/ share one origin by design,
  # so a strict CSP is the only thing standing between an injected script on
  # the admin pages and the Bearer token the SPA keeps in sessionStorage.
  #
  # `add_header` at server level is inherited by every location that does
  # not set one of its own, so the values are selected by a `map` on $uri:
  # the routes with a policy of their own get theirs; nginx omits a header
  # whose value
  # is empty. Fail-closed on purpose: the default arm is the strict policy, so
  # a new admin route is covered without anyone remembering to add it.
  #
  # (Chosen over per-location `add_header` because that form *replaces* the
  # inherited set, so any header added at server level later would silently
  # vanish from exactly the locations that need it most.)
  #
  # img-src carries `data:` for Vite: anything imported under
  # build.assetsInlineLimit (4 KiB by default) is emitted as a data: URI rather
  # than a file, and a CSS background-image is an img-src fetch. The SPA
  # imports no binary assets today, so the grant is currently unused — it is
  # kept because the first small inlined SVG would otherwise fail in a console
  # nobody is watching, on a box with no shell. To drop it, pair
  # `img-src 'self'` with `build.assetsInlineLimit = 0` in
  # admin-ui/app/vite.config.ts so the ban is enforced where it can be seen.
  # (It previously covered a chevron inlined by the plain-JS pages'
  # design-system stylesheet, which is no longer served.)
  #
  # connect-src is plain 'self': it used to also carry http(s)://$host:3456
  # for the dashboard's cross-origin probe of the Tahoe web UI, and Tahoe-LAFS
  # is gone — with it the :3456 vhost, the probe, and any reason for this page
  # to talk to a second origin.
  #
  # script-src carries 'wasm-unsafe-eval' for LosOS Diagrams, a screen of
  # this page at /diagrams whose core is Rust compiled to WebAssembly
  # (admin-ui/lab/core). It lets the page compile WebAssembly and nothing
  # more: eval() and new Function() stay refused, and compiling a module
  # takes script that is already running, which this policy still allows
  # only from 'self'. It was a policy of its own while Diagrams was a page
  # of its own; as a screen it is reached by the router without a page load,
  # so the admin document needs it. img-src's blob: is the pictures and
  # setup files Diagrams hands out.
  adminCsp = lib.concatStringsSep "; " [
    "default-src 'none'"
    "script-src 'self' 'wasm-unsafe-eval'"
    "style-src 'self'"
    "img-src 'self' data: blob:"
    "font-src 'self'"
    "connect-src 'self'"
    # The board embeds `/widget-frame/` (below) for the owner's hand-written
    # widgets. It has to be spelled out: under CSP3 frame-src falls back to
    # child-src and then to default-src, which is 'none' above, so an absent
    # frame-src is a *block*, not an inherit. 'self' and nothing wider.
    "frame-src 'self'"
    "form-action 'none'"
    # Unchanged, and it governs the OTHER direction: who may frame the admin
    # page. Nothing about embedding a frame needs this loosened, and
    # nix/tests/front-vhost.nix asserts it stayed DENY precisely because reaching
    # for it is the obvious wrong way to unblock an iframe.
    "frame-ancestors 'none'"
    "base-uri 'none'"
  ];

  # The handbook's policy. Docusaurus boots its colour mode from an inline
  # script in <head> and the pages carry inline style attributes, neither of
  # which the admin SPA's policy allows; the site is static text this
  # repository builds, with nothing an owner typed in it, so 'unsafe-inline'
  # is a grant to our own build and nothing else. Everything else stays as
  # tight as the admin policy: no remote origins at all (the handbook has to
  # work with no internet, so it never asks for any), no framing, no forms.
  # Why a separate arm rather than loosening adminCsp: the admin page keeps
  # the token in sessionStorage, and 'unsafe-inline' there would turn any
  # injected markup into a token read.
  handbookCsp = lib.concatStringsSep "; " [
    "default-src 'none'"
    "script-src 'self' 'unsafe-inline'"
    "style-src 'self' 'unsafe-inline'"
    "img-src 'self' data:"
    "font-src 'self'"
    "connect-src 'self'"
    "frame-ancestors 'none'"
    "base-uri 'none'"
    "form-action 'none'"
  ];

  # LosOS Chat's policy (modules/chat.nix, admin-ui/app/chat/): the admin
  # policy's 'wasm-unsafe-eval' for its end-to-end encryption (matrix-js-sdk's
  # Rust crypto), plus blob: media for the voice messages and videos it
  # decrypts or downloads (an authenticated download is a fetch with a
  # header, shown from a blob: URL) and for the camera and microphone of a
  # call. connect-src 'self': the homeserver is this origin's /_matrix/.
  chatCsp = lib.concatStringsSep "; " [
    "default-src 'none'"
    "script-src 'self' 'wasm-unsafe-eval'"
    "style-src 'self'"
    "img-src 'self' data: blob:"
    "media-src 'self' blob:"
    "font-src 'self'"
    "connect-src 'self'"
    "frame-ancestors 'none'"
    "base-uri 'none'"
    "form-action 'none'"
  ];

  # The pages for a browser that runs no JavaScript (/basic/, drawn by
  # lososd's basic plugin, backend/plugins/basic/): no script at all, the
  # admin bundle's stylesheet and fonts, and forms that post to the box
  # itself. Their own arm because the admin policy's form-action is 'none',
  # and a page without a script changes things only by forms.
  basicCsp = lib.concatStringsSep "; " [
    "default-src 'none'"
    "style-src 'self'"
    "img-src 'self' data:"
    "font-src 'self'"
    "form-action 'self'"
    "frame-ancestors 'none'"
    "base-uri 'none'"
  ];

  # ── The widget frame ──────────────────────────────────────────────────
  # Widgets the owner writes by hand (backend/src/look.rs) are HTML with
  # their own script, and the admin CSP above refuses inline script for
  # good reason. They run instead inside `/widget-frame/`, one static page
  # in the SPA bundle that the board embeds as
  # `<iframe sandbox="allow-scripts">`: no `allow-same-origin`, so the frame
  # is an opaque origin that cannot read the page's sessionStorage (the
  # admin token), its DOM, or the API — a fetch to /api from there has no
  # token and no CORS answer. The page hands it the widget's source and the
  # box's readings over postMessage, nothing else.
  #
  # The frame needs a policy of its own: an `<iframe srcdoc>` or a `blob:`
  # document INHERITS the embedding page's CSP, so inline script in either is
  # refused just the same, and that is why the frame is a real URL. Its
  # policy is permissive for itself and nothing more: inline script and
  # style, pictures and fetches off the box for a weather tile, and
  # `frame-ancestors 'self'` so only this origin's own pages may embed it.
  # It is NOT `default-src 'none'`-strict and must never be served for any
  # other path — the arm below is anchored to the directory.
  #
  # X-Frame-Options is omitted for the same path (`DENY` blocks same-origin
  # framing too); `frame-ancestors` carries that half of the rule instead.
  widgetFrameCsp = lib.concatStringsSep "; " [
    "default-src 'none'"
    "script-src 'unsafe-inline' 'unsafe-eval'"
    "style-src 'unsafe-inline'"
    "img-src data: https: http:"
    "font-src data: https: http:"
    "media-src data: https: http:"
    "connect-src https: http:"
    "frame-ancestors 'self'"
    "base-uri 'none'"
    "form-action 'none'"
  ];

  # ── The app frame ─────────────────────────────────────────────────────
  # The owner's own apps (backend/src/user_apps.rs, src/apps/ in the SPA)
  # run in `/app-frame/` under `<iframe sandbox="allow-scripts allow-forms">`,
  # for the reasons the widget frame gives above. The difference is the
  # script: the frame loads the box's own runtime (React and the LosOS
  # components, /app-frame/runtime.js, so 'self') and the runtime adds the
  # app, which lososd compiled from the owner's JSX, as an inline script.
  # 'self' matches the frame's URL even though the sandbox gives the
  # document an opaque origin. No 'unsafe-eval': nothing in an app needs it.
  # `allow-forms` lets an app's <form> call its own submit handler;
  # `form-action 'none'` keeps the form from going anywhere.
  appFrameCsp = lib.concatStringsSep "; " [
    "default-src 'none'"
    "script-src 'self' 'unsafe-inline'"
    "style-src 'self' 'unsafe-inline'"
    "img-src data: blob: https: http:"
    "font-src 'self' data: https: http:"
    "media-src data: blob: https: http:"
    "connect-src https: http:"
    "frame-ancestors 'self'"
    "base-uri 'none'"
    "form-action 'none'"
  ];

  # ── A message's HTML ──────────────────────────────────────────────────
  # LosOS Mail shows an HTML message in `<iframe sandbox>` from lososd's
  # /api/mail/messages/<id>/html, never inside the Mail page. The document
  # gets backend/src/http.rs's MAIL_FRAME_CSP, the same text as here: a CSP
  # sandbox (no script, no same origin), inline style for the message's own
  # look, and no load from anywhere, so a remote picture cannot tell its
  # sender the message was opened. X-Frame-Options is dropped on that one
  # path (DENY would refuse the Mail page too); `frame-ancestors 'self'`
  # carries the rule. Anchored to the whole path, so no other /api/ route
  # loses a header.
  mailFrameCsp = lib.concatStringsSep "; " [
    "sandbox allow-popups allow-popups-to-escape-sandbox"
    "default-src 'none'"
    "style-src 'unsafe-inline'"
    "img-src data:"
    "font-src data:"
    "frame-ancestors 'self'"
    "base-uri 'none'"
    "form-action 'none'"
  ];

  # One map per header: value on the admin surface, empty (header omitted) on
  # LosOS Login under /auth/, which sends its own (the admin `base-uri 'none'`
  # drops its `<base href>`, and every page below /auth/ then loads its scripts
  # from the wrong folder and stays blank), the handbook's and chat's own
  # values (or the admin value, for the headers they share) under /handbook/
  # and /chat/ (and chat's for the guest call page, /calls/guest/), each
  # frame's own value (or nothing) under /widget-frame/ and /app-frame/, and
  # a mail message's under its HTML route, and the pages without
  # JavaScript under /basic/.
  #
  # Collabora Online's two prefixes (modules/office.nix) get nothing either,
  # only while it is on: coolwsd sends its own policy, whose frame-ancestors
  # lets the box's own pages frame the editor. Off, the arms are gone, so the
  # SPA fallback under those paths keeps the strict policy.
  officeArms = lib.optionalString config.losos.office.enable ''
    ~^/browser/      "";
        ~^/cool/         "";
  '';
  adminHeaderMap = variable: value: handbookValue: chatValue: frameValue: appFrameValue: mailValue: basicValue: ''
    map $uri ${variable} {
        default          "${value}";
        ~^/basic/        "${basicValue}";
        ${officeArms}
        ~^/auth/         "";
        ~^/handbook/     "${handbookValue}";
        ~^/chat/         "${chatValue}";
        ~^/calls/guest/  "${chatValue}";
        ~^/widget-frame/ "${frameValue}";
        ~^/app-frame/    "${appFrameValue}";
        ~^/api/mail/messages/[A-Za-z0-9_-]+/html$ "${mailValue}";
    }
  '';

  adminHeaders = [
    {
      name = "Content-Security-Policy";
      variable = "$losos_csp";
      value = adminCsp;
      handbookValue = handbookCsp;
      chatValue = chatCsp;
      frameValue = widgetFrameCsp;
      appFrameValue = appFrameCsp;
      mailValue = mailFrameCsp;
      basicValue = basicCsp;
    }
    {
      name = "X-Frame-Options";
      variable = "$losos_frame_options";
      value = "DENY";
      frameValue = "";
      mailValue = "";
    }
    {
      name = "X-Content-Type-Options";
      variable = "$losos_content_type_options";
      value = "nosniff";
      frameValue = "nosniff";
    }
    {
      name = "Referrer-Policy";
      variable = "$losos_referrer_policy";
      value = "no-referrer";
      frameValue = "no-referrer";
    }
  ];

  adminHeaderMaps = lib.concatMapStrings (
    h:
    adminHeaderMap h.variable h.value (h.handbookValue or h.value) (h.chatValue or h.value) h.frameValue (
      h.appFrameValue or h.frameValue
    ) (
      h.mailValue or h.value
    ) (h.basicValue or h.value)
  ) adminHeaders;

  # `always` so the headers ride on the 403s the lanOnly guard emits too.
  adminHeaderDirectives = lib.concatMapStrings (
    h: "add_header ${h.name} ${h.variable} always;\n"
  ) adminHeaders;
in
{
  assertions = [
    {
      assertion = config.losos.admin.enable -> config.losos.admin.ui != null;
      message = ''
        losos.admin.enable is on but losos.admin.ui is null, so there is no
        admin SPA to serve and every admin route would 404.
        modules/defaults.nix wires losos.admin.ui to the flake's
        losos-admin-ui package; either restore that or set
        losos.admin.enable = false.
      '';
    }
  ];

  # ── Nginx front router ────────────────────────────────────────────────────
  # One default_server vhost on :80: the admin SPA at /, lososd's JSON API at
  # /api/*, and the two workload routes. losos.admin.ui is a built Vite tree
  # (index.html + hashed assets/ + theme-boot.js), so it is the vhost root
  # outright — there is no page pair to alias alongside each other any more,
  # and every asset the document loads is an absolute /assets/… path served
  # from that same root.
  services.nginx = {
    enable = true;
    recommendedProxySettings = true;

    # Header value selectors — see `adminCsp` above for why they are maps.
    # Then the pages without JavaScript: the path the SPA document was asked
    # for, for its <noscript> (letters, digits and `/_-`, anything else is
    # the overview), the page a sign-in returns to, and the spare key as a
    # Bearer header.
    appendHttpConfig = adminHeaderMaps + ''
      map $request_uri $losos_basic_here {
          "~^(?<p>/[A-Za-z0-9/_-]*)(\?.*)?$" $p;
          default "/";
      }
      map $uri $losos_basic_back {
          "~^/basic/[A-Za-z0-9/_-]*$" $uri;
          default "/basic/";
      }
      map $cookie_losos_spare $losos_basic_bearer {
          "" "";
          default "Bearer $cookie_losos_spare";
      }
    '';

    virtualHosts."losos-front" = {
      default = true; # default_server — master-proxy traffic arrives with a public hostname
      # mkDefault because modules/tls.nix restates this listener alongside its
      # :443 one, on the same vhost (deliberately — see that file's header for
      # why HTTPS must not be a second vhost). `listen` is a listOf, and two
      # plain definitions would *concatenate* rather than override: the merged
      # config would carry `listen 0.0.0.0:80 default_server;` twice and nginx
      # would refuse to start with "a duplicate listen 0.0.0.0:80".
      #
      # That failure is invisible at build time — the nginx.conf derivation runs
      # gixy, not `nginx -t` — so it lands in nginx.service's ExecStartPre at
      # boot, which on this box means the only front door crash-loops with no
      # shell to see it from. nix/tests/setup.nix is what caught it, by being the
      # first test to import containers.nix and tls.nix together.
      #
      # The loopback listeners after it are where the box's Tor and I2P
      # addresses deliver (modules/hidden-services.nix).
      listen = lib.mkDefault (
        [
          {
            addr = "0.0.0.0";
            port = 80;
          }
        ]
        ++ hiddenListen
      );
      root = lib.mkIf adminEnabled "${adminUi}";
      extraConfig = adminHeaderDirectives;
      locations = lib.mkMerge [
        (lib.mkIf adminEnabled {
          # The whole admin surface is one location now: one SPA, one
          # document, real paths. LAN-only, like every admin location below.
          #
          # The `/index.html` arm is the load-bearing one. The app routes on
          # real paths rather than hashes (/storage, /settings/network), so
          # without it a deep link — or an ordinary reload of one — 404s.
          # admin-ui/app/src/App.tsx names this requirement in its own header.
          #
          # A consequence worth stating so nobody "fixes" it: an unknown path
          # under / now returns index.html with 200, and the SPA's catch-all
          # route renders "nothing here", where the old page pair returned
          # 404. That is what an SPA fallback means.
          "/" = {
            index = "index.html";
            tryFiles = "$uri $uri/ /index.html";
            # With JavaScript off the document sends the browser to the same
            # page under /basic/ (admin-ui/app/index.html's <noscript>). The
            # path is the one the browser asked for, from $losos_basic_here,
            # which lets letters, digits and `/_-` through and nothing else.
            extraConfig = ''
              ${lanOnly}
              sub_filter_once on;
              sub_filter 'url=/basic/"' 'url=/basic$losos_basic_here"';
            '';
          };
          # Content-hashed filenames, so a year is safe: a changed bundle gets
          # a changed name. `expires`, not `add_header` — an add_header at
          # location scope REPLACES the inherited set rather than adding to
          # it, which would silently drop the security headers this vhost sets
          # at server level. `expires` is a different directive and leaves
          # them alone.
          #
          # theme-boot.js is deliberately NOT covered: it is the one unhashed
          # file in the bundle (admin-ui/app/vite.config.ts says why), so it
          # has to keep revalidating against its ETag.
          # Not LAN-only while LosOS Cloud or LosOS Git signs people in:
          # their pages at /cloud/ (modules/cloud.nix) and /git/ are reached
          # through the tunnel too and load the same bundle. The files are
          # the app's code, the same on every box, and hold nothing of the
          # owner's.
          "/assets/" = {
            extraConfig = ''
              ${lib.optionalString (!(config.lososInternal.appSignIn.cloud || signInGit)) lanOnly}
              expires 1y;
            '';
          };
          # Fail closed rather than handing the SPA's index.html to a client
          # that asked for JSON. modules/setup.nix merges `= /setup/state.json`
          # and `= /setup/losos-ca.crt` into this same vhost; an exact match
          # beats this prefix, so where that module is imported nothing
          # changes. Where it is not, the first-run wizard gets a 404 — which
          # it has a branch for ("this box is too old to have a setup route")
          # — instead of an HTML document it would try to parse as JSON.
          "/setup/" = {
            tryFiles = "$uri =404";
            extraConfig = lanOnly;
          };
          "/api/" = {
            proxyPass = "http://127.0.0.1:${toString adminApiPort}";
            proxyWebsockets = true;
            # lososd only sees nginx as its peer; it throttles and audits by
            # this header instead. nginx overwrites any client-supplied value.
            extraConfig = ''
              ${lanOnly}
              proxy_set_header X-Real-IP $remote_addr;
              ${notTheEdge}
              # lososd answers most calls in milliseconds, but the first
              # claim runs `occ user:resetpassword` inside a pod that is
              # still warming up, and under emulation that outlived the
              # 60 s nginx defaults to. nginx answered 504 and closed the
              # connection while lososd went on to finish: password set, box
              # claimed, and the admin key — handed out exactly once, in
              # that reply — delivered to nobody. Ten minutes is longer than
              # any call here takes on real hardware; the replay window in
              # backend/src/receipt.rs covers the rest.
              proxy_read_timeout 600s;
              proxy_send_timeout 600s;
            '';
          };
          # The admin pages for a browser that runs no JavaScript: lososd's
          # basic plugin draws them at /api/basic/ (backend/plugins/basic/).
          # They keep what a script page keeps in the browser in cookies on
          # /basic, and nginx hands lososd those cookies as headers, which it
          # sets from the cookies alone, whatever the client sent. The spare
          # key, once typed on the sign-in page, rides as `losos_spare` and
          # goes on as the Bearer header the script pages send. A 401 (not
          # signed in) is the sign-in page.
          "/basic/" = {
            proxyPass = "http://127.0.0.1:${toString adminApiPort}/api/basic/";
            extraConfig = ''
              ${lanOnly}
              ${basicToLososd}
              proxy_set_header Authorization $losos_basic_bearer;
              proxy_set_header X-Losos-Login-Session "";
              proxy_intercept_errors on;
              error_page 401 = @losos-basic-sign-in;
            '';
          };
          "@losos-basic-sign-in".extraConfig = "return 303 /basic/sign-in?next=$losos_basic_back;";
          # The owner's own disk image for a market machine: gigabytes,
          # streamed through lososd to the edge (backend/src/vm_upload.rs),
          # which holds it to its own size limit. Unbuffered, so nginx does
          # not spool the file to disk first, and with no body cap of its
          # own. The same LAN-only guard and the same lososd as /api/.
          "= /api/vms/images" = {
            proxyPass = "http://127.0.0.1:${toString adminApiPort}";
            extraConfig = ''
              ${lanOnly}
              proxy_set_header X-Real-IP $remote_addr;
              ${notTheEdge}
              client_max_body_size 0;
              proxy_request_buffering off;
              proxy_http_version 1.1;
              proxy_read_timeout 6h;
              proxy_send_timeout 6h;
            '';
          };
          # An answer from a model split across boxes takes minutes; lososd
          # gives the edge 17 of them.
          "= /api/market/llm/chat" = {
            proxyPass = "http://127.0.0.1:${toString adminApiPort}";
            extraConfig = ''
              ${lanOnly}
              proxy_set_header X-Real-IP $remote_addr;
              ${notTheEdge}
              # The edge's cap (MAX_CHAT_BYTES in edge/registrar/src/server/llm.rs).
              client_max_body_size 2m;
              proxy_read_timeout 1200s;
              proxy_send_timeout 1200s;
            '';
          };
        })
        # The sign-in steps, signing out, the choices a visitor makes before
        # signing in, and the plural words the Apply bar counts with: open to
        # someone not signed in. Only the sign-in steps and signing out see
        # LosOS Login's session, which they hand on to it.
        (lib.mkIf adminEnabled (
          lib.genAttrs (map (p: "= /basic/${p}") basicSession) (name: {
          proxyPass = "http://127.0.0.1:${toString adminApiPort}/api${lib.removePrefix "= " name}";
          extraConfig = ''
            ${lanOnly}
            ${basicToLososd}
            proxy_set_header Authorization "";
            proxy_set_header X-Losos-Login-Session $cookie_losos_session;
          '';
        })
        // lib.genAttrs (map (p: "= /basic/${p}") basicOpen) (name: {
          proxyPass = "http://127.0.0.1:${toString adminApiPort}/api${lib.removePrefix "= " name}";
          extraConfig = ''
            ${lanOnly}
            ${basicToLososd}
            proxy_set_header Authorization "";
            proxy_set_header X-Losos-Login-Session "";
          '';
        })
        ))
        {
          # A company edge's manage page (manage/index.html, src/manage/)
          # rides in the same bundle. The edge serves it; the box never does.
          "/manage/".return = "404";
        }
        (lib.mkIf labEnabled {
          # LosOS Diagrams is a screen of the SPA at /diagrams, reached
          # through `/`'s fallback like every pane; its scripts and the
          # core's .wasm are in /assets/. This is the page's old address,
          # from before it was LosOS Diagrams. A return runs before the
          # access phase, so no guard here: the redirect says nothing, and
          # /diagrams/ keeps `/`'s.
          "/lab/".return = "301 /diagrams/$is_args$args";
        })
        (lib.mkIf (adminEnabled && !labEnabled) {
          # With Diagrams off (losos.admin.lab) there is no screen to fall
          # back to: both spellings answer 404, as Mail's do off the shared
          # side.
          "= /diagrams".return = "404";
          "/diagrams/".return = "404";
        })
        (lib.mkIf (adminEnabled && !mailEnabled) {
          # LosOS Mail is a screen of the SPA at /mail, reached through `/`'s
          # fallback while mail is live. Off the shared side there is no
          # mail page: both spellings answer 404.
          "= /mail".return = "404";
          "/mail/".return = "404";
        })
        (lib.mkIf (adminEnabled && !contactsEnabled) {
          # LosOS Contacts is a screen of the SPA at /contacts while LosOS
          # Cloud runs. Without it there is no book: both spellings answer
          # 404, as /cloud/api/ does.
          "= /contacts".return = "404";
          "/contacts/".return = "404";
          # LosOS Calendar, at /calendar, is LosOS Cloud's calendars.
          "= /calendar".return = "404";
          "/calendar/".return = "404";
        })
        (lib.mkIf (adminEnabled && !ciEnabled) {
          # Mesh CI's screen at /ci, gone with Mesh CI.
          "= /ci".return = "404";
          "/ci/".return = "404";
        })
        (lib.mkIf labLibvirt {
          # The Lab guests' serial consoles and network cards, as WebSockets
          # straight to `losos-registrar lab`. Longer than /api/ so it wins
          # the prefix match. Same LAN-only guard. The page's own origin is
          # what the helper checks the Origin header against, so the Host
          # goes through as the browser sent it, port included. `connect-src
          # 'self'` in adminCsp covers ws:// and wss:// to the same host.
          "/api/lab/ws/" = {
            proxyPass = "http://127.0.0.1:${toString config.losos.lab.libvirt.port}/lab/v1/guests/";
            proxyWebsockets = true;
            extraConfig = ''
              ${lanOnly}
              proxy_set_header Host $http_host;
              proxy_read_timeout 1h;
              proxy_send_timeout 1h;
            '';
          };
          # The libvirt relay for the Lab's WebAssembly libvirt client
          # (admin-ui/lab/virt-rpc): bytes between this WebSocket and
          # libvirt's socket, at the helper's uid. Admitted by the helper
          # only with a single-use ticket from POST /api/lab/virt-ticket,
          # which lososd hands out under the admin token. An exact match,
          # because a prefix would also take /api/lab/virt-ticket from
          # lososd.
          "= /api/lab/virt" = {
            proxyPass = "http://127.0.0.1:${toString config.losos.lab.libvirt.port}/lab/v1/virt";
            proxyWebsockets = true;
            extraConfig = ''
              ${lanOnly}
              proxy_set_header Host $http_host;
              proxy_read_timeout 1h;
              proxy_send_timeout 1h;
            '';
          };
        })
        (lib.mkIf handbookEnabled {
          # The owner's handbook, a second static tree beside the SPA's. It is
          # built with every link rooted at /handbook/, so `alias` (not
          # `root`) maps the prefix onto the store path. Docusaurus emits one
          # directory per page with its own index.html, so `$uri/` with the
          # index directive answers every page address; a path that is none
          # of those gets the site's own 404 page rather than the SPA's
          # index.html, which is what the `/` location's fallback would hand
          # out and which would render "nothing here" under the wrong app.
          # LAN-only like the rest of the admin surface; the header map
          # above gives this prefix its own CSP.
          "/handbook/" = {
            alias = "${handbook}/";
            index = "index.html";
            tryFiles = "$uri $uri/ =404";
            extraConfig = ''
              ${lanOnly}
              error_page 404 /handbook/404.html;
            '';
          };
        })
        (lib.mkIf gitOn (
          let
            # Pushes and clones of any size, streamed both ways, and the
            # person LosOS Login names on every request
            # (modules/app-sign-in.nix); a git client's own token in
            # Authorization skips that question (`/internal/losos-app-authz`).
            toGit = ''
              ${if signInGit then signInMaybe else clearSignIn}
              client_max_body_size 0;
              client_body_buffer_size 1m;
              proxy_request_buffering off;
              proxy_buffering off;
              proxy_http_version 1.1;
              proxy_read_timeout 1h;
              proxy_send_timeout 1h;
            '';
          in
          {
            # LosOS Git: its API and git's smart HTTP on the Unix socket of
            # modules/git.nix with the /git prefix taken off, and its pages
            # from the admin UI's bundle (admin-ui/app/git/). Not LAN-guarded,
            # like LosOS Cloud: through an edge it is how the owner clones from
            # anywhere, and every repository is checked by LosOS Git itself.
            "/git/api/" = {
              proxyPass = "http://unix:${gitSocket}:/api/";
              extraConfig = toGit;
            };
            # What git asks for: /git/<owner>/<repo>[.git]/info/refs and the
            # two service calls. Regex, so it wins over the pages' prefix.
            "~ ^/git/[^/]+/[^/]+/(?:info/refs|git-upload-pack|git-receive-pack)$" = {
              proxyPass = "http://unix:${gitSocket}";
              extraConfig = ''
                rewrite ^/git(/.*)$ $1 break;
                ${toGit}
              '';
            };
            # The pages: one document for every path. They hold no data, so
            # a visitor gets them too and reads the public repositories
            # through the API; with `git.site.requireSignIn` they are behind
            # LosOS Login like LosOS Cloud's (modules/cloud.nix). Without the
            # admin UI there are no pages, and the socket answers the rest as
            # before.
            "/git/" =
              if gitPages then
                {
                  tryFiles = "$uri /git/index.html";
                  extraConfig = lib.optionalString (signInGit && config.losos.git.site.requireSignIn) ''
                    auth_request /internal/losos-app-sign-in;
                    error_page 401 = @losos-app-sign-in;
                    error_page 403 @losos-app-held;
                    error_page 500 =503 @losos-app-waiting;
                  '';
                }
              else
                {
                  proxyPass = "http://unix:${gitSocket}:/";
                  extraConfig = toGit;
                };
          }
          # LosOS Wiki's published sites: a repository's wiki read as a site
          # of its own at /wiki/<owner>/<name>/ (admin-ui/app/wiki/), open to
          # whoever LosOS Git lets read that repository, through the same
          # API, and behind LosOS Login wherever the Git pages are.
          // lib.optionalAttrs gitPages {
            "/wiki/" = {
              tryFiles = "$uri /wiki/index.html";
              extraConfig = lib.optionalString (signInGit && config.losos.git.site.requireSignIn) ''
                auth_request /internal/losos-app-sign-in;
                error_page 401 = @losos-app-sign-in;
                error_page 403 @losos-app-held;
                error_page 500 =503 @losos-app-waiting;
              '';
            };
          }
          # The pre-paint theme script the pages load, outside the LAN guard
          # of `/` for the same reason as /assets/ below.
          // lib.optionalAttrs (gitPages && signInGit) { "= /theme-boot.js" = { }; }
        ))
      ];
    };
  };

  # ── Firewall ──────────────────────────────────────────────────────────────
  # Nginx owns :80 and nothing else on the box holds a public port: LosOS
  # Git binds a Unix socket and 127.0.0.1 (modules/git.nix), LosOS Cloud a
  # socket (modules/cloud.nix), the lososd API is loopback, and neither Kubernetes
  # instance adds to allowedTCPPorts, so 6443/9345/10250/10260 stay closed
  # against NixOS' default-deny firewall. The port is opened whenever the front
  # vhost has any route to serve — the admin SPA, LosOS Cloud or Git — because
  # gating it on the admin flag alone took the service routes down with the
  # dashboard.
  networking.firewall.allowedTCPPorts = lib.optional (
    adminEnabled || config.losos.cloud.enable || gitOn
  ) 80;
}
