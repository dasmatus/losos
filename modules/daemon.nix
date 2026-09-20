# The lososd control-plane daemon + facade wiring.
#
# lososd runs as a root systemd service: it owns /var/lib/losos/state.json,
# runs supervised `systemd-run` rebuild transient units, exports the
# org.losos1 name on the system D-Bus (org.losos.Control1 interface: State /
# Settings / Change / Apply / FactoryReset / Status), and serves a
# Bearer-authed loopback JSON API on 127.0.0.1:<losos.admin.apiPort> —
# the same contract as backend/schema.json (see backend/src/dbus.rs for the
# bus interface and backend/src/http.rs for the HTTP surface).
#
# The `losos-ctl` facade CLI (same package) relays subcommands over D-Bus, so
# nothing but the daemon ever rewrites the flake or triggers a rebuild. This
# module replaces the old sudoers bridge (nextcloud user -> sudo losos-ctl).
{
  pkgs,
  lib,
  config,
  ...
}:

let
  enabled = config.losos.backend.package != null;
  pkg = config.losos.backend.package;

  # Which way `losos-ctl set-password` has to reach into Nextcloud.
  #
  # There is no default on the daemon's side and there must not be: lososd
  # treats an unset LOSOS_NEXTCLOUD_MODE as a hard error naming this module,
  # because guessing runs `crictl` against a box that has no containerd or
  # `nextcloud-occ` against a box whose closure never contained it — and both
  # failures surface as "the owner cannot set their password", on an appliance
  # with no shell to investigate from.
  ncMode = config.losos.nextcloud.mode;
  ncContainer = ncMode == "container";

  # modules/cluster.nix's `localCriSocket`, which is a `let` binding there and
  # so cannot be read from here. Repeated as a literal rather than promoted to
  # an option: it is containerd's compiled-in default and nothing in this repo
  # moves it. If it ever does move, it moves in both files or set-password
  # starts reporting that the pod is not running.
  #
  # The box's *own* k3s containerd. Deliberately not /run/k3s/…, which belongs
  # to the mesh rke2 agent — that one runs the edge's workloads and has never
  # heard of this box's Nextcloud.
  localCriSocket = "unix:///run/containerd/containerd.sock";

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
        # See the `ncMode` binding above for why this is always set and never
        # defaulted daemon-side.
        LOSOS_NEXTCLOUD_MODE = ncMode;
      }
      # Only in container mode: in native mode lososd never looks at it, and
      # setting it anyway would suggest a CRI socket is consulted on a box that
      # runs no containers.
      // lib.optionalAttrs ncContainer {
        LOSOS_CRI_SOCKET = localCriSocket;
      }
      # What `cryptsetup resize` authenticates with during `losos-ctl grow`.
      #
      # Only on the no-TPM path. With a TPM the volume key is in the kernel
      # keyring after the initrd unlock and cryptsetup finds it there; without
      # one, it would fall back to prompting on a stdin the daemon does not
      # have and fail *after* lvextend had already run.
      // lib.optionalAttrs (!config.losos.tpm.enable) {
        LOSOS_LUKS_KEYFILE = "/etc/keys/persist-keyfile";
      };
      # What `losos-ctl grow` shells out to. A systemd unit's default PATH does
      # not include these, and the failure is the unhelpful kind: the grow
      # reports "No such file or directory" for lvextend after the admin has
      # already been told the disk is full.
      #
      # coreutils is for `df`, which is how the daemon measures whether the
      # filesystem actually got bigger rather than trusting an exit status.
      #
      # `set-password` adds one entry per mode, and only the one that mode
      # uses. `path` *replaces* PATH rather than extending it, so neither
      # binary is reachable by accident: without the line below, the command
      # fails at the exec with ENOENT after it has already staged a plaintext
      # password on disk.
      path = [
        pkgs.lvm2
        pkgs.cryptsetup
        pkgs.e2fsprogs
        pkgs.coreutils
        # What `GET /api/apps/search` shells out to. The admin pages are served
        # under `connect-src 'self'`, so the browser cannot reach a catalogue
        # itself and lososd does the search — see backend/src/catalogue.rs for
        # why that is a subprocess rather than a client crate. Same failure
        # shape as the grow binaries above if this line goes: ENOENT at the
        # exec, reported to the owner as a search that will not come back.
        pkgs.curl
      ]
      # crictl, to find and exec into the Nextcloud workload container.
      ++ lib.optional ncContainer pkgs.cri-tools
      # The nixpkgs `nextcloud-occ` wrapper. Gated on native mode for more than
      # tidiness: in container mode services.nextcloud is never enabled, so
      # this attribute pulls a whole second Nextcloud closure into a system
      # that does not run one. `lib.optional false` does not force its
      # argument, so the attribute is not even evaluated there.
      ++ lib.optional (!ncContainer) config.services.nextcloud.occ;
      serviceConfig = {
        ExecStart = "${pkg}/bin/lososd";
        # `always`, not `on-failure`: a clean exit(0) — an unhandled shutdown
        # path, a bus disconnect the daemon treats as terminal — would
        # otherwise leave the box with no control plane and no way in (no SSH,
        # no shell logins). The rathole/registrar units use `always` for the
        # same reason.
        Restart = "always";
        RestartSec = "2";
        # /var/lib/losos (state.json + rebuild.log) — persisted via
        # impermanence's whole-/var bind mount. 0700: state.json records the
        # appliance's mode/settings and the rebuild log quotes the flake, and
        # systemd's default 0755 makes both world-readable.
        StateDirectory = "losos";
        StateDirectoryMode = "0700";

        # The daemon needs real root: it rewrites /etc/nixos and drives
        # `nixos-rebuild`. So this is not a sandbox, it is damage limitation on
        # the one process that terminates untrusted HTTP. The rebuild itself
        # runs in a `systemd-run` transient unit, which PID 1 starts outside
        # every namespace below — so none of these restrict it.
        #
        # ProtectHome is the load-bearing one: /home/notshared/data and
        # /home/shared/data are the two isolated data domains, and a compromised
        # request handler has no business reading either.
        ProtectHome = true;
        NoNewPrivileges = true;
        PrivateTmp = true;
        ProtectKernelTunables = true;
        LockPersonality = true;
      };
    };
  };
}
