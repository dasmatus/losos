# The edge as a way out to the internet (losos.edge.exit.enable,
# handbook/docs/in-depth/edge-exit.md).
#
# A tenant marked `exit` (losos.edge.tenants.<id>.exit) enrols a WireGuard
# key with the registrar's `POST /exit` and gets one address on the
# `losos-exit` interface. What it sends there leaves from the edge's public
# address through NAT, the way a VPN's traffic does. LosOS Desktop's network
# machine is the caller.
#
# The registrar holds no WireGuard state of its own: it writes the enrolled
# peers to /var/lib/losos-exit/peers.json (edge/registrar/src/exit.rs), and
# losos-exit-sync, run by a path unit whenever the file changes, puts them
# on the interface and takes the ones that left off it. The edge's private
# key stays in /var/lib/losos-exit-key, which only root reads; its public
# half is written beside the peers for the registrar to hand out.
#
# With onion.enable the same enrolment also opens `exit-socks`, a SOCKS5
# server behind an onion service of the edge's own, for a computer whose
# traffic goes through Tor first (LosOS Desktop's schizo mode). It runs as
# losos-exit, reads the peers file for the password digests and nothing
# else, and connects only to public addresses.
{
  config,
  lib,
  pkgs,
  utils,
  ...
}:

let
  cfg = config.losos.edge.exit;
  on = config.losos.edge.enable && cfg.enable;
  registrar = config.losos.edge.registrar.package;

  dir = "/var/lib/losos-exit";
  keyDir = "/var/lib/losos-exit-key";
  peersFile = "${dir}/peers.json";
  publicKeyFile = "${dir}/public.key";

  # The edge takes the subnet's first address. The registrar refuses a
  # subnet with host bits set, so the base ends in a zero to add one to.
  subnetParts = lib.splitString "/" cfg.subnet;
  base = lib.head subnetParts;
  prefix = lib.last subnetParts;
  octets = lib.splitString "." base;
  first = lib.concatStringsSep "." (
    lib.take 3 octets ++ [ (toString (lib.toInt (lib.last octets) + 1)) ]
  );

  # Never forwarded from the exit: what is not the internet. The edge's own
  # networks, a cloud's metadata service and another tenant on the same
  # interface would otherwise all be one hop away from every enrolled peer.
  closed = [
    "0.0.0.0/8"
    "10.0.0.0/8"
    "100.64.0.0/10"
    "127.0.0.0/8"
    "169.254.0.0/16"
    "172.16.0.0/12"
    "192.0.0.0/24"
    "192.0.2.0/24"
    "192.168.0.0/16"
    "198.18.0.0/15"
    "198.51.100.0/24"
    "203.0.113.0/24"
    "224.0.0.0/4"
    "240.0.0.0/4"
  ];
  chain = "losos-exit-fwd";

  sync = pkgs.writeShellApplication {
    name = "losos-exit-sync";
    runtimeInputs = [
      pkgs.jq
      pkgs.wireguard-tools
      pkgs.coreutils
    ];
    text = ''
      # Make the interface's peers what peers.json says: add or move the
      # enrolled ones, remove the rest. A missing file is no peers.
      want=$(jq -r 'to_entries[] | "\(.value.public_key) \(.value.address)"' ${peersFile} 2>/dev/null || true)
      have=$(wg show losos-exit peers)
      while read -r key address; do
        [ -n "$key" ] || continue
        wg set losos-exit peer "$key" allowed-ips "$address/32"
      done <<<"$want"
      for key in $have; do
        grep -q "^$key " <<<"$want" || wg set losos-exit peer "$key" remove
      done
    '';
  };
