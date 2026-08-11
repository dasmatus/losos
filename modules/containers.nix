# Containerized services + the Nginx front router.
#
# Active only when the corresponding losos.*.mode selects the container path:
#   losos.nextcloud.mode == "aio"      -> Nextcloud All-in-One (rootless Podman)
#   losos.forgejo.mode  == "container" -> Forgejo (rootless Podman, behind Nginx)
# The native services these replace live in services.nix, gated on the *other*
# mode value, so the two paths never collide and you flip the whole deployment
# back and forth by changing one option.
#
# Runtime: rootless Podman under the `containers` user (uid losos.containers.uid,
# declared in configuration.nix). Its home holds all image + volume storage
# (~/.local/share/containers) and is persisted via impermanence. The user's
# systemd runs at boot thanks to the linger marker below (no login needed), so
# the podman socket and the container services come up unattended.
#
# Container *declarations* live in Nix (systemd.user.services running `podman
# run`) — there is no compose YAML file. AIO is special: only its *master*
# container is declared here; the master itself spawns the actual
# Nextcloud/Postgres/Redis/Apache/Watchtower containers over the user podman
# socket (https://github.com/nextcloud/all-in-one).
#
# Nginx owns :80 (and :443 once TLS is added), freeing those ports from the
# native Nextcloud that used to bind :80. AIO's Apache (the real cloud) and
# Forgejo publish on host loopback at high ports; Nginx proxies to them.
{
  pkgs,
  lib,
  config,
  ...
}:

let
  cuser = config.losos.containers.user;
  cuid = toString config.losos.containers.uid;
  cgroup = cuser;
  # Rootless podman socket the AIO master drives sibling containers through.
  podmanSock = "/run/user/${cuid}/podman/podman.sock";
  podman = lib.getExe pkgs.podman;
  # The AIO management interface port (one-time initial setup: domain, TLS,
  # master password). LAN-only.
  interfacePort = toString config.losos.aio.interfacePort;
  apachePort = toString config.losos.aio.apachePort;
  forgejoPort = "3000"; # in-container; published to host 127.0.0.1:3000

  nextcloudAio = config.losos.nextcloud.mode == "aio";
  forgejoContainer = config.losos.forgejo.mode == "container";
  containerActive = nextcloudAio || forgejoContainer;
