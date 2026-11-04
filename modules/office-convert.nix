# LosOS Office's conversions: a document to Word, OpenDocument, PDF, a web
# page or PowerPoint, and a Word or OpenDocument file back to a document.
# Pandoc does them, in a rootless Podman container started for each one.
#
#   browser ── POST /api/office/export|import/<format> ──► nginx ──► lososd
#     office plugin (backend/plugins/office) ── HTTP ──► /run/losos-convert.sock
#       losos-convert@<n> (one per connection, user losos-convert):
#         podman run --rm --network=none --read-only … losos-convert export|import
#
# What is load-bearing here:
#
#   * The socket is root's, 0600: lososd is its only client, and lososd
#     reaches it only through the plugin's manifest.
#   * The container has no network, no capabilities, a read-only image and a
#     512 MiB tmpfs to work in. Its root is losos-convert itself (no sub-ids:
#     every file in the image is root's, so one uid is the whole mapping),
#     and Podman runs without its own cgroups, so the container stays in the
#     connection's unit, under its memory, task and time limits.
#   * The image comes from the Nix store (`.#losos-image-convert`), loaded
#     once per build; nothing is ever pulled.
#   * What a document says has been through losos-mdx's allowlist before
#     Pandoc reads it, and a web page goes out with no script.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.losos.office.convert;
  on = cfg.enable && cfg.package != null && cfg.image != null && config.losos.backend.plugins != null;

  home = "/var/lib/losos-convert";
  runtimeDir = "/run/losos-convert";
  socket = "/run/losos-convert.sock";
  podman = config.virtualisation.podman.package;
  image = "localhost/${cfg.image.imageName}:${cfg.image.imageTag}";

  storageConf = pkgs.writeText "losos-convert-storage.conf" ''
    [storage]
    driver = "overlay"
    graphroot = "${home}/storage"
    runroot = "${runtimeDir}/containers"
  '';

  podmanEnv = {
    HOME = home;
    XDG_RUNTIME_DIR = runtimeDir;
    CONTAINERS_STORAGE_CONF = "${storageConf}";
  };

  sandbox = {
    User = "losos-convert";
    Group = "losos-convert";
    NoNewPrivileges = true;
    PrivateNetwork = true;
    PrivateTmp = true;
    ProtectSystem = "strict";
    ProtectHome = true;
    ReadWritePaths = [
      home
      runtimeDir
    ];
    ProtectKernelModules = true;
    # No ProtectKernelLogs or ProtectKernelTunables: both mount over parts
    # of /proc, and the kernel then refuses the container its own /proc
    # ("Mount too revealing").
    ProtectControlGroups = true;
    RestrictRealtime = true;
    LockPersonality = true;
    UMask = "0077";
  };

  # The build's image, loaded unless Podman has it, and every older one
  # removed: a box keeps one copy.
  loadImage = pkgs.writeShellApplication {
    name = "losos-convert-image";
    runtimeInputs = [ podman ];
    text = ''
      if ! podman image exists ${lib.escapeShellArg image}; then
        podman load --quiet --input ${cfg.image}
      fi
      podman images --format '{{.Repository}}:{{.Tag}}' \
        | grep '^localhost/${cfg.image.imageName}:' \
        | grep -vxF ${lib.escapeShellArg image} \
        | xargs --no-run-if-empty podman rmi --force || true
    '';
  };
in
{
  config = lib.mkIf on {
    virtualisation.podman.enable = true;

    users.users.losos-convert = {
      isSystemUser = true;
      group = "losos-convert";
      inherit home;
      description = "LosOS Office's conversions";
    };
    users.groups.losos-convert = { };

    systemd.tmpfiles.rules = [
      "d ${home} 0700 losos-convert losos-convert -"
      "d ${home}/storage 0700 losos-convert losos-convert -"
      "d ${runtimeDir} 0700 losos-convert losos-convert -"
    ];

    systemd.sockets.losos-convert = {
      description = "LosOS Office's conversions, for lososd";
      wantedBy = [ "sockets.target" ];
      listenStreams = [ socket ];
      socketConfig = {
        Accept = true;
        SocketMode = "0600";
        # A conversion is a container; a few at once is what a small box
        # carries.
        MaxConnections = 4;
      };
    };

    # One conversion per connection: the socket is stdin and stdout.
    systemd.services."losos-convert@" = {
      description = "A LosOS Office conversion";
      after = [ "losos-convert-image.service" ];
      environment = podmanEnv;
      serviceConfig = sandbox // {
        ExecStart = lib.escapeShellArgs [
          (lib.getExe cfg.package)
          "serve"
          "--podman"
          "${podman}/bin/podman"
          "--image"
          image
          "--archive"
          "${cfg.image}"
        ];
        StandardInput = "socket";
        StandardOutput = "socket";
        StandardError = "journal";
        RuntimeMaxSec = "200s";
        MemoryMax = "1536M";
        TasksMax = 512;
        Nice = 5;
        LogLevelMax = "notice";
      };
    };

    # The image loaded at boot, so the first conversion does not wait for it.
    systemd.services.losos-convert-image = {
      description = "Load LosOS Office's conversion image into Podman";
      wantedBy = [ "multi-user.target" ];
      after = [ "systemd-tmpfiles-setup.service" ];
      environment = podmanEnv;
      serviceConfig = sandbox // {
        Type = "oneshot";
        RemainAfterExit = true;
        ExecStart = lib.getExe loadImage;
        TimeoutStartSec = "15min";
        Nice = 10;
      };
    };
  };
}
