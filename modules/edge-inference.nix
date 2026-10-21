# Models split across boxes, edge side (losos.edge.inference).
#
# Boxes lend memory (losos.inference.lend, modules/inference-lend.nix): each
# runs a llama.cpp RPC server on its own loopback, and its rathole client
# carries it to this edge as the service `<appliance id>-llm`, which the
# registrar binds on 127.0.0.1 at a port of `portRange`. The registrar groups
# lending boxes until a model of the catalogue fits, and for each group runs
# one head: llama-server over the group's RPC ports, listening on a UNIX
# socket the registrar proxies buyers' requests to and counts tokens on.
#
# The registrar (root) drives the heads through systemd. For a group it
# writes /run/losos-llm-env/<group>.env:
#
#   LLM_MODEL=<the GGUF's store path>
#   LLM_RPC=127.0.0.1:<port>,127.0.0.1:<port>,...
#   LLM_SPLIT=<share>,<share>,...
#   LLM_CTX=<tokens>
#
# then starts losos-llm-head@<group>.service, which answers on
# /run/losos-llm-heads/<group>.sock, and stops it to dissolve the group. A
# head is never restarted by systemd: the registrar decides.
#
# The heads run as `losos-llm`, and an owner rule on loopback lets only that
# user connect to the RPC ports, so nothing else on the edge can drive a
# box's lending server through its tunnel.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  edge = config.losos.edge;
  cfg = edge.inference;

  # What the registrar reads (--inference-models): one object per model,
  # the file as its store path.
  catalogue = pkgs.writeText "inference-models.json" (
    builtins.toJSON (
      map (m: {
        inherit (m)
          id
          name
          needMiB
          context
          priceIn
          priceOut
          licence
          ;
        file = "${m.file}";
      }) cfg.models
    )
  );

  envDir = "/run/losos-llm-env";
  socketDir = "/run/losos-llm-heads";

  rangeSize = r: r.to - r.from + 1;
  ratholeRange =
    let
      bounds = map lib.toInt (lib.splitString "-" edge.ratholePortRange);
    in
    {
      from = builtins.elemAt bounds 0;
      to = builtins.elemAt bounds 1;
    };

  # Only losos-llm (the heads) may open a connection to the RPC ports over
  # loopback; see the box's rule in modules/inference-lend.nix.
  rule = lib.concatStringsSep " " [
    "OUTPUT -o lo -p tcp -d 127.0.0.1"
    "--dport ${toString cfg.portRange.from}:${toString cfg.portRange.to}"
    "-m owner ! --uid-owner losos-llm"
    "-j REJECT --reject-with tcp-reset"
  ];

  ids = map (m: m.id) cfg.models;
in
{
  config = lib.mkIf (edge.enable && cfg.enable) {
    assertions = [
      {
        assertion = cfg.portRange.from <= cfg.portRange.to;
        message = "losos.edge.inference.portRange: `from` is above `to`.";
      }
      {
        assertion = rangeSize cfg.portRange >= rangeSize ratholeRange;
        message = "losos.edge.inference.portRange holds ${toString (rangeSize cfg.portRange)} ports, fewer than the ${toString (rangeSize ratholeRange)} of losos.edge.ratholePortRange: every box needs one.";
      }
      {
        assertion = cfg.portRange.to < ratholeRange.from || cfg.portRange.from > ratholeRange.to;
        message = "losos.edge.inference.portRange overlaps losos.edge.ratholePortRange.";
      }
      {
        assertion = lib.length ids == lib.length (lib.unique ids);
        message = "losos.edge.inference.models: two models have the same id.";
      }
    ];

    environment.etc."losos/inference-models.json".source = catalogue;
    # The registrar reads the catalogue once, at start.
    systemd.services.losos-registrar.restartTriggers = [ catalogue ];

    users.users.losos-llm = {
      isSystemUser = true;
      group = "losos-llm";
      description = "Heads of models split across boxes";
    };
    users.groups.losos-llm = { };

    systemd.tmpfiles.rules = [
      "d ${envDir} 0755 root root - -"
      "d ${socketDir} 0750 losos-llm losos-llm - -"
    ];

    networking.firewall = lib.mkIf config.networking.firewall.enable {
      extraCommands = ''
        iptables -w -D ${rule} 2>/dev/null || true
        iptables -w -I ${rule}
      '';
      extraStopCommands = ''
        iptables -w -D ${rule} 2>/dev/null || true
      '';
    };

    systemd.services."losos-llm-head@" = {
      description = "Head of a model split across boxes, group %i";
      after = [ "network.target" ];
      serviceConfig = {
        EnvironmentFile = "${envDir}/%i.env";
        # A socket left by a head that did not stop cleanly would make the
        # bind fail.
        ExecStartPre = "${pkgs.coreutils}/bin/rm -f ${socketDir}/%i.sock";
        ExecStart = lib.concatStringsSep " " [
          "${cfg.package}/bin/llama-server"
          "-m \${LLM_MODEL}"
          "--rpc \${LLM_RPC}"
          "-ngl 999"
          "-ts \${LLM_SPLIT}"
          "-c \${LLM_CTX}"
          "-np 1"
          "--no-webui"
          "--host ${socketDir}/%i.sock"
        ];
        ExecStopPost = "${pkgs.coreutils}/bin/rm -f ${socketDir}/%i.sock";
        Restart = "no";
        User = "losos-llm";
        Group = "losos-llm";
        ReadWritePaths = [ socketDir ];
        UMask = "0007";

        CapabilityBoundingSet = "";
        AmbientCapabilities = "";
        NoNewPrivileges = true;
        ProtectSystem = "strict";
        ProtectHome = true;
        PrivateTmp = true;
        PrivateDevices = true;
        PrivateIPC = true;
        ProtectProc = "invisible";
        ProtectKernelTunables = true;
        ProtectKernelModules = true;
        ProtectKernelLogs = true;
        ProtectControlGroups = true;
        ProtectClock = true;
        ProtectHostname = true;
        # TCP to the boxes' RPC ports on loopback, and its own socket.
        RestrictAddressFamilies = [
          "AF_INET"
          "AF_UNIX"
        ];
        IPAddressAllow = "localhost";
        IPAddressDeny = "any";
        RestrictNamespaces = true;
        RestrictRealtime = true;
        RestrictSUIDSGID = true;
        LockPersonality = true;
        MemoryDenyWriteExecute = true;
        RemoveIPC = true;
        SystemCallArchitectures = "native";
        SystemCallFilter = [
          "@system-service"
          "~@privileged"
        ];
        SystemCallErrorNumber = "EPERM";
      };
    };
  };
}
