# Master proxy — appliance side.
#
# The appliance keeps its no-SSH/no-public-ports invariant: it opens zero
# inbound ports. A tunnel client (`losos-registrar tunnel-client`, QUIC)
# dials out to an edge; the edge carries public traffic back to this box's
# Nginx on :80 (the slave proxy) as streams of that one connection. A
# losos-registrar `announce` service registers this appliance with the edge
# (POST /register) and heartbeats so the edge keeps the Traefik route alive.
#
# Which edge: lososd decides (backend/src/edge.rs, "the path"), and tells
# the two units here through /run/losos/edge-path.env — an edge found on the
# LAN first, the configured official one (losos.proxy.registrarUrl,
# losos.proxy.edgeTunnelEndpoint) when the LAN has none, and when neither
# answers it writes /run/losos/edge-none and stops both units, so a box with
# no edge in reach runs no tunnel at all. Both files live on /run and are
# gone at boot: until lososd's first scan the units dial the configured
# edge, which is also what they do on a box that runs no lososd
# (nix/tests/edge-vm.nix). The daemon only writes the files when
# losos.proxy.enable is on; see modules/daemon.nix.
#
# See handbook/docs/in-depth/master-proxy.md, the edge side in
# modules/edge.nix, and handbook/docs/in-depth/edge-federation.md for the
# path rule.
#
# Secrets: the client's file holds the box's token. It is generated at
# service start from the secret file into a 0600 runtime file
# (/run/losos-tunnel/client.json), never baked into the world-readable nix
# store.
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
  lend = config.losos.inference.lend.enable;

  # Runtime, 0700, root. Holds the generated client.json (with the token).
  confDir = "/run/losos-tunnel";

  # What lososd writes (backend/src/io_backend.rs). The env file names the
  # path: LOSOS_EDGE_PATH_URL (the registrar), LOSOS_EDGE_PATH_TUNNEL (the
  # tunnel endpoint) and LOSOS_EDGE_PATH_TUNNEL_PIN (where that edge's
  # certificate is pinned). The marker means "no edge in reach".
  pathEnv = "/run/losos/edge-path.env";
  noneMarker = "/run/losos/edge-none";
  managedMarker = "/run/losos/edge-managed";

  # The name the edge that manages this box calls it by through the tunnel.
  # `.invalid` never resolves, so no browser or public route can ask for it:
  # Traefik on the edge routes a box by its own host names only.
  manageHost = "losos-manage.invalid";

  # Generates client.json at start. The endpoint and the services are
  # templated in; the token is read from its 0600 file so it never lands in
  # the store.
  #
  # The pin happens here too, per edge: the edge makes its tunnel
  # certificate on its first start, and before the tunnel first dials an
  # edge the box fetches the certificate's SHA-256 from that edge's
  # registrar (the TLS-fronted URL it registers with, or the LAN URL it was
  # advertised at) and keeps it, so the tunnel accepts that certificate and
  # no other from then on: trust on first use, nothing to provision by
  # hand. A file already at the pin path (provisioned out of band) is never
  # overwritten, which is also how to pin a certificate you distributed
  # yourself. Anything that is not 64 hex characters is refused rather than
  # written, and the unit fails and retries (Restart=always).
  #
  # A box that lends memory to AI models (modules/inference-lend.nix) serves
  # a second service, the llama.cpp RPC server's port, under the name the
  # edge gives it, with the box's own token.
  genClientConf = pkgs.writeShellScript "losos-tunnel-client-conf" ''
    set -eu
    remote="''${LOSOS_EDGE_PATH_TUNNEL:-${cfg.edgeTunnelEndpoint}}"
    registrar="''${LOSOS_EDGE_PATH_URL:-${cfg.registrarUrl}}"
    pin_file="''${LOSOS_EDGE_PATH_TUNNEL_PIN:-${toString cfg.tunnelPinFile}}"
    if [ ! -s "$pin_file" ]; then
      pin="$(curl -fsS --max-time 20 "$registrar/tunnel-pin")"
      printf '%s' "$pin" | grep -Eq '^[0-9a-f]{64}$' || {
        echo "$registrar returned something that is not a tunnel pin" >&2
        exit 1
      }
      install -d -m 0700 "$(dirname "$pin_file")"
      printf '%s\n' "$pin" > "$pin_file.new"
      mv "$pin_file.new" "$pin_file"
      echo "pinned the tunnel of $registrar into $pin_file"
    fi
    install -d -m 0700 ${confDir}
    umask 077
    jq -n \
      --arg remote "$remote" \
      --arg pin "$(tr -d '[:space:]' < "$pin_file")" \
      --rawfile token ${cfg.tokenFile} \
      '($token | sub("\\s+$"; "")) as $token | {
        remote: $remote,
        pin: $pin,
        services: {
          ${builtins.toJSON cfg.applianceId}: { token: $token, local: "127.0.0.1:80" }
    ${lib.optionalString lend ''
      , ${builtins.toJSON "${cfg.applianceId}:llm"}: { token: $token, local: "127.0.0.1:50052" }
    ''}
        }
      }' > ${confDir}/client.json.new
    mv ${confDir}/client.json.new ${confDir}/client.json
  '';

  # announce flags. --token-file is a path (read at runtime), not the secret,
  # so it's safe to put on the command line / in the store. The registrar URL
  # is systemd's own `${VAR}` expansion of the path lososd wrote, with the
  # configured edge as the unit's Environment= default (EnvironmentFile=
  # overrides Environment=, in that order, by systemd.exec(5)).
  announceArgs = lib.concatStringsSep " " [
    "announce"
    "--registrar-url"
    "\${LOSOS_EDGE_PATH_URL}"
    "--appliance-id"
    (lib.escapeShellArg cfg.applianceId)
    "--hostname"
    (lib.escapeShellArg cfg.hostname)
    "--token-file"
    (lib.escapeShellArg cfg.tokenFile)
    "--heartbeat-interval"
    (lib.escapeShellArg cfg.heartbeatInterval)
    # Read from losos.cluster.* rather than losos.proxy.*: the announce loop is
    # the appliance's only regular conversation with the edge, so the mesh's
    # idle signal rides on it, but the knob belongs to the mesh feature that
    # uses it. An appliance with the proxy on and the mesh off sends the bit
    # and nothing reads it, which costs one boolean per heartbeat.
    "--idle-load-threshold"
    (lib.escapeShellArg (toString config.losos.cluster.idleLoadThreshold))
    # The relay pass lososd fetches from the official edge with the domains
    # view (backend/src/losos.rs, record_view): a local edge forwards it so
    # the official edge routes this box's custom domains through it. Read on
    # every heartbeat; no file sends none.
    "--relay-pass-file"
    "/run/losos/relay-pass"
    # lososd's marker for "the owner let this edge manage the box"
    # (backend/src/io_backend.rs, publish_managed). Read on every heartbeat,
    # so the edge's page lists the box within one interval of the switch.
    "--managed-file"
    managedMarker
  ];

  # What the box lends to models split across boxes, written by
  # losos-llm-rpc while it runs (modules/inference-lend.nix). Read on every
  # heartbeat; no file lends nothing.
  lendArgs = lib.optionalString lend " --llm-lend-file /run/losos-llm/lend";
