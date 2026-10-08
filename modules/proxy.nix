# Master proxy — appliance side.
#
# The appliance keeps its no-SSH/no-public-ports invariant: it opens zero
# inbound ports. A rathole client dials out to an edge; the edge rathole
# server forwards public traffic back over that tunnel to this box's Nginx
# on :80 (the slave proxy). A losos-registrar `announce` service registers
# this appliance with the edge (POST /register) and heartbeats so the edge
# keeps the Traefik route alive.
#
# Which edge: lososd decides (backend/src/edge.rs, "the path"), and tells
# the two units here through /run/losos/edge-path.env — an edge found on the
# LAN first, the configured official one (losos.proxy.registrarUrl,
# losos.proxy.edgeRatholeEndpoint) when the LAN has none, and when neither
# answers it writes /run/losos/edge-none and stops both units, so a box with
# no edge in reach runs no tunnel at all. Both files live on /run and are
# gone at boot: until lososd's first scan the units dial the configured
# edge, which is also what they do on a box that runs no lososd
# (tests/edge-vm.nix). The daemon only writes the files when
# losos.proxy.enable is on; see modules/daemon.nix.
#
# See docs/superpowers/specs/2026-08-17-master-proxy-design.md, the edge
# side in modules/edge.nix, and wiki/Edge-Federation.md for the path rule.
# Replaces the retired losos.cfd (Cloudflare).
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

  # What lososd writes (backend/src/io_backend.rs). The env file names the
  # path: LOSOS_EDGE_PATH_URL (the registrar), LOSOS_EDGE_PATH_RATHOLE (the
  # tunnel endpoint) and LOSOS_EDGE_PATH_NOISE_PUB (where that edge's Noise
  # key is pinned). The marker means "no edge in reach".
  pathEnv = "/run/losos/edge-path.env";
  noneMarker = "/run/losos/edge-none";

  # Generates client.toml at start from the secret files. The non-secret
  # parts (remote_addr, service name, local_addr) are templated in; the
  # secrets are read from their 0600 files so no token ever lands in the
  # store. rathole auto-detects [client] mode from the file.
  #
  # The Noise pin happens here too, per edge: the edge generates its own
  # keypair, and before the tunnel first dials an edge the box fetches the
  # public half from that edge's registrar (the TLS-fronted URL it registers
  # with, or the LAN URL it was advertised at) and keeps it, so the tunnel is
  # pinned to that key from then on: trust on first use, nothing to provision
  # by hand. A file already at the pin path (provisioned out of band) is
  # never overwritten, which is also how to pin a key you distributed
  # yourself. A key that does not look like 32 base64 bytes is refused rather
  # than written, and the unit fails and retries (Restart=always).
  genClientConf = pkgs.writeShellScript "losos-rathole-client-conf" (
    ''
          set -eu
          remote="''${LOSOS_EDGE_PATH_RATHOLE:-${cfg.edgeRatholeEndpoint}}"
          registrar="''${LOSOS_EDGE_PATH_URL:-${cfg.registrarUrl}}"
          install -d -m 0700 ${confDir}
          umask 077
          bootstrap="$(cat ${cfg.bootstrapTokenFile})"
          token="$(cat ${cfg.tokenFile})"
          cat > ${confDir}/client.toml <<EOF
      [client]
      remote_addr = "$remote"
      default_token = "$bootstrap"

      [client.services.${cfg.applianceId}]
      token = "$token"
      local_addr = "127.0.0.1:80"
      EOF
    ''
    + lib.optionalString (cfg.noisePublicKeyFile != null) ''
          # Noise (NK): the tunnel is encrypted to the edge's public key, read at
          # runtime like the tokens so no key material is in the store.
          pin="''${LOSOS_EDGE_PATH_NOISE_PUB:-${toString cfg.noisePublicKeyFile}}"
          if [ ! -s "$pin" ]; then
            key="$(curl -fsS --max-time 20 "$registrar/noise-public-key")"
            printf '%s' "$key" | grep -Eq '^[A-Za-z0-9+/]{43}=$' || {
              echo "$registrar returned something that is not an X25519 public key" >&2
              exit 1
            }
            install -d -m 0700 "$(dirname "$pin")"
            printf '%s\n' "$key" > "$pin.new"
            mv "$pin.new" "$pin"
            echo "pinned the Noise public key of $registrar into $pin"
          fi
          pub="$(tr -d '[:space:]' < "$pin")"
          cat >> ${confDir}/client.toml <<EOF

      [client.transport]
      type = "noise"

      [client.transport.noise]
      remote_public_key = "$pub"
      EOF
    ''
  );

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
      # The condition is read when the unit starts: lososd stops the unit
      # when it writes the marker, and a later `systemctl restart` from the
      # daemon (the marker gone, a path written) starts it again.
      unitConfig.ConditionPathExists = "!${noneMarker}";
      path = [
        pkgs.curl
        pkgs.coreutils
        pkgs.gnugrep
      ];
      serviceConfig = {
        EnvironmentFile = "-${pathEnv}";
        ExecStartPre = genClientConf;
        ExecStart = "${rathole}/bin/rathole -c ${confDir}/client.toml";
        Restart = "always";
        RestartSec = 5;
        PrivateTmp = true;
        NoNewPrivileges = true;
        ProtectSystem = "strict";
        # The pins are written under /var/secrets by ExecStartPre; the
        # configured edge's at losos.proxy.noisePublicKeyFile, a LAN edge's
        # under losos-edge-pins/ (named by lososd).
        ReadWritePaths = [
          confDir
          "/var/secrets"
        ];
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
      unitConfig.ConditionPathExists = "!${noneMarker}";
      environment.LOSOS_EDGE_PATH_URL = cfg.registrarUrl;
      # The rathole tunnel is up independently; announce just tells the edge
      # to publish the route. Don't hard-depend (announce retries forever).
      serviceConfig = {
        EnvironmentFile = "-${pathEnv}";
        ExecStart = "${registrar}/bin/losos-registrar ${announceArgs}";
        Restart = "always";
        RestartSec = 5;
        PrivateTmp = true;
        NoNewPrivileges = true;
      };
    };
  };
}
