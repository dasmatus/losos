# Lending memory to models split across boxes (losos.inference.lend).
#
# A llama.cpp RPC server (`ggml-rpc-server`) holds this box's share of a
# model's layers and computes on them for a head that runs on the edge
# (modules/edge-inference.nix). It listens on 127.0.0.1:50052 only. The edge
# reaches it through this box's own rathole tunnel: the client carries a
# second service, `<appliance id>-llm`, beside the box's main one
# (modules/proxy.nix), and the announce heartbeat tells the edge how much
# memory the box lends by reading /run/losos-llm/lend.
#
# The head is not fully trusted, so the server is fenced in three ways:
#
#   * it runs as `shared` (uid 1001), the mesh-storage user, never as the
#     owner's `notshared`, in a sandbox that sees neither home, no devices
#     and no address but loopback;
#   * MemoryMax is the lent memory plus a little for the server itself, so a
#     head that asks for more gets this unit killed, and the kernel's own OOM
#     killer picks it before anything of the owner's;
#   * an owner rule on loopback lets only root (rathole's client) connect to
#     the port, so nothing else on the box, the pods and `shared` included,
#     can talk to the server directly.
#
# `-d CPU` picks the CPU device by name. Left to choose, rpc-server prefers
# any accelerator it loaded, which on this build is the BLAS backend, and
# BLAS alone cannot compute a whole graph.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.losos.inference.lend;

  # modules/proxy.nix points the `-llm` tunnel service at the same port.
  port = 50052;
  stateDir = "losos-llm-rpc";
  lendFile = "/run/losos-llm/lend";

  # Room for the server itself on top of the memory it lends: its code, the
  # thread stacks and the graph it is computing.
  overheadMiB = 256;

  # The MiB lent and nothing else, readable by every user (announce reads
  # it). Gone with the runtime directory when the unit stops.
  writeLend = pkgs.writeShellScript "losos-llm-lend" ''
    printf '%s' ${toString cfg.memoryMiB} > ${lendFile}.new
    chmod 0644 ${lendFile}.new
    mv ${lendFile}.new ${lendFile}
  '';

  # Only root may open a connection to the RPC port over loopback; see the
  # same rule for lososd's API in modules/daemon.nix. `-I` so it sits ahead
  # of anything appended later, the delete first so a reload is idempotent.
  rule = lib.concatStringsSep " " [
    "OUTPUT -o lo -p tcp -d 127.0.0.1 --dport ${toString port}"
    "-m owner ! --uid-owner 0"
    "-j REJECT --reject-with tcp-reset"
  ];
in
{
  config = lib.mkIf cfg.enable {
    networking.firewall = lib.mkIf config.networking.firewall.enable {
      extraCommands = ''
        iptables -w -D ${rule} 2>/dev/null || true
        iptables -w -I ${rule}
      '';
      extraStopCommands = ''
        iptables -w -D ${rule} 2>/dev/null || true
      '';
    };

    systemd.services.losos-llm-rpc = {
      description = "Lent memory for models split across boxes (llama.cpp RPC server)";
      wantedBy = [ "multi-user.target" ];
      after = [ "network.target" ];
      # The file cache (-c) keeps the tensors a head has sent once, so the
      # next group that loads the same model sends hashes instead. Under
      # /var/lib, which is on /persist.
      environment.LLAMA_CACHE = "/var/lib/${stateDir}";
      serviceConfig = {
        ExecStart = lib.concatStringsSep " " (
          [
            "${cfg.package}/bin/ggml-rpc-server"
            "-d CPU"
            "-H 127.0.0.1"
            "-p ${toString port}"
            "-c"
          ]
          ++ lib.optional (cfg.threads != null) "-t ${toString cfg.threads}"
        );
        ExecStartPost = writeLend;
        User = "shared";
        Group = "shared";
        StateDirectory = stateDir;
        StateDirectoryMode = "0700";
        RuntimeDirectory = "losos-llm";
        RuntimeDirectoryMode = "0755";
        Restart = "always";
        RestartSec = 10;

        MemoryMax = "${toString (cfg.memoryMiB + overheadMiB)}M";
        MemorySwapMax = 0;
        OOMScoreAdjust = 1000;
        CPUWeight = 20;
        IOWeight = 20;

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
