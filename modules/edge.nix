# Master proxy — edge side.
#
# The edge is a second NixOS system (nixosConfigurations.edge) living on a
# public VPS. It runs the MASTER proxy: Traefik (public TLS on :443), a
# rathole server (appliance clients dial :2333), and losos-registrar (serve)
# — the stub Rust HTTP server that *constantly updates* Traefik's dynamic
# file-provider config and rathole's server config from a tenant registry.
#
# Traefik's file provider watches /etc/traefik/dynamic as a DIRECTORY:
#   * register.yml  — the ONE static route (register.<domain> → the
#     registrar's loopback API). NixOS-managed (this module), symlinked in.
#   * losos.yml      — per-appliance routers, written at runtime by the
#     registrar. Traefik auto-reloads on change.
# The registrar is the sole writer of losos.yml and /etc/rathole/server.toml;
# it never touches register.yml. That separation keeps the registrar from
# being able to lock itself out of the control plane.
#
# See docs/superpowers/specs/2026-08-17-master-proxy-design.md. The appliance
# side is modules/proxy.nix. Replaces the retired losos.cfd (Cloudflare).
#
# NOTE(transport): rathole runs plain TCP for now. The tunnel carries
# post-TLS-termination HTTP, so enabling rathole's Noise transport (with a
# distributed static keypair) is a hardening step before production — see the
# spec's "Gotchas" and the rathole [transport] noise section. The registrar's
# config writer is where that flip lands.
{
  config,
  lib,
  pkgs,
  self,
  ...
}:

let
  cfg = config.losos.edge;
  enabled = cfg.enable;
  rathole = cfg.rathole.package;
  registrar = cfg.registrar.package;

  registerDomain = "register.${cfg.publicDomain}";

  # Format a `host:port` socket string for rathole's `bind_addr`, bracketing
  # IPv6 literals: `::` → `[::]:2333`, `0.0.0.0` → `0.0.0.0:2333`. rathole parses
  # `bind_addr` as a `SocketAddr`, so an unbracketed IPv6 literal (`::2333`)
  # is rejected. The default `ratholeBindAddr` is `::` (dual-stack — Linux
  # accepts IPv4-mapped connections on an `::` bind), so an appliance that
  # resolves the edge over IPv6 reaches the tunnel. MUST stay byte-identical
  # to the `format_bind` helper in backend-registrar/src/config.rs — the
  # declarative seed this writes and the registrar's runtime output must
  # compare equal for the zero-tenant steady state (pinned by
  # config.rs::server_block_matches_nix_seed_byte_for_byte).
  fmtBind = addr: port:
    if lib.hasInfix ":" addr then "[${addr}]:${toString port}" else "${addr}:${toString port}";

  # Static Traefik config. We bypass services.traefik.staticConfigOptions
  # because the NixOS module force-merges `providers.file.filename` (a single
  # file); we need `providers.file.directory` so Traefik watches the dir the
  # registrar drops losos.yml into alongside the static register.yml.
  tomlFmt = pkgs.formats.toml { };
  staticCfg = tomlFmt.generate "losos-traefik-static.toml" {
    entryPoints.web.address = ":80";
    entryPoints.web.http.redirections.entryPoint.to = "websecure";
    entryPoints.web.http.redirections.entryPoint.scheme = "https";
    entryPoints.websecure.address = ":443";
    providers.file.directory = "/etc/traefik/dynamic";
    certificatesResolvers.le.acme.email =
      if cfg.acmeEmail == null then "unconfigured@losos.cfd" else cfg.acmeEmail;
    certificatesResolvers.le.acme.storage = "/var/lib/traefik/acme.json";
    certificatesResolvers.le.acme.tlsChallenge = { };
  };

  # The one static route: register.<domain> → the registrar's loopback API.
  # Has a cert before any appliance registers (avoids the register/route
  # chicken-and-egg). The registrar never rewrites this file.
  registerYml = pkgs.writeText "losos-register-route.yml" ''
    http:
      routers:
        register:
          rule: "Host(`${registerDomain}`)"
          service: register
          entryPoints:
            - websecure
          tls:
            certResolver: le
            domains:
              - main: "${registerDomain}"
      services:
        register:
          loadBalancer:
            servers:
              - url: "http://127.0.0.1:${toString cfg.registrarApiPort}"
  '';

  # tenants.json: id → {hostname, token_file}. token_file is a PATH (not the
  # secret), so this file can live in the world-readable store — the secret is
  # the file's *contents*, read by the registrar at reconcile time.
  tenantsJson = pkgs.writeText "losos-registrar-tenants.json" (
    builtins.toJSON (lib.mapAttrs (_: v: {
      hostname = v.hostname;
      token_file = toString v.tokenFile;
    }) cfg.tenants)
  );

  # Seed the rathole server config to the node: a declarative [server] base
  # (bind_addr + default_token) + an empty [server.services] table, written at
  # boot so rathole can start INDEPENDENTLY of the registrar. rathole's server
  # config REQUIRES a `services` field, so the empty table is mandatory —
  # without it rathole refuses to start ("missing field `services`"). The
  # registrar later rewrites the whole file (same [server] base from the same
  # options + [server.services.*] as appliances register) and the file watcher
  # hot-reloads; with no tenants the registrar's output is byte-identical to
  # this seed (pinned by config.rs::server_block_matches_nix_seed_byte_for_byte),
  # so steady-state is zero churn. Idempotent: only seeds when the file is
  # absent (first boot) — after that the registrar is the writer.
  ratholeSeed = pkgs.writeShellScript "losos-rathole-seed" ''
    set -eu
    if [ -f /etc/rathole/server.toml ]; then exit 0; fi
    install -d -m 0700 /etc/rathole
    umask 077
    bootstrap="$(cat ${toString cfg.bootstrapTokenFile})"
    cat > /etc/rathole/server.toml <<EOF
