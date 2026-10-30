# Mesh CI: the GitHub Actions workflows of LosOS Git's repositories, run on
# the boxes of a mesh (`losos-ci`, backend/src/ci/, a binary of lososd's
# crate). Only while lososInternal.ci.live: the shared side and LosOS Git.
#
# Two services:
#
#   losos-ci          as `losos-ci`. Takes LosOS Git's pushes and pull
#                     requests, plans the runs, hands jobs to the edge's
#                     queue or keeps them on the box, follows them and
#                     writes each job's check on the commit. It alone holds
#                     the edge token (a credential) and reads commits
#                     (LosOS Git's internal socket, group losos-git-peers).
#   losos-ci-runner   as `shared`, the mesh user. Asks the daemon for one
#                     job at a time, its own box's first, then other boxes'
#                     through the edge, and runs it in rootless Podman
#                     through pm (github.com/losos-project/pm): the runner
#                     writes each job's containers as compose files and pm
#                     starts and stops them with Mesh CI's own pm plugin
#                     (backend/plugins/pm-ci/), which answers for those
#                     containers alone. Memory, processor and process
#                     limits, a low processor share. It sees no token and no
#                     repository but the one the job checked out. A step
#                     `uses: losos-project/pm` builds a signed pm recipe in
#                     pm's own jail.
#
# Sockets, each in a directory that is its gate:
#
#   /run/losos-ci-events/events.sock   LosOS Git (group losos-ci-events)
#   /run/losos-ci-runner/runner.sock   the runner (group shared)
#   /run/losos-ci/api.sock             lososd: its `ci` plugin, and joined
#                                      LosOS Desktop computers' jobs (root)
#
# A job of another box must not reach this box's network: an owner rule
# refuses every new connection of uid `shared` to a private, link-local or
# loopback address, DNS aside. That is the runner's pods (rootless Podman's
# network is a process of the user) and the shared side's own services,
# which only answer on loopback and connect out to the internet.
#
# /var/lib is on /persist through impermanence's whole-/var entry.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.losos.ci;
  live = config.lososInternal.ci.live;
  pkg = config.losos.backend.package;
  proxy = config.losos.proxy;
  window = config.losos.cluster.computeWindow;

  # The account the shared side runs as (lososInternal.sideUser): `shared`
  # in a mesh, `kindashared` with the local gateways only.
  user = if config.lososInternal.sideUser == null then "shared" else config.lososInternal.sideUser;
  shared = config.users.users.${user};
  sharedUid = toString shared.uid;
  subIds = if user == "shared" then 300000 else 400000;
  runtimeDir = "/run/user/${sharedUid}";
  runnerRoot = "/var/lib/losos-ci-runner";

  eventsDir = "/run/losos-ci-events";
  runnerDir = "/run/losos-ci-runner";
  pageDir = "/run/losos-ci";
  pageSocket = "${pageDir}/api.sock";

  podmanPkg = config.virtualisation.podman.package;

  storageConf = pkgs.writeText "losos-ci-storage.conf" ''
    [storage]
    driver = "overlay"
    graphroot = "${runnerRoot}/storage"
    runroot = "${runtimeDir}/losos-ci-containers"
  '';

  podmanEnv = {
    XDG_RUNTIME_DIR = runtimeDir;
    DBUS_SESSION_BUS_ADDRESS = "unix:path=${runtimeDir}/bus";
    CONTAINERS_STORAGE_CONF = "${storageConf}";
    HOME = runnerRoot;
    XDG_CONFIG_HOME = "${cfg.pmPlugin}";
  };

  # Every Mesh CI container a runner left behind, at its stop, through pm
  # like everything else the runner does to them.
  cleanContainers = pkgs.writeShellScript "losos-ci-runner-clean" ''
    export XDG_CONFIG_HOME=${cfg.pmPlugin} RUST_LOG=warn
    ${cfg.pm}/bin/pm container list | while read -r name; do
      ${cfg.pm}/bin/pm container remove "$name" || true
    done
  '';

  # Private, link-local, shared-address-space and loopback destinations.
  lan4 = [
    "10.0.0.0/8"
    "172.16.0.0/12"
    "192.168.0.0/16"
    "169.254.0.0/16"
    "100.64.0.0/10"
    "127.0.0.0/8"
  ];
  lan6 = [
    "fc00::/7"
    "fe80::/10"
    "::1/128"
  ];
  # New connections only: the shared side's servers still answer. DNS goes
  # to systemd-resolved on 127.0.0.53, so port 53 stays open.
  rule =
    dst:
    lib.concatStringsSep " " [
      "OUTPUT -d ${dst}"
      "-m owner --uid-owner ${sharedUid}"
      "-m conntrack --ctstate NEW"
      "-p tcp ! --dport 53"
      "-j REJECT --reject-with tcp-reset"
    ];
  ruleUdp =
    dst:
    lib.concatStringsSep " " [
      "OUTPUT -d ${dst}"
      "-m owner --uid-owner ${sharedUid}"
      "-m conntrack --ctstate NEW"
      "-p udp ! --dport 53"
      "-j REJECT"
    ];
  rules =
    tool: dsts:
    lib.concatMap (
      d:
      map (r: "${tool} -w %s ${r d}") [
        rule
        ruleUdp
      ]
    ) dsts;
  allRules = rules "iptables" lan4 ++ rules "ip6tables" lan6;
  apply =
    verb: suffix:
    lib.concatMapStringsSep "\n" (r: lib.replaceStrings [ "%s" ] [ verb ] r + suffix) allRules;
