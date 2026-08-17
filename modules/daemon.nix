# The lososd control-plane daemon + facade wiring.
#
# lososd runs as a root systemd service: it owns /var/lib/losos/state.json,
# runs supervised `systemd-run` rebuild transient units, exports the
# org.losos1 name on the system D-Bus (org.losos.Control1 interface: State /
# Settings / Change / Apply / FactoryReset / Status), and serves a
# Bearer-authed loopback JSON API on 127.0.0.1:<losos.admin.apiPort> —
# the same contract as backend/schema.json (see backend/src/Daemon.hs).
#
# The `losos-ctl` facade CLI (same package) relays subcommands over D-Bus, so
# nothing but the daemon ever rewrites the flake or triggers a rebuild. This
# module REPLACES the old sudoers bridge (nextcloud user -> sudo losos-ctl).
{
  pkgs,
  lib,
  config,
  ...
}:

let
  enabled = config.losos.backend.package != null;
  pkg = config.losos.backend.package;

  # System-bus policy: root owns org.losos1; members of the fixed `losos`
  # group may send to it (the facade runs as root or a losos-group caller).
  # Everyone else is denied.
  dbusPolicy = pkgs.writeTextDir "share/dbus-1/system.d/org.losos1.conf" ''
    <?xml version="1.0"?>
    <!DOCTYPE busconfig PUBLIC "-//freedesktop.org//DTD D-Bus Bus Configuration 1.0//EN"
      "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
    <busconfig>
      <policy user="root">
        <allow own="org.losos1"/>
        <allow send_destination="org.losos1"/>
      </policy>
      <policy group="losos">
        <allow send_destination="org.losos1"/>
      </policy>
      <policy context="default">
        <deny send_destination="org.losos1"/>
      </policy>
    </busconfig>
  '';
in
{
  config = lib.mkIf enabled {
    # Callers of the facade (root plus any future in-container tooling) send
    # to org.losos1 via this group.
    users.groups.losos = { };

    # The facade CLI for root; the daemon binary is started by the unit below.
    environment.systemPackages = [ pkg ];

    services.dbus.packages = [ dbusPolicy ];

    systemd.services.lososd = {
      description = "losos appliance control daemon (D-Bus + admin HTTP API)";
      wantedBy = [ "multi-user.target" ];
      after = [ "dbus.service" ];
      requires = [ "dbus.service" ];
      environment = {
        LOSOS_ADMIN_PORT = toString config.losos.admin.apiPort;
        LOSOS_ADMIN_TOKEN_FILE = toString config.losos.admin.tokenFile;
      };
      serviceConfig = {
        ExecStart = "${pkg}/bin/lososd";
        Restart = "on-failure";
        RestartSec = "2";
        # /var/lib/losos (state.json + rebuild.log) — persisted via
        # impermanence's whole-/var bind mount.
        StateDirectory = "losos";
        # The daemon runs supervised nixos-rebuild transient units and tails
        # the rebuild log; full root is the intent (it IS the appliance's
        # privileged core).
      };
    };
  };
}
