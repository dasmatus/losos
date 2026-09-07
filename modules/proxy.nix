# Master proxy — appliance side.
#
# The appliance keeps its no-SSH/no-public-ports invariant: it opens zero
# inbound ports. A rathole client dials out to the edge (losos.proxy.edge
# .edgeRatholeEndpoint); the edge rathole server forwards public traffic
# back over that tunnel to this box's Nginx on :80 (the slave proxy). A
# losos-registrar `announce` service registers this appliance with the edge
# (POST /register) and heartbeats so the edge keeps the Traefik route alive.
#
# See docs/superpowers/specs/2026-08-17-master-proxy-design.md and the edge
# side in modules/edge.nix. Replaces the retired losos.cfd (Cloudflare).
#
# Secrets: the rathole client config contains tokens (bootstrap + per-
# appliance). It is generated at service start from the secret files into a
# 0600 runtime file (/run/losos-rathole/client.toml) — never baked into the
# world-readable nix store.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.losos.proxy;
  enabled = cfg.enable;
  registrar = cfg.registrar.package;
  rathole = cfg.rathole.package;

  # Runtime, 0700, root. Holds the generated client.toml (with secrets).
  confDir = "/run/losos-rathole";

  # Generates client.toml at start from the secret files. The non-secret
  # parts (remote_addr, service name, local_addr) are templated in; the
  # secrets are read from their 0600 files so no token ever lands in the
  # store. rathole auto-detects [client] mode from the file.
  genClientConf = pkgs.writeShellScript "losos-rathole-client-conf" ''
        set -eu
        install -d -m 0700 ${confDir}
        umask 077
        bootstrap="$(cat ${cfg.bootstrapTokenFile})"
        token="$(cat ${cfg.tokenFile})"
        cat > ${confDir}/client.toml <<EOF
    [client]
    remote_addr = "${cfg.edgeRatholeEndpoint}"
    default_token = "$bootstrap"

    [client.services.${cfg.applianceId}]
    token = "$token"
    local_addr = "127.0.0.1:80"
    EOF
  '';

  # announce flags. --token-file is a path (read at runtime), not the secret,
  # so it's safe to put on the command line / in the store.
  announceArgs = lib.concatStringsSep " " [
    "announce"
    "--registrar-url"
    (lib.escapeShellArg cfg.registrarUrl)
    "--appliance-id"
    (lib.escapeShellArg cfg.applianceId)
    "--hostname"
    (lib.escapeShellArg cfg.hostname)
    "--token-file"
    (lib.escapeShellArg cfg.tokenFile)
    "--heartbeat-interval"
    (lib.escapeShellArg cfg.heartbeatInterval)
  ];
in
{
  config = lib.mkIf enabled {
    # Hard requirements: the registrar binary and rathole must be available,
    # and the secrets must be readable. (We can't assert file *contents* at
    # eval time; we assert the packages exist.)
    assertions = [
      {
        assertion = registrar != null;
        message = "losos.proxy.enable needs losos.proxy.registrar.package (wired by defaults.nix).";
      }
    ];

    # ── rathole client (outbound tunnel) ─────────────────────────────────
    systemd.services.losos-rathole-client = {
      description = "losos rathole client — outbound tunnel to the master-proxy edge";
      wantedBy = [ "multi-user.target" ];
      after = [ "network-online.target" ];
      wants = [ "network-online.target" ];
      serviceConfig = {
        ExecStartPre = genClientConf;
        ExecStart = "${rathole}/bin/rathole -c ${confDir}/client.toml";
        Restart = "always";
        RestartSec = 5;
        PrivateTmp = true;
        NoNewPrivileges = true;
        ProtectSystem = "strict";
        ReadWritePaths = [ confDir ];
        RuntimeDirectory = "losos-rathole";
        RuntimeDirectoryMode = "0700";
      };
    };

    # ── losos-registrar announce (register + heartbeat) ─────────────────
    systemd.services.losos-registrar-announce = {
      description = "losos registrar announce — register + heartbeat with the edge";
      wantedBy = [ "multi-user.target" ];
      after = [ "network-online.target" ];
      wants = [ "network-online.target" ];
      # The rathole tunnel is up independently; announce just tells the edge
      # to publish the route. Don't hard-depend (announce retries forever).
      serviceConfig = {
        ExecStart = "${registrar}/bin/losos-registrar ${announceArgs}";
        Restart = "always";
        RestartSec = 5;
        PrivateTmp = true;
        NoNewPrivileges = true;
      };
    };
  };
}