in
{
  config = lib.mkIf enabled {
    # Hard requirements: the registrar binary must be available,
    # and the secrets must be readable. (We can't assert file *contents* at
    # eval time; we assert the packages exist.)
    assertions = [
      {
        assertion = registrar != null;
        message = "losos.proxy.enable needs losos.proxy.registrar.package (wired by defaults.nix).";
      }
    ];

    # ── tunnel client (outbound, QUIC) ───────────────────────────────────
    systemd.services.losos-tunnel-client = {
      description = "losos tunnel client — outbound QUIC tunnel to the master-proxy edge";
      wantedBy = [ "multi-user.target" ];
      after = [ "network-online.target" ];
      wants = [ "network-online.target" ];
      # The condition is read when the unit starts: lososd stops the unit
      # when it writes the marker, and a later `systemctl restart` from the
      # daemon (the marker gone, a path written) starts it again.
      unitConfig.ConditionPathExists = "!${noneMarker}";
      path = [
        pkgs.curl
        pkgs.coreutils
        pkgs.gnugrep
        pkgs.jq
      ];
      serviceConfig = {
        EnvironmentFile = "-${pathEnv}";
        ExecStartPre = genClientConf;
        ExecStart = "${registrar}/bin/losos-registrar tunnel-client --config ${confDir}/client.json";
        Restart = "always";
        RestartSec = 5;
        PrivateTmp = true;
        NoNewPrivileges = true;
        ProtectSystem = "strict";
        # The pins are written under /var/secrets by ExecStartPre; the
        # configured edge's at losos.proxy.tunnelPinFile, a LAN edge's
        # under losos-edge-pins/ (named by lososd).
        ReadWritePaths = [
          confDir
          "/var/secrets"
        ];
        RuntimeDirectory = "losos-tunnel";
        RuntimeDirectoryMode = "0700";
      };
    };

    # ── the edge that manages this box ───────────────────────────────────
    # A company's own edge may change LosOS Cloud, LosOS Git and the apps,
    # once the owner switched that on for it on the Mesh page
    # (backend/src/edge.rs, "management"). Its calls come through this box's
    # own tunnel, which lands on 127.0.0.1:80, so this server takes loopback
    # only; the front door's LAN guard turns loopback away, which is why it is
    # a server of its own. It tells lososd the call came this way and passes
    # the box's proxy token the edge sends; lososd checks the token and the
    # owner's switch on every call and opens only the sections
    # backend/sections.json lists under `edge`. Nothing a person can send
    # reaches lososd from here: no spare key, no signed-in name.
    services.nginx.virtualHosts.losos-manage =
      lib.mkIf (config.losos.admin.enable && config.services.nginx.enable)
        {
          serverName = manageHost;
          listen = [
            {
              addr = "0.0.0.0";
              port = 80;
            }
          ];
          locations."/".return = "404";
          locations."/api/" = {
            proxyPass = "http://127.0.0.1:${toString config.losos.admin.apiPort}";
            extraConfig = ''
              allow 127.0.0.1;
              deny all;
              proxy_set_header X-Real-IP $remote_addr;
              proxy_set_header X-Losos-Via-Edge 1;
              proxy_set_header Authorization "";
              proxy_set_header Remote-User "";
              proxy_set_header Remote-Groups "";
              proxy_set_header X-Losos-As "";
              proxy_read_timeout 600s;
              proxy_send_timeout 600s;
            '';
          };
        };

    # ── losos-registrar announce (register + heartbeat) ─────────────────
    systemd.services.losos-registrar-announce = {
      description = "losos registrar announce — register + heartbeat with the edge";
      wantedBy = [ "multi-user.target" ];
      after = [ "network-online.target" ];
      wants = [ "network-online.target" ];
      unitConfig.ConditionPathExists = "!${noneMarker}";
      environment.LOSOS_EDGE_PATH_URL = cfg.registrarUrl;
      # The tunnel is up independently; announce just tells the edge
      # to publish the route. Don't hard-depend (announce retries forever).
      serviceConfig = {
        EnvironmentFile = "-${pathEnv}";
        ExecStart = "${registrar}/bin/losos-registrar ${announceArgs}${lendArgs}";
        Restart = "always";
        RestartSec = 5;
        PrivateTmp = true;
        NoNewPrivileges = true;
      };
    };
  };
}