in
{
  config = lib.mkIf on (
    lib.mkMerge [
      {
        users.users.losos-exit = {
          isSystemUser = true;
          group = "losos-exit";
          description = "The edge's SOCKS exit";
        };
        users.groups.losos-exit = { };

        # Setgid, so peers.json, which the registrar writes as root, is
        # readable by exit-socks's group and nobody else.
        systemd.tmpfiles.rules = [
          "d ${dir} 2750 root losos-exit - -"
          "d ${keyDir} 0700 root root - -"
        ];

        networking.wireguard.interfaces.losos-exit = {
          ips = [ "${first}/${prefix}" ];
          listenPort = cfg.port;
          privateKeyFile = "${keyDir}/private";
          generatePrivateKeyFile = true;
        };

        systemd.services.losos-exit-public-key = {
          description = "Write the exit's WireGuard public key for the registrar";
          wantedBy = [ "multi-user.target" ];
          requires = [ "wireguard-losos-exit.service" ];
          after = [
            "wireguard-losos-exit.service"
            "systemd-tmpfiles-setup.service"
          ];
          before = [ "losos-registrar.service" ];
          serviceConfig = {
            Type = "oneshot";
            RemainAfterExit = true;
            UMask = "0027";
          };
          script = ''
            ${pkgs.wireguard-tools}/bin/wg pubkey <${keyDir}/private >${publicKeyFile}.tmp
            ${pkgs.coreutils}/bin/mv ${publicKeyFile}.tmp ${publicKeyFile}
          '';
        };

        systemd.services.losos-exit-sync = {
          description = "Put the exit's enrolled peers on its interface";
          requires = [ "wireguard-losos-exit.service" ];
          # Again after the interface comes back: a restarted interface
          # starts with no peers.
          after = [ "wireguard-losos-exit.service" ];
          wantedBy = [ "wireguard-losos-exit.service" ];
          serviceConfig = {
            Type = "oneshot";
            ExecStart = lib.getExe sync;
          };
        };
        systemd.paths.losos-exit-sync = {
          wantedBy = [ "paths.target" ];
          pathConfig.PathChanged = peersFile;
        };

        networking.nat = {
          enable = true;
          internalInterfaces = [ "losos-exit" ];
        };

        networking.firewall = {
          allowedUDPPorts = [ cfg.port ];
          # Out to the internet only, and never peer to peer. IPv6 is not
          # handed out, so nothing of it is forwarded either.
          extraCommands = ''
            iptables -w -D FORWARD -i losos-exit -j ${chain} 2>/dev/null || true
            iptables -w -F ${chain} 2>/dev/null || iptables -w -N ${chain}
            ${lib.concatMapStrings (net: ''
              iptables -w -A ${chain} -d ${net} -j DROP
            '') closed}
            iptables -w -A ${chain} -o losos-exit -j DROP
            iptables -w -I FORWARD -i losos-exit -j ${chain}
            ip6tables -w -D FORWARD -i losos-exit -j DROP 2>/dev/null || true
            ip6tables -w -I FORWARD -i losos-exit -j DROP
          '';
          extraStopCommands = ''
            iptables -w -D FORWARD -i losos-exit -j ${chain} 2>/dev/null || true
            iptables -w -F ${chain} 2>/dev/null || true
            iptables -w -X ${chain} 2>/dev/null || true
            ip6tables -w -D FORWARD -i losos-exit -j DROP 2>/dev/null || true
          '';
        };

        systemd.services.losos-registrar = {
          wants = [ "losos-exit-public-key.service" ];
          after = [ "losos-exit-public-key.service" ];
        };
      }

      (lib.mkIf cfg.onion.enable {
        services.tor = {
          enable = true;
          relay.onionServices.losos-exit.map = [
            {
              port = cfg.onion.port;
              target = {
                addr = "127.0.0.1";
                port = cfg.socksPort;
              };
            }
          ];
        };

        systemd.services.losos-registrar = {
          wants = [ "tor.service" ];
          after = [ "tor.service" ];
        };

        systemd.services.losos-exit-socks = {
          description = "SOCKS5 exit for enrolled tenants, behind the edge's onion";
          wantedBy = [ "multi-user.target" ];
          after = [ "network.target" ];
          serviceConfig = {
            ExecStart = utils.escapeSystemdExecArgs [
              "${registrar}/bin/losos-registrar"
              "exit-socks"
              "--listen"
              "127.0.0.1:${toString cfg.socksPort}"
              "--peers"
              peersFile
            ];
            User = "losos-exit";
            Group = "losos-exit";
            Restart = "always";
            RestartSec = 5;
            # The server checks every destination itself; this is the
            # second layer. Loopback stays open for Tor to reach it, and
            # names are looked up through nscd's socket, so a resolver on
            # the cloud's private network is not needed from here.
            IPAddressDeny = closed ++ [
              "::/127"
              "fc00::/7"
              "fe80::/10"
              "ff00::/8"
            ];
            IPAddressAllow = [ "127.0.0.0/8" ];
            ReadOnlyPaths = [ dir ];
            NoNewPrivileges = true;
            PrivateTmp = true;
            PrivateDevices = true;
            ProtectSystem = "strict";
            ProtectHome = true;
            ProtectKernelTunables = true;
            ProtectKernelModules = true;
            ProtectControlGroups = true;
            RestrictAddressFamilies = [
              "AF_INET"
              "AF_INET6"
              "AF_UNIX"
            ];
            CapabilityBoundingSet = "";
            LockPersonality = true;
            MemoryDenyWriteExecute = true;
            SystemCallFilter = [ "@system-service" ];
          };
        };
      })
    ]
  );
}