in
{
  config = lib.mkIf live {
    users.groups.losos-ci = { };
    users.groups.losos-ci-events = { };
    users.users.losos-ci = {
      isSystemUser = true;
      group = "losos-ci";
      # LosOS Git's internal socket (modules/git.nix).
      extraGroups = [ "losos-git-peers" ];
      description = "LosOS Mesh CI";
    };
    # LosOS Git sends its events through the socket in this group's directory.
    users.users.losos-git.extraGroups = [ "losos-ci-events" ];

    # Rootless Podman as the side's account: sub-ids of its own, apart from
    # `notshared`'s (modules/apps.nix) and from the other sharing account's,
    # and a user manager from boot.
    virtualisation.podman.enable = true;
    users.users.${user} = {
      linger = true;
      subUidRanges = [
        {
          startUid = subIds;
          count = 65536;
        }
      ];
      subGidRanges = [
        {
          startGid = subIds;
          count = 65536;
        }
      ];
    };

    systemd.tmpfiles.rules = [
      "d ${eventsDir} 0750 losos-ci losos-ci-events - -"
      "d ${runnerDir} 0750 losos-ci ${user} - -"
      "d ${pageDir} 0700 losos-ci losos-ci - -"
      "d ${runnerRoot} 0700 ${user} ${user} - -"
      "d ${runnerRoot}/storage 0700 ${user} ${user} - -"
      "d ${runnerRoot}/jobs 0700 ${user} ${user} - -"
    ];

    networking.firewall = lib.mkIf config.networking.firewall.enable {
      extraCommands = ''
        ${apply "-D" " 2>/dev/null || true"}
        ${apply "-I" ""}
      '';
      extraStopCommands = ''
        ${apply "-D" " 2>/dev/null || true"}
      '';
    };

    # lososd's `ci` plugin relays the admin pages here; /api/me lists `ci`.
    systemd.services.lososd.environment.LOSOS_CI_SOCKET = pageSocket;

    # Podman runs a service's health check from a timer in the user manager,
    # with the manager's environment rather than the runner's: without the
    # same storage it finds no container and the service never turns
    # healthy. The manager hands this on to everything it starts.
    systemd.services."user@${sharedUid}" = {
      overrideStrategy = "asDropin";
      environment.CONTAINERS_STORAGE_CONF = "${storageConf}";
    };

    systemd.services.losos-ci = {
      description = "LosOS Mesh CI";
      wantedBy = [ "multi-user.target" ];
      after = [
        "network-online.target"
        "systemd-tmpfiles-setup.service"
      ];
      wants = [ "network-online.target" ];
      # LosOS Git connects to the events socket per event; it waits for
      # nothing of ours.
      before = [ "losos-git.service" ];
      path = [ pkgs.curl ];
      environment = {
        RUST_LOG = "info";
        LOSOS_CI_APPLIANCE_ID = proxy.applianceId;
        LOSOS_CI_REGISTRAR_URL = lib.optionalString proxy.enable proxy.registrarUrl;
        LOSOS_EDGE_PATH_FILE = "/run/losos/edge-path.env";
        LOSOS_EDGE_NONE_FILE = "/run/losos/edge-none";
      };
      serviceConfig = {
        User = "losos-ci";
        Group = "losos-ci";
        ExecStart = lib.escapeShellArgs (
          [
            "${pkg}/bin/losos-ci"
            "serve"
            "--state-dir"
            "/var/lib/losos-ci"
            "--events-socket"
            "${eventsDir}/events.sock"
            "--runner-socket"
            "${runnerDir}/runner.sock"
            "--page-socket"
            pageSocket
            "--git-socket"
            "/run/losos-git-peers/internal.sock"
            "--server-url"
            "https://${config.losos.hostName}.local/git"
            "--window"
            "${window.start}-${window.end}"
          ]
          ++ lib.optional cfg.privateOnMesh "--private-on-mesh"
        );
        # The edge token, readable by this unit alone. No edge configured,
        # no token: jobs stay on the box.
        LoadCredential = lib.optional proxy.enable "edge-token:${toString proxy.tokenFile}";
        Restart = "on-failure";
        RestartSec = "2s";
        StateDirectory = "losos-ci";
        StateDirectoryMode = "0700";
        ReadWritePaths = [
          eventsDir
          runnerDir
          pageDir
        ];
        UMask = "0077";

        NoNewPrivileges = true;
        CapabilityBoundingSet = "";
        PrivateTmp = true;
        PrivateDevices = true;
        ProtectSystem = "strict";
        ProtectHome = true;
        ProtectKernelTunables = true;
        ProtectKernelModules = true;
        ProtectKernelLogs = true;
        ProtectControlGroups = true;
        ProtectClock = true;
        ProtectHostname = true;
        ProtectProc = "invisible";
        RestrictAddressFamilies = [
          "AF_UNIX"
          "AF_INET"
          "AF_INET6"
          "AF_NETLINK"
        ];
        RestrictNamespaces = true;
        RestrictRealtime = true;
        RestrictSUIDSGID = true;
        LockPersonality = true;
        MemoryDenyWriteExecute = true;
        SystemCallArchitectures = "native";
        SystemCallFilter = [
          "@system-service"
          "~@privileged"
        ];
      };
    };

    systemd.services.losos-ci-runner = {
      description = "LosOS Mesh CI runner, as the ${user} user";
      wantedBy = [ "multi-user.target" ];
      wants = [
        "losos-ci.service"
        "user@${sharedUid}.service"
      ];
      after = [
        "losos-ci.service"
        "user@${sharedUid}.service"
        "systemd-tmpfiles-setup.service"
      ]
      ++ lib.optional config.losos.${user}.fscrypt.enable "losos-fscrypt-${user}.service";
      environment = podmanEnv // {
        RUST_LOG = "info";
      };
      # newuidmap and newgidmap are setuid wrappers; podman finds conmon,
      # crun and pasta through its own wrapper.
      #
      # pm's build jail runs a recipe's programs by their resolved path, and
      # nixpkgs' coreutils is one binary behind symlinks that reads its name
      # from argv[0]: `install -d` would reach it as `coreutils -d`. A
      # coreutils of separate programs, ahead of the one systemd adds.
      path = [
        "/run/wrappers"
        (pkgs.coreutils.override { singleBinary = false; })
        podmanPkg
        pkgs.podman-compose
        pkgs.curl
        pkgs.gnutar
        pkgs.gzip
        # pm packs a recipe's .cpkg with `tar -cJf`.
        pkgs.xz
      ];
      serviceConfig = {
        User = user;
        Group = user;
        ExecStart = lib.escapeShellArgs (
          [
            "${pkg}/bin/losos-ci"
            "runner"
            "--socket"
            "${runnerDir}/runner.sock"
            "--work-dir"
            "${runnerRoot}/jobs"
            "--name"
            config.losos.proxy.applianceId
            "--default-image"
            cfg.pmPlugin.baseImageName
            "--pm"
            "${cfg.pm}/bin/pm"
            "--pm-config"
            "${cfg.pmPlugin}"
            "--memory"
            cfg.runner.memory
            "--cpus"
            cfg.runner.cpus
            "--node"
            "${pkgs.nodejs_22}/bin/node"
          ]
          ++ lib.concatMap (i: [
            "--image"
            i
          ]) cfg.runner.images
        );
        ExecStopPost = "${cleanContainers}";
        Restart = "always";
        RestartSec = "5s";
        # The runner itself is small; the pods live in the user manager.
        CPUWeight = 20;
        IOWeight = 20;
        OOMScoreAdjust = 500;
      };
    };
  };
}
