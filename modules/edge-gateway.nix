# The edge gateway: a losos edge preset for a machine that sits on a LAN
# beside the boxes and needs no provisioning before it is useful
# (handbook/docs/in-depth/edge-federation.md, "Delivery").
#
# `losos.edge.gateway.enable` turns the edge module into the gateway: it
# advertises itself on the LAN, enrols any box that finds it trust-on-first-use
# (so a box installed from the stock ISO is published through it within a
# minute of booting), and relays those boxes to a hub, an official edge, as
# soon as its owner has entered the hub's tenant row on the manage page. The
# uplink is read from a runtime file, which is what lets one disk image serve
# every site: the image carries no secret and no site name.
#
# Like the appliance, the gateway has no shell: no console login on any
# terminal, no SSH, no root password. Its screen shows where its web UI is
# (`<lan.url>/manage/`) and the key that signs in there; whoever can read the
# screen is standing at the machine. Everything its owner does happens on
# that page (edge/registrar/src/server/manage/gateway.rs): the owner's signed
# settings, the hub, forgetting a box that left.
#
# `flake.nix` builds it two ways: `nixosConfigurations.edge-gateway` plus the
# QCOW2 in nix/disk-images.nix (`nix build .#losos-disk-edge-qcow2`, attached
# to every tagged release), and `nixosModules.edge` for an existing NixOS
# machine, which imports this file and sets the one option. The option takes
# that machine's logins away too: run the plain edge (`losos.edge.enable`)
# on a machine that should keep them.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.losos.edge.gateway;
  edge = config.losos.edge;

  # Runtime state the gateway owns: the uplink the owner entered on the
  # manage page, with the hub's tokens beside it.
  stateDir = "/var/lib/losos-edge";
  uplinkJson = "${stateDir}/uplink.json";
  uplinkToml = "/etc/rathole/uplink.toml";
  manageKey = "/var/lib/losos-registrar/manage-key";
  manageUrl = "${edge.lan.url}/manage/";

  # What tty1 shows: where the web UI is and the key that signs in there.
  # Redrawn when the address, the key or the uplink changes (DHCP arrives
  # some seconds after boot, the key once the registrar has made it).
  screen = pkgs.writeShellApplication {
    name = "losos-edge-screen";
    runtimeInputs = [
      pkgs.coreutils
      pkgs.gawk
      pkgs.gnugrep
      pkgs.iproute2
    ];
    text = ''
      addresses() {
        ip -4 -o addr show scope global 2>/dev/null \
          | awk '{ split($4, a, "/"); print a[1] }' \
          | grep -Ev '^(10\.0\.2\.|169\.254\.)' || true
      }
      draw() {
        printf '\033[2J\033[H'
        printf '\n  LosOS edge gateway\n\n'
        printf '  Open this in a browser on the same network:\n\n'
        printf '      %s\n' ${lib.escapeShellArg manageUrl}
        for ip in $(addresses); do
          printf '      http://%s:%s/manage/\n' "$ip" ${toString edge.registrarApiPort}
        done
        printf '\n  Sign in there with this key:\n\n'
        if [ -s ${manageKey} ]; then
          printf '      %s\n' "$(tr -d '\n' < ${manageKey} | fold -w 16 | paste -sd ' ')"
        else
          printf '      (the gateway is still starting)\n'
        fi
        printf '\n'
        if [ -s ${uplinkJson} ]; then
          printf '  Boxes here are relayed through a hub.\n'
        else
          printf '  Boxes on this network find the gateway on their own.\n'
          printf '  To reach them from anywhere, set a hub on that page.\n'
        fi
        printf '\n  This machine has no console login and no SSH.\n'
      }
      last=""
      while true; do
        now="$(addresses | tr '\n' ' ')|$(cat ${manageKey} 2>/dev/null || true)|$([ -s ${uplinkJson} ] && echo up)"
        if [ "$now" != "$last" ]; then
          draw
          last="$now"
        fi
        sleep 5
      done
    '';
  };