[server]
bind_addr = "${fmtBind cfg.ratholeBindAddr cfg.ratholeBindPort}"
default_token = "$bootstrap"

[server.services]
EOF
  '';

  serveArgs = lib.concatStringsSep " " [
    "${registrar}/bin/losos-registrar"
    "serve"
    "--listen"
    "${fmtBind cfg.registrarApiBind cfg.registrarApiPort}"
    "--registry"
    "/var/lib/losos-registrar/registry.json"
    "--traefik-dir"
    "/etc/traefik/dynamic"
    "--rathole-config"
    "/etc/rathole/server.toml"
    "--rathole-bind-addr"
    cfg.ratholeBindAddr
    "--rathole-bind-port"
    (toString cfg.ratholeBindPort)
    "--port-range"
    cfg.ratholePortRange
    "--bootstrap-token-file"
    (toString cfg.bootstrapTokenFile)
    "--tenants-file"
    (toString tenantsJson)
    "--reconcile-interval"
    cfg.reconcileInterval
    "--heartbeat-ttl"
    cfg.heartbeatTtl
    "--upload-dir"
    "/run/losos-registrar"
  ];
in
{
  config = lib.mkIf enabled {
    # The edge module is only imported by the edge system, so wiring the
    # registrar package here (gated on `enabled`, which is always true for the
    # edge system) keeps edge.nix self-contained — mirrors defaults.nix wiring
    # losos.backend.package on the appliance.
    losos.edge.registrar.package = self.packages.x86_64-linux.losos-registrar;

    assertions = [
      {
        assertion = cfg.acmeEmail != null;
        message = "losos.edge.enable requires losos.edge.acmeEmail (Let's Encrypt account email).";
      }
    ];

    # ── Traefik (master proxy) ───────────────────────────────────────────
    services.traefik = {
      enable = true;
      staticConfigFile = staticCfg;
      group = "traefik";
    };

    # /etc/traefik/dynamic: real dir (0755, root) holding the static
    # register.yml (symlinked from the store) + the registrar's runtime
    # losos.yml. Traefik's file provider reads both.
    systemd.tmpfiles.rules = [
      "d /etc/traefik/dynamic 0755 root root - -"
      "L+ /etc/traefik/dynamic/register.yml - - - - ${toString registerYml}"
    ];

    # ── losos-registrar (registration API + reconciler) ─────────────────
    systemd.services.losos-registrar = {
      description = "losos edge registrar — registration API + Traefik/rathole config reconciler";
      wantedBy = [ "multi-user.target" ];
      after = [
        "network-online.target"
        "losos-rathole-seed.service"
      ];
      wants = [ "network-online.target" ];
      serviceConfig = {
        ExecStart = serveArgs;
        StateDirectory = "losos-registrar";
        StateDirectoryMode = "0700";
        # /run/losos-registrar: tmpfs spill dir for the POST /config upload
        # endpoint. Deliberately a RuntimeDirectory (tmpfs, wiped on reboot)
        # — NOT under /persist — so an uploaded config that survives a missed
        # unlink still vanishes on reboot (data-retention minimisation).
        RuntimeDirectory = "losos-registrar";
        RuntimeDirectoryMode = "0700";
        Restart = "always";
        RestartSec = 5;
        # Writes /etc/traefik/dynamic + /etc/rathole; reads 0600 token files.
        # Runs as root; deliberately no ProtectSystem (would make /etc RO).
        PrivateTmp = true;
        NoNewPrivileges = true;
      };
    };

    # ── rathole server config seed ───────────────────────────────────────
    # Writes the declarative [server] base so rathole can start before the
    # registrar. Idempotent (only when server.toml is absent). The registrar
    # rewrites the file afterward, so this is a first-boot seed only.
    systemd.services.losos-rathole-seed = {
      description = "losos rathole server config seed (declarative [server] base)";
      wantedBy = [ "multi-user.target" ];
      before = [
        "losos-rathole.service"
        "losos-registrar.service"
      ];
      serviceConfig = {
        ExecStart = ratholeSeed;
        Type = "oneshot";
        RemainAfterExit = true;
        PrivateTmp = true;
      };
    };

    # ── rathole server (tunnel endpoint) ─────────────────────────────────
    # Starts from the seeded [server] base (losos-rathole-seed), independent
    # of the registrar. The registrar rewrites server.toml to add
    # [server.services.*]; rathole's `notify` file-watcher hot-reloads the
    # config the instant it's rewritten (atomic temp+rename) — no signal
    # needed. SIGHUP would kill rathole 0.5 (no handler) and Restart=always
    # would resurrect it ~5s later, so the registrar sends nothing.
    systemd.services.losos-rathole = {
      description = "losos rathole server — master-proxy tunnel endpoint";
      wantedBy = [ "multi-user.target" ];
      after = [
        "network-online.target"
        "losos-rathole-seed.service"
      ];
      requires = [ "losos-rathole-seed.service" ];
      wants = [ "network-online.target" ];
      serviceConfig = {
        ExecStart = "${rathole}/bin/rathole /etc/rathole/server.toml";
        Restart = "always";
        RestartSec = 5;
        PrivateTmp = true;
      };
    };

    # ── Firewall: public web + the rathole client-dial port ──────────────
    # The registrar API is loopback-only in production (Traefik fronts it at
    # register.<publicDomain>); only open it in the firewall when it is bound
    # off-loopback — which the option docs say is a test-only direct-dial
    # posture, never a deployment one.
    networking.firewall.allowedTCPPorts =
      [
        80
        443
        cfg.ratholeBindPort
      ]
      ++ lib.optional (cfg.registrarApiBind != "127.0.0.1") cfg.registrarApiPort;
  };
}