in
{
  # ── Rootless Podman runtime ──────────────────────────────────────────────
  # System podman is enabled only so the package, the subuid/subgid plumbing,
  # and the storage helpers are available; the containers themselves run
  # rootless under the `containers` user via systemd user services. We do NOT
  # enable dockerCompat (that would start a root-owned /var/run/docker.sock,
  # defeating the rootless intent) — AIO talks to the podman socket API
  # directly via the bind-mounted user socket, which is docker-API compatible.
  virtualisation.podman = lib.mkIf containerActive {
    enable = true;
    defaultNetwork.settings.dns_enabled = true;
  };

  # Lingering: the `containers` user's systemd must run at boot without anyone
  # logging in, so the podman socket + container services start unattended.
  # The marker lives under /var (persisted) but tmpfiles recreates it each
  # boot to be safe. (NixOS has no per-user `linger` option; this is the
  # canonical way.)
  systemd.tmpfiles.rules = lib.mkIf containerActive (
    [ "f /var/lib/systemd/linger/${cuser} 0644 root root - -" ]
    # AIO Nextcloud data dir on the host must be writable by the rootless
    # `containers` user (AIO bind-mounts it into the Nextcloud container).
    ++ lib.optional nextcloudAio "d ${config.losos.aio.datadir} 0750 ${cuser} ${cgroup} - -"
  );

  # ── Per-user podman socket (rootless) ─────────────────────────────────────
  # The user-scoped podman socket the AIO master container drives its siblings
  # through. systemd user units are installed for every user, but only the
  # `containers` user (the only one with lingering) actually reaches
  # sockets.target at boot, so only it starts the socket + containers.
  systemd.user.sockets.podman = lib.mkIf containerActive {
    description = "Podman API Socket (rootless, per-user)";
    wantedBy = [ "sockets.target" ];
    socketConfig = {
      ListenStream = "%t/podman/podman.sock";
      SocketMode = "0660";
    };
  };
  systemd.user.services.podman = lib.mkIf containerActive {
    description = "Podman API Service (rootless, per-user)";
    serviceConfig = {
      Type = "exec";
      ExecStart = "${podman} system service --time=0";
    };
  };

  # ── Nextcloud All-in-One (master container) ───────────────────────────────
  # Only the AIO *master* is declared; it spawns Nextcloud/Postgres/Redis/
  # Apache/Watchtower itself over the user podman socket. AIO rootless is
  # community-supported (not the official rootful docker path) — verify the
  # env against https://github.com/nextcloud/all-in-one rootless docs on first
  # deploy. First-run setup is done via the AIO interface on
  # losos.aio.interfacePort (LAN-only).
  systemd.user.services.nextcloud-aio = lib.mkIf nextcloudAio {
    description = "Nextcloud All-in-One master container (rootless Podman)";
    wantedBy = [ "default.target" ];
    after = [ "podman.socket" ];
    requires = [ "podman.socket" ];
    serviceConfig = {
      Type = "simple";
      # `--replace` recreates the container if one with that name exists
      # (named volumes — incl. nextcloud_aio_mastercontainer — persist).
      ExecStart = lib.concatStringsSep " " (
        [
          podman
          "run"
          "--name"
          "nextcloud-aio-mastercontainer"
          "--replace"
          "-e"
          "APACHE_PORT=${apachePort}"
          "-e"
          "APACHE_IP_BINDING=0.0.0.0"
          "-e"
          "NEXTCLOUD_DATADIR=${config.losos.aio.datadir}"
          "-e"
          "WATCHTOWER_DOCKER_SOCKET_PATH=/var/run/docker.sock"
          # `.local` is an mDNS name, not DNS — skip AIO's domain-resolution check
          # so the appliance can serve the cloud at mattbox.local without a DNS server.
          "-e"
          "SKIP_DOMAIN_VALIDATION=true"
          "-v"
          "${podmanSock}:/var/run/docker.sock"
          "-v"
          "nextcloud_aio_mastercontainer:/mnt/docker-aio-config"
          "-v"
          "${config.losos.aio.datadir}:${config.losos.aio.datadir}"
          "-p"
          "${interfacePort}:8000"
        ]
        # GPU passthrough: expose the host DRI devices and keep the `containers`
        # user's render/video supplementary groups inside the container so the
        # VA-API devices are accessible (requires losos.gpu.enable, which also
        # adds the host group memberships + graphics drivers).
        ++ lib.optionals config.losos.gpu.enable [
          "--device=/dev/dri"
          "--group-add=keep-groups"
        ]
        ++ [ "docker.io/nextcloud/all-in-one:latest" ]
      );
      ExecStop = "-${podman} stop -t 10 nextcloud-aio-mastercontainer";
      TimeoutStopSec = "30";
      Restart = "on-failure";
      RestartSec = "10";
    };
    unitConfig = {
      # The AIO master writes config into a named volume under the user's
      # podman storage; make sure the storage tree is ready before we start.
      RequiresMountsFor = "/home/${cuser}";
    };
  };

  # ── Forgejo (container) ───────────────────────────────────────────────────
  systemd.user.services.forgejo = lib.mkIf forgejoContainer {
    description = "Forgejo git host (rootless Podman)";
    wantedBy = [ "default.target" ];
    after = [ "podman.socket" ];
    requires = [ "podman.socket" ];
    serviceConfig = {
      Type = "simple";
      ExecStart = lib.concatStringsSep " " [
        podman
        "run"
        "--name"
        "forgejo"
        "--replace"
        # Publish to host loopback only; Nginx (a system service) fronts it on
        # :8888. Rootless binds a >1024 host port — fine.
        "-p"
        "127.0.0.1:${forgejoPort}:3000"
        "-v"
        "forgejo-data:/var/lib/gitea"
        "-e"
        "FORGEJO__server__ROOT_URL=http://mattbox.local:8888/"
        "-e"
        "FORGEJO__server__HTTP_PORT=3000"
        "codeberg.org/forgejo/forgejo:10"
      ];
      ExecStop = "-${podman} stop -t 10 forgejo";
      TimeoutStopSec = "30";
      Restart = "on-failure";
      RestartSec = "10";
    };
    unitConfig = {
      RequiresMountsFor = "/home/${cuser}";
    };
  };

  # ── Nginx front router ────────────────────────────────────────────────────
  # Owns :80 (the cloud) and :8888 (Forgejo), freeing those ports from the
  # native services. Each vhost proxies to its container's loopback port.
  services.nginx = lib.mkIf containerActive {
    enable = true;
    recommendedProxySettings = true;
    virtualHosts =
      (lib.optionalAttrs nextcloudAio {
        "mattbox.local" = {
          # The actual Nextcloud (AIO Apache). Large uploads / long-running
          # requests (sync, Talk) need the body + timeout bumps below.
          locations."/" = {
            proxyPass = "http://127.0.0.1:${apachePort}";
            proxyWebsockets = true;
            extraConfig = ''
              client_max_body_size 0;
              proxy_request_buffering off;
              proxy_read_timeout 86400s;
              proxy_send_timeout 86400s;
            '';
          };
        };
      })
      // (lib.optionalAttrs forgejoContainer {
        "forgejo" = {
          listen = [
            {
              addr = "0.0.0.0";
              port = 8888;
            }
          ];
          locations."/" = {
            proxyPass = "http://127.0.0.1:${forgejoPort}";
            proxyWebsockets = true;
          };
        };
      });
  };

  # ── Firewall ──────────────────────────────────────────────────────────────
  # Open the front-facing ports. The AIO interface port is opened so the
  # one-time setup UI is reachable on the LAN (no SSH, no other way in);
  # close it after initial setup if you prefer. The container loopback ports
  # (apachePort, forgejoPort) are NOT opened — Nginx reaches them locally.
  networking.firewall.allowedTCPPorts =
    lib.optional nextcloudAio 80
    ++ lib.optional forgejoContainer 8888
    ++ lib.optional nextcloudAio config.losos.aio.interfacePort;
}