# LosOS Git: the box's own Git server (`losos-git`, backend/src/forge/, a
# binary of lososd's crate).
#
# One service, on the host, as its own user:
#
#   /run/losos-git/http.sock   nginx's way in (modules/containers.nix,
#                              `/git/`). The only listener that believes the
#                              person LosOS Login names in the
#                              X-Losos-Cloud-* headers (modules/app-sign-in.nix),
#                              so it is 0660 and nginx is the only other
#                              member of the group.
#   127.0.0.1:3000             tokens only, never the headers. lososd keeps
#                              the configuration repository and makes
#                              computers' tokens through it with the admin
#                              token LosOS Git writes on its first start
#                              (/var/lib/losos-git/admin-token, read by
#                              lososd as root).
#
# Repositories are bare, at /var/lib/losos-git/repositories/<owner>/<name>.git,
# the layout Forgejo used, and the accounts, tokens and repository records
# are one SQLite file beside them. A box that ran Forgejo has its
# repositories moved over on the first start (`migrate` below), and the
# server takes in every one it finds without a record.
#
# /var/lib is on /persist through impermanence's whole-/var entry, so
# nothing else is listed for it.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.losos.git;
  pkg = config.losos.backend.package;
  on = cfg.enable && pkg != null;

  stateDir = "/var/lib/losos-git";
  # Literals, repeated in modules/containers.nix (the socket) and
  # modules/config-repo.nix (the port, the token file): nothing off the box
  # ever sees either, so they are not options.
  socket = "/run/losos-git/http.sock";
  port = 3000;
  # uid and gid Forgejo had, so its repositories keep their owner.
  id = 1003;

  git = pkgs.gitMinimal;

  # What Forgejo left: its repositories, moved once, before anything else
  # writes here. Everything else of Forgejo's (its database, its sessions,
  # its avatars) stays where it was and is no longer read.
  migrate = pkgs.writeShellScript "losos-git-migrate" ''
    set -eu
    old=/var/lib/forgejo/repositories
    new=${stateDir}/repositories
    if [ -d "$old" ] && [ ! -e "$new" ]; then
      echo "losos-git: moving Forgejo's repositories into LosOS Git" >&2
      mv -- "$old" "$new"
    fi
    if [ -d "$new" ]; then
      chown -R ${toString id}:${toString id} -- "$new"
    fi
  '';
in
{
  config = lib.mkIf on {
    users.groups.losos-git.gid = id;
    users.users.losos-git = {
      uid = id;
      group = "losos-git";
      isSystemUser = true;
      description = "LosOS Git";
    };
    # The socket's group: nginx and nobody else.
    users.users.${config.services.nginx.user}.extraGroups = lib.mkIf config.services.nginx.enable [
      "losos-git"
    ];

    systemd.services.losos-git = {
      description = "LosOS Git";
      wantedBy = [ "multi-user.target" ];
      after = [ "network.target" ];
      # nginx opens the socket; it waits for nothing of ours to start.
      before = lib.optional config.services.nginx.enable "nginx.service";
      path = [ git ];
      environment.RUST_LOG = "info";
      # Keep trying: on the first switch from a Forgejo generation the old
      # pod can hold port 3000 for a moment (modules/workloads.nix).
      startLimitIntervalSec = 0;
      serviceConfig = {
        User = "losos-git";
        Group = "losos-git";
        ExecStartPre = [ "+${migrate}" ];
        ExecStart = lib.escapeShellArgs (
          [
            "${pkg}/bin/losos-git"
            "serve"
            "--state-dir"
            stateDir
            "--socket"
            socket
            "--listen"
            "127.0.0.1:${toString port}"
            "--git"
            "${git}/bin/git"
          ]
          ++ lib.optional cfg.site.requireSignIn "--require-sign-in"
          ++ lib.optional (cfg.site.defaultPrivate == "public") "--public-by-default"
          ++ lib.optional cfg.site.pushCreate "--push-create"
        );
        Restart = "on-failure";
        RestartSec = "2s";
        StateDirectory = "losos-git";
        StateDirectoryMode = "0700";
        RuntimeDirectory = "losos-git";
        # nginx (in the group) has to reach the socket inside.
        RuntimeDirectoryMode = "0750";
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
        ];
        RestrictNamespaces = true;
        RestrictRealtime = true;
        RestrictSUIDSGID = true;
        LockPersonality = true;
        SystemCallArchitectures = "native";
        SystemCallFilter = [
          "@system-service"
          "~@privileged"
        ];
      };
    };
  };
}
