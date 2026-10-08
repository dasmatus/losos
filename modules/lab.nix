# LosOS Lab's guests under libvirt on the box itself (losos.lab.libvirt.*).
#
# The Lab at /lab/ draws a setup and, where it can, boots one small x86_64
# guest per device. The copy the box serves has no qemu-wasm engine, so its
# consoles are simulated, unless this module runs `losos-registrar lab`
# (backend-registrar/src/lab/) next to libvirtd: then each guest is a
# transient libvirt domain, under KVM when the CPU has it.
#
# Three hops, each with its own guard:
#   * /api/lab/{hello,guests,virt-ticket} go nginx -> lososd (the admin
#     token, the throttle, the audit log) -> the helper, which lososd calls
#     with the same token. The helper reads it from the credential below.
#   * /api/lab/ws/<guest>/{console,nic/<n>} and /api/lab/virt go nginx ->
#     the helper directly (modules/containers.nix): a WebSocket through
#     actix would be a second relay for no gain. The helper admits a socket
#     only with a ticket: the per-guest one a create answered with, or one
#     of the two from virt-ticket (the relay's, single-use, and the network
#     card's for the page's own domain), and only a token holder gets any.
#     /api/lab/virt is a raw byte relay to libvirtd's socket for the Lab's
#     WebAssembly libvirt client, so it hands out libvirt at this unit's
#     uid; in the libvirtd group that is root-equivalent, no more than the
#     admin token already is.
#   * The helper's port gets the same owner match lososd's has
#     (modules/daemon.nix): the Nextcloud and Forgejo pods share this
#     loopback, and only root (lososd) and nginx may reach it.
#
# Off by default: libvirtd is a daemon the appliance has no other use for.
# Nothing here needs persisting beyond /var (libvirt's state and the images
# folder are under it), and the guests are transient, so a reboot leaves
# none behind.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.losos.lab.libvirt;
  registrar = config.losos.proxy.registrar.package;
  enabled = cfg.enable && registrar != null && config.losos.backend.package != null;
  port = toString cfg.port;
in
{
  config = lib.mkIf enabled {
    virtualisation.libvirtd.enable = true;

    users.users.losos-lab = {
      isSystemUser = true;
      group = "losos-lab";
      # libvirtd's socket group: qemu:///system without root.
      extraGroups = [ "libvirtd" ];
    };
    users.groups.losos-lab = { };

    # qemu reads the images as its own user; the folder is world-readable.
    systemd.tmpfiles.rules = [ "d ${cfg.images} 0755 root root -" ];

    systemd.services.losos-lab-libvirt = {
      description = "LosOS Lab guests under libvirt (losos-registrar lab)";
      wantedBy = [ "multi-user.target" ];
      # lososd mints the admin token on its first start; the credential
      # below cannot load before it exists.
      after = [
        "libvirtd.service"
        "lososd.service"
      ];
      wants = [ "libvirtd.service" ];
      # `virsh` is the only thing the helper runs. `path` replaces PATH.
      path = [ config.virtualisation.libvirtd.package ];
      serviceConfig = {
        User = "losos-lab";
        Group = "losos-lab";
        # The admin token, so lososd's relayed calls authenticate. Read from
        # the credential directory, never from the environment.
        LoadCredential = "token:${toString config.losos.admin.tokenFile}";
        ExecStart = lib.concatStringsSep " " [
          "${registrar}/bin/losos-registrar lab"
          "--listen 127.0.0.1:${port}"
          "--connect qemu:///system"
          "--images ${lib.escapeShellArg cfg.images}"
          "--token-file %d/token"
          "--max-guests ${toString cfg.maxGuests}"
          "--memory ${toString cfg.memoryMiB}"
        ];
        # SIGTERM destroys every guest before the helper exits.
        KillSignal = "SIGTERM";
        TimeoutStopSec = 60;
        Restart = "on-failure";
        RestartSec = 5;
        NoNewPrivileges = true;
        ProtectSystem = "strict";
        ProtectHome = true;
        PrivateTmp = true;
      };
    };

    systemd.services.lososd.environment.LOSOS_LAB_URL = "http://127.0.0.1:${port}";

    networking.firewall =
      let
        rule = lib.concatStringsSep " " (
          [
            "OUTPUT -o lo -p tcp -d 127.0.0.1 --dport ${port}"
            "-m owner ! --uid-owner 0"
          ]
          ++ lib.optional config.services.nginx.enable "-m owner ! --uid-owner ${config.services.nginx.user}"
          ++ [ "-j REJECT --reject-with tcp-reset" ]
        );
      in
      lib.mkIf config.networking.firewall.enable {
        extraCommands = ''
          iptables -w -D ${rule} 2>/dev/null || true
          iptables -w -I ${rule}
        '';
        extraStopCommands = ''
          iptables -w -D ${rule} 2>/dev/null || true
        '';
      };
  };
}