in
{
  options.losos.edge.gateway.enable = lib.mkOption {
    type = lib.types.bool;
    default = false;
    description = ''
      Make this machine a LosOS edge gateway
      (handbook/docs/in-depth/edge-federation.md): the edge module with LAN
      advertising, open enrolment and the manage page on, the uplink read
      from ${uplinkJson} (written from the manage page), and no shell: no
      console login, no SSH, no root password. Its screen shows the manage
      page's address and key. The shape of the `losos-disk-edge-qcow2`
      image; import nixosModules.edge and set this on an existing NixOS
      machine for the same thing without the image, which takes that
      machine's logins away.
    '';
  };

  config = lib.mkIf cfg.enable {
    losos.edge = {
      enable = true;
      lan.advertise = lib.mkDefault true;
      lan.openEnrolment = lib.mkDefault true;
      manage.enable = lib.mkDefault true;
      passport.enable = lib.mkDefault true;
      uplink.enable = lib.mkDefault true;
      uplink.configFile = lib.mkDefault uplinkJson;
      # Traefik's ACME needs a public name this machine does not have; the
      # failure is logged and the LAN routes work regardless, as in the LAN
      # demo. The address is a placeholder for an account that is never made.
      acmeEmail = lib.mkDefault "gateway@losos.cfd";
    };

    # ── Secrets the edge module expects to find ──────────────────────────
    # The rathole bootstrap token is a transport secret every box shares
    # with the edge; with open enrolment no box knows it in advance, and
    # every box service carries its own token, so its value is immaterial.
    # Generated once, before the seed and the registrar read it, so the image
    # boots with no provisioning step and no two gateways share a token.
    systemd.services.losos-edge-gateway-secrets = {
      description = "losos edge gateway — first-boot secrets";
      wantedBy = [ "multi-user.target" ];
      before = [
        "losos-rathole-seed.service"
        "losos-registrar.service"
      ];
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
      };
      path = [ pkgs.coreutils ];
      script = ''
        set -eu
        umask 077
        install -d -m 0700 /var/secrets ${stateDir}
        token=${toString edge.bootstrapTokenFile}
        if [ ! -s "$token" ]; then
          head -c 32 /dev/urandom | od -An -tx1 | tr -d ' \n' > "$token.new"
          mv "$token.new" "$token"
        fi
      '';
    };

    # ── No shell ─────────────────────────────────────────────────────────
    # No login prompt on any terminal, serial line or container console,
    # no SSH, and a root account with no password: the manage page is the
    # only way in. The emergency and rescue shells go too, so a failed boot
    # is fixed by the image, not by hand.
    systemd.services."getty@".enable = false;
    systemd.services."autovt@".enable = false;
    systemd.services."serial-getty@".enable = false;
    systemd.services."container-getty@".enable = false;
    services.logind.settings.Login = {
      NAutoVTs = "0";
      ReserveVT = "0";
    };
    services.openssh.enable = false;
    users.mutableUsers = lib.mkDefault false;
    users.users.root.hashedPassword = "!";
    # Locked out on purpose: the manage page is the way in.
    users.allowNoPasswordLogin = true;
    systemd.enableEmergencyMode = false;
    boot.initrd.systemd.emergencyAccess = false;
    boot.kernel.sysctl."kernel.sysrq" = lib.mkDefault 0;

    # What tty1 shows instead (`screen` above). Only errors reach the
    # console: at the default level the kernel's informational messages
    # (VLAN filters, regulatory database) land on top of it.
    boot.consoleLogLevel = lib.mkDefault 3;
    systemd.services.losos-edge-screen = {
      description = "Show the gateway's web UI and key on tty1";
      wantedBy = [ "multi-user.target" ];
      after = [
        "systemd-user-sessions.service"
        "systemd-vconsole-setup.service"
        "losos-registrar.service"
      ];
      serviceConfig = {
        ExecStart = lib.getExe screen;
        Restart = "always";
        RestartSec = 2;
        StandardInput = "tty";
        StandardOutput = "tty";
        StandardError = "journal";
        TTYPath = "/dev/tty1";
        TTYReset = true;
        TTYVHangup = true;
        TTYVTDisallocate = true;
      };
    };

    # The manage page writes the uplink; the tunnel follows it. A hub that
    # changed under a running tunnel needs a restart (its pinned key went
    # with the old row), and a cleared uplink stops it.
    systemd.paths.losos-edge-uplink-changed = {
      description = "follow the uplink the manage page writes";
      wantedBy = [ "multi-user.target" ];
      pathConfig.PathChanged = stateDir;
    };
    systemd.services.losos-edge-uplink-changed = {
      description = "restart or stop the uplink tunnel after the manage page changed it";
      serviceConfig.Type = "oneshot";
      path = [ pkgs.systemd ];
      script = ''
        if [ -s ${uplinkJson} ]; then
          systemctl try-restart losos-rathole-uplink.service
        else
          rm -f ${uplinkToml}
          systemctl stop losos-rathole-uplink.service
        fi
      '';
    };

    # Resolve the boxes' `.local` names on the gateway itself, even with
    # lan.advertise turned off (modules/mdns.nix).
    losos.mdns.enable = true;
  };
}